mod auth;
mod clean;
mod completions;
mod rules;

pub use auth::AuthCommand;
pub use clean::CleanCommand;
pub use completions::CompletionsCommand;
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

    /// Generate shell completions
    Completions(CompletionsCommand),
}
