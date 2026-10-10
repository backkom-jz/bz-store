use crate::{
    common::{ApiResult, AppError},
    config::EmailConfig,
};
use aes_gcm::{Aes256Gcm, KeyInit, Nonce, aead::Aead};
use async_trait::async_trait;
use lettre::{
    AsyncSmtpTransport, AsyncTransport, Message, Tokio1Executor, message::Mailbox,
    transport::smtp::authentication::Credentials,
};
use serde::{Deserialize, Serialize};
use sqlx::{MySql, MySqlPool, Row, Transaction};
use time::OffsetDateTime;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct EmailMessage {
    pub to: String,
    pub subject: String,
    pub body: String,
}

#[derive(Debug, thiserror::Error)]
pub enum EmailError {
    #[error("SMTP delivery failed")]
    Delivery,
    #[error("email payload could not be decoded")]
    Payload,
}

#[async_trait]
pub trait EmailSender: Send + Sync {
    async fn send(&self, message: EmailMessage) -> Result<(), EmailError>;
}

pub struct SmtpSender {
    transport: AsyncSmtpTransport<Tokio1Executor>,
    from: Mailbox,
}

impl SmtpSender {
    pub fn new(config: &EmailConfig) -> ApiResult<Self> {
        let from: Mailbox = config
            .from
            .parse()
            .map_err(|_| AppError::invalid("EMAIL_FROM 无效"))?;
        let credentials =
            Credentials::new(config.smtp_username.clone(), config.smtp_password.clone());
        let mut builder = if config.smtp_tls {
            AsyncSmtpTransport::<Tokio1Executor>::relay(&config.smtp_host)
                .map_err(|_| AppError::invalid("SMTP_HOST 无效"))?
        } else {
            AsyncSmtpTransport::<Tokio1Executor>::builder_dangerous(&config.smtp_host)
        }
        .port(config.smtp_port);
        if !config.smtp_username.is_empty() {
            builder = builder.credentials(credentials);
        }
        let transport = builder.build();
        Ok(Self { transport, from })
    }
}

#[async_trait]
impl EmailSender for SmtpSender {
    async fn send(&self, message: EmailMessage) -> Result<(), EmailError> {
        let to: Mailbox = message.to.parse().map_err(|_| EmailError::Payload)?;
        let mail = Message::builder()
            .from(self.from.clone())
            .to(to)
            .subject(message.subject)
            .body(message.body)
            .map_err(|_| EmailError::Payload)?;
        self.transport
            .send(mail)
            .await
            .map(|_| ())
            .map_err(|_| EmailError::Delivery)
    }
}

fn cipher(key: &[u8; 32]) -> Result<Aes256Gcm, EmailError> {
    Aes256Gcm::new_from_slice(key).map_err(|_| EmailError::Payload)
}

fn seal(key: &[u8; 32], message: &EmailMessage, nonce: [u8; 12]) -> Result<Vec<u8>, EmailError> {
    let plaintext = serde_json::to_vec(message).map_err(|_| EmailError::Payload)?;
    let mut sealed = nonce.to_vec();
    sealed.extend(
        cipher(key)?
            .encrypt(Nonce::from_slice(&nonce), plaintext.as_ref())
            .map_err(|_| EmailError::Payload)?,
    );
    Ok(sealed)
}

fn open(key: &[u8; 32], sealed: &[u8]) -> Result<EmailMessage, EmailError> {
    let (nonce, ciphertext) = sealed.split_at_checked(12).ok_or(EmailError::Payload)?;
    let plaintext = cipher(key)?
        .decrypt(Nonce::from_slice(nonce), ciphertext)
        .map_err(|_| EmailError::Payload)?;
    serde_json::from_slice(&plaintext).map_err(|_| EmailError::Payload)
}

pub async fn enqueue(
    tx: &mut Transaction<'_, MySql>,
    key: &[u8; 32],
    message: &EmailMessage,
    kind: &str,
) -> ApiResult<Uuid> {
    if message.to.len() > 255
        || kind.len() > 32
        || !matches!(kind, "email_verification" | "password_reset")
    {
        return Err(AppError::invalid("邮件数据无效"));
    }
    let id = Uuid::new_v4();
    let mut nonce = [0_u8; 12];
    rand::RngCore::fill_bytes(&mut rand::rngs::OsRng, &mut nonce);
    let sealed = seal(key, message, nonce).map_err(|_| AppError::internal())?;
    sqlx::query("INSERT INTO email_outbox(id,recipient,kind,encrypted_payload) VALUES(?,?,?,?)")
        .bind(id)
        .bind(&message.to)
        .bind(kind)
        .bind(sealed)
        .execute(&mut **tx)
        .await?;
    Ok(id)
}

pub async fn send_one(
    pool: &MySqlPool,
    key: &[u8; 32],
    sender: &dyn EmailSender,
    now: OffsetDateTime,
) -> ApiResult<bool> {
    let mut tx = pool.begin().await?;
    let row = sqlx::query("SELECT id,encrypted_payload,attempts FROM email_outbox WHERE (status='pending' AND available_at<=?) OR (status='sending' AND locked_until<?) ORDER BY created_at,id LIMIT 1 FOR UPDATE SKIP LOCKED")
        .bind(now).bind(now).fetch_optional(&mut *tx).await?;
    let Some(row) = row else {
        tx.commit().await?;
        return Ok(false);
    };
    let id: Uuid = row.try_get("id")?;
    let encrypted: Vec<u8> = row.try_get("encrypted_payload")?;
    let attempts: i16 = row.try_get("attempts")?;
    sqlx::query("UPDATE email_outbox SET status='sending',locked_until=DATE_ADD(UTC_TIMESTAMP(6),INTERVAL 2 MINUTE),attempts=attempts+1 WHERE id=?")
        .bind(id).execute(&mut *tx).await?;
    tx.commit().await?;

    let delivered = match open(key, &encrypted) {
        Ok(message) => sender.send(message).await.is_ok(),
        Err(_) => false,
    };
    if delivered {
        sqlx::query("UPDATE email_outbox SET status='sent',encrypted_payload='',sent_at=UTC_TIMESTAMP(6),locked_until=NULL,last_error=NULL WHERE id=? AND status='sending'")
            .bind(id).execute(pool).await?;
    } else {
        let next_attempt = attempts.saturating_add(1);
        let status = if next_attempt >= 8 {
            "failed"
        } else {
            "pending"
        };
        let delay = 1_i64 << next_attempt.min(8);
        sqlx::query("UPDATE email_outbox SET status=?,available_at=DATE_ADD(UTC_TIMESTAMP(6),INTERVAL ? SECOND),locked_until=NULL,last_error='delivery_failed' WHERE id=? AND status='sending'")
            .bind(status).bind(delay).bind(id).execute(pool).await?;
    }
    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn payload_is_authenticated_and_encrypted() {
        let message = EmailMessage {
            to: "person@example.com".into(),
            subject: "验证邮箱".into(),
            body: "https://example.test/token=secret".into(),
        };
        let key = [7_u8; 32];
        let sealed = seal(&key, &message, [3_u8; 12]).unwrap();
        assert!(!String::from_utf8_lossy(&sealed).contains("secret"));
        assert_eq!(open(&key, &sealed).unwrap(), message);
        assert!(open(&[8_u8; 32], &sealed).is_err());
    }
}
