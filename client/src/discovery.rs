use anyhow::{Context, Result};
use std::env;
use std::fs;
use std::path::PathBuf;
use tracing::{debug, info, warn};

/// Module for server address discovery mechanisms on the client side
/// This module provides cross-platform functionality for discovering server addresses
/// saved by the server, enabling automatic connection without hardcoded paths
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
    debug!("Looking for server data directory: {:?}", nymquest_dir);
    Ok(nymquest_dir)
}

/// Get all possible locations where the client should look for server address file
/// Returns paths in order of preference (most specific to most general)
pub fn get_server_address_discovery_paths() -> Vec<PathBuf> {
    let mut paths = Vec::new();

    // 1. Environment variable override (highest priority)
    if let Ok(custom_path) = env::var(SERVER_ADDRESS_ENV_VAR) {
        debug!("Added custom path from environment: {}", custom_path);
        paths.push(PathBuf::from(custom_path));
    }

    // 2. Platform-specific data directory (preferred standard location)
    if let Ok(data_dir) = get_server_data_dir() {
        paths.push(data_dir.join(SERVER_ADDRESS_FILENAME));
    }

    // 3. Current working directory (for development/testing)
    paths.push(PathBuf::from(SERVER_ADDRESS_FILENAME));
    paths.push(PathBuf::from("server_address.txt")); // Legacy name for compatibility

    // 4. Legacy relative paths (for backward compatibility)
    paths.push(PathBuf::from("../client/server_address.txt"));
    paths.push(PathBuf::from("../server_address.txt"));

    // 5. Home directory fallback
    if let Some(home_dir) = dirs_next::data_dir().or_else(dirs_next::home_dir) {
        paths.push(home_dir.join(".nymquest").join(SERVER_ADDRESS_FILENAME));
        paths.push(home_dir.join("server_address.txt"));
    }

    debug!("Generated {} discovery paths", paths.len());
    paths
}

/// Find and read the server address file from discovery paths
/// Returns the first valid file found along with its path and content
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
        \n\nDiscovery paths tried:\n{}\n\
        \nTo specify a custom location, set the environment variable:\n\
        export {}=/path/to/your/server/address/file",
        discovery_paths
            .iter()
            .map(|p| format!("  - {}", p.display()))
            .collect::<Vec<_>>()
            .join("\n"),
        SERVER_ADDRESS_ENV_VAR
    ))
}

/// Validate server address file format
/// Expected format: "nym_address;base64_auth_key"
pub fn validate_server_address_format(content: &str) -> Result<(String, String)> {
    let parts: Vec<&str> = content.split(';').collect();
    if parts.len() != 2 {
        return Err(anyhow::anyhow!(
            "Invalid server address file format. Expected 'nym_address;auth_key' format, got {} parts", 
            parts.len()
        ));
    }

    let server_address = parts[0].trim();
    let auth_key = parts[1].trim();

    if server_address.is_empty() || auth_key.is_empty() {
        return Err(anyhow::anyhow!(
            "Server address or authentication key is empty"
        ));
    }

    // Basic validation of Nym address format (should contain dots for proper addressing)
    if !server_address.contains('.') {
        return Err(anyhow::anyhow!(
            "Invalid Nym address format: missing domain separators"
        ));
    }

    Ok((server_address.to_string(), auth_key.to_string()))
}

/// Load server connection information using the discovery mechanism
/// Returns the server address and authentication key
pub fn load_server_connection_info() -> Result<(String, String)> {
    let (path, content) =
        discover_server_address_file().with_context(|| "Failed to discover server address file")?;

    let (server_address, auth_key_b64) = validate_server_address_format(&content)
        .with_context(|| format!("Invalid server address file format at {:?}", path))?;

    info!(
        "Successfully loaded server connection info from: {:?}",
        path
    );
    Ok((server_address, auth_key_b64))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::File;
    use std::io::Write;
    use tempfile::TempDir;

    #[test]
    fn test_discovery_paths_generation() {
        let paths = get_server_address_discovery_paths();
        assert!(!paths.is_empty());

        // Should include current directory
        assert!(paths
            .iter()
            .any(|p| p.file_name() == Some(std::ffi::OsStr::new(SERVER_ADDRESS_FILENAME))));

        // Should include legacy compatibility
        assert!(paths
            .iter()
            .any(|p| p.file_name() == Some(std::ffi::OsStr::new("server_address.txt"))));
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
    fn test_load_server_connection_info() {
        let temp_dir = TempDir::new().unwrap();
        let test_file = temp_dir.path().join(SERVER_ADDRESS_FILENAME);

        // Create a test server address file with a valid Nym address format
        let mut file = File::create(&test_file).unwrap();
        writeln!(file, "server.nym.test;dGVzdF9hdXRoX2tleQ==").unwrap();
        // Flush to ensure the content is written
        file.flush().unwrap();
        drop(file); // Explicitly close the file

        // Convert to canonical path to avoid any path resolution issues
        let canonical_path = test_file.canonicalize().unwrap();

        // Set environment variable to point to our test file using canonical path
        env::set_var(
            SERVER_ADDRESS_ENV_VAR,
            canonical_path.to_string_lossy().to_string(),
        );

        // Verify the file exists and has content
        assert!(
            canonical_path.exists(),
            "Test file does not exist at: {:?}",
            canonical_path
        );
        let content = fs::read_to_string(&canonical_path).unwrap();
        assert!(!content.trim().is_empty(), "Test file is empty");

        let result = load_server_connection_info();
        if let Err(ref e) = result {
            eprintln!("Test error: {}", e);
        }
        assert!(result.is_ok());

        let (address, key) = result.unwrap();
        assert_eq!(address, "server.nym.test");
        assert_eq!(key, "dGVzdF9hdXRoX2tleQ==");

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
