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
    let items:Vec<Value>=sqlx::query_scalar("SELECT to_jsonb(p) FROM products p LEFT JOIN categories c ON c.id=p.category_id WHERE (p.status='published' OR $3) AND position(lower($1) in lower(p.title))>0 AND ($2='' OR c.slug=$2) ORDER BY p.created_at DESC,p.id DESC LIMIT $4 OFFSET $5").bind(&q.q).bind(&q.category).bind(admin).bind(q.size).bind(q.offset).fetch_all(&state.pool).await?;
    let total:i64=sqlx::query_scalar("SELECT count(*) FROM products p LEFT JOIN categories c ON c.id=p.category_id WHERE (p.status='published' OR $3) AND position(lower($1) in lower(p.title))>0 AND ($2='' OR c.slug=$2)").bind(&q.q).bind(&q.category).bind(admin).fetch_one(&state.pool).await?;
    Ok(json!({"items":items,"total":total,"page":q.page,"page_size":q.size}))
}
pub async fn detail(state: &AppState, slug: &str) -> ApiResult<Value> {
    sqlx::query_scalar("SELECT to_jsonb(p)||jsonb_build_object('tutorial_slug',t.slug) FROM products p LEFT JOIN tutorials t ON t.product_id=p.id WHERE p.slug=$1 AND p.status='published'").bind(slug).fetch_optional(&state.pool).await?.ok_or_else(AppError::missing)
}
pub async fn save(
    state: &AppState,
    actor: &CurrentUser,
    id: Option<Uuid>,
    input: Value,
) -> ApiResult<Value> {
    let mut tx = state.pool.begin().await?;
    let input: ProductInput = if let Some(id) = id {
        let old:Value=sqlx::query_scalar("SELECT to_jsonb(p)-'id'-'created_at'-'updated_at'-'currency'-'is_demo' FROM products p WHERE id=$1 FOR UPDATE").bind(id).fetch_optional(&mut *tx).await?.ok_or_else(AppError::missing)?;
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
        let ready:bool=sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM tutorials t JOIN tutorial_chapters c ON c.tutorial_id=t.id WHERE t.product_id=$1 AND t.status='published' AND c.status='published' AND (NOT c.is_preview OR $2=0))").bind(existing).bind(input.price_cents).fetch_one(&mut *tx).await?;
        if !ready {
            return Err(AppError::invalid("请先发布教程及有效的付费章节"));
        }
    }
    let product_id = if let Some(id) = id {
        let changed:bool=sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM products p JOIN order_items oi ON oi.product_id=p.id WHERE p.id=$1 AND p.type<>$2)").bind(id).bind(&input.kind).fetch_one(&mut *tx).await?;
        if changed {
            return Err(AppError::conflict("已售商品不能更改类型"));
        }
        sqlx::query("UPDATE products SET title=$1,slug=$2,type=$3,price_cents=$4,description=$5,status=$6,category_id=$7 WHERE id=$8").bind(&input.title).bind(&input.slug).bind(&input.kind).bind(input.price_cents).bind(&input.description).bind(&input.status).bind(input.category_id).bind(id).execute(&mut *tx).await?;
        id
    } else {
        sqlx::query_scalar("INSERT INTO products(title,slug,type,price_cents,description,status,category_id) VALUES($1,$2,$3,$4,$5,$6,$7) RETURNING id").bind(&input.title).bind(&input.slug).bind(&input.kind).bind(input.price_cents).bind(&input.description).bind(&input.status).bind(input.category_id).fetch_one(&mut *tx).await?
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
    let result: Value = sqlx::query_scalar("SELECT to_jsonb(p) FROM products p WHERE id=$1")
        .bind(product_id)
        .fetch_one(&mut *tx)
        .await?;
    tx.commit().await?;
    Ok(result)
}
