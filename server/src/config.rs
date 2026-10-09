use crate::common::{ApiResult, AppError};
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
    pub clamav_addr: String,
    pub trusted_proxies: Vec<std::net::IpAddr>,
}
impl Config {
    pub fn from_env() -> ApiResult<Self> {
        fn required(key: &str) -> ApiResult<String> {
            std::env::var(key)
                .ok()
                .filter(|v| !v.trim().is_empty())
                .ok_or_else(|| AppError::invalid(format!("缺少配置 {key}")))
        }
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
            clamav_addr: required("CLAMAV_ADDR")?,
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
        let site =
            reqwest::Url::parse(&self.site_url).map_err(|_| AppError::invalid("SITE_URL 无效"))?;
        if !matches!(site.scheme(), "http" | "https")
            || site.origin().ascii_serialization() != self.site_url
            || site.host_str().is_none()
        {
            return Err(AppError::invalid("SITE_URL 必须是无路径的完整来源地址"));
        }
        if self.secure() && (site.scheme() != "https" || !self.s3_endpoint.starts_with("https://"))
        {
            return Err(AppError::invalid("生产环境必须使用 HTTPS 站点和对象存储"));
        }
        Ok(())
    }
}
