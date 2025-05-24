use crate::message_auth::AuthKey;
use anyhow::{Context, Result};
use std::fs::File;
use std::io::Write;
use std::path::Path;

/// Save the server address and authentication key to a file that the client can read
pub fn save_server_address(address: &str, auth_key: &AuthKey, file_path: &str) -> Result<()> {
    let path = Path::new(file_path);
    let mut file =
        File::create(path).with_context(|| format!("Failed to create file at {}", file_path))?;

    // Write server address and authentication key in base64 format
    writeln!(file, "{};{}", address, auth_key.to_base64())
        .with_context(|| "Failed to write server address to file")?;

    println!(
        "Server address and authentication key saved to {}",
        file_path
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
