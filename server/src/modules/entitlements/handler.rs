use crate::{
    common::{ApiResult, extractors::ApiQuery, ok, pagination::ListQuery},
    middleware::auth::CurrentUser,
    state::AppState,
};
use axum::{Router, extract::State, response::IntoResponse, routing::get};
pub fn router() -> Router<AppState> {
    Router::new().route("/api/v1/me/library", get(library))
}
async fn library(
    State(s): State<AppState>,
    u: CurrentUser,
    ApiQuery(q): ApiQuery<ListQuery>,
) -> ApiResult<impl IntoResponse> {
    Ok(ok(super::service::library(&s, &u, q.validate()?).await?))
}
