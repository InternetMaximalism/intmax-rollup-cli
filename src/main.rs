use dotenv::dotenv;
use intmax::controller::Command;
use structopt::StructOpt;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let _ = dotenv().ok();

    Command::from_args().invoke().await?;

    Ok(())
}
