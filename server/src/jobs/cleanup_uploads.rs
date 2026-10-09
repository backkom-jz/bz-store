use crate::{common::ApiResult, state::AppState};
use object_store::{ObjectStore, path::Path};
use sqlx::Row;
use time::OffsetDateTime;
use uuid::Uuid;
pub async fn cleanup_batch(state: &AppState, cutoff: OffsetDateTime) -> ApiResult<u64> {
    let files=sqlx::query("WITH candidates AS (SELECT f.id FROM file_objects f WHERE f.created_at<$1 AND NOT EXISTS(SELECT 1 FROM payment_submissions p WHERE p.file_id=f.id) ORDER BY f.created_at,f.id FOR UPDATE SKIP LOCKED LIMIT 100) UPDATE file_objects SET status='deleting' WHERE id IN(SELECT id FROM candidates) RETURNING id,storage_key").bind(cutoff).fetch_all(&state.pool).await?;
    let mut count = 0;
    for row in files {
        let id: Uuid = row.get("id");
        let key = Path::from(row.get::<String, _>("storage_key"));
        match state.storage.delete(&key).await {
            Ok(()) | Err(object_store::Error::NotFound { .. }) => {
                count+=sqlx::query("DELETE FROM file_objects f WHERE id=$1 AND status='deleting' AND NOT EXISTS(SELECT 1 FROM payment_submissions p WHERE p.file_id=f.id)").bind(id).execute(&state.pool).await?.rows_affected();
            }
            Err(_) => tracing::warn!(file_id=%id,"orphan object deletion will retry"),
        }
    }
    Ok(count)
}
