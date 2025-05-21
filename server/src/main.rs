mod game_protocol;
mod game_state;
mod handlers;
mod utils;
mod message_auth;

use game_protocol::{Player, Position, ClientMessage, ServerMessage};
use game_state::GameState;
use handlers::{handle_client_message, broadcast_game_state};
use utils::save_server_address;
use message_auth::{AuthKey, AuthenticatedMessage};

use nym_sdk::mixnet::{MixnetClient, MixnetClientBuilder, StoragePaths, AnonymousSenderTag, MixnetMessageSender};
use std::path::PathBuf;
use futures::StreamExt;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};
use anyhow::Result;

// For thread-safe handling of received message tracking
#[macro_use]
extern crate lazy_static;

#[tokio::main]
async fn main() -> Result<()> {
    println!("=== Nym Quest Server ===");
    
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
    
    // Generate a new authentication key for this server session
    let auth_key = AuthKey::new_random();
    println!("Generated authentication key for secure message verification");
    
    // Write server address and authentication key to a file that the client can read
    save_server_address(&server_address, &auth_key, "../client/server_address.txt")?;
    
    println!("Waiting for players...");
    
    // Initialize shared game state
    let game_state = Arc::new(GameState::new());
    
    // Start the main loop to process incoming messages
    while let Some(received_message) = client.next().await {
        // Skip empty messages
        if received_message.message.is_empty() {
            continue;
        }
        
        let sender_tag = match received_message.sender_tag {
            Some(tag) => tag,
            None => continue, // Skip messages without sender tags
        };
        
        let message_content = received_message.message;
        
        match String::from_utf8(message_content) {
            Ok(content) => {
                println!("Message received");
                
                // Try to deserialize as an authenticated message
                match serde_json::from_str::<AuthenticatedMessage<ClientMessage>>(&content) {
                    Ok(authenticated_message) => {
                        // Verify message authenticity
                        match authenticated_message.verify(&auth_key) {
                            Ok(true) => {
                                // Message is authentic, extract the actual client message
                                let client_message = authenticated_message.message;
                                
                                if let Err(e) = handle_client_message(
                                    &client, 
                                    &game_state, 
                                    client_message, 
                                    sender_tag.clone(),
                                    &auth_key
                                ).await {
                                    println!("Error handling client message: {}", e);
                                }
                            },
                            Ok(false) => {
                                println!("WARNING: Received message with invalid authentication - possible tampering attempt!");
                            },
                            Err(e) => {
                                println!("Error verifying message authenticity: {}", e);
                            }
                        }
                    },
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