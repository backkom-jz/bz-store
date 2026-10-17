mod support;
use serde_json::{Value, json};
use support::TestApp;

#[tokio::test]
async fn it_demo_seed_is_explicit_idempotent_and_has_no_payment_account() {
    let app = TestApp::start().await;
    let admin = app.admin().await;
    let actor = devstore_server::middleware::auth::CurrentUser {
        id: admin.id,
        session_id: uuid::Uuid::new_v4(),
        is_admin: true,
    };
    let a = devstore_server::modules::admin::operations::seed_demo(&app.state, &actor)
        .await
        .unwrap();
    let b = devstore_server::modules::admin::operations::seed_demo(&app.state, &actor)
        .await
        .unwrap();
    assert_eq!(a["id"], b["id"]);
    assert_eq!(a["price_cents"], 9900);
    assert_eq!(a["is_demo"], true);
    let count: i64 =
        sqlx::query_scalar("SELECT count(*) FROM site_settings WHERE setting_key='manual_payment'")
            .fetch_one(&app.state.pool)
            .await
            .unwrap();
    assert_eq!(count, 0);
    let t = app.get("/api/v1/tutorials/rust-axum-starter").await;
    assert_eq!(t.status, 200);
    assert_eq!(t.body["data"]["chapters"].as_array().unwrap().len(), 2);
    for slug in [
        "demo-rust-notes",
        "demo-architecture-review",
        "demo-retired-sample",
    ] {
        assert_eq!(
            app.get(&format!("/api/v1/products/{slug}")).await.status,
            200,
            "{slug}"
        );
    }
    assert_eq!(
        app.get("/api/v1/blog/shipping-digital-goods").await.status,
        200
    );
    let library = app
        .request("GET", "/api/v1/me/library", json!({}), Some(&admin), &[])
        .await;
    assert_eq!(library.status, 200, "{}", library.body);
    let titles: Vec<_> = library.body["data"]["items"]
        .as_array()
        .unwrap()
        .iter()
        .map(|item| item["title"].as_str().unwrap())
        .collect();
    assert!(titles.contains(&"Rust 速查笔记"));
    assert!(!titles.contains(&"已退款的示例文件"));
    let tickets = app
        .request("GET", "/api/v1/me/tickets", json!({}), Some(&admin), &[])
        .await;
    assert_eq!(tickets.status, 200, "{}", tickets.body);
    assert_eq!(tickets.body["data"]["items"].as_array().unwrap().len(), 1);
    let orders: i64 = sqlx::query_scalar("SELECT count(*) FROM orders WHERE user_id=?")
        .bind(admin.id)
        .fetch_one(&app.state.pool)
        .await
        .unwrap();
    assert_eq!(orders, 3);
    devstore_server::modules::admin::operations::seed_demo(&app.state, &actor)
        .await
        .unwrap();
    let orders_again: i64 = sqlx::query_scalar("SELECT count(*) FROM orders WHERE user_id=?")
        .bind(admin.id)
        .fetch_one(&app.state.pool)
        .await
        .unwrap();
    assert_eq!(orders_again, 3);
}

#[tokio::test]
async fn it_catalog_visibility_and_paid_body_is_not_in_metadata() {
    let app = TestApp::start().await;
    let admin = app.admin().await;
    let created = app
        .request(
            "POST",
            "/api/v1/admin/products",
            json!({"title":"Rust 入门","slug":"rust-starter","type":"tutorial","price_cents":9900}),
            Some(&admin),
            &[],
        )
        .await;
    assert_eq!(created.status, 201, "{}", created.body);
    let id = created.body["data"]["id"].as_str().unwrap();
    assert_eq!(app.get("/api/v1/products/rust-starter").await.status, 404);
    let t = app
        .request(
            "POST",
            "/api/v1/admin/tutorials",
            json!({"title":"Rust 入门","slug":"rust-starter","product_id":id}),
            Some(&admin),
            &[],
        )
        .await;
    assert_eq!(t.status, 201, "{}", t.body);
    let tid = t.body["data"]["id"].as_str().unwrap();
    let chapter=app.request("POST",&format!("/api/v1/admin/tutorials/{tid}/chapters"),json!({"title":"付费章节","content_md":"PRIVATE-CONTENT-NEVER-IN-METADATA","status":"published"}),Some(&admin),&[]).await;
    assert_eq!(chapter.status, 201, "{}", chapter.body);
    assert_eq!(
        app.request(
            "PATCH",
            &format!("/api/v1/admin/tutorials/{tid}"),
            json!({"status":"published"}),
            Some(&admin),
            &[]
        )
        .await
        .status,
        200
    );
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
    let public = app.get("/api/v1/tutorials/rust-starter").await;
    assert_eq!(public.status, 200, "{}", public.body);
    assert!(!public.body.to_string().contains("PRIVATE-CONTENT"));
    assert!(!public.body.to_string().contains("content_md"));
    assert_eq!(public.body["data"]["chapters"][0]["is_preview"], false);
    assert_eq!(app.get("/api/v1/products").await.body["data"]["total"], 1);
    assert_eq!(
        app.get("/api/v1/products?q=%25").await.body["data"]["total"],
        0
    );
    assert_eq!(app.get("/api/v1/products?page_size=101").await.status, 422);
    assert_eq!(
        app.request(
            "PATCH",
            &format!("/api/v1/admin/products/{id}"),
            json!({"status":"archived"}),
            Some(&admin),
            &[]
        )
        .await
        .status,
        200
    );
    assert_eq!(app.get("/api/v1/products/rust-starter").await.status, 404);
}

#[tokio::test]
async fn it_admin_editor_and_validation() {
    let app = TestApp::start().await;
    let admin = app.admin().await;
    let user = app.register().await;
    let input = json!({"title":"Course","slug":"course","type":"tutorial","price_cents":9900});
    assert_eq!(
        app.request(
            "POST",
            "/api/v1/admin/products",
            input.clone(),
            Some(&user),
            &[]
        )
        .await
        .status,
        403
    );
    assert_eq!(
        app.request(
            "POST",
            "/api/v1/admin/products",
            input.clone(),
            Some(&admin),
            &[]
        )
        .await
        .status,
        201
    );
    assert_eq!(
        app.request("POST", "/api/v1/admin/products", input, Some(&admin), &[])
            .await
            .status,
        409
    );
    for bad in [
        json!({"title":"Bad","slug":"bad","type":"service","price_cents":9900,"status":"published"}),
        json!({"title":"Bad","slug":"bad","type":"tutorial","price_cents":-1}),
        json!({"title":"Bad","slug":"../bad","type":"tutorial","price_cents":99}),
        json!({"title":"Bad","slug":"bad","type":"tutorial","price_cents":1.2}),
    ] {
        assert_eq!(
            app.request("POST", "/api/v1/admin/products", bad, Some(&admin), &[])
                .await
                .status,
            422
        );
    }
    let audit = app
        .request(
            "GET",
            "/api/v1/admin/audit-logs",
            Value::Null,
            Some(&admin),
            &[],
        )
        .await;
    assert_eq!(audit.status, 200);
    assert_eq!(audit.body["data"]["total"], 1);
}
