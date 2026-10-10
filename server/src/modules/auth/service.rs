use super::dto::*;
use crate::{
    common::{ApiResult, AppError},
    infrastructure::email::{self, EmailMessage},
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

// Precomputed with Argon2id (m=19_456, t=2, p=1). Unknown-account logins
// verify this fixed hash so they perform one Argon2 verification, like a
// known-account login, without hashing a new dummy password on every request.
const DUMMY_PASSWORD_HASH: &str = "$argon2id$v=19$m=19456,t=2,p=1$ZGV2c3RvcmUtc3RhdGljLXNhbHQtMjAyNg$ilBBvdl/BXN+P1wFQ2FL30I+yczbAb8XVgebaZyIWyw";
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
    sqlx::query_as("SELECT u.id,u.email,u.display_name,EXISTS(SELECT 1 FROM user_roles ur JOIN roles r ON r.id=ur.role_id WHERE ur.user_id=u.id AND r.code='admin') AS is_admin FROM users u WHERE u.id=? AND u.status=1")
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
    new_session_with_mfa(state, user, None).await
}
async fn new_session_with_mfa(
    state: &AppState,
    user: PublicUser,
    mfa_method: Option<&str>,
) -> ApiResult<SessionResult> {
    let sid = Uuid::new_v4();
    let refresh = random_token();
    sqlx::query("INSERT INTO user_sessions(id,user_id,token_hash,csrf_hash,expires_at,mfa_verified_at,mfa_method) VALUES(?,?,?,?,DATE_ADD(UTC_TIMESTAMP(6), INTERVAL 7 DAY),IF(? IS NULL,NULL,UTC_TIMESTAMP(6)),?)")
 .bind(sid).bind(user.id).bind(digest(&refresh)).bind(digest(&csrf_token(state,sid))).bind(mfa_method).bind(mfa_method).execute(&state.pool).await?;
    Ok(SessionResult {
        access: access_token(state, user.id, sid)?,
        refresh,
        user,
    })
}
async fn queue_auth_email(
    state: &AppState,
    tx: &mut sqlx::Transaction<'_, sqlx::MySql>,
    user: Uuid,
    email_address: &str,
    purpose: &str,
) -> ApiResult<()> {
    let config = state
        .config
        .email
        .as_ref()
        .ok_or_else(AppError::unavailable)?;
    let token = random_token();
    sqlx::query("INSERT INTO auth_tokens(id,user_id,purpose,token_hash,expires_at) VALUES(?,?,?,?,DATE_ADD(UTC_TIMESTAMP(6),INTERVAL ? HOUR))")
        .bind(Uuid::new_v4()).bind(user).bind(purpose).bind(digest(&token))
        .bind(if purpose == "email_verification" { 24_i64 } else { 1_i64 })
        .execute(&mut **tx).await?;
    let (path, subject, lead) = if purpose == "email_verification" {
        (
            "/verify-email",
            "请验证你的 DevStore 邮箱",
            "完成邮箱验证：",
        )
    } else {
        (
            "/reset-password",
            "重置你的 DevStore 密码",
            "如需重置密码，请打开此链接：",
        )
    };
    let message = EmailMessage {
        to: email_address.to_owned(),
        subject: subject.to_owned(),
        body: format!(
            "{lead}\n{}/{path}?token={token}\n\n如果这不是你的操作，请忽略此邮件。",
            state.config.site_url
        ),
    };
    email::enqueue(tx, &config.outbox_key, &message, purpose).await?;
    Ok(())
}

