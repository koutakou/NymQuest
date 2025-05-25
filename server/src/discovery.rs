use anyhow::{Context, Result};
use std::env;
use std::fs;
use std::path::PathBuf;
use tracing::{debug, info, warn};

/// Module for server address discovery and sharing mechanisms
/// This module provides cross-platform functionality for saving and discovering server
/// addresses in a secure, reliable manner. It handles platform-specific data directories
/// while maintaining privacy and anonymity requirements through the mixnet.
pub const SERVER_ADDRESS_ENV_VAR: &str = "NYMQUEST_SERVER_ADDRESS_FILE";

/// Default filename for server address file
pub const SERVER_ADDRESS_FILENAME: &str = "nymquest_server.addr";

/// Get the platform-specific data directory for NymQuest server configuration
pub fn get_server_data_dir() -> Result<PathBuf> {
    // Use XDG Base Directory specification on Unix, AppData on Windows
    let base_dir = dirs_next::data_dir()
        .or_else(dirs_next::home_dir)
        .ok_or_else(|| anyhow::anyhow!("Cannot determine user data directory"))?;

    let nymquest_dir = base_dir.join("nymquest").join("server");

    // Ensure the directory exists
    fs::create_dir_all(&nymquest_dir).with_context(|| {
        format!(
            "Failed to create NymQuest data directory: {:?}",
            nymquest_dir
        )
    })?;

    debug!("Using server data directory: {:?}", nymquest_dir);
    Ok(nymquest_dir)
}

/// Get the full path where the server should save the address file
pub fn get_server_address_file_path() -> Result<PathBuf> {
    // Check for environment variable override first
    if let Ok(custom_path) = env::var(SERVER_ADDRESS_ENV_VAR) {
        let path = PathBuf::from(custom_path);
        info!(
            "Using custom server address file path from environment: {:?}",
            path
        );

        // Ensure parent directory exists
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).with_context(|| {
                format!(
                    "Failed to create parent directory for custom path: {:?}",
                    parent
                )
            })?;
        }

        return Ok(path);
    }

    // Use platform-specific data directory
    let data_dir = get_server_data_dir()?;
    let address_file = data_dir.join(SERVER_ADDRESS_FILENAME);

    debug!("Default server address file path: {:?}", address_file);
    Ok(address_file)
}

/// Generate a list of potential discovery paths for server address files
/// Returns paths in order of priority (highest to lowest)
#[allow(dead_code)]
pub fn get_server_address_discovery_paths() -> Vec<PathBuf> {
    let mut paths = Vec::new();

    // 1. Environment variable override (highest priority)
    if let Ok(custom_path) = env::var(SERVER_ADDRESS_ENV_VAR) {
        paths.push(PathBuf::from(custom_path));
    }

    // 2. Data directory (recommended standard location)
    if let Ok(data_dir) = get_server_data_dir() {
        paths.push(data_dir.join(SERVER_ADDRESS_FILENAME));
    }

    // 3. Current working directory (for development/testing)
    paths.push(PathBuf::from(SERVER_ADDRESS_FILENAME));
    paths.push(PathBuf::from("server_address.txt")); // Legacy compatibility

    // 4. Legacy relative paths (for backward compatibility)
    paths.push(PathBuf::from("../client/server_address.txt"));
    paths.push(PathBuf::from("../server_address.txt"));

    // 5. Home directory fallback
    if let Some(home_dir) = dirs_next::data_dir().or_else(dirs_next::home_dir) {
        paths.push(home_dir.join(".nymquest").join(SERVER_ADDRESS_FILENAME));
        paths.push(home_dir.join("server_address.txt")); // Legacy
    }

    debug!("Generated {} discovery paths", paths.len());
    paths
}

