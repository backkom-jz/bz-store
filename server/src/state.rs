use crate::{common::ApiResult, config::Config, infrastructure::storage};
use sqlx::{
    MySqlPool,
    mysql::{MySqlConnectOptions, MySqlPoolOptions, MySqlSslMode},
};
use std::str::FromStr;
use std::sync::Arc;
#[derive(Clone)]
pub struct AppState {
    pub namespace: String,
    pub pool: MySqlPool,
    pub config: Arc<Config>,
    pub redis: redis::aio::ConnectionManager,
    pub storage: Arc<object_store::aws::AmazonS3>,
    pub email_sender: Option<std::sync::Arc<dyn crate::infrastructure::email::EmailSender>>,
    pub password_slots: Arc<tokio::sync::Semaphore>,
    pub upload_slots: Arc<tokio::sync::Semaphore>,
}
impl AppState {
    pub async fn connect(config: Config) -> ApiResult<Self> {
        let mut options = MySqlConnectOptions::from_str(&config.database_url)
            .map_err(|_| crate::common::AppError::invalid("DATABASE_URL 无效"))?;
        if config.secure() {
            options = options.ssl_mode(MySqlSslMode::VerifyIdentity);
        }
        let pool = MySqlPoolOptions::new()
            .max_connections(20)
            .acquire_timeout(std::time::Duration::from_secs(5))
            .after_connect(|connection, _| {
                Box::pin(async move {
                    sqlx::query("SET time_zone = '+00:00'")
                        .execute(connection)
                        .await?;
                    Ok(())
                })
            })
            .connect_with(options)
            .await?;
        Self::with_pool(config, pool).await
    }
    pub async fn with_pool(config: Config, pool: MySqlPool) -> ApiResult<Self> {
        let redis = redis::Client::open(config.redis_url.as_str())?
            .get_connection_manager()
            .await?;
        let storage = Arc::new(storage::build(&config)?);
        let email_sender = config
            .email
            .as_ref()
            .map(crate::infrastructure::email::SmtpSender::new)
            .transpose()?
            .map(|sender| {
                std::sync::Arc::new(sender)
                    as std::sync::Arc<dyn crate::infrastructure::email::EmailSender>
            });
        let namespace: String = sqlx::query_scalar("SELECT DATABASE()")
            .fetch_one(&pool)
            .await?;
        Ok(Self {
            namespace,
            pool,
            config: Arc::new(config),
            redis,
            storage,
            email_sender,
            password_slots: Arc::new(tokio::sync::Semaphore::new(4)),
            upload_slots: Arc::new(tokio::sync::Semaphore::new(4)),
        })
    }
    pub async fn migrate(&self) -> ApiResult<()> {
        sqlx::migrate!("./migrations")
            .run(&self.pool)
            .await
            .map_err(|error| {
                tracing::error!(error = %error, "database migration failed");
                crate::common::AppError::new(
                    axum::http::StatusCode::INTERNAL_SERVER_ERROR,
                    "INTERNAL_ERROR",
                    format!("数据库迁移失败: {error}"),
                )
            })?;
        Ok(())
    }
}
