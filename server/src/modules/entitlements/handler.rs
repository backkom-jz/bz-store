use crate::{
    common::{
        ApiResult,
        extractors::{ApiPath, ApiQuery},
        ok,
        pagination::ListQuery,
    },
    middleware::auth::CurrentUser,
    state::AppState,
};
use axum::{Router, extract::State, response::IntoResponse, routing::get};
use uuid::Uuid;
pub fn router() -> Router<AppState> {
    Router::new()
        .route("/api/v1/me/library", get(library))
        .route("/api/v1/me/purchases/{product_id}", get(purchase_state))
}
async fn library(
    State(s): State<AppState>,
    u: CurrentUser,
    ApiQuery(q): ApiQuery<ListQuery>,
) -> ApiResult<impl IntoResponse> {
    Ok(ok(super::service::library(&s, &u, q.validate()?).await?))
}
async fn purchase_state(
    State(s): State<AppState>,
    u: CurrentUser,
    ApiPath(product_id): ApiPath<Uuid>,
) -> ApiResult<impl IntoResponse> {
    Ok(ok(super::service::purchase_state(&s, &u, product_id).await?))
}
