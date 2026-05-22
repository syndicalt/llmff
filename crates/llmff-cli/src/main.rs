use clap::Parser;

mod commands;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    commands::run(commands::Cli::parse()).await
}
