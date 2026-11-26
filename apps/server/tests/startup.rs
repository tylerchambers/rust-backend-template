#![allow(clippy::unwrap_used, reason = "Subprocess assertions fail immediately")]
use std::process::Command;

#[test]
fn runtime_role_and_listener_failures_are_distinct() {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let admin = std::env::var("TEST_DATABASE_URL").unwrap();
    let runtime = std::env::var("DATABASE_URL").unwrap();
    for (database, category) in [
        (admin.as_str(), "runtime_role"),
        (runtime.as_str(), "listener_bind"),
    ] {
        let output = Command::new(env!("CARGO_BIN_EXE_server"))
            .env_clear()
            .env("DATABASE_URL", database)
            .env("PUBLIC_ORIGIN", "http://localhost:58039")
            .env("ENVIRONMENT", "development")
            .env("MAIL_MODE", "capture")
            .env("BIND_ADDRESS", listener.local_addr().unwrap().to_string())
            .output()
            .unwrap();
        assert!(!output.status.success());
        let stderr = String::from_utf8(output.stderr).unwrap();
        assert!(stderr.contains(category), "{stderr}");
        assert!(!stderr.contains("local-app-test"));
        assert!(!stderr.contains(&admin));
    }
}

#[test]
fn startup_failures_are_classified_without_echoing_configuration() {
    for (database, origin, category) in [
        (None, None, "configuration"),
        (None, Some("http://localhost:58039"), "configuration"),
        (
            Some("postgres://secret-marker:secret-marker@127.0.0.1:1/missing"),
            Some("http://localhost:58039"),
            "database_connect",
        ),
        (
            Some("secret-marker"),
            Some("https://secret-marker:secret-marker@example.test"),
            "configuration",
        ),
    ] {
        let mut command = Command::new(env!("CARGO_BIN_EXE_server"));
        command
            .env_clear()
            .env("MAIL_MODE", "capture")
            .env("ENVIRONMENT", "development");
        if let Some(value) = database {
            command.env("DATABASE_URL", value);
        }
        if let Some(value) = origin {
            command.env("PUBLIC_ORIGIN", value);
        }
        let output = command.output().unwrap();
        assert!(!output.status.success());
        let stderr = String::from_utf8(output.stderr).unwrap();
        assert!(stderr.contains(category), "{stderr}");
        assert!(!stderr.contains("secret-marker"), "{stderr}");
    }
}
