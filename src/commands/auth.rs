use crate::auth::Auth;
use anyhow::Result;
use clap::{Args, Subcommand};

#[derive(Debug, Args)]
pub struct AuthCommand {
    #[command(subcommand)]
    command: AuthCommands,
}

#[derive(Debug, Subcommand)]
enum AuthCommands {
    /// Login to Microsoft Graph API
    Login,

    /// Logout and remove saved credentials
    Logout,

    /// Check authentication status
    Status,

    /// Run diagnostic tests for authentication
    Debug,
}

impl AuthCommand {
    pub async fn execute(self) -> Result<()> {
        let auth = Auth::new()?;

        match self.command {
            AuthCommands::Login => auth.login().await,
            AuthCommands::Logout => auth.logout(),
            AuthCommands::Status => auth.check().await,
            AuthCommands::Debug => crate::debug_auth::debug_auth()
                .await
                .map_err(|e| anyhow::anyhow!("{}", e)),
        }
    }
}
