#[test]
fn production_rejects_insecure_origin_and_capture_mail() {
    assert!(bootstrap::Config::validate("http://example.com", "smtp", false).is_err());
    assert!(bootstrap::Config::validate("https://example.com", "capture", false).is_err());
    assert!(bootstrap::Config::validate("http://localhost:58039", "capture", true).is_ok());
    for origin in [
        "https://",
        "https://user:secret@example.com",
        "https://example.com/path",
        "https://example.com?token=x",
        "http://localhost:58039@evil.test",
    ] {
        assert!(
            bootstrap::Config::validate(origin, "smtp", true).is_err(),
            "accepted {origin}"
        );
    }
}
