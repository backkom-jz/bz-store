use crate::{common::ApiResult, state::AppState};
use time::OffsetDateTime;
pub async fn expire_batch(state: &AppState, now: OffsetDateTime) -> ApiResult<u64> {
    let mut tx = state.pool.begin().await?;
    let rows = sqlx::query("SELECT id FROM orders WHERE status='pending_payment' AND expires_at<=? ORDER BY expires_at,id LIMIT 100 FOR UPDATE SKIP LOCKED")
        .bind(now)
        .fetch_all(&mut *tx)
        .await?;
    let mut affected = 0;
    for row in rows {
        let id: uuid::Uuid = sqlx::Row::get(&row, "id");
        affected += sqlx::query(
            "UPDATE orders SET status='cancelled' WHERE id=? AND status='pending_payment'",
        )
        .bind(id)
        .execute(&mut *tx)
        .await?
        .rows_affected();
    }
    tx.commit().await?;
    Ok(affected)
}
