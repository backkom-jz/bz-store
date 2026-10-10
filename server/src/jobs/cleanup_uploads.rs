use crate::{common::ApiResult, state::AppState};
use object_store::{ObjectStore, path::Path};
use sqlx::Row;
use time::OffsetDateTime;
use uuid::Uuid;
pub async fn cleanup_batch(state: &AppState, cutoff: OffsetDateTime) -> ApiResult<u64> {
    let mut tx = state.pool.begin().await?;
    let files=sqlx::query("SELECT f.id,f.storage_key FROM file_objects f WHERE f.created_at<? AND NOT EXISTS(SELECT 1 FROM payment_submissions p WHERE p.file_id=f.id) AND f.status IN ('quarantined','rejected') ORDER BY f.created_at,f.id LIMIT 100 FOR UPDATE SKIP LOCKED").bind(cutoff).fetch_all(&mut *tx).await?;
    for row in &files {
        let id: Uuid = row.get("id");
        sqlx::query("UPDATE file_objects SET status='deleting' WHERE id=?")
            .bind(id)
            .execute(&mut *tx)
            .await?;
    }
    tx.commit().await?;
    let mut count = 0;
    for row in files {
        let id: Uuid = row.get("id");
        let key = Path::from(row.get::<String, _>("storage_key"));
        match state.storage.delete(&key).await {
            Ok(()) | Err(object_store::Error::NotFound { .. }) => {
                count+=sqlx::query("DELETE FROM file_objects WHERE id=? AND status='deleting' AND NOT EXISTS(SELECT 1 FROM payment_submissions p WHERE p.file_id=file_objects.id)").bind(id).execute(&state.pool).await?.rows_affected();
            }
            Err(_) => tracing::warn!(file_id=%id,"orphan object deletion will retry"),
        }
    }
    Ok(count)
}