pub async fn register(state: &AppState, input: Credentials) -> ApiResult<()> {
    let email = normalize(&input)?;
    if state.config.email.is_none() {
        return Err(AppError::unavailable());
    }
    let hash = hash_password(state, input.password).await?;
    let mut tx = state.pool.begin().await?;
    let id = Uuid::new_v4();
    let inserted = sqlx::query("INSERT IGNORE INTO users(id,email,password_hash) VALUES(?,?,?)")
        .bind(id)
        .bind(&email)
        .bind(hash)
        .execute(&mut *tx)
        .await?;
    if inserted.rows_affected() == 0 {
        tx.commit().await?;
        return Ok(());
    }
    sqlx::query("INSERT INTO user_roles(user_id,role_id) SELECT ?,id FROM roles WHERE code='user'")
        .bind(id)
        .execute(&mut *tx)
        .await?;
    queue_auth_email(state, &mut tx, id, &email, "email_verification").await?;
    tx.commit().await?;
    Ok(())
}
pub async fn bootstrap_admin(state: &AppState, input: Credentials) -> ApiResult<PublicUser> {
    let email = normalize(&input)?;
    let hash = hash_password(state, input.password).await?;
    let mut tx = state.pool.begin().await?;
    let id = Uuid::new_v4();
    sqlx::query("INSERT INTO users(id,email,password_hash,email_verified_at) VALUES(?,?,?,UTC_TIMESTAMP(6))")
        .bind(id)
        .bind(email)
        .bind(hash)
        .execute(&mut *tx)
        .await?;
    sqlx::query("INSERT INTO user_roles(user_id,role_id) SELECT ?,id FROM roles WHERE code IN ('user','admin')").bind(id).execute(&mut *tx).await?;
    sqlx::query("INSERT INTO admin_audit_logs(id,actor_id,action,target_id,detail) VALUES(? ,?,'admin.bootstrap',?,JSON_OBJECT('source','local_cli'))").bind(Uuid::new_v4()).bind(id).bind(id).execute(&mut *tx).await?;
    tx.commit().await?;
    public_user(state, id).await
}
pub async fn login(
    state: &AppState,
    input: Credentials,
    totp_code: Option<&str>,
) -> ApiResult<SessionResult> {
    let email = normalize(&input)?;
    let row = sqlx::query("SELECT id,password_hash,status FROM users WHERE email=?")
        .bind(email)
        .fetch_optional(&state.pool)
        .await?;
    // Verify one Argon2 hash even for unknown accounts to avoid a fast email oracle.
    let hash = match &row {
        Some(r) => r.get::<String, _>("password_hash"),
        None => DUMMY_PASSWORD_HASH.to_owned(),
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
    let verified: Option<OffsetDateTime> =
        sqlx::query_scalar("SELECT email_verified_at FROM users WHERE id=?")
            .bind(row.get::<Uuid, _>("id"))
            .fetch_one(&state.pool)
            .await?;
    if verified.is_none() {
        return Err(AppError::unauthorized());
    }
    let user = public_user(state, row.get("id")).await?;
    let mfa_method = if user.is_admin {
        let code = totp_code.ok_or_else(|| {
            AppError::new(
                axum::http::StatusCode::UNAUTHORIZED,
                "MFA_REQUIRED",
                "请输入管理员动态验证码或恢复码",
            )
        })?;
        Some(
            crate::infrastructure::mfa::verify(
                &state.pool,
                &state.config,
                user.id,
                code,
                OffsetDateTime::now_utc(),
            )
            .await?,
        )
    } else {
        None
    };
    new_session_with_mfa(state, user, mfa_method).await
}

pub async fn verify_email(state: &AppState, raw: &str) -> ApiResult<()> {
    consume_auth_token(state, raw, "email_verification")
        .await
        .map(|_| ())
}

async fn consume_auth_token(state: &AppState, raw: &str, purpose: &str) -> ApiResult<Uuid> {
    let mut tx = state.pool.begin().await?;
    let row = sqlx::query("SELECT id,user_id FROM auth_tokens WHERE token_hash=? AND purpose=? AND consumed_at IS NULL AND expires_at>UTC_TIMESTAMP(6) FOR UPDATE")
        .bind(digest(raw)).bind(purpose).fetch_optional(&mut *tx).await?.ok_or_else(AppError::unauthorized)?;
    let token_id: Uuid = row.get("id");
    let user_id: Uuid = row.get("user_id");
    sqlx::query(
        "UPDATE auth_tokens SET consumed_at=UTC_TIMESTAMP(6) WHERE id=? AND consumed_at IS NULL",
    )
    .bind(token_id)
    .execute(&mut *tx)
    .await?;
    if purpose == "email_verification" {
        sqlx::query("UPDATE users SET email_verified_at=COALESCE(email_verified_at,UTC_TIMESTAMP(6)) WHERE id=?")
            .bind(user_id).execute(&mut *tx).await?;
        sqlx::query("UPDATE auth_tokens SET consumed_at=UTC_TIMESTAMP(6) WHERE user_id=? AND purpose='email_verification' AND consumed_at IS NULL")
            .bind(user_id).execute(&mut *tx).await?;
    }
    tx.commit().await?;
    Ok(user_id)
}

pub async fn resend_verification(state: &AppState, email: &str) -> ApiResult<()> {
    let normalized = email.trim().to_lowercase();
    if !validator::ValidateEmail::validate_email(&normalized) || state.config.email.is_none() {
        return Ok(());
    }
    let mut tx = state.pool.begin().await?;
    let row = sqlx::query(
        "SELECT id FROM users WHERE email=? AND status=1 AND email_verified_at IS NULL FOR UPDATE",
    )
    .bind(&normalized)
    .fetch_optional(&mut *tx)
    .await?;
    if let Some(row) = row {
        let id: Uuid = row.get("id");
        queue_auth_email(state, &mut tx, id, &normalized, "email_verification").await?;
    }
    tx.commit().await?;
    Ok(())
}

pub async fn forgot_password(state: &AppState, email: &str) -> ApiResult<()> {
    let normalized = email.trim().to_lowercase();
    if !validator::ValidateEmail::validate_email(&normalized) || state.config.email.is_none() {
        return Ok(());
    }
    let mut tx = state.pool.begin().await?;
    let row = sqlx::query("SELECT id FROM users WHERE email=? AND status=1 AND email_verified_at IS NOT NULL FOR UPDATE")
        .bind(&normalized).fetch_optional(&mut *tx).await?;
    if let Some(row) = row {
        let id: Uuid = row.get("id");
        queue_auth_email(state, &mut tx, id, &normalized, "password_reset").await?;
    }
    tx.commit().await?;
    Ok(())
}

pub async fn reset_password(state: &AppState, raw: &str, password: String) -> ApiResult<()> {
    if !(12..=128).contains(&password.chars().count()) || password.len() > 512 {
        return Err(AppError::invalid("密码长度须为 12–128 个字符"));
    }
    let mut tx = state.pool.begin().await?;
    let row = sqlx::query("SELECT id,user_id FROM auth_tokens WHERE token_hash=? AND purpose='password_reset' AND consumed_at IS NULL AND expires_at>UTC_TIMESTAMP(6) FOR UPDATE")
        .bind(digest(raw)).fetch_optional(&mut *tx).await?.ok_or_else(AppError::unauthorized)?;
    let token_id: Uuid = row.get("id");
    let user_id: Uuid = row.get("user_id");
    // Lock and validate the one-time token before spending an Argon2 slot. The
    // row lock serializes concurrent reset attempts for the same token.
    let hash = hash_password(state, password).await?;
    sqlx::query(
        "UPDATE auth_tokens SET consumed_at=UTC_TIMESTAMP(6) WHERE id=? AND consumed_at IS NULL",
    )
    .bind(token_id)
    .execute(&mut *tx)
    .await?;
    sqlx::query("UPDATE auth_tokens SET consumed_at=UTC_TIMESTAMP(6) WHERE user_id=? AND purpose='password_reset' AND consumed_at IS NULL")
        .bind(user_id).execute(&mut *tx).await?;
    sqlx::query("UPDATE users SET password_hash=? WHERE id=?")
        .bind(hash)
        .bind(user_id)
        .execute(&mut *tx)
        .await?;
    sqlx::query("UPDATE user_sessions SET revoked_at=UTC_TIMESTAMP(6) WHERE user_id=? AND revoked_at IS NULL")
        .bind(user_id).execute(&mut *tx).await?;
    tx.commit().await?;
    Ok(())
}
pub async fn refresh(state: &AppState, raw: &str, csrf: &str) -> ApiResult<SessionResult> {
    let mut tx = state.pool.begin().await?;
    let row=sqlx::query("SELECT s.id,s.user_id,s.csrf_hash FROM user_sessions s JOIN users u ON u.id=s.user_id WHERE s.token_hash=? AND s.revoked_at IS NULL AND s.expires_at>UTC_TIMESTAMP(6) AND u.status=1 FOR UPDATE").bind(digest(raw)).fetch_optional(&mut *tx).await?.ok_or_else(AppError::unauthorized)?;
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
    sqlx::query("UPDATE user_sessions SET token_hash=? WHERE id=?")
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dummy_login_hash_is_valid_argon2id() {
        let hash = PasswordHash::new(DUMMY_PASSWORD_HASH).expect("valid dummy PHC string");
        Argon2::default()
            .verify_password(b"devstore-invalid-login-dummy", &hash)
            .expect("dummy password verifies against the precomputed hash");
    }
}
