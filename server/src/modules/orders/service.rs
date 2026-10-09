use super::dto::*;
use crate::{
    common::{ApiResult, AppError, pagination::Pagination},
    middleware::auth::CurrentUser,
    modules::{auth::service::digest, entitlements::service::grant_tutorial},
    state::AppState,
};
use serde_json::{Value, json};
use sqlx::Row;
use uuid::Uuid;
pub const ORDER_VIEW: &str = "SELECT to_jsonb(o)-'payment_instructions'||jsonb_build_object('item',to_jsonb(i),'payments',COALESCE((SELECT jsonb_agg(jsonb_build_object('id',p.id,'status',p.status,'review_note',p.review_note,'created_at',p.created_at,'reviewed_at',p.reviewed_at) ORDER BY p.created_at,p.id) FROM payment_submissions p WHERE p.order_id=o.id),'[]'::jsonb)) FROM orders o JOIN order_items i ON i.order_id=o.id WHERE o.id=$1 AND ($2::uuid IS NULL OR o.user_id=$2)";
pub async fn view(conn: &mut sqlx::PgConnection, id: Uuid, user: Option<Uuid>) -> ApiResult<Value> {
    sqlx::query_scalar(ORDER_VIEW)
        .bind(id)
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
    sqlx::query("INSERT INTO order_requests(user_id,idempotency_key,request_fingerprint) VALUES($1,$2,$3) ON CONFLICT DO NOTHING").bind(actor.id).bind(key).bind(&fingerprint).execute(&mut *tx).await?;
    let existing=sqlx::query("SELECT request_fingerprint,order_id FROM order_requests WHERE user_id=$1 AND idempotency_key=$2 FOR UPDATE").bind(actor.id).bind(key).fetch_one(&mut *tx).await?;
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
        "SELECT id,title,type,price_cents,currency,status FROM products WHERE id=$1 FOR UPDATE",
    )
    .bind(input.product_id)
    .fetch_optional(&mut *tx)
    .await?
    .ok_or_else(AppError::missing)?;
    if product.get::<String, _>("status") != "published"
        || product.get::<String, _>("type") != "tutorial"
    {
        return Err(AppError::conflict("商品尚未上架或不支持交付"));
    }
    let price: i64 = product.get("price_cents");
    let payment: Value = if price > 0 {
        let value: Value =
            sqlx::query_scalar("SELECT value_json FROM site_settings WHERE key='manual_payment'")
                .fetch_optional(&mut *tx)
                .await?
                .ok_or_else(AppError::unavailable)?;
        let config: PaymentConfig =
            serde_json::from_value(value.clone()).map_err(|_| AppError::unavailable())?;
        config.validate().map_err(|_| AppError::unavailable())?;
        value
    } else {
        json!({})
    };
    let id = Uuid::new_v4();
    let item = Uuid::new_v4();
    let no = format!("DS{}", id.simple().to_string().to_uppercase());
    sqlx::query("INSERT INTO orders(id,order_no,user_id,amount_cents,status,payment_instructions) VALUES($1,$2,$3,$4,$5,$6)").bind(id).bind(no).bind(actor.id).bind(price).bind(if price==0{"completed"}else{"pending_payment"}).bind(payment).execute(&mut *tx).await?;
    sqlx::query("INSERT INTO order_items(id,order_id,product_id,product_title,product_type,unit_price_cents,quantity,subtotal_cents) VALUES($1,$2,$3,$4,'tutorial',$5,1,$5)").bind(item).bind(id).bind(input.product_id).bind(product.get::<String,_>("title")).bind(price).execute(&mut *tx).await?;
    if price == 0 {
        grant_tutorial(&mut tx, actor.id, item, input.product_id).await?;
    }
    sqlx::query("UPDATE order_requests SET order_id=$1 WHERE user_id=$2 AND idempotency_key=$3")
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
        sqlx::query_scalar("SELECT status FROM orders WHERE id=$1 AND user_id=$2 FOR UPDATE")
            .bind(id)
            .bind(actor.id)
            .fetch_optional(&mut *tx)
            .await?
            .ok_or_else(AppError::missing)?;
    if !matches!(status.as_str(), "pending_payment" | "cancelled") {
        return Err(AppError::conflict("当前状态不能取消订单"));
    }
    if status == "pending_payment" {
        sqlx::query("UPDATE orders SET status='cancelled' WHERE id=$1")
            .bind(id)
            .execute(&mut *tx)
            .await?;
    }
    let result = view(&mut tx, id, Some(actor.id)).await?;
    tx.commit().await?;
    Ok(result)
}
pub async fn list(state: &AppState, user: Option<Uuid>, q: Pagination) -> ApiResult<Value> {
    let items:Vec<Value>=sqlx::query_scalar("SELECT to_jsonb(o)-'payment_instructions'||jsonb_build_object('item',to_jsonb(i),'user_email',u.email) FROM orders o JOIN order_items i ON i.order_id=o.id JOIN users u ON u.id=o.user_id WHERE ($1::uuid IS NULL OR o.user_id=$1) AND ($2='' OR o.status=$2) ORDER BY o.created_at DESC,o.id DESC LIMIT $3 OFFSET $4").bind(user).bind(&q.status).bind(q.size).bind(q.offset).fetch_all(&state.pool).await?;
    let total:i64=sqlx::query_scalar("SELECT count(*) FROM orders WHERE ($1::uuid IS NULL OR user_id=$1) AND ($2='' OR status=$2)").bind(user).bind(&q.status).fetch_one(&state.pool).await?;
    Ok(json!({"items":items,"total":total,"page":q.page,"page_size":q.size}))
}
