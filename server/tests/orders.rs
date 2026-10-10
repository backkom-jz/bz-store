mod support;
use serde_json::{Value, json};
use support::TestApp;
use uuid::Uuid;

#[tokio::test]
async fn it_expiry_workers_preserve_reviewing_orders() {
    let app = TestApp::start().await;
    let admin = app.admin().await;
    let buyer = app.register().await;
    let course = app.course(&admin, 9900).await;
    app.payment_config().await;
    let mut ids = Vec::new();
    for _ in 0..2 {
        let result = app
            .request(
                "POST",
                "/api/v1/orders",
                json!({"product_id":course.product,"quantity":1}),
                Some(&buyer),
                &[("idempotency-key", &Uuid::new_v4().to_string())],
            )
            .await;
        assert_eq!(result.status, 201, "{}", result.body);
        ids.push(Uuid::parse_str(result.body["data"]["id"].as_str().unwrap()).unwrap());
    }
    sqlx::query("UPDATE orders SET expires_at=DATE_SUB(UTC_TIMESTAMP(6),INTERVAL 1 HOUR)")
        .execute(&app.state.pool)
        .await
        .unwrap();
    sqlx::query("UPDATE orders SET status='payment_review' WHERE id=?")
        .bind(ids[1])
        .execute(&app.state.pool)
        .await
        .unwrap();
    let now = time::OffsetDateTime::now_utc();
    let (a, b) = tokio::join!(
        devstore_server::jobs::expire_orders::expire_batch(&app.state, now),
        devstore_server::jobs::expire_orders::expire_batch(&app.state, now)
    );
    assert_eq!(a.unwrap() + b.unwrap(), 1);
    for (id, status) in [(ids[0], "cancelled"), (ids[1], "payment_review")] {
        let r = app
            .request(
                "GET",
                &format!("/api/v1/orders/{id}"),
                Value::Null,
                Some(&buyer),
                &[],
            )
            .await;
        assert_eq!(r.body["data"]["status"], status);
    }
    assert_eq!(
        app.request(
            "POST",
            &format!("/api/v1/orders/{}/cancel", ids[1]),
            json!({}),
            Some(&buyer),
            &[]
        )
        .await
        .status,
        409
    );
}

#[tokio::test]
async fn it_idempotent_checkout_and_price_snapshots() {
    let app = TestApp::start().await;
    let admin = app.admin().await;
    let buyer = app.register().await;
    let other = app.register().await;
    let course = app.course(&admin, 9900).await;
    let input = json!({"product_id":course.product,"quantity":1});
    let key = Uuid::new_v4().to_string();
    assert_eq!(
        app.request(
            "POST",
            "/api/v1/orders",
            input.clone(),
            Some(&buyer),
            &[("idempotency-key", &key)]
        )
        .await
        .status,
        503
    );
    app.payment_config().await;
    let headers = [("idempotency-key", key.as_str())];
    let responses = futures::future::join_all((0..16).map(|_| {
        app.request(
            "POST",
            "/api/v1/orders",
            input.clone(),
            Some(&buyer),
            &headers,
        )
    }))
    .await;
    for r in &responses {
        assert!([200, 201].contains(&r.status), "{}", r.body);
    }
    let id = responses[0].body["data"]["id"].as_str().unwrap();
    assert!(responses.iter().all(|r| r.body["data"]["id"] == id));
    let count: i64 = sqlx::query_scalar("SELECT count(*) FROM orders")
        .fetch_one(&app.state.pool)
        .await
        .unwrap();
    assert_eq!(count, 1);
    assert_eq!(responses[0].body["data"]["item"]["unit_price_cents"], 9900);
    assert_eq!(
        app.request(
            "GET",
            &format!("/api/v1/orders/{id}"),
            Value::Null,
            Some(&other),
            &[]
        )
        .await
        .status,
        404
    );
    assert_eq!(
        app.request(
            "PATCH",
            &format!("/api/v1/admin/products/{}", course.product),
            json!({"price_cents":19900}),
            Some(&admin),
            &[]
        )
        .await
        .status,
        200
    );
    let order = app
        .request(
            "GET",
            &format!("/api/v1/orders/{id}"),
            Value::Null,
            Some(&buyer),
            &[],
        )
        .await;
    assert_eq!(order.body["data"]["amount_cents"], 9900);
    assert_eq!(order.body["data"]["item"]["unit_price_cents"], 9900);
    assert!(
        [200, 201].contains(
            &app.request(
                "POST",
                "/api/v1/orders",
                input.clone(),
                Some(&other),
                &[("idempotency-key", &key)]
            )
            .await
            .status
        )
    );
    assert_eq!(
        app.request(
            "POST",
            "/api/v1/orders",
            json!({"product_id":Uuid::new_v4(),"quantity":1}),
            Some(&buyer),
            &[("idempotency-key", &key)]
        )
        .await
        .status,
        409
    );
    assert_eq!(
        app.request(
            "POST",
            "/api/v1/orders",
            json!({"product_id":course.product,"quantity":1,"amount_cents":1}),
            Some(&buyer),
            &[("idempotency-key", &key)]
        )
        .await
        .status,
        422
    );
}

#[tokio::test]
async fn it_free_checkout_and_cancel() {
    let app = TestApp::start().await;
    let admin = app.admin().await;
    let buyer = app.register().await;
    let free = app.course(&admin, 0).await;
    let key = Uuid::new_v4().to_string();
    for _ in 0..2 {
        let r = app
            .request(
                "POST",
                "/api/v1/orders",
                json!({"product_id":free.product,"quantity":1}),
                Some(&buyer),
                &[("idempotency-key", &key)],
            )
            .await;
        assert_eq!(r.body["data"]["status"], "completed", "{}", r.body);
    }
    let count: i64 = sqlx::query_scalar("SELECT count(*) FROM user_entitlements")
        .fetch_one(&app.state.pool)
        .await
        .unwrap();
    assert_eq!(count, 1);
    app.payment_config().await;
    let paid = app.course(&admin, 9900).await;
    let order = app
        .request(
            "POST",
            "/api/v1/orders",
            json!({"product_id":paid.product,"quantity":1}),
            Some(&buyer),
            &[("idempotency-key", &Uuid::new_v4().to_string())],
        )
        .await;
    let id = order.body["data"]["id"].as_str().unwrap();
    let cancel = app
        .request(
            "POST",
            &format!("/api/v1/orders/{id}/cancel"),
            json!({}),
            Some(&buyer),
            &[],
        )
        .await;
    assert_eq!(cancel.body["data"]["status"], "cancelled");
    assert_eq!(
        app.request(
            "POST",
            &format!("/api/v1/orders/{id}/cancel"),
            json!({}),
            Some(&buyer),
            &[]
        )
        .await
        .status,
        200
    );
}
