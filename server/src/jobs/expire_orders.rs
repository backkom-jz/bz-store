use crate::{common::ApiResult, state::AppState};
use time::OffsetDateTime;
pub async fn expire_batch(state: &AppState, now: OffsetDateTime) -> ApiResult<u64> {
    let result=sqlx::query("WITH candidates AS (SELECT id FROM orders WHERE status='pending_payment' AND expires_at<=$1 ORDER BY expires_at,id FOR UPDATE SKIP LOCKED LIMIT 100) UPDATE orders SET status='cancelled' WHERE id IN (SELECT id FROM candidates)")
 .bind(now).execute(&state.pool).await?;
    Ok(result.rows_affected())
}
