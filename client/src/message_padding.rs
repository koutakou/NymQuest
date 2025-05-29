use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicUsize, Ordering};
use tracing::debug;

/// Base message size buckets in bytes for padding
/// Each message will be padded to the nearest bucket size above its actual size
/// These are the base sizes, actual padding sizes may vary slightly based on the dynamic jitter
const BASE_MESSAGE_SIZE_BUCKETS: [usize; 6] = [128, 256, 512, 1024, 2048, 4096];

/// Maximum allowed message size before rejecting
const MAX_ALLOWED_MESSAGE_SIZE: usize = 4096;

/// Maximum jitter percentage to apply to bucket sizes (5%)
const BUCKET_SIZE_JITTER_PERCENT: usize = 5;

/// Tracks the total number of messages processed for adaptive sizing
static MESSAGE_COUNT: AtomicUsize = AtomicUsize::new(0);

/// Rotation interval for bucket size jitter (every 100 messages)
const BUCKET_ROTATION_INTERVAL: usize = 100;

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

/// Determine the appropriate target size for a message with dynamic jitter
/// This adds small variations to the bucket sizes to prevent statistical analysis
fn get_target_size(actual_size: usize) -> usize {
    // Increment the message counter for rotation
    let message_number = MESSAGE_COUNT.fetch_add(1, Ordering::SeqCst);

    // If message exceeds max allowed size, just return actual size
    // (this will be caught and rejected later in processing)
    if actual_size > MAX_ALLOWED_MESSAGE_SIZE {
        return actual_size;
    }

    // Determine if we should apply jitter based on message count
    // This ensures the jitter changes periodically for better privacy
    let apply_jitter = message_number % BUCKET_ROTATION_INTERVAL == 0;

    // If we're at a rotation point, log it (but only occasionally to reduce log noise)
    if apply_jitter && message_number > 0 {
        debug!("Rotating message bucket sizes for enhanced privacy protection");
    }

    // Find the smallest bucket size that fits the message, with potential jitter
    for &base_bucket_size in &BASE_MESSAGE_SIZE_BUCKETS {
        if base_bucket_size >= actual_size {
            // Apply jitter to the bucket size if needed
            if apply_jitter {
                // Create a deterministic but varying jitter based on message_number
                // This ensures the jitter is different for each rotation interval
                let jitter_seed = message_number / BUCKET_ROTATION_INTERVAL;
                let jitter_factor = (((jitter_seed * 17) + 13) % 100) as f64 / 100.0;

                // Calculate jitter amount (up to BUCKET_SIZE_JITTER_PERCENT % of bucket size)
                let max_jitter = (base_bucket_size * BUCKET_SIZE_JITTER_PERCENT) / 100;
                let jitter_amount = (max_jitter as f64 * jitter_factor) as usize;

                // Apply jitter to increase bucket size (never decrease for security)
                let jittered_size = base_bucket_size + jitter_amount;
                debug!(
                    "Applied {}% jitter to bucket size {} → {}",
                    (jitter_amount * 100) / base_bucket_size,
                    base_bucket_size,
                    jittered_size
                );

                return jittered_size;
            }

            return base_bucket_size;
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
