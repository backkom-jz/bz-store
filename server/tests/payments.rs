mod support;
use serde_json::{Value, json};
use support::TestApp;
fn approval(reference: &str) -> Value {
    json!({"actual_amount_cents":9900,"payment_channel":"bank","transaction_reference":reference,"receipt_confirmed":true})
}
#[tokio::test]
async fn it_approval_idempotency_and_reference_uniqueness() {
    let app = TestApp::start().await;
    let admin = app.admin().await;
    let buyer = app.register().await;
    let course = app.course(&admin, 9900).await;
    app.payment_config().await;
    let order = app.order(&buyer, course.product).await;
    let proof = app.proof(&buyer, order).await;
    let path = format!("/api/v1/admin/payments/{proof}/approve");
    assert_eq!(
        app.request("POST", &path, approval("test-001"), Some(&buyer), &[])
            .await
            .status,
        403
    );
    let mut invalid = approval("test-001");
    invalid["actual_amount_cents"] = json!(9800);
    assert_eq!(
        app.request("POST", &path, invalid, Some(&admin), &[])
            .await
            .status,
        422
    );
    let mut invalid = approval("test-001");
    invalid["receipt_confirmed"] = json!(false);
    assert_eq!(
        app.request("POST", &path, invalid, Some(&admin), &[])
            .await
            .status,
        422
    );
    let results = futures::future::join_all(
        (0..16).map(|_| app.request("POST", &path, approval("test-001"), Some(&admin), &[])),
    )
    .await;
    for r in results {
        assert_eq!(r.status, 200, "{}", r.body);
        assert_eq!(r.body["data"]["status"], "completed");
    }
    for query in [
        "SELECT count(*) FROM user_entitlements",
        "SELECT count(*) FROM admin_audit_logs WHERE action='payment.approve'",
    ] {
        let n: i64 = sqlx::query_scalar(query)
            .fetch_one(&app.state.pool)
            .await
            .unwrap();
        assert_eq!(n, 1);
    }
    let second = app.order(&buyer, course.product).await;
    let proof2 = app.proof(&buyer, second).await;
    assert_eq!(
        app.request(
            "POST",
            &format!("/api/v1/admin/payments/{proof2}/approve"),
            approval("test-001"),
            Some(&admin),
            &[]
        )
        .await
        .status,
        409
    );
    assert_eq!(
        app.request(
            "GET",
            &format!("/api/v1/orders/{second}"),
            Value::Null,
            Some(&buyer),
            &[]
        )
        .await
        .body["data"]["status"],
        "payment_review"
    );
}
#[tokio::test]
async fn it_review_rollback_reject_and_resubmit() {
    let app = TestApp::start().await;
    let admin = app.admin().await;
    let buyer = app.register().await;
    let course = app.course(&admin, 9900).await;
    app.payment_config().await;
    let order = app.order(&buyer, course.product).await;
    let proof = app.proof(&buyer, order).await;
    // Only the isolated MySQL test database receives this fault-injection constraint.
    sqlx::query("ALTER TABLE admin_audit_logs ADD CONSTRAINT reject_payment_audit CHECK(action<>'payment.approve')").execute(&app.state.pool).await.unwrap();
    let r = app
        .request(
            "POST",
            &format!("/api/v1/admin/payments/{proof}/approve"),
            approval("rollback-001"),
            Some(&admin),
            &[],
        )
        .await;
    assert_ne!(r.status, 200);
    let e: i64 = sqlx::query_scalar("SELECT count(*) FROM user_entitlements")
        .fetch_one(&app.state.pool)
        .await
        .unwrap();
    assert_eq!(e, 0);
    let current = app
        .request(
            "GET",
            &format!("/api/v1/orders/{order}"),
            Value::Null,
            Some(&buyer),
            &[],
        )
        .await;
    assert_eq!(current.body["data"]["status"], "payment_review");
    assert_eq!(current.body["data"]["payments"][0]["status"], "pending");
    sqlx::query("ALTER TABLE admin_audit_logs DROP CHECK reject_payment_audit")
        .execute(&app.state.pool)
        .await
        .unwrap();
    let path = format!("/api/v1/admin/payments/{proof}/reject");
    assert_eq!(
        app.request("POST", &path, json!({"reason":" "}), Some(&admin), &[])
            .await
            .status,
        422
    );
    sqlx::query(
        "UPDATE orders SET expires_at=DATE_SUB(UTC_TIMESTAMP(6),INTERVAL 2 DAY) WHERE id=?",
    )
    .bind(order)
    .execute(&app.state.pool)
    .await
    .unwrap();
    let before = time::OffsetDateTime::now_utc();
    let rejected = app
        .request(
            "POST",
            &path,
            json!({"reason":"请上传与订单金额相符的收款记录"}),
            Some(&admin),
            &[],
        )
        .await;
    assert_eq!(rejected.status, 200, "{}", rejected.body);
    let expires: time::OffsetDateTime =
        sqlx::query_scalar("SELECT expires_at FROM orders WHERE id=?")
            .bind(order)
            .fetch_one(&app.state.pool)
            .await
            .unwrap();
    assert!(expires >= before + time::Duration::days(1));
    assert_eq!(
        app.request(
            "POST",
            &format!("/api/v1/admin/payments/{proof}/approve"),
            approval("old-proof"),
            Some(&admin),
            &[]
        )
        .await
        .status,
        409
    );
    let new = app.proof(&buyer, order).await;
    let approve_path = format!("/api/v1/admin/payments/{new}/approve");
    let cancel_path = format!("/api/v1/orders/{order}/cancel");
    let (approve, expire, cancel) = tokio::join!(
        app.request(
            "POST",
            &approve_path,
            approval("new-proof"),
            Some(&admin),
            &[]
        ),
        devstore_server::jobs::expire_orders::expire_batch(
            &app.state,
            time::OffsetDateTime::now_utc() + time::Duration::days(7)
        ),
        app.request("POST", &cancel_path, json!({}), Some(&buyer), &[])
    );
    assert_eq!(approve.status, 200);
    assert_eq!(expire.unwrap(), 0);
    assert_eq!(cancel.status, 409);
}
