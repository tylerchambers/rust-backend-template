#[tokio::main]
async fn main() {
    if bootstrap::server().await.is_err() {
        eprintln!("{{\"level\":\"error\",\"code\":\"startup_or_runtime_failure\"}}");
        std::process::exit(1);
    }
}
