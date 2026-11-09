#[tokio::main]
async fn main() {
    if let Err(error) = bootstrap::server().await {
        eprintln!("{{\"level\":\"error\",\"code\":\"{}\"}}", error.category());
        std::process::exit(1);
    }
}
