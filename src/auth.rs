use anyhow::Result;
use oauth2::{
    AuthUrl, ClientId, DeviceAuthorizationUrl, RedirectUrl, Scope, TokenResponse, TokenUrl,
    basic::{BasicClient, BasicTokenResponse},
    devicecode::{DeviceAuthorizationResponse, EmptyExtraDeviceAuthorizationFields},
    reqwest::async_http_client,
};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::time::Duration;
use time::OffsetDateTime;

// Azure App registration details for mailsweep
// - multitenant
// - public client flow
const CLIENT_ID: &str = "0cadb66e-6914-4a9f-8058-3ba6e5cb58d8";

// Common Microsoft Identity Platform (Azure AD v2.0) endpoints
const MS_GRAPH_AUTH_URL: &str = "https://login.microsoftonline.com/common/oauth2/v2.0/authorize";
const MS_GRAPH_TOKEN_URL: &str = "https://login.microsoftonline.com/common/oauth2/v2.0/token";
const MS_GRAPH_DEVICE_AUTH_URL: &str =
    "https://login.microsoftonline.com/common/oauth2/v2.0/devicecode";

#[derive(Debug, Serialize, Deserialize)]
pub struct TokenCache {
    pub access_token: String,
    pub refresh_token: String,
    pub expires_at: OffsetDateTime,
}

// Using UserInfo from graph_client

impl TokenCache {
    pub fn is_expired(&self) -> bool {
        OffsetDateTime::now_utc() >= self.expires_at
    }

    pub fn from_token_response(token: BasicTokenResponse) -> Self {
        let expires_in = token.expires_in().unwrap_or(Duration::from_secs(3600));
        let expires_at =
            OffsetDateTime::now_utc() + time::Duration::seconds(expires_in.as_secs() as i64);

        Self {
            access_token: token.access_token().secret().clone(),
            refresh_token: token.refresh_token().unwrap().secret().clone(),
            expires_at,
        }
    }
}

pub struct Auth {
    client: BasicClient,
    token_cache_path: PathBuf,
}

impl Auth {
    pub fn new() -> Result<Self> {
        // Create OAuth2 client for Microsoft identity platform
        let client = BasicClient::new(
            ClientId::new(CLIENT_ID.to_string()),
            None, // No client secret for public client
            AuthUrl::new(MS_GRAPH_AUTH_URL.to_string())?,
            Some(TokenUrl::new(MS_GRAPH_TOKEN_URL.to_string())?),
        )
        .set_device_authorization_url(DeviceAuthorizationUrl::new(
            MS_GRAPH_DEVICE_AUTH_URL.to_string(),
        )?)
        .set_redirect_uri(RedirectUrl::new("http://localhost".to_string())?); // Not used with device flow

        // Use our config module to get the token cache path
        let token_cache_path = crate::config::place_config_file("token_cache.yaml")?;

        Ok(Self {
            client,
            token_cache_path,
        })
    }

    /// Performs device code authentication flow with Microsoft Graph
    pub async fn login(&self) -> Result<()> {
        println!(
            "Starting authentication flow with Microsoft Graph (client ID: {})",
            CLIENT_ID
        );

        // Define scopes needed for the application
        let scopes = vec![
            "offline_access",                             // Required for refresh tokens
            "https://graph.microsoft.com/Mail.ReadWrite", // Includes Mail.Read capabilities
            "User.Read",                                  // For accessing user profile information
        ];

        println!(
            "Requesting device code authentication with scopes: {:?}",
            scopes
        );

        // Start device code flow
        let details: DeviceAuthorizationResponse<EmptyExtraDeviceAuthorizationFields> = self
            .client
            .exchange_device_code()?
            .add_scopes(scopes.iter().map(|s| Scope::new(s.to_string())))
            .request_async(async_http_client)
            .await
            .map_err(|e| anyhow::anyhow!("Device code request failed: {:?}", e))?;

        // Display user instructions
        println!("\nTo sign in to Microsoft Graph, use a web browser to open:");
        println!("  {}", details.verification_uri().as_str());
        println!("\nAnd enter the code: {}", details.user_code().secret());
        println!("\nWaiting for authentication...");

        // Poll for token (the library handles polling automatically)
        let token = self
            .client
            .exchange_device_access_token(&details)
            .request_async(async_http_client, tokio::time::sleep, None)
            .await
            .map_err(|e| anyhow::anyhow!("Token exchange failed: {:?}", e))?;

        // Save token to cache
        let token_cache = TokenCache::from_token_response(token);
        self.save_token_cache(&token_cache)?;

        // Get user information using GraphClient
        let graph_client = crate::graph_client::GraphClient::new(token_cache.access_token.clone());
        match graph_client.get_user_info().await {
            Ok(user_info) => {
                println!(
                    "Authentication successful! You are signed in as {}",
                    user_info.display_name
                );
            }
            Err(_) => {
                println!("Authentication successful! Token has been saved.");
            }
        }
        Ok(())
    }

    /// Refreshes the token if it's expired
    pub async fn ensure_valid_token(&self) -> Result<TokenCache> {
        if let Ok(mut token_cache) = self.load_token_cache() {
            if token_cache.is_expired() {
                // Silently refresh the token
                let token = self
                    .client
                    .exchange_refresh_token(&oauth2::RefreshToken::new(
                        token_cache.refresh_token.clone(),
                    ))
                    .request_async(async_http_client)
                    .await
                    .map_err(|e| anyhow::anyhow!("Failed to refresh token: {:?}", e))?;

                // Update cache with new token
                token_cache = TokenCache::from_token_response(token);
                self.save_token_cache(&token_cache)?;
            }
            Ok(token_cache)
        } else {
            Err(anyhow::anyhow!(
                "Not authenticated. Run 'mailsweep auth login' first."
            ))
        }
    }

    /// Checks if we're authenticated and the token is valid
    pub async fn check(&self) -> Result<()> {
        match self.ensure_valid_token().await {
            Ok(token) => {
                // Get the user's name from Microsoft Graph using GraphClient
                let graph_client = crate::graph_client::GraphClient::new(token.access_token);
                let user_info = graph_client.get_user_info().await?;
                println!("Authenticated as {}", user_info.display_name);
                Ok(())
            }
            Err(e) => Err(e),
        }
    }

    /// Logs out by removing the token cache
    pub fn logout(&self) -> Result<()> {
        if self.token_cache_path.exists() {
            std::fs::remove_file(&self.token_cache_path)?;
            println!("Successfully logged out");
            Ok(())
        } else {
            println!("Not logged in");
            Ok(())
        }
    }

    /// Saves token cache to file
    fn save_token_cache(&self, token_cache: &TokenCache) -> Result<()> {
        let yaml = serde_yaml::to_string(token_cache)?;
        std::fs::write(&self.token_cache_path, yaml)?;
        Ok(())
    }

    /// Loads token cache from file
    fn load_token_cache(&self) -> Result<TokenCache> {
        let yaml = std::fs::read_to_string(&self.token_cache_path)?;
        let token_cache: TokenCache = serde_yaml::from_str(&yaml)?;
        Ok(token_cache)
    }
}
