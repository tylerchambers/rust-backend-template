#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    reason = "Test setup and assertions must fail immediately on unexpected errors"
)]

use application::authentication::LoginMailer;
#[tokio::test]
async fn smtp_adapter_delivers_to_external_capture() {
    let mail = infrastructure::smtp::SmtpMailer::local_capture(
        "127.0.0.1",
        51029,
        "http://localhost:58039",
    )
    .unwrap();
    mail.send_login("adapter@example.test", "capture-proof-not-a-real-token")
        .await
        .unwrap();
}

#[test]
fn invalid_sender_is_rejected_when_configuring_production_mail() {
    let mail = infrastructure::smtp::SmtpMailer::production(
        "smtp.example.test",
        "user".into(),
        "password".into(),
        "not a mailbox",
        "https://example.test".into(),
    );
    assert!(matches!(mail, Err(domain::Error::Invalid)));
}
