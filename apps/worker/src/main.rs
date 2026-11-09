#[tokio::main]
async fn main() {
    if let Err(error) = bootstrap::worker().await {
        eprintln!("{{\"level\":\"error\",\"code\":\"{}\"}}", error.category());
        std::process::exit(1);
    }
}
