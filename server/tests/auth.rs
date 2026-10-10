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
    let hash: String = sqlx::query_scalar("SELECT password_hash FROM users WHERE id=?")
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
    assert_eq!(duplicate.status, 202);
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
    let unknown = app
        .request(
            "POST",
            "/api/v1/auth/login",
            json!({"email":"missing@example.test","password":"wrong-password-123"}),
            None,
            &[],
        )
        .await;
    assert_eq!(unknown.status, 401);
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
async fn unverified_account_cannot_login_and_verification_token_is_single_use() {
    let app = TestApp::start().await;
    let email = "verify-me@example.test";
    let registration = app
        .request(
            "POST",
            "/api/v1/auth/register",
            json!({"email":email,"password":"a-long-password-123"}),
            None,
            &[],
        )
        .await;
    assert_eq!(registration.status, 202);
    assert_eq!(
        app.request(
            "POST",
            "/api/v1/auth/login",
            json!({"email":email,"password":"a-long-password-123"}),
            None,
            &[]
        )
        .await
        .status,
        401
    );
    let token = app.email_token(email, "email_verification").await;
    let verified = app
        .request(
            "POST",
            "/api/v1/auth/verify-email",
            json!({"token":token}),
            None,
            &[],
        )
        .await;
    assert_eq!(verified.status, 200, "{}", verified.body);
    assert_eq!(
        app.request(
            "POST",
            "/api/v1/auth/verify-email",
            json!({"token":token}),
            None,
            &[]
        )
        .await
        .status,
        401
    );
    assert_eq!(
        app.request(
            "POST",
            "/api/v1/auth/login",
            json!({"email":email,"password":"a-long-password-123"}),
            None,
            &[]
        )
        .await
        .status,
        200
    );
}

#[tokio::test]
async fn registration_does_not_disclose_existing_email_and_tokens_expire() {
    let app = TestApp::start().await;
    let email = "private@example.test";
    let first = app
        .request(
            "POST",
            "/api/v1/auth/register",
            json!({"email":email,"password":"a-long-password-123"}),
            None,
            &[],
        )
        .await;
    let duplicate = app
        .request(
            "POST",
            "/api/v1/auth/register",
            json!({"email":email,"password":"a-long-password-123"}),
            None,
            &[],
        )
        .await;
    assert_eq!(first.status, 202);
    assert_eq!(duplicate.status, 202);
    assert_eq!(first.body["code"], duplicate.body["code"]);
    assert_eq!(first.body["message"], duplicate.body["message"]);
    assert_eq!(first.body["data"], duplicate.body["data"]);
    let token = app.email_token(email, "email_verification").await;
    sqlx::query("UPDATE auth_tokens SET expires_at=DATE_SUB(UTC_TIMESTAMP(6),INTERVAL 1 SECOND) WHERE token_hash=?").bind(devstore_server::modules::auth::service::digest(&token)).execute(&app.state.pool).await.unwrap();
    let expired = app
        .request(
            "POST",
            "/api/v1/auth/verify-email",
            json!({"token":token}),
            None,
            &[],
        )
        .await;
    assert_eq!(expired.status, 401);
}

#[tokio::test]
async fn password_forgot_is_enumeration_safe_and_reset_revokes_sessions() {
    let app = TestApp::start().await;
    let session = app.register_as("reset@example.test").await;
    let known = app
        .request(
            "POST",
            "/api/v1/auth/password/forgot",
            json!({"email":"reset@example.test"}),
            None,
            &[],
        )
        .await;
    let unknown = app
        .request(
            "POST",
            "/api/v1/auth/password/forgot",
            json!({"email":"unknown@example.test"}),
            None,
            &[],
        )
        .await;
    assert_eq!(known.status, unknown.status);
    assert_eq!(known.body["code"], unknown.body["code"]);
    assert_eq!(known.body["message"], unknown.body["message"]);
    assert_eq!(known.body["data"], unknown.body["data"]);
    let token = app
        .email_token("reset@example.test", "password_reset")
        .await;
    let reset = app
        .request(
            "POST",
            "/api/v1/auth/password/reset",
            json!({"token":token,"password":"a-different-password-456"}),
            None,
            &[],
        )
        .await;
    assert_eq!(reset.status, 200, "{}", reset.body);
    assert_eq!(
        app.request("GET", "/api/v1/users/me", Value::Null, Some(&session), &[])
            .await
            .status,
        401
    );
    assert_eq!(
        app.request(
            "POST",
            "/api/v1/auth/password/reset",
            json!({"token":token,"password":"another-password-789"}),
            None,
            &[]
        )
        .await
        .status,
        401
    );
    assert_eq!(
        app.request(
            "POST",
            "/api/v1/auth/login",
            json!({"email":"reset@example.test","password":"a-long-password-123"}),
            None,
            &[]
        )
        .await
        .status,
        401
    );
    assert_eq!(
        app.request(
            "POST",
            "/api/v1/auth/login",
            json!({"email":"reset@example.test","password":"a-different-password-456"}),
            None,
            &[]
        )
        .await
        .status,
        200
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
    sqlx::query("UPDATE users SET status=0 WHERE id=?")
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
        "SELECT count(*) FROM admin_audit_logs WHERE actor_id=? AND action='admin.bootstrap'",
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
    let sid: uuid::Uuid = sqlx::query_scalar("SELECT id FROM user_sessions WHERE user_id=?")
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
