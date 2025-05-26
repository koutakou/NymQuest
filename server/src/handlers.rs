#![allow(clippy::clone_on_copy)]

use anyhow::Result;
use nym_sdk::mixnet::{AnonymousSenderTag, MixnetClient, MixnetMessageSender};
use rand::{thread_rng, Rng};
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tracing::{debug, error, info, trace, warn};

use crate::message_auth::{AuthKey, AuthenticatedMessage};

use crate::config::GameConfig;
use crate::game_protocol::{
    ClientMessage, ClientMessageType, Direction, EmoteType, Position, ProtocolVersion,
    ServerMessage, WorldBoundaries,
};
use crate::game_state::GameState;

/// Message priority enum for privacy-enhancing load management
/// Different message types have different priorities to prevent
/// timing correlation attacks during high server load
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum MessagePriority {
    /// Highest priority: critical system messages
    Critical = 0,
    /// High priority: authentication and connection management
    High = 1,
    /// Medium priority: gameplay affecting actions
    Medium = 2,
    /// Low priority: non-essential social interactions
    Low = 3,
}

/// Determine message priority based on message type
/// This helps randomize processing time while maintaining game responsiveness
pub fn get_message_priority(msg_type: &ClientMessageType) -> MessagePriority {
    match msg_type {
        // Critical system messages
        ClientMessageType::Disconnect => MessagePriority::Critical,
        ClientMessageType::Heartbeat => MessagePriority::High,
        ClientMessageType::Register => MessagePriority::High,

        // Gameplay affecting actions
        ClientMessageType::Move => MessagePriority::Medium,
        ClientMessageType::Attack => MessagePriority::Medium,

        // Social interactions (lower priority)
        ClientMessageType::Chat => MessagePriority::Low,
        ClientMessageType::Emote => MessagePriority::Low,
        ClientMessageType::Whisper => MessagePriority::Low,

        // Acks are processed immediately
        ClientMessageType::Ack => MessagePriority::Critical,
    }
}

/// Token bucket rate limiter for DoS protection
/// Tracks rate limits per connection to prevent spam while preserving privacy
#[derive(Debug, Clone)]
struct TokenBucket {
    tokens: f32,
    max_tokens: u32,
    refill_rate: f32, // tokens per second
    last_refill: SystemTime,
}

impl TokenBucket {
    fn new(max_tokens: u32, refill_rate: f32) -> Self {
        Self {
            tokens: max_tokens as f32,
            max_tokens,
            refill_rate,
            last_refill: SystemTime::now(),
        }
    }

    /// Attempt to consume a token, returns true if successful
    fn try_consume(&mut self) -> bool {
        self.refill_tokens();

        if self.tokens >= 1.0 {
            self.tokens -= 1.0;
            true
        } else {
            false
        }
    }

    /// Refill tokens based on elapsed time
    fn refill_tokens(&mut self) {
        let now = SystemTime::now();
        if let Ok(elapsed) = now.duration_since(self.last_refill) {
            let elapsed_secs = elapsed.as_secs_f32();
            let new_tokens = elapsed_secs * self.refill_rate;
            self.tokens = (self.tokens + new_tokens).min(self.max_tokens as f32);
            self.last_refill = now;
        }
    }
}

/// Rate limiter manager for all connections
/// Maps connection tags to token buckets for privacy-preserving rate limiting
#[derive(Debug)]
struct RateLimiter {
    buckets: HashMap<String, TokenBucket>,
    max_tokens: u32,
    refill_rate: f32,
}

impl RateLimiter {
    fn new(config: &GameConfig) -> Self {
        Self {
            buckets: HashMap::new(),
            max_tokens: config.message_burst_size,
            refill_rate: config.message_rate_limit,
        }
    }

    /// Check if a message from this connection should be allowed
    /// Returns true if allowed, false if rate limited
    fn check_rate_limit(&mut self, connection_id: &str) -> bool {
        let bucket = self
            .buckets
            .entry(connection_id.to_string())
            .or_insert_with(|| TokenBucket::new(self.max_tokens, self.refill_rate));

        bucket.try_consume()
    }

    /// Clean up old buckets to prevent memory leaks
    /// This should be called periodically
    fn cleanup_old_buckets(&mut self) {
        const CLEANUP_THRESHOLD_SECS: u64 = 300; // 5 minutes
        let now = SystemTime::now();

        self.buckets.retain(|_, bucket| {
            if let Ok(elapsed) = now.duration_since(bucket.last_refill) {
                elapsed.as_secs() < CLEANUP_THRESHOLD_SECS
            } else {
                true // Keep if we can't determine elapsed time
            }
        });
    }
}

// Helper function to convert Direction to a human-readable string
#[allow(dead_code)]
fn print_direction(direction: &Direction) -> &'static str {
    match direction {
        Direction::Up => "up",
        Direction::Down => "down",
        Direction::Left => "left",
        Direction::Right => "right",
        Direction::UpLeft => "up-left",
        Direction::UpRight => "up-right",
        Direction::DownLeft => "down-left",
        Direction::DownRight => "down-right",
    }
}

// Thread-safe sequence number generator for server messages
static SERVER_SEQ_NUM: AtomicU64 = AtomicU64::new(1);

// Thread-safe function to get the next server sequence number
fn next_seq_num() -> u64 {
    SERVER_SEQ_NUM.fetch_add(1, Ordering::SeqCst)
}

/// Calculate maximum jitter in milliseconds based on base interval and jitter percentage
/// This enhances privacy by making message processing timing less predictable
fn calculate_max_jitter(base_interval_ms: u64, jitter_percent: u8) -> u64 {
    if jitter_percent == 0 {
        return 0;
    }

    // Cap jitter percentage at 100% for safety
    let capped_percent = jitter_percent.min(100) as u64;

    // Calculate jitter as percentage of base interval
    // Formula: (base_interval * jitter_percent) / 100
    (base_interval_ms * capped_percent) / 100
}

