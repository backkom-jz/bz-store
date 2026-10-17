use crate::common::ApiResult;
use uuid::Uuid;
pub async fn library(
    state: &crate::state::AppState,
    actor: &crate::middleware::auth::CurrentUser,
    q: crate::common::pagination::Pagination,
) -> ApiResult<serde_json::Value> {
    let items:Vec<serde_json::Value>=sqlx::query_scalar("SELECT JSON_OBJECT('product_id',BIN_TO_UUID(p.id),'title',p.title,'type',p.type,'tutorial_id',BIN_TO_UUID(t.id),'slug',COALESCE(t.slug,p.slug),'file_id',BIN_TO_UUID(f.file_id),'is_demo',p.is_demo,'granted_at',MAX(e.granted_at)) FROM user_entitlements e JOIN products p ON p.id=e.product_id LEFT JOIN tutorials t ON t.product_id=p.id AND t.status='published' LEFT JOIN product_files f ON f.product_id=p.id WHERE e.user_id=? AND e.status='active' GROUP BY p.id,p.title,p.type,t.id,t.slug,p.slug,f.file_id,p.is_demo ORDER BY MAX(e.granted_at) DESC,p.id DESC LIMIT ? OFFSET ?").bind(actor.id).bind(q.size).bind(q.offset).fetch_all(&state.pool).await?;
    let total:i64=sqlx::query_scalar("SELECT count(DISTINCT e.product_id) FROM user_entitlements e WHERE e.user_id=? AND e.status='active'").bind(actor.id).fetch_one(&state.pool).await?;
    Ok(serde_json::json!({"items":items,"total":total,"page":q.page,"page_size":q.size}))
}
pub async fn purchase_state(
    state: &crate::state::AppState,
    actor: &crate::middleware::auth::CurrentUser,
    product_id: Uuid,
) -> ApiResult<serde_json::Value> {
    let exists: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM products WHERE id=?)")
        .bind(product_id)
        .fetch_one(&state.pool)
        .await?;
    if !exists {
        return Err(crate::common::AppError::missing());
    }
    let owned: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM user_entitlements WHERE user_id=? AND product_id=? AND status='active')",
    )
    .bind(actor.id)
    .bind(product_id)
    .fetch_one(&state.pool)
    .await?;
    let open_order_id: Option<Uuid> = sqlx::query_scalar(
        "SELECT o.id FROM orders o JOIN order_items i ON i.order_id=o.id WHERE o.user_id=? AND i.product_id=? AND o.status IN ('pending_payment','payment_review') ORDER BY o.created_at DESC,o.id DESC LIMIT 1",
    )
    .bind(actor.id)
    .bind(product_id)
    .fetch_optional(&state.pool)
    .await?;
    let tutorial_slug: Option<String> = sqlx::query_scalar(
        "SELECT slug FROM tutorials WHERE product_id=? AND status='published' ORDER BY created_at,id LIMIT 1",
    )
    .bind(product_id)
    .fetch_optional(&state.pool)
    .await?;
    let file_id: Option<Uuid> =
        sqlx::query_scalar("SELECT file_id FROM product_files WHERE product_id=?")
            .bind(product_id)
            .fetch_optional(&state.pool)
            .await?;
    Ok(serde_json::json!({
        "owned": owned,
        "open_order_id": open_order_id,
        "tutorial_slug": tutorial_slug,
        "file_id": file_id,
    }))
}
pub async fn grant_tutorial(
    tx: &mut sqlx::MySqlConnection,
    user_id: Uuid,
    item_id: Uuid,
    product_id: Uuid,
) -> ApiResult<()> {
    sqlx::query("INSERT INTO user_entitlements(id,user_id,order_item_id,product_id) VALUES(?,?,?,?) ON DUPLICATE KEY UPDATE order_item_id=order_item_id").bind(Uuid::new_v4()).bind(user_id).bind(item_id).bind(product_id).execute(tx).await?;
    Ok(())
}
