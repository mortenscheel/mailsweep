use oauth2::{
    AuthUrl, ClientId, DeviceAuthorizationUrl, TokenUrl,
    basic::BasicClient,
};
use std::io::{BufRead, BufReader};
use std::process::{Command, Stdio};

pub async fn debug_auth() -> Result<(), Box<dyn std::error::Error>> {
    let client_id = "0cadb66e-6914-4a9f-8058-3ba6e5cb58d8";
    let auth_url = "https://login.microsoftonline.com/common/oauth2/v2.0/authorize".to_string();
    let token_url = "https://login.microsoftonline.com/common/oauth2/v2.0/token".to_string();
    let device_auth_url = "https://login.microsoftonline.com/common/oauth2/v2.0/devicecode".to_string();

    println!("Debug: Creating client with:");
    println!("Debug: - Client ID: {}", client_id);
    println!("Debug: - Auth URL: {}", auth_url);
    println!("Debug: - Token URL: {}", token_url);
    println!("Debug: - Device Auth URL: {}", device_auth_url);

    let _client = BasicClient::new(
        ClientId::new(client_id.to_string()),
        None,
        AuthUrl::new(auth_url)?,
        Some(TokenUrl::new(token_url)?)
    )
    .set_device_authorization_url(DeviceAuthorizationUrl::new(device_auth_url)?);

    println!("Debug: Client created successfully");

    // Test device code flow
    let scopes = vec![
        "https://graph.microsoft.com/Mail.ReadWrite", // Includes Mail.Read capabilities
        "offline_access",
        "User.Read",
    ];

    println!("Debug: Requesting device code with scopes: {:?}", scopes);

    // Try to make the HTTP request manually to see if we get a response
    let mut curl = Command::new("curl")
        .arg("-v") // Verbose output
        .arg("-X")
        .arg("POST")
        .arg("https://login.microsoftonline.com/common/oauth2/v2.0/devicecode")
        .arg("-H")
        .arg("Content-Type: application/x-www-form-urlencoded")
        .arg("-d")
        .arg(format!(
            "client_id={}&scope={}",
            client_id,
            scopes.join(" ")
        ))
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?;

    println!("Debug: Executed curl command, waiting for response...");

    let stdout = BufReader::new(curl.stdout.take().unwrap());
    let stderr = BufReader::new(curl.stderr.take().unwrap());

    println!("Debug: HTTP Response:");
    for line in stdout.lines() {
        println!("Debug: stdout: {}", line?);
    }

    println!("Debug: HTTP Details:");
    for line in stderr.lines() {
        println!("Debug: stderr: {}", line?);
    }

    let status = curl.wait()?;
    println!("Debug: curl exited with status: {}", status);

    Ok(())
}