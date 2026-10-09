use crate::{
    common::ApiResult,
    config::Config,
    infrastructure::{scanner::Scanner, storage},
};
use sqlx::{PgPool, postgres::PgPoolOptions};
use std::sync::Arc;
#[derive(Clone)]
pub struct AppState {
    pub pool: PgPool,
    pub config: Arc<Config>,
    pub redis: redis::aio::ConnectionManager,
    pub storage: Arc<object_store::aws::AmazonS3>,
    pub scanner: Scanner,
    pub password_slots: Arc<tokio::sync::Semaphore>,
}
impl AppState {
    pub async fn connect(config: Config) -> ApiResult<Self> {
        let pool = PgPoolOptions::new()
            .max_connections(20)
            .acquire_timeout(std::time::Duration::from_secs(5))
            .connect(&config.database_url)
            .await?;
        Self::with_pool(config, pool).await
    }
    pub async fn with_pool(config: Config, pool: PgPool) -> ApiResult<Self> {
        let redis = redis::Client::open(config.redis_url.as_str())?
            .get_connection_manager()
            .await?;
        let storage = Arc::new(storage::build(&config)?);
        let scanner = Scanner {
            addr: config.clamav_addr.clone(),
        };
        Ok(Self {
            pool,
            config: Arc::new(config),
            redis,
            storage,
            scanner,
            password_slots: Arc::new(tokio::sync::Semaphore::new(4)),
        })
    }
    pub async fn migrate(&self) -> ApiResult<()> {
        sqlx::migrate!("./migrations")
            .run(&self.pool)
            .await
            .map_err(|_| crate::common::AppError::internal())?;
        Ok(())
    }
}
