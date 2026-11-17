#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    reason = "Test setup and assertions must fail immediately on unexpected errors"
)]

#[tokio::test]
async fn readiness_tracks_database_availability() {
    let pool = sqlx::PgPool::connect(&std::env::var("TEST_DATABASE_URL").unwrap())
        .await
        .unwrap();
    let db = infrastructure::postgres::Database::new(pool.clone()).unwrap();
    assert!(db.ready().await);
    let mut registry = prometheus_client::registry::Registry::default();
    db.collect_metrics(&mut registry).await.unwrap();
    let mut metrics = String::new();
    prometheus_client::encoding::text::encode(&mut metrics, &registry).unwrap();
    assert!(metrics.contains("# TYPE river_attempt_retries gauge"));
    assert!(metrics.ends_with("# EOF\n"));
    for name in [
        "river_queue_lag_seconds",
        "river_attempt_retries",
        "db_pool_connections",
        "db_pool_idle",
    ] {
        assert!(
            metrics
                .lines()
                .any(|line| line.starts_with(&format!("{name} "))),
            "missing metric {name}"
        );
    }
    pool.close().await;
    assert!(!db.ready().await);
}
