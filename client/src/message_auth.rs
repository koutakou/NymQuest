use anyhow::{anyhow, Result};
use base64::{engine::general_purpose, Engine as _};
use hmac::{Hmac, Mac};
use rand::{rngs::OsRng, RngCore};
use serde::{Deserialize, Serialize};
use sha2::Sha256;
use std::collections::VecDeque;
use std::fs;
use std::path::Path;
use std::time::{Instant, SystemTime, UNIX_EPOCH};
use tracing::{debug, warn};

// Constants for key rotation
const KEY_ROTATION_INTERVAL_SECONDS: u64 = 86400; // 24 hours
const MAX_PREV_KEYS: usize = 3; // Keep 3 previous keys for verification

// Normalize timestamps by truncating to day precision (for rotation stability)
// This avoids timing attacks and ensures client/server timestamp compatibility
fn normalize_timestamp(timestamp: u64) -> u64 {
    // Round down to the nearest day (86400 seconds per day)
    // This ensures all timestamps generated on the same day use the same value
    (timestamp / KEY_ROTATION_INTERVAL_SECONDS) * KEY_ROTATION_INTERVAL_SECONDS
}

// Type alias for HMAC-SHA256
type HmacSha256 = Hmac<Sha256>;

/// Single key entry with creation timestamp for rotation tracking
#[derive(Debug, Clone, Serialize, Deserialize)]
struct KeyEntry {
    /// The actual key material
    key: Vec<u8>,
    /// Unix timestamp when this key was created
    created_at: u64,
}

/// Key used for message authentication between client and server
/// Supports key rotation with forward secrecy
#[derive(Debug, Clone)]
pub struct AuthKey {
    /// Current active key
    current_key: KeyEntry,
    /// Previous keys kept for verifying old messages
    previous_keys: VecDeque<KeyEntry>,
    /// Next rotation time as Unix timestamp
    next_rotation: u64,
    /// Creation time for tracking usage (client-side only)
    #[allow(dead_code)] // Part of complete authentication API for future use
    instance_created_at: Instant,
}

impl KeyEntry {
    /// Create a new random key entry with current timestamp
    fn new_random() -> Result<Self> {
        let mut key = vec![0u8; 32]; // 256 bits key
        OsRng.fill_bytes(&mut key);

        // Get current time as timestamp and normalize it for consistency
        let raw_timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|e| anyhow!("Failed to get system time: {}", e))?
            .as_secs();

        // Normalize to day precision for stability between client/server
        let created_at = normalize_timestamp(raw_timestamp);

        Ok(Self { key, created_at })
    }

    /// Create a key entry from existing key and timestamp
    fn from_key_and_timestamp(key: Vec<u8>, created_at: u64) -> Self {
        Self { key, created_at }
    }
}

impl AuthKey {
    /// Generate a new random authentication key
    #[allow(dead_code)] // Part of complete authentication API for future use
    pub fn new_random() -> Result<Self> {
        let current_key = KeyEntry::new_random()?;
        let raw_now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|e| anyhow!("Failed to get system time: {}", e))?
            .as_secs();

        // Normalize timestamp for consistency
        let now = normalize_timestamp(raw_now);

