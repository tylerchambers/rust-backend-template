#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    reason = "Test setup and assertions must fail immediately on unexpected errors"
)]

use application::authentication::IdentityRepository;
use std::{
    io::Write,
    sync::{Arc, Mutex},
};
#[derive(Clone)]
struct Capture(Arc<Mutex<Vec<u8>>>);
impl Write for Capture {
    fn write(&mut self, b: &[u8]) -> std::io::Result<usize> {
        self.0.lock().unwrap().extend_from_slice(b);
        Ok(b.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}
#[tokio::test(flavor = "current_thread")]
async fn storage_failure_is_categorized_without_token_or_cause_leakage() {
    let output = Arc::new(Mutex::new(Vec::new()));
    let copy = output.clone();
    let subscriber = tracing_subscriber::fmt()
        .json()
        .with_writer(move || Capture(copy.clone()))
        .finish();
    let _guard = tracing::subscriber::set_default(subscriber);
    let pool = sqlx::PgPool::connect_lazy("postgres://unused@127.0.0.1/unused").unwrap();
    let db = infrastructure::postgres::Database::new(pool.clone()).unwrap();
    pool.close().await;
    let secret = "sensitive-test-token".repeat(4);
    assert!(matches!(
        db.revoke(&secret).await,
        Err(domain::Error::Unavailable)
    ));
    let logs = String::from_utf8(output.lock().unwrap().clone()).unwrap();
    assert!(
        logs.contains("pool_closed"),
        "missing safe storage category: {logs}"
    );
    assert!(!logs.contains(&secret));
    assert!(!logs.contains("postgres://"));
}
