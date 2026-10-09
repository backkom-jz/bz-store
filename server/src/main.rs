#[tokio::main]
async fn main() {
    dotenvy::dotenv().ok();
    tracing_subscriber::fmt()
        .json()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into()),
        )
        .init();
    let config =
        devstore_server::config::Config::from_env().expect("invalid application configuration");
    let port = config.port;
    let state = devstore_server::state::AppState::connect(config)
        .await
        .expect("dependencies unavailable");
    state.migrate().await.expect("database migration failed");
    let listener = tokio::net::TcpListener::bind(("0.0.0.0", port))
        .await
        .unwrap();
    axum::serve(
        listener,
        devstore_server::build_router(state)
            .into_make_service_with_connect_info::<std::net::SocketAddr>(),
    )
    .with_graceful_shutdown(async {
        tokio::signal::ctrl_c().await.ok();
    })
    .await
    .unwrap();
}
