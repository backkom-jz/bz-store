use crate::common::ApiResult;
use uuid::Uuid;
pub async fn grant_tutorial(
    tx: &mut sqlx::PgConnection,
    user_id: Uuid,
    item_id: Uuid,
    product_id: Uuid,
) -> ApiResult<()> {
    sqlx::query("INSERT INTO user_entitlements(user_id,order_item_id,product_id) VALUES($1,$2,$3) ON CONFLICT(order_item_id) DO NOTHING").bind(user_id).bind(item_id).bind(product_id).execute(tx).await?;
    Ok(())
}
