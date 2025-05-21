use hmac::{Hmac, Mac};
use sha2::Sha256;
use anyhow::{Result, anyhow};
use serde::{Serialize, Deserialize};
use rand::{RngCore, rngs::OsRng};
use base64::{Engine as _, engine::general_purpose};

// Type alias for HMAC-SHA256
type HmacSha256 = Hmac<Sha256>;

/// Key used for message authentication between client and server
#[derive(Clone, Debug)]
pub struct AuthKey {
    key: Vec<u8>,
}

impl AuthKey {
    /// Generate a new random authentication key
    pub fn new_random() -> Self {
        let mut key = vec![0u8; 32]; // 256 bits key
        OsRng.fill_bytes(&mut key);
        Self { key }
    }

    /// Create an auth key from an existing byte array
    pub fn from_bytes(bytes: &[u8]) -> Self {
        Self { key: bytes.to_vec() }
    }

    /// Encode the key as a base64 string for storage
    pub fn to_base64(&self) -> String {
        general_purpose::STANDARD.encode(&self.key)
    }

    /// Create an auth key from a base64 encoded string
    pub fn from_base64(encoded: &str) -> Result<Self> {
        let key = general_purpose::STANDARD.decode(encoded)
            .map_err(|e| anyhow!("Failed to decode auth key: {}", e))?;
        Ok(Self { key })
    }

    /// Generate an authentication tag for the given message
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
    pub fn verify_tag<T: Serialize>(&self, message: &T, tag: &str) -> Result<bool> {
        let expected_tag = self.generate_tag(message)?;
        Ok(expected_tag == tag)
    }
}

/// A wrapper for messages that includes authentication
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct AuthenticatedMessage<T> {
    pub message: T,
    pub auth_tag: String,
}

impl<T: Serialize> AuthenticatedMessage<T> {
    /// Create a new authenticated message by generating a tag
    pub fn new(message: T, auth_key: &AuthKey) -> Result<Self> {
        let auth_tag = auth_key.generate_tag(&message)?;
        Ok(Self { message, auth_tag })
    }

    /// Verify that this message has not been tampered with
    pub fn verify(&self, auth_key: &AuthKey) -> Result<bool>
    where T: Serialize + Clone {
        auth_key.verify_tag(&self.message, &self.auth_tag)
    }
}
