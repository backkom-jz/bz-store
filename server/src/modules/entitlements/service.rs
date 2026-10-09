use crate::common::ApiResult;
use uuid::Uuid;
pub async fn library(
    state: &crate::state::AppState,
    actor: &crate::middleware::auth::CurrentUser,
    q: crate::common::pagination::Pagination,
) -> ApiResult<serde_json::Value> {
    let items:Vec<serde_json::Value>=sqlx::query_scalar("SELECT jsonb_build_object('product_id',p.id,'title',p.title,'tutorial_id',t.id,'slug',t.slug,'is_demo',p.is_demo,'granted_at',max(e.granted_at)) FROM user_entitlements e JOIN products p ON p.id=e.product_id JOIN tutorials t ON t.product_id=p.id WHERE e.user_id=$1 AND e.status='active' AND t.status='published' GROUP BY p.id,t.id ORDER BY max(e.granted_at) DESC,p.id DESC LIMIT $2 OFFSET $3").bind(actor.id).bind(q.size).bind(q.offset).fetch_all(&state.pool).await?;
    let total:i64=sqlx::query_scalar("SELECT count(DISTINCT e.product_id) FROM user_entitlements e JOIN tutorials t ON t.product_id=e.product_id WHERE e.user_id=$1 AND e.status='active' AND t.status='published'").bind(actor.id).fetch_one(&state.pool).await?;
    Ok(serde_json::json!({"items":items,"total":total,"page":q.page,"page_size":q.size}))
}
pub async fn grant_tutorial(
    tx: &mut sqlx::PgConnection,
    user_id: Uuid,
    item_id: Uuid,
    product_id: Uuid,
) -> ApiResult<()> {
    sqlx::query("INSERT INTO user_entitlements(user_id,order_item_id,product_id) VALUES($1,$2,$3) ON CONFLICT(order_item_id) DO NOTHING").bind(user_id).bind(item_id).bind(product_id).execute(tx).await?;
    Ok(())
}
