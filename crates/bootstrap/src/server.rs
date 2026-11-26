use crate::{
    Failure,
    config::ServerConfig,
    services::{ServerServices, enforce_runtime_role},
    telemetry::{logging, monitored_router, shutdown},
};
use tower_http::services::{ServeDir, ServeFile};

/// Compose and serve the HTTP application until shutdown.
/// # Errors
/// Returns a closed configuration, dependency, bind or serving category.
pub async fn server() -> Result<(), Failure> {
    logging();
    let config = ServerConfig::load().map_err(|_| Failure::Configuration)?;
    let pool = config
        .runtime
        .pool()
        .await
        .map_err(|_| Failure::DatabaseConnect)?;
    enforce_runtime_role(&pool)
        .await
        .map_err(|_| Failure::RuntimeRole)?;
    let services = ServerServices::build(&config.runtime, &pool).await?;
    let listener = tokio::net::TcpListener::bind(config.bind)
        .await
        .map_err(|_| Failure::ListenerBind)?;
    tracing::info!(operation = "server.ready");
    let workspace = ServeFile::new("apps/frontend/dist/index.html");
    let router = monitored_router(services.state, services.db, services.redis)
        .route_service("/", workspace.clone())
        .route_service("/sign-in", workspace)
        .nest_service("/assets", ServeDir::new("apps/frontend/dist"));
    axum::serve(
        listener,
        router.into_make_service_with_connect_info::<std::net::SocketAddr>(),
    )
    .with_graceful_shutdown(shutdown())
    .await
    .map_err(|_| Failure::Serve)?;
    pool.close().await;
    Ok(())
}
