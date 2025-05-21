use anyhow::Result;
use nym_sdk::mixnet::{MixnetClient, AnonymousSenderTag, MixnetMessageSender};
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};
use std::collections::{HashMap, HashSet};
use rand::{thread_rng, Rng};

use crate::game_protocol::{ClientMessage, ServerMessage, Direction, Position, ClientMessageType, ServerMessageType};
use crate::game_state::GameState;

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

// Global sequence number for server messages
static mut SERVER_SEQ_NUM: u64 = 1;

// Thread-safe function to get the next server sequence number
fn next_seq_num() -> u64 {
    unsafe {
        let num = SERVER_SEQ_NUM;
        SERVER_SEQ_NUM += 1;
        num
    }
}

// Tracking received client messages to prevent replays
lazy_static::lazy_static! {
    static ref RECEIVED_CLIENT_MSGS: Mutex<HashMap<String, HashSet<u64>>> = Mutex::new(HashMap::new());
}

// Check if we've seen this message before (replay protection)
fn is_message_replay(tag: &AnonymousSenderTag, seq_num: u64) -> bool {
    let tag_str = tag.to_string();
    let mut received = RECEIVED_CLIENT_MSGS.lock().unwrap();
    
    // Get or create the set for this client
    let client_msgs = received.entry(tag_str).or_insert_with(HashSet::new);
    
    // Check if we've seen this sequence number
    if client_msgs.contains(&seq_num) {
        true
    } else {
        // Record this sequence number
        client_msgs.insert(seq_num);
        
        // Prune old sequence numbers to avoid unbounded growth
        if client_msgs.len() > 1000 {
            // Keep only the 500 highest sequence numbers
            let mut seqs: Vec<_> = client_msgs.iter().cloned().collect();
            seqs.sort_unstable_by(|a, b| b.cmp(a)); // Descending order
            seqs.truncate(500);
            
            // Create a new set with just these sequence numbers
            *client_msgs = seqs.into_iter().collect();
        }
        
        false
    }
}

/// Send an acknowledgment for a client message
async fn send_ack(
    client: &MixnetClient,
    sender_tag: &AnonymousSenderTag,
    client_message: &ClientMessage
) -> Result<()> {
    // Create acknowledgment
    let ack = ServerMessage::Ack {
        client_seq_num: client_message.get_seq_num(),
        original_type: client_message.get_type(),
    };
    
    // Serialize and send
    let message = serde_json::to_string(&ack)?;
    client.send_reply(sender_tag.clone(), message).await?;
    
    Ok(())
}

/// Broadcast game state to all active players
pub async fn broadcast_game_state(
    client: &MixnetClient,
    game_state: &Arc<GameState>,
    exclude_tag: Option<AnonymousSenderTag>
) -> Result<()> {
    // Get the current game state
    let players = game_state.get_players();
    
    // Create game state message with sequence number
    let game_state_msg = ServerMessage::GameState { 
        players,
        seq_num: next_seq_num(),
    };
    
    let message = serde_json::to_string(&game_state_msg)?;
    
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
        if let Err(e) = client.send_reply(tag.clone(), message.clone()).await {
            println!("Failed to send game state to player {}: {}", player_id, e);
            failed_tags.push(tag);
        }
    }
    
    // Clean up any players that we couldn't reach
    for tag in failed_tags {
        if let Some(player_id) = game_state.remove_player(&tag) {
            println!("Removed unreachable player: {}", player_id);
        }
    }
    
    Ok(())
}

