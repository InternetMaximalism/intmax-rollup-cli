use dotenv::dotenv;
use intmax::controller::invoke_command;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let _ = dotenv().ok();

    invoke_command().await?;

    Ok(())
}
