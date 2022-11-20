use intmax_client::controller::invoke_command;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    invoke_command()?;

    Ok(())
}
