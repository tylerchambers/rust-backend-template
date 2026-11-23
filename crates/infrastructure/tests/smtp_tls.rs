#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    reason = "Test setup and assertions must fail immediately on unexpected errors"
)]

use application::authentication::LoginMailer;
use infrastructure::smtp::SmtpMailer;
#[tokio::test]
async fn production_smtp_requires_verified_starttls_and_delivers_to_capture() {
    let ca = std::fs::read(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../.local/smtp/cert.pem"
    ))
    .unwrap();
    let mail = SmtpMailer::production_endpoint(
        "localhost",
        51029,
        "capture".into(),
        "capture".into(),
        "login@example.test",
        "https://example.test".into(),
        Some(&ca),
    )
    .unwrap();
    mail.send_login(
        "tls@example.test",
        "tls-test-only-not-an-actual-login-token",
    )
    .await
    .unwrap();
    let untrusted = SmtpMailer::production_endpoint(
        "localhost",
        51029,
        "capture".into(),
        "capture".into(),
        "login@example.test",
        "https://example.test".into(),
        None,
    )
    .unwrap();
    assert!(
        untrusted
            .send_login("tls@example.test", "not-a-token")
            .await
            .is_err()
    );
}
