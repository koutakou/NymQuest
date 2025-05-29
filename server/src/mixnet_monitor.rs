//! Mixnet Connection Monitoring for Server
//!
//! This module provides server-side monitoring for the Nym mixnet connection, tracking message
//! reception and delivery statistics while maintaining the privacy guarantees of the mixnet.
//!
//! The monitor provides:
//! - Message reception tracking
//! - Message sending success/failure tracking
//! - Connection quality assessment
//! - Periodic logging of connection statistics
//!
//! The monitoring system is designed to be privacy-preserving, tracking only metadata about
//! message delivery success rates without inspecting or recording any message content.

use anyhow::Result;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::Mutex;
use tokio::time::sleep;
use tracing::{debug, error, info, warn};

/// Constants for mixnet monitoring
/// Interval between health checks (10 seconds)
const MONITOR_INTERVAL_MS: u64 = 10000;
/// Number of messages to consider for quality assessment
#[allow(dead_code)]
const CONNECTION_QUALITY_WINDOW: usize = 20;
/// Threshold for poor connection quality (30% success rate)
const CONNECTION_QUALITY_THRESHOLD_POOR: f32 = 0.3;
/// Threshold for fair connection quality (70% success rate)
const CONNECTION_QUALITY_THRESHOLD_FAIR: f32 = 0.7;
/// Timeout for connection inactivity (30 seconds)
const CONNECTION_TIMEOUT_MS: u64 = 30000;

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

/// MixnetMonitor tracks mixnet connection health on the server side
#[derive(Debug)]
pub struct MixnetMonitor {
    /// Last successful message reception time
    last_message_received: Mutex<Option<Instant>>,
    /// Last successful message sending time
    last_message_sent: Mutex<Option<Instant>>,
    /// Total number of messages received
    messages_received: AtomicU64,
    /// Total number of messages sent
    messages_sent: AtomicU64,
    /// Total number of message send failures
    send_failures: AtomicU64,
    /// Is monitor running
    monitor_running: AtomicBool,
    /// Current connection quality
    connection_quality: Mutex<ConnectionQuality>,
    /// Time when monitoring started
    started_at: Instant,
}

impl MixnetMonitor {
    /// Create a new MixnetMonitor instance
    pub fn new() -> Arc<Self> {
        Arc::new(Self {
            last_message_received: Mutex::new(None),
            last_message_sent: Mutex::new(None),
            messages_received: AtomicU64::new(0),
            messages_sent: AtomicU64::new(0),
            send_failures: AtomicU64::new(0),
            monitor_running: AtomicBool::new(false),
            connection_quality: Mutex::new(ConnectionQuality::Fair), // Start with assumption of fair quality
            started_at: Instant::now(),
        })
    }

    /// Record a successful message reception
    pub async fn record_message_received(&self) {
        self.messages_received.fetch_add(1, Ordering::SeqCst);
        let mut last_received = self.last_message_received.lock().await;
        *last_received = Some(Instant::now());
    }

    /// Record a successful message sending
    pub async fn record_message_sent(&self) {
        self.messages_sent.fetch_add(1, Ordering::SeqCst);
        let mut last_sent = self.last_message_sent.lock().await;
        *last_sent = Some(Instant::now());
    }

    /// Record a message send failure
    pub fn record_send_failure(&self) {
        self.send_failures.fetch_add(1, Ordering::SeqCst);
    }

    /// Get the current connection quality
    #[allow(dead_code)]
    pub async fn get_connection_quality(&self) -> ConnectionQuality {
        let quality = self.connection_quality.lock().await;
        *quality
    }

    /// Calculate message success rate
    pub fn calculate_success_rate(&self) -> f32 {
        let sent = self.messages_sent.load(Ordering::SeqCst);
        let failures = self.send_failures.load(Ordering::SeqCst);

        if sent == 0 {
            return 1.0; // No messages sent yet, assume perfect
        }

        let successes = sent.saturating_sub(failures);
        successes as f32 / sent as f32
    }