/// Find and read the server address file from discovery paths
/// Returns the first valid file found
#[allow(dead_code)]
pub fn discover_server_address_file() -> Result<(PathBuf, String)> {
    let discovery_paths = get_server_address_discovery_paths();

    for path in &discovery_paths {
        match fs::read_to_string(path) {
            Ok(content) => {
                let trimmed_content = content.trim();
                if !trimmed_content.is_empty() {
                    info!("Found server address file at: {:?}", path);
                    return Ok((path.clone(), trimmed_content.to_string()));
                } else {
                    warn!("Server address file is empty: {:?}", path);
                }
            }
            Err(e) => {
                debug!("Cannot read server address file at {:?}: {}", path, e);
            }
        }
    }

    Err(anyhow::anyhow!(
        "Server address file not found in any discovery location. \
        Make sure the server is running and has saved its address. \
        Discovery paths tried: {:?}",
        discovery_paths
            .iter()
            .map(|p| p.display().to_string())
            .collect::<Vec<_>>()
    ))
}

/// Validate the format of server address file content
/// Expected format: "nym_address;auth_key_base64"
/// Returns the parsed server address and auth key
#[allow(dead_code)]
pub fn validate_server_address_format(content: &str) -> Result<(String, String)> {
    let parts: Vec<&str> = content.split(';').collect();
    if parts.len() != 2 {
        return Err(anyhow::anyhow!(
            "Invalid server address file format. Expected 'nym_address;auth_key' format, got: {}",
            content.len().min(100) // Limit length for security
        ));
    }

    let server_address = parts[0].trim();
    let auth_key = parts[1].trim();

    if server_address.is_empty() || auth_key.is_empty() {
        return Err(anyhow::anyhow!(
            "Server address or authentication key is empty"
        ));
    }

    // Basic validation of Nym address format (should start with certain pattern)
    if !server_address.contains('.') {
        return Err(anyhow::anyhow!(
            "Invalid Nym address format: {}",
            server_address.chars().take(50).collect::<String>() // Limit for security
        ));
    }

    Ok((server_address.to_string(), auth_key.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::File;
    use std::io::Write;
    use tempfile::TempDir;

    #[test]
    fn test_server_data_dir_creation() {
        let result = get_server_data_dir();
        assert!(result.is_ok());
        let path = result.unwrap();
        assert!(path.exists());
        assert!(path.is_dir());
    }

    #[test]
    fn test_discovery_paths_generation() {
        let paths = get_server_address_discovery_paths();
        assert!(!paths.is_empty());

        // Should include current directory
        assert!(paths
            .iter()
            .any(|p| p.file_name() == Some(std::ffi::OsStr::new(SERVER_ADDRESS_FILENAME))));
    }

    #[test]
    fn test_validate_server_address_format() {
        // Valid format
        let valid = "test.address.nym;YWJjZGVmZ2hpams=";
        let result = validate_server_address_format(valid);
        assert!(result.is_ok());
        let (addr, key) = result.unwrap();
        assert_eq!(addr, "test.address.nym");
        assert_eq!(key, "YWJjZGVmZ2hpams=");

        // Invalid formats
        assert!(validate_server_address_format("no_semicolon").is_err());
        assert!(validate_server_address_format("too;many;semicolons").is_err());
        assert!(validate_server_address_format(";empty_address").is_err());
        assert!(validate_server_address_format("address;").is_err());
        assert!(validate_server_address_format("invalid_address;key").is_err());
    }

    #[test]
    fn test_discover_server_address_file() {
        let temp_dir = TempDir::new().unwrap();
        let test_file = temp_dir.path().join(SERVER_ADDRESS_FILENAME);

        // Create a test server address file
        let mut file = File::create(&test_file).unwrap();
        writeln!(file, "test.address.nym;dGVzdF9hdXRoX2tleQ==").unwrap();

        // Set environment variable to point to our test file
        env::set_var(SERVER_ADDRESS_ENV_VAR, &test_file);

        let result = discover_server_address_file();
        assert!(result.is_ok());

        let (found_path, content) = result.unwrap();
        assert_eq!(found_path, test_file);
        assert!(content.contains("test.address.nym"));

        // Clean up
        env::remove_var(SERVER_ADDRESS_ENV_VAR);
    }

    #[test]
    fn test_environment_variable_override() {
        let custom_path = "/tmp/custom_nymquest_address.txt";
        env::set_var(SERVER_ADDRESS_ENV_VAR, custom_path);

        let paths = get_server_address_discovery_paths();
        assert_eq!(paths[0], PathBuf::from(custom_path));

        // Clean up
        env::remove_var(SERVER_ADDRESS_ENV_VAR);
    }
}