/// Apply message processing pacing with jitter to enhance privacy by preventing timing correlation attacks
/// Returns the applied jitter in milliseconds
pub async fn apply_message_processing_jitter(
    base_interval_ms: u64,
    jitter_percent: u8,
    priority: Option<MessagePriority>,
) -> u64 {
    // If no base interval is set, return immediately
    if base_interval_ms == 0 {
        return 0;
    }

    // Calculate jitter to add randomness to timing (prevents timing analysis)
    let mut rng = rand::thread_rng();
    let max_jitter = calculate_max_jitter(base_interval_ms, jitter_percent);

    // Adjust jitter based on message priority if provided
    // This creates a more realistic timing pattern while maintaining privacy
    let jitter_ms = if let Some(priority) = priority {
        match priority {
            // Critical messages get minimal jitter (0-25% of max)
            MessagePriority::Critical => {
                if max_jitter > 0 {
                    rng.gen_range(0..=max_jitter / 4)
                } else {
                    0
                }
            }
            // High priority messages get reduced jitter (0-50% of max)
            MessagePriority::High => {
                if max_jitter > 0 {
                    rng.gen_range(0..=max_jitter / 2)
                } else {
                    0
                }
            }
            // Medium priority messages get standard jitter (0-75% of max)
            MessagePriority::Medium => {
                if max_jitter > 0 {
                    rng.gen_range(0..=(max_jitter * 3 / 4))
                } else {
                    0
                }
            }
            // Low priority messages get full jitter range (0-100% of max)
            MessagePriority::Low => rng.gen_range(0..=max_jitter),
        }
    } else {
        // If no priority specified, use full jitter range
        rng.gen_range(0..=max_jitter)
    };

    // Apply the calculated delay with jitter
    let delay_duration = Duration::from_millis(jitter_ms);
    trace!(
        "Applying message processing jitter, waiting {}ms (priority: {:?})",
        jitter_ms,
        priority
    );
    tokio::time::sleep(delay_duration).await;

    // Return the amount of jitter applied for logging/monitoring
    jitter_ms
}

// Structure to manage replay protection using a sliding window approach
struct ReplayProtectionWindow {
    // Highest sequence number seen so far
    highest_seq: u64,
    // Bitmap window to track received sequence numbers below the highest
    // Each bit represents whether we've seen (highest_seq - bit_position)
    window: u128,
    // Window size - how many previous sequence numbers we track
    window_size: u8,
    // Base window size (from configuration)
    base_window_size: u8,
    // Maximum window size (to limit memory usage)
    max_window_size: u8,
    // Minimum window size (to ensure sufficient security)
    min_window_size: u8,
    // Count of out-of-order messages received
    out_of_order_count: u32,
    // Total messages processed (for calculating adaptation rate)
    total_messages: u32,
    // Last time window size was adjusted
    last_adjustment: SystemTime,
    // Cooldown period between adjustments (in seconds)
    adjustment_cooldown: u64,
}

impl ReplayProtectionWindow {
    // Create a new replay protection window with adaptive sizing capability
    fn new(window_size: u8) -> Self {
        // Set reasonable bounds for window size adaptation
        let min_size = std::cmp::max(16, window_size / 2);
        let max_size = std::cmp::min(127, window_size * 2);

        Self {
            highest_seq: 0,
            window: 0,
            window_size,
            base_window_size: window_size,
            max_window_size: max_size,
            min_window_size: min_size,
            out_of_order_count: 0,
            total_messages: 0,
            last_adjustment: SystemTime::now(),
            adjustment_cooldown: 60, // Default 60 second cooldown between adjustments
        }
    }

    // Adjust window size based on network conditions
    fn adjust_window_size(&mut self) {
        // Check if we're in cooldown period
        if let Ok(elapsed) = SystemTime::now().duration_since(self.last_adjustment) {
            if elapsed.as_secs() < self.adjustment_cooldown {
                return;
            }
        } else {
            return; // Error in time calculation, skip adjustment
        }

        // Require minimum number of messages before adjusting
        if self.total_messages < 20 {
            return;
        }

        // Calculate out-of-order ratio (percentage of out-of-order messages)
        let out_of_order_ratio = if self.total_messages > 0 {
            self.out_of_order_count as f32 / self.total_messages as f32
        } else {
            0.0
        };

        // Adjust window size based on ratio
        let new_size = if out_of_order_ratio > 0.15 {
            // High out-of-order rate: increase window size
            std::cmp::min(self.window_size + 8, self.max_window_size)
        } else if out_of_order_ratio < 0.05 {
            // Low out-of-order rate: gradually decrease window size toward base
            if self.window_size > self.base_window_size {
                self.window_size - 4
            } else if self.window_size > self.min_window_size {
                self.window_size - 2
            } else {
                self.window_size
            }
        } else {
            // Moderate out-of-order rate: maintain current size
            self.window_size
        };

        // Apply changes if needed
        if new_size != self.window_size {
            trace!(
                "Adaptive replay protection: window size adjusted from {} to {}",
                self.window_size,
                new_size
            );
            self.window_size = new_size;
        }

        // Reset counters and update timestamp
        self.out_of_order_count = 0;
        self.total_messages = 0;
        self.last_adjustment = SystemTime::now();
    }

    // Process a sequence number and determine if it's a replay
    // Returns true if the message is a replay, false if it's new
    fn process(&mut self, seq_num: u64) -> bool {
        // Increment total messages counter for adaptive sizing
        self.total_messages = self.total_messages.saturating_add(1);

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

            // Consider adjusting window size periodically
            if self.total_messages % 100 == 0 {
                self.adjust_window_size();
            }

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

        // This is an out-of-order message (but within window) - track for adaptive sizing
        self.out_of_order_count = self.out_of_order_count.saturating_add(1);

        // Check if we've already seen this sequence number
        let mask = 1u128 << (offset as u8);
        if (self.window & mask) != 0 {
            return true; // Already seen, it's a replay
        }

        // Mark this sequence number as seen
        self.window |= mask;

        // Consider adjusting window size periodically
        if self.total_messages % 100 == 0 {
            self.adjust_window_size();
        }

        false // Not a replay
    }
}