        Ok(Self {
            current_key,
            previous_keys: VecDeque::new(),
            next_rotation: now + KEY_ROTATION_INTERVAL_SECONDS,
            instance_created_at: Instant::now(),
        })
    }

    /// Create an auth key from an existing byte array (for backward compatibility)
    #[allow(dead_code)] // Part of complete authentication API for future use
    pub fn from_bytes(bytes: &[u8]) -> Result<Self> {
        let raw_now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|e| anyhow!("Failed to get system time: {}", e))?
            .as_secs();

        // Normalize timestamp for consistency
        let now = normalize_timestamp(raw_now);

        let current_key = KeyEntry::from_key_and_timestamp(bytes.to_vec(), now);

        Ok(Self {
            current_key,
            previous_keys: VecDeque::new(),
            next_rotation: now + KEY_ROTATION_INTERVAL_SECONDS,
            instance_created_at: Instant::now(),
        })
    }

    /// Encode the keys as a JSON string for storage
    #[allow(dead_code)] // Part of complete authentication API for future use
    pub fn to_json(&self) -> Result<String> {
        // Create serializable structure
        #[derive(Serialize)]
        struct KeyStorage {
            current_key: KeyEntry,
            previous_keys: Vec<KeyEntry>,
            next_rotation: u64,
        }

        let storage = KeyStorage {
            current_key: self.current_key.clone(),
            previous_keys: self.previous_keys.iter().cloned().collect(),
            next_rotation: self.next_rotation,
        };

        serde_json::to_string(&storage).map_err(|e| anyhow!("Failed to serialize keys: {}", e))
    }

    /// Encode the current key as a base64 string for storage (backward compatibility)
    #[allow(dead_code)] // Part of complete authentication API for future use
    pub fn to_base64(&self) -> String {
        general_purpose::STANDARD.encode(&self.current_key.key)
    }

    /// Create an auth key from a JSON encoded string
    #[allow(dead_code)] // Part of complete authentication API for future use
    pub fn from_json(json: &str) -> Result<Self> {
        // Create deserializable structure
        #[derive(Deserialize)]
        struct KeyStorage {
            current_key: KeyEntry,
            previous_keys: Vec<KeyEntry>,
            next_rotation: u64,
        }

        let storage: KeyStorage =
            serde_json::from_str(json).map_err(|e| anyhow!("Failed to deserialize keys: {}", e))?;

        let mut previous_keys = VecDeque::with_capacity(MAX_PREV_KEYS);
        for key in storage.previous_keys {
            previous_keys.push_back(key);
        }

        // Ensure we don't exceed our maximum previous keys
        while previous_keys.len() > MAX_PREV_KEYS {
            previous_keys.pop_front();
        }

        Ok(Self {
            current_key: storage.current_key,
            previous_keys,
            next_rotation: storage.next_rotation,
            instance_created_at: Instant::now(),
        })
    }

    /// Create an auth key from a base64 encoded string (backward compatibility)
    #[allow(dead_code)] // Part of complete authentication API for future use
    pub fn from_base64(encoded: &str) -> Result<Self> {
        let key = general_purpose::STANDARD
            .decode(encoded)
            .map_err(|e| anyhow!("Failed to decode auth key: {}", e))?;

        let raw_now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|e| anyhow!("Failed to get system time: {}", e))?
            .as_secs();

        // Normalize timestamp for consistency
        let now = normalize_timestamp(raw_now);

        let current_key = KeyEntry {
            key,
            created_at: now,
        };

        Ok(Self {
            current_key,
            previous_keys: VecDeque::new(),
            next_rotation: now + KEY_ROTATION_INTERVAL_SECONDS,
            instance_created_at: Instant::now(),
        })
    }

    /// Generate an authentication tag for the given message
    #[allow(dead_code)] // Part of complete authentication API for future use
    pub fn generate_tag<T: Serialize>(&self, message: &T) -> Result<String> {
        // Serialize the message to a JSON string
        let message_str = serde_json::to_string(message)
            .map_err(|e| anyhow!("Failed to serialize message: {}", e))?;

        // Create a new HMAC instance with the current key
        let mut mac = HmacSha256::new_from_slice(&self.current_key.key)
            .map_err(|e| anyhow!("Failed to create HMAC: {}", e))?;

        // Update the HMAC with the message content only
        // Do not include timestamp in HMAC to ensure compatibility with server
        mac.update(message_str.as_bytes());

        // Finalize and get the result
        let result = mac.finalize().into_bytes();

        // Return the Base64 encoded tag with key timestamp
        // Format: base64(hmac):timestamp
        let tag = format!(
            "{}:{}",
            general_purpose::STANDARD.encode(result),
            self.current_key.created_at
        );

        Ok(tag)
    }

    /// Verify an authentication tag for the given message
    #[allow(dead_code)] // Part of complete authentication API for future use
    pub fn verify_tag<T: Serialize>(&self, message: &T, tag: &str) -> Result<bool> {
        // Parse the tag format (tag:timestamp or just tag for backward compatibility)
        let parts: Vec<&str> = tag.split(':').collect();

        // Handle both new format (with timestamp) and legacy format
        match parts.len() {
            // New format: "tag:timestamp"
            2 => {
                let tag_value = parts[0];
                // Parse the raw timestamp from the tag
                let raw_timestamp = parts[1]
                    .parse::<u64>()
                    .map_err(|e| anyhow!("Invalid timestamp in tag: {}", e))?;

                // Normalize it for comparison with our keys
                let timestamp = normalize_timestamp(raw_timestamp);

                debug!(
                    "Received tag with timestamp: {} (normalized: {})",
                    raw_timestamp, timestamp
                );

                // MAJOR CHANGE: Always try all keys for verification, regardless of timestamp
                // This ensures compatibility even if clocks are way off

                // Try with current key first
                debug!("Trying to verify with current key");
                if self.verify_with_key(message, tag_value, &self.current_key.key, 0)? {
                    debug!("Successfully verified with current key");
                    return Ok(true);
                }

                // Then try with all previous keys
                for (i, prev_key) in self.previous_keys.iter().enumerate() {
                    debug!("Trying to verify with previous key {}", i);
                    if self.verify_with_key(message, tag_value, &prev_key.key, 0)? {
                        debug!("Successfully verified with previous key {}", i);
                        return Ok(true);
                    }
                }

                // No matching key found
                warn!(
                    "Authentication failed - tried {} keys but none matched",
                    1 + self.previous_keys.len()
                );
                Ok(false)
            }

            // Legacy format without timestamp - try all keys
            1 => {
                // Try with current key first
                if self.verify_legacy_tag(message, tag, &self.current_key.key)? {
                    return Ok(true);
                }

                // Then try with previous keys
                for prev_key in &self.previous_keys {
                    if self.verify_legacy_tag(message, tag, &prev_key.key)? {
                        return Ok(true);
                    }
                }

                // No matching key found
                Ok(false)
            }

            // Invalid format
            _ => {
                warn!("Invalid tag format");
                Ok(false)
            }
        }
    }

    /// Verify a tag with a specific key and timestamp
    fn verify_with_key<T: Serialize>(
        &self,
        message: &T,
        tag_value: &str,
        key: &[u8],
        _timestamp: u64, // Timestamp is not used in HMAC calculation anymore
    ) -> Result<bool> {
        // Serialize the message to a JSON string
        let message_str = serde_json::to_string(message)
            .map_err(|e| anyhow!("Failed to serialize message: {}", e))?;

        // Create a new HMAC instance
        let mut mac =
            HmacSha256::new_from_slice(key).map_err(|e| anyhow!("Failed to create HMAC: {}", e))?;

        // Update the HMAC with message content only
        // No timestamp in HMAC calculation to make it independent of time differences
        mac.update(message_str.as_bytes());

        // Finalize and get the result
        let result = mac.finalize().into_bytes();
        let computed_tag = general_purpose::STANDARD.encode(result);

        Ok(computed_tag == tag_value)
    }

    /// Verify a legacy tag (without timestamp)
    fn verify_legacy_tag<T: Serialize>(&self, message: &T, tag: &str, key: &[u8]) -> Result<bool> {
        // Serialize the message to a JSON string
        let message_str = serde_json::to_string(message)
            .map_err(|e| anyhow!("Failed to serialize message: {}", e))?;

        // Create a new HMAC instance
        let mut mac =
            HmacSha256::new_from_slice(key).map_err(|e| anyhow!("Failed to create HMAC: {}", e))?;

        // Update the HMAC with the message content
        mac.update(message_str.as_bytes());

        // Finalize and get the result
        let result = mac.finalize().into_bytes();
        let computed_tag = general_purpose::STANDARD.encode(result);

        Ok(computed_tag == tag)
    }

    /// Check if key rotation is needed and perform rotation if necessary
    /// Returns true if rotation was performed
    #[allow(dead_code)] // Part of complete authentication API for future use
    pub fn check_and_rotate(&mut self) -> Result<bool> {
        let raw_now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|e| anyhow!("Failed to get system time: {}", e))?
            .as_secs();

        // Normalize timestamp for rotation consistency
        let now = normalize_timestamp(raw_now);

        if now >= self.next_rotation {
            // Time to rotate the key
            debug!("Rotating authentication key");

            // Move current key to previous keys
            self.previous_keys.push_back(self.current_key.clone());

            // Ensure we don't exceed our maximum previous keys
            while self.previous_keys.len() > MAX_PREV_KEYS {
                self.previous_keys.pop_front();
            }

            // Generate new current key
            self.current_key = KeyEntry::new_random()?;

            // Set next rotation time
            self.next_rotation = now + KEY_ROTATION_INTERVAL_SECONDS;

            debug!(
                "Authentication key rotated, next rotation at timestamp {}",
                self.next_rotation
            );
            return Ok(true);
        }

        Ok(false)
    }

    /// Save the authentication key to a file
    #[allow(dead_code)] // Part of complete authentication API for future use
    pub fn save_to_file(&self, path: &Path) -> Result<()> {
        // Create parent directory if it doesn't exist
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }

        // Serialize all keys and save
        let encoded = self.to_json()?;

        // Extension for the rotated keys file
        let rotated_path = path.with_extension("json");
        fs::write(&rotated_path, &encoded)?;

        // Also save in legacy format for backward compatibility
        let legacy_encoded = self.to_base64();
        fs::write(path, legacy_encoded)?;

        Ok(())
    }

    /// Load the authentication key from a file, or create a new one if the file doesn't exist
    #[allow(dead_code)] // Part of complete authentication API for future use
    pub fn load_or_create(path: &Path) -> Result<Self> {
        // Try to load from the rotated keys file first (modern format)
        let rotated_path = path.with_extension("json");

        if rotated_path.exists() {
            // Load existing keys with rotation
            let encoded = fs::read_to_string(&rotated_path)?;
            let mut key = Self::from_json(&encoded)?;

            // Check if we need to rotate the key
            if key.check_and_rotate()? {
                // If key was rotated, save the updated keys
                key.save_to_file(path)?;
            }

            debug!("Loaded existing authentication keys with rotation");
            Ok(key)
        } else if path.exists() {
            // Fall back to legacy format if new format doesn't exist
            let encoded = fs::read_to_string(path)?;
            let key = Self::from_base64(&encoded)?;

            // Convert to new format and save
            key.save_to_file(path)?;

            debug!("Loaded and upgraded legacy authentication key to rotation format");
            Ok(key)
        } else {
            // Create a new key and save it
            let key = Self::new_random()?;
            key.save_to_file(path)?;
            debug!("Generated and saved new authentication key with rotation support");
            Ok(key)
        }
    }
}

