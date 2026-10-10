use crate::{
    common::{ApiResult, AppError},
    middleware::auth::CurrentUser,
    state::AppState,
};
use axum::http::StatusCode;
use bytes::Bytes;
use object_store::{ObjectStore, path::Path};
use serde_json::Value;
use sha2::{Digest, Sha256};
use sqlx::Row;
use uuid::Uuid;
pub const MAX_FILE: usize = 5 * 1024 * 1024;
pub fn too_large() -> AppError {
    AppError::new(
        StatusCode::PAYLOAD_TOO_LARGE,
        "FILE_TOO_LARGE",
        "付款凭证不能超过 5 MiB",
    )
}
pub struct ValidatedImage {
    pub content_type: &'static str,
    pub width: u32,
    pub height: u32,
}
pub fn validate_receipt(bytes: &[u8]) -> ApiResult<ValidatedImage> {
    if bytes.len() > MAX_FILE {
        return Err(too_large());
    }
    let format = image::guess_format(bytes)
        .map_err(|_| AppError::invalid("仅支持完整的 PNG 或 JPEG 图片"))?;
    let content_type = match format {
        image::ImageFormat::Png => "image/png",
        image::ImageFormat::Jpeg => "image/jpeg",
        _ => return Err(AppError::invalid("仅支持 PNG 或 JPEG 图片")),
    };
    let reader = || image::ImageReader::with_format(std::io::Cursor::new(bytes), format);
    let (width, height) = reader()
        .into_dimensions()
        .map_err(|_| AppError::invalid("图片无法读取"))?;
    if width == 0
        || height == 0
        || width > 8000
        || height > 8000
        || u64::from(width) * u64::from(height) > 20_000_000
    {
        return Err(AppError::invalid("图片尺寸超过限制"));
    }
    let mut decoder = reader();
    let mut limits = image::Limits::default();
    limits.max_image_width = Some(8000);
    limits.max_image_height = Some(8000);
    limits.max_alloc = Some(100 * 1024 * 1024);
    decoder.limits(limits);
    decoder
        .decode()
        .map_err(|_| AppError::invalid("图片不完整或无法解码"))?;
    Ok(ValidatedImage {
        content_type,
        width,
        height,
    })
}
pub async fn check_order(state: &AppState, actor: &CurrentUser, id: Uuid) -> ApiResult<()> {
    let row =
        sqlx::query("SELECT status,expires_at>NOW() AS valid FROM orders WHERE id=? AND user_id=?")
            .bind(id)
            .bind(actor.id)
            .fetch_optional(&state.pool)
            .await?
            .ok_or_else(AppError::missing)?;
    if row.get::<String, _>("status") != "pending_payment" || !row.get::<bool, _>("valid") {
        return Err(AppError::conflict("订单已过期或不再等待付款"));
    }
    Ok(())
}
pub async fn submit_proof(
    state: &AppState,
    actor: &CurrentUser,
    order: Uuid,
    bytes: Bytes,
) -> ApiResult<Value> {
    let permit = state
        .upload_slots
        .clone()
        .acquire_owned()
        .await
        .map_err(|_| AppError::unavailable())?;
    let data = bytes.clone();
    let validated = tokio::task::spawn_blocking(move || validate_receipt(&data))
        .await
        .map_err(|_| AppError::internal())??;
    let file = Uuid::new_v4();
    let key = format!("quarantine/{}/{}", actor.id, file);
    sqlx::query("INSERT INTO file_objects(id,owner_id,purpose,storage_key,content_type,size_bytes,sha256,status) VALUES(?,?,'payment_proof',?,?,?,?,'quarantined')").bind(file).bind(actor.id).bind(&key).bind(validated.content_type).bind(bytes.len() as i64).bind(format!("{:x}",Sha256::digest(&bytes))).execute(&state.pool).await?;
    state
        .storage
        .put(&Path::from(key), bytes.clone().into())
        .await?;
    drop(permit);
    let mut tx = state.pool.begin().await?;
    let row = sqlx::query(
        "SELECT status,expires_at>NOW() AS valid FROM orders WHERE id=? AND user_id=? FOR UPDATE",
    )
    .bind(order)
    .bind(actor.id)
    .fetch_optional(&mut *tx)
    .await?
    .ok_or_else(AppError::missing)?;
    if row.get::<String, _>("status") != "pending_payment" || !row.get::<bool, _>("valid") {
        return Err(AppError::conflict("订单已过期或不再等待付款"));
    }
    let proof_id = Uuid::new_v4();
    sqlx::query("INSERT INTO payment_submissions(id,order_id,file_id) VALUES(?,?,?)")
        .bind(proof_id)
        .bind(order)
        .bind(file)
        .execute(&mut *tx)
        .await?;
    let proof:Value=sqlx::query_scalar("SELECT JSON_OBJECT('id',BIN_TO_UUID(id),'order_id',BIN_TO_UUID(order_id),'status',status,'created_at',created_at) FROM payment_submissions WHERE id=?").bind(proof_id).fetch_one(&mut *tx).await?;
    sqlx::query("UPDATE orders SET status='payment_review' WHERE id=?")
        .bind(order)
        .execute(&mut *tx)
        .await?;
    tx.commit().await?;
    Ok(proof)
}
pub struct PrivateFile {
    pub content_type: String,
    pub bytes: Bytes,
}
pub async fn proof_bytes(
    state: &AppState,
    actor: &CurrentUser,
    proof: Uuid,
    order: Option<Uuid>,
) -> ApiResult<PrivateFile> {
    let row=sqlx::query("SELECT f.storage_key,f.content_type FROM payment_submissions p JOIN orders o ON o.id=p.order_id JOIN file_objects f ON f.id=p.file_id WHERE p.id=? AND f.status IN ('quarantined','clean') AND ((? IS NULL AND ?) OR (o.id=? AND o.user_id=?))").bind(proof).bind(order).bind(actor.is_admin).bind(order).bind(actor.id).fetch_optional(&state.pool).await?.ok_or_else(AppError::missing)?;
    let bytes = state
        .storage
        .get(&Path::from(row.get::<String, _>("storage_key")))
        .await?
        .bytes()
        .await?;
    Ok(PrivateFile {
        content_type: row.get("content_type"),
        bytes,
    })
}
