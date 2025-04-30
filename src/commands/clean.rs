use crate::auth::Auth;
use crate::graph_client::{BatchOperation, GraphClient};
use crate::rules::{RuleAction, Rules};
use anyhow::Result;
use chrono::Utc;
use clap::Args;
use inquire::Confirm;
use std::cmp::max;
use std::collections::HashMap;
use tabled::Tabled; // Keep only the Tabled derive
use terminal_size::{Width as TermWidth, terminal_size};

// Use this struct to display messages in the table
#[derive(Tabled, Debug, Clone)]
struct MessageDisplay {
    #[tabled(rename = "Action")]
    action: String,

    #[tabled(rename = "Sender")]
    sender: String,

    #[tabled(rename = "Subject")]
    subject: String,

    #[tabled(rename = "Received")]
    received: String,
}

// Import Message from graph_client and use it directly

#[derive(Debug, Args)]
pub struct CleanCommand {
    /// Maximum number of messages to process
    #[arg(long)]
    max_messages: Option<usize>,

    /// Process all matching messages without confirmation
    #[arg(long)]
    yes: bool,
}

impl CleanCommand {
    pub async fn execute(self) -> Result<()> {
        // Load auth and rules
        let auth = Auth::new()?;
        let token = auth.ensure_valid_token().await.map_err(|_| {
            anyhow::anyhow!("You are not authenticated. Please run 'mailsweep auth login' first.")
        })?;
        let rules = Rules::load()?;

        // Create Microsoft Graph client
        let graph_client = GraphClient::new(token.access_token);

        // Default max messages per page (MS Graph API limit is 1000)
        let per_page = self.max_messages.unwrap_or(50);
        println!("Fetching messages from your inbox...");

        // If no rules are configured, prompt the user
        if rules.items.is_empty() {
            println!("⚠️ No rules configured. Use 'mailsweep rules edit' to add rules.");
            return Ok(());
        }

        // Get messages from inbox with pagination
        let mut all_messages_json = Vec::new();
        let mut next_link: Option<String>;

        // First page
        let (messages, next) = graph_client.fetch_messages_page(per_page, None).await?;
        if !messages.is_empty() {
            all_messages_json.extend(messages);
        }
        next_link = next;

        // Fetch subsequent pages if available
        while let Some(link) = next_link {
            let (messages, next) = graph_client
                .fetch_messages_page(per_page, Some(&link))
                .await?;
            if !messages.is_empty() {
                all_messages_json.extend(messages);
            }
            next_link = next;
        }

        if all_messages_json.is_empty() {
            println!("No messages found in your inbox.");
            return Ok(());
        }

        // Process messages to find matches
        let mut messages = Vec::new();

        for msg_json in &all_messages_json {
            let mut message = graph_client.parse_message(msg_json);

            // Check each rule
            for rule in &rules.items {
                // Use the Rule.matches method
                if rule.matches(&message.sender, &message.subject) {
                    message.matched_rule = Some(rule.name.clone());
                    message.action = Some(rule.action.clone());
                    break; // Stop processing rules for this message
                }
            }

            // Only keep messages that matched a rule
            if message.matched_rule.is_some() {
                messages.push(message);
            }
        }

        // Check if any messages matched rules
        if messages.is_empty() {
            println!("No messages matched your rules.");
            return Ok(());
        }

        // First sort messages by rule name for grouping
        messages.sort_by(|a, b| {
            // First compare by rule name
            let rule_cmp = a.matched_rule.cmp(&b.matched_rule);
            // If rules are the same, then sort by received date (newest first)
            if rule_cmp.is_eq() {
                b.received_date.cmp(&a.received_date)
            } else {
                rule_cmp
            }
        });

        // Create table data
        let mut table_data = Vec::new();

        for msg in &messages {
            // Get a nice human-readable action name with emoji
            let action_str = match msg.action.as_ref().unwrap() {
                // Use fixed-width emojis with proper spacing
                RuleAction::Archive => "📥 Archive ",
                RuleAction::Delete => "🗑️ Delete  ",
                RuleAction::MarkRead => "👁️ Mark Read",
            };

            // Format the received date as a relative time
            let now = Utc::now();
            let diff = now.signed_duration_since(msg.received_date);

            let received_relative = if diff.num_days() > 0 {
                format!("{} days ago", diff.num_days())
            } else if diff.num_hours() > 0 {
                format!("{} hours ago", diff.num_hours())
            } else if diff.num_minutes() > 0 {
                format!("{} minutes ago", diff.num_minutes())
            } else {
                "just now".to_string()
            };

            // Add colored action
            let action_with_color = match msg.action.as_ref().unwrap() {
                RuleAction::Archive => format!("\x1b[34m{}\x1b[0m", action_str),
                RuleAction::Delete => format!("\x1b[31m{}\x1b[0m", action_str),
                RuleAction::MarkRead => format!("\x1b[32m{}\x1b[0m", action_str),
            };

            table_data.push(MessageDisplay {
                action: action_with_color,
                sender: msg.sender.clone(),
                subject: msg.subject.clone(),
                received: received_relative,
            });
        }

        // Calculate an appropriate width for the table based on terminal size
        let term_width = match terminal_size() {
            Some((TermWidth(w), _)) => {
                // For very wide terminals, don't use the full width
                if w > 200 {
                    180
                } else {
                    max(80, w as usize - 5) // Leave minimal padding
                }
            }
            None => 100, // Default width if terminal size can't be determined
        };

        // Display the count message before the table
        println!("\n\x1b[1;36m{} matching messages:\x1b[0m\n", messages.len());

        // Define fixed column widths
        let action_width = 15; // Fixed width for action column
        let received_width = 15; // Fixed width for received column

        // Calculate dynamic widths based on percentage of available space
        // Reserve space for spacing between columns (3 spaces between each column × 3 gaps)
        let available_width = term_width - 9;

        // For very wide terminals, use a reasonable width for the sender column
        let sender_width = if term_width > 160 {
            45 // Fixed width for very wide terminals
        } else {
            // Otherwise use a percentage of available width
            (available_width as f32 * 0.3) as usize // 30% of available width
        };

        // Ensure subject gets remaining space but has a minimum width
        let subject_width = max(
            30,
            available_width - action_width - received_width - sender_width,
        );

        // Create table borders with appropriate width
        let header_border = "-".repeat(term_width);

        // Print header with proper spacing and alignment
        println!("{}", header_border);
        println!(
            "\x1b[1;34m{:<action_width$}\x1b[0m   \x1b[1;32m{:<sender_width$}\x1b[0m   \x1b[1;33m{:<subject_width$}\x1b[0m   \x1b[1;31m{:<received_width$}\x1b[0m",
            "Action", "Sender", "Subject", "Received"
        );
        println!("{}", header_border);

        // Track the current rule group to know when to print a group header
        let mut current_rule: Option<&str> = None;

        // Loop through messages to display them grouped by rule
        for (i, msg) in messages.iter().enumerate() {
            // Check if we're starting a new rule group
            let rule_name = msg.matched_rule.as_ref().unwrap();

            // If this is a new rule group or the first message
            if current_rule.is_none() || current_rule != Some(rule_name) {
                // Print the rule group header centered
                let rule_display = format!(" {} ", rule_name);
                let padding = term_width.saturating_sub(rule_display.len()) / 2;
                let centered_header = format!("{}{}{}", "·".repeat(padding), rule_display, "·".repeat(padding));
                println!("\x1b[1;36m{}\x1b[0m", centered_header);

                // Update current rule
                current_rule = Some(rule_name);
            }

            // Get the corresponding table data row
            let table_row = &table_data[i];

            // Strip ANSI escape codes for width calculation
            let mut action_visible = String::new();
            let mut in_escape = false;

            for c in table_row.action.chars() {
                if c == '\x1b' {
                    in_escape = true;
                    continue;
                }

                if in_escape {
                    if c == 'm' {
                        in_escape = false;
                    }
                    continue;
                }

                action_visible.push(c);
            }

            // Account for emoji width (each emoji typically counts as 2 char width)
            // Calculate padded action string
            let mut action_padded = table_row.action.clone();

            // Count emojis (simplistic approach - just counts emoji-like characters)
            let emoji_count = action_visible
                .chars()
                .filter(|&c| {
                    ('\u{1F300}'..='\u{1F6FF}').contains(&c)
                        || ('\u{2600}'..='\u{26FF}').contains(&c)
                })
                .count();

            // Adjust visible length to account for emoji width (each emoji is 1 char but often displays as 2 width)
            let visible_len = action_visible.chars().count() + emoji_count;

            let action_display_padding = action_width.saturating_sub(visible_len);
            if action_display_padding > 0 {
                action_padded.push_str(&" ".repeat(action_display_padding));
            }

            // Calculate display width for sender
            let sender_chars = table_row.sender.chars().count();
            let sender_display = if sender_chars > sender_width {
                let mut shortened_sender = String::new();

                for (i, c) in table_row.sender.chars().enumerate() {
                    // Leave space for the ellipsis (3 chars)
                    if i >= sender_width - 3 {
                        break;
                    }
                    shortened_sender.push(c);
                }
                format!("{}...", shortened_sender)
            } else {
                format!("{:<sender_width$}", table_row.sender)
            };

            // Calculate display width for subject
            let subject_chars = table_row.subject.chars().count();
            let subject_display = if subject_chars > subject_width {
                let mut shortened_subject = String::new();

                for (i, c) in table_row.subject.chars().enumerate() {
                    // Leave space for the ellipsis (3 chars)
                    if i >= subject_width - 3 {
                        break;
                    }
                    shortened_subject.push(c);
                }
                format!("{}...", shortened_subject)
            } else {
                format!("{:<subject_width$}", table_row.subject)
            };

            // Calculate display width for received
            let received_chars = table_row.received.chars().count();
            let received_display = if received_chars > received_width {
                let mut shortened_received = String::new();

                for (i, c) in table_row.received.chars().enumerate() {
                    // Leave space for the ellipsis (3 chars)
                    if i >= received_width - 3 {
                        break;
                    }
                    shortened_received.push(c);
                }
                format!("{}...", shortened_received)
            } else {
                format!("{:<received_width$}", table_row.received)
            };

            // Print the row with fixed-width separators
            println!(
                "{}   {}   {}   {}",
                action_padded, sender_display, subject_display, received_display
            );
        }

        println!("{}\n", header_border);

        // Ask for confirmation unless --yes flag is used
        let proceed = if self.yes {
            true
        } else {
            println!("The actions above will be applied to the matching messages.");
            Confirm::new("Do you want to proceed?")
                .with_default(false)
                .with_help_message("Select 'Yes' to apply these actions, 'No' to cancel")
                .prompt()
                .unwrap_or(false)
        };

        if !proceed {
            println!("Operation cancelled. No changes made.");
            return Ok(());
        }

        // Process the messages using batch requests
        println!("Processing messages...");

        // Group messages by action type
        let mut archive_messages = Vec::new();
        let mut delete_messages = Vec::new();
        let mut mark_read_messages = Vec::new();

        for message in &messages {
            match message.action.as_ref().unwrap() {
                RuleAction::Archive => archive_messages.push(message),
                RuleAction::Delete => delete_messages.push(message),
                RuleAction::MarkRead => mark_read_messages.push(message),
            }
        }

        // Use batch requests to process messages in parallel
        let mut batch_results = Vec::new();

        // Process archive messages if any
        if !archive_messages.is_empty() {
            let result = graph_client
                .process_messages_batch(&archive_messages, BatchOperation::Archive)
                .await;

            batch_results.push((result, "archive"));
        }

        // Process delete messages if any
        if !delete_messages.is_empty() {
            let result = graph_client
                .process_messages_batch(&delete_messages, BatchOperation::Delete)
                .await;

            batch_results.push((result, "delete"));
        }

        // Process mark read messages if any
        if !mark_read_messages.is_empty() {
            let result = graph_client
                .process_messages_batch(&mark_read_messages, BatchOperation::MarkRead)
                .await;

            batch_results.push((result, "mark read"));
        }

        // Collect results by action type
        let mut action_counts = HashMap::new();
        let mut failed = 0;

        for (result, operation) in batch_results {
            match result {
                Ok(stats) => {
                    // Add the successful operations to the counts
                    match operation {
                        "archive" => *action_counts.entry("archived").or_insert(0) += stats.0,
                        "delete" => *action_counts.entry("deleted").or_insert(0) += stats.0,
                        "mark read" => {
                            *action_counts.entry("marked as read").or_insert(0) += stats.0
                        }
                        _ => *action_counts.entry(operation).or_insert(0) += stats.0,
                    }

                    // Report any failures by operation type
                    if stats.1 > 0 {
                        failed += stats.1;
                        println!("  {} operation: {} failed", operation, stats.1);
                    }
                }
                Err(e) => {
                    failed += 1; // At least one failure
                    eprintln!("Error processing {} batch: {}", operation, e);
                }
            }
        }

        // Display summary by action
        let mut summary = String::new();
        let mut total_processed = 0;

        for (action, count) in action_counts.iter() {
            if *count > 0 {
                // Format like "5 messages archived"
                let msg_text = if *count == 1 { "message" } else { "messages" };

                if !summary.is_empty() {
                    summary.push_str(", ");
                }
                summary.push_str(&format!("{} {} {}", count, msg_text, action));

                total_processed += count;
            }
        }

        // Add failure count if any
        if failed > 0 {
            let failure_text = if failed == 1 { "failure" } else { "failures" };
            if !summary.is_empty() {
                summary.push_str(", ");
            }
            summary.push_str(&format!("{} {}", failed, failure_text));
        }

        println!("\nCompleted: {}", summary);

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rules::{PatternSet, Rule};

    // Use the Rule's matches method
    fn matches_rule(rule: &Rule, sender: &str, subject: &str) -> bool {
        rule.matches(sender, subject)
    }

    #[test]
    fn test_rule_matching() {
        // Test rule with only sender pattern
        let sender_rule = Rule {
            name: "Sender rule".to_string(),
            sender_contains: PatternSet::with_patterns(vec!["example.com".to_string()]),
            subject_contains: PatternSet::new(),
            action: RuleAction::Archive,
        };

        assert!(
            matches_rule(&sender_rule, "user@example.com", "Any subject"),
            "Should match sender pattern"
        );
        assert!(
            !matches_rule(&sender_rule, "user@different.com", "Any subject"),
            "Should not match different sender"
        );

        // Test rule with only subject pattern
        let subject_rule = Rule {
            name: "Subject rule".to_string(),
            sender_contains: PatternSet::new(),
            subject_contains: PatternSet::with_patterns(vec!["important".to_string()]),
            action: RuleAction::MarkRead,
        };

        assert!(
            matches_rule(&subject_rule, "any@example.com", "This is important"),
            "Should match subject pattern"
        );
        assert!(
            !matches_rule(&subject_rule, "any@example.com", "Regular mail"),
            "Should not match different subject"
        );

        // Test rule with both sender and subject patterns (must match both)
        let combined_rule = Rule {
            name: "Combined rule".to_string(),
            sender_contains: PatternSet::with_patterns(vec!["newsletter".to_string()]),
            subject_contains: PatternSet::with_patterns(vec!["updates".to_string()]),
            action: RuleAction::Delete,
        };

        assert!(
            matches_rule(&combined_rule, "newsletter@example.com", "Weekly updates"),
            "Should match both patterns"
        );
        assert!(
            !matches_rule(&combined_rule, "newsletter@example.com", "Welcome"),
            "Should not match when only sender matches"
        );
        assert!(
            !matches_rule(&combined_rule, "user@example.com", "Weekly updates"),
            "Should not match when only subject matches"
        );

        // Test case insensitivity
        assert!(
            matches_rule(&combined_rule, "NEWSLETTER@example.com", "Weekly UPDATES"),
            "Should match case-insensitively"
        );
    }
}
