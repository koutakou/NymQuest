use colored::{ColoredString, Colorize};
use std::collections::VecDeque;
use std::time::{Duration, Instant};

/// Maximum number of latency samples to keep for statistics
const MAX_LATENCY_SAMPLES: usize = 20;

/// Connection health status levels
#[derive(Debug, Clone, PartialEq)]
pub enum ConnectionHealth {
    Excellent,
    Good,
    Fair,
    Poor,
    Critical,
}

/// Privacy protection level indicators
#[derive(Debug, Clone, PartialEq)]
pub enum PrivacyLevel {
    FullyProtected, // Connected through mixnet with full anonymity
    Protected,      // Connected through mixnet with some metadata leakage
    Degraded,       // Connection issues affecting privacy
    Compromised,    // Direct connection or privacy failure
}

/// Message delivery status
#[derive(Debug, Clone, PartialEq)]
pub enum DeliveryStatus {
    Sent, // Message sent to mixnet
    #[allow(dead_code)] // Part of complete delivery tracking API for future use
    InTransit, // Message in mixnet pipeline
    Delivered, // Acknowledgment received
    Failed, // Delivery failed
    Timeout, // Delivery timeout
}

/// Represents a tracked message for delivery confirmation
#[derive(Debug, Clone)]
pub struct TrackedMessage {
    pub seq_num: u64,
    #[allow(dead_code)] // Part of complete message tracking API for future use
    pub sent_at: Instant,
    pub status: DeliveryStatus,
    #[allow(dead_code)] // Part of complete message tracking API for future use
    pub retries: u32,
}

/// Network statistics for monitoring connection health
#[derive(Debug, Clone)]
pub struct NetworkStats {
    /// Recent latency measurements in milliseconds
    pub latency_samples: VecDeque<u64>,
    /// Average latency over recent samples
    pub avg_latency_ms: u64,
    /// Number of sent messages
    pub messages_sent: u64,
    /// Number of successfully delivered messages
    pub messages_delivered: u64,
    /// Number of failed messages
    pub messages_failed: u64,
    /// Last successful communication timestamp
    pub last_successful_communication: Option<Instant>,
    /// Number of consecutive failures
    pub consecutive_failures: u32,
    /// Mixnet routing hops (for privacy assessment)
    pub estimated_hops: u32,
}

impl NetworkStats {
    pub fn new() -> Self {
        Self {
            latency_samples: VecDeque::with_capacity(MAX_LATENCY_SAMPLES),
            avg_latency_ms: 0,
            messages_sent: 0,
            messages_delivered: 0,
            messages_failed: 0,
            last_successful_communication: None,
            consecutive_failures: 0,
            estimated_hops: 3, // Default mixnet hops
        }
    }

    /// Calculate packet loss percentage
    pub fn packet_loss_percentage(&self) -> f32 {
        if self.messages_sent == 0 {
            return 0.0;
        }
        (self.messages_failed as f32 / self.messages_sent as f32) * 100.0
    }

    /// Get delivery success rate
    pub fn delivery_success_rate(&self) -> f32 {
        if self.messages_sent == 0 {
            return 100.0;
        }
        (self.messages_delivered as f32 / self.messages_sent as f32) * 100.0
    }
}

/// Information about message pacing for privacy protection
#[derive(Debug, Clone)]
pub struct PacingInfo {
    /// Whether message pacing is enabled
    pub enabled: bool,
    /// Base pacing interval in milliseconds
    pub interval_ms: u64,
    /// Last jitter value applied in milliseconds
    pub jitter_ms: u64,
    /// Timestamp of last pacing update
    pub last_update: Instant,
}

impl PacingInfo {
    pub fn new() -> Self {
        Self {
            enabled: false,
            interval_ms: 0,
            jitter_ms: 0,
            last_update: Instant::now(),
        }
    }
}