    /// Start monitoring mixnet connection health
    pub async fn start_monitoring(monitor: Arc<Self>) -> Result<()> {
        // Check if monitoring is already running
        if monitor.monitor_running.load(Ordering::SeqCst) {
            return Ok(());
        }

        // Mark as running
        monitor.monitor_running.store(true, Ordering::SeqCst);

        // Clone Arc reference for the async task
        let monitor_ref = monitor.clone();

        // Spawn the monitoring task
        tokio::spawn(async move {
            info!("Starting mixnet connection health monitoring");
            let mut interval = tokio::time::interval(Duration::from_millis(MONITOR_INTERVAL_MS));

            loop {
                interval.tick().await;

                // Assess connection quality
                let quality = MixnetMonitor::assess_connection_quality(&monitor_ref).await;

                // Update stored quality
                {
                    let mut stored_quality = monitor_ref.connection_quality.lock().await;
                    *stored_quality = quality;
                }

                // Log concerning connection quality
                match quality {
                    ConnectionQuality::Poor => {
                        warn!("Mixnet connection quality is poor");
                    }
                    ConnectionQuality::Down => {
                        error!("Mixnet connection appears to be down");
                    }
                    ConnectionQuality::Fair => {
                        debug!("Mixnet connection quality is fair");
                    }
                    ConnectionQuality::Good => {
                        debug!("Mixnet connection quality is good");
                    }
                }

                // Add a small sleep to prevent excessive CPU usage
                sleep(Duration::from_millis(100)).await;
            }
        });

        Ok(())
    }

    /// Assess current connection quality based on metrics
    async fn assess_connection_quality(monitor: &Arc<MixnetMonitor>) -> ConnectionQuality {
        let success_rate = monitor.calculate_success_rate();

        // Check if we have any activity
        let last_sent = *monitor.last_message_sent.lock().await;
        let last_received = *monitor.last_message_received.lock().await;

        // If no activity at all, check if we've just started
        if last_sent.is_none() && last_received.is_none() {
            // If we just started (within 10 seconds), assume Fair quality
            // This prevents immediately showing Down status right after connecting
            if monitor.started_at.elapsed() < Duration::from_secs(10) {
                return ConnectionQuality::Fair;
            }
            return ConnectionQuality::Down;
        }

        // If recent activity but no messages for a while, connection might be down
        if let Some(last_time) = last_received {
            let elapsed = last_time.elapsed().as_millis() as u64;
            if elapsed > CONNECTION_TIMEOUT_MS {
                debug!(
                    "No message received for {}ms, marking connection as Down",
                    elapsed
                );
                return ConnectionQuality::Down;
            }
        }

        if let Some(last_time) = last_sent {
            let elapsed = last_time.elapsed().as_millis() as u64;
            if elapsed > CONNECTION_TIMEOUT_MS {
                debug!(
                    "No message sent for {}ms, marking connection as Down",
                    elapsed
                );
                return ConnectionQuality::Down;
            }
        }

        // Determine quality based on success rate thresholds
        if success_rate >= CONNECTION_QUALITY_THRESHOLD_FAIR {
            ConnectionQuality::Good
        } else if success_rate >= CONNECTION_QUALITY_THRESHOLD_POOR {
            ConnectionQuality::Fair
        } else {
            ConnectionQuality::Poor
        }
    }

    /// Get current monitoring statistics
    pub async fn get_stats(&self) -> (u64, u64, u64, ConnectionQuality) {
        let received = self.messages_received.load(Ordering::SeqCst);
        let sent = self.messages_sent.load(Ordering::SeqCst);
        let failures = self.send_failures.load(Ordering::SeqCst);
        let quality = *self.connection_quality.lock().await;

        (received, sent, failures, quality)
    }

