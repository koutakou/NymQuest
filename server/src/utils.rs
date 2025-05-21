use std::fs::File;
use std::io::Write;
use std::path::Path;
use anyhow::{Result, Context};

/// Save the server address to a file that the client can read
pub fn save_server_address(address: &str, file_path: &str) -> Result<()> {
    let path = Path::new(file_path);
    let mut file = File::create(path)
        .with_context(|| format!("Failed to create file at {}", file_path))?;
    
    writeln!(file, "{}", address)
        .with_context(|| "Failed to write server address to file")?;
    
    println!("Server address saved to {}", file_path);
    Ok(())
}

/// Get current timestamp in seconds
pub fn current_timestamp() -> u64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}
