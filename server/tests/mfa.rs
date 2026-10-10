mod support;
use serde_json::json;
use support::TestApp;
use uuid::Uuid;

#[tokio::test]
async fn admin_api_rejects_session_without_fresh_mfa_marker() {
    let app = TestApp::start().await;
    let admin = app.admin().await;
    sqlx::query("UPDATE user_sessions SET mfa_verified_at=NULL,mfa_method=NULL WHERE user_id=?")
        .bind(admin.id)
        .execute(&app.state.pool)
        .await
        .unwrap();
    let response = app
        .request(
            "GET",
            "/api/v1/admin/orders",
            serde_json::Value::Null,
            Some(&admin),
            &[],
        )
        .await;
    assert_eq!(response.status, 403);
    assert_eq!(response.body["code"], "ADMIN_MFA_REQUIRED");
}

#[tokio::test]
async fn admin_login_requires_totp_and_rejects_replayed_counter() {
    let app = TestApp::start().await;
    let admin = app.admin().await;
    let email: String = sqlx::query_scalar("SELECT email FROM users WHERE id=?")
        .bind(admin.id)
        .fetch_one(&app.state.pool)
        .await
        .unwrap();
    let no_code = app
        .request(
            "POST",
            "/api/v1/auth/login",
            json!({"email":email,"password":"a-long-password-123"}),
            None,
            &[],
        )
        .await;
    assert_eq!(no_code.status, 401);
    assert_eq!(no_code.body["code"], "MFA_REQUIRED");
    let last: i64 = sqlx::query_scalar("SELECT last_counter FROM admin_mfa WHERE user_id=?")
        .bind(admin.id)
        .fetch_one(&app.state.pool)
        .await
        .unwrap();
    let seed = app.admin_totp_secret(admin.id).await;
    let replay = devstore_server::infrastructure::mfa::totp_code(&seed, last as u64);
    assert!(
        devstore_server::infrastructure::mfa::verify(
            &app.state.pool,
            &app.state.config,
            admin.id,
            &replay,
            time::OffsetDateTime::now_utc()
        )
        .await
        .is_err()
    );
    assert_eq!(
        app.request(
            "GET",
            "/api/v1/admin/orders",
            serde_json::Value::Null,
            Some(&admin),
            &[]
        )
        .await
        .status,
        200
    );
}

#[tokio::test]
async fn recovery_code_is_consumed_once() {
    let app = TestApp::start().await;
    let email = format!("mfa-{}@example.test", Uuid::new_v4().simple());
    let _session = app.register_as(&email).await;
    let user: Uuid = sqlx::query_scalar("SELECT id FROM users WHERE email=?")
        .bind(&email)
        .fetch_one(&app.state.pool)
        .await
        .unwrap();
    sqlx::query(
        "INSERT INTO user_roles(user_id,role_id) SELECT ?,id FROM roles WHERE code='admin'",
    )
    .bind(user)
    .execute(&app.state.pool)
    .await
    .unwrap();
    let (seed, codes) =
        devstore_server::infrastructure::mfa::begin(&app.state.pool, &app.state.config, user)
            .await
            .unwrap();
    let secret = data_encoding::BASE32_NOPAD.decode(seed.as_bytes()).unwrap();
    let current = time::OffsetDateTime::now_utc()
        .unix_timestamp()
        .div_euclid(30) as u64;
    let confirmation = devstore_server::infrastructure::mfa::totp_code(&secret, current);
    devstore_server::infrastructure::mfa::confirm(
        &app.state.pool,
        &app.state.config,
        user,
        &confirmation,
        time::OffsetDateTime::now_utc(),
    )
    .await
    .unwrap();
    let login = app
        .request(
            "POST",
            "/api/v1/auth/login",
            json!({"email":email,"password":"a-long-password-123","totp_code":codes[0]}),
            None,
            &[],
        )
        .await;
    assert_eq!(login.status, 200, "{}", login.body);
    let replay = app
        .request(
            "POST",
            "/api/v1/auth/login",
            json!({"email":email,"password":"a-long-password-123","totp_code":codes[0]}),
            None,
            &[],
        )
        .await;
    assert_eq!(replay.status, 401);
}