    /// Log current connection statistics
    pub async fn log_connection_stats(&self) {
        let (received, sent, failures, quality) = self.get_stats().await;
        let success_rate = self.calculate_success_rate() * 100.0;

        let quality_str = match quality {
            ConnectionQuality::Good => "Good",
            ConnectionQuality::Fair => "Fair",
            ConnectionQuality::Poor => "Poor",
            ConnectionQuality::Down => "Down",
        };

        let uptime = self.started_at.elapsed().as_secs();
        let hours = uptime / 3600;
        let minutes = (uptime % 3600) / 60;
        let seconds = uptime % 60;

        info!(
            "Mixnet connection statistics - Quality: {}, Uptime: {}h {}m {}s, Messages received: {}, Messages sent: {}, Failures: {}, Success rate: {:.1}%",
            quality_str, hours, minutes, seconds, received, sent, failures, success_rate
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_connection_quality_calculation() {
        let monitor = MixnetMonitor::new();

        // Initially should be Fair
        assert_eq!(
            monitor.get_connection_quality().await,
            ConnectionQuality::Fair
        );

        // Step 1: Record 10 successful messages (10 sent, 0 failures = 100% success)
        for _ in 0..10 {
            monitor.record_message_sent().await;
        }

        // Manually update quality
        let quality = MixnetMonitor::assess_connection_quality(&monitor).await;
        {
            let mut stored_quality = monitor.connection_quality.lock().await;
            *stored_quality = quality;
        }

        // Should be Good with 100% success rate
        assert_eq!(
            monitor.get_connection_quality().await,
            ConnectionQuality::Good
        );

        // Step 2: Create a new monitor for the fair quality test
        let monitor = MixnetMonitor::new();

        // Record 10 sent with 5 failures (50% success rate)
        for _ in 0..10 {
            monitor.record_message_sent().await;
        }
        for _ in 0..5 {
            monitor.record_send_failure();
        }

        // Manually update quality
        let quality = MixnetMonitor::assess_connection_quality(&monitor).await;
        {
            let mut stored_quality = monitor.connection_quality.lock().await;
            *stored_quality = quality;
        }

        // Should be Fair with 50% success rate
        assert_eq!(
            monitor.get_connection_quality().await,
            ConnectionQuality::Fair
        );

        // Step 3: Create a new monitor for the poor quality test
        let monitor = MixnetMonitor::new();

        // Record 10 sent with 8 failures (20% success rate)
        for _ in 0..10 {
            monitor.record_message_sent().await;
        }
        for _ in 0..8 {
            monitor.record_send_failure();
        }

        // Manually update quality
        let quality = MixnetMonitor::assess_connection_quality(&monitor).await;
        {
            let mut stored_quality = monitor.connection_quality.lock().await;
            *stored_quality = quality;
        }

        // Should be Poor with 20% success rate (below POOR threshold of 30%)
        assert_eq!(
            monitor.get_connection_quality().await,
            ConnectionQuality::Poor
        );
    }

    #[tokio::test]
    async fn test_message_recording() {
        let monitor = MixnetMonitor::new();

        // Record some activity
        monitor.record_message_received().await;
        monitor.record_message_sent().await;
        monitor.record_message_sent().await;
        monitor.record_send_failure();

        // Check stats
        let (received, sent, failures, _) = monitor.get_stats().await;
        assert_eq!(received, 1);
        assert_eq!(sent, 2);
        assert_eq!(failures, 1);

        // Success rate should be 50%
        assert_eq!(monitor.calculate_success_rate(), 0.5);
    }

    #[tokio::test]
    async fn test_connection_quality_assessment() {
        // Test 1: 100% success rate
        {
            let monitor = MixnetMonitor::new();
            for _ in 0..20 {
                monitor.record_message_sent().await;
            }

            let quality = MixnetMonitor::assess_connection_quality(&monitor).await;
            assert_eq!(quality, ConnectionQuality::Good);
        }

        // Test 2: 50% success rate (10 sent, 5 failures = 5/10 = 50%)
        {
            let monitor = MixnetMonitor::new();
            for _ in 0..10 {
                monitor.record_message_sent().await;
            }
            for _ in 0..5 {
                monitor.record_send_failure();
            }

            let quality = MixnetMonitor::assess_connection_quality(&monitor).await;
            assert_eq!(quality, ConnectionQuality::Fair);
        }

        // Test 3: 25% success rate (20 sent, 15 failures = 5/20 = 25%)
        {
            let monitor = MixnetMonitor::new();
            for _ in 0..20 {
                monitor.record_message_sent().await;
            }
            for _ in 0..15 {
                monitor.record_send_failure();
            }

            let quality = MixnetMonitor::assess_connection_quality(&monitor).await;
            assert_eq!(quality, ConnectionQuality::Poor);
        }
    }

    #[tokio::test]
    async fn test_stats_reporting() {
        let monitor = MixnetMonitor::new();

        // Record some messages
        for _ in 0..5 {
            monitor.record_message_received().await;
        }

        for _ in 0..10 {
            monitor.record_message_sent().await;
        }

        for _ in 0..2 {
            monitor.record_send_failure();
        }

        // Check stats
        let (received, sent, failures, _) = monitor.get_stats().await;
        assert_eq!(received, 5);
        assert_eq!(sent, 10);
        assert_eq!(failures, 2);

        // Check success rate
        let success_rate = monitor.calculate_success_rate();
        // 8 successful out of 10 sent = 80%
        assert_eq!(success_rate, 0.8);
    }

    #[tokio::test]
    async fn test_timestamp_updates() {
        let monitor = MixnetMonitor::new();

        // Initially timestamps should be None
        {
            let received = monitor.last_message_received.lock().await;
            let sent = monitor.last_message_sent.lock().await;
            assert!(received.is_none());
            assert!(sent.is_none());
        }

        // Record message reception
        monitor.record_message_received().await;

        // Check timestamp was updated
        {
            let received = monitor.last_message_received.lock().await;
            assert!(received.is_some());
        }

        // Record message sending
        monitor.record_message_sent().await;

        // Check timestamp was updated
        {
            let sent = monitor.last_message_sent.lock().await;
            assert!(sent.is_some());
        }
    }
}
