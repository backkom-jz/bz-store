use super::{dto::CreateOrderInput, service};
use crate::{
    common::{
        ApiResult, AppError,
        extractors::{ApiJson, ApiPath, ApiQuery},
        ok,
        pagination::ListQuery,
    },
    middleware::auth::{AdminUser, CurrentUser},
    state::AppState,
};
use axum::{
    Router,
    extract::State,
    http::{HeaderMap, StatusCode},
    response::IntoResponse,
    routing::{get, post},
};
use serde_json::Value;
use uuid::Uuid;
pub fn router() -> Router<AppState> {
    Router::new()
        .route("/api/v1/orders", get(list).post(create))
        .route("/api/v1/orders/{id}", get(detail))
        .route("/api/v1/orders/{id}/cancel", post(cancel))
        .route(
            "/api/v1/orders/{id}/payment-instructions",
            get(instructions),
        )
        .route("/api/v1/admin/orders", get(admin_list))
        .route("/api/v1/admin/orders/{id}", get(admin_detail))
}
async fn create(
    State(s): State<AppState>,
    u: CurrentUser,
    h: HeaderMap,
    ApiJson(v): ApiJson<CreateOrderInput>,
) -> ApiResult<impl IntoResponse> {
    let key = h
        .get("idempotency-key")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");
    Ok((
        StatusCode::CREATED,
        ok(service::create(&s, &u, key, v).await?),
    ))
}
async fn detail(
    State(s): State<AppState>,
    u: CurrentUser,
    ApiPath(id): ApiPath<Uuid>,
) -> ApiResult<impl IntoResponse> {
    Ok(ok(service::get(&s, &u, id).await?))
}
async fn cancel(
    State(s): State<AppState>,
    u: CurrentUser,
    ApiPath(id): ApiPath<Uuid>,
) -> ApiResult<impl IntoResponse> {
    Ok(ok(service::cancel(&s, &u, id).await?))
}
async fn list(
    State(s): State<AppState>,
    u: CurrentUser,
    ApiQuery(q): ApiQuery<ListQuery>,
) -> ApiResult<impl IntoResponse> {
    Ok(ok(service::list(&s, Some(u.id), q.validate()?).await?))
}
async fn admin_list(
    State(s): State<AppState>,
    _: AdminUser,
    ApiQuery(q): ApiQuery<ListQuery>,
) -> ApiResult<impl IntoResponse> {
    Ok(ok(service::list(&s, None, q.validate()?).await?))
}
async fn admin_detail(
    State(s): State<AppState>,
    _: AdminUser,
    ApiPath(id): ApiPath<Uuid>,
) -> ApiResult<impl IntoResponse> {
    Ok(ok(
        service::view(&mut *s.pool.acquire().await?, id, None).await?
    ))
}
async fn instructions(
    State(s): State<AppState>,
    u: CurrentUser,
    ApiPath(id): ApiPath<Uuid>,
) -> ApiResult<impl IntoResponse> {
    let row:Value=sqlx::query_scalar("SELECT JSON_MERGE_PATCH(payment_instructions,JSON_OBJECT('order_no',order_no,'amount_cents',amount_cents,'currency',currency,'expires_at',expires_at,'status',status)) FROM orders WHERE id=? AND user_id=?").bind(id).bind(u.id).fetch_optional(&s.pool).await?.ok_or_else(AppError::missing)?;
    Ok(ok(row))
}
