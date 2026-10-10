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
pub async fn chapter(
    state: &AppState,
    actor: Option<&CurrentUser>,
    tid: Uuid,
    cid: Uuid,
) -> ApiResult<Value> {
    let user = actor.map(|a| a.id);
    // Body is selected only when authorization is true in this same SQL statement.
    let content:Option<Value>=sqlx::query_scalar("SELECT JSON_OBJECT('id',BIN_TO_UUID(c.id),'title',c.title,'content_md',c.content_md) FROM tutorial_chapters c JOIN tutorials t ON t.id=c.tutorial_id JOIN products p ON p.id=t.product_id WHERE c.id=? AND t.id=? AND c.status='published' AND t.status='published' AND ((c.is_preview AND p.status='published') OR EXISTS(SELECT 1 FROM user_entitlements e WHERE e.user_id=? AND e.product_id=p.id AND e.status='active'))").bind(cid).bind(tid).bind(user).fetch_optional(&state.pool).await?;
    if let Some(value) = content {
        return Ok(value);
    }
    let public:Option<bool>=sqlx::query_scalar("SELECT c.is_preview FROM tutorial_chapters c JOIN tutorials t ON t.id=c.tutorial_id WHERE c.id=? AND t.id=? AND c.status='published' AND t.status='published'").bind(cid).bind(tid).fetch_optional(&state.pool).await?;
    match public {
        None | Some(true) => Err(AppError::missing()),
        Some(false) if actor.is_none() => Err(AppError::unauthorized()),
        _ => Err(AppError::forbidden()),
    }
}
pub async fn list(state: &AppState, q: Pagination, admin: bool) -> ApiResult<Value> {
    let items:Vec<Value>=sqlx::query_scalar("SELECT JSON_MERGE_PATCH(JSON_OBJECT('id',BIN_TO_UUID(t.id),'product_id',BIN_TO_UUID(t.product_id),'title',t.title,'slug',t.slug,'description',t.description,'status',t.status,'created_at',t.created_at,'updated_at',t.updated_at),JSON_OBJECT('price_cents',p.price_cents,'product_slug',p.slug,'is_demo',p.is_demo)) FROM tutorials t JOIN products p ON p.id=t.product_id WHERE (? OR (t.status='published' AND p.status='published')) AND LOCATE(LOWER(?),LOWER(t.title))>0 ORDER BY t.created_at DESC,t.id DESC LIMIT ? OFFSET ?").bind(admin).bind(&q.q).bind(q.size).bind(q.offset).fetch_all(&state.pool).await?;
    let total:i64=sqlx::query_scalar("SELECT count(*) FROM tutorials t JOIN products p ON p.id=t.product_id WHERE (? OR (t.status='published' AND p.status='published')) AND LOCATE(LOWER(?),LOWER(t.title))>0").bind(admin).bind(&q.q).fetch_one(&state.pool).await?;
    Ok(json!({"items":items,"total":total,"page":q.page,"page_size":q.size}))
}
pub async fn detail(
    state: &AppState,
    key: &str,
    user: Option<Uuid>,
    admin: bool,
) -> ApiResult<Value> {
    let maybe_id = Uuid::parse_str(key).ok();
    let mut value:Value=sqlx::query_scalar("SELECT JSON_MERGE_PATCH(JSON_OBJECT('id',BIN_TO_UUID(t.id),'product_id',BIN_TO_UUID(t.product_id),'title',t.title,'slug',t.slug,'description',t.description,'status',t.status,'created_at',t.created_at,'updated_at',t.updated_at),JSON_OBJECT('price_cents',p.price_cents,'product_slug',p.slug,'product_status',p.status,'is_demo',p.is_demo)) FROM tutorials t JOIN products p ON p.id=t.product_id WHERE (t.slug=? OR t.id=?) AND (? OR (t.status='published' AND (p.status='published' OR EXISTS(SELECT 1 FROM user_entitlements e WHERE e.product_id=p.id AND e.user_id=? AND e.status='active'))))").bind(key).bind(maybe_id).bind(admin).bind(user).fetch_optional(&state.pool).await?.ok_or_else(AppError::missing)?;
    let id = Uuid::parse_str(value["id"].as_str().unwrap()).map_err(|_| AppError::internal())?;
    let chapters: Vec<Value> = if admin {
        sqlx::query_scalar("SELECT JSON_OBJECT('id',BIN_TO_UUID(id),'tutorial_id',BIN_TO_UUID(tutorial_id),'title',title,'content_md',content_md,'is_preview',is_preview,'sort_order',sort_order,'status',status,'created_at',created_at,'updated_at',updated_at) FROM tutorial_chapters WHERE tutorial_id=? ORDER BY sort_order,id").bind(id).fetch_all(&state.pool).await?
    } else {
        sqlx::query_scalar("SELECT JSON_OBJECT('id',BIN_TO_UUID(id),'title',title,'sort_order',sort_order,'is_preview',is_preview) FROM tutorial_chapters WHERE tutorial_id=? AND status='published' ORDER BY sort_order,id").bind(id).fetch_all(&state.pool).await?
    };
    value["chapters"] = json!(chapters);
    Ok(value)
}
async fn ensure_published_valid(conn: &mut sqlx::MySqlConnection, product: Uuid) -> ApiResult<()> {
    let invalid:bool=sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM products p WHERE p.id=? AND p.status='published' AND NOT EXISTS(SELECT 1 FROM tutorials t JOIN tutorial_chapters c ON c.tutorial_id=t.id WHERE t.product_id=p.id AND t.status='published' AND c.status='published' AND (NOT c.is_preview OR p.price_cents=0)))").bind(product).fetch_one(conn).await?;
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
        // Serialize editor changes on the parent before reading values to merge.
        sqlx::query("SELECT p.id FROM products p JOIN tutorials t ON t.product_id=p.id WHERE t.id=? FOR UPDATE")
            .bind(id).fetch_optional(&mut *tx).await?.ok_or_else(AppError::missing)?;
        let old: Value = sqlx::query_scalar(
            "SELECT JSON_OBJECT('product_id',BIN_TO_UUID(product_id),'title',title,'slug',slug,'description',description,'status',status) FROM tutorials WHERE id=?",
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
        sqlx::query_scalar("SELECT type FROM products WHERE id=? FOR UPDATE")
            .bind(input.product_id)
            .fetch_optional(&mut *tx)
            .await?;
    if kind.as_deref() != Some("tutorial") {
        return Err(AppError::invalid("教程必须关联教程类型商品"));
    }
    let tid: Uuid = if let Some(id) = id {
        sqlx::query("UPDATE tutorials SET title=?,slug=?,description=?,status=? WHERE id=?")
            .bind(&input.title)
            .bind(&input.slug)
            .bind(&input.description)
            .bind(&input.status)
            .bind(id)
            .execute(&mut *tx)
            .await?;
        id
    } else {
        let tutorial_id = Uuid::new_v4();
        sqlx::query("INSERT INTO tutorials(id,product_id,title,slug,description,status) VALUES(?,?,?,?,?,?)").bind(tutorial_id).bind(input.product_id).bind(&input.title).bind(&input.slug).bind(&input.description).bind(&input.status).execute(&mut *tx).await?;
        tutorial_id
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
    let product: Uuid = sqlx::query_scalar(
        "SELECT p.id FROM products p JOIN tutorials t ON t.product_id=p.id WHERE t.id=? FOR UPDATE",
    )
    .bind(tid)
    .fetch_optional(&mut *tx)
    .await?
    .ok_or_else(AppError::missing)?;
    let input: ChapterInput = if let Some(id) = id {
        let old:Value=sqlx::query_scalar("SELECT JSON_OBJECT('title',title,'content_md',content_md,'is_preview',is_preview,'sort_order',sort_order,'status',status) FROM tutorial_chapters WHERE id=? AND tutorial_id=? FOR UPDATE").bind(id).bind(tid).fetch_optional(&mut *tx).await?.ok_or_else(AppError::missing)?;
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
        sqlx::query("UPDATE tutorial_chapters SET title=?,content_md=?,is_preview=?,sort_order=?,status=? WHERE id=?").bind(&input.title).bind(&input.content_md).bind(input.is_preview).bind(input.sort_order).bind(&input.status).bind(id).execute(&mut *tx).await?;
        id
    } else {
        let chapter_id = Uuid::new_v4();
        sqlx::query("INSERT INTO tutorial_chapters(id,tutorial_id,title,content_md,is_preview,sort_order,status) VALUES(?,?,?,?,?,?,?)").bind(chapter_id).bind(tid).bind(&input.title).bind(&input.content_md).bind(input.is_preview).bind(input.sort_order).bind(&input.status).execute(&mut *tx).await?;
        chapter_id
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
        sqlx::query_scalar("SELECT JSON_OBJECT('id',BIN_TO_UUID(id),'tutorial_id',BIN_TO_UUID(tutorial_id),'title',title,'content_md',content_md,'is_preview',is_preview,'sort_order',sort_order,'status',status,'created_at',created_at,'updated_at',updated_at) FROM tutorial_chapters WHERE id=?")
            .bind(cid)
            .fetch_one(&mut *tx)
            .await?;
    tx.commit().await?;
    Ok(result)
}
