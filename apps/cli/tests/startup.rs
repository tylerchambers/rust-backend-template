#![allow(clippy::unwrap_used, reason = "Subprocess assertions fail immediately")]

#[test]
fn invalid_command_reports_fixed_usage_without_echoing_arguments() {
    let output = std::process::Command::new(env!("CARGO_BIN_EXE_cli"))
        .arg("secret-marker")
        .env_clear()
        .output()
        .unwrap();
    assert!(!output.status.success());
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(stderr.contains("invalid_command"));
    assert!(stderr.contains("usage: cli migrate|openapi"));
    assert!(!stderr.contains("secret-marker"));
}
