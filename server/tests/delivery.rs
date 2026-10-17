mod support;

use axum::{body::Body, http::Request};
use serde_json::{Value, json};
use support::{Session, TestApp};
use uuid::Uuid;

const PDF: &[u8] = b"%PDF-1.4\nDevStore private delivery fixture\n%%EOF\n";

async fn draft(app: &TestApp, admin: &Session, kind: &str, price: i64) -> Uuid {
    let result = app
        .request(
            "POST",
            "/api/v1/admin/products",
            json!({
                "title":"交付回归商品", "slug":format!("delivery-{}", Uuid::new_v4().simple()),
                "type":kind, "price_cents":price
            }),
            Some(admin),
            &[],
        )
        .await;
    assert_eq!(result.status, 201, "{}", result.body);
    Uuid::parse_str(result.body["data"]["id"].as_str().unwrap()).unwrap()
}

async fn attach(app: &TestApp, admin: &Session, product: Uuid) -> support::TestResponse {
    let boundary = "delivery-regression";
    let mut body = format!("--{boundary}\r\nContent-Disposition: form-data; name=\"file\"; filename=\"private.pdf\"\r\nContent-Type: application/pdf\r\n\r\n").into_bytes();
    body.extend_from_slice(PDF);
    body.extend_from_slice(format!("\r\n--{boundary}--\r\n").as_bytes());
    app.raw(
        Request::builder()
            .method("POST")
            .uri(format!("/api/v1/admin/products/{product}/file"))
            .header("origin", &app.state.config.site_url)
            .header("cookie", &admin.cookie)
            .header("x-csrf-token", &admin.csrf)
            .header(
                "content-type",
                format!("multipart/form-data; boundary={boundary}"),
            )
            .body(Body::from(body))
            .unwrap(),
    )
    .await
}

async fn product(app: &TestApp, admin: &Session, kind: &str, price: i64) -> (Uuid, Option<Uuid>) {
    let id = draft(app, admin, kind, price).await;
    let file = if kind == "digital" {
        let uploaded = attach(app, admin, id).await;
        assert_eq!(uploaded.status, 201, "{}", uploaded.body);
        Some(Uuid::parse_str(uploaded.body["data"]["file_id"].as_str().unwrap()).unwrap())
    } else {
        None
    };
    let published = app
        .request(
            "PATCH",
            &format!("/api/v1/admin/products/{id}"),
            json!({"status":"published"}),
            Some(admin),
            &[],
        )
        .await;
    assert_eq!(published.status, 200, "{}", published.body);
    (id, file)
}

async fn paid(app: &TestApp, admin: &Session, buyer: &Session, product: Uuid) -> (Uuid, Uuid) {
    let order = app.order(buyer, product).await;
    let proof = app.proof(buyer, order).await;
    let approved = app.request("POST", &format!("/api/v1/admin/payments/{proof}/approve"), json!({
        "actual_amount_cents":9900, "payment_channel":"bank", "transaction_reference":Uuid::new_v4().to_string(), "receipt_confirmed":true
    }), Some(admin), &[]).await;
    assert_eq!(approved.status, 200, "{}", approved.body);
    (order, proof)
}

fn refund(reference: &str, amount: i64) -> Value {
    json!({"amount_cents":amount, "reference_no":reference, "note":"测试：已人工核对整单退款"})
}

async fn ticket(app: &TestApp, buyer: &Session) -> Uuid {
    let list = app
        .request("GET", "/api/v1/me/tickets", Value::Null, Some(buyer), &[])
        .await;
    assert_eq!(list.status, 200, "{}", list.body);
    assert_eq!(list.body["data"]["total"], 1);
    Uuid::parse_str(list.body["data"]["items"][0]["id"].as_str().unwrap()).unwrap()
}