/// Main status monitor for tracking connection health and privacy
#[derive(Debug, Clone)]
pub struct StatusMonitor {
    /// Network statistics and metrics
    pub network_stats: NetworkStats,
    /// Currently tracked messages awaiting delivery confirmation
    pub tracked_messages: VecDeque<TrackedMessage>,
    /// Current privacy protection level
    pub privacy_level: PrivacyLevel,
    /// Connection health based on network metrics
    pub connection_health: ConnectionHealth,
    /// Is the client connected to the mixnet
    pub connected: bool,
    /// Pacing information for privacy protection
    pub pacing_info: PacingInfo,
    /// Anonymity set size (number of participants in the mixnet)
    pub anonymity_set_size: u32,
    /// Last game state update message (to display in UI)
    pub last_game_update: Option<String>,
    /// Last connection status update (to display in UI)
    pub last_connection_update: Option<String>,
}

impl StatusMonitor {
    /// Create a new status monitor
    pub fn new() -> Self {
        Self {
            network_stats: NetworkStats::new(),
            tracked_messages: VecDeque::with_capacity(100),
            privacy_level: PrivacyLevel::Compromised, // Start pessimistic
            connection_health: ConnectionHealth::Poor,
            connected: false,
            anonymity_set_size: 0,
            pacing_info: PacingInfo::new(),
            last_game_update: None,
            last_connection_update: None,
        }
    }

    /// Update game state information to display in the UI
    pub fn update_game_state_info(&mut self, message: String) {
        self.last_game_update = Some(message);
    }

    /// Update connection status information to display in the UI
    pub fn update_connection_info(&mut self, message: String) {
        self.last_connection_update = Some(message);
    }

    /// Get game state update message if available
    pub fn get_game_state_info(&self) -> Option<&String> {
        self.last_game_update.as_ref()
    }

    /// Get connection status update message if available
    pub fn get_connection_info(&self) -> Option<&String> {
        self.last_connection_update.as_ref()
    }

    /// Record that a message was sent
    pub fn record_message_sent(&mut self, seq_num: u64) {
        self.network_stats.messages_sent += 1;

        // Add message to tracking queue
        let tracked_msg = TrackedMessage {
            seq_num,
            sent_at: Instant::now(),
            status: DeliveryStatus::Sent,
            retries: 0,
        };

        self.tracked_messages.push_back(tracked_msg);

        // Limit tracking queue size
        while self.tracked_messages.len() > 100 {
            self.tracked_messages.pop_front();
        }

        self.update_status();
    }

    /// Record that a message was successfully delivered (acknowledgment received)
    pub fn record_message_delivered(&mut self, seq_num: u64, latency_ms: u64) {
        // Update network stats
        self.network_stats.messages_delivered += 1;
        self.network_stats.last_successful_communication = Some(Instant::now());
        self.network_stats.consecutive_failures = 0;

        // Add latency sample
        self.network_stats.latency_samples.push_back(latency_ms);
        if self.network_stats.latency_samples.len() > MAX_LATENCY_SAMPLES {
            self.network_stats.latency_samples.pop_front();
        }

        // Recalculate average latency
        let sum: u64 = self.network_stats.latency_samples.iter().sum();
        self.network_stats.avg_latency_ms = sum / self.network_stats.latency_samples.len() as u64;

        // Update tracked message status
        if let Some(msg) = self
            .tracked_messages
            .iter_mut()
            .find(|m| m.seq_num == seq_num)
        {
            msg.status = DeliveryStatus::Delivered;
        }

        self.update_status();
    }

    /// Record that a message failed to deliver
    pub fn record_message_failed(&mut self, seq_num: u64) {
        self.network_stats.messages_failed += 1;
        self.network_stats.consecutive_failures += 1;

        // Update tracked message status
        if let Some(msg) = self
            .tracked_messages
            .iter_mut()
            .find(|m| m.seq_num == seq_num)
        {
            msg.status = DeliveryStatus::Failed;
        }

        self.update_status();
    }

    /// Record that a message timed out
    pub fn record_message_timeout(&mut self, seq_num: u64) {
        self.network_stats.messages_failed += 1;
        self.network_stats.consecutive_failures += 1;

        // Update tracked message status
        if let Some(msg) = self
            .tracked_messages
            .iter_mut()
            .find(|m| m.seq_num == seq_num)
        {
            msg.status = DeliveryStatus::Timeout;
        }

        self.update_status();
    }

