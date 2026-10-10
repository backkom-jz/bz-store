use super::dto::ApproveInput;
use crate::{
    common::{
        ApiResult, AppError,
        pagination::{Pagination, text},
    },
    middleware::auth::CurrentUser,
    modules::{admin::audit, entitlements::service::grant_tutorial, orders},
    state::AppState,
};
use serde_json::{Value, json};
use sqlx::{Row, mysql::MySqlRow};
use uuid::Uuid;
// Every mutation locks order first, then proof. Upload and cancellation use the same order lock.
async fn locked(tx: &mut sqlx::MySqlConnection, proof: Uuid) -> ApiResult<(MySqlRow, MySqlRow)> {
    let id: Uuid = sqlx::query_scalar("SELECT order_id FROM payment_submissions WHERE id=?")
        .bind(proof)
        .fetch_optional(&mut *tx)
        .await?
        .ok_or_else(AppError::missing)?;
    let order=sqlx::query("SELECT id,user_id,status,amount_cents,payment_instructions FROM orders WHERE id=? FOR UPDATE").bind(id).fetch_one(&mut *tx).await?;
    let payment = sqlx::query("SELECT id,status FROM payment_submissions WHERE id=? FOR UPDATE")
        .bind(proof)
        .fetch_one(&mut *tx)
        .await?;
    Ok((order, payment))
}
pub async fn approve(
    state: &AppState,
    actor: &CurrentUser,
    proof: Uuid,
    input: ApproveInput,
) -> ApiResult<Value> {
    if !actor.is_admin {
        return Err(AppError::forbidden());
    }
    let mut tx = state.pool.begin().await?;
    let (order, payment) = locked(&mut tx, proof).await?;
    let id: Uuid = order.get("id");
    if payment.get::<String, _>("status") == "approved" {
        let result = orders::service::view(&mut tx, id, None).await?;
        tx.commit().await?;
        return Ok(result);
    }
    if payment.get::<String, _>("status") != "pending"
        || order.get::<String, _>("status") != "payment_review"
    {
        return Err(AppError::conflict("凭证或订单已被处理"));
    }
    if !input.receipt_confirmed || input.actual_amount_cents != order.get::<i64, _>("amount_cents")
    {
        return Err(AppError::invalid(
            "请核实真实到账记录，金额必须与订单完全一致",
        ));
    }
    let reference = input.transaction_reference.trim();
    text(reference, 128)?;
    if reference.chars().any(char::is_control) {
        return Err(AppError::invalid("流水号不能含控制字符"));
    }
    let config: Value = order.get("payment_instructions");
    if config["channel"].as_str() != Some(&input.payment_channel) {
        return Err(AppError::invalid("收款渠道与订单付款说明不一致"));
    }
    sqlx::query("UPDATE payment_submissions SET status='approved',reviewed_by=?,reviewed_at=NOW(),actual_amount_cents=?,payment_channel=?,transaction_reference=? WHERE id=?").bind(actor.id).bind(input.actual_amount_cents).bind(&input.payment_channel).bind(reference).bind(proof).execute(&mut *tx).await?;
    let item = sqlx::query("SELECT id,product_id,product_type FROM order_items WHERE order_id=?")
        .bind(id)
        .fetch_one(&mut *tx)
        .await?;
    if item.get::<String, _>("product_type") != "tutorial" {
        return Err(AppError::conflict("当前版本只支持教程交付"));
    }
    grant_tutorial(
        &mut tx,
        order.get("user_id"),
        item.get("id"),
        item.get("product_id"),
    )
    .await?;
    sqlx::query("UPDATE orders SET status='completed',paid_at=NOW() WHERE id=?")
        .bind(id)
        .execute(&mut *tx)
        .await?;
    audit::record(&mut tx,actor.id,"payment.approve",proof,json!({"order_id":id,"amount_cents":input.actual_amount_cents,"channel":input.payment_channel,"transaction_reference":reference})).await?;
    let result = orders::service::view(&mut tx, id, None).await?;
    tx.commit().await?;
    Ok(result)
}
pub async fn reject(
    state: &AppState,
    actor: &CurrentUser,
    proof: Uuid,
    reason: &str,
) -> ApiResult<Value> {
    if !actor.is_admin {
        return Err(AppError::forbidden());
    }
    text(reason, 1000)?;
    let mut tx = state.pool.begin().await?;
    let (order, payment) = locked(&mut tx, proof).await?;
    let id: Uuid = order.get("id");
    if payment.get::<String, _>("status") == "rejected" {
        let result = orders::service::view(&mut tx, id, None).await?;
        tx.commit().await?;
        return Ok(result);
    }
    if payment.get::<String, _>("status") != "pending"
        || order.get::<String, _>("status") != "payment_review"
    {
        return Err(AppError::conflict("凭证或订单已被处理"));
    }
    sqlx::query("UPDATE payment_submissions SET status='rejected',reviewed_by=?,reviewed_at=NOW(),review_note=? WHERE id=?").bind(actor.id).bind(reason.trim()).bind(proof).execute(&mut *tx).await?;
    sqlx::query("UPDATE orders SET status='pending_payment',expires_at=GREATEST(expires_at,DATE_ADD(UTC_TIMESTAMP(6),INTERVAL 24 HOUR)) WHERE id=?").bind(id).execute(&mut *tx).await?;
    audit::record(
        &mut tx,
        actor.id,
        "payment.reject",
        proof,
        json!({"order_id":id,"reason":reason.trim()}),
    )
    .await?;
    let result = orders::service::view(&mut tx, id, None).await?;
    tx.commit().await?;
    Ok(result)
}
pub async fn list(state: &AppState, q: Pagination) -> ApiResult<Value> {
    let items:Vec<Value>=sqlx::query_scalar("SELECT JSON_MERGE_PATCH(JSON_OBJECT('id',BIN_TO_UUID(p.id),'order_id',BIN_TO_UUID(p.order_id),'status',p.status,'reviewed_by',BIN_TO_UUID(p.reviewed_by),'reviewed_at',p.reviewed_at,'review_note',p.review_note,'actual_amount_cents',p.actual_amount_cents,'payment_channel',p.payment_channel,'transaction_reference',p.transaction_reference,'created_at',p.created_at,'updated_at',p.updated_at),JSON_OBJECT('order_no',o.order_no,'amount_cents',o.amount_cents,'payment_channel_expected',JSON_UNQUOTE(JSON_EXTRACT(o.payment_instructions,'$.channel')),'product_title',i.product_title,'user_email',u.email,'overdue',p.status='pending' AND p.created_at<UTC_TIMESTAMP(6)-INTERVAL 24 HOUR)) FROM payment_submissions p JOIN orders o ON o.id=p.order_id JOIN order_items i ON i.order_id=o.id JOIN users u ON u.id=o.user_id WHERE (?='' OR p.status=?) AND (LOCATE(LOWER(?),LOWER(o.order_no))>0 OR LOCATE(LOWER(?),LOWER(u.email))>0) ORDER BY p.created_at DESC,p.id DESC LIMIT ? OFFSET ?").bind(&q.status).bind(&q.status).bind(&q.q).bind(&q.q).bind(q.size).bind(q.offset).fetch_all(&state.pool).await?;
    let total:i64=sqlx::query_scalar("SELECT count(*) FROM payment_submissions p JOIN orders o ON o.id=p.order_id JOIN users u ON u.id=o.user_id WHERE (?='' OR p.status=?) AND (LOCATE(LOWER(?),LOWER(o.order_no))>0 OR LOCATE(LOWER(?),LOWER(u.email))>0)").bind(&q.status).bind(&q.status).bind(&q.q).bind(&q.q).fetch_one(&state.pool).await?;
    Ok(json!({"items":items,"total":total,"page":q.page,"page_size":q.size}))
}
