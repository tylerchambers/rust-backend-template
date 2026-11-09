#[tokio::main]
async fn main() {
    if let Err(error) = bootstrap::cli().await {
        eprintln!("{{\"level\":\"error\",\"code\":\"{}\"}}", error.category());
        if matches!(error, bootstrap::Failure::InvalidCommand) {
            eprintln!("usage: cli migrate|openapi");
        }
        std::process::exit(1);
    }
}
