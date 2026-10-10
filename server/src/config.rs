use crate::common::{ApiResult, AppError};
use base64::{Engine, engine::general_purpose::STANDARD};
#[derive(Clone)]
pub struct Config {
    pub env: String,
    pub port: u16,
    pub database_url: String,
    pub redis_url: String,
    pub site_url: String,
    pub jwt_secret: String,
    pub csrf_secret: String,
    pub s3_endpoint: String,
    pub s3_access_key: String,
    pub s3_secret_key: String,
    pub s3_bucket: String,
    pub trusted_proxies: Vec<std::net::IpAddr>,
    pub email: Option<EmailConfig>,
    pub mfa_encryption_key: Option<[u8; 32]>,
}
#[derive(Clone)]
pub struct EmailConfig {
    pub smtp_host: String,
    pub smtp_port: u16,
    pub smtp_username: String,
    pub smtp_password: String,
    pub smtp_tls: bool,
    pub from: String,
    pub outbox_key: [u8; 32],
}
impl Config {
    pub fn from_env() -> ApiResult<Self> {
        fn required(key: &str) -> ApiResult<String> {
            std::env::var(key)
                .ok()
                .filter(|v| !v.trim().is_empty())
                .ok_or_else(|| AppError::invalid(format!("缺少配置 {key}")))
        }
        let mail_values = ["SMTP_HOST", "EMAIL_FROM", "EMAIL_OUTBOX_ENCRYPTION_KEY"].map(|name| {
            std::env::var(name)
                .ok()
                .filter(|value| !value.trim().is_empty())
        });
        let smtp_username = std::env::var("SMTP_USERNAME").unwrap_or_default();
        let smtp_password = std::env::var("SMTP_PASSWORD").unwrap_or_default();
        let email = if mail_values.iter().all(Option::is_none)
            && smtp_username.is_empty()
            && smtp_password.is_empty()
        {
            None
        } else if mail_values.iter().any(Option::is_none)
            || smtp_username.is_empty() != smtp_password.is_empty()
        {
            return Err(AppError::invalid("SMTP 配置不完整，邮件功能已关闭"));
        } else {
            let key = STANDARD
                .decode(mail_values[2].as_deref().unwrap_or_default())
                .map_err(|_| {
                    AppError::invalid("EMAIL_OUTBOX_ENCRYPTION_KEY 必须是 32 字节 Base64 密钥")
                })?;
            let outbox_key: [u8; 32] = key.try_into().map_err(|_| {
                AppError::invalid("EMAIL_OUTBOX_ENCRYPTION_KEY 必须是 32 字节 Base64 密钥")
            })?;
            Some(EmailConfig {
                smtp_host: mail_values[0].as_deref().unwrap().to_owned(),
                smtp_port: std::env::var("SMTP_PORT")
                    .unwrap_or_else(|_| "587".into())
                    .parse()
                    .map_err(|_| AppError::invalid("SMTP_PORT 无效"))?,
                smtp_username,
                smtp_password,
                smtp_tls: std::env::var("APP_ENV").unwrap_or_else(|_| "development".into())
                    == "production"
                    || std::env::var("SMTP_TLS").is_ok_and(|value| value == "true"),
                from: mail_values[1].as_deref().unwrap().to_owned(),
                outbox_key,
            })
        };
        let mfa_encryption_key = std::env::var("MFA_ENCRYPTION_KEY")
            .ok()
            .filter(|value| !value.trim().is_empty())
            .map(|value| {
                let bytes = STANDARD.decode(value).map_err(|_| {
                    AppError::invalid("MFA_ENCRYPTION_KEY 必须是 32 字节 Base64 密钥")
                })?;
                bytes
                    .try_into()
                    .map_err(|_| AppError::invalid("MFA_ENCRYPTION_KEY 必须是 32 字节 Base64 密钥"))
            })
            .transpose()?;
        let config = Self {
            env: std::env::var("APP_ENV").unwrap_or_else(|_| "development".into()),
            port: std::env::var("APP_PORT")
                .unwrap_or_else(|_| "8080".into())
                .parse()
                .map_err(|_| AppError::invalid("APP_PORT 无效"))?,
            database_url: required("DATABASE_URL")?,
            redis_url: required("REDIS_URL")?,
            site_url: required("SITE_URL")?,
            jwt_secret: required("JWT_SECRET")?,
            csrf_secret: required("CSRF_SECRET")?,
            s3_endpoint: required("S3_ENDPOINT")?,
            s3_access_key: required("S3_ACCESS_KEY")?,
            s3_secret_key: required("S3_SECRET_KEY")?,
            s3_bucket: required("S3_BUCKET")?,
            trusted_proxies: std::env::var("TRUSTED_PROXIES")
                .unwrap_or_default()
                .split(',')
                .filter(|s| !s.trim().is_empty())
                .map(|s| {
                    s.trim()
                        .parse()
                        .map_err(|_| AppError::invalid("TRUSTED_PROXIES 无效"))
                })
                .collect::<ApiResult<_>>()?,
            email,
            mfa_encryption_key,
        };
        config.validate()?;
        Ok(config)
    }
    pub fn secure(&self) -> bool {
        self.env == "production"
    }
    pub fn validate(&self) -> ApiResult<()> {
        if !matches!(self.env.as_str(), "production" | "development" | "test") {
            return Err(AppError::invalid("APP_ENV 无效"));
        }
        if self.jwt_secret.len() < 32
            || self.csrf_secret.len() < 32
            || self.jwt_secret == self.csrf_secret
        {
            return Err(AppError::invalid(
                "JWT_SECRET 与 CSRF_SECRET 必须不同且至少 32 字节",
            ));
        }
        let database = reqwest::Url::parse(&self.database_url)
            .map_err(|_| AppError::invalid("DATABASE_URL 无效"))?;
        let expected_database = if self.env == "test" {
            "bz_ztore_test"
        } else {
            "bz_ztore"
        };
        if database.scheme() != "mysql"
            || database.host_str().is_none()
            || database.path() != format!("/{expected_database}")
        {
            return Err(AppError::invalid(
                "DATABASE_URL 必须指向 DevStore 对应的 MySQL 数据库",
            ));
        }
        let site =
            reqwest::Url::parse(&self.site_url).map_err(|_| AppError::invalid("SITE_URL 无效"))?;
        if !matches!(site.scheme(), "http" | "https")
            || site.origin().ascii_serialization() != self.site_url
            || site.host_str().is_none()
        {
            return Err(AppError::invalid("SITE_URL 必须是无路径的完整来源地址"));
        }
        if self.secure()
            && (site.origin().ascii_serialization() != "https://store.bengzi.cn"
                || !self.s3_endpoint.starts_with("https://"))
        {
            return Err(AppError::invalid("生产环境必须使用 HTTPS 站点和对象存储"));
        }
        if let Some(email) = &self.email
            && !email.smtp_tls
            && (self.secure()
                || !matches!(
                    email.smtp_host.as_str(),
                    "localhost" | "127.0.0.1" | "mailpit"
                ))
        {
            return Err(AppError::invalid("仅本地 Mailpit 可关闭 SMTP TLS"));
        }
        if self.secure() && self.mfa_encryption_key.is_none() {
            return Err(AppError::invalid("生产环境必须配置 MFA_ENCRYPTION_KEY"));
        }
        Ok(())
    }
}