// Tracking received client messages to prevent replays
lazy_static::lazy_static! {
    static ref REPLAY_PROTECTION: Mutex<HashMap<String, ReplayProtectionWindow>> = Mutex::new(HashMap::new());
}

// Check if we've seen this message before (replay protection)
fn is_message_replay(tag: &AnonymousSenderTag, seq_num: u64) -> bool {
    // Get a string representation of the sender tag for HashMap lookup
    let tag_str = tag.to_string();

    // Try to acquire the lock on the replay protection map
    if let Ok(mut replay_map) = REPLAY_PROTECTION.lock() {
        // Load adaptive replay protection settings from config
        let window_size = get_replay_protection_window_size();
        let (adaptive, min_window, max_window, cooldown) = match crate::config::GameConfig::load() {
            Ok(config) => (
                config.replay_protection_adaptive,
                config.replay_protection_min_window,
                config.replay_protection_max_window,
                config.replay_protection_adjustment_cooldown,
            ),
            Err(_) => (true, 32, 96, 60), // Default values if config can't be loaded
        };

        // Get or create a replay protection window for this sender
        let window = replay_map.entry(tag_str).or_insert_with(|| {
            let mut w = ReplayProtectionWindow::new(window_size);

            // If adaptive mode is enabled, apply the adaptive parameters
            if adaptive {
                w.min_window_size = min_window;
                w.max_window_size = max_window;
                w.adjustment_cooldown = cooldown;
            }

            w
        });

        // Process the sequence number and check if it's a replay
        window.process(seq_num)
    } else {
        // If we failed to acquire the lock, we conservatively assume it could be a replay
        error!("Warning: Failed to access replay protection data");
        true // Assume it's a replay to be safe
    }
}

/// Get the configured replay protection window size and related settings
/// Returns the window size from config, or default 64 if config can't be loaded
fn get_replay_protection_window_size() -> u8 {
    match crate::config::GameConfig::load() {
        Ok(config) => {
            debug!(
                "Using configured replay protection window size: {}",
                config.replay_protection_window_size
            );

            if config.replay_protection_adaptive {
                debug!(
                    "Adaptive replay protection enabled (min: {}, max: {}, cooldown: {}s)",
                    config.replay_protection_min_window,
                    config.replay_protection_max_window,
                    config.replay_protection_adjustment_cooldown
                );
            }

            config.replay_protection_window_size
        }
        Err(e) => {
            warn!(
                "Failed to load config for replay protection, using default: {}",
                e
            );
            64 // Default window size if we can't load config
        }
    }
}

/// Send an acknowledgment message back to the client
#[allow(clippy::clone_on_copy)]
async fn send_ack(
    client: &MixnetClient,
    sender_tag: &AnonymousSenderTag,
    seq_num: u64,
    msg_type: ClientMessageType,
    auth_key: &AuthKey,
) -> Result<()> {
    // Create acknowledgment
    let ack = ServerMessage::Ack {
        client_seq_num: seq_num,
        original_type: msg_type,
    };

    // Create an authenticated acknowledgment message
    let authenticated_ack = AuthenticatedMessage::new(ack, auth_key)?;

    // Serialize and send the authenticated acknowledgment
    let ack_json = serde_json::to_string(&authenticated_ack)?;
    client.send_reply(sender_tag.clone(), ack_json).await?;

    Ok(())
}

/// Broadcast a server shutdown notification to all connected players
pub async fn broadcast_shutdown_notification(
    client: &MixnetClient,
    game_state: &Arc<GameState>,
    message: &str,
    shutdown_in_seconds: u8,
    _auth_key: &AuthKey,
) -> Result<()> {
    // Get all connected players
    let player_tags = game_state.get_player_tags();
    info!(
        "Broadcasting shutdown notification to {} players",
        player_tags.len()
    );

    // Get next sequence number for the message
    let seq_num = next_seq_num();

    // Create shutdown message with the warning time
    let shutdown_msg = ServerMessage::ServerShutdown {
        message: message.to_string(),
        seq_num,
        shutdown_in_seconds,
    };

    let message_json = serde_json::to_string(&shutdown_msg)?;

    // Broadcast to all players
    for tag in player_tags {
        if let Err(e) = client.send_reply(tag.clone(), message_json.clone()).await {
            warn!("Failed to send shutdown notification to a player: {}", e);
        }
    }

    info!("Shutdown notification sent to all players");
    Ok(())
}

/// Broadcast game state to all active players
#[allow(dead_code)]
pub async fn broadcast_game_state(
    client: &MixnetClient,
    game_state: &Arc<GameState>,
    exclude_tag: Option<AnonymousSenderTag>,
    auth_key: &AuthKey,
) -> Result<()> {
    // Get the current game state
    let players = game_state.get_players();

    // Create the game state message
    let game_state_message = ServerMessage::GameState {
        players,
        seq_num: next_seq_num(),
    };

    // Create an authenticated message with HMAC
    let authenticated_message = AuthenticatedMessage::new(game_state_message, auth_key)?;
    let serialized = serde_json::to_string(&authenticated_message)?;

    // Get a copy of all active connections
    let connections = game_state.get_connections();

    // Keep track of players we failed to send messages to
    let mut failed_tags = Vec::new();

    // Send state update to each connected player
    for (player_id, tag) in connections {
        // Skip excluded player if any
        if let Some(exclude) = &exclude_tag {
            if exclude.to_string() == tag.to_string() {
                continue;
            }
        }

        // Send the update to this player and track failures
        if let Err(e) = client.send_reply(tag.clone(), serialized.clone()).await {
            error!("Failed to send game state to player {}: {}", player_id, e);
            failed_tags.push(tag);
        }
    }

    // Clean up any players that we couldn't reach
    for tag in failed_tags {
        if let Some(player_id) = game_state.remove_player(&tag) {
            info!("Removed unreachable player: {}", player_id);
        }
    }

    Ok(())
}

