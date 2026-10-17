use crate::{
    common::{
        ApiResult, AppError,
        extractors::{ApiJson, ApiPath, ApiQuery},
        ok,
        pagination::{ListQuery, slug, text},
    },
    middleware::auth::AdminUser,
    state::AppState,
};
use axum::{Router, extract::State, http::StatusCode, response::IntoResponse, routing::get};
use serde::Deserialize;
use serde_json::{Value, json};
use uuid::Uuid;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct PostInput {
    title: String,
    slug: String,
    excerpt: String,
    content_md: String,
    status: String,
}

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/api/v1/blog", get(published))
        .route("/api/v1/blog/{slug}", get(published_one))
        .route("/api/v1/admin/blog", get(admin_list).post(create))
        .route("/api/v1/admin/blog/{id}", get(admin_one).patch(update))
}

fn markdown(value: &str) -> ApiResult<()> {
    text(value, 100_000)?;
    if value.contains('<') || value.contains('>') {
        return Err(AppError::invalid("正文不能包含 HTML 标签"));
    }
    Ok(())
}

fn validate(input: &PostInput) -> ApiResult<()> {
    text(&input.title, 200)?;
    slug(&input.slug)?;
    if input.excerpt.chars().count() > 500 {
        return Err(AppError::invalid("摘要超过长度限制"));
    }
    markdown(&input.content_md)?;
    if !matches!(input.status.as_str(), "draft" | "published") {
        return Err(AppError::invalid("文章状态无效"));
    }
    Ok(())
}

async fn published(
    State(s): State<AppState>,
    ApiQuery(q): ApiQuery<ListQuery>,
) -> ApiResult<impl IntoResponse> {
    let q = q.validate()?;
    let items: Vec<Value> = sqlx::query_scalar("SELECT JSON_OBJECT('id',BIN_TO_UUID(id),'title',title,'slug',slug,'excerpt',excerpt,'published_at',published_at) FROM blog_posts WHERE status='published' ORDER BY published_at DESC,id DESC LIMIT ? OFFSET ?")
        .bind(q.size).bind(q.offset).fetch_all(&s.pool).await?;
    let total: i64 = sqlx::query_scalar("SELECT count(*) FROM blog_posts WHERE status='published'")
        .fetch_one(&s.pool)
        .await?;
    Ok(ok(
        json!({"items": items, "total": total, "page": q.page, "page_size": q.size}),
    ))
}

async fn published_one(
    State(s): State<AppState>,
    ApiPath(slug): ApiPath<String>,
) -> ApiResult<impl IntoResponse> {
    crate::common::pagination::slug(&slug)?;
    let row: Option<Value> = sqlx::query_scalar("SELECT JSON_OBJECT('id',BIN_TO_UUID(id),'title',title,'slug',slug,'excerpt',excerpt,'content_md',content_md,'published_at',published_at) FROM blog_posts WHERE slug=? AND status='published'")
        .bind(slug).fetch_optional(&s.pool).await?;
    Ok(ok(row.ok_or_else(AppError::missing)?))
}

async fn admin_list(
    State(s): State<AppState>,
    _: AdminUser,
    ApiQuery(q): ApiQuery<ListQuery>,
) -> ApiResult<impl IntoResponse> {
    let q = q.validate()?;
    let items: Vec<Value> = sqlx::query_scalar("SELECT JSON_OBJECT('id',BIN_TO_UUID(id),'title',title,'slug',slug,'excerpt',excerpt,'status',status,'published_at',published_at,'updated_at',updated_at) FROM blog_posts ORDER BY updated_at DESC,id DESC LIMIT ? OFFSET ?")
        .bind(q.size).bind(q.offset).fetch_all(&s.pool).await?;
    let total: i64 = sqlx::query_scalar("SELECT count(*) FROM blog_posts")
        .fetch_one(&s.pool)
        .await?;
    Ok(ok(
        json!({"items": items, "total": total, "page": q.page, "page_size": q.size}),
    ))
}

async fn admin_one(
    State(s): State<AppState>,
    _: AdminUser,
    ApiPath(id): ApiPath<Uuid>,
) -> ApiResult<impl IntoResponse> {
    Ok(ok(one(&s, id).await?))
}

async fn create(
    State(s): State<AppState>,
    _: AdminUser,
    ApiJson(input): ApiJson<PostInput>,
) -> ApiResult<impl IntoResponse> {
    validate(&input)?;
    let id = Uuid::new_v4();
    sqlx::query("INSERT INTO blog_posts(id,title,slug,excerpt,content_md,status,published_at) VALUES(?,?,?,?,?,?,IF(?='published',UTC_TIMESTAMP(6),NULL))")
        .bind(id).bind(input.title.trim()).bind(&input.slug).bind(input.excerpt.trim()).bind(&input.content_md).bind(&input.status).bind(&input.status)
        .execute(&s.pool).await.map_err(|_| AppError::conflict("文章 slug 已存在"))?;
    Ok((StatusCode::CREATED, ok(one(&s, id).await?)))
}

async fn update(
    State(s): State<AppState>,
    _: AdminUser,
    ApiPath(id): ApiPath<Uuid>,
    ApiJson(input): ApiJson<PostInput>,
) -> ApiResult<impl IntoResponse> {
    validate(&input)?;
    let changed = sqlx::query("UPDATE blog_posts SET title=?,slug=?,excerpt=?,content_md=?,status=?,published_at=IF(?='published',COALESCE(published_at,UTC_TIMESTAMP(6)),NULL) WHERE id=?")
        .bind(input.title.trim()).bind(&input.slug).bind(input.excerpt.trim()).bind(&input.content_md).bind(&input.status).bind(&input.status).bind(id)
        .execute(&s.pool).await.map_err(|_| AppError::conflict("文章 slug 已存在"))?;
    if changed.rows_affected() == 0 {
        return Err(AppError::missing());
    }
    Ok(ok(one(&s, id).await?))
}

async fn one(state: &AppState, id: Uuid) -> ApiResult<Value> {
    let row: Option<Value> = sqlx::query_scalar("SELECT JSON_OBJECT('id',BIN_TO_UUID(id),'title',title,'slug',slug,'excerpt',excerpt,'content_md',content_md,'status',status,'published_at',published_at,'updated_at',updated_at) FROM blog_posts WHERE id=?")
        .bind(id).fetch_optional(&state.pool).await?;
    row.ok_or_else(AppError::missing)
}
