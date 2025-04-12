use crate::rules::Rules;
use anyhow::Result;
use clap::{Args, Subcommand};
use inquire::Confirm;
use serde_yaml;
use std::process::Command;

#[derive(Debug, Args)]
pub struct RulesCommand {
    #[command(subcommand)]
    command: RulesCommands,
}

#[derive(Debug, Subcommand)]
enum RulesCommands {
    /// Show current rules
    Show,
    
    /// Edit rules in default editor
    Edit,
    
    /// Get path to rules file
    Path,
    
    /// Check rules for errors
    Check,
    
    /// Reset rules to default
    Reset {
        /// Force reset without confirmation prompt
        #[arg(short, long)]
        force: bool,
    },
    
    /// Add a new rule
    Add {
        /// Name of the rule
        #[arg(short, long)]
        name: String,
        
        /// Action to take (archive, delete, mark_read)
        #[arg(short, long)]
        action: String,
        
        /// Sender patterns to match (can be specified multiple times)
        #[arg(long)]
        sender: Vec<String>,
        
        /// Subject patterns to match (can be specified multiple times)
        #[arg(long)]
        subject: Vec<String>,
    },
}

impl RulesCommand {
    pub async fn execute(self) -> Result<()> {
        match self.command {
            RulesCommands::Show => {
                let rules = Rules::load()?;
                println!("{}", serde_yaml::to_string(&rules.items)?);
                Ok(())
            },
            RulesCommands::Edit => {
                // Make sure rules exist
                let is_new = !Rules::get_rules_path()?.exists();
                Rules::load()?;
                
                // Get path to rules file
                let rules_path = Rules::get_rules_path_str()?;
                
                // Determine editor
                let editor = std::env::var("EDITOR").unwrap_or_else(|_| "vi".to_string());
                
                if is_new {
                    println!("Creating a new rules file at: {}", rules_path);
                    println!("Editor: {} (set $EDITOR environment variable to change)", editor);
                    println!("Press any key to continue...");
                    let _ = inquire::Text::new("").prompt();
                } else {
                    println!("Editing rules at: {}", rules_path);
                    println!("Editor: {} (set $EDITOR environment variable to change)", editor);
                }
                
                // Open editor
                let status = Command::new(editor)
                    .arg(&rules_path)
                    .status()?;
                
                if !status.success() {
                    anyhow::bail!("Editor exited with non-zero status");
                }
                
                println!("Rules saved at {}", rules_path);
                println!("Use 'mailsweep rules show' to view your rules");
                Ok(())
            },
            RulesCommands::Path => {
                let path = Rules::get_rules_path_str()?;
                println!("{}", path);
                Ok(())
            },
            RulesCommands::Check => {
                // Try to load the rules
                let rules_path = Rules::get_rules_path()?;
                
                if !rules_path.exists() {
                    println!("❌ Rules file not found at: {}", rules_path.display());
                    println!("Run 'mailsweep rules edit' to create one.");
                    return Ok(());
                }
                
                // Attempt to parse the YAML file
                match std::fs::read_to_string(&rules_path) {
                    Ok(yaml_str) => {
                        // Parse the YAML
                        match serde_yaml::from_str::<Rules>(&yaml_str) {
                            Ok(rules) => {
                                // File exists and is valid YAML, now validate the content
                                let validation_errors = rules.validate();
                                
                                if validation_errors.is_empty() {
                                    println!("✅ Rules are valid");
                                    
                                    // Show some stats
                                    println!("\nOverview:");
                                    println!("  Rules: {}", rules.items.len());
                                    
                                    if rules.items.is_empty() {
                                        println!("\n⚠️ Warning: No rules defined. Messages won't be processed.");
                                        println!("Run 'mailsweep rules edit' to add rules.");
                                    }
                                } else {
                                    println!("❌ Rules have {} validation error(s):", validation_errors.len());
                                    for (i, error) in validation_errors.iter().enumerate() {
                                        println!("  {}. {}", i + 1, error);
                                    }
                                    println!("\nRun 'mailsweep rules edit' to fix these errors.");
                                }
                            },
                            Err(err) => {
                                println!("❌ Invalid YAML in rules file: {}", err);
                                println!("Run 'mailsweep rules edit' to fix the syntax errors.");
                            }
                        }
                    },
                    Err(err) => {
                        println!("❌ Error reading rules file: {}", err);
                    }
                }
                
                Ok(())
            },
            RulesCommands::Reset { force } => {
                // Get the rules path
                let rules_path = Rules::get_rules_path()?;
                
                let should_reset = if rules_path.exists() && !force {
                    // Ask for confirmation if not forced using inquire
                    Confirm::new("Are you sure you want to reset your rules to default?")
                        .with_default(false)
                        .with_help_message("This will remove all your current rules")
                        .prompt()
                        .unwrap_or(false) // Default to false (no reset) if there's an error
                } else {
                    // Force reset or rules don't exist
                    true
                };
                
                if should_reset {
                    // Create a new default rules set
                    let default_rules = Rules::new();
                    
                    // Update the schema file to the latest version
                    Rules::update_schema_file()?;
                    
                    // Save the rules
                    default_rules.save()?;
                    
                    if rules_path.exists() {
                        println!("Rules and schema have been reset to default.");
                    } else {
                        println!("Default rules file created.");
                    }
                } else {
                    println!("Reset cancelled.");
                }
                
                Ok(())
            },
            RulesCommands::Add { name, action, sender, subject } => {
                // Validate inputs
                if name.trim().is_empty() {
                    anyhow::bail!("Rule name cannot be empty");
                }
                
                if sender.is_empty() && subject.is_empty() {
                    anyhow::bail!("At least one sender or subject pattern must be provided");
                }
                
                // Parse action
                let action_lower = action.to_lowercase();
                let rule_action = match action_lower.as_str() {
                    "archive" => crate::rules::RuleAction::Archive,
                    "delete" => crate::rules::RuleAction::Delete,
                    "mark_read" | "markread" => crate::rules::RuleAction::MarkRead,
                    _ => {
                        anyhow::bail!("Invalid action: '{}'. Must be one of: archive, delete, mark_read", action);
                    }
                };
                
                // Create the new rule
                let new_rule = crate::rules::Rule {
                    name,
                    sender_contains: crate::rules::PatternSet::with_patterns(sender),
                    subject_contains: crate::rules::PatternSet::with_patterns(subject),
                    action: rule_action
                };
                
                // Load existing rules
                let mut rules = crate::rules::Rules::load()?;
                
                // Add the new rule
                rules.items.push(new_rule);
                
                // Save the updated rules
                rules.save()?;
                
                println!("✅ Rule added successfully");
                println!("Run 'mailsweep rules show' to see all rules");
                Ok(())
            },
        }
    }
}
