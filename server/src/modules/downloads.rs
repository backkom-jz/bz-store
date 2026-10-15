use crate::{
    common::{ApiResult, AppError, extractors::ApiPath, ok},
    middleware::auth::{AdminUser, CurrentUser},
    state::AppState,
};
use axum::{
    Router,
    extract::{DefaultBodyLimit, Multipart, State},
    http::{HeaderMap, StatusCode, header},
    response::IntoResponse,
    routing::{get, post},
};
use bytes::Bytes;
use object_store::{ObjectStore, path::Path};
use sha2::{Digest, Sha256};
use sqlx::Row;
use uuid::Uuid;

const MAX_FILE: usize = 5 * 1024 * 1024;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/api/v1/admin/products/{id}/file", post(attach).layer(DefaultBodyLimit::max(6 * 1024 * 1024)))
        .route("/api/v1/me/downloads/{id}", get(download))
}

fn classify(bytes: &[u8]) -> ApiResult<&'static str> {
    if bytes.len() > MAX_FILE || bytes.is_empty() {
        return Err(AppError::new(StatusCode::PAYLOAD_TOO_LARGE, "FILE_TOO_LARGE", "数字商品文件不能超过 5 MiB"));
    }
    if bytes.starts_with(b"%PDF-") {
        return Ok("application/pdf");
    }
    if bytes.starts_with(b"PK\x03\x04") {
        return Ok("application/zip");
    }
    Err(AppError::invalid("仅支持 PDF 或 ZIP 文件"))
}

async fn attach(
    State(state): State<AppState>,
    actor: AdminUser,
    ApiPath(product_id): ApiPath<Uuid>,
    multipart: Result<Multipart, axum::extract::multipart::MultipartRejection>,
) -> ApiResult<impl IntoResponse> {
    let mut form = multipart.map_err(|_| AppError::invalid("需要 multipart 文件上传"))?;
    let mut payload: Option<Bytes> = None;
    while let Some(mut field) = form.next_field().await.map_err(|_| AppError::invalid("文件无法读取"))? {
        if field.name() != Some("file") {
            continue;
        }
        let mut data = bytes::BytesMut::new();
        while let Some(chunk) = field.chunk().await.map_err(|error| {
            if error.status() == StatusCode::PAYLOAD_TOO_LARGE {
                AppError::new(StatusCode::PAYLOAD_TOO_LARGE, "FILE_TOO_LARGE", "数字商品文件不能超过 5 MiB")
            } else {
                AppError::invalid("文件无法读取")
            }
        })? {
            if data.len() + chunk.len() > MAX_FILE {
                return Err(AppError::new(StatusCode::PAYLOAD_TOO_LARGE, "FILE_TOO_LARGE", "数字商品文件不能超过 5 MiB"));
            }
            data.extend_from_slice(&chunk);
        }
        payload = Some(data.freeze());
    }
    let bytes = payload.ok_or_else(|| AppError::invalid("请选择要上传的文件"))?;
    let content_type = classify(&bytes)?;
    let mut tx = state.pool.begin().await?;
    let kind: Option<String> = sqlx::query_scalar("SELECT type FROM products WHERE id=? FOR UPDATE").bind(product_id).fetch_optional(&mut *tx).await?;
    let Some(kind) = kind else { return Err(AppError::missing()); };
    if kind != "digital" {
        return Err(AppError::invalid("只有数字商品可以上传交付文件"));
    }
    let sold: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM order_items WHERE product_id=?)").bind(product_id).fetch_one(&mut *tx).await?;
    if sold {
        return Err(AppError::conflict("已有订单的商品不能更换文件"));
    }
    let file_id = Uuid::new_v4();
    let key = format!("digital/{file_id}");
    sqlx::query("INSERT INTO file_objects(id,owner_id,purpose,storage_key,content_type,size_bytes,sha256,status) VALUES(?,?,'digital_asset',?,?,?,?,'clean')")
        .bind(file_id).bind(actor.0.id).bind(&key).bind(content_type).bind(bytes.len() as i64).bind(format!("{:x}", Sha256::digest(&bytes)))
        .execute(&mut *tx).await?;
    sqlx::query("INSERT INTO product_files(id,product_id,file_id) VALUES(?,?,?) ON DUPLICATE KEY UPDATE file_id=VALUES(file_id)")
        .bind(Uuid::new_v4()).bind(product_id).bind(file_id).execute(&mut *tx).await?;
    crate::modules::admin::audit::record(&mut tx, actor.0.id, "product.file", product_id, serde_json::json!({"file_id": file_id, "content_type": content_type, "size_bytes": bytes.len()})).await?;
    state.storage.put(&Path::from(key), bytes.into()).await.map_err(|_| AppError::unavailable())?;
    tx.commit().await?;
    Ok((StatusCode::CREATED, ok(serde_json::json!({"file_id": file_id, "content_type": content_type}))))
}

async fn download(State(state): State<AppState>, actor: CurrentUser, ApiPath(id): ApiPath<Uuid>) -> ApiResult<impl IntoResponse> {
    let row = sqlx::query("SELECT f.storage_key,f.content_type,p.slug,e.id AS entitlement_id FROM product_files pf JOIN file_objects f ON f.id=pf.file_id JOIN products p ON p.id=pf.product_id JOIN user_entitlements e ON e.product_id=pf.product_id AND e.user_id=? AND e.status='active' WHERE pf.file_id=? LIMIT 1")
        .bind(actor.id).bind(id).fetch_optional(&state.pool).await?.ok_or_else(AppError::missing)?;
    let stored = state.storage.get(&Path::from(row.get::<String, _>("storage_key"))).await.map_err(|_| AppError::missing())?;
    let bytes = stored.bytes().await.map_err(|_| AppError::unavailable())?;
    sqlx::query("INSERT INTO download_logs(id,user_id,file_id,entitlement_id) VALUES(?,?,?,?)")
        .bind(Uuid::new_v4()).bind(actor.id).bind(id).bind(row.get::<Uuid, _>("entitlement_id")).execute(&state.pool).await?;
    let content_type = row.get::<String, _>("content_type");
    let extension = if content_type == "application/pdf" { "pdf" } else { "zip" };
    let filename = format!("{}.{}", row.get::<String, _>("slug"), extension);
    let mut headers = HeaderMap::new();
    headers.insert(header::CONTENT_TYPE, content_type.parse().map_err(|_| AppError::internal())?);
    headers.insert(header::CONTENT_DISPOSITION, format!("attachment; filename=\"{filename}\"").parse().map_err(|_| AppError::internal())?);
    headers.insert(header::CACHE_CONTROL, "private, no-store".parse().map_err(|_| AppError::internal())?);
    headers.insert("x-content-type-options", "nosniff".parse().map_err(|_| AppError::internal())?);
    Ok((headers, bytes))
}
