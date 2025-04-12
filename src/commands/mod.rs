mod auth;
mod rules;
mod clean;

pub use auth::AuthCommand;
pub use rules::RulesCommand;
pub use clean::CleanCommand;

use clap::Subcommand;

#[derive(Subcommand, Debug)]
pub enum Commands {
    /// Manage authentication with Microsoft Graph API
    Auth(AuthCommand),
    
    /// Manage rules for email processing
    Rules(RulesCommand),
    
    /// Clean inbox based on configured rules
    Clean(CleanCommand),
}