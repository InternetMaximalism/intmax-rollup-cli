use intmax::controller::Config;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut command = Config::new();

    command.invoke_command().await?;

    Ok(())
}
