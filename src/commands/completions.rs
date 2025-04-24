use anyhow::Result;
use clap::{Args, CommandFactory, ValueEnum};
use clap_complete::{generate, shells};
use std::io;

use crate::Cli;

#[derive(Args, Debug)]
pub struct CompletionsCommand {
    /// Shell to generate completions for
    #[arg(value_enum)]
    shell: Shell,

    /// Output to file instead of stdout
    #[arg(short, long)]
    output: Option<String>,
}

#[derive(Debug, Copy, Clone, ValueEnum)]
pub enum Shell {
    Bash,
    Zsh,
    Fish,
    Posh, // PowerShell
    Elvish,
}

impl CompletionsCommand {
    pub fn execute(&self) -> Result<()> {
        let mut cmd = Cli::command();

        if let Some(path) = &self.output {
            let mut file = std::fs::File::create(path)?;
            match self.shell {
                Shell::Bash => generate(shells::Bash, &mut cmd, "mailsweep", &mut file),
                Shell::Zsh => generate(shells::Zsh, &mut cmd, "mailsweep", &mut file),
                Shell::Fish => generate(shells::Fish, &mut cmd, "mailsweep", &mut file),
                Shell::Posh => generate(shells::PowerShell, &mut cmd, "mailsweep", &mut file),
                Shell::Elvish => generate(shells::Elvish, &mut cmd, "mailsweep", &mut file),
            }
            println!("Completions written to {}", path);
        } else {
            match self.shell {
                Shell::Bash => generate(shells::Bash, &mut cmd, "mailsweep", &mut io::stdout()),
                Shell::Zsh => generate(shells::Zsh, &mut cmd, "mailsweep", &mut io::stdout()),
                Shell::Fish => generate(shells::Fish, &mut cmd, "mailsweep", &mut io::stdout()),
                Shell::Posh => {
                    generate(shells::PowerShell, &mut cmd, "mailsweep", &mut io::stdout())
                }
                Shell::Elvish => generate(shells::Elvish, &mut cmd, "mailsweep", &mut io::stdout()),
            }
        }

        Ok(())
    }
}
