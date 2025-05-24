use anyhow::Result;
use nym_sdk::mixnet::{MixnetClient, AnonymousSenderTag, MixnetMessageSender, Recipient};
use std::sync::{Arc, Mutex};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};
use std::collections::{HashMap, HashSet};
use rand::{thread_rng, Rng};
use tracing::{info, warn, error, debug, trace};

use crate::message_auth::{AuthKey, AuthenticatedMessage};

use crate::game_protocol::{ClientMessage, ServerMessage, Direction, Position, ClientMessageType, ServerMessageType, WorldBoundaries, EmoteType};
use crate::game_state::GameState;
use crate::config::GameConfig;

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
        let bucket = self.buckets.entry(connection_id.to_string())
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

// Structure to manage replay protection using a sliding window approach
struct ReplayProtectionWindow {
    // Highest sequence number seen so far
    highest_seq: u64,
    // Bitmap window to track received sequence numbers below the highest
    // Each bit represents whether we've seen (highest_seq - bit_position) 
    window: u128,
    // Window size - how many previous sequence numbers we track
    window_size: u8,
}

impl ReplayProtectionWindow {
    // Create a new replay protection window
    fn new(window_size: u8) -> Self {
        // window_size should be at most 128 (size of u128 in bits)
        let window_size = std::cmp::min(window_size, 128);
        ReplayProtectionWindow {
            highest_seq: 0,
            window: 0,
            window_size,
        }
    }
    
