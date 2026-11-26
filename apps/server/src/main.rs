#[tokio::main]
async fn main() {
    let mut args = std::env::args_os().skip(1);
    let result = match (args.next(), args.next()) {
        (None, None) => bootstrap::server().await,
        (Some(role), None) if role == "worker" => bootstrap::worker().await,
        (Some(role), None) if role == "migrate" => bootstrap::migrate().await,
        _ => Err(bootstrap::Failure::InvalidCommand),
    };
    if let Err(error) = result {
        eprintln!("{{\"level\":\"error\",\"code\":\"{}\"}}", error.category());
        std::process::exit(1);
    }
}
