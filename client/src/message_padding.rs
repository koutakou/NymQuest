use anyhow::Result;
use rand::{rngs::ThreadRng, Rng};
use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};
use tracing::{debug, trace};

/// Base message size buckets in bytes for padding
/// Each message will be padded to the nearest bucket size above its actual size
/// These are the base sizes, actual padding sizes may vary based on the adaptive jitter
const BASE_MESSAGE_SIZE_BUCKETS: [usize; 6] = [128, 256, 512, 1024, 2048, 4096];

/// Maximum allowed message size before rejecting
const MAX_ALLOWED_MESSAGE_SIZE: usize = 4096;

/// Minimum jitter percentage to apply to bucket sizes (2%)
const MIN_BUCKET_SIZE_JITTER_PERCENT: usize = 2;

/// Maximum jitter percentage to apply to bucket sizes (8%)
const MAX_BUCKET_SIZE_JITTER_PERCENT: usize = 8;

/// Tracks the total number of messages processed for adaptive sizing
static MESSAGE_COUNT: AtomicUsize = AtomicUsize::new(0);

/// Rotation interval for jitter strategy (every 50-150 messages)
/// The exact interval varies to prevent predictable patterns
static JITTER_ROTATION_INTERVAL: AtomicUsize = AtomicUsize::new(100);

/// Track last time jitter was rotated for time-based entropy
static mut LAST_JITTER_ROTATION: Option<u64> = None;

/// Entropy sources for jitter calculation
#[derive(Debug, Clone, Copy)]
enum EntropySource {
    /// Message count based entropy
    MessageCount,
    /// Time based entropy
    TimeOfDay,
    /// Combined entropy sources
    Combined,
    /// Random entropy
    Random,
}

/// Current entropy source (rotated periodically)
static mut CURRENT_ENTROPY_SOURCE: EntropySource = EntropySource::Combined;

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
            // Use ThreadRng for better randomness while keeping performance
            let mut rng = rand::thread_rng();
            rng.fill(padding.as_mut_slice());
            padding
        } else {
            Vec::new()
        };

        // Log padding details for debugging
        if serialized_message.len() < 1024 {
            // Detailed logging for smaller messages
            debug!(
                "Padded message from {} bytes to {} bytes (added {} padding bytes)",
                serialized_message.len(),
                serialized_message.len() + padding.len(),
                padding.len()
            );
        } else {
            // Less detailed logging for larger messages to reduce log volume
            trace!(
                "Padded large message ({}KB) to {}KB",
                serialized_message.len() / 1024,
                (serialized_message.len() + padding.len()) / 1024
            );
        }

        Self { message, padding }
    }

    /// Extracts the message, discarding padding
    pub fn into_inner(self) -> T {
        self.message
    }
}

/// Get current unix timestamp in milliseconds for entropy source
fn get_unix_time_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

/// Rotate the jitter strategy periodically to prevent pattern analysis
fn maybe_rotate_jitter_strategy() {
    let message_number = MESSAGE_COUNT.load(Ordering::SeqCst);
    let rotation_interval = JITTER_ROTATION_INTERVAL.load(Ordering::SeqCst);

    // Check if we need to rotate based on message count
    if message_number % rotation_interval == 0 && message_number > 0 {
        // Get current time for entropy and tracking
        let now = get_unix_time_ms();

        // Thread-safe random number generator
        let mut rng = rand::thread_rng();

        // Select a new entropy source randomly
        let new_source = match rng.gen_range(0..4) {
            0 => EntropySource::MessageCount,
            1 => EntropySource::TimeOfDay,
            2 => EntropySource::Combined,
            _ => EntropySource::Random,
        };

        // Select a new rotation interval (50-150 messages)
        // This makes the pattern less predictable
        let new_interval = rng.gen_range(50..=150);
        JITTER_ROTATION_INTERVAL.store(new_interval, Ordering::SeqCst);

        // Update the entropy source (safely)
        unsafe {
            CURRENT_ENTROPY_SOURCE = new_source;
            LAST_JITTER_ROTATION = Some(now);
        }

        debug!(
            "Rotated padding strategy: new entropy source {:?}, next rotation in {} messages",
            new_source, new_interval
        );
    }
}

