use nym_sdk::mixnet::{MixnetClient, MixnetClientBuilder, MixnetMessageSender, StoragePaths, AnonymousSenderTag};
use std::path::PathBuf;
use futures::StreamExt;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use uuid::Uuid;
use rand::{thread_rng, Rng};
use std::fs::File;
use std::io::Write;
use std::time::{SystemTime, UNIX_EPOCH};

mod game_protocol;
use game_protocol::{Player, Position, ClientMessage, ServerMessage};

// We'll store sender tags separately from players to avoid ownership issues

// New type to store active player connections
type PlayerTag = (String, AnonymousSenderTag); // (player_id, sender_tag)

// Function to broadcast game state to all active players
async fn broadcast_game_state(
    client: &MixnetClient,
    game_state: &Arc<Mutex<HashMap<String, Player>>>,
    active_connections: &Arc<Mutex<Vec<PlayerTag>>>,
    exclude_tag: Option<AnonymousSenderTag>
) -> anyhow::Result<()> {
    // Get the current game state
    let state = game_state.lock().unwrap().clone();
    let game_state_msg = ServerMessage::GameState { players: state };
    let message = serde_json::to_string(&game_state_msg)?;
    
    // Get a copy of all active connections
    let connections = active_connections.lock().unwrap().clone();
    
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

// Function to generate a random position that is not already occupied by another player
fn generate_available_position(players: &HashMap<String, Player>) -> Position {
    let mut rng = thread_rng();
    let position_tolerance: f32 = 2.0; // Minimum distance between players
    
    // Maximum number of attempts to find a free position
    const MAX_ATTEMPTS: usize = 100;
    
    for _ in 0..MAX_ATTEMPTS {
        // Generate a random position
        let position = Position {
            x: rng.gen_range(-100.0..100.0),
            y: rng.gen_range(-100.0..100.0),
        };
        
        // Check if this position is far enough from all other players
        let position_is_available = players.values().all(|player| {
            let dx = player.position.x - position.x;
            let dy = player.position.y - position.y;
            let distance_squared = dx * dx + dy * dy;
            
            // Consider the position available if the squared distance is greater than tolerance squared
            distance_squared > position_tolerance * position_tolerance
        });
        
        if position_is_available {
            return position;
        }
    }
    
    // If we can't find an available position after max attempts, just return a random one
    // This is a fallback that should rarely be needed
    Position {
        x: rng.gen_range(-100.0..100.0),
        y: rng.gen_range(-100.0..100.0),
    }
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    println!("=== Nym MMORPG Server ===");
    
    // Configure Nym client
    let config_dir = PathBuf::from("/tmp/nym_mmorpg_server");
    let storage_paths = StoragePaths::new_from_dir(&config_dir)?;
    
    println!("Initializing Nym client...");
    let client = MixnetClientBuilder::new_with_default_storage(storage_paths)
        .await?
        .build()?;
    
    let mut client = client.connect_to_mixnet().await?;
    
    let server_address = client.nym_address().to_string();
    println!("Server address: {}", server_address);
    
    // Write server address to a file that the client can read
    let config_dir = PathBuf::from("../client/server_address.txt");
    let mut file = File::create(config_dir)?;
    writeln!(file, "{}", server_address)?;
    println!("Server address saved to client/server_address.txt");
    
    println!("Waiting for players...");
    
    // Store game state and active connections
    let game_state = Arc::new(Mutex::new(HashMap::<String, Player>::new()));
    let active_connections = Arc::new(Mutex::new(Vec::<PlayerTag>::new()));
    
    // Process received messages
    while let Some(received_message) = client.next().await {
        if received_message.message.is_empty() {
            continue;
        }
        
        // Try to deserialize the message as a ClientMessage
        match String::from_utf8(received_message.message.clone()) {
            Ok(message_str) => {
                println!("Message received: {}", message_str);
                
                match serde_json::from_str::<ClientMessage>(&message_str) {
                    Ok(client_message) => {
                        if let Some(sender_tag) = received_message.sender_tag {
                            // Process client message
                            match client_message {
                                ClientMessage::Register { name } => {
                                    // Generate a unique ID for the player
                                    let player_id = Uuid::new_v4().to_string();
                                    
                                    // Get current game state to check for available positions
                                    let state = game_state.lock().unwrap();
                                    
                                    // Generate a position that's not occupied by another player
                                    let available_position = generate_available_position(&state);
                                    
                                    // Drop the lock after getting the available position
                                    drop(state);
                                    
                                    // Get current time for initializing attack cooldown
                                    let now = SystemTime::now().duration_since(UNIX_EPOCH)
                                        .unwrap_or_default()
                                        .as_secs();
                                        
                                    // Create a new player with the available position
                                    let player = Player {
                                        id: player_id.clone(),
                                        name,
                                        position: available_position,
                                        health: 100,
                                        last_attack_time: now,
                                    };
                                    
                                    // Add the player to the game state
                                    game_state.lock().unwrap().insert(player_id.clone(), player);
                                    
                                    // Store this active connection
                                    active_connections.lock().unwrap().push((player_id.clone(), sender_tag.clone()));
                                    
                                    // Send registration confirmation to the new player first
                                    let register_ack = ServerMessage::RegisterAck { player_id: player_id.clone() };
                                    let message = serde_json::to_string(&register_ack)?;
                                    client.send_reply(sender_tag.clone(), message).await?;
                                    
                                    // Then broadcast the updated game state to all players including the new one
                                    broadcast_game_state(&client, &game_state, &active_connections, None).await?
                                }
                                
                                ClientMessage::Move { direction } => {
                                    let mut found_player = None;
                                    let movement_speed = 5.0;
                                    
                                    // Find the player ID from active connections
                                    for (player_id, tag) in active_connections.lock().unwrap().iter() {
                                        if tag.to_string() == sender_tag.to_string() {
                                            found_player = Some(player_id.clone());
                                            break;
                                        }
                                    }
                                    
                                    // If a player was found, update their position
                                    if let Some(player_id) = found_player {
                                         // Get current player's position and calculate new intended position
                                         let current_position;
                                         let new_position;
                                         let (dx, dy) = direction.to_vector();
                                         
                                         // First, get the current player's position without holding a mutable borrow
                                         {
                                             let state = game_state.lock().unwrap();
                                             if let Some(player) = state.get(&player_id) {
                                                 println!("Moving player {} with direction {:?}, vector: ({}, {})", 
                                                         player_id, direction, dx, dy);
                                                 
                                                 // Save the current position
                                                 current_position = player.position;
                                                 
                                                 // Calculate the intended new position
                                                 new_position = Position {
                                                     x: current_position.x + dx * movement_speed,
                                                     y: current_position.y + dy * movement_speed,
                                                 };
                                             } else {
                                                 // Player not found, nothing to do
                                                 break;
                                             }
                                         }
                                         
                                         // Check if the new position would collide with any other player
                                         let position_tolerance: f32 = 2.0; // Minimum distance between players
                                         let position_is_available;
                                         {
                                             let state = game_state.lock().unwrap();
                                             position_is_available = state.iter().all(|(id, other_player)| {
                                                 // Skip checking against ourselves
                                                 if *id == player_id {
                                                     return true;
                                                 }
                                                 
                                                 let dx = other_player.position.x - new_position.x;
                                                 let dy = other_player.position.y - new_position.y;
                                                 let distance_squared = dx * dx + dy * dy;
                                                 
                                                 // Consider the position available if the squared distance is greater than tolerance squared
                                                 distance_squared > position_tolerance * position_tolerance
                                             });
                                         }
                                         
                                         // Only update position if it's available
                                         let needs_state_update;
                                         if position_is_available {
                                             // Now get a mutable reference to update the player's position
                                             let mut state = game_state.lock().unwrap();
                                             if let Some(player) = state.get_mut(&player_id) {
                                                 player.position = new_position;
                                                 println!("Player {} moved to position ({}, {})", player_id, new_position.x, new_position.y);
                                             }
                                             needs_state_update = true;
                                         } else {
                                             println!("Player {} can't move - position is occupied", player_id);
                                             needs_state_update = false;
                                         }
                                         
                                         // Send updated game state to all players if we moved
                                         if needs_state_update {
                                             broadcast_game_state(&client, &game_state, &active_connections, None).await?;
                                         }
                                     }
                                }
                                
                                ClientMessage::Attack { target_id } => {
                                    let mut attacker_id = None;
                                    
                                    // Find the attacker ID from active connections
                                    for (player_id, tag) in active_connections.lock().unwrap().iter() {
                                        if tag.to_string() == sender_tag.to_string() {
                                            attacker_id = Some(player_id.clone());
                                            break;
                                        }
                                    }
                                    
                                    if let Some(attacker_id) = attacker_id {
                                        let mut state = game_state.lock().unwrap();
                                        
                                        // Get current time for cooldown check
                                        let now = SystemTime::now().duration_since(UNIX_EPOCH)
                                            .unwrap_or_default()
                                            .as_secs();
                                        
                                        // Check if attacker exists and is not on cooldown
                                        if let Some(attacker) = state.get_mut(&attacker_id) {
                                            // Check if the attack cooldown has expired (3 seconds cooldown)
                                            const ATTACK_COOLDOWN: u64 = 3;
                                            
                                            if now - attacker.last_attack_time < ATTACK_COOLDOWN {
                                                // Player is still on cooldown
                                                let remaining = ATTACK_COOLDOWN - (now - attacker.last_attack_time);
                                                let cooldown_msg = ServerMessage::Error { 
                                                    message: format!("Attack on cooldown! Wait {} more seconds.", remaining) 
                                                };
                                                let message = serde_json::to_string(&cooldown_msg)?;
                                                client.send_reply(sender_tag.clone(), message).await?;
                                                return Ok(());
                                            }
                                            
                                            // Update the last attack time
                                            attacker.last_attack_time = now;
                                            
                                            if let Some(target) = state.get_mut(&target_id) {
                                                // For simplicity, we always apply damage
                                                // without checking the distance
                                                let damage = 10;
                                                if target.health <= damage {
                                                    target.health = 0;
                                                    
                                                    // Reset the defeated player
                                                    let mut rng = thread_rng();
                                                    target.position.x = rng.gen_range(-100.0..100.0);
                                                    target.position.y = rng.gen_range(-100.0..100.0);
                                                    target.health = 100;
                                                    
                                                    // Send an event to the attacker
                                                    let event = ServerMessage::Event { 
                                                        message: format!("Player {} has been defeated!", target_id) 
                                                    };
                                                    let message = serde_json::to_string(&event)?;
                                                    client.send_reply(sender_tag.clone(), message).await?;
                                                    
                                                    // Broadcast the updated game state to all players
                                                    broadcast_game_state(&client, &game_state, &active_connections, None).await?
                                                } else {
                                                    target.health -= damage;
                                                    
                                                    // Broadcast the updated game state to all players
                                                    broadcast_game_state(&client, &game_state, &active_connections, None).await?
                                                }
                                            }
                                        }
                                    }
                                }
                                
                                ClientMessage::Chat { message } => {
                                    // Find sender's player ID and name from the sender tag
                                    let mut sender_id = String::new();
                                    let mut sender_name = String::new();
                                    
                                    // Look up the player ID in our active connections
                                    for (player_id, tag) in active_connections.lock().unwrap().iter() {
                                        if tag.to_string() == sender_tag.to_string() {
                                            sender_id = player_id.clone();
                                            break;
                                        }
                                    }
                                    
                                    // Get the player's name from game state
                                    if let Some(player) = game_state.lock().unwrap().get(&sender_id) {
                                        sender_name = player.name.clone();
                                        
                                        // Create a chat message
                                        let chat_msg = ServerMessage::ChatMessage {
                                            sender_name: sender_name.clone(),
                                            message: message.clone(),
                                        };
                                        let serialized = serde_json::to_string(&chat_msg)?;
                                        
                                        // Send to all players except the sender
                                        for (_, tag) in active_connections.lock().unwrap().iter() {
                                            if tag.to_string() != sender_tag.to_string() {
                                                let _ = client.send_reply(tag.clone(), serialized.clone()).await;
                                            }
                                        }
                                        
                                        println!("Chat message from {}: {}", sender_name, message);
                                    }
                                },
                                
                                ClientMessage::Disconnect => {
                                    // Find and remove the player using the sender tag
                                    let mut player_to_remove = None;
                                    
                                    // Look up the player ID in our active connections
                                    let mut index_to_remove = None;
                                    for (i, (player_id, tag)) in active_connections.lock().unwrap().iter().enumerate() {
                                        if tag.to_string() == sender_tag.to_string() {
                                            player_to_remove = Some(player_id.clone());
                                            index_to_remove = Some(i);
                                            break;
                                        }
                                    }
                                    
                                    if let Some(id) = player_to_remove {
                                        // Remove the player from game state
                                        game_state.lock().unwrap().remove(&id);
                                        
                                        // Remove from active connections
                                        if let Some(index) = index_to_remove {
                                            active_connections.lock().unwrap().remove(index);
                                        }
                                        
                                        println!("Player {} disconnected", id);
                                        
                                        // Broadcast the updated game state to all remaining players
                                        let _ = broadcast_game_state(&client, &game_state, &active_connections, None).await;
                                    }
                                }
                            }
                        }
                    }
                    Err(e) => println!("Deserialization error: {}", e),
                }
            }
            Err(e) => println!("Non-UTF8 message received: {}", e),
        }
    }
    
    println!("Server stopped");
    client.disconnect().await;
    
    Ok(())
}