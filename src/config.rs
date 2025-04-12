use anyhow::Result;
use std::fs;
use std::path::PathBuf;
use std::env;

/// Get the application config directory path
pub fn get_app_config_dir() -> Result<PathBuf> {
    // Cross-platform config directory
    let config_dir = if cfg!(target_os = "windows") {
        // On Windows, use %APPDATA%\mailsweep
        match env::var("APPDATA") {
            Ok(appdata) => PathBuf::from(appdata).join("mailsweep"),
            Err(_) => {
                // Fallback to user profile directory if APPDATA is not set
                let home = env::var("USERPROFILE").map_err(|_| {
                    anyhow::anyhow!("Could not find user profile directory on Windows")
                })?;
                PathBuf::from(home).join("AppData").join("Roaming").join("mailsweep")
            }
        }
    } else {
        // On Unix-like systems (Linux, macOS), use XDG
        #[cfg(not(target_os = "windows"))]
        {
            let xdg_dirs = xdg::BaseDirectories::with_prefix("mailsweep")
                .map_err(|e| anyhow::anyhow!("Failed to initialize XDG base directories: {}", e))?;
            xdg_dirs.get_config_home()
        }
        
        #[cfg(target_os = "windows")]
        {
            // This code is never reached but needed for compilation on Windows
            // where xdg is not available
            PathBuf::new()
        }
    };

    // Ensure directory exists
    if !config_dir.exists() {
        fs::create_dir_all(&config_dir)?;
    }

    Ok(config_dir)
}

/// Get the path to a configuration file
pub fn get_config_file_path(filename: &str) -> Result<PathBuf> {
    let config_dir = get_app_config_dir()?;
    Ok(config_dir.join(filename))
}

/// Get the path to a configuration file as a string
pub fn get_config_file_path_str(filename: &str) -> Result<String> {
    Ok(get_config_file_path(filename)?
        .to_string_lossy()
        .to_string())
}

/// Place a config file and ensure its parent directory exists
pub fn place_config_file(filename: &str) -> Result<PathBuf> {
    let path = get_config_file_path(filename)?;
    
    // Ensure the parent directory exists
    if let Some(parent) = path.parent() {
        if !parent.exists() {
            fs::create_dir_all(parent)?;
        }
    }
    
    Ok(path)
}
