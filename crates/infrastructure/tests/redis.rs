use application::optional::{ProjectCache, SecurityLimiter};
use domain::Project;
use infrastructure::redis::RedisAdapters;
use uuid::Uuid;
#[tokio::test]
async fn cache_is_tenant_scoped_and_security_limiter_fails_closed() {
    let redis = RedisAdapters::connect("redis://127.0.0.1:56379", "integration")
        .await
        .unwrap();
    let a = Uuid::now_v7();
    let b = Uuid::now_v7();
    let p = Project::new(Uuid::now_v7(), a, "cached".into()).unwrap();
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
    assert!(
        RedisAdapters::connect("redis://127.0.0.1:1", "integration")
            .await
            .is_err()
    );
}
