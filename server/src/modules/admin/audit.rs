use crate::{
    common::{ApiResult, extractors::ApiQuery, ok, pagination::ListQuery},
    middleware::auth::AdminUser,
    state::AppState,
};
use serde_json::{Value, json};
use uuid::Uuid;
pub async fn record(
    conn: &mut sqlx::MySqlConnection,
    actor: Uuid,
    action: &str,
    target: Uuid,
    detail: Value,
) -> ApiResult<()> {
    sqlx::query(
        "INSERT INTO admin_audit_logs(id,actor_id,action,target_id,detail) VALUES(?,?,?,?,?)",
    )
    .bind(Uuid::new_v4())
    .bind(actor)
    .bind(action)
    .bind(target)
    .bind(detail)
    .execute(conn)
    .await?;
    Ok(())
}
pub async fn list(
    axum::extract::State(s): axum::extract::State<AppState>,
    _: AdminUser,
    ApiQuery(q): ApiQuery<ListQuery>,
) -> ApiResult<impl axum::response::IntoResponse> {
    let q = q.validate()?;
    let items:Vec<Value>=sqlx::query_scalar("SELECT JSON_MERGE_PATCH(JSON_OBJECT('id',BIN_TO_UUID(a.id),'actor_id',BIN_TO_UUID(a.actor_id),'action',a.action,'target_id',BIN_TO_UUID(a.target_id),'detail',a.detail,'created_at',a.created_at),JSON_OBJECT('actor_email',u.email)) FROM admin_audit_logs a LEFT JOIN users u ON u.id=a.actor_id ORDER BY a.created_at DESC,a.id DESC LIMIT ? OFFSET ?").bind(q.size).bind(q.offset).fetch_all(&s.pool).await?;
    let total: i64 = sqlx::query_scalar("SELECT count(*) FROM admin_audit_logs")
        .fetch_one(&s.pool)
        .await?;
    Ok(ok(
        json!({"items":items,"total":total,"page":q.page,"page_size":q.size}),
    ))
}