    /// Update mixnet connection status
    pub fn update_mixnet_status(
        &mut self,
        connected: bool,
        estimated_hops: Option<u32>,
        anonymity_set_size: Option<u32>,
    ) {
        self.connected = connected;

        if let Some(hops) = estimated_hops {
            self.network_stats.estimated_hops = hops;
        }

        if let Some(anon_size) = anonymity_set_size {
            self.anonymity_set_size = anon_size;
        }

        self.update_status();
    }

    /// Check for message timeouts and update their status
    #[allow(dead_code)] // Part of complete message tracking API for future use
    pub fn check_message_timeouts(&mut self, timeout_duration: Duration) {
        let now = Instant::now();

        for msg in &mut self.tracked_messages {
            if (msg.status == DeliveryStatus::Sent || msg.status == DeliveryStatus::InTransit)
                && now.duration_since(msg.sent_at) > timeout_duration
            {
                msg.status = DeliveryStatus::Timeout;
                self.network_stats.messages_failed += 1;
                self.network_stats.consecutive_failures += 1;
            }
        }

        self.update_status();
    }

    /// Update overall connection health and privacy level assessment
    fn update_status(&mut self) {
        // Calculate current connection health and privacy level
        self.connection_health = self.assess_connection_health();
        self.privacy_level = self.assess_privacy_level();

        // Update last status update timestamp in network stats
        self.network_stats.last_successful_communication = Some(Instant::now());
    }

    /// Update the connection status with information from the mixnet health monitor
    pub fn update_mixnet_health(&mut self, quality: String) {
        // Update connection info with detailed mixnet health quality info
        self.update_connection_info(format!("Mixnet health: {}", quality));

        // Update connection health based on mixnet quality
        self.connection_health = match quality.as_str() {
            "Excellent" => ConnectionHealth::Excellent,
            "Good" => ConnectionHealth::Good,
            "Fair" => ConnectionHealth::Fair,
            "Poor" => ConnectionHealth::Poor,
            _ => ConnectionHealth::Critical,
        };

        // Update privacy level based on connection health
        self.update_status();

        // Update the last successful communication timestamp
        self.network_stats.last_successful_communication = Some(Instant::now());
    }

    /// Update connection status message
    pub fn update_connection_status(&mut self, status: &str) {
        self.update_connection_info(format!("Connection: {}", status));
        self.update_status();
    }

    /// Assess current connection health based on metrics
    fn assess_connection_health(&self) -> ConnectionHealth {
        let packet_loss = self.network_stats.packet_loss_percentage();
        let avg_latency = self.network_stats.avg_latency_ms;
        let consecutive_failures = self.network_stats.consecutive_failures;

        // Health assessment based on multiple factors
        if consecutive_failures > 5 || packet_loss > 50.0 {
            ConnectionHealth::Critical
        } else if consecutive_failures > 3 || packet_loss > 25.0 || avg_latency > 10000 {
            ConnectionHealth::Poor
        } else if consecutive_failures > 1 || packet_loss > 10.0 || avg_latency > 5000 {
            ConnectionHealth::Fair
        } else if packet_loss > 5.0 || avg_latency > 2000 {
            ConnectionHealth::Good
        } else {
            ConnectionHealth::Excellent
        }
    }

    /// Assess current privacy level based on mixnet status, connection health, and pacing
    fn assess_privacy_level(&self) -> PrivacyLevel {
        if !self.connected {
            return PrivacyLevel::Compromised;
        }

        // Consider message pacing as a factor in privacy assessment
        let has_pacing = self.pacing_info.enabled && self.pacing_info.interval_ms > 50;

        match self.connection_health {
            ConnectionHealth::Critical => PrivacyLevel::Compromised,
            ConnectionHealth::Poor => PrivacyLevel::Degraded,
            ConnectionHealth::Fair => {
                if self.network_stats.estimated_hops >= 3 && self.anonymity_set_size > 10 {
                    // Message pacing helps protect against timing analysis
                    if has_pacing {
                        PrivacyLevel::Protected
                    } else {
                        // Without message pacing, we're more vulnerable to timing attacks
                        PrivacyLevel::Degraded
                    }
                } else {
                    PrivacyLevel::Degraded
                }
            }
            ConnectionHealth::Good => {
                if self.network_stats.estimated_hops >= 3 && self.anonymity_set_size > 50 {
                    // Message pacing is needed for full protection
                    if has_pacing {
                        PrivacyLevel::FullyProtected
                    } else {
                        PrivacyLevel::Protected
                    }
                } else {
                    PrivacyLevel::Protected
                }
            }
            ConnectionHealth::Excellent => {
                if self.network_stats.estimated_hops >= 3 && self.anonymity_set_size > 100 {
                    // Message pacing is needed for full protection
                    if has_pacing {
                        PrivacyLevel::FullyProtected
                    } else {
                        PrivacyLevel::Protected
                    }
                } else {
                    PrivacyLevel::Protected
                }
            }
        }
    }