// Global rate limiter instance for DoS protection
lazy_static::lazy_static! {
    static ref GLOBAL_RATE_LIMITER: Arc<Mutex<Option<RateLimiter>>> = Arc::new(Mutex::new(None));
}

/// Initialize the global rate limiter with game configuration
/// This should be called once during server startup
pub fn init_rate_limiter(config: &GameConfig) {
    let mut limiter = GLOBAL_RATE_LIMITER.lock().unwrap();
    *limiter = Some(RateLimiter::new(config));
    info!(
        "Rate limiter initialized: {:.1} msg/sec, burst: {}",
        config.message_rate_limit, config.message_burst_size
    );
}

/// Clean up old rate limiting buckets
/// This should be called periodically to prevent memory leaks
pub fn cleanup_rate_limiter() {
    if let Ok(mut limiter_guard) = GLOBAL_RATE_LIMITER.lock() {
        if let Some(ref mut limiter) = *limiter_guard {
            limiter.cleanup_old_buckets();
        }
    }
}

/// Handle a message from a client
pub async fn handle_client_message(
    client: &MixnetClient,
    game_state: &Arc<GameState>,
    message: ClientMessage,
    sender_tag: AnonymousSenderTag,
    auth_key: &AuthKey,
) -> Result<()> {
    // Check rate limit
    let should_rate_limit = if let Ok(mut limiter_guard) = GLOBAL_RATE_LIMITER.lock() {
        if let Some(ref mut limiter) = *limiter_guard {
            !limiter.check_rate_limit(&sender_tag.to_string())
        } else {
            false
        }
    } else {
        false
    };

    if should_rate_limit {
        warn!("Rate limit exceeded for connection {}", sender_tag);

        // Send rate limit error to client
        let error_msg = ServerMessage::Error {
            message: "Rate limit exceeded. Please slow down your message frequency.".to_string(),
            seq_num: next_seq_num(),
        };
        let authenticated_response = AuthenticatedMessage::new(error_msg, auth_key)?;
        let response_str = serde_json::to_string(&authenticated_response)?;

        // Send reply to the rate-limited client
        let _ = client.send_reply(sender_tag.clone(), response_str).await;

        return Ok(());
    }

    // Get sequence number for replay protection and acknowledgments
    let seq_num = message.get_seq_num();
    let msg_type = message.get_type();

    // Handle acknowledgments separately and directly
    if let ClientMessage::Ack { .. } = &message {
        // We don't need to do anything with acks in this simple implementation
        // In a more complex system, we might track which messages were acknowledged
        return Ok(());
    }

    // Check for message replay, but only for non-ack messages
    if is_message_replay(&sender_tag, seq_num) {
        warn!(
            "Detected replay attack or duplicate message: seq {} from {:?}",
            seq_num, sender_tag
        );
        return Ok(());
    }

    // Determine message priority for privacy-enhancing processing
    let priority = get_message_priority(&msg_type);

    // Apply priority-based jitter before processing to enhance privacy
    // Get server configuration for message processing
    let (processing_interval, jitter_percent) = {
        let config = GameConfig::load().unwrap_or_default();
        if config.enable_message_processing_pacing {
            (
                config.message_processing_interval_ms,
                config.message_processing_jitter_percent,
            )
        } else {
            (0, 0)
        }
    };

    // Apply priority-based message processing jitter
    if processing_interval > 0 {
        let jitter_applied =
            apply_message_processing_jitter(processing_interval, jitter_percent, Some(priority))
                .await;

        trace!(
            "Applied {}ms jitter for {:?} message (priority: {:?})",
            jitter_applied,
            msg_type,
            priority
        );
    }

    // Send acknowledgment first for all non-ack messages
    send_ack(client, &sender_tag, seq_num, msg_type, auth_key).await?;

    // Process the message based on its type
    match message {
        ClientMessage::Register {
            name,
            seq_num: _,
            protocol_version,
        } => {
            // First, check protocol version compatibility
            let server_version = ProtocolVersion::default();
            let negotiated_version = match server_version.negotiate_with(&protocol_version) {
                Some(version) => {
                    info!(
                        "Protocol version negotiated: v{} (client: v{}, server: v{})",
                        version, protocol_version.current, server_version.current
                    );
                    version
                }
                None => {
                    error!("Protocol version incompatible: client v{} (min: v{}), server v{} (min: v{})",
                           protocol_version.current, protocol_version.min_supported,
                           server_version.current, server_version.min_supported);

                    // Send error message for incompatible version
                    let error_msg = ServerMessage::Error {
                        message: format!("Protocol version incompatible. Server supports v{}-v{}, client requested v{}-v{}",
                                       server_version.min_supported, server_version.current,
                                       protocol_version.min_supported, protocol_version.current),
                        seq_num: next_seq_num(),
                    };

                    let authenticated_error = AuthenticatedMessage::new(error_msg, auth_key)?;
                    let error_json = serde_json::to_string(&authenticated_error)?;
                    client.send_reply(sender_tag.clone(), error_json).await?;

                    return Ok(());
                }
            };

            // Check if this sender_tag is already associated with a registered player
            if let Some(existing_player_id) = game_state.get_player_id(&sender_tag) {
                // Client is already registered, send an error message
                let error_msg = ServerMessage::Error {
                    message: format!("Player {} is already registered", existing_player_id),
                    seq_num: next_seq_num(),
                };

                let authenticated_error = AuthenticatedMessage::new(error_msg, auth_key)?;
                let error_json = serde_json::to_string(&authenticated_error)?;

                client.send_reply(sender_tag.clone(), error_json).await?;
                return Ok(());
            }

            // Register the new player
            let player_id = game_state.add_player(name, sender_tag);

            // Create a successful registration response with negotiated version
            let register_ack = ServerMessage::RegisterAck {
                player_id: player_id.clone(),
                seq_num: next_seq_num(),
                world_boundaries: WorldBoundaries::from_config(game_state.get_config()),
                negotiated_version,
            };

            let authenticated_ack = AuthenticatedMessage::new(register_ack, auth_key)?;
            let register_ack_json = serde_json::to_string(&authenticated_ack)?;

            // Send the registration confirmation to the new player
            client
                .send_reply(sender_tag.clone(), register_ack_json)
                .await?;

            // Broadcast updated game state to all players
            broadcast_game_state(client, game_state, None, auth_key).await?;

            info!(
                "New player registered: {} (protocol v{})",
                player_id, negotiated_version
            );
            Ok(())
        }
        ClientMessage::Move { direction, .. } => {
            handle_move(client, game_state, direction, sender_tag, auth_key).await
        }
        ClientMessage::Attack {
            target_display_id, ..
        } => handle_attack(client, game_state, target_display_id, sender_tag, auth_key).await,
        ClientMessage::Chat { message, .. } => {
            handle_chat(client, game_state, message, sender_tag, auth_key).await
        }
        ClientMessage::Emote { emote_type, .. } => {
            handle_emote(client, game_state, emote_type, sender_tag, auth_key).await
        }
        ClientMessage::Disconnect { seq_num } => {
            debug!("Processing disconnect message with seq_num: {}", seq_num);

            // Send acknowledgment first
            send_ack(
                client,
                &sender_tag,
                seq_num,
                ClientMessageType::Disconnect,
                auth_key,
            )
            .await?;

            // Handle the disconnection
            handle_disconnect(client, game_state, sender_tag, auth_key).await?;
            Ok(())
        }
        ClientMessage::Heartbeat { seq_num } => {
            debug!("Processing heartbeat message with seq_num: {}", seq_num);

            // Send acknowledgment first
            send_ack(
                client,
                &sender_tag,
                seq_num,
                ClientMessageType::Heartbeat,
                auth_key,
            )
            .await?;

            // Update the heartbeat timestamp
            handle_heartbeat(client, game_state, sender_tag, auth_key).await?;
            Ok(())
        }
        ClientMessage::Ack { .. } => {
            // Already handled above
            Ok(())
        }
        ClientMessage::Whisper {
            target_display_id,
            message,
            seq_num,
        } => {
            debug!("Processing whisper message with seq_num: {}", seq_num);

            // Send acknowledgment first
            send_ack(
                client,
                &sender_tag,
                seq_num,
                ClientMessageType::Whisper,
                auth_key,
            )
            .await?;

            // Handle the whisper message
            handle_whisper(
                client,
                game_state,
                target_display_id,
                message,
                sender_tag,
                auth_key,
            )
            .await
        }
    }
}