/// Calculate jitter factor based on current entropy source
fn calculate_jitter_factor(message_number: usize, rng: &mut ThreadRng) -> f64 {
    // Safety: Reading static mut in a controlled manner
    let entropy_source = unsafe { CURRENT_ENTROPY_SOURCE };
    let last_rotation = unsafe { LAST_JITTER_ROTATION.unwrap_or(0) };

    match entropy_source {
        EntropySource::MessageCount => {
            // Deterministic but varying based on message count
            let seed = message_number.wrapping_mul(17).wrapping_add(13);
            (seed % 100) as f64 / 100.0
        }
        EntropySource::TimeOfDay => {
            // Time-based entropy (time of day in ms)
            let time_ms = get_unix_time_ms() % 86_400_000; // ms in a day
            let seed = (time_ms.wrapping_mul(19).wrapping_add(7)) % 100;
            seed as f64 / 100.0
        }
        EntropySource::Combined => {
            // Combine message count and time for more entropy
            let count_factor = message_number.wrapping_mul(13).wrapping_add(7) % 100;
            let time_factor = ((get_unix_time_ms() - last_rotation) % 10000).wrapping_mul(23) % 100;
            let combined = (count_factor.wrapping_add(time_factor as usize)) % 100;
            combined as f64 / 100.0
        }
        EntropySource::Random => {
            // Pure randomness - most unpredictable but non-deterministic
            rng.gen::<f64>()
        }
    }
}

/// Get jitter percentage based on the jitter factor
fn get_jitter_percentage(jitter_factor: f64) -> usize {
    // Scale jitter factor to the range between MIN and MAX jitter percentages
    let range = MAX_BUCKET_SIZE_JITTER_PERCENT - MIN_BUCKET_SIZE_JITTER_PERCENT;
    (jitter_factor * range as f64) as usize + MIN_BUCKET_SIZE_JITTER_PERCENT
}

/// Determine the appropriate target size for a message with adaptive jitter
/// This adds variations to the bucket sizes to prevent statistical analysis
fn get_target_size(actual_size: usize) -> usize {
    // Increment the message counter for tracking
    let message_number = MESSAGE_COUNT.fetch_add(1, Ordering::SeqCst);

    // If message exceeds max allowed size, just return actual size
    // (this will be caught and rejected later in processing)
    if actual_size > MAX_ALLOWED_MESSAGE_SIZE {
        return actual_size;
    }

    // Check if we need to rotate the jitter strategy
    maybe_rotate_jitter_strategy();

    // Create thread-local RNG for jitter calculation
    let mut rng = rand::thread_rng();

    // Always apply jitter but with varying amounts
    let jitter_factor = calculate_jitter_factor(message_number, &mut rng);
    let jitter_percent = get_jitter_percentage(jitter_factor);

    // Find the smallest bucket size that fits the message
    for &base_bucket_size in &BASE_MESSAGE_SIZE_BUCKETS {
        if base_bucket_size >= actual_size {
            // Calculate jitter amount based on bucket size and jitter percentage
            let jitter_amount = (base_bucket_size * jitter_percent) / 100;

            // Apply jitter to increase bucket size (never decrease for security)
            let jittered_size = base_bucket_size + jitter_amount;

            trace!(
                "Applied {}% jitter to bucket size {} → {}",
                jitter_percent,
                base_bucket_size,
                jittered_size
            );

            return jittered_size;
        }
    }

    // If it's larger than our biggest bucket, use the maximum allowed size
    MAX_ALLOWED_MESSAGE_SIZE
}

/// Pad a message to a standard size bucket to prevent size correlation attacks
pub fn pad_message<T: Serialize>(message: T) -> Result<PaddedMessage<T>, anyhow::Error> {
    // Convert the message to JSON first to determine its size
    let serialized = serde_json::to_vec(&message)?;

    // Verify the message doesn't exceed maximum allowed size
    if serialized.len() > MAX_ALLOWED_MESSAGE_SIZE {
        return Err(anyhow::anyhow!(
            "Message size {} exceeds maximum allowed size {}",
            serialized.len(),
            MAX_ALLOWED_MESSAGE_SIZE
        ));
    }

    Ok(PaddedMessage::new(message, &serialized))
}

/// Extract the original message from a padded message
pub fn unpad_message<T>(padded: PaddedMessage<T>) -> T {
    padded.into_inner()
}