    /// Get a colored status indicator for connection health
    #[allow(dead_code)] // Part of complete monitoring API for future use
    pub fn health_indicator(&self) -> ColoredString {
        match self.connection_health {
            ConnectionHealth::Excellent => "●".green().bold(),
            ConnectionHealth::Good => "●".cyan(),
            ConnectionHealth::Fair => "●".yellow(),
            ConnectionHealth::Poor => "●".red(),
            ConnectionHealth::Critical => "●".red().bold(),
        }
    }

    /// Get a colored status indicator for privacy level
    #[allow(dead_code)] // Part of complete monitoring API for future use
    pub fn privacy_indicator(&self) -> ColoredString {
        match self.privacy_level {
            PrivacyLevel::FullyProtected => "🛡️".green().bold(),
            PrivacyLevel::Protected => "🛡️".cyan(),
            PrivacyLevel::Degraded => "⚠️".yellow(),
            PrivacyLevel::Compromised => "🚨".red().bold(),
        }
    }

    /// Update message pacing information
    pub fn update_message_pacing(&mut self, enabled: bool, interval_ms: u64, jitter_ms: u64) {
        self.pacing_info.enabled = enabled;
        self.pacing_info.interval_ms = interval_ms;
        self.pacing_info.jitter_ms = jitter_ms;
        self.pacing_info.last_update = Instant::now();

        // Updating status potentially affects privacy level
        self.update_status();
    }

    /// Get current message pacing status
    #[allow(dead_code)] // Part of complete monitoring API for future use
    pub fn get_pacing_status(&self) -> (bool, u64, u64) {
        (
            self.pacing_info.enabled,
            self.pacing_info.interval_ms,
            self.pacing_info.jitter_ms,
        )
    }

    /// Get a colored status indicator for message pacing
    #[allow(dead_code)] // Part of complete monitoring API for future use
    pub fn pacing_indicator(&self) -> ColoredString {
        if self.pacing_info.enabled {
            match self.pacing_info.interval_ms {
                0..=50 => "⏱️".yellow(), // Minimal pacing
                51..=150 => "⏱️".cyan(), // Moderate pacing
                _ => "⏱️".green(),       // Strong pacing
            }
        } else {
            "⏱️".red() // Disabled
        }
    }

    /// Get human-readable description of current status
    pub fn status_description(&self) -> (String, String) {
        let health_desc = match self.connection_health {
            ConnectionHealth::Excellent => "Excellent".to_string(),
            ConnectionHealth::Good => "Good".to_string(),
            ConnectionHealth::Fair => "Fair".to_string(),
            ConnectionHealth::Poor => "Poor".to_string(),
            ConnectionHealth::Critical => "Critical".to_string(),
        };

        let privacy_desc = match self.privacy_level {
            PrivacyLevel::FullyProtected => "Fully Protected".to_string(),
            PrivacyLevel::Protected => "Protected".to_string(),
            PrivacyLevel::Degraded => "Degraded".to_string(),
            PrivacyLevel::Compromised => "Compromised".to_string(),
        };

        (health_desc, privacy_desc)
    }

    /// Get pending message count (sent but not yet delivered)
    #[allow(dead_code)] // Part of complete monitoring API for future use
    pub fn pending_message_count(&self) -> usize {
        self.tracked_messages
            .iter()
            .filter(|msg| matches!(msg.status, DeliveryStatus::Sent | DeliveryStatus::InTransit))
            .count()
    }
}

impl Default for StatusMonitor {
    fn default() -> Self {
        Self::new()
    }
}
