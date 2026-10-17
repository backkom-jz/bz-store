use super::dto::*;
use crate::{
    common::{ApiResult, AppError, pagination::Pagination},
    middleware::auth::{AdminUser, CurrentUser},
    modules::{auth::service::digest, entitlements::service::grant_tutorial},
    state::AppState,
};
use serde_json::{Value, json};
use sqlx::Row;
use uuid::Uuid;
pub const ORDER_VIEW: &str = "SELECT JSON_MERGE_PATCH(JSON_OBJECT('id',BIN_TO_UUID(o.id),'order_no',o.order_no,'user_id',BIN_TO_UUID(o.user_id),'amount_cents',o.amount_cents,'currency',o.currency,'status',o.status,'expires_at',o.expires_at,'paid_at',o.paid_at,'created_at',o.created_at,'updated_at',o.updated_at),'{}',JSON_OBJECT('item',JSON_OBJECT('id',BIN_TO_UUID(i.id),'product_id',BIN_TO_UUID(i.product_id),'product_title',i.product_title,'product_type',i.product_type,'unit_price_cents',i.unit_price_cents,'quantity',i.quantity,'subtotal_cents',i.subtotal_cents,'currency',i.currency),'payments',COALESCE((SELECT JSON_ARRAYAGG(JSON_OBJECT('id',BIN_TO_UUID(p.id),'status',p.status,'review_note',p.review_note,'created_at',p.created_at,'reviewed_at',p.reviewed_at)) FROM payment_submissions p WHERE p.order_id=o.id),JSON_ARRAY()))) FROM orders o JOIN order_items i ON i.order_id=o.id WHERE o.id=? AND (? IS NULL OR o.user_id=?)";
pub async fn view(
    conn: &mut sqlx::MySqlConnection,
    id: Uuid,
    user: Option<Uuid>,
) -> ApiResult<Value> {
    sqlx::query_scalar(ORDER_VIEW)
        .bind(id)
        .bind(user)
        .bind(user)
        .fetch_optional(conn)
        .await?
        .ok_or_else(AppError::missing)
}
pub async fn get(state: &AppState, actor: &CurrentUser, id: Uuid) -> ApiResult<Value> {
    view(&mut *state.pool.acquire().await?, id, Some(actor.id)).await
}
pub async fn create(
    state: &AppState,
    actor: &CurrentUser,
    key: &str,
    input: CreateOrderInput,
) -> ApiResult<Value> {
    if !(16..=128).contains(&key.len()) || !key.bytes().all(|b| b.is_ascii_graphic()) {
        return Err(AppError::invalid("需要 16–128 字符的 Idempotency-Key"));
    }
    let fingerprint = digest(&format!("{}:{}", input.product_id, input.quantity));
    let mut tx = state.pool.begin().await?;
    sqlx::query("INSERT INTO order_requests(user_id,idempotency_key,request_fingerprint) VALUES(?,?,?) ON DUPLICATE KEY UPDATE idempotency_key=idempotency_key").bind(actor.id).bind(key).bind(&fingerprint).execute(&mut *tx).await?;
    let existing=sqlx::query("SELECT request_fingerprint,order_id FROM order_requests WHERE user_id=? AND idempotency_key=? FOR UPDATE").bind(actor.id).bind(key).fetch_one(&mut *tx).await?;
    if existing.get::<String, _>("request_fingerprint") != fingerprint {
        return Err(AppError::conflict("此幂等键已用于其他下单内容"));
    }
    if let Some(id) = existing.get::<Option<Uuid>, _>("order_id") {
        let result = view(&mut tx, id, Some(actor.id)).await?;
        tx.commit().await?;
        return Ok(result);
    }
    if input.quantity != 1 {
        return Err(AppError::invalid("本阶段每单仅可购买一件商品"));
    }
    let product = sqlx::query(
        "SELECT id,title,type,price_cents,currency,status FROM products WHERE id=? FOR UPDATE",
    )
    .bind(input.product_id)
    .fetch_optional(&mut *tx)
    .await?
    .ok_or_else(AppError::missing)?;
    if product.get::<String, _>("status") != "published"
        || !matches!(
            product.get::<String, _>("type").as_str(),
            "tutorial" | "digital" | "service"
        )
    {
        return Err(AppError::conflict("商品尚未上架或不支持交付"));
    }
    let price: i64 = product.get("price_cents");
    let payment: Value = if price > 0 {
        let value: Value = sqlx::query_scalar(
            "SELECT value_json FROM site_settings WHERE setting_key='manual_payment'",
        )
        .fetch_optional(&mut *tx)
        .await?
        .ok_or_else(|| {
            AppError::new(
                axum::http::StatusCode::SERVICE_UNAVAILABLE,
                "PAYMENT_UNAVAILABLE",
                "尚未配置收款信息，暂时不能购买",
            )
        })?;
        let config: PaymentConfig = serde_json::from_value(value.clone()).map_err(|_| {
            AppError::new(
                axum::http::StatusCode::SERVICE_UNAVAILABLE,
                "PAYMENT_UNAVAILABLE",
                "尚未配置收款信息，暂时不能购买",
            )
        })?;
        config.validate().map_err(|_| {
            AppError::new(
                axum::http::StatusCode::SERVICE_UNAVAILABLE,
                "PAYMENT_UNAVAILABLE",
                "尚未配置收款信息，暂时不能购买",
            )
        })?;
        value
    } else {
        json!({})
    };
    let id = Uuid::new_v4();
    let item = Uuid::new_v4();
    let no = format!("DS{}", id.simple().to_string().to_uppercase());
    sqlx::query("INSERT INTO orders(id,order_no,user_id,amount_cents,status,payment_instructions,expires_at) VALUES(?,?,?,?,?,?,DATE_ADD(UTC_TIMESTAMP(6), INTERVAL 24 HOUR))").bind(id).bind(no).bind(actor.id).bind(price).bind(if price==0{"completed"}else{"pending_payment"}).bind(payment).execute(&mut *tx).await?;
    let kind: String = product.get("type");
    if !matches!(kind.as_str(), "tutorial" | "digital" | "service") {
        return Err(AppError::conflict("商品尚未上架或不支持交付"));
    }
    if kind == "digital" {
        let ready: bool =
            sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM product_files WHERE product_id=?)")
                .bind(input.product_id)
                .fetch_one(&mut *tx)
                .await?;
        if !ready {
            return Err(AppError::conflict("商品尚未上架或不支持交付"));
        }
    }
    sqlx::query("INSERT INTO order_items(id,order_id,product_id,product_title,product_type,unit_price_cents,quantity,subtotal_cents) VALUES(?,?,?,?,?,?,1,?)").bind(item).bind(id).bind(input.product_id).bind(product.get::<String,_>("title")).bind(&kind).bind(price).bind(price).execute(&mut *tx).await?;
    if price == 0 {
        match kind.as_str() {
            "service" => {
                crate::modules::tickets::open_ticket(&mut tx, id, item, actor.id).await?;
                sqlx::query("UPDATE orders SET status='fulfilling' WHERE id=?")
                    .bind(id)
                    .execute(&mut *tx)
                    .await?;
            }
            _ => grant_tutorial(&mut tx, actor.id, item, input.product_id).await?,
        }
    }
    sqlx::query("UPDATE order_requests SET order_id=? WHERE user_id=? AND idempotency_key=?")
        .bind(id)
        .bind(actor.id)
        .bind(key)
        .execute(&mut *tx)
        .await?;
    let result = view(&mut tx, id, Some(actor.id)).await?;
    tx.commit().await?;
    Ok(result)
}
pub async fn cancel(state: &AppState, actor: &CurrentUser, id: Uuid) -> ApiResult<Value> {
    let mut tx = state.pool.begin().await?;
    let status: String =
        sqlx::query_scalar("SELECT status FROM orders WHERE id=? AND user_id=? FOR UPDATE")
            .bind(id)
            .bind(actor.id)
            .fetch_optional(&mut *tx)
            .await?
            .ok_or_else(AppError::missing)?;
    if !matches!(status.as_str(), "pending_payment" | "cancelled") {
        return Err(AppError::conflict("当前状态不能取消订单"));
    }
    if status == "pending_payment" {
        sqlx::query("UPDATE orders SET status='cancelled' WHERE id=?")
            .bind(id)
            .execute(&mut *tx)
            .await?;
    }
    let result = view(&mut tx, id, Some(actor.id)).await?;
    tx.commit().await?;
    Ok(result)
}
pub async fn list(state: &AppState, user: Option<Uuid>, q: Pagination) -> ApiResult<Value> {
    let items:Vec<Value>=sqlx::query_scalar("SELECT JSON_MERGE_PATCH(JSON_OBJECT('id',BIN_TO_UUID(o.id),'order_no',o.order_no,'user_id',BIN_TO_UUID(o.user_id),'amount_cents',o.amount_cents,'currency',o.currency,'status',o.status,'expires_at',o.expires_at,'paid_at',o.paid_at,'created_at',o.created_at,'updated_at',o.updated_at),JSON_OBJECT('item',JSON_OBJECT('id',BIN_TO_UUID(i.id),'product_id',BIN_TO_UUID(i.product_id),'product_title',i.product_title,'product_type',i.product_type,'unit_price_cents',i.unit_price_cents,'quantity',i.quantity,'subtotal_cents',i.subtotal_cents,'currency',i.currency),'user_email',u.email)) FROM orders o JOIN order_items i ON i.order_id=o.id JOIN users u ON u.id=o.user_id WHERE (? IS NULL OR o.user_id=?) AND (?='' OR o.status=?) ORDER BY o.created_at DESC,o.id DESC LIMIT ? OFFSET ?").bind(user).bind(user).bind(&q.status).bind(&q.status).bind(q.size).bind(q.offset).fetch_all(&state.pool).await?;
    let total: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM orders WHERE (? IS NULL OR user_id=?) AND (?='' OR status=?)",
    )
    .bind(user)
    .bind(user)
    .bind(&q.status)
    .bind(&q.status)
    .fetch_one(&state.pool)
    .await?;
    Ok(json!({"items":items,"total":total,"page":q.page,"page_size":q.size}))
}
pub async fn refund(
    state: &AppState,
    actor: &AdminUser,
    id: Uuid,
    input: RefundInput,
) -> ApiResult<Value> {
    use crate::common::pagination::text;
    text(&input.reference_no, 128)?;
    text(&input.note, 2000)?;
    if !(0..=100_000_000).contains(&input.amount_cents) {
        return Err(AppError::invalid("退款金额超出范围"));
    }
    let mut tx = state.pool.begin().await?;
    let order = sqlx::query("SELECT status,amount_cents FROM orders WHERE id=? FOR UPDATE")
        .bind(id)
        .fetch_optional(&mut *tx)
        .await?;
    let Some(order) = order else {
        return Err(AppError::missing());
    };
    let status: String = order.get("status");
    let amount_cents: i64 = order.get("amount_cents");
    if !matches!(status.as_str(), "completed" | "fulfilling") {
        return Err(AppError::conflict("只有已交付或履约中的订单可以登记退款"));
    }
    if input.amount_cents != amount_cents {
        return Err(AppError::invalid("当前只支持整单退款，金额必须与订单一致"));
    }
    let duplicated: bool =
        sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM refunds WHERE reference_no=?)")
            .bind(&input.reference_no)
            .fetch_one(&mut *tx)
            .await?;
    if duplicated {
        return Err(AppError::conflict("退款流水号已登记"));
    }
    sqlx::query("INSERT INTO refunds(id,order_id,amount_cents,reference_no,note,actor_id) VALUES(?,?,?,?,?,?)")
        .bind(Uuid::new_v4()).bind(id).bind(input.amount_cents).bind(&input.reference_no).bind(input.note.trim()).bind(actor.0.id)
        .execute(&mut *tx).await?;
    sqlx::query("UPDATE user_entitlements SET status='revoked',revoked_at=UTC_TIMESTAMP(6) WHERE order_item_id=(SELECT id FROM order_items WHERE order_id=?) AND status='active'")
        .bind(id).execute(&mut *tx).await?;
    sqlx::query("UPDATE service_tickets SET status='closed',closed_at=UTC_TIMESTAMP(6) WHERE order_id=? AND status='open'")
        .bind(id).execute(&mut *tx).await?;
    sqlx::query("UPDATE orders SET status='refunded' WHERE id=?")
        .bind(id)
        .execute(&mut *tx)
        .await?;
    crate::modules::admin::audit::record(
        &mut tx,
        actor.0.id,
        "order.refund",
        id,
        json!({"amount_cents": input.amount_cents, "reference_no": input.reference_no}),
    )
    .await?;
    let result = view(&mut tx, id, None).await?;
    tx.commit().await?;
    Ok(result)
}
