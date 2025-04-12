use anyhow::Result;
use chrono::{DateTime, Utc};
use serde::Deserialize;
use serde_json::Value;

/// Url constants for Microsoft Graph API
pub const GRAPH_BASE_URL: &str = "https://graph.microsoft.com/v1.0";

/// UserInfo returned from Microsoft Graph
#[derive(Debug, Deserialize)]
pub struct UserInfo {
    #[serde(rename = "displayName")]
    pub display_name: String,
}

/// Structure representing an email message
#[derive(Debug, Clone)]
pub struct Message {
    pub id: String,
    pub subject: String,
    pub sender: String,
    pub received_date: DateTime<Utc>,
    pub matched_rule: Option<String>,
    pub action: Option<crate::rules::RuleAction>,
}

/// Operations that can be performed on messages
#[derive(Debug, Clone, Copy)]
pub enum BatchOperation {
    Archive,
    Delete,
    MarkRead,
}

/// Result of a batch operation (success_count, failure_count)
pub type BatchResult = (usize, usize);

/// Client for interacting with Microsoft Graph API
pub struct GraphClient {
    client: reqwest::Client,
    access_token: String,
}

impl GraphClient {
    /// Create a new Microsoft Graph client with the given access token
    pub fn new(access_token: String) -> Self {
        Self {
            client: reqwest::Client::new(),
            access_token,
        }
    }

    /// Get the authenticated user's information
    pub async fn get_user_info(&self) -> Result<UserInfo> {
        let url = format!("{}/me", GRAPH_BASE_URL);
        let response = self
            .client
            .get(&url)
            .header("Authorization", format!("Bearer {}", self.access_token))
            .send()
            .await
            .map_err(|e| anyhow::anyhow!("Failed to fetch user info: {}", e))?;

        if !response.status().is_success() {
            let status = response.status();
            let error_text = response
                .text()
                .await
                .unwrap_or_else(|_| "Could not get error details".to_string());
            return Err(anyhow::anyhow!(
                "Failed to get user info (HTTP {}): {}",
                status,
                error_text
            ));
        }

        let user_info: UserInfo = response
            .json()
            .await
            .map_err(|e| anyhow::anyhow!("Failed to parse user info response: {}", e))?;

        Ok(user_info)
    }

    /// Fetch a page of messages from the inbox
    pub async fn fetch_messages_page(
        &self,
        per_page: usize,
        next_link: Option<&str>,
    ) -> Result<(Vec<Value>, Option<String>)> {
        let url = if let Some(link) = next_link {
            link.to_string()
        } else {
            format!(
                "{}/me/mailFolders/inbox/messages?$top={}&$select=id,subject,from,receivedDateTime",
                GRAPH_BASE_URL, per_page
            )
        };

        let response = self
            .client
            .get(&url)
            .header("Authorization", format!("Bearer {}", self.access_token))
            .send()
            .await?;

        if !response.status().is_success() {
            let error_text = response.text().await?;
            anyhow::bail!("Failed to fetch messages: {}", error_text);
        }

        let data: Value = response.json().await?;
        let messages = data["value"]
            .as_array()
            .ok_or_else(|| anyhow::anyhow!("Unexpected response format"))?
            .clone();

        // Check for @odata.nextLink for pagination
        let next_link = data["@odata.nextLink"].as_str().map(|s| s.to_string());

        Ok((messages, next_link))
    }

    /// Convert raw JSON message data to a Message struct
    pub fn parse_message(&self, msg_json: &Value) -> Message {
        let id = msg_json["id"].as_str().unwrap_or("unknown").to_string();
        let subject = msg_json["subject"]
            .as_str()
            .unwrap_or("(No subject)")
            .to_string();
        let sender_email = msg_json["from"]["emailAddress"]["address"]
            .as_str()
            .unwrap_or("unknown")
            .to_string();
        let sender_name = msg_json["from"]["emailAddress"]["name"]
            .as_str()
            .unwrap_or(&sender_email)
            .to_string();
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

        Message {
            id,
            subject,
            sender,
            received_date,
            matched_rule: None,
            action: None,
        }
    }

    /// Process a batch of messages with the same operation type
    pub async fn process_messages_batch(
        &self,
        messages: &[&Message],
        operation: BatchOperation,
    ) -> Result<BatchResult> {
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
                let (method, url, body) = match operation {
                    BatchOperation::Archive => {
                        let url = format!("/me/messages/{}/move", message.id);
                        let body = serde_json::json!({
                            "destinationId": "archive"
                        });
                        ("POST", url, Some(body))
                    }
                    BatchOperation::Delete => {
                        let url = format!("/me/messages/{}", message.id);
                        ("DELETE", url, None)
                    }
                    BatchOperation::MarkRead => {
                        let url = format!("/me/messages/{}", message.id);
                        let body = serde_json::json!({
                            "isRead": true
                        });
                        ("PATCH", url, Some(body))
                    }
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
            let url = format!("{}/$batch", GRAPH_BASE_URL);
            let response = self
                .client
                .post(&url)
                .header("Authorization", format!("Bearer {}", self.access_token))
                .header("Content-Type", "application/json")
                .json(&batch_payload)
                .send()
                .await?;

            if !response.status().is_success() {
                let error_text = response.text().await?;
                anyhow::bail!("Failed to process batch request: {}", error_text);
            }

            // Process batch response
            let batch_response: Value = response.json().await?;
            let responses = batch_response["responses"]
                .as_array()
                .ok_or_else(|| anyhow::anyhow!("Invalid batch response format"))?;

            // Count successes and failures
            for response in responses {
                let status = response["status"].as_u64().unwrap_or(500);

                if (200..300).contains(&status) {
                    succeeded += 1;
                } else {
                    failed += 1;
                    let error = response["body"]["error"]["message"]
                        .as_str()
                        .unwrap_or("Unknown error");
                    eprintln!(
                        "Error in batch request: Status {}, Message: {}",
                        status, error
                    );
                }
            }
        }

        Ok((succeeded, failed))
    }
}
