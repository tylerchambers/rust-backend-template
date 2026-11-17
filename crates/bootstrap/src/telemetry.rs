use infrastructure::postgres::Database;
use std::sync::Arc;
pub(crate) fn logging() {
    let _ = tracing_subscriber::fmt()
        .json()
        .with_writer(std::io::stderr)
        .with_env_filter("bootstrap=info,api=info,infrastructure=info")
        .try_init();
}
#[expect(
    clippy::expect_used,
    reason = "Failure to install the OS shutdown handler is a fatal runtime defect"
)]
pub(crate) async fn shutdown() {
    #[cfg(unix)]
    {
        let mut term = tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
            .expect("signal handler");
        tokio::select! {_ = tokio::signal::ctrl_c()=>{},_ = term.recv()=>{}}
    }
    #[cfg(not(unix))]
    {
        let _ = tokio::signal::ctrl_c().await;
    }
}
pub(crate) fn monitored_router(
    state: api::State,
    db: Arc<Database>,
    redis_metrics: Option<Arc<infrastructure::redis::RedisAdapters>>,
) -> axum::Router {
    let readiness = db.clone();
    let metrics = db;
    let http_metrics = state.clone();
    api::router(state)
        .route(
            "/ready",
            axum::routing::get(move || {
                let db = readiness.clone();
                async move {
                    if db.ready().await {
                        axum::http::StatusCode::OK
                    } else {
                        axum::http::StatusCode::SERVICE_UNAVAILABLE
                    }
                }
            }),
        )
        .route(
            "/metrics",
            axum::routing::get(move || {
                let db = metrics.clone();
                let redis = redis_metrics.clone();
                let http = http_metrics.clone();
                async move {
                    let mut registry = prometheus_client::registry::Registry::default();
                    db.collect_metrics(&mut registry)
                        .await
                        .map_err(|_| axum::http::StatusCode::SERVICE_UNAVAILABLE)?;
                    http.register_metrics(&mut registry);
                    if let Some(redis) = redis {
                        redis.register_metrics(&mut registry);
                    }
                    let mut output = String::new();
                    prometheus_client::encoding::text::encode(&mut output, &registry)
                        .map_err(|_| axum::http::StatusCode::SERVICE_UNAVAILABLE)?;
                    Ok::<_, axum::http::StatusCode>((
                        [(
                            axum::http::header::CONTENT_TYPE,
                            "application/openmetrics-text; version=1.0.0; charset=utf-8",
                        )],
                        output,
                    ))
                }
            }),
        )
}
