use super::dto::*;
use crate::{
    common::{ApiResult, AppError},
    state::AppState,
};
use argon2::{Argon2, PasswordHash, PasswordHasher, PasswordVerifier, password_hash::SaltString};
use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use hmac::{Hmac, Mac};
use rand::{RngCore, rngs::OsRng};
use sha2::{Digest, Sha256};
use sqlx::Row;
use time::OffsetDateTime;
use uuid::Uuid;
pub fn digest(value: &str) -> String {
    hex::encode(Sha256::digest(value.as_bytes()))
}
pub fn random_token() -> String {
    let mut bytes = [0u8; 32];
    OsRng.fill_bytes(&mut bytes);
    URL_SAFE_NO_PAD.encode(bytes)
}
pub fn csrf_token(state: &AppState, sid: Uuid) -> String {
    let mut mac = Hmac::<Sha256>::new_from_slice(state.config.csrf_secret.as_bytes())
        .expect("HMAC supports any key");
    mac.update(sid.as_bytes());
    URL_SAFE_NO_PAD.encode(mac.finalize().into_bytes())
}
pub fn normalize(input: &Credentials) -> ApiResult<String> {
    let email = input.email.trim().to_lowercase();
    if email.len() > 255 || !validator::ValidateEmail::validate_email(&email) {
        return Err(AppError::invalid("请输入有效邮箱"));
    }
    if !(12..=128).contains(&input.password.chars().count()) || input.password.len() > 512 {
        return Err(AppError::invalid("密码长度须为 12–128 个字符"));
    }
    Ok(email)
}
pub async fn hash_password(state: &AppState, password: String) -> ApiResult<String> {
    let slot = state
        .password_slots
        .clone()
        .acquire_owned()
        .await
        .map_err(|_| AppError::unavailable())?;
    tokio::task::spawn_blocking(move || {
        let _slot = slot;
        Argon2::default()
            .hash_password(password.as_bytes(), &SaltString::generate(&mut OsRng))
            .map(|p| p.to_string())
            .map_err(|_| AppError::internal())
    })
    .await
    .map_err(|_| AppError::internal())?
}
pub async fn public_user(state: &AppState, id: Uuid) -> ApiResult<PublicUser> {
    sqlx::query_as("SELECT u.id,u.email,u.display_name,EXISTS(SELECT 1 FROM user_roles ur JOIN roles r ON r.id=ur.role_id WHERE ur.user_id=u.id AND r.code='admin') AS is_admin FROM users u WHERE u.id=$1 AND u.status=1")
 .bind(id).fetch_optional(&state.pool).await?.ok_or_else(AppError::unauthorized)
}
pub fn access_token(state: &AppState, user: Uuid, sid: Uuid) -> ApiResult<String> {
    let now = OffsetDateTime::now_utc().unix_timestamp();
    jsonwebtoken::encode(
        &jsonwebtoken::Header::new(jsonwebtoken::Algorithm::HS256),
        &Claims {
            sub: user,
            sid,
            iat: now,
            exp: now + 900,
            iss: state.config.site_url.clone(),
            aud: "devstore".into(),
        },
        &jsonwebtoken::EncodingKey::from_secret(state.config.jwt_secret.as_bytes()),
    )
    .map_err(|_| AppError::internal())
}
pub async fn new_session(state: &AppState, user: PublicUser) -> ApiResult<SessionResult> {
    let sid = Uuid::new_v4();
    let refresh = random_token();
    sqlx::query("INSERT INTO user_sessions(id,user_id,token_hash,csrf_hash,expires_at) VALUES($1,$2,$3,$4,now()+interval '7 days')")
 .bind(sid).bind(user.id).bind(digest(&refresh)).bind(digest(&csrf_token(state,sid))).execute(&state.pool).await?;
    Ok(SessionResult {
        access: access_token(state, user.id, sid)?,
        refresh,
        user,
    })
}
pub async fn register(state: &AppState, input: Credentials) -> ApiResult<SessionResult> {
    let email = normalize(&input)?;
    let hash = hash_password(state, input.password).await?;
    let mut tx = state.pool.begin().await?;
    let id: Uuid =
        sqlx::query_scalar("INSERT INTO users(email,password_hash) VALUES($1,$2) RETURNING id")
            .bind(email)
            .bind(hash)
            .fetch_one(&mut *tx)
            .await?;
    sqlx::query(
        "INSERT INTO user_roles(user_id,role_id) SELECT $1,id FROM roles WHERE code='user'",
    )
    .bind(id)
    .execute(&mut *tx)
    .await?;
    tx.commit().await?;
    new_session(state, public_user(state, id).await?).await
}
pub async fn bootstrap_admin(state: &AppState, input: Credentials) -> ApiResult<PublicUser> {
    let email = normalize(&input)?;
    let hash = hash_password(state, input.password).await?;
    let mut tx = state.pool.begin().await?;
    let id: Uuid =
        sqlx::query_scalar("INSERT INTO users(email,password_hash) VALUES($1,$2) RETURNING id")
            .bind(email)
            .bind(hash)
            .fetch_one(&mut *tx)
            .await?;
    sqlx::query("INSERT INTO user_roles(user_id,role_id) SELECT $1,id FROM roles WHERE code IN ('user','admin')").bind(id).execute(&mut *tx).await?;
    sqlx::query("INSERT INTO admin_audit_logs(actor_id,action,target_id,detail) VALUES($1,'admin.bootstrap',$1,'{\"source\":\"local_cli\"}')").bind(id).execute(&mut *tx).await?;
    tx.commit().await?;
    public_user(state, id).await
}
pub async fn login(state: &AppState, input: Credentials) -> ApiResult<SessionResult> {
    let email = normalize(&input)?;
    let row = sqlx::query("SELECT id,password_hash,status FROM users WHERE email=$1")
        .bind(email)
        .fetch_optional(&state.pool)
        .await?;
    // Verify an Argon2 hash even for unknown accounts to avoid a fast email oracle.
    let hash = match &row {
        Some(r) => r.get::<String, _>("password_hash"),
        None => hash_password(state, "dummy-login-password".into()).await?,
    };
    let slot = state
        .password_slots
        .clone()
        .acquire_owned()
        .await
        .map_err(|_| AppError::unavailable())?;
    let password = input.password;
    let valid = tokio::task::spawn_blocking(move || {
        let _slot = slot;
        PasswordHash::new(&hash).ok().is_some_and(|h| {
            Argon2::default()
                .verify_password(password.as_bytes(), &h)
                .is_ok()
        })
    })
    .await
    .map_err(|_| AppError::internal())?;
    let row = row
        .filter(|r| r.get::<i16, _>("status") == 1)
        .ok_or_else(AppError::unauthorized)?;
    if !valid {
        return Err(AppError::unauthorized());
    }
    new_session(state, public_user(state, row.get("id")).await?).await
}
pub async fn refresh(state: &AppState, raw: &str, csrf: &str) -> ApiResult<SessionResult> {
    let mut tx = state.pool.begin().await?;
    let row=sqlx::query("SELECT s.id,s.user_id,s.csrf_hash FROM user_sessions s JOIN users u ON u.id=s.user_id WHERE s.token_hash=$1 AND s.revoked_at IS NULL AND s.expires_at>now() AND u.status=1 FOR UPDATE OF s").bind(digest(raw)).fetch_optional(&mut *tx).await?.ok_or_else(AppError::unauthorized)?;
    use subtle::ConstantTimeEq;
    let expected: String = row.get("csrf_hash");
    if expected
        .as_bytes()
        .ct_eq(digest(csrf).as_bytes())
        .unwrap_u8()
        != 1
    {
        return Err(AppError::forbidden());
    }
    let sid: Uuid = row.get("id");
    let user_id: Uuid = row.get("user_id");
    let refresh = random_token();
    sqlx::query("UPDATE user_sessions SET token_hash=$1 WHERE id=$2")
        .bind(digest(&refresh))
        .bind(sid)
        .execute(&mut *tx)
        .await?;
    tx.commit().await?;
    Ok(SessionResult {
        access: access_token(state, user_id, sid)?,
        refresh,
        user: public_user(state, user_id).await?,
    })
}
