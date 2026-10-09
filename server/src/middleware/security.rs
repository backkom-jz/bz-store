use crate::{common::AppError, state::AppState};
use axum::{
    extract::{Request, State},
    http::Method,
    middleware::Next,
    response::{IntoResponse, Response},
};
pub async fn origin(State(state): State<AppState>, request: Request, next: Next) -> Response {
    if !matches!(
        *request.method(),
        Method::GET | Method::HEAD | Method::OPTIONS
    ) {
        let origins: Vec<_> = request.headers().get_all("origin").iter().collect();
        if origins.len() != 1 || origins[0].to_str().ok() != Some(&state.config.site_url) {
            return AppError::forbidden().into_response();
        }
    }
    next.run(request).await
}
