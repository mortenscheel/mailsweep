use anyhow::Result;
use clap::Args;
use crate::auth::Auth;
use crate::rules::{Rules, RuleAction};
use tabled::Tabled; // Keep only the Tabled derive
use terminal_size::{Width as TermWidth, terminal_size};
use chrono::{DateTime, Utc};
use inquire::Confirm;
use std::cmp::max;
use std::collections::HashMap;

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

// Internal struct for message data
#[derive(Debug, Clone)]
struct Message {
    id: String,
    subject: String,
    sender: String,
    received_date: DateTime<Utc>,
    matched_rule: Option<String>,
    action: Option<RuleAction>,
}

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
        let token = auth.ensure_valid_token().await
            .map_err(|_| anyhow::anyhow!("You are not authenticated. Please run 'mailsweep auth login' first."))?;
        let rules = Rules::load()?;
        
        // Create Microsoft Graph client
        let client = reqwest::Client::new();
        
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
        let (messages, next) = fetch_messages_page(&client, &token.access_token, per_page, None).await?;
        if !messages.is_empty() {
            all_messages_json.extend(messages);
        }
        next_link = next;
        
        // Fetch subsequent pages if available
        while let Some(link) = next_link {
            let (messages, next) = fetch_messages_page(&client, &token.access_token, per_page, Some(&link)).await?;
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
            let id = msg_json["id"].as_str().unwrap_or("unknown").to_string();
            let subject = msg_json["subject"].as_str().unwrap_or("(No subject)").to_string();
            let sender_email = msg_json["from"]["emailAddress"]["address"].as_str().unwrap_or("unknown").to_string();
            let sender_name = msg_json["from"]["emailAddress"]["name"].as_str().unwrap_or(&sender_email).to_string();
            let sender = if sender_name != sender_email {
                format!("{} <{}>", sender_name, sender_email)
            } else {
                sender_email
            };
            
            // Parse received date
            let received_str = msg_json["receivedDateTime"].as_str().unwrap_or("");
            let received_date = if !received_str.is_empty() {
                match DateTime::parse_from_rfc3339(received_str) {
                    Ok(dt) => dt.with_timezone(&Utc),
                    Err(_) => Utc::now(), // Default to now if parsing fails
                }
            } else {
                Utc::now()
            };
            
            let mut message = Message {
                id,
                subject,
                sender,
                received_date,
                matched_rule: None,
                action: None,
            };
            
            // Check each rule
            for rule in &rules.items {
                let mut rule_matched = false;
                
                let sender_patterns = rule.sender_contains.to_vec();
                let subject_patterns = rule.subject_contains.to_vec();
                
                // Skip empty rules (should be caught by validation, but just in case)
                if sender_patterns.is_empty() && subject_patterns.is_empty() {
                    continue;
                }
                
                // If both pattern types are present, need to match at least one from each
                if !sender_patterns.is_empty() && !subject_patterns.is_empty() {
                    // Check for sender match
                    let mut sender_matched = false;
                    for pattern in &sender_patterns {
                        if message.sender.to_lowercase().contains(&pattern.to_lowercase()) {
                            sender_matched = true;
                            break;
                        }
                    }
                    
                    // Check for subject match
                    let mut subject_matched = false;
                    for pattern in &subject_patterns {
                        if message.subject.to_lowercase().contains(&pattern.to_lowercase()) {
                            subject_matched = true;
                            break;
                        }
                    }
                    
                    // Both must match for the rule to apply
                    rule_matched = sender_matched && subject_matched;
                }
                // If only sender patterns exist
                else if !sender_patterns.is_empty() {
                    for pattern in &sender_patterns {
                        if message.sender.to_lowercase().contains(&pattern.to_lowercase()) {
                            rule_matched = true;
                            break;
                        }
                    }
                }
                // If only subject patterns exist
                else if !subject_patterns.is_empty() {
                    for pattern in &subject_patterns {
                        if message.subject.to_lowercase().contains(&pattern.to_lowercase()) {
                            rule_matched = true;
                            break;
                        }
                    }
                }
                
                // If rule matched, save it
                if rule_matched {
                    message.matched_rule = Some(rule.name.clone());
                    message.action = Some(rule.action.clone());
                    break;  // Stop processing rules for this message
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
        
        // Sort messages by received date (newest first)
        messages.sort_by(|a, b| b.received_date.cmp(&a.received_date));
        
        // Create table for display
        let mut table_data = Vec::new();
        
        for msg in &messages {
            // Get a nice human-readable action name with emoji
            let action_str = match msg.action.as_ref().unwrap() {
                RuleAction::Archive => "📥 Archive",
                RuleAction::Delete => "🗑️ Delete",
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
            },
            None => 100, // Default width if terminal size can't be determined
        };
        
        // Display the count message before the table
        println!("\n\x1b[1;36m{} matching messages:\x1b[0m\n", messages.len());
        
        // Define fixed column widths
        let action_width = 15;     // Fixed width for action column
        let received_width = 15;   // Fixed width for received column
        
        // Calculate dynamic widths based on percentage of available space
        // Reserve space for spacing between columns (3 spaces between each column × 3 gaps)
        let available_width = term_width - 9; 
        
        // For very wide terminals, use a reasonable width for the sender column
        let sender_width = if term_width > 160 {
            45  // Fixed width for very wide terminals
        } else {
            // Otherwise use a percentage of available width
            (available_width as f32 * 0.3) as usize  // 30% of available width
        };
        
        // Ensure subject gets remaining space but has a minimum width
        let subject_width = max(30, available_width - action_width - received_width - sender_width);
        
        // Create table borders with appropriate width
        let header_border = "-".repeat(term_width);
        
        // Print header with proper spacing and alignment - ensure proper formatting
        println!("\x1b[1;34m{:<action_width$}\x1b[0m   \x1b[1;32m{:<sender_width$}\x1b[0m   \x1b[1;33m{:<subject_width$}\x1b[0m   \x1b[1;31m{:<received_width$}\x1b[0m", 
                "Action", "Sender", "Subject", "Received");
        println!("{}", header_border);
        
        // Display each message in a more controlled format
        for msg in &table_data {
            // Truncate sender if needed
            let sender_display = if msg.sender.len() > sender_width {
                format!("{}...", &msg.sender[0..(sender_width - 3)])
            } else {
                format!("{:<sender_width$}", msg.sender)
            };
            
            // Truncate subject if needed
            let subject_display = if msg.subject.len() > subject_width {
                format!("{}...", &msg.subject[0..(subject_width - 3)])
            } else {
                format!("{:<subject_width$}", msg.subject)
            };
            
            // Format received to fixed width
            let received_display = format!("{:<received_width$}", msg.received);
            
            // Print the row with proper spacing
            println!("{}   {}   {}   {}", 
                     format!("{:<action_width$}", msg.action),
                     sender_display,
                     subject_display,
                     received_display);
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
            let result = process_messages_batch(
                &client, 
                &token.access_token, 
                &archive_messages,
                BatchOperation::Archive
            ).await;
            
            batch_results.push((result, "archive"));
        }
        
        // Process delete messages if any
        if !delete_messages.is_empty() {
            let result = process_messages_batch(
                &client, 
                &token.access_token, 
                &delete_messages,
                BatchOperation::Delete
            ).await;
            
            batch_results.push((result, "delete"));
        }
        
        // Process mark read messages if any
        if !mark_read_messages.is_empty() {
            let result = process_messages_batch(
                &client, 
                &token.access_token, 
                &mark_read_messages,
                BatchOperation::MarkRead
            ).await;
            
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
                        "mark read" => *action_counts.entry("marked as read").or_insert(0) += stats.0,
                        _ => *action_counts.entry(operation).or_insert(0) += stats.0,
                    }
                    
                    // Report any failures by operation type
                    if stats.1 > 0 {
                        failed += stats.1;
                        println!("  {} operation: {} failed", operation, stats.1);
                    }
                },
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

async fn fetch_messages_page(
    client: &reqwest::Client, 
    token: &str, 
    per_page: usize, 
    next_link: Option<&str>
) -> Result<(Vec<serde_json::Value>, Option<String>)> {
    let url = if let Some(link) = next_link {
        link.to_string()
    } else {
        format!("https://graph.microsoft.com/v1.0/me/mailFolders/inbox/messages?$top={}&$select=id,subject,from,receivedDateTime", per_page)
    };
    
    let response = client.get(&url)
        .header("Authorization", format!("Bearer {}", token))
        .send()
        .await?;
        
    if !response.status().is_success() {
        let error_text = response.text().await?;
        anyhow::bail!("Failed to fetch messages: {}", error_text);
    }
    
    let data: serde_json::Value = response.json().await?;
    let messages = data["value"].as_array()
        .ok_or_else(|| anyhow::anyhow!("Unexpected response format"))?
        .clone();
    
    // Check for @odata.nextLink for pagination
    let next_link = data["@odata.nextLink"].as_str().map(|s| s.to_string());
    
    Ok((messages, next_link))
}

/// Enum defining the types of operations that can be performed on messages
enum BatchOperation {
    Archive,
    Delete,
    MarkRead,
}

/// Process a batch of messages with the same operation type
async fn process_messages_batch(
    client: &reqwest::Client,
    token: &str,
    messages: &[&Message],
    operation: BatchOperation
) -> Result<(usize, usize)> {
    // MS Graph allows up to 20 requests in a single batch
    const BATCH_SIZE: usize = 20;
    let mut succeeded = 0;
    let mut failed = 0;
    
    // Process messages in batches of BATCH_SIZE
    for chunk in messages.chunks(BATCH_SIZE) {
        let mut batch_requests = Vec::new();
        
        // Create batch requests
        for (i, message) in chunk.iter().enumerate() {
            let request_id = format!("{}", i + 1); // 1-based request IDs
            let (method, url, body) = match &operation {
                BatchOperation::Archive => {
                    let url = format!("/me/messages/{}/move", message.id);
                    let body = serde_json::json!({
                        "destinationId": "archive"
                    });
                    ("POST", url, Some(body))
                },
                BatchOperation::Delete => {
                    let url = format!("/me/messages/{}", message.id);
                    ("DELETE", url, None)
                },
                BatchOperation::MarkRead => {
                    let url = format!("/me/messages/{}", message.id);
                    let body = serde_json::json!({
                        "isRead": true
                    });
                    ("PATCH", url, Some(body))
                },
            };
            
            let mut request = serde_json::json!({
                "id": request_id,
                "method": method,
                "url": url,
                "headers": {
                    "Content-Type": "application/json"
                }
            });
            
            if let Some(body_json) = body {
                request["body"] = body_json;
            }
            
            batch_requests.push(request);
        }
        
        // Create the batch request
        let batch_payload = serde_json::json!({
            "requests": batch_requests
        });
        
        // Send the batch request
        let url = "https://graph.microsoft.com/v1.0/$batch";
        let response = client.post(url)
            .header("Authorization", format!("Bearer {}", token))
            .header("Content-Type", "application/json")
            .json(&batch_payload)
            .send()
            .await?;
            
        if !response.status().is_success() {
            let error_text = response.text().await?;
            anyhow::bail!("Failed to process batch request: {}", error_text);
        }
        
        // Process batch response
        let batch_response: serde_json::Value = response.json().await?;
        let responses = batch_response["responses"].as_array()
            .ok_or_else(|| anyhow::anyhow!("Invalid batch response format"))?;
        
        // Count successes and failures
        for response in responses {
            let status = response["status"].as_u64().unwrap_or(500);
            
            if (200..300).contains(&status) {
                succeeded += 1;
            } else {
                failed += 1;
                let error = response["body"]["error"]["message"].as_str().unwrap_or("Unknown error");
                eprintln!("Error in batch request: Status {}, Message: {}", status, error);
            }
        }
    }
    
    Ok((succeeded, failed))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rules::{Rule, PatternSet};
    
    // Helper function to simulate rule matching logic from execute method
    fn matches_rule(rule: &Rule, sender: &str, subject: &str) -> bool {
        let sender_patterns = rule.sender_contains.to_vec();
        let subject_patterns = rule.subject_contains.to_vec();
        
        if sender_patterns.is_empty() && subject_patterns.is_empty() {
            return false;
        }
        
        if !sender_patterns.is_empty() && !subject_patterns.is_empty() {
            // Both sender and subject patterns must match
            let mut sender_matched = false;
            for pattern in &sender_patterns {
                if sender.to_lowercase().contains(&pattern.to_lowercase()) {
                    sender_matched = true;
                    break;
                }
            }
            
            let mut subject_matched = false;
            for pattern in &subject_patterns {
                if subject.to_lowercase().contains(&pattern.to_lowercase()) {
                    subject_matched = true;
                    break;
                }
            }
            
            return sender_matched && subject_matched;
        } else if !sender_patterns.is_empty() {
            // Only sender patterns exist
            for pattern in &sender_patterns {
                if sender.to_lowercase().contains(&pattern.to_lowercase()) {
                    return true;
                }
            }
        } else if !subject_patterns.is_empty() {
            // Only subject patterns exist
            for pattern in &subject_patterns {
                if subject.to_lowercase().contains(&pattern.to_lowercase()) {
                    return true;
                }
            }
        }
        
        false
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
        
        assert!(matches_rule(&sender_rule, "user@example.com", "Any subject"), 
                "Should match sender pattern");
        assert!(!matches_rule(&sender_rule, "user@different.com", "Any subject"), 
                "Should not match different sender");
        
        // Test rule with only subject pattern
        let subject_rule = Rule {
            name: "Subject rule".to_string(),
            sender_contains: PatternSet::new(),
            subject_contains: PatternSet::with_patterns(vec!["important".to_string()]),
            action: RuleAction::MarkRead,
        };
        
        assert!(matches_rule(&subject_rule, "any@example.com", "This is important"), 
                "Should match subject pattern");
        assert!(!matches_rule(&subject_rule, "any@example.com", "Regular mail"), 
                "Should not match different subject");
        
        // Test rule with both sender and subject patterns (must match both)
        let combined_rule = Rule {
            name: "Combined rule".to_string(),
            sender_contains: PatternSet::with_patterns(vec!["newsletter".to_string()]),
            subject_contains: PatternSet::with_patterns(vec!["updates".to_string()]),
            action: RuleAction::Delete,
        };
        
        assert!(matches_rule(&combined_rule, "newsletter@example.com", "Weekly updates"), 
                "Should match both patterns");
        assert!(!matches_rule(&combined_rule, "newsletter@example.com", "Welcome"), 
                "Should not match when only sender matches");
        assert!(!matches_rule(&combined_rule, "user@example.com", "Weekly updates"), 
                "Should not match when only subject matches");
                
        // Test case insensitivity
        assert!(matches_rule(&combined_rule, "NEWSLETTER@example.com", "Weekly UPDATES"), 
                "Should match case-insensitively");
    }
}

