#[tokio::main]
async fn main() {
    if bootstrap::worker().await.is_err() {
        eprintln!("{{\"level\":\"error\",\"code\":\"startup_or_runtime_failure\"}}");
        std::process::exit(1);
    }
}
