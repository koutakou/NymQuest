use anyhow::Result;
use nym_sdk::mixnet::{MixnetClient, AnonymousSenderTag, MixnetMessageSender};
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::game_protocol::{ClientMessage, ServerMessage, Direction, Position};
use crate::game_state::GameState;

/// Broadcast game state to all active players
pub async fn broadcast_game_state(
    client: &MixnetClient,
    game_state: &Arc<GameState>,
    exclude_tag: Option<AnonymousSenderTag>
) -> Result<()> {
    // Get the current game state
    let players = game_state.get_players();
    let game_state_msg = ServerMessage::GameState { players };
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
    match message {
        ClientMessage::Register { name } => {
            handle_register(client, game_state, name, sender_tag).await
        },
        ClientMessage::Move { direction } => {
            handle_move(client, game_state, direction, sender_tag).await
        },
        ClientMessage::Attack { target_id } => {
            handle_attack(client, game_state, target_id, sender_tag).await
        },
        ClientMessage::Chat { message } => {
            handle_chat(client, game_state, message, sender_tag).await
        },
        ClientMessage::Disconnect => {
            handle_disconnect(client, game_state, sender_tag).await
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
    
    // Send registration confirmation
    let register_ack = ServerMessage::RegisterAck { player_id: player_id.clone() };
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
                    message: format!("Moved {:?} to position ({:.1}, {:.1})", direction, new_position.x, new_position.y) 
                };
                let confirm_msg = serde_json::to_string(&move_confirm)?;
                client.send_reply(sender_tag.clone(), confirm_msg).await?;
                
                // Broadcast updated state to all players
                broadcast_game_state(client, game_state, None).await?
            } else {
                // Movement failed (collision with another player or obstacle)
                let error_msg = ServerMessage::Error { 
                    message: "Cannot move to that position - there's an obstacle or another player there".to_string() 
                };
                let message = serde_json::to_string(&error_msg)?;
                client.send_reply(sender_tag.clone(), message).await?
            }
        }
    }
    
    Ok(())
}

/// Handle player attacks
async fn handle_attack(
    client: &MixnetClient,
    game_state: &Arc<GameState>,
    target_id: String,
    sender_tag: AnonymousSenderTag
) -> Result<()> {
    // Find the attacker ID from sender tag
    if let Some(attacker_id) = game_state.get_player_id(&sender_tag) {
        // Get current time for cooldown check
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        
        // Define attack cooldown
        const ATTACK_COOLDOWN: u64 = 3;
        
        // Check if the player can attack (not on cooldown)
        if !game_state.can_attack(&attacker_id, now, ATTACK_COOLDOWN) {
            // Player is still on cooldown
            let remaining = ATTACK_COOLDOWN - 
                (now - game_state.get_player(&attacker_id)
                    .map_or(0, |p| p.last_attack_time));
            
            let cooldown_msg = ServerMessage::Error { 
                message: format!("Attack on cooldown! Wait {} more seconds.", remaining) 
            };
            let message = serde_json::to_string(&cooldown_msg)?;
            client.send_reply(sender_tag.clone(), message).await?;
            return Ok(());
        }
        
        // Update the last attack time
        game_state.update_attack_time(&attacker_id, now);
        
        // Apply damage to the target
        let damage = 10;
        let target_defeated = game_state.apply_damage(&target_id, damage);
        
        if target_defeated {
            // Send an event to the attacker
            let event = ServerMessage::Event { 
                message: format!("Player {} has been defeated!", target_id) 
            };
            let message = serde_json::to_string(&event)?;
            client.send_reply(sender_tag.clone(), message).await?;
        }
        
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
            };
            let serialized = serde_json::to_string(&chat_msg)?;
            
            // Create confirmation message for the sender
            let confirm_msg = ServerMessage::Event {
                message: format!("Your message has been sent: {}", message),
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