/// Handle a private message (whisper) between players
async fn handle_whisper(
    client: &MixnetClient,
    game_state: &Arc<GameState>,
    target_display_id: String,
    message: String,
    sender_tag: AnonymousSenderTag,
    auth_key: &AuthKey,
) -> Result<()> {
    // Find the player ID from sender tag
    let sender_id = match game_state.get_player_id(&sender_tag) {
        Some(id) => id,
        None => {
            // Player not found (not registered)
            let error_msg = ServerMessage::Error {
                message: "You must be registered to send whispers".to_string(),
                seq_num: next_seq_num(),
            };
            let authenticated_error = AuthenticatedMessage::new(error_msg, auth_key)?;
            let error_json = serde_json::to_string(&authenticated_error)?;
            client.send_reply(sender_tag, error_json).await?;
            return Ok(());
        }
    };

    // Get the sender's player data
    let sender_name = match game_state.get_player(&sender_id) {
        Some(player) => player.name.clone(),
        None => {
            // This shouldn't happen if get_player_id worked, but just in case
            return Ok(());
        }
    };

    // Find the target player by display ID
    let target_player_id = match game_state.get_player_id_by_display_id(&target_display_id) {
        Some(id) => id,
        None => {
            // Target player not found
            let error_msg = ServerMessage::Error {
                message: format!("Player '{}' not found", target_display_id),
                seq_num: next_seq_num(),
            };
            let authenticated_error = AuthenticatedMessage::new(error_msg, auth_key)?;
            let error_json = serde_json::to_string(&authenticated_error)?;
            client.send_reply(sender_tag, error_json).await?;
            return Ok(());
        }
    };

    // Get the target player's connection tag
    let target_tag = match game_state.get_connection_tag(&target_player_id) {
        Some(tag) => tag,
        None => {
            // Target player doesn't have an active connection
            let error_msg = ServerMessage::Error {
                message: format!("Player '{}' is not connected", target_display_id),
                seq_num: next_seq_num(),
            };
            let authenticated_error = AuthenticatedMessage::new(error_msg, auth_key)?;
            let error_json = serde_json::to_string(&authenticated_error)?;
            client.send_reply(sender_tag, error_json).await?;
            return Ok(());
        }
    };

    // Send the whisper message to the target player
    let whisper_msg = ServerMessage::WhisperMessage {
        sender_name,
        message,
        seq_num: next_seq_num(),
    };

    let authenticated_whisper = AuthenticatedMessage::new(whisper_msg, auth_key)?;
    let whisper_json = serde_json::to_string(&authenticated_whisper)?;
    client.send_reply(target_tag, whisper_json).await?;

    debug!(
        "Sent whisper from player {} to {}",
        sender_id, target_display_id
    );
    Ok(())
}