#[tokio::test]
async fn digital_purchase_download_and_refund_enforce_current_ownership() {
    let app = TestApp::start().await;
    let admin = app.admin().await;
    let buyer = app.register().await;
    let stranger = app.register().await;
    app.payment_config().await;
    let (product, file) = product(&app, &admin, "digital", 9900).await;
    let file = file.unwrap();
    let download = format!("/api/v1/me/downloads/{file}");
    assert_eq!(app.get(&download).await.status, 401);
    assert_eq!(
        app.request("GET", &download, Value::Null, Some(&buyer), &[])
            .await
            .status,
        404
    );
    let (order, _) = paid(&app, &admin, &buyer, product).await;
    assert_eq!(
        app.request("GET", &download, Value::Null, Some(&stranger), &[])
            .await
            .status,
        404
    );
    let response = app
        .request("GET", &download, Value::Null, Some(&buyer), &[])
        .await;
    assert_eq!(response.status, 200);
    assert_eq!(response.bytes.as_ref(), PDF);
    assert_eq!(response.headers["content-type"], "application/pdf");
    assert_eq!(response.headers["cache-control"], "private, no-store");
    assert_eq!(response.headers["x-content-type-options"], "nosniff");
    assert!(
        response.headers["content-disposition"]
            .to_str()
            .unwrap()
            .ends_with(".pdf\"")
    );
    let path = format!("/api/v1/admin/orders/{order}/refund");
    let reference = Uuid::new_v4().to_string();
    assert_eq!(
        app.request("POST", &path, refund(&reference, 9800), Some(&admin), &[])
            .await
            .status,
        422
    );
    assert_eq!(
        app.request(
            "POST",
            &path,
            refund(&reference, 9900),
            Some(&stranger),
            &[]
        )
        .await
        .status,
        403
    );
    let refunded = app
        .request("POST", &path, refund(&reference, 9900), Some(&admin), &[])
        .await;
    assert_eq!(refunded.status, 200, "{}", refunded.body);
    assert_eq!(refunded.body["data"]["status"], "refunded");
    assert_eq!(
        app.request("GET", &download, Value::Null, Some(&buyer), &[])
            .await
            .status,
        404
    );
    let logs: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM download_logs WHERE user_id=? AND file_id=?")
            .bind(buyer.id)
            .bind(file)
            .fetch_one(&app.state.pool)
            .await
            .unwrap();
    assert_eq!(logs, 1);
    let library = app
        .request("GET", "/api/v1/me/library", Value::Null, Some(&buyer), &[])
        .await;
    assert_eq!(library.body["data"]["total"], 0);
}

#[tokio::test]
async fn digital_publication_requires_a_file_and_sold_files_cannot_be_replaced() {
    let app = TestApp::start().await;
    let admin = app.admin().await;
    let buyer = app.register().await;
    let id = draft(&app, &admin, "digital", 0).await;
    assert_eq!(
        app.request(
            "PATCH",
            &format!("/api/v1/admin/products/{id}"),
            json!({"status":"published"}),
            Some(&admin),
            &[]
        )
        .await
        .status,
        422
    );
    assert_eq!(attach(&app, &admin, id).await.status, 201);
    assert_eq!(
        app.request(
            "PATCH",
            &format!("/api/v1/admin/products/{id}"),
            json!({"status":"published"}),
            Some(&admin),
            &[]
        )
        .await
        .status,
        200
    );
    let order = app.order(&buyer, id).await;
    assert_eq!(
        app.request(
            "GET",
            &format!("/api/v1/orders/{order}"),
            Value::Null,
            Some(&buyer),
            &[]
        )
        .await
        .body["data"]["status"],
        "completed"
    );
    assert_eq!(attach(&app, &admin, id).await.status, 409);
}

#[tokio::test]
async fn refund_revokes_only_its_order_and_concurrent_repeats_do_not_duplicate_records() {
    let app = TestApp::start().await;
    let admin = app.admin().await;
    let buyer = app.register().await;
    app.payment_config().await;
    let (product, file) = product(&app, &admin, "digital", 9900).await;
    let (first, _) = paid(&app, &admin, &buyer, product).await;
    let (_second, _) = paid(&app, &admin, &buyer, product).await;
    let path = format!("/api/v1/admin/orders/{first}/refund");
    let body = refund(&Uuid::new_v4().to_string(), 9900);
    let results = futures::future::join_all(
        (0..4).map(|_| app.request("POST", &path, body.clone(), Some(&admin), &[])),
    )
    .await;
    assert_eq!(results.iter().filter(|r| r.status == 200).count(), 1);
    assert!(results.iter().all(|r| matches!(r.status, 200 | 409)));
    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM refunds WHERE order_id=?")
        .bind(first)
        .fetch_one(&app.state.pool)
        .await
        .unwrap();
    assert_eq!(count, 1);
    let audits: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM admin_audit_logs WHERE action='order.refund' AND target_id=?",
    )
    .bind(first)
    .fetch_one(&app.state.pool)
    .await
    .unwrap();
    assert_eq!(audits, 1);
    let response = app
        .request(
            "GET",
            &format!("/api/v1/me/downloads/{}", file.unwrap()),
            Value::Null,
            Some(&buyer),
            &[],
        )
        .await;
    assert_eq!(response.status, 200, "{}", response.body);
    let active: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM user_entitlements WHERE user_id=? AND product_id=? AND status='active'").bind(buyer.id).bind(product).fetch_one(&app.state.pool).await.unwrap();
    assert_eq!(active, 1);
}

