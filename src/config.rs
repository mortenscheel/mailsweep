use anyhow::Result;
use std::fs;
use std::path::PathBuf;

/// Get the application config directory path
pub fn get_app_config_dir() -> Result<PathBuf> {
    // Get directory using XDG Base Directory specification
    let xdg_dirs = xdg::BaseDirectories::with_prefix("mailsweep")
        .map_err(|e| anyhow::anyhow!("Failed to initialize XDG base directories: {}", e))?;

    let app_config_dir = xdg_dirs.get_config_home();

    // Ensure directory exists
    if !app_config_dir.exists() {
        fs::create_dir_all(&app_config_dir)?;
    }

    Ok(app_config_dir)
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
    let xdg_dirs = xdg::BaseDirectories::with_prefix("mailsweep")
        .map_err(|e| anyhow::anyhow!("Failed to initialize XDG base directories: {}", e))?;

    xdg_dirs
        .place_config_file(filename)
        .map_err(|e| anyhow::anyhow!("Failed to determine path for {}: {}", filename, e))
}
