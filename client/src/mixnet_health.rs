//! Mixnet Health Monitoring Module
//!
//! This module implements a privacy-preserving health monitoring system for the Nym mixnet connection.
//! It tracks connection quality, manages reconnection attempts with exponential backoff, and
//! provides metrics for the connection status without compromising the privacy guarantees of the mixnet.
//!
//! The health monitor tracks:
//! - Message delivery success rates
//! - Reconnection attempts and timing
//! - Overall connection quality assessment
//!
//! Configuration for the health monitoring system is loaded from the client configuration
//! and can be adjusted via environment variables.

use anyhow::{anyhow, Result};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tokio::time::sleep;
use tracing::{error, info, warn};

use crate::status_monitor::StatusMonitor;

// These constants are now moved to the client config file and are left here only as defaults
// for any code that might still reference them directly.
/// Default health check interval in milliseconds (10 seconds)
pub const HEALTH_CHECK_INTERVAL_MS: u64 = 10000;
/// Default minimum reconnection interval in milliseconds (5 seconds)
pub const MIN_RECONNECT_INTERVAL_MS: u64 = 5000;
/// Default maximum number of consecutive reconnection attempts
pub const MAX_RECONNECT_ATTEMPTS: u8 = 5;
/// Default exponential backoff multiplier for reconnection attempts
pub const BACKOFF_MULTIPLIER: f32 = 1.5;
/// Default threshold for poor connection quality (30% success rate)
pub const CONNECTION_QUALITY_THRESHOLD_POOR: f32 = 0.3;
/// Default threshold for fair connection quality (70% success rate)
pub const CONNECTION_QUALITY_THRESHOLD_FAIR: f32 = 0.7;
/// Default window size for connection quality assessment
pub const CONNECTION_QUALITY_WINDOW: usize = 20;

/// MixnetHealth monitors and manages the health of the mixnet connection while
/// preserving the privacy-enhancing properties of the Nym mixnet
#[derive(Debug)]
pub struct MixnetHealth {
    /// Last successful message reception time
    last_message_received: Option<Instant>,
    /// Last successful message sending time
    last_message_sent: Option<Instant>,
    /// Time of last reconnection attempt
    last_reconnection_attempt: Option<Instant>,
    /// Number of consecutive reconnection attempts
    consecutive_reconnect_attempts: u8,
    /// History of message delivery success/failure (true = success)
    message_delivery_history: Vec<bool>,
    /// Number of successful deliveries
    successful_deliveries: usize,
    /// Number of failed deliveries
    failed_deliveries: usize,
    /// Is health check running
    health_check_running: bool,
}

/// Represents the quality of the mixnet connection
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ConnectionQuality {
    /// Connection is working well (>70% success rate)
    Good,
    /// Connection is experiencing some issues (30-70% success rate)
    Fair,
    /// Connection is poor (<30% success rate)
    Poor,
    /// Connection is down (no recent successful messages)
    Down,
}

impl MixnetHealth {
    /// Create a new MixnetHealth instance
    pub fn new() -> Self {
        Self {
            last_message_received: None,
            last_message_sent: None,
            last_reconnection_attempt: None,
            consecutive_reconnect_attempts: 0,
            message_delivery_history: Vec::with_capacity(CONNECTION_QUALITY_WINDOW),
            successful_deliveries: 0,
            failed_deliveries: 0,
            health_check_running: false,
        }
    }

    /// Record a successful message reception
    pub fn record_message_received(&mut self) {
        self.last_message_received = Some(Instant::now());
        // Reset consecutive reconnection attempts on successful reception
        self.consecutive_reconnect_attempts = 0;
    }

    /// Record a successful message sending
    pub fn record_message_sent(&mut self) {
        self.last_message_sent = Some(Instant::now());
    }

    /// Record message delivery outcome
    pub fn record_delivery_outcome(&mut self, success: bool) {
        // Add to history, maintaining the window size
        if self.message_delivery_history.len() >= CONNECTION_QUALITY_WINDOW {
            // Remove the oldest entry if we're at capacity
            let removed = self.message_delivery_history.remove(0);
            // Update counters based on the removed entry
            if removed {
                self.successful_deliveries = self.successful_deliveries.saturating_sub(1);
            } else {
                self.failed_deliveries = self.failed_deliveries.saturating_sub(1);
            }
        }

        // Add the new outcome
        self.message_delivery_history.push(success);
        if success {
            self.successful_deliveries += 1;
        } else {
            self.failed_deliveries += 1;
        }
    }

