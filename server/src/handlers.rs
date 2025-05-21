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
    
    // Send state update to each connected player
    for (_, tag) in connections {
        // Skip excluded player if any
        if let Some(exclude) = &exclude_tag {
            if exclude.to_string() == tag.to_string() {
                continue;
            }
        }
        
        // Send the update to this player
        let _ = client.send_reply(tag, message.clone()).await;
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
            // Calculate movement vector and new position
            let (dx, dy) = direction.to_vector();
            let movement_speed = 5.0;
            
            let new_position = Position {
                x: player.position.x + dx * movement_speed,
                y: player.position.y + dy * movement_speed,
            };
            
            // Try to update the player's position
            if game_state.update_player_position(&player_id, new_position) {
                println!("Player {} moved to position ({}, {})", 
                    player_id, new_position.x, new_position.y);
                
                // Broadcast the updated game state to all players
                broadcast_game_state(client, game_state, None).await?;
            } else {
                println!("Player {} can't move - position is occupied", player_id);
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
            
            // Create the chat message
            let chat_msg = ServerMessage::ChatMessage {
                sender_name: sender_name.clone(),
                message: message.clone(),
            };
            let serialized = serde_json::to_string(&chat_msg)?;
            
            // Send to all players except the sender
            for (_, tag) in game_state.get_connections() {
                if tag.to_string() != sender_tag.to_string() {
                    let _ = client.send_reply(tag.clone(), serialized.clone()).await;
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