/// Handle a message from a client
pub async fn handle_client_message(
    client: &MixnetClient,
    game_state: &Arc<GameState>,
    message: ClientMessage,
    sender_tag: AnonymousSenderTag
) -> Result<()> {
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
        println!("Detected replay attack or duplicate message: seq {} from {:?}", seq_num, sender_tag);
        return Ok(());
    }
    
    // Send acknowledgment first for all non-ack messages
    send_ack(client, &sender_tag, &message).await?;
    
    // Process the message based on its type
    match message {
        ClientMessage::Register { name, .. } => {
            handle_register(client, game_state, name, sender_tag).await
        },
        ClientMessage::Move { direction, .. } => {
            handle_move(client, game_state, direction, sender_tag).await
        },
        ClientMessage::Attack { target_display_id, .. } => {
            handle_attack(client, game_state, target_display_id, sender_tag).await
        },
        ClientMessage::Chat { message, .. } => {
            handle_chat(client, game_state, message, sender_tag).await
        },
        ClientMessage::Disconnect { .. } => {
            handle_disconnect(client, game_state, sender_tag).await
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
    sender_tag: AnonymousSenderTag
) -> Result<()> {
    // Add the player to the game state
    let player_id = game_state.add_player(name, sender_tag.clone());
    
    // Send registration confirmation with sequence number
    let register_ack = ServerMessage::RegisterAck { 
        player_id: player_id.clone(), 
        seq_num: next_seq_num(),
    };
    
    let message = serde_json::to_string(&register_ack)?;
    client.send_reply(sender_tag.clone(), message).await?;
    
    // Broadcast updated game state to all players
    broadcast_game_state(client, game_state, None).await?;
    
    println!("New player registered: {}", player_id);
    Ok(())
}

/// Handle player movement
async fn handle_move(
    client: &MixnetClient,
    game_state: &Arc<GameState>,
    direction: Direction,
    sender_tag: AnonymousSenderTag
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
            new_position.x = new_position.x.clamp(-100.0, 100.0);
            new_position.y = new_position.y.clamp(-100.0, 100.0);
            
            // Try to update position
            if game_state.update_player_position(&player_id, new_position) {
                // Movement was successful
                // Provide immediate feedback to the player who moved
                let move_confirm = ServerMessage::Event { 
                    message: format!("Moved {:?} to position ({:.1}, {:.1})", direction, new_position.x, new_position.y),
                    seq_num: next_seq_num()
                };
                let confirm_msg = serde_json::to_string(&move_confirm)?;
                client.send_reply(sender_tag.clone(), confirm_msg).await?;
                
                // Broadcast updated state to all players
                broadcast_game_state(client, game_state, None).await?
            } else {
                // Movement failed (collision with another player or obstacle)
                let error_msg = ServerMessage::Error { 
                    message: "Cannot move to that position - there's an obstacle or another player there".to_string(),
                    seq_num: next_seq_num()
                };
                let message = serde_json::to_string(&error_msg)?;
                client.send_reply(sender_tag.clone(), message).await?;
            }
        } else {
            // Player not found
            let error_msg = ServerMessage::Error { 
                message: "You need to register before moving".to_string(),
                seq_num: next_seq_num(),
            };
            let message = serde_json::to_string(&error_msg)?;
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
    sender_tag: AnonymousSenderTag
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
        
        println!("Player {} attacking player with display ID {}", attacker_id, target_display_id);
        
        // Get current time
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
            
        // Cooldown in seconds
        const ATTACK_COOLDOWN: u64 = 3;
        
        // Check if the player is on cooldown
        if !game_state.can_attack(&attacker_id, now, ATTACK_COOLDOWN) {
            // Send an error message if on cooldown
            let cooldown_msg = ServerMessage::Error { 
                message: format!("Attack on cooldown! Wait {} more seconds.", ATTACK_COOLDOWN - 
                    (now - game_state.get_player(&attacker_id)
                        .map_or(0, |p| p.last_attack_time))),
                seq_num: next_seq_num(),
            };
            let message = serde_json::to_string(&cooldown_msg)?;
            client.send_reply(sender_tag.clone(), message).await?;
            return Ok(());
        }
        
        // Check if attacker and target are within range
        const ATTACK_RANGE: f32 = 28.0; // Maximum attack range (2 cases * 14.0 units per case)
        
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
        
        if distance > ATTACK_RANGE {
            // Target is out of range
            let error = ServerMessage::Error { 
                message: format!("Attack failed: Target is out of range ({:.1} > {:.1}).", distance, ATTACK_RANGE),
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
        const BASE_DAMAGE: u32 = 10; // Base damage amount
        const CRIT_CHANCE: f32 = 0.15; // 15% chance for critical hit
        const CRIT_MULTIPLIER: f32 = 2.0; // Critical hits do 2x damage
        
        // Calculate if this is a critical hit
        let mut rng = thread_rng();
        let is_critical = rng.gen::<f32>() < CRIT_CHANCE;
        
        // Calculate final damage
        let damage = if is_critical {
            (BASE_DAMAGE as f32 * CRIT_MULTIPLIER) as u32
        } else {
            BASE_DAMAGE
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
        let attacker_msg = serde_json::to_string(&attacker_notification)?;
        client.send_reply(sender_tag.clone(), attacker_msg).await?;
        
        // This event is now sent in the move handler itself, we don't need to send it again here
        
        // Broadcast the updated game state to all players
        broadcast_game_state(client, game_state, None).await?;
    }
    
    Ok(())
}

/// Handle chat messages
async fn handle_chat(
    client: &MixnetClient,
    game_state: &Arc<GameState>,
    message: String,
    sender_tag: AnonymousSenderTag
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
            let serialized = serde_json::to_string(&chat_msg)?;
            
            // Create confirmation message for the sender
            let confirm_msg = ServerMessage::Event {
                message: format!("Your message has been sent: {}", message),
                seq_num: next_seq_num(),
            };
            let confirm_serialized = serde_json::to_string(&confirm_msg)?;
            
            // Send confirmation to the original sender
            if let Err(e) = client.send_reply(sender_tag.clone(), confirm_serialized).await {
                println!("Failed to send confirmation to sender {}: {}", sender_id, e);
            } else {
                println!("Confirmation sent to sender {}", sender_id);
            }
            
            // Send to all other players
            let connections = game_state.get_connections();
            println!("Broadcasting chat to {} players", connections.len());
            
            // Get sender tag as bytes for more reliable comparison
            let sender_tag_bytes = sender_tag.to_string().into_bytes();
            
            for (player_id, tag) in connections {
                // Skip sending to the original sender by comparing the tag bytes
                if tag.to_string().into_bytes() != sender_tag_bytes {
                    match client.send_reply(tag.clone(), serialized.clone()).await {
                        Ok(_) => {
                            println!("Chat message sent to player {}", player_id);
                        },
                        Err(e) => {
                            println!("Failed to send chat message to player {}: {}", player_id, e);
                        }
                    }
                }
            }
            
            println!("Chat message from {}: {}", sender_name, message);
        }
    }
    
    Ok(())
}

/// Handle player disconnection
async fn handle_disconnect(
    client: &MixnetClient,
    game_state: &Arc<GameState>,
    sender_tag: AnonymousSenderTag
) -> Result<()> {
    // Remove the player
    if let Some(player_id) = game_state.remove_player(&sender_tag) {
        println!("Player {} disconnected", player_id);
        
        // Broadcast the updated game state to all remaining players
        broadcast_game_state(client, game_state, None).await?;
    }
    
    Ok(())
}
