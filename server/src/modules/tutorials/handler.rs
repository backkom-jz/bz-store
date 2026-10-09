use super::service;
use crate::{
    common::{
        ApiResult,
        extractors::{ApiJson, ApiPath, ApiQuery},
        ok,
        pagination::ListQuery,
    },
    middleware::auth::{AdminUser, authenticate},
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
        .route("/api/v1/tutorials", get(list))
        .route("/api/v1/tutorials/{slug}", get(detail))
        .route("/api/v1/tutorials/{id}/chapters/{chapter_id}", get(chapter))
        .route("/api/v1/admin/tutorials", get(admin_list).post(create))
        .route(
            "/api/v1/admin/tutorials/{id}",
            get(admin_detail).patch(update),
        )
        .route(
            "/api/v1/admin/tutorials/{id}/chapters",
            post(create_chapter),
        )
        .route(
            "/api/v1/admin/tutorials/{id}/chapters/{chapter_id}",
            axum::routing::patch(update_chapter),
        )
}
async fn chapter(
    State(s): State<AppState>,
    headers: HeaderMap,
    ApiPath((id, cid)): ApiPath<(Uuid, Uuid)>,
) -> ApiResult<impl IntoResponse> {
    let actor = match authenticate(&s, &headers).await {
        Ok(user) => Some(user),
        Err(error) if error.status == StatusCode::UNAUTHORIZED => None,
        Err(error) => return Err(error),
    };
    Ok(ok(service::chapter(&s, actor.as_ref(), id, cid).await?))
}
async fn list(
    State(s): State<AppState>,
    ApiQuery(q): ApiQuery<ListQuery>,
) -> ApiResult<impl IntoResponse> {
    Ok(ok(service::list(&s, q.validate()?, false).await?))
}
async fn admin_list(
    State(s): State<AppState>,
    _: AdminUser,
    ApiQuery(q): ApiQuery<ListQuery>,
) -> ApiResult<impl IntoResponse> {
    Ok(ok(service::list(&s, q.validate()?, true).await?))
}
async fn detail(
    State(s): State<AppState>,
    headers: HeaderMap,
    ApiPath(slug): ApiPath<String>,
) -> ApiResult<impl IntoResponse> {
    let user = match authenticate(&s, &headers).await {
        Ok(u) => Some(u.id),
        Err(e) if e.status == StatusCode::UNAUTHORIZED => None,
        Err(e) => return Err(e),
    };
    Ok(ok(service::detail(&s, &slug, user, false).await?))
}
async fn admin_detail(
    State(s): State<AppState>,
    AdminUser(a): AdminUser,
    ApiPath(id): ApiPath<Uuid>,
) -> ApiResult<impl IntoResponse> {
    Ok(ok(
        service::detail(&s, &id.to_string(), Some(a.id), true).await?
    ))
}
async fn create(
    State(s): State<AppState>,
    AdminUser(a): AdminUser,
    ApiJson(v): ApiJson<Value>,
) -> ApiResult<impl IntoResponse> {
    Ok((
        StatusCode::CREATED,
        ok(service::save(&s, &a, None, v).await?),
    ))
}
async fn update(
    State(s): State<AppState>,
    AdminUser(a): AdminUser,
    ApiPath(id): ApiPath<Uuid>,
    ApiJson(v): ApiJson<Value>,
) -> ApiResult<impl IntoResponse> {
    Ok(ok(service::save(&s, &a, Some(id), v).await?))
}
async fn create_chapter(
    State(s): State<AppState>,
    AdminUser(a): AdminUser,
    ApiPath(id): ApiPath<Uuid>,
    ApiJson(v): ApiJson<Value>,
) -> ApiResult<impl IntoResponse> {
    Ok((
        StatusCode::CREATED,
        ok(service::save_chapter(&s, &a, id, None, v).await?),
    ))
}
async fn update_chapter(
    State(s): State<AppState>,
    AdminUser(a): AdminUser,
    ApiPath((id, cid)): ApiPath<(Uuid, Uuid)>,
    ApiJson(v): ApiJson<Value>,
) -> ApiResult<impl IntoResponse> {
    Ok(ok(service::save_chapter(&s, &a, id, Some(cid), v).await?))
}
