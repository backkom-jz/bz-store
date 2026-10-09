use super::{
    dto::{ApproveInput, RejectInput},
    service, upload,
};
use crate::{
    common::{
        ApiResult, AppError,
        extractors::{ApiJson, ApiPath, ApiQuery},
        ok,
        pagination::ListQuery,
    },
    infrastructure::cache,
    middleware::auth::{AdminUser, CurrentUser},
    state::AppState,
};
use axum::{
    Router,
    extract::{DefaultBodyLimit, Multipart, State},
    http::{StatusCode, header},
    response::IntoResponse,
    routing::{get, post},
};
use uuid::Uuid;
pub fn router() -> Router<AppState> {
    Router::new()
        .route(
            "/api/v1/orders/{id}/payment-proof",
            post(submit).layer(DefaultBodyLimit::max(6 * 1024 * 1024)),
        )
        .route("/api/v1/orders/{id}/payment-proof/{proof}", get(own_proof))
        .route("/api/v1/admin/payments/{proof}/proof", get(admin_proof))
        .route("/api/v1/admin/payments", get(list))
        .route("/api/v1/admin/payments/{proof}/approve", post(approve))
        .route("/api/v1/admin/payments/{proof}/reject", post(reject))
}
async fn list(
    State(s): State<AppState>,
    _: AdminUser,
    ApiQuery(q): ApiQuery<ListQuery>,
) -> ApiResult<impl IntoResponse> {
    Ok(ok(service::list(&s, q.validate()?).await?))
}
async fn approve(
    State(s): State<AppState>,
    AdminUser(u): AdminUser,
    ApiPath(id): ApiPath<Uuid>,
    ApiJson(input): ApiJson<ApproveInput>,
) -> ApiResult<impl IntoResponse> {
    Ok(ok(service::approve(&s, &u, id, input).await?))
}
async fn reject(
    State(s): State<AppState>,
    AdminUser(u): AdminUser,
    ApiPath(id): ApiPath<Uuid>,
    ApiJson(input): ApiJson<RejectInput>,
) -> ApiResult<impl IntoResponse> {
    Ok(ok(service::reject(&s, &u, id, &input.reason).await?))
}
fn multipart_error(error: axum::extract::multipart::MultipartError) -> AppError {
    if error.status() == StatusCode::PAYLOAD_TOO_LARGE {
        upload::too_large()
    } else {
        AppError::invalid("上传格式不正确，请选择一张图片")
    }
}
async fn submit(
    State(s): State<AppState>,
    u: CurrentUser,
    ApiPath(id): ApiPath<Uuid>,
    multipart: Result<Multipart, axum::extract::multipart::MultipartRejection>,
) -> ApiResult<impl IntoResponse> {
    upload::check_order(&s, &u, id).await?;
    cache::limit(&s, "proof", &u.id.to_string(), 10, 3600).await?;
    let mut form = multipart.map_err(|_| AppError::invalid("需要 multipart 图片上传"))?;
    let mut field = form
        .next_field()
        .await
        .map_err(multipart_error)?
        .ok_or_else(|| AppError::invalid("请选择图片"))?;
    if field.name() != Some("file") {
        return Err(AppError::invalid("只接受 file 字段"));
    }
    let mut data = bytes::BytesMut::new();
    while let Some(chunk) = field.chunk().await.map_err(multipart_error)? {
        if data.len() + chunk.len() > upload::MAX_FILE {
            return Err(upload::too_large());
        }
        data.extend_from_slice(&chunk);
    }
    drop(field);
    if form.next_field().await.map_err(multipart_error)?.is_some() {
        return Err(AppError::invalid("每次只接受一张图片"));
    }
    Ok((
        StatusCode::CREATED,
        ok(upload::submit_proof(&s, &u, id, data.freeze()).await?),
    ))
}
async fn own_proof(
    State(s): State<AppState>,
    u: CurrentUser,
    ApiPath((id, proof)): ApiPath<(Uuid, Uuid)>,
) -> ApiResult<impl IntoResponse> {
    let f = upload::proof_bytes(&s, &u, proof, Some(id)).await?;
    Ok(([(header::CONTENT_TYPE, f.content_type)], f.bytes))
}
async fn admin_proof(
    State(s): State<AppState>,
    AdminUser(u): AdminUser,
    ApiPath(id): ApiPath<Uuid>,
) -> ApiResult<impl IntoResponse> {
    let f = upload::proof_bytes(&s, &u, id, None).await?;
    Ok(([(header::CONTENT_TYPE, f.content_type)], f.bytes))
}