    /// Get the current connection quality based on message delivery history
    pub fn get_connection_quality(&self) -> ConnectionQuality {
        // If we have no history, base it on last received message time
        if self.message_delivery_history.is_empty() {
            match self.last_message_received {
                Some(time) if time.elapsed() < Duration::from_secs(30) => ConnectionQuality::Fair,
                Some(_) => ConnectionQuality::Poor, // No messages in last 30 seconds
                None => ConnectionQuality::Down,    // Never received a message
            }
        } else {
            let total = self.successful_deliveries + self.failed_deliveries;
            if total == 0 {
                return ConnectionQuality::Down;
            }

            let success_rate = self.successful_deliveries as f32 / total as f32;
            if success_rate >= CONNECTION_QUALITY_THRESHOLD_FAIR {
                ConnectionQuality::Good
            } else if success_rate >= CONNECTION_QUALITY_THRESHOLD_POOR {
                ConnectionQuality::Fair
            } else {
                ConnectionQuality::Poor
            }
        }
    }

    /// Check if it's appropriate to attempt reconnection based on backoff policy
    pub fn should_attempt_reconnection(&mut self) -> bool {
        // Don't reconnect if we're in good shape
        if matches!(self.get_connection_quality(), ConnectionQuality::Good) {
            return false;
        }

        // Check backoff timing
        match self.last_reconnection_attempt {
            Some(last_attempt) => {
                // Calculate backoff time based on consecutive attempts
                let backoff_factor =
                    BACKOFF_MULTIPLIER.powf(self.consecutive_reconnect_attempts as f32);
                let backoff_ms = (MIN_RECONNECT_INTERVAL_MS as f32 * backoff_factor) as u64;
                let backoff_duration = Duration::from_millis(backoff_ms);

                // Only attempt reconnection if we've waited long enough
                if last_attempt.elapsed() >= backoff_duration {
                    // Check if we've exceeded max attempts
                    if self.consecutive_reconnect_attempts >= MAX_RECONNECT_ATTEMPTS {
                        // If max attempts reached, only try again after a longer timeout
                        if last_attempt.elapsed() >= Duration::from_secs(60) {
                            // Reset counter and allow another sequence of attempts
                            self.consecutive_reconnect_attempts = 0;
                            self.last_reconnection_attempt = Some(Instant::now());
                            true
                        } else {
                            false
                        }
                    } else {
                        // Increment attempts and update timestamp
                        self.consecutive_reconnect_attempts += 1;
                        self.last_reconnection_attempt = Some(Instant::now());
                        true
                    }
                } else {
                    false
                }
            }
            None => {
                // First reconnection attempt
                self.consecutive_reconnect_attempts = 1;
                self.last_reconnection_attempt = Some(Instant::now());
                true
            }
        }
    }

    /// Reset the reconnection attempt counter (called after successful reconnection)
    pub fn reset_reconnection_attempts(&mut self) {
        self.consecutive_reconnect_attempts = 0;
    }

    /// Time since last received message
    #[allow(dead_code)]
    pub fn time_since_last_received(&self) -> Option<Duration> {
        self.last_message_received.map(|time| time.elapsed())
    }

    #[allow(dead_code)]
    pub fn time_since_last_sent(&self) -> Option<Duration> {
        self.last_message_sent.map(|time| time.elapsed())
    }