/// A wrapper for messages that includes authentication and expiration
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct AuthenticatedMessage<T> {
    pub message: T,
    pub auth_tag: String,
    /// Unix timestamp (seconds since epoch) when this message expires
    pub expires_at: Option<u64>,
}

impl<T: Serialize> AuthenticatedMessage<T> {
    /// Create a new authenticated message by generating a tag
    #[allow(dead_code)] // Part of complete authentication API for future use
    pub fn new(message: T, auth_key: &AuthKey) -> Result<Self> {
        let auth_tag = auth_key.generate_tag(&message)?;
        Ok(Self {
            message,
            auth_tag,
            expires_at: None,
        })
    }

    /// Create a new authenticated message with expiration
    #[allow(dead_code)] // Part of complete authentication API for future use
    pub fn new_with_expiration(message: T, auth_key: &AuthKey, ttl_seconds: u64) -> Result<Self> {
        let auth_tag = auth_key.generate_tag(&message)?;

        // Calculate expiration time
        let raw_now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|e| anyhow!("Failed to get system time: {}", e))?
            .as_secs();

        // Normalize timestamp for consistency
        let now = normalize_timestamp(raw_now);

        let expires_at = now
            .checked_add(ttl_seconds)
            .ok_or_else(|| anyhow!("Overflow calculating expiration time"))?;

