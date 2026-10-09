pub mod common;
pub mod config;
pub mod infrastructure;
pub mod middleware;
pub mod state;
pub fn base_router() -> axum::Router {
    axum::Router::new()
        .route(
            "/api/v1/health/live",
            axum::routing::get(async || common::ok(serde_json::json!({"status":"alive"}))),
        )
        .fallback(async || common::AppError::missing())
        .layer(axum::middleware::from_fn(middleware::request_id::wrap))
}
pub fn build_router(state: state::AppState) -> axum::Router {
    base_router()
        .merge(
            axum::Router::new()
                .route("/api/v1/health/ready", axum::routing::get(ready))
                .with_state(state),
        )
        .layer(axum::middleware::from_fn(middleware::request_id::wrap))
}
async fn ready(
    axum::extract::State(state): axum::extract::State<state::AppState>,
) -> common::ApiResult<impl axum::response::IntoResponse> {
    use futures::TryStreamExt;
    use object_store::ObjectStore;
    tokio::time::timeout(std::time::Duration::from_secs(5), async {
        sqlx::query("SELECT 1")
            .execute(&state.pool)
            .await
            .map_err(|_| common::AppError::unavailable())?;
        let _: String = redis::cmd("PING")
            .query_async(&mut state.redis.clone())
            .await?;
        state.storage.list(None).try_next().await?;
        state.scanner.ping().await?;
        Ok::<_, common::AppError>(())
    })
    .await
    .map_err(|_| common::AppError::unavailable())??;
    Ok(common::ok(serde_json::json!({"status":"ready"})))
}
