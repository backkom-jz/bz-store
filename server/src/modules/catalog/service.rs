use super::dto::ProductInput;
use crate::{
    common::{
        ApiResult, AppError,
        pagination::{Pagination, patch, slug, text},
    },
    middleware::auth::CurrentUser,
    modules::admin::audit,
    state::AppState,
};
use serde_json::{Value, json};
use uuid::Uuid;
pub async fn list(state: &AppState, q: Pagination, admin: bool) -> ApiResult<Value> {
    let items:Vec<Value>=sqlx::query_scalar("SELECT JSON_OBJECT('id',BIN_TO_UUID(p.id),'category_id',BIN_TO_UUID(p.category_id),'type',p.type,'title',p.title,'slug',p.slug,'description',p.description,'price_cents',p.price_cents,'currency',p.currency,'status',p.status,'is_demo',p.is_demo,'created_at',p.created_at,'updated_at',p.updated_at) FROM products p LEFT JOIN categories c ON c.id=p.category_id WHERE (p.status='published' OR ?) AND LOCATE(LOWER(?),LOWER(p.title))>0 AND (?='' OR c.slug=?) ORDER BY p.created_at DESC,p.id DESC LIMIT ? OFFSET ?").bind(admin).bind(&q.q).bind(&q.category).bind(&q.category).bind(q.size).bind(q.offset).fetch_all(&state.pool).await?;
    let total:i64=sqlx::query_scalar("SELECT count(*) FROM products p LEFT JOIN categories c ON c.id=p.category_id WHERE (p.status='published' OR ?) AND LOCATE(LOWER(?),LOWER(p.title))>0 AND (?='' OR c.slug=?)").bind(admin).bind(&q.q).bind(&q.category).bind(&q.category).fetch_one(&state.pool).await?;
    Ok(json!({"items":items,"total":total,"page":q.page,"page_size":q.size}))
}
pub async fn detail(state: &AppState, slug: &str) -> ApiResult<Value> {
    sqlx::query_scalar("SELECT JSON_MERGE_PATCH(JSON_OBJECT('id',BIN_TO_UUID(p.id),'category_id',BIN_TO_UUID(p.category_id),'type',p.type,'title',p.title,'slug',p.slug,'description',p.description,'price_cents',p.price_cents,'currency',p.currency,'status',p.status,'is_demo',p.is_demo,'created_at',p.created_at,'updated_at',p.updated_at),JSON_OBJECT('tutorial_slug',t.slug)) FROM products p LEFT JOIN tutorials t ON t.product_id=p.id WHERE p.slug=? AND p.status='published'").bind(slug).fetch_optional(&state.pool).await?.ok_or_else(AppError::missing)
}
pub async fn save(
    state: &AppState,
    actor: &CurrentUser,
    id: Option<Uuid>,
    input: Value,
) -> ApiResult<Value> {
    let mut tx = state.pool.begin().await?;
    let input: ProductInput = if let Some(id) = id {
        let old:Value=sqlx::query_scalar("SELECT JSON_OBJECT('title',title,'slug',slug,'type',type,'price_cents',price_cents,'description',description,'status',status,'category_id',category_id) FROM products WHERE id=? FOR UPDATE").bind(id).fetch_optional(&mut *tx).await?.ok_or_else(AppError::missing)?;
        patch(
            serde_json::from_value::<ProductInput>(old).map_err(|_| AppError::internal())?,
            input,
        )?
    } else {
        serde_json::from_value(input).map_err(|_| AppError::invalid("商品字段类型无效"))?
    };
    text(&input.title, 200)?;
    slug(&input.slug)?;
    if input.description.chars().count() > 20000
        || !(0..=100_000_000).contains(&input.price_cents)
        || !matches!(input.status.as_str(), "draft" | "published" | "archived")
        || !matches!(input.kind.as_str(), "tutorial" | "digital" | "service")
    {
        return Err(AppError::invalid("商品信息无效"));
    }
    if input.status == "published" {
        if input.kind != "tutorial" {
            return Err(AppError::invalid("本阶段仅支持发布教程"));
        }
        let existing = id.ok_or_else(|| AppError::invalid("请先保存草稿并创建教程章节"))?;
        let ready:bool=sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM tutorials t JOIN tutorial_chapters c ON c.tutorial_id=t.id WHERE t.product_id=? AND t.status='published' AND c.status='published' AND (NOT c.is_preview OR ?=0))").bind(existing).bind(input.price_cents).fetch_one(&mut *tx).await?;
        if !ready {
            return Err(AppError::invalid("请先发布教程及有效的付费章节"));
        }
    }
    let product_id = if let Some(id) = id {
        let changed:bool=sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM products p JOIN order_items oi ON oi.product_id=p.id WHERE p.id=? AND p.type<>?)").bind(id).bind(&input.kind).fetch_one(&mut *tx).await?;
        if changed {
            return Err(AppError::conflict("已售商品不能更改类型"));
        }
        sqlx::query("UPDATE products SET title=?,slug=?,type=?,price_cents=?,description=?,status=?,category_id=? WHERE id=?").bind(&input.title).bind(&input.slug).bind(&input.kind).bind(input.price_cents).bind(&input.description).bind(&input.status).bind(input.category_id).bind(id).execute(&mut *tx).await?;
        id
    } else {
        let product_id = Uuid::new_v4();
        sqlx::query("INSERT INTO products(id,title,slug,type,price_cents,description,status,category_id) VALUES(?,?,?,?,?,?,?,?)").bind(product_id).bind(&input.title).bind(&input.slug).bind(&input.kind).bind(input.price_cents).bind(&input.description).bind(&input.status).bind(input.category_id).execute(&mut *tx).await?;
        product_id
    };
    audit::record(
        &mut tx,
        actor.id,
        if id.is_some() {
            "product.update"
        } else {
            "product.create"
        },
        product_id,
        json!({"status":input.status,"price_cents":input.price_cents}),
    )
    .await?;
    let result: Value = sqlx::query_scalar("SELECT JSON_OBJECT('id',BIN_TO_UUID(p.id),'category_id',BIN_TO_UUID(p.category_id),'type',p.type,'title',p.title,'slug',p.slug,'description',p.description,'price_cents',p.price_cents,'currency',p.currency,'status',p.status,'is_demo',p.is_demo,'created_at',p.created_at,'updated_at',p.updated_at) FROM products p WHERE id=?")
        .bind(product_id)
        .fetch_one(&mut *tx)
        .await?;
    tx.commit().await?;
    Ok(result)
}
