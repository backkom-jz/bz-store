use super::service;
use crate::{
    common::{
        ApiResult, AppError,
        extractors::{ApiJson, ApiPath, ApiQuery},
        ok,
        pagination::ListQuery,
    },
    middleware::auth::AdminUser,
    state::AppState,
};
use axum::{
    Router,
    extract::State,
    http::StatusCode,
    response::IntoResponse,
    routing::{get, post},
};
use serde_json::Value;
use uuid::Uuid;
pub fn router() -> Router<AppState> {
    Router::new()
        .route("/api/v1/products", get(list))
        .route("/api/v1/products/{slug}", get(detail))
        .route("/api/v1/categories", get(categories))
        .route("/api/v1/admin/products", get(admin_list).post(create))
        .route(
            "/api/v1/admin/products/{id}",
            get(admin_detail).patch(update),
        )
        .route(
            "/api/v1/admin/audit-logs",
            get(crate::modules::admin::audit::list),
        )
        .route("/api/v1/admin/categories", post(create_category))
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
    ApiPath(slug): ApiPath<String>,
) -> ApiResult<impl IntoResponse> {
    Ok(ok(service::detail(&s, &slug).await?))
}
async fn admin_detail(
    State(s): State<AppState>,
    _: AdminUser,
    ApiPath(id): ApiPath<Uuid>,
) -> ApiResult<impl IntoResponse> {
    let v: Value = sqlx::query_scalar("SELECT to_jsonb(p) FROM products p WHERE id=$1")
        .bind(id)
        .fetch_optional(&s.pool)
        .await?
        .ok_or_else(AppError::missing)?;
    Ok(ok(v))
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
async fn categories(State(s): State<AppState>) -> ApiResult<impl IntoResponse> {
    let rows: Vec<Value> =
        sqlx::query_scalar("SELECT to_jsonb(c) FROM categories c ORDER BY name,id")
            .fetch_all(&s.pool)
            .await?;
    Ok(ok(rows))
}
#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct CategoryInput {
    name: String,
    slug: String,
}
async fn create_category(
    State(s): State<AppState>,
    AdminUser(a): AdminUser,
    ApiJson(v): ApiJson<CategoryInput>,
) -> ApiResult<impl IntoResponse> {
    crate::common::pagination::text(&v.name, 80)?;
    crate::common::pagination::slug(&v.slug)?;
    let mut tx = s.pool.begin().await?;
    let id: Uuid =
        sqlx::query_scalar("INSERT INTO categories(name,slug) VALUES($1,$2) RETURNING id")
            .bind(&v.name)
            .bind(&v.slug)
            .fetch_one(&mut *tx)
            .await?;
    crate::modules::admin::audit::record(
        &mut tx,
        a.id,
        "category.create",
        id,
        serde_json::json!({"name":v.name}),
    )
    .await?;
    tx.commit().await?;
    Ok((
        StatusCode::CREATED,
        ok(serde_json::json!({"id":id,"name":v.name,"slug":v.slug})),
    ))
}
