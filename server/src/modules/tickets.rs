use crate::{
    common::{
        ApiResult, AppError,
        extractors::{ApiJson, ApiPath, ApiQuery},
        ok,
        pagination::{ListQuery, text},
    },
    middleware::auth::{AdminUser, CurrentUser},
    state::AppState,
};
use axum::{
    Router,
    extract::State,
    response::IntoResponse,
    routing::{get, post},
};
use serde::Deserialize;
use serde_json::{Value, json};
use sqlx::Row;
use uuid::Uuid;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct MessageInput {
    body: String,
}

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/api/v1/me/tickets", get(mine))
        .route("/api/v1/me/tickets/{id}", get(mine_detail))
        .route("/api/v1/me/tickets/{id}/messages", post(mine_message))
        .route("/api/v1/me/tickets/{id}/accept", post(accept))
        .route("/api/v1/admin/tickets", get(admin_list))
        .route("/api/v1/admin/tickets/{id}", get(admin_detail))
        .route("/api/v1/admin/tickets/{id}/messages", post(admin_message))
}

pub async fn open_ticket(
    tx: &mut sqlx::MySqlConnection,
    order_id: Uuid,
    item_id: Uuid,
    user_id: Uuid,
) -> ApiResult<()> {
    let ticket = Uuid::new_v4();
    sqlx::query("INSERT INTO service_tickets(id,order_id,order_item_id,user_id,status) VALUES(?,?,?,?,'open')")
        .bind(ticket).bind(order_id).bind(item_id).bind(user_id).execute(&mut *tx).await?;
    sqlx::query("INSERT INTO ticket_messages(id,ticket_id,author_id,body) VALUES(?,?,?,?)")
        .bind(Uuid::new_v4())
        .bind(ticket)
        .bind(user_id)
        .bind("服务已开通，请在这里沟通交付内容。")
        .execute(&mut *tx)
        .await?;
    Ok(())
}

async fn mine(
    State(s): State<AppState>,
    actor: CurrentUser,
    ApiQuery(q): ApiQuery<ListQuery>,
) -> ApiResult<impl IntoResponse> {
    Ok(ok(list(&s, Some(actor.id), q.validate()?).await?))
}
async fn admin_list(
    State(s): State<AppState>,
    _: AdminUser,
    ApiQuery(q): ApiQuery<ListQuery>,
) -> ApiResult<impl IntoResponse> {
    Ok(ok(list(&s, None, q.validate()?).await?))
}
async fn mine_detail(
    State(s): State<AppState>,
    actor: CurrentUser,
    ApiPath(id): ApiPath<Uuid>,
) -> ApiResult<impl IntoResponse> {
    Ok(ok(detail(&s, id, Some(actor.id)).await?))
}
async fn admin_detail(
    State(s): State<AppState>,
    _: AdminUser,
    ApiPath(id): ApiPath<Uuid>,
) -> ApiResult<impl IntoResponse> {
    Ok(ok(detail(&s, id, None).await?))
}
async fn mine_message(
    State(s): State<AppState>,
    actor: CurrentUser,
    ApiPath(id): ApiPath<Uuid>,
    ApiJson(input): ApiJson<MessageInput>,
) -> ApiResult<impl IntoResponse> {
    Ok(ok(message(&s, id, actor.id, Some(actor.id), input).await?))
}
async fn admin_message(
    State(s): State<AppState>,
    actor: AdminUser,
    ApiPath(id): ApiPath<Uuid>,
    ApiJson(input): ApiJson<MessageInput>,
) -> ApiResult<impl IntoResponse> {
    Ok(ok(message(&s, id, actor.0.id, None, input).await?))
}
async fn accept(
    State(s): State<AppState>,
    actor: CurrentUser,
    ApiPath(id): ApiPath<Uuid>,
) -> ApiResult<impl IntoResponse> {
    let mut tx = s.pool.begin().await?;
    // Find the immutable order link, then follow refund's order -> ticket lock order.
    let order_id: Uuid =
        sqlx::query_scalar("SELECT order_id FROM service_tickets WHERE id=? AND user_id=?")
            .bind(id)
            .bind(actor.id)
            .fetch_optional(&mut *tx)
            .await?
            .ok_or_else(AppError::missing)?;
    let order_status: String =
        sqlx::query_scalar("SELECT status FROM orders WHERE id=? FOR UPDATE")
            .bind(order_id)
            .fetch_one(&mut *tx)
            .await?;
    // Re-read under the ticket lock; the initial lookup must not decide acceptance.
    let row = sqlx::query(
        "SELECT status FROM service_tickets WHERE id=? AND user_id=? AND order_id=? FOR UPDATE",
    )
    .bind(id)
    .bind(actor.id)
    .bind(order_id)
    .fetch_optional(&mut *tx)
    .await?
    .ok_or_else(AppError::missing)?;
    let status: String = row.get("status");
    if status != "open" {
        return Err(AppError::conflict("这个工单已经结束"));
    }
    if order_status != "fulfilling" {
        return Err(AppError::conflict("当前订单不能确认收货"));
    }
    sqlx::query(
        "UPDATE service_tickets SET status='completed',closed_at=UTC_TIMESTAMP(6) WHERE id=?",
    )
    .bind(id)
    .execute(&mut *tx)
    .await?;
    sqlx::query("UPDATE orders SET status='completed' WHERE id=?")
        .bind(order_id)
        .execute(&mut *tx)
        .await?;
    let result = detail_tx(&mut tx, id, Some(actor.id)).await?;
    tx.commit().await?;
    Ok(ok(result))
}

