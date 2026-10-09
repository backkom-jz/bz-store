mod support;
use devstore_server::modules::auth::{
    dto::{Claims, Credentials},
    service,
};
use serde_json::{Value, json};
use support::TestApp;

#[tokio::test]
async fn it_auth_lifecycle() {
    let app = TestApp::start().await;
    let s = app.register_as("Alice@EXAMPLE.test").await;
    let me = app
        .request("GET", "/api/v1/users/me", Value::Null, Some(&s), &[])
        .await;
    assert_eq!(me.status, 200);
    assert_eq!(me.body["data"]["email"], "alice@example.test");
    let hash: String = sqlx::query_scalar("SELECT password_hash FROM users WHERE id=$1")
        .bind(s.id)
        .fetch_one(&app.state.pool)
        .await
        .unwrap();
    assert!(hash.starts_with("$argon2id$"));
    assert_ne!(hash, "a-long-password-123");
    let duplicate = app
        .request(
            "POST",
            "/api/v1/auth/register",
            json!({"email":"ALICE@example.test","password":"a-long-password-123"}),
            None,
            &[],
        )
        .await;
    assert_eq!(duplicate.status, 409);
    let bad = app
        .request(
            "POST",
            "/api/v1/auth/login",
            json!({"email":"alice@example.test","password":"wrong-password-123"}),
            None,
            &[],
        )
        .await;
    assert_eq!(bad.status, 401);
    let good = app
        .request(
            "POST",
            "/api/v1/auth/login",
            json!({"email":"ALICE@example.test","password":"a-long-password-123"}),
            None,
            &[],
        )
        .await;
    assert_eq!(good.status, 200);
    assert!(!good.body.to_string().contains("token"));
    assert!(
        good.headers
            .get_all("set-cookie")
            .iter()
            .all(|h| h.to_str().unwrap().contains("HttpOnly"))
    );
    assert_eq!(
        app.request("POST", "/api/v1/auth/logout", json!({}), Some(&s), &[])
            .await
            .status,
        200
    );
    assert_eq!(
        app.request("GET", "/api/v1/users/me", Value::Null, Some(&s), &[])
            .await
            .status,
        401
    );
}

#[tokio::test]
async fn it_csrf_and_origin() {
    let app = TestApp::start().await;
    let mut s = app.register().await;
    s.csrf = "incorrect".into();
    assert_eq!(
        app.request("POST", "/api/v1/auth/logout", json!({}), Some(&s), &[])
            .await
            .status,
        403
    );
    let request = axum::http::Request::builder()
        .method("POST")
        .uri("/api/v1/auth/login")
        .header("origin", "https://attacker.test")
        .header("content-type", "application/json")
        .body(axum::body::Body::from(
            r#"{"email":"x@example.test","password":"a-long-password-123"}"#,
        ))
        .unwrap();
    assert_eq!(app.raw(request).await.status, 403);
}

#[tokio::test]
async fn it_refresh_race() {
    let app = TestApp::start().await;
    let s = app.register().await;
    let (a, b) = tokio::join!(
        app.request("POST", "/api/v1/auth/refresh", json!({}), Some(&s), &[]),
        app.request("POST", "/api/v1/auth/refresh", json!({}), Some(&s), &[])
    );
    assert_eq!(
        [a.status, b.status]
            .into_iter()
            .filter(|s| *s == 200)
            .count(),
        1
    );
    assert_eq!(
        [a.status, b.status]
            .into_iter()
            .filter(|s| *s == 401)
            .count(),
        1
    );
    let fresh = app
        .session_from(if a.status == 200 { &a } else { &b })
        .await;
    assert_eq!(
        app.request("GET", "/api/v1/users/me", Value::Null, Some(&fresh), &[])
            .await
            .status,
        200
    );
    assert_eq!(
        app.request("POST", "/api/v1/auth/refresh", json!({}), Some(&s), &[])
            .await
            .status,
        401
    );
    sqlx::query("UPDATE users SET status=0 WHERE id=$1")
        .bind(s.id)
        .execute(&app.state.pool)
        .await
        .unwrap();
    assert_eq!(
        app.request("GET", "/api/v1/users/me", Value::Null, Some(&fresh), &[])
            .await
            .status,
        401
    );
}

#[tokio::test]
async fn it_shared_rate_limit() {
    let app = TestApp::start().await;
    for _ in 0..10 {
        assert_eq!(
            app.request(
                "POST",
                "/api/v1/auth/login",
                json!({"email":"nobody@example.test","password":"invalid-password"}),
                None,
                &[]
            )
            .await
            .status,
            401
        );
    }
    let r = app
        .request(
            "POST",
            "/api/v1/auth/login",
            json!({"email":"nobody@example.test","password":"invalid-password"}),
            None,
            &[],
        )
        .await;
    assert_eq!(r.status, 429);
    assert!(r.headers.contains_key("retry-after"));
}

#[tokio::test]
async fn it_admin_bootstrap_is_explicit_and_audited() {
    let app = TestApp::start().await;
    let admin = service::bootstrap_admin(
        &app.state,
        Credentials {
            email: "operator@example.test".into(),
            password: "a-long-password-123".into(),
        },
    )
    .await
    .unwrap();
    assert!(admin.is_admin);
    let count: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM admin_audit_logs WHERE actor_id=$1 AND action='admin.bootstrap'",
    )
    .bind(admin.id)
    .fetch_one(&app.state.pool)
    .await
    .unwrap();
    assert_eq!(count, 1);
    assert_eq!(
        service::bootstrap_admin(
            &app.state,
            Credentials {
                email: "operator@example.test".into(),
                password: "replacement-password-123".into()
            }
        )
        .await
        .unwrap_err()
        .status,
        409
    );
}

#[tokio::test]
async fn it_expired_access_can_refresh_without_exposing_secrets() {
    let app = TestApp::start().await;
    let mut s = app.register().await;
    let sid: uuid::Uuid = sqlx::query_scalar("SELECT id FROM user_sessions WHERE user_id=$1")
        .bind(s.id)
        .fetch_one(&app.state.pool)
        .await
        .unwrap();
    let now = time::OffsetDateTime::now_utc().unix_timestamp();
    let expired = jsonwebtoken::encode(
        &jsonwebtoken::Header::default(),
        &Claims {
            sub: s.id,
            sid,
            iat: now - 1800,
            exp: now - 900,
            iss: app.state.config.site_url.clone(),
            aud: "devstore".into(),
        },
        &jsonwebtoken::EncodingKey::from_secret(app.state.config.jwt_secret.as_bytes()),
    )
    .unwrap();
    let refresh = s
        .cookie
        .split("; ")
        .find(|s| s.starts_with("ds_refresh="))
        .unwrap()
        .to_string();
    s.cookie = format!("ds_access={expired}; {refresh}");
    assert_eq!(
        app.request("GET", "/api/v1/users/me", Value::Null, Some(&s), &[])
            .await
            .status,
        401
    );
    assert_eq!(
        app.request("GET", "/api/v1/auth/csrf", Value::Null, Some(&s), &[])
            .await
            .status,
        200
    );
    assert_eq!(
        app.request("POST", "/api/v1/auth/refresh", json!({}), Some(&s), &[])
            .await
            .status,
        200
    );
}
