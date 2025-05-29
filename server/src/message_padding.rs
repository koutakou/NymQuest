use serde::{Deserialize, Serialize};
use tracing::debug;

/// Standard message size buckets in bytes for padding
/// Each message will be padded to the nearest bucket size above its actual size
const MESSAGE_SIZE_BUCKETS: [usize; 6] = [128, 256, 512, 1024, 2048, 4096];

/// Maximum allowed message size before rejecting
const MAX_ALLOWED_MESSAGE_SIZE: usize = 4096;

/// Structure to wrap any message in padding for enhanced privacy
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PaddedMessage<T> {
    /// The actual message content
    pub message: T,
    /// Random padding bytes to normalize message size
    #[serde(with = "serde_bytes")]
    pub padding: Vec<u8>,
}

impl<T> PaddedMessage<T> {
    /// Create a new padded message, normalizing its size to prevent correlation attacks
    pub fn new(message: T, serialized_message: &[u8]) -> Self {
        let target_size = get_target_size(serialized_message.len());
        let padding_size = target_size.saturating_sub(serialized_message.len());

        // Generate random padding bytes
        let padding = if padding_size > 0 {
            let mut padding = vec![0u8; padding_size];
            // Fill with random data (we don't need cryptographic randomness here)
            for byte in padding.iter_mut() {
                *byte = rand::random::<u8>();
            }
            padding
        } else {
            Vec::new()
        };

        debug!(
            "Padded message from {} bytes to {} bytes (added {} padding bytes)",
            serialized_message.len(),
            serialized_message.len() + padding.len(),
            padding.len()
        );

        Self { message, padding }
    }

    /// Extracts the message, discarding padding
    pub fn into_inner(self) -> T {
        self.message
    }
}

/// Determine the appropriate target size for a message
fn get_target_size(actual_size: usize) -> usize {
    // If message exceeds max allowed size, just return actual size
    // (this will be caught and rejected later in processing)
    if actual_size > MAX_ALLOWED_MESSAGE_SIZE {
        return actual_size;
    }

    // Find the smallest bucket size that fits the message
    for bucket_size in &MESSAGE_SIZE_BUCKETS {
        if *bucket_size >= actual_size {
            return *bucket_size;
        }
    }

    // If it's larger than our biggest bucket, use the maximum allowed size
    MAX_ALLOWED_MESSAGE_SIZE
}

/// Pad a message to a standard size bucket to prevent size correlation attacks
pub fn pad_message<T: Serialize>(message: T) -> Result<PaddedMessage<T>, serde_json::Error> {
    // Convert the message to JSON first to determine its size
    let serialized = serde_json::to_vec(&message)?;
    Ok(PaddedMessage::new(message, &serialized))
}

/// Extract the original message from a padded message
pub fn unpad_message<T>(padded: PaddedMessage<T>) -> T {
    padded.into_inner()
}
