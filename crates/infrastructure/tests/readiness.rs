#[tokio::test]
async fn readiness_tracks_database_availability() {
    let pool = sqlx::PgPool::connect(&std::env::var("TEST_DATABASE_URL").unwrap())
        .await
        .unwrap();
    let db = infrastructure::Database::new(pool.clone()).unwrap();
    assert!(db.ready().await);
    let metrics = db.operational_metrics().await.unwrap();
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
