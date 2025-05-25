use crate::message_auth::AuthKey;
use anyhow::{Context, Result};
use std::fs::File;
use std::io::Write;
use tracing::info;

/// Save the server address and authentication key using the cross-platform discovery mechanism
/// This replaces the previous hardcoded path approach with a robust, production-ready solution
pub fn save_server_address(address: &str, auth_key: &AuthKey) -> Result<()> {
    use crate::discovery;

    // Get the appropriate file path using the discovery mechanism
    let file_path = discovery::get_server_address_file_path()
        .with_context(|| "Failed to determine server address file path")?;

    // Ensure parent directory exists
    if let Some(parent) = file_path.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("Failed to create parent directory: {:?}", parent))?;
    }

    // Create and write to the file atomically
    let mut file = File::create(&file_path)
        .with_context(|| format!("Failed to create server address file at {:?}", file_path))?;

    // Write server address and authentication key in the standard format
    writeln!(file, "{};{}", address, auth_key.to_base64())
        .with_context(|| "Failed to write server address and auth key to file")?;

    // Ensure data is written to disk
    file.sync_all()
        .with_context(|| "Failed to sync server address file to disk")?;

    info!(
        "Server address and authentication key saved to: {:?}",
        file_path
    );

    // Log discovery information for debugging
    info!(
        "Clients can find this server using discovery paths or by setting {}=/path/to/file",
        discovery::SERVER_ADDRESS_ENV_VAR
    );

    Ok(())
}

/// Get current timestamp in seconds
#[allow(dead_code)]
pub fn current_timestamp() -> u64 {
    use std::time::{SystemTime, UNIX_EPOCH};

    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}