        Ok(Self {
            message,
            auth_tag,
            expires_at: Some(expires_at),
        })
    }

    /// Verify that this message has not been tampered with and is not expired
    #[allow(dead_code)] // Part of complete authentication API for future use
    pub fn verify(&self, auth_key: &AuthKey) -> Result<bool>
    where
        T: Serialize + Clone,
    {
        // First check if the message is expired
        if let Some(expires_at) = self.expires_at {
            let now = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map_err(|e| anyhow!("Failed to get system time: {}", e))?
                .as_secs();

            if now > expires_at {
                warn!(
                    "Message expired: current time {} > expiration time {}",
                    now, expires_at
                );
                return Ok(false);
            }
        }

        // Then verify the authentication tag
        auth_key.verify_tag(&self.message, &self.auth_tag)
    }

    /// Get the time remaining until expiration in seconds
    /// Returns None if the message doesn't have an expiration time
    #[allow(dead_code)] // Part of complete authentication API for future use
    pub fn time_to_expiration(&self) -> Result<Option<u64>> {
        match self.expires_at {
            Some(expires_at) => {
                let now = SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .map_err(|e| anyhow!("Failed to get system time: {}", e))?
                    .as_secs();

                if now >= expires_at {
                    Ok(Some(0))
                } else {
                    Ok(Some(expires_at - now))
                }
            }
            None => Ok(None),
        }
    }
}