#[tokio::test]
async fn service_approval_creates_one_private_ticket_and_acceptance_completes_order() {
    let app = TestApp::start().await;
    let admin = app.admin().await;
    let buyer = app.register().await;
    let stranger = app.register().await;
    app.payment_config().await;
    let (product, _) = product(&app, &admin, "service", 9900).await;
    let (order, proof) = paid(&app, &admin, &buyer, product).await;
    let repeated = app.request("POST", &format!("/api/v1/admin/payments/{proof}/approve"), json!({"actual_amount_cents":9900,"payment_channel":"bank","transaction_reference":"ignored-duplicate","receipt_confirmed":true}), Some(&admin), &[]).await;
    assert_eq!(repeated.status, 200);
    assert_eq!(repeated.body["data"]["status"], "fulfilling");
    let id = ticket(&app, &buyer).await;
    let path = format!("/api/v1/me/tickets/{id}");
    assert_eq!(
        app.request("GET", &path, Value::Null, Some(&stranger), &[])
            .await
            .status,
        404
    );
    assert_eq!(
        app.request(
            "POST",
            &format!("{path}/messages"),
            json!({"body":"跨账号留言"}),
            Some(&stranger),
            &[]
        )
        .await
        .status,
        404
    );
    assert_eq!(
        app.request(
            "POST",
            &format!("/api/v1/admin/tickets/{id}/messages"),
            json!({"body":"服务已交付"}),
            Some(&admin),
            &[]
        )
        .await
        .status,
        200
    );
    assert_eq!(
        app.request(
            "POST",
            &format!("{path}/accept"),
            json!({}),
            Some(&stranger),
            &[]
        )
        .await
        .status,
        404
    );
    let accepted = app
        .request(
            "POST",
            &format!("{path}/accept"),
            json!({}),
            Some(&buyer),
            &[],
        )
        .await;
    assert_eq!(accepted.status, 200, "{}", accepted.body);
    assert_eq!(accepted.body["data"]["status"], "completed");
    assert_eq!(
        app.request(
            "GET",
            &format!("/api/v1/orders/{order}"),
            Value::Null,
            Some(&buyer),
            &[]
        )
        .await
        .body["data"]["status"],
        "completed"
    );
    assert_eq!(
        app.request(
            "POST",
            &format!("{path}/messages"),
            json!({"body":"结束后不能留言"}),
            Some(&buyer),
            &[]
        )
        .await
        .status,
        409
    );
}

#[tokio::test]
async fn service_refund_closes_open_ticket_and_prevents_acceptance() {
    let app = TestApp::start().await;
    let admin = app.admin().await;
    let buyer = app.register().await;
    app.payment_config().await;
    let (product, _) = product(&app, &admin, "service", 9900).await;
    let (order, _) = paid(&app, &admin, &buyer, product).await;
    let id = ticket(&app, &buyer).await;
    let refunded = app
        .request(
            "POST",
            &format!("/api/v1/admin/orders/{order}/refund"),
            refund(&Uuid::new_v4().to_string(), 9900),
            Some(&admin),
            &[],
        )
        .await;
    assert_eq!(refunded.status, 200, "{}", refunded.body);
    let path = format!("/api/v1/me/tickets/{id}");
    assert_eq!(
        app.request("GET", &path, Value::Null, Some(&buyer), &[])
            .await
            .body["data"]["status"],
        "closed"
    );
    assert_eq!(
        app.request(
            "POST",
            &format!("{path}/accept"),
            json!({}),
            Some(&buyer),
            &[]
        )
        .await
        .status,
        409
    );
    assert_eq!(
        app.request(
            "POST",
            &format!("/api/v1/admin/tickets/{id}/messages"),
            json!({"body":"退款后不能继续交付"}),
            Some(&admin),
            &[]
        )
        .await
        .status,
        409
    );
}