/// Handle player movement
async fn handle_move(
    client: &MixnetClient,
    game_state: &Arc<GameState>,
    direction: Direction,
    sender_tag: AnonymousSenderTag,
    auth_key: &AuthKey,
) -> Result<()> {
    // Find the player ID from sender tag
    if let Some(player_id) = game_state.get_player_id(&sender_tag) {
        // Get the current player information
        if let Some(player) = game_state.get_player(&player_id) {
            // Calculate movement vector
            let (dx, dy) = direction.to_vector();

            // Get config for movement parameters and boundaries
            let config = game_state.get_config();

            // Use the configured movement speed from server settings
            // This ensures consistency between server behavior and client configuration
            let movement_speed = config.movement_speed;

            // Apply movement vector to get new position
            let mut new_position = Position {
                x: player.position.x + dx * movement_speed,
                y: player.position.y + dy * movement_speed,
            };

            // Ensure the position stays within world boundaries
            let (clamped_x, clamped_y) = config.clamp_position(new_position.x, new_position.y);
            new_position.x = clamped_x;
            new_position.y = clamped_y;

            // Check for collisions with other players before updating position
            let collision_detected = {
                // Get all other players to check for collisions
                let other_players = game_state.get_all_players_except(&player_id);

                // Define minimum distance between players (based on the configured collision radius)
                let min_distance = config.player_collision_radius * 2.0; // Twice the radius for two players

                // Check if the new position would collide with any other player
                other_players.iter().any(|other_player| {
                    WorldBoundaries::would_positions_collide(
                        &new_position,
                        &other_player.position,
                        min_distance,
                    )
                })
            };

            if !collision_detected {
                // No collision detected, update the position
                if game_state.update_player_position(&player_id, new_position) {
                    // Movement was successful
                    // Provide immediate feedback to the player who moved
                    let move_confirm = ServerMessage::Event {
                        message: format!(
                            "Moved {:?} to position ({:.1}, {:.1})",
                            direction, new_position.x, new_position.y
                        ),
                        seq_num: next_seq_num(),
                    };

                    // Create an authenticated message
                    let authenticated_confirm = AuthenticatedMessage::new(move_confirm, auth_key)?;
                    let confirm_msg = serde_json::to_string(&authenticated_confirm)?;
                    client.send_reply(sender_tag.clone(), confirm_msg).await?;

                    // Broadcast updated state to all players
                    broadcast_game_state(client, game_state, None, auth_key).await?
                }
            } else {
                // Collision detected with another player
                let error_msg = ServerMessage::Error {
                    message: "Cannot move to that position - another player is already there"
                        .to_string(),
                    seq_num: next_seq_num(),
                };

                // Create an authenticated message
                let authenticated_error = AuthenticatedMessage::new(error_msg, auth_key)?;
                let message = serde_json::to_string(&authenticated_error)?;
                client.send_reply(sender_tag.clone(), message).await?;
            }
        } else {
            // Player not found
            let error_msg = ServerMessage::Error {
                message: "You need to register before moving".to_string(),
                seq_num: next_seq_num(),
            };

            // Create an authenticated message
            let authenticated_error = AuthenticatedMessage::new(error_msg, auth_key)?;
            let message = serde_json::to_string(&authenticated_error)?;
            client.send_reply(sender_tag.clone(), message).await?;
        }
    }

    Ok(())
}

