#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    reason = "Test setup and assertions must fail immediately on unexpected errors"
)]

use application::optional::{ProjectCache, SecurityLimiter};
use domain::Project;
use infrastructure::redis::RedisAdapters;
use uuid::Uuid;
#[tokio::test]
async fn cache_is_tenant_scoped_and_security_limiter_fails_closed() {
    let redis = RedisAdapters::connect("redis://127.0.0.1:56379", "integration")
        .await
        .unwrap();
    let a = domain::OrganizationId::from(Uuid::now_v7());
    let b = domain::OrganizationId::from(Uuid::now_v7());
    let p = Project::new(Uuid::now_v7().into(), a, "cached".into()).unwrap();
    redis.put(a, std::slice::from_ref(&p)).await.unwrap();
    assert_eq!(redis.get(a).await.unwrap().unwrap()[0].id, p.id);
    assert!(redis.get(b).await.unwrap().is_none());
    redis.invalidate(a).await.unwrap();
    assert!(redis.get(a).await.unwrap().is_none());
    let key = Uuid::now_v7().to_string();
    for _ in 0..5 {
        redis.admit(&key).await.unwrap();
    }
    assert!(redis.admit(&key).await.is_err());
    let broken = format!("broken-{key}");
    let raw = redis::Client::open("redis://127.0.0.1:56379").unwrap();
    let mut connection = raw.get_multiplexed_async_connection().await.unwrap();
    let redis_key = format!(
        "rust-backend-template:integration:auth:{}",
        hex::encode(infrastructure::auth::token_hash(&broken))
    );
    redis::cmd("SET")
        .arg(&redis_key)
        .arg("not-an-integer")
        .query_async::<()>(&mut connection)
        .await
        .unwrap();
    assert!(redis.admit(&broken).await.is_err());
    let mut registry = prometheus_client::registry::Registry::default();
    redis.register_metrics(&mut registry);
    let mut metrics = String::new();
    prometheus_client::encoding::text::encode(&mut metrics, &registry).unwrap();
    assert!(metrics.contains("redis_errors_total 1\n"));
    assert!(metrics.contains("redis_operation_duration_seconds_sum "));
    assert!(metrics.contains("# TYPE redis_operation_duration_seconds histogram"));
    assert!(metrics.contains("redis_operation_duration_seconds_count 12"));
    redis::cmd("DEL")
        .arg(redis_key)
        .query_async::<()>(&mut connection)
        .await
        .unwrap();
    assert!(
        RedisAdapters::connect("redis://127.0.0.1:1", "integration")
            .await
            .is_err()
    );
}
