mod support;
use serde_json::{Value, json};
use support::TestApp;
#[tokio::test]
async fn it_paid_content_access_and_archived_entitlements() {
    let app = TestApp::start().await;
    let admin = app.admin().await;
    let buyer = app.register().await;
    let other = app.register().await;
    let course = app.course(&admin, 9900).await;
    app.payment_config().await;
    let path = format!(
        "/api/v1/tutorials/{}/chapters/{}",
        course.tutorial, course.paid
    );
    let preview = format!(
        "/api/v1/tutorials/{}/chapters/{}",
        course.tutorial, course.preview
    );
    assert_eq!(
        app.request("GET", &path, Value::Null, None, &[])
            .await
            .status,
        401
    );
    assert_eq!(
        app.request("GET", &path, Value::Null, Some(&buyer), &[])
            .await
            .status,
        403
    );
    assert_eq!(
        app.request("GET", &preview, Value::Null, None, &[])
            .await
            .status,
        200
    );
    let order = app.order(&buyer, course.product).await;
    let proof = app.proof(&buyer, order).await;
    let r=app.request("POST",&format!("/api/v1/admin/payments/{proof}/approve"),json!({"actual_amount_cents":9900,"payment_channel":"bank","transaction_reference":"access-test","receipt_confirmed":true}),Some(&admin),&[]).await;
    assert_eq!(r.status, 200, "{}", r.body);
    let content = app
        .request("GET", &path, Value::Null, Some(&buyer), &[])
        .await;
    assert_eq!(content.status, 200);
    assert!(
        content.body["data"]["content_md"]
            .as_str()
            .unwrap()
            .contains("Paid secret")
    );
    assert_eq!(
        app.request("GET", &path, Value::Null, Some(&other), &[])
            .await
            .status,
        403
    );
    let library = app
        .request("GET", "/api/v1/me/library", Value::Null, Some(&buyer), &[])
        .await;
    assert_eq!(library.status, 200);
    assert_eq!(library.body["data"]["items"].as_array().unwrap().len(), 1);
    assert_eq!(
        app.request(
            "PATCH",
            &format!("/api/v1/admin/products/{}", course.product),
            json!({"status":"archived"}),
            Some(&admin),
            &[]
        )
        .await
        .status,
        200
    );
    assert_eq!(
        app.request("GET", &path, Value::Null, Some(&buyer), &[])
            .await
            .status,
        200
    );
    assert_eq!(
        app.request(
            "GET",
            &format!("/api/v1/products/{}", course.slug),
            Value::Null,
            None,
            &[]
        )
        .await
        .status,
        404
    );
    let detail = app
        .request(
            "GET",
            &format!("/api/v1/tutorials/{}", course.slug),
            Value::Null,
            Some(&buyer),
            &[],
        )
        .await;
    assert_eq!(detail.status, 200);
    assert!(!detail.body.to_string().contains("Paid secret"));
    assert_eq!(
        app.request(
            "PATCH",
            &format!(
                "/api/v1/admin/tutorials/{}/chapters/{}",
                course.tutorial, course.paid
            ),
            json!({"status":"draft"}),
            Some(&admin),
            &[]
        )
        .await
        .status,
        200
    );
    assert_eq!(
        app.request("GET", &path, Value::Null, Some(&buyer), &[])
            .await
            .status,
        404
    );
    assert_eq!(
        app.request(
            "PATCH",
            &format!(
                "/api/v1/admin/tutorials/{}/chapters/{}",
                course.tutorial, course.paid
            ),
            json!({"status":"published"}),
            Some(&admin),
            &[]
        )
        .await
        .status,
        200
    );
    sqlx::query("UPDATE user_entitlements SET status='revoked',revoked_at=NOW() WHERE user_id=$1")
        .bind(buyer.id)
        .execute(&app.state.pool)
        .await
        .unwrap();
    assert_eq!(
        app.request("GET", &path, Value::Null, Some(&buyer), &[])
            .await
            .status,
        403
    );
}