async fn list(
    state: &AppState,
    user: Option<Uuid>,
    q: crate::common::pagination::Pagination,
) -> ApiResult<Value> {
    let items: Vec<Value> = sqlx::query_scalar("SELECT JSON_OBJECT('id',BIN_TO_UUID(t.id),'order_id',BIN_TO_UUID(t.order_id),'status',t.status,'created_at',t.created_at,'product_title',i.product_title,'user_email',u.email) FROM service_tickets t JOIN order_items i ON i.id=t.order_item_id JOIN users u ON u.id=t.user_id WHERE (? IS NULL OR t.user_id=?) ORDER BY t.created_at DESC,t.id DESC LIMIT ? OFFSET ?")
        .bind(user).bind(user).bind(q.size).bind(q.offset).fetch_all(&state.pool).await?;
    let total: i64 =
        sqlx::query_scalar("SELECT count(*) FROM service_tickets WHERE (? IS NULL OR user_id=?)")
            .bind(user)
            .bind(user)
            .fetch_one(&state.pool)
            .await?;
    Ok(json!({"items": items, "total": total, "page": q.page, "page_size": q.size}))
}

async fn detail(state: &AppState, id: Uuid, user: Option<Uuid>) -> ApiResult<Value> {
    detail_tx(&mut *state.pool.acquire().await?, id, user).await
}

async fn detail_tx(
    conn: &mut sqlx::MySqlConnection,
    id: Uuid,
    user: Option<Uuid>,
) -> ApiResult<Value> {
    let mut row: Value = sqlx::query_scalar("SELECT JSON_OBJECT('id',BIN_TO_UUID(t.id),'order_id',BIN_TO_UUID(t.order_id),'status',t.status,'created_at',t.created_at,'product_title',i.product_title,'user_email',u.email) FROM service_tickets t JOIN order_items i ON i.id=t.order_item_id JOIN users u ON u.id=t.user_id WHERE t.id=? AND (? IS NULL OR t.user_id=?)")
        .bind(id).bind(user).bind(user).fetch_optional(&mut *conn).await?.ok_or_else(AppError::missing)?;
    let messages: Vec<Value> = sqlx::query_scalar("SELECT JSON_OBJECT('id',BIN_TO_UUID(id),'author_id',BIN_TO_UUID(author_id),'body',body,'created_at',created_at) FROM ticket_messages WHERE ticket_id=? ORDER BY created_at,id")
        .bind(id).fetch_all(&mut *conn).await?;
    row["messages"] = Value::Array(messages);
    Ok(row)
}

async fn message(
    state: &AppState,
    id: Uuid,
    author: Uuid,
    owner: Option<Uuid>,
    input: MessageInput,
) -> ApiResult<Value> {
    text(&input.body, 4000)?;
    let mut tx = state.pool.begin().await?;
    let status: Option<String> = sqlx::query_scalar(
        "SELECT status FROM service_tickets WHERE id=? AND (? IS NULL OR user_id=?) FOR UPDATE",
    )
    .bind(id)
    .bind(owner)
    .bind(owner)
    .fetch_optional(&mut *tx)
    .await?;
    let Some(status) = status else {
        return Err(AppError::missing());
    };
    if status != "open" {
        return Err(AppError::conflict("已结束的工单不能继续留言"));
    }
    sqlx::query("INSERT INTO ticket_messages(id,ticket_id,author_id,body) VALUES(?,?,?,?)")
        .bind(Uuid::new_v4())
        .bind(id)
        .bind(author)
        .bind(input.body.trim())
        .execute(&mut *tx)
        .await?;
    let result = detail_tx(&mut tx, id, owner).await?;
    tx.commit().await?;
    Ok(result)
}