    /// Start a background task to periodically check connection health
    pub async fn start_health_check(
        mixnet_health: Arc<Mutex<MixnetHealth>>,
        status_monitor: Arc<Mutex<StatusMonitor>>,
    ) -> Result<()> {
        // Check if health check is already running
        {
            let mut health = mixnet_health
                .lock()
                .map_err(|e| anyhow!("Failed to lock mixnet health: {}", e))?;
            if health.health_check_running {
                return Ok(());
            }
            health.health_check_running = true;
        }

        // Clone Arc references for the async task
        let health_ref = mixnet_health.clone();
        let monitor_ref = status_monitor.clone();

        // Spawn the health check task
        tokio::spawn(async move {
            info!("Starting mixnet connection health monitoring");
            let mut interval =
                tokio::time::interval(Duration::from_millis(HEALTH_CHECK_INTERVAL_MS));

            loop {
                interval.tick().await;

                // Analyze connection health
                let quality = {
                    let health = match health_ref.lock() {
                        Ok(health) => health,
                        Err(e) => {
                            error!("Failed to lock mixnet health in health check: {}", e);
                            continue;
                        }
                    };
                    health.get_connection_quality()
                };

                // Update status monitor with current quality
                if let Ok(mut monitor) = monitor_ref.lock() {
                    let quality_str = match quality {
                        ConnectionQuality::Good => "Good",
                        ConnectionQuality::Fair => "Fair",
                        ConnectionQuality::Poor => "Poor",
                        ConnectionQuality::Down => "Down",
                    };
                    monitor.update_mixnet_health(quality_str.to_string());
                }

                // Log concerning connection quality
                match quality {
                    ConnectionQuality::Poor => {
                        warn!("Mixnet connection quality is poor");
                    }
                    ConnectionQuality::Down => {
                        error!("Mixnet connection appears to be down");
                    }
                    _ => {
                        // No need to log good/fair status
                    }
                }

                // Add a small sleep to prevent excessive CPU usage
                sleep(Duration::from_millis(100)).await;
            }
        });

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn test_connection_quality_calculation() {
        let mut health = MixnetHealth::new();

        // Initially should be Down with no history
        assert_eq!(health.get_connection_quality(), ConnectionQuality::Down);

        // Record some mixed results (60% success rate)
        for _ in 0..6 {
            health.record_delivery_outcome(true);
        }
        for _ in 0..4 {
            health.record_delivery_outcome(false);
        }

        // Should be Fair (between 30% and 70%)
        assert_eq!(health.get_connection_quality(), ConnectionQuality::Fair);

        // Record more successful deliveries to make it Good
        for _ in 0..8 {
            health.record_delivery_outcome(true);
        }

        // Should be Good (>70% success)
        assert_eq!(health.get_connection_quality(), ConnectionQuality::Good);

        // Test window size maintenance
        let _initial_history_size = health.message_delivery_history.len();
        for _ in 0..30 {
            health.record_delivery_outcome(false);
        }
        assert!(health.message_delivery_history.len() <= CONNECTION_QUALITY_WINDOW);

        // Should be Poor now (<30% success)
        assert_eq!(health.get_connection_quality(), ConnectionQuality::Poor);
    }

    #[test]
    fn test_reconnection_backoff() {
        let mut health = MixnetHealth::new();

        // First attempt should be allowed
        assert!(health.should_attempt_reconnection());
        assert_eq!(health.consecutive_reconnect_attempts, 1);

        // Second attempt should be denied (not enough time elapsed)
        assert!(!health.should_attempt_reconnection());

        // Simulate enough time passing for backoff
        health.last_reconnection_attempt =
            Some(Instant::now() - Duration::from_millis(MIN_RECONNECT_INTERVAL_MS * 2));

        // Now should allow another attempt
        assert!(health.should_attempt_reconnection());
        assert_eq!(health.consecutive_reconnect_attempts, 2);

        // Reset should clear the counter
        health.reset_reconnection_attempts();
        assert_eq!(health.consecutive_reconnect_attempts, 0);
    }

    #[test]
    fn test_message_reception_tracking() {
        let mut health = MixnetHealth::new();

        // Initially both timestamps should be None
        assert!(health.last_message_received.is_none());
        assert!(health.last_message_sent.is_none());

        // Record message reception
        health.record_message_received();
        assert!(health.last_message_received.is_some());

        // Record message sending
        health.record_message_sent();
        assert!(health.last_message_sent.is_some());

        // Recording a message reception should reset reconnection attempts
        health.consecutive_reconnect_attempts = 3;
        health.record_message_received();
        assert_eq!(health.consecutive_reconnect_attempts, 0);
    }

    #[test]
    fn test_delivery_outcome_statistics() {
        let mut health = MixnetHealth::new();

        // Record alternating success/failure
        for i in 0..10 {
            health.record_delivery_outcome(i % 2 == 0); // Even indices are success
        }

        // Should have 5 successes and 5 failures
        assert_eq!(health.successful_deliveries, 5);
        assert_eq!(health.failed_deliveries, 5);
        assert_eq!(health.message_delivery_history.len(), 10);

        // Add more outcomes to test window size management
        for _ in 0..15 {
            health.record_delivery_outcome(true);
        }

        // Should be limited by CONNECTION_QUALITY_WINDOW size
        assert_eq!(
            health.message_delivery_history.len(),
            CONNECTION_QUALITY_WINDOW
        );

        // Should now have mostly successes since we added 15 successful outcomes
        assert!(health.successful_deliveries > health.failed_deliveries);
    }

    #[test]
    fn test_exponential_backoff() {
        let mut health = MixnetHealth::new();

        // First attempt is always allowed
        assert!(health.should_attempt_reconnection());

        // For subsequent attempts, we need to wait increasingly longer
        for attempt in 2..=5 {
            // Set the last attempt to have happened just now
            health.last_reconnection_attempt = Some(Instant::now());

            // Attempting again immediately should be denied
            assert!(!health.should_attempt_reconnection());

            // Calculate the required wait time with exponential backoff
            let required_wait =
                MIN_RECONNECT_INTERVAL_MS as f32 * BACKOFF_MULTIPLIER.powi(attempt - 1);

            // Set the last attempt to be just shy of the required wait time
            health.last_reconnection_attempt =
                Some(Instant::now() - Duration::from_millis(required_wait as u64 - 100));

            // Should still be denied
            assert!(!health.should_attempt_reconnection());

            // Set the last attempt to be just beyond the required wait time
            health.last_reconnection_attempt =
                Some(Instant::now() - Duration::from_millis(required_wait as u64 + 100));

            // Now should be allowed
            assert!(health.should_attempt_reconnection());
            assert_eq!(health.consecutive_reconnect_attempts, attempt as u8);
        }
    }
}
