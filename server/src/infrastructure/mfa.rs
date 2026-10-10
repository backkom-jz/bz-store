use crate::{
    common::{ApiResult, AppError},
    config::Config,
};
use aes_gcm::{Aes256Gcm, KeyInit, Nonce, aead::Aead};
use data_encoding::BASE32_NOPAD;
use hmac::{Hmac, Mac};
use rand::{RngCore, rngs::OsRng};
use sha1::Sha1;
use sqlx::{MySqlPool, Row};
use time::OffsetDateTime;
use uuid::Uuid;

fn cipher(key: &[u8; 32]) -> Result<Aes256Gcm, AppError> {
    Aes256Gcm::new_from_slice(key).map_err(|_| AppError::internal())
}
fn seal(key: &[u8; 32], secret: &[u8]) -> ApiResult<Vec<u8>> {
    let mut nonce = [0_u8; 12];
    OsRng.fill_bytes(&mut nonce);
    let mut output = nonce.to_vec();
    output.extend(
        cipher(key)?
            .encrypt(Nonce::from_slice(&nonce), secret)
            .map_err(|_| AppError::internal())?,
    );
    Ok(output)
}
fn open(key: &[u8; 32], encrypted: &[u8]) -> ApiResult<Vec<u8>> {
    let (nonce, ciphertext) = encrypted
        .split_at_checked(12)
        .ok_or_else(AppError::internal)?;
    cipher(key)?
        .decrypt(Nonce::from_slice(nonce), ciphertext)
        .map_err(|_| AppError::internal())
}
pub fn generate_secret() -> (String, Vec<u8>) {
    let mut bytes = vec![0_u8; 20];
    OsRng.fill_bytes(&mut bytes);
    (BASE32_NOPAD.encode(&bytes), bytes)
}
pub fn totp_code(secret: &[u8], counter: u64) -> String {
    let mut mac = <Hmac<Sha1> as Mac>::new_from_slice(secret).expect("HMAC accepts any key");
    mac.update(&counter.to_be_bytes());
    let bytes = mac.finalize().into_bytes();
    let offset = (bytes[19] & 0x0f) as usize;
    let value = (u32::from(bytes[offset] & 0x7f) << 24)
        | (u32::from(bytes[offset + 1]) << 16)
        | (u32::from(bytes[offset + 2]) << 8)
        | u32::from(bytes[offset + 3]);
    format!("{:06}", value % 1_000_000)
}
fn matching_counter(
    secret: &[u8],
    code: &str,
    now: OffsetDateTime,
    last: Option<i64>,
) -> Option<i64> {
    if code.len() != 6 || !code.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    let current = now.unix_timestamp().div_euclid(30);
    [current - 1, current, current + 1]
        .into_iter()
        .find(|counter| *counter > last.unwrap_or(-1) && totp_code(secret, *counter as u64) == code)
}
fn key(config: &Config) -> ApiResult<&[u8; 32]> {
    config
        .mfa_encryption_key
        .as_ref()
        .ok_or_else(AppError::unavailable)
}
pub async fn begin(
    pool: &MySqlPool,
    config: &Config,
    user: Uuid,
) -> ApiResult<(String, Vec<String>)> {
    let secret = generate_secret();
    let encrypted = seal(key(config)?, &secret.1)?;
    let mut recovery = Vec::new();
    for _ in 0..10 {
        let mut value = [0_u8; 10];
        OsRng.fill_bytes(&mut value);
        recovery.push(hex::encode(value));
    }
    let mut tx = pool.begin().await?;
    let admin:bool=sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM user_roles ur JOIN roles r ON r.id=ur.role_id WHERE ur.user_id=? AND r.code='admin')").bind(user).fetch_one(&mut *tx).await?;
    if !admin {
        return Err(AppError::forbidden());
    }
    sqlx::query("INSERT INTO admin_mfa(user_id,encrypted_secret,status,last_counter,enabled_at) VALUES(?,?,'pending',NULL,NULL) ON DUPLICATE KEY UPDATE encrypted_secret=VALUES(encrypted_secret),status='pending',last_counter=NULL,enabled_at=NULL")
        .bind(user).bind(encrypted).execute(&mut *tx).await?;
    sqlx::query("DELETE FROM admin_recovery_codes WHERE user_id=?")
        .bind(user)
        .execute(&mut *tx)
        .await?;
    for code in &recovery {
        sqlx::query("INSERT INTO admin_recovery_codes(id,user_id,code_hash) VALUES(?,?,?)")
            .bind(Uuid::new_v4())
            .bind(user)
            .bind(crate::modules::auth::service::digest(code))
            .execute(&mut *tx)
            .await?;
    }
    sqlx::query("UPDATE user_sessions SET revoked_at=UTC_TIMESTAMP(6) WHERE user_id=? AND revoked_at IS NULL").bind(user).execute(&mut *tx).await?;
    sqlx::query("INSERT INTO admin_audit_logs(id,actor_id,action,target_id,detail) VALUES(?,?,'admin.mfa.begin',?,JSON_OBJECT('source','local_cli'))").bind(Uuid::new_v4()).bind(user).bind(user).execute(&mut *tx).await?;
    tx.commit().await?;
    Ok((secret.0, recovery))
}
pub async fn confirm(
    pool: &MySqlPool,
    config: &Config,
    user: Uuid,
    code: &str,
    now: OffsetDateTime,
) -> ApiResult<()> {
    let mut tx = pool.begin().await?;
    let row =
        sqlx::query("SELECT encrypted_secret,status FROM admin_mfa WHERE user_id=? FOR UPDATE")
            .bind(user)
            .fetch_optional(&mut *tx)
            .await?
            .ok_or_else(AppError::missing)?;
    if row.get::<String, _>("status") != "pending" {
        return Err(AppError::conflict("MFA 已启用或无需确认"));
    }
    let secret = open(key(config)?, &row.get::<Vec<u8>, _>("encrypted_secret"))?;
    let counter = matching_counter(&secret, code, now, None).ok_or_else(AppError::unauthorized)?;
    sqlx::query("UPDATE admin_mfa SET status='enabled',last_counter=?,enabled_at=UTC_TIMESTAMP(6) WHERE user_id=?").bind(counter).bind(user).execute(&mut *tx).await?;
    sqlx::query("INSERT INTO admin_audit_logs(id,actor_id,action,target_id,detail) VALUES(?,?,'admin.mfa.enabled',?,JSON_OBJECT('source','local_cli'))").bind(Uuid::new_v4()).bind(user).bind(user).execute(&mut *tx).await?;
    tx.commit().await?;
    Ok(())
}
pub async fn verify(
    pool: &MySqlPool,
    config: &Config,
    user: Uuid,
    code: &str,
    now: OffsetDateTime,
) -> ApiResult<&'static str> {
    let mut tx = pool.begin().await?;
    let row = sqlx::query(
        "SELECT encrypted_secret,status,last_counter FROM admin_mfa WHERE user_id=? FOR UPDATE",
    )
    .bind(user)
    .fetch_optional(&mut *tx)
    .await?
    .ok_or_else(|| {
        AppError::new(
            axum::http::StatusCode::FORBIDDEN,
            "MFA_ENROLLMENT_REQUIRED",
            "管理员需要先通过命令行启用双重验证",
        )
    })?;
    if row.get::<String, _>("status") != "enabled" {
        return Err(AppError::new(
            axum::http::StatusCode::FORBIDDEN,
            "MFA_ENROLLMENT_REQUIRED",
            "管理员需要先通过命令行启用双重验证",
        ));
    }
    let secret = open(key(config)?, &row.get::<Vec<u8>, _>("encrypted_secret"))?;
    let last: Option<i64> = row.get("last_counter");
    if let Some(counter) = matching_counter(&secret, code, now, last) {
        sqlx::query("UPDATE admin_mfa SET last_counter=? WHERE user_id=?")
            .bind(counter)
            .bind(user)
            .execute(&mut *tx)
            .await?;
        tx.commit().await?;
        return Ok("totp");
    }
    let recovery=sqlx::query("SELECT id FROM admin_recovery_codes WHERE user_id=? AND code_hash=? AND consumed_at IS NULL FOR UPDATE").bind(user).bind(crate::modules::auth::service::digest(code)).fetch_optional(&mut *tx).await?;
    if let Some(recovery) = recovery {
        sqlx::query("UPDATE admin_recovery_codes SET consumed_at=UTC_TIMESTAMP(6) WHERE id=? AND consumed_at IS NULL").bind(recovery.get::<Uuid,_>("id")).execute(&mut *tx).await?;
        tx.commit().await?;
        return Ok("recovery");
    }
    Err(AppError::unauthorized())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn totp_uses_the_expected_time_counter_and_six_digits() {
        assert_eq!(totp_code(b"12345678901234567890", 1), "287082");
        let now = OffsetDateTime::from_unix_timestamp(59).unwrap();
        assert_eq!(
            matching_counter(b"12345678901234567890", "287082", now, None),
            Some(1)
        );
        assert_eq!(
            matching_counter(b"12345678901234567890", "287082", now, Some(1)),
            None
        );
    }
}
