use lazy_static::lazy_static;
use std::collections::HashMap;
use std::sync::Mutex;
use tracing::debug;

use crate::config::ClientConfig;

/// Structure to manage replay protection using a sliding window approach
/// This is used to prevent replay attacks by tracking received message sequence numbers
pub struct ReplayProtectionWindow {
    /// Highest sequence number seen so far
    highest_seq: u64,
    /// Bitmap window to track received sequence numbers below the highest
    /// Each bit represents whether we've seen (highest_seq - bit_position)
    window: u128,
    /// Window size - how many previous sequence numbers we track
    window_size: u8,
}

impl ReplayProtectionWindow {
    /// Create a new replay protection window
    pub fn new(window_size: u8) -> Self {
        // window_size should be at most 128 (size of u128 in bits)
        let window_size = std::cmp::min(window_size, 128);
        ReplayProtectionWindow {
            highest_seq: 0,
            window: 0,
            window_size,
        }
    }

    /// Process a sequence number and determine if it's a replay
    /// Returns true if the message is a replay, false if it's new
    pub fn process(&mut self, seq_num: u64) -> bool {
        // Handle the very first message (when highest_seq is 0)
        if self.highest_seq == 0 {
            self.highest_seq = seq_num;
            self.window = 1; // Mark the first sequence number as seen (bit 0)
            return false; // Not a replay
        }

        // If the sequence number is higher than what we've seen, it's definitely not a replay
        if seq_num > self.highest_seq {
            // Calculate how much the window needs to slide
            let shift = std::cmp::min((seq_num - self.highest_seq) as u8, self.window_size);

            // Shift the window to accommodate the new highest sequence number
            self.window = if shift >= 128 {
                // If shift is >= 128, all bits will be shifted out, so clear the window
                0
            } else {
                self.window << shift
            };

            // Update the highest sequence number after shifting the window
            let old_highest = self.highest_seq;
            self.highest_seq = seq_num;

            // For security, we need to mark all sequence numbers between old_highest and new highest
            // that would fall within our window as "seen" to prevent replay attacks in that range
            if seq_num - old_highest <= self.window_size as u64 {
                // This is a normal case - mark all the intermediate sequence numbers as seen
                for i in 1..=shift {
                    // Mark bits for all sequence numbers between old_highest and new highest
                    self.window |= 1u128 << (shift - i);
                }
            }

            // Mark the new highest sequence number as seen (bit 0 represents highest_seq)
            self.window |= 1;

            return false; // Not a replay
        }

        // If the sequence number is the same as highest, it's a replay
        if seq_num == self.highest_seq {
            return true; // Replay
        }

        // Check if the sequence number is within our window
        let offset = self.highest_seq - seq_num;

        // If it's too old (outside our window), we consider it a replay for safety
        if offset as u8 > self.window_size {
            return true; // Too old, consider it a replay
        }

        // Check if we've already seen this sequence number
        let mask = 1u128 << (offset as u8);
        if (self.window & mask) != 0 {
            return true; // Already seen, it's a replay
        }

        // Mark this sequence number as seen
        self.window |= mask;

        false // Not a replay
    }
}

// Tracking received server messages to prevent replays
lazy_static! {
    pub static ref REPLAY_PROTECTION: Mutex<HashMap<String, ReplayProtectionWindow>> =
        Mutex::new(HashMap::new());
}

/// Check if we've seen this message before (replay protection)
/// Returns true if the message is a replay, false if it's new
pub fn is_message_replay(server_address: &str, seq_num: u64) -> bool {
    // Get the configured window size or use default if config access fails
    let window_size = get_replay_protection_window_size();

    match REPLAY_PROTECTION.lock() {
        Ok(mut protection) => {
            // Check if window exists first
            let needs_creation = !protection.contains_key(server_address);

            // Get or create the replay protection window for this server
            let window = protection
                .entry(server_address.to_string())
                .or_insert_with(|| ReplayProtectionWindow::new(window_size));

            // Check and update the window
            let result = window.process(seq_num);

            // Log after releasing the lock if this was a new window
            if needs_creation {
                drop(protection); // Explicitly release the lock
                debug!(
                    "Created replay protection window with size: {}",
                    window_size
                );
            }

            result
        }
        Err(e) => {
            tracing::error!("Warning: Failed to access replay protection data: {}", e);
            // In case of mutex poisoning, err on the side of caution and allow the message
            false
        }
    }
}

/// Get the configured replay protection window size
/// Returns the window size from config, or default 64 if config can't be loaded
fn get_replay_protection_window_size() -> u8 {
    // Default window size if we can't access the config
    const DEFAULT_WINDOW_SIZE: u8 = 64;

    // Try to load the client configuration
    match ClientConfig::load() {
        Ok(config) => config.replay_protection_window_size,
        Err(e) => {
            tracing::warn!(
                "Failed to load config for replay protection window size: {}",
                e
            );
            tracing::warn!("Using default window size of {}", DEFAULT_WINDOW_SIZE);
            DEFAULT_WINDOW_SIZE
        }
    }
}