/// Handle player attacks
async fn handle_attack(
    client: &MixnetClient,
    game_state: &Arc<GameState>,
    target_display_id: String,
    sender_tag: AnonymousSenderTag,
    auth_key: &AuthKey,
) -> Result<()> {
    // Find the attacker ID from sender tag
    if let Some(attacker_id) = game_state.get_player_id(&sender_tag) {
        // Convert target display ID to real ID
        let target_id = match game_state.get_player_id_by_display_id(&target_display_id) {
            Some(id) => id,
            None => {
                // Target display ID doesn't exist
                let error = ServerMessage::Error {
                    message: format!("Attack failed: Player '{}' not found.", target_display_id),
                    seq_num: next_seq_num(),
                };
                let message = serde_json::to_string(&error)?;
                client.send_reply(sender_tag.clone(), message).await?;
                return Ok(());
            }
        };

        info!(
            "Player {} attacking player with display ID {}",
            attacker_id, target_display_id
        );

        // Get current time
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        // Check if the player is on cooldown (using configuration)
        if !game_state.can_attack(&attacker_id, now) {
            // Get remaining cooldown time for better error message
            let remaining_cooldown = match game_state.get_player(&attacker_id) {
                Some(player) => {
                    let config = game_state.get_config();
                    let time_since_last = now.saturating_sub(player.last_attack_time);
                    config
                        .attack_cooldown_seconds
                        .saturating_sub(time_since_last)
                }
                None => 0,
            };

            // Send an error message if on cooldown
            let cooldown_msg = ServerMessage::Error {
                message: format!(
                    "Attack on cooldown! Wait {} more seconds.",
                    remaining_cooldown
                ),
                seq_num: next_seq_num(),
            };
            let message = serde_json::to_string(&cooldown_msg)?;
            client.send_reply(sender_tag.clone(), message).await?;
            return Ok(());
        }

        // Check if attacker and target are within range (using configuration)
        let attack_range = game_state.get_config().attack_range;

        // Get attacker's position
        let attacker_pos = match game_state.get_player(&attacker_id) {
            Some(player) => player.position,
            None => {
                // This shouldn't happen but handle it anyway
                let error = ServerMessage::Error {
                    message: "Attack failed: Unable to find your player.".to_string(),
                    seq_num: next_seq_num(),
                };
                let message = serde_json::to_string(&error)?;
                client.send_reply(sender_tag.clone(), message).await?;
                return Ok(());
            }
        };

        // Get target's position
        let target_pos = match game_state.get_player(&target_id) {
            Some(player) => player.position,
            None => {
                // Target doesn't exist
                let error = ServerMessage::Error {
                    message: "Attack failed: Target does not exist.".to_string(),
                    seq_num: next_seq_num(),
                };
                let message = serde_json::to_string(&error)?;
                client.send_reply(sender_tag.clone(), message).await?;
                return Ok(());
            }
        };

        // Calculate distance between attacker and target
        let distance = attacker_pos.distance_to(&target_pos);

        if distance > attack_range {
            // Target is out of range
            let error = ServerMessage::Error {
                message: format!(
                    "Attack failed: Target is out of range ({:.1} > {:.1}).",
                    distance, attack_range
                ),
                seq_num: next_seq_num(),
            };
            let message = serde_json::to_string(&error)?;
            client.send_reply(sender_tag.clone(), message).await?;
            return Ok(());
        }

        // Update the last attack time
        game_state.update_attack_time(&attacker_id, now);

        // Get target player's name and sender tag for notification
        let (target_name, target_tag) = {
            let connections = game_state.get_connections();
            let mut target_name = "Unknown".to_string();
            let mut target_tag = None;

            // Find the target's connection
            for (id, tag) in connections {
                if id == target_id {
                    if let Some(player) = game_state.get_player(&id) {
                        target_name = player.name.clone();
                    }
                    target_tag = Some(tag);
                    break;
                }
            }

            (target_name, target_tag)
        };

        // Get attacker's name for the notification
        let attacker_name = game_state
            .get_player(&attacker_id)
            .map(|p| p.name.clone())
            .unwrap_or("Unknown".to_string());

        // Apply damage
        let config = game_state.get_config();
        let base_damage = config.base_damage;
        let crit_chance = config.crit_chance;
        let crit_multiplier = config.crit_multiplier;

        // Calculate if this is a critical hit
        let mut rng = thread_rng();
        let is_critical = rng.gen::<f32>() < crit_chance;

        // Calculate final damage
        let damage = if is_critical {
            (base_damage as f32 * crit_multiplier) as u32
        } else {
            base_damage
        };

        // Apply damage and check if target was defeated
        let target_defeated = game_state.apply_damage(&target_id, damage);

        // Send notification to the target
        if let Some(tag) = target_tag {
            // Create attack notification for target player
            let attack_notification = ServerMessage::Event {
                message: format!(
                    "⚠️ You are being attacked by {}! You lost {} health points.",
                    attacker_name, damage
                ),
                seq_num: next_seq_num(),
            };
            let notification_msg = serde_json::to_string(&attack_notification)?;
            client.send_reply(tag.clone(), notification_msg).await?;
        }

        // Send notification to the attacker
        let attacker_notification = ServerMessage::Event {
            message: if target_defeated {
                format!(
                    "You defeated {}{}",
                    target_name,
                    if is_critical {
                        " with a critical hit!"
                    } else {
                        "!"
                    }
                )
            } else {
                format!(
                    "You hit {} for {} damage{}",
                    target_name,
                    damage,
                    if is_critical { " (CRITICAL HIT!)" } else { "" }
                )
            },
            seq_num: next_seq_num(),
        };

        // Create an authenticated message
        let authenticated_notification =
            AuthenticatedMessage::new(attacker_notification, auth_key)?;
        let attacker_msg = serde_json::to_string(&authenticated_notification)?;
        client.send_reply(sender_tag.clone(), attacker_msg).await?;

        // This event is now sent in the move handler itself, we don't need to send it again here

        // Broadcast the updated game state to all players
        broadcast_game_state(client, game_state, None, auth_key).await?;
    }

    Ok(())
}

/// Handle emote messages
async fn handle_emote(
    client: &MixnetClient,
    game_state: &Arc<GameState>,
    emote_type: EmoteType,
    sender_tag: AnonymousSenderTag,
    auth_key: &AuthKey,
) -> Result<()> {
    // Find the player ID from sender tag
    if let Some(sender_id) = game_state.get_player_id(&sender_tag) {
        // Get the player's name
        if let Some(player) = game_state.get_player(&sender_id) {
            let sender_name = player.name.clone();

            // Create the emote message to broadcast to other players
            let emote_msg = format!(
                "{} {} {}",
                emote_type.display_icon(),
                sender_name,
                emote_type.display_text()
            );

            // Create a chat-like message with the emote
            let chat_msg = ServerMessage::ChatMessage {
                sender_name: "Emote".to_string(), // Special sender name for emotes
                message: emote_msg,
                seq_num: next_seq_num(),
            };

            // Create an authenticated chat message
            let authenticated_chat = AuthenticatedMessage::new(chat_msg, auth_key)?;
            let serialized = serde_json::to_string(&authenticated_chat)?;

            // Create confirmation message for the sender
            let confirm_msg = ServerMessage::Event {
                message: format!("You {}", emote_type.display_text()),
                seq_num: next_seq_num(),
            };

            // Create an authenticated confirmation message
            let authenticated_confirm = AuthenticatedMessage::new(confirm_msg, auth_key)?;
            let confirm_serialized = serde_json::to_string(&authenticated_confirm)?;

            // Send confirmation to the original sender
            if let Err(e) = client
                .send_reply(sender_tag.clone(), confirm_serialized)
                .await
            {
                error!(
                    "Failed to send emote confirmation to sender {}: {}",
                    sender_id, e
                );
            } else {
                debug!("Emote confirmation sent to sender {}", sender_id);
            }

            // Get a copy of all active connections
            let connections = game_state.get_connections();
            debug!("Broadcasting emote to {} players", connections.len());

            // Prepare exclude tag as bytes for more reliable comparison
            let exclude_bytes = sender_tag.to_string().into_bytes();

            // Broadcast emote to all other players
            for (player_id, tag) in connections {
                // Skip sending to the original sender by comparing the tag bytes
                if tag.to_string().into_bytes() != exclude_bytes {
                    match client.send_reply(tag.clone(), serialized.clone()).await {
                        Ok(_) => {
                            trace!("Emote message sent to player {}", player_id);
                        }
                        Err(e) => {
                            error!(
                                "Failed to send emote message to player {}: {}",
                                player_id, e
                            );
                        }
                    }
                }
            }

            info!("Emote from {}: {}", sender_name, emote_type.display_text());
        }
    }

    Ok(())
}

