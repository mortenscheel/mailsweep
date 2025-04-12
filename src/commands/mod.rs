mod auth;
mod clean;
mod rules;

pub use auth::AuthCommand;
pub use clean::CleanCommand;
pub use rules::RulesCommand;

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
