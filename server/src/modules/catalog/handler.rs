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
    let v: Value = sqlx::query_scalar("SELECT JSON_OBJECT('id',BIN_TO_UUID(p.id),'category_id',BIN_TO_UUID(p.category_id),'type',p.type,'title',p.title,'slug',p.slug,'description',p.description,'price_cents',p.price_cents,'currency',p.currency,'status',p.status,'is_demo',p.is_demo,'created_at',p.created_at,'updated_at',p.updated_at) FROM products p WHERE id=?")
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
        sqlx::query_scalar("SELECT JSON_OBJECT('id',BIN_TO_UUID(c.id),'parent_id',BIN_TO_UUID(c.parent_id),'name',c.name,'slug',c.slug,'created_at',c.created_at,'updated_at',c.updated_at) FROM categories c ORDER BY name,id")
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
    let id = Uuid::new_v4();
    sqlx::query("INSERT INTO categories(id,name,slug) VALUES(?,?,?)")
        .bind(id)
        .bind(&v.name)
        .bind(&v.slug)
        .execute(&mut *tx)
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