/// Handle chat messages
async fn handle_chat(
    client: &MixnetClient,
    game_state: &Arc<GameState>,
    message: String,
    sender_tag: AnonymousSenderTag,
    auth_key: &AuthKey,
) -> Result<()> {
    // Find the player ID from sender tag
    if let Some(sender_id) = game_state.get_player_id(&sender_tag) {
        // Get the player's name
        if let Some(player) = game_state.get_player(&sender_id) {
            let sender_name = player.name.clone();

            // Create the chat message for other players
            let chat_msg = ServerMessage::ChatMessage {
                sender_name: sender_name.clone(),
                message: message.clone(),
                seq_num: next_seq_num(),
            };

            // Create an authenticated chat message
            let authenticated_chat = AuthenticatedMessage::new(chat_msg, auth_key)?;
            let serialized = serde_json::to_string(&authenticated_chat)?;

            // Create confirmation message for the sender
            let confirm_msg = ServerMessage::Event {
                message: format!("Your message has been sent: {}", message),
                seq_num: next_seq_num(),
            };

            // Create an authenticated confirmation message
            let authenticated_confirm = AuthenticatedMessage::new(confirm_msg, auth_key)?;
            let confirm_serialized = serde_json::to_string(&authenticated_confirm)?;

            // Send confirmation to the original sender
            if let Err(e) = client
                .send_reply(sender_tag.clone(), confirm_serialized)
                .await
            {
                error!("Failed to send confirmation to sender {}: {}", sender_id, e);
            } else {
                info!("Confirmation sent to sender {}", sender_id);
            }

            // Get a copy of all active connections
            let connections = game_state.get_connections();
            info!("Broadcasting chat to {} players", connections.len());

            // Prepare exclude tag as bytes for more reliable comparison
            let exclude_bytes = sender_tag.to_string().into_bytes();

            for (player_id, tag) in connections {
                // Skip sending to the original sender by comparing the tag bytes
                if tag.to_string().into_bytes() != exclude_bytes {
                    match client.send_reply(tag.clone(), serialized.clone()).await {
                        Ok(_) => {
                            info!("Chat message sent to player {}", player_id);
                        }
                        Err(e) => {
                            error!("Failed to send chat message to player {}: {}", player_id, e);
                        }
                    }
                }
            }

            info!("Chat message from {}: {}", sender_name, message);
        }
    }

    Ok(())
}

/// Handle player disconnection
async fn handle_disconnect(
    client: &MixnetClient,
    game_state: &Arc<GameState>,
    sender_tag: AnonymousSenderTag,
    auth_key: &AuthKey,
) -> Result<()> {
    // Remove the player
    if let Some(player_id) = game_state.remove_player(&sender_tag) {
        info!("Player {} disconnected", player_id);

        // Broadcast the updated game state to all remaining players
        broadcast_game_state(client, game_state, None, auth_key).await?;
    }

    Ok(())
}

/// Handle heartbeat messages
async fn handle_heartbeat(
    _client: &MixnetClient,
    game_state: &Arc<GameState>,
    sender_tag: AnonymousSenderTag,
    _auth_key: &AuthKey,
) -> Result<()> {
    // Find the player ID from sender tag
    if let Some(player_id) = game_state.get_player_id(&sender_tag) {
        // Update the heartbeat timestamp for this player
        game_state.update_heartbeat(&player_id);

        debug!("Heartbeat received from player {}", player_id);
    } else {
        debug!("Received heartbeat from unregistered player");
    }

    Ok(())
}

/// Send heartbeat request to all connected players
pub async fn send_heartbeat_requests(
    client: &MixnetClient,
    game_state: &Arc<GameState>,
    auth_key: &AuthKey,
) -> Result<()> {
    let connections = game_state.get_connections();

    if connections.is_empty() {
        return Ok(());
    }

    let heartbeat_request = ServerMessage::HeartbeatRequest {
        seq_num: next_seq_num(),
    };

    let authenticated_request = AuthenticatedMessage::new(heartbeat_request, auth_key)?;
    let serialized = serde_json::to_string(&authenticated_request)?;

    debug!(
        "Sending heartbeat requests to {} players",
        connections.len()
    );

    for (player_id, tag) in connections {
        match client.send_reply(tag.clone(), serialized.clone()).await {
            Ok(_) => {
                trace!("Heartbeat request sent to player {}", player_id);
            }
            Err(e) => {
                warn!(
                    "Failed to send heartbeat request to player {}: {}",
                    player_id, e
                );
            }
        }
    }

    Ok(())
}

/// Check for inactive players and remove them from the game
pub async fn cleanup_inactive_players(
    client: &MixnetClient,
    game_state: &Arc<GameState>,
    auth_key: &AuthKey,
) -> Result<()> {
    let inactive_players = game_state.get_inactive_players();

    if inactive_players.is_empty() {
        return Ok(());
    }

    info!(
        "Found {} inactive players to remove",
        inactive_players.len()
    );

    // Remove the inactive players
    let removed_players = game_state.remove_players_by_ids(&inactive_players);

    if !removed_players.is_empty() {
        info!("Removed {} inactive players", removed_players.len());

        // Broadcast updated game state to remaining players
        broadcast_game_state(client, game_state, None, auth_key).await?;
    }

    Ok(())
}
