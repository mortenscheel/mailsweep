mod auth;
mod rules;
mod commands;
mod debug_auth;

use clap::Parser;
use commands::Commands;

#[derive(Parser, Debug)]
#[command(
    name = "mailsweep",
    about = "Clean up your Outlook inbox using rules",
    author, 
    version
)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Initialize logging
    tracing_subscriber::fmt::init();
    
    // Parse command line arguments
    let cli = Cli::parse();
    
    // Execute the specified command
    match cli.command {
        Commands::Auth(cmd) => cmd.execute().await,
        Commands::Rules(cmd) => cmd.execute().await,
        Commands::Clean(cmd) => cmd.execute().await,
    }
}