#[tokio::test]
async fn blog_drafts_stay_private_until_published_and_unpublish_revokes_public_access() {
    let app = TestApp::start().await;
    let admin = app.admin().await;
    let buyer = app.register().await;
    let slug = format!("blog-{}", Uuid::new_v4().simple());
    let mut post = json!({"title":"测试文章","slug":slug,"excerpt":"公开摘要","content_md":"# 未发布正文","status":"draft"});
    assert_eq!(
        app.request(
            "POST",
            "/api/v1/admin/blog",
            post.clone(),
            Some(&buyer),
            &[]
        )
        .await
        .status,
        403
    );
    let created = app
        .request(
            "POST",
            "/api/v1/admin/blog",
            post.clone(),
            Some(&admin),
            &[],
        )
        .await;
    assert_eq!(created.status, 201, "{}", created.body);
    let id = created.body["data"]["id"].as_str().unwrap();
    assert_eq!(app.get(&format!("/api/v1/blog/{slug}")).await.status, 404);
    post["status"] = json!("published");
    assert_eq!(
        app.request(
            "PATCH",
            &format!("/api/v1/admin/blog/{id}"),
            post.clone(),
            Some(&admin),
            &[]
        )
        .await
        .status,
        200
    );
    let visible = app.get(&format!("/api/v1/blog/{slug}")).await;
    assert_eq!(visible.status, 200);
    assert_eq!(visible.body["data"]["content_md"], "# 未发布正文");
    let listing = app.get("/api/v1/blog").await;
    assert_eq!(listing.body["data"]["total"], 1);
    assert!(listing.body["data"]["items"][0].get("content_md").is_none());
    post["status"] = json!("draft");
    assert_eq!(
        app.request(
            "PATCH",
            &format!("/api/v1/admin/blog/{id}"),
            post.clone(),
            Some(&admin),
            &[]
        )
        .await
        .status,
        200
    );
    assert_eq!(app.get(&format!("/api/v1/blog/{slug}")).await.status, 404);
}

#[tokio::test]
async fn concurrent_service_acceptance_and_refund_preserve_consistent_terminal_states() {
    let app = TestApp::start().await;
    let admin = app.admin().await;
    let buyer = app.register().await;
    app.payment_config().await;
    let (product, _) = product(&app, &admin, "service", 9900).await;
    let (order, _) = paid(&app, &admin, &buyer, product).await;
    let id = ticket(&app, &buyer).await;
    let accept_path = format!("/api/v1/me/tickets/{id}/accept");
    let refund_path = format!("/api/v1/admin/orders/{order}/refund");
    let (accepted, refunded) = tokio::time::timeout(std::time::Duration::from_secs(15), async {
        tokio::join!(
            app.request("POST", &accept_path, json!({}), Some(&buyer), &[]),
            app.request(
                "POST",
                &refund_path,
                refund(&Uuid::new_v4().to_string(), 9900),
                Some(&admin),
                &[]
            )
        )
    })
    .await
    .expect("acceptance and refund must not deadlock or time out");
    assert_eq!(refunded.status, 200, "{}", refunded.body);
    assert!(matches!(accepted.status, 200 | 409), "{}", accepted.body);
    let order = app
        .request(
            "GET",
            &format!("/api/v1/orders/{order}"),
            Value::Null,
            Some(&buyer),
            &[],
        )
        .await;
    assert_eq!(order.body["data"]["status"], "refunded");
    let ticket = app
        .request(
            "GET",
            &format!("/api/v1/me/tickets/{id}"),
            Value::Null,
            Some(&buyer),
            &[],
        )
        .await;
    assert_eq!(
        ticket.body["data"]["status"],
        if accepted.status == 200 {
            "completed"
        } else {
            "closed"
        }
    );
}
