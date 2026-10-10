use crate::common::ApiResult;
use uuid::Uuid;
pub async fn library(
    state: &crate::state::AppState,
    actor: &crate::middleware::auth::CurrentUser,
    q: crate::common::pagination::Pagination,
) -> ApiResult<serde_json::Value> {
    let items:Vec<serde_json::Value>=sqlx::query_scalar("SELECT JSON_OBJECT('product_id',BIN_TO_UUID(p.id),'title',p.title,'tutorial_id',BIN_TO_UUID(t.id),'slug',t.slug,'is_demo',p.is_demo,'granted_at',MAX(e.granted_at)) FROM user_entitlements e JOIN products p ON p.id=e.product_id JOIN tutorials t ON t.product_id=p.id WHERE e.user_id=? AND e.status='active' AND t.status='published' GROUP BY p.id,t.id ORDER BY MAX(e.granted_at) DESC,p.id DESC LIMIT ? OFFSET ?").bind(actor.id).bind(q.size).bind(q.offset).fetch_all(&state.pool).await?;
    let total:i64=sqlx::query_scalar("SELECT count(DISTINCT e.product_id) FROM user_entitlements e JOIN tutorials t ON t.product_id=e.product_id WHERE e.user_id=? AND e.status='active' AND t.status='published'").bind(actor.id).fetch_one(&state.pool).await?;
    Ok(serde_json::json!({"items":items,"total":total,"page":q.page,"page_size":q.size}))
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
