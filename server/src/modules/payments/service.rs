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
use sqlx::{Row, postgres::PgRow};
use uuid::Uuid;
// Every mutation locks order first, then proof. Upload and cancellation use the same order lock.
async fn locked(tx: &mut sqlx::PgConnection, proof: Uuid) -> ApiResult<(PgRow, PgRow)> {
    let id: Uuid = sqlx::query_scalar("SELECT order_id FROM payment_submissions WHERE id=$1")
        .bind(proof)
        .fetch_optional(&mut *tx)
        .await?
        .ok_or_else(AppError::missing)?;
    let order=sqlx::query("SELECT id,user_id,status,amount_cents,payment_instructions FROM orders WHERE id=$1 FOR UPDATE").bind(id).fetch_one(&mut *tx).await?;
    let payment = sqlx::query("SELECT id,status FROM payment_submissions WHERE id=$1 FOR UPDATE")
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
    sqlx::query("UPDATE payment_submissions SET status='approved',reviewed_by=$1,reviewed_at=NOW(),actual_amount_cents=$2,payment_channel=$3,transaction_reference=$4 WHERE id=$5").bind(actor.id).bind(input.actual_amount_cents).bind(&input.payment_channel).bind(reference).bind(proof).execute(&mut *tx).await?;
    let item = sqlx::query("SELECT id,product_id,product_type FROM order_items WHERE order_id=$1")
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
    sqlx::query("UPDATE orders SET status='completed',paid_at=NOW() WHERE id=$1")
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
    sqlx::query("UPDATE payment_submissions SET status='rejected',reviewed_by=$1,reviewed_at=NOW(),review_note=$2 WHERE id=$3").bind(actor.id).bind(reason.trim()).bind(proof).execute(&mut *tx).await?;
    sqlx::query("UPDATE orders SET status='pending_payment',expires_at=GREATEST(expires_at,NOW()+INTERVAL '24 hours') WHERE id=$1").bind(id).execute(&mut *tx).await?;
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
    let items:Vec<Value>=sqlx::query_scalar("SELECT (to_jsonb(p)-'file_id')||jsonb_build_object('order_no',o.order_no,'order_id',o.id,'amount_cents',o.amount_cents,'payment_channel_expected',o.payment_instructions->>'channel','product_title',i.product_title,'user_email',u.email,'overdue',p.status='pending' AND p.created_at<NOW()-INTERVAL '24 hours') FROM payment_submissions p JOIN orders o ON o.id=p.order_id JOIN order_items i ON i.order_id=o.id JOIN users u ON u.id=o.user_id WHERE ($1='' OR p.status=$1) AND (position(lower($2) in lower(o.order_no))>0 OR position(lower($2) in lower(u.email))>0) ORDER BY p.created_at DESC,p.id DESC LIMIT $3 OFFSET $4").bind(&q.status).bind(&q.q).bind(q.size).bind(q.offset).fetch_all(&state.pool).await?;
    let total:i64=sqlx::query_scalar("SELECT count(*) FROM payment_submissions p JOIN orders o ON o.id=p.order_id JOIN users u ON u.id=o.user_id WHERE ($1='' OR p.status=$1) AND (position(lower($2) in lower(o.order_no))>0 OR position(lower($2) in lower(u.email))>0)").bind(&q.status).bind(&q.q).fetch_one(&state.pool).await?;
    Ok(json!({"items":items,"total":total,"page":q.page,"page_size":q.size}))
}
