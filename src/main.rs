use intmax::controller::Command;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut command = Command::new();

    command.invoke_command().await?;

    Ok(())
}