    // Process a sequence number and determine if it's a replay
    // Returns true if the message is a replay, false if it's new
    fn process(&mut self, seq_num: u64) -> bool {
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

// Tracking received client messages to prevent replays
lazy_static::lazy_static! {
    static ref REPLAY_PROTECTION: Mutex<HashMap<String, ReplayProtectionWindow>> = Mutex::new(HashMap::new());
}

// Check if we've seen this message before (replay protection)
fn is_message_replay(tag: &AnonymousSenderTag, seq_num: u64) -> bool {
    let tag_str = tag.to_string();
    match REPLAY_PROTECTION.lock() {
        Ok(mut protection) => {
            // Get or create the replay protection window for this client
            let window = protection.entry(tag_str).or_insert_with(|| {
                // Window size of 64 means we track the last 64 sequence numbers
                ReplayProtectionWindow::new(64)
            });
            
            // Check and update the window
            window.process(seq_num)
        },
        Err(e) => {
            error!("Warning: Failed to access replay protection data: {}", e);
            // In case of mutex poisoning, err on the side of caution and allow the message
            false
        }
    }
}

/// Send an acknowledgment message back to the client
async fn send_ack(
    client: &MixnetClient,
    sender_tag: &AnonymousSenderTag,
    seq_num: u64,
    msg_type: ClientMessageType,
    auth_key: &AuthKey
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

/// Broadcast game state to all active players
pub async fn broadcast_game_state(
    client: &MixnetClient,
    game_state: &Arc<GameState>,
    exclude_tag: Option<AnonymousSenderTag>,
    auth_key: &AuthKey
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

/// Global rate limiter instance for DoS protection
/// Uses lazy_static to ensure thread-safe initialization
lazy_static::lazy_static! {
    static ref GLOBAL_RATE_LIMITER: Arc<Mutex<Option<RateLimiter>>> = Arc::new(Mutex::new(None));
}

/// Initialize the global rate limiter with game configuration
/// This should be called once during server startup
pub fn init_rate_limiter(config: &GameConfig) {
    let mut limiter = GLOBAL_RATE_LIMITER.lock().unwrap();
    *limiter = Some(RateLimiter::new(config));
    info!("Rate limiter initialized: {:.1} msg/sec, burst: {}", 
          config.message_rate_limit, config.message_burst_size);
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
    auth_key: &AuthKey
) -> Result<()> {
    // Check rate limit
    if let Ok(mut limiter_guard) = GLOBAL_RATE_LIMITER.lock() {
        if let Some(ref mut limiter) = *limiter_guard {
            if !limiter.check_rate_limit(&sender_tag.to_string()) {
                warn!("Rate limit exceeded for connection {}", sender_tag);
                
                // Send rate limit error to client
                let error_msg = ServerMessage::Error { 
                    message: "Rate limit exceeded. Please slow down your message frequency.".to_string(),
                    seq_num: next_seq_num()
                };
                let authenticated_response = AuthenticatedMessage::new(error_msg, auth_key)?;
                let response_str = serde_json::to_string(&authenticated_response)?;
                
                // Send reply to the rate-limited client
                let _ = client.send_reply(sender_tag.clone(), response_str).await;
                
                return Ok(());
            }
        }
    }
    
    // Get sequence number for replay protection and acknowledgments
    let seq_num = message.get_seq_num();
    
    // Handle acknowledgments separately and directly
    if let ClientMessage::Ack { .. } = &message {
        // We don't need to do anything with acks in this simple implementation
        // In a more complex system, we might track which messages were acknowledged
        return Ok(());
    }
    
    // Check for message replay, but only for non-ack messages
    if is_message_replay(&sender_tag, seq_num) {
        warn!("Detected replay attack or duplicate message: seq {} from {:?}", seq_num, sender_tag);
        return Ok(());
    }
    
    // Send acknowledgment first for all non-ack messages
    send_ack(client, &sender_tag, seq_num, message.get_type(), auth_key).await?;
    
    // Process the message based on its type
    match message {
        ClientMessage::Register { name, .. } => {
            handle_register(client, game_state, name, sender_tag, auth_key, game_state.get_config()).await
        },
        ClientMessage::Move { direction, .. } => {
            handle_move(client, game_state, direction, sender_tag, auth_key).await
        },
        ClientMessage::Attack { target_display_id, .. } => {
            handle_attack(client, game_state, target_display_id, sender_tag, auth_key).await
        },
        ClientMessage::Chat { message, .. } => {
            handle_chat(client, game_state, message, sender_tag, auth_key).await
        },
        ClientMessage::Emote { emote_type, .. } => {
            handle_emote(client, game_state, emote_type, sender_tag, auth_key).await
        },
        ClientMessage::Disconnect { seq_num } => {
            debug!("Processing disconnect message with seq_num: {}", seq_num);
            
            // Send acknowledgment first
            send_ack(client, &sender_tag, seq_num, ClientMessageType::Disconnect, auth_key).await?;
            
            // Handle the disconnection
            handle_disconnect(client, game_state, sender_tag, auth_key).await?;
            Ok(())
        },
        ClientMessage::Heartbeat { seq_num } => {
            debug!("Processing heartbeat message with seq_num: {}", seq_num);
            
            // Send acknowledgment first
            send_ack(client, &sender_tag, seq_num, ClientMessageType::Heartbeat, auth_key).await?;
            
            // Update the heartbeat timestamp
            handle_heartbeat(client, game_state, sender_tag, auth_key).await?;
            Ok(())
        },
        ClientMessage::Ack { .. } => {
            // Already handled above
            Ok(())
        },
    }
}

/// Handle player registration
async fn handle_register(
    client: &MixnetClient,
    game_state: &Arc<GameState>,
    name: String,
    sender_tag: AnonymousSenderTag,
    auth_key: &AuthKey,
    config: &GameConfig,
) -> Result<()> {
    // Check if this sender_tag is already associated with a registered player
    if let Some(existing_player_id) = game_state.get_player_id(&sender_tag) {
        // Client is already registered, send an error message
        let error_msg = ServerMessage::Error { 
            message: "You are already registered. Please disconnect first before registering again.".to_string(),
            seq_num: next_seq_num()
        };
        
        // Create an authenticated message
        let authenticated_error = AuthenticatedMessage::new(error_msg, auth_key)?;
        let error_json = serde_json::to_string(&authenticated_error)?;
        
        // Send the error message to the client
        client.send_reply(sender_tag.clone(), error_json).await?;
        
        info!("Registration attempt rejected: Client already registered as {}", existing_player_id);
        return Ok(());
    }
    
    // Add the player to the game state
    let player_id = game_state.add_player(name, sender_tag.clone());
    
    // Create a welcome message for this player
    let register_ack = ServerMessage::RegisterAck {
        player_id: player_id.clone(),
        seq_num: next_seq_num(),
        world_boundaries: WorldBoundaries::from_config(config),
    };
    
    // Create an authenticated message
    let authenticated_ack = AuthenticatedMessage::new(register_ack, auth_key)?;
    let register_ack_json = serde_json::to_string(&authenticated_ack)?;
    
    // Send the registration confirmation to the new player
    client.send_reply(sender_tag.clone(), register_ack_json).await?;
    
    // Broadcast updated game state to all players
    broadcast_game_state(client, game_state, None, auth_key).await?;
    
    info!("New player registered: {}", player_id);
    Ok(())
}

/// Handle player movement
async fn handle_move(
    client: &MixnetClient,
    game_state: &Arc<GameState>,
    direction: Direction,
    sender_tag: AnonymousSenderTag,
    auth_key: &AuthKey
) -> Result<()> {
    // Find the player ID from sender tag
    if let Some(player_id) = game_state.get_player_id(&sender_tag) {
        // Get the current player information
        if let Some(player) = game_state.get_player(&player_id) {
            // Calculate movement vector
            let (dx, dy) = direction.to_vector();
            
            // Calculate speed needed to move exactly one cell in the mini-map
            // The mini-map is 15x15 with world boundaries of -100 to 100, so each cell represents about 14 units
            let mini_map_cell_size = 14.0;
            
            // Apply movement vector to get new position
            let mut new_position = Position {
                x: player.position.x + dx * mini_map_cell_size,
                y: player.position.y + dy * mini_map_cell_size,
            };
            
            // Ensure the position stays within world boundaries
            let config = game_state.get_config();
            let (clamped_x, clamped_y) = config.clamp_position(new_position.x, new_position.y);
            new_position.x = clamped_x;
            new_position.y = clamped_y;
            
            // Try to update position
            if game_state.update_player_position(&player_id, new_position) {
                // Movement was successful
                // Provide immediate feedback to the player who moved
                let move_confirm = ServerMessage::Event { 
                    message: format!("Moved {:?} to position ({:.1}, {:.1})", direction, new_position.x, new_position.y),
                    seq_num: next_seq_num()
                };
                
                // Create an authenticated message
                let authenticated_confirm = AuthenticatedMessage::new(move_confirm, auth_key)?;
                let confirm_msg = serde_json::to_string(&authenticated_confirm)?;
                client.send_reply(sender_tag.clone(), confirm_msg).await?;
                
                // Broadcast updated state to all players
                broadcast_game_state(client, game_state, None, auth_key).await?
            } else {
                // Movement failed (collision with another player or obstacle)
                let error_msg = ServerMessage::Error { 
                    message: "Cannot move to that position - there's an obstacle or another player there".to_string(),
                    seq_num: next_seq_num()
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
    auth_key: &AuthKey
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
        
        info!("Player {} attacking player with display ID {}", attacker_id, target_display_id);
        
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
                    config.attack_cooldown_seconds.saturating_sub(time_since_last)
                },
                None => 0,
            };
            
            // Send an error message if on cooldown
            let cooldown_msg = ServerMessage::Error { 
                message: format!("Attack on cooldown! Wait {} more seconds.", remaining_cooldown),
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
                message: format!("Attack failed: Target is out of range ({:.1} > {:.1}).", distance, attack_range),
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
        let attacker_name = game_state.get_player(&attacker_id)
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
                message: format!("⚠️ You are being attacked by {}! You lost {} health points.", attacker_name, damage),
                seq_num: next_seq_num(),
            };
            let notification_msg = serde_json::to_string(&attack_notification)?;
            client.send_reply(tag, notification_msg).await?;
        }
        
        // Send notification to the attacker
        let attacker_notification = ServerMessage::Event {
            message: if target_defeated {
                format!("You defeated {}{}", 
                    target_name, 
                    if is_critical { " with a critical hit!" } else { "!" })
            } else {
                format!("You hit {} for {} damage{}", 
                    target_name, 
                    damage, 
                    if is_critical { " (CRITICAL HIT!)" } else { "" })
            },
            seq_num: next_seq_num(),
        };
        
        // Create an authenticated message
        let authenticated_notification = AuthenticatedMessage::new(attacker_notification, auth_key)?;
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
    auth_key: &AuthKey
) -> Result<()> {
    // Find the player ID from sender tag
    if let Some(sender_id) = game_state.get_player_id(&sender_tag) {
        // Get the player's name
        if let Some(player) = game_state.get_player(&sender_id) {
            let sender_name = player.name.clone();
            
            // Create the emote message to broadcast to other players
            let emote_msg = format!("{} {}", emote_type.display_icon(), format!("{} {}", sender_name, emote_type.display_text()));
            
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
            if let Err(e) = client.send_reply(sender_tag.clone(), confirm_serialized).await {
                error!("Failed to send emote confirmation to sender {}: {}", sender_id, e);
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
                        },
                        Err(e) => {
                            error!("Failed to send emote message to player {}: {}", player_id, e);
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
    auth_key: &AuthKey
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
            if let Err(e) = client.send_reply(sender_tag.clone(), confirm_serialized).await {
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
                        },
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
    auth_key: &AuthKey
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
    client: &MixnetClient,
    game_state: &Arc<GameState>,
    sender_tag: AnonymousSenderTag,
    auth_key: &AuthKey
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
    auth_key: &AuthKey
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
    
    debug!("Sending heartbeat requests to {} players", connections.len());
    
    for (player_id, tag) in connections {
        match client.send_reply(tag, serialized.clone()).await {
            Ok(_) => {
                trace!("Heartbeat request sent to player {}", player_id);
            },
            Err(e) => {
                warn!("Failed to send heartbeat request to player {}: {}", player_id, e);
            }
        }
    }
    
    Ok(())
}

/// Check for inactive players and remove them from the game
pub async fn cleanup_inactive_players(
    client: &MixnetClient,
    game_state: &Arc<GameState>,
    auth_key: &AuthKey
) -> Result<()> {
    let inactive_players = game_state.get_inactive_players();
    
    if inactive_players.is_empty() {
        return Ok(());
    }
    
    info!("Found {} inactive players to remove", inactive_players.len());
    
    // Remove the inactive players
    let removed_players = game_state.remove_players_by_ids(&inactive_players);
    
    if !removed_players.is_empty() {
        info!("Removed {} inactive players", removed_players.len());
        
        // Broadcast updated game state to remaining players
        broadcast_game_state(client, game_state, None, auth_key).await?;
    }
    
    Ok(())
}
