use anyhow::{anyhow, Result};
use base64::{engine::general_purpose, Engine as _};
use hmac::{Hmac, Mac};
use rand::{rngs::OsRng, RngCore};
use serde::{Deserialize, Serialize};
use sha2::Sha256;
use std::time::{Instant, SystemTime, UNIX_EPOCH};
use tracing::warn;

// Type alias for HMAC-SHA256
type HmacSha256 = Hmac<Sha256>;

/// Key used for message authentication between client and server
#[derive(Debug, Clone)]
pub struct AuthKey {
    key: Vec<u8>,
    #[allow(dead_code)] // Part of complete authentication API for future use
    created_at: Instant,
}

impl AuthKey {
    /// Generate a new random authentication key
    #[allow(dead_code)] // Part of complete authentication API for future use
    pub fn new_random() -> Self {
        let mut key = vec![0u8; 32]; // 256 bits key
        OsRng.fill_bytes(&mut key);
        Self {
            key,
            created_at: Instant::now(),
        }
    }

    /// Create an auth key from an existing byte array
    #[allow(dead_code)] // Part of complete authentication API for future use
    pub fn from_bytes(bytes: &[u8]) -> Self {
        Self {
            key: bytes.to_vec(),
            created_at: Instant::now(),
        }
    }

    /// Encode the key as a base64 string for storage
    #[allow(dead_code)] // Part of complete authentication API for future use
    pub fn to_base64(&self) -> String {
        general_purpose::STANDARD.encode(&self.key)
    }

    /// Create an auth key from a base64 encoded string
    #[allow(dead_code)] // Part of complete authentication API for future use
    pub fn from_base64(encoded: &str) -> Result<Self> {
        let key = general_purpose::STANDARD
            .decode(encoded)
            .map_err(|e| anyhow!("Failed to decode auth key: {}", e))?;
        Ok(Self {
            key,
            created_at: Instant::now(),
        })
    }

    /// Generate an authentication tag for the given message
    #[allow(dead_code)] // Part of complete authentication API for future use
    pub fn generate_tag<T: Serialize>(&self, message: &T) -> Result<String> {
        // Serialize the message to a JSON string
        let message_str = serde_json::to_string(message)
            .map_err(|e| anyhow!("Failed to serialize message: {}", e))?;

        // Create a new HMAC instance
        let mut mac = HmacSha256::new_from_slice(&self.key)
            .map_err(|e| anyhow!("Failed to create HMAC: {}", e))?;

        // Update the HMAC with the message content
        mac.update(message_str.as_bytes());

        // Finalize and get the result
        let result = mac.finalize().into_bytes();

        // Return the Base64 encoded tag
        Ok(general_purpose::STANDARD.encode(result))
    }

    /// Verify an authentication tag for the given message
    #[allow(dead_code)] // Part of complete authentication API for future use
    pub fn verify_tag<T: Serialize>(&self, message: &T, tag: &str) -> Result<bool> {
        let expected_tag = self.generate_tag(message)?;
        Ok(expected_tag == tag)
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
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|e| anyhow!("Failed to get system time: {}", e))?
            .as_secs();

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
