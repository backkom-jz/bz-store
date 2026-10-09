use super::dto::*;
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
    let items:Vec<Value>=sqlx::query_scalar("SELECT to_jsonb(t)||jsonb_build_object('price_cents',p.price_cents,'product_slug',p.slug,'is_demo',p.is_demo) FROM tutorials t JOIN products p ON p.id=t.product_id WHERE ($2 OR (t.status='published' AND p.status='published')) AND position(lower($1) in lower(t.title))>0 ORDER BY t.created_at DESC,t.id DESC LIMIT $3 OFFSET $4").bind(&q.q).bind(admin).bind(q.size).bind(q.offset).fetch_all(&state.pool).await?;
    let total:i64=sqlx::query_scalar("SELECT count(*) FROM tutorials t JOIN products p ON p.id=t.product_id WHERE ($2 OR (t.status='published' AND p.status='published')) AND position(lower($1) in lower(t.title))>0").bind(&q.q).bind(admin).fetch_one(&state.pool).await?;
    Ok(json!({"items":items,"total":total,"page":q.page,"page_size":q.size}))
}
pub async fn detail(
    state: &AppState,
    key: &str,
    user: Option<Uuid>,
    admin: bool,
) -> ApiResult<Value> {
    let mut value:Value=sqlx::query_scalar("SELECT to_jsonb(t)||jsonb_build_object('price_cents',p.price_cents,'product_slug',p.slug,'product_status',p.status,'is_demo',p.is_demo) FROM tutorials t JOIN products p ON p.id=t.product_id WHERE (t.slug=$1 OR t.id::text=$1) AND ($3 OR (t.status='published' AND (p.status='published' OR EXISTS(SELECT 1 FROM user_entitlements e WHERE e.product_id=p.id AND e.user_id=$2 AND e.status='active'))))").bind(key).bind(user).bind(admin).fetch_optional(&state.pool).await?.ok_or_else(AppError::missing)?;
    let id = Uuid::parse_str(value["id"].as_str().unwrap()).map_err(|_| AppError::internal())?;
    let chapters: Vec<Value> = if admin {
        sqlx::query_scalar("SELECT to_jsonb(c) FROM tutorial_chapters c WHERE tutorial_id=$1 ORDER BY sort_order,id").bind(id).fetch_all(&state.pool).await?
    } else {
        sqlx::query_scalar("SELECT jsonb_build_object('id',id,'title',title,'sort_order',sort_order,'is_preview',is_preview) FROM tutorial_chapters WHERE tutorial_id=$1 AND status='published' ORDER BY sort_order,id").bind(id).fetch_all(&state.pool).await?
    };
    value["chapters"] = json!(chapters);
    Ok(value)
}
async fn ensure_published_valid(conn: &mut sqlx::PgConnection, product: Uuid) -> ApiResult<()> {
    let invalid:bool=sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM products p WHERE p.id=$1 AND p.status='published' AND NOT EXISTS(SELECT 1 FROM tutorials t JOIN tutorial_chapters c ON c.tutorial_id=t.id WHERE t.product_id=p.id AND t.status='published' AND c.status='published' AND (NOT c.is_preview OR p.price_cents=0)))").bind(product).fetch_one(conn).await?;
    if invalid {
        return Err(AppError::conflict("请先下架商品，再移除其可交付章节"));
    }
    Ok(())
}
pub async fn save(
    state: &AppState,
    actor: &CurrentUser,
    id: Option<Uuid>,
    value: Value,
) -> ApiResult<Value> {
    let mut tx = state.pool.begin().await?;
    let existing: Option<TutorialInput> = if let Some(id) = id {
        let old: Value = sqlx::query_scalar(
            "SELECT to_jsonb(t)-'id'-'created_at'-'updated_at' FROM tutorials t WHERE id=$1",
        )
        .bind(id)
        .fetch_optional(&mut *tx)
        .await?
        .ok_or_else(AppError::missing)?;
        Some(serde_json::from_value(old).map_err(|_| AppError::internal())?)
    } else {
        None
    };
    let previous_product = existing.as_ref().map(|v| v.product_id);
    let input: TutorialInput = if let Some(old) = existing {
        patch(old, value)?
    } else {
        serde_json::from_value(value).map_err(|_| AppError::invalid("教程字段无效"))?
    };
    text(&input.title, 200)?;
    slug(&input.slug)?;
    if input.description.chars().count() > 20000
        || !matches!(input.status.as_str(), "draft" | "published")
    {
        return Err(AppError::invalid("教程状态或介绍无效"));
    }
    if previous_product.is_some_and(|p| p != input.product_id) {
        return Err(AppError::conflict("已创建教程不能移动到其他商品"));
    }
    let kind: Option<String> =
        sqlx::query_scalar("SELECT type FROM products WHERE id=$1 FOR UPDATE")
            .bind(input.product_id)
            .fetch_optional(&mut *tx)
            .await?;
    if kind.as_deref() != Some("tutorial") {
        return Err(AppError::invalid("教程必须关联教程类型商品"));
    }
    let tid: Uuid = if let Some(id) = id {
        sqlx::query("UPDATE tutorials SET title=$1,slug=$2,description=$3,status=$4 WHERE id=$5")
            .bind(&input.title)
            .bind(&input.slug)
            .bind(&input.description)
            .bind(&input.status)
            .bind(id)
            .execute(&mut *tx)
            .await?;
        id
    } else {
        sqlx::query_scalar("INSERT INTO tutorials(product_id,title,slug,description,status) VALUES($1,$2,$3,$4,$5) RETURNING id").bind(input.product_id).bind(&input.title).bind(&input.slug).bind(&input.description).bind(&input.status).fetch_one(&mut *tx).await?
    };
    ensure_published_valid(&mut tx, input.product_id).await?;
    audit::record(
        &mut tx,
        actor.id,
        if id.is_some() {
            "tutorial.update"
        } else {
            "tutorial.create"
        },
        tid,
        json!({"status":input.status}),
    )
    .await?;
    tx.commit().await?;
    detail(state, &tid.to_string(), Some(actor.id), true).await
}
pub async fn save_chapter(
    state: &AppState,
    actor: &CurrentUser,
    tid: Uuid,
    id: Option<Uuid>,
    value: Value,
) -> ApiResult<Value> {
    let mut tx = state.pool.begin().await?;
    let product:Uuid=sqlx::query_scalar("SELECT p.id FROM products p JOIN tutorials t ON t.product_id=p.id WHERE t.id=$1 FOR UPDATE OF p").bind(tid).fetch_optional(&mut *tx).await?.ok_or_else(AppError::missing)?;
    let input: ChapterInput = if let Some(id) = id {
        let old:Value=sqlx::query_scalar("SELECT to_jsonb(c)-'id'-'created_at'-'updated_at'-'tutorial_id' FROM tutorial_chapters c WHERE id=$1 AND tutorial_id=$2 FOR UPDATE").bind(id).bind(tid).fetch_optional(&mut *tx).await?.ok_or_else(AppError::missing)?;
        patch(
            serde_json::from_value::<ChapterInput>(old).map_err(|_| AppError::internal())?,
            value,
        )?
    } else {
        serde_json::from_value(value).map_err(|_| AppError::invalid("章节字段无效"))?
    };
    text(&input.title, 200)?;
    if input.content_md.len() > 500_000
        || input.sort_order < 0
        || !matches!(input.status.as_str(), "draft" | "published")
    {
        return Err(AppError::invalid("章节内容、排序或状态无效"));
    }
    let cid: Uuid = if let Some(id) = id {
        sqlx::query("UPDATE tutorial_chapters SET title=$1,content_md=$2,is_preview=$3,sort_order=$4,status=$5 WHERE id=$6").bind(&input.title).bind(&input.content_md).bind(input.is_preview).bind(input.sort_order).bind(&input.status).bind(id).execute(&mut *tx).await?;
        id
    } else {
        sqlx::query_scalar("INSERT INTO tutorial_chapters(tutorial_id,title,content_md,is_preview,sort_order,status) VALUES($1,$2,$3,$4,$5,$6) RETURNING id").bind(tid).bind(&input.title).bind(&input.content_md).bind(input.is_preview).bind(input.sort_order).bind(&input.status).fetch_one(&mut *tx).await?
    };
    ensure_published_valid(&mut tx, product).await?;
    audit::record(
        &mut tx,
        actor.id,
        if id.is_some() {
            "chapter.update"
        } else {
            "chapter.create"
        },
        cid,
        json!({"is_preview":input.is_preview,"status":input.status}),
    )
    .await?;
    let result: Value =
        sqlx::query_scalar("SELECT to_jsonb(c) FROM tutorial_chapters c WHERE id=$1")
            .bind(cid)
            .fetch_one(&mut *tx)
            .await?;
    tx.commit().await?;
    Ok(result)
}
