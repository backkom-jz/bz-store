use crate::{
    common::{ApiResult, extractors::ApiQuery, ok, pagination::ListQuery},
    middleware::auth::AdminUser,
    state::AppState,
};
use serde_json::{Value, json};
use uuid::Uuid;
pub async fn record(
    conn: &mut sqlx::PgConnection,
    actor: Uuid,
    action: &str,
    target: Uuid,
    detail: Value,
) -> ApiResult<()> {
    sqlx::query(
        "INSERT INTO admin_audit_logs(actor_id,action,target_id,detail) VALUES($1,$2,$3,$4)",
    )
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
    let items:Vec<Value>=sqlx::query_scalar("SELECT to_jsonb(a)||jsonb_build_object('actor_email',u.email) FROM admin_audit_logs a LEFT JOIN users u ON u.id=a.actor_id ORDER BY a.created_at DESC,a.id DESC LIMIT $1 OFFSET $2").bind(q.size).bind(q.offset).fetch_all(&s.pool).await?;
    let total: i64 = sqlx::query_scalar("SELECT count(*) FROM admin_audit_logs")
        .fetch_one(&s.pool)
        .await?;
    Ok(ok(
        json!({"items":items,"total":total,"page":q.page,"page_size":q.size}),
    ))
}
