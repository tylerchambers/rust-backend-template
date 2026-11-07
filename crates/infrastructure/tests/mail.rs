use application::LoginMailer;
#[tokio::test]
async fn smtp_adapter_delivers_to_external_capture() {
    let mail = infrastructure::mail::SmtpMailer::local_capture(
        "127.0.0.1",
        51029,
        "http://localhost:58039",
    )
    .unwrap();
    mail.send_login("adapter@example.test", "capture-proof-not-a-real-token")
        .await
        .unwrap();
}
