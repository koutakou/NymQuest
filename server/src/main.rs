mod game_protocol;
mod game_state;
mod handlers;
mod utils;

use game_protocol::{Player, Position, ClientMessage, ServerMessage};
use game_state::GameState;
use handlers::{handle_client_message, broadcast_game_state};
use utils::save_server_address;

use nym_sdk::mixnet::{MixnetClient, MixnetClientBuilder, StoragePaths, AnonymousSenderTag, MixnetMessageSender};
use std::path::PathBuf;
use futures::StreamExt;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};
use anyhow::Result;

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
    
    // Write server address to a file that the client can read
    save_server_address(&server_address, "../client/server_address.txt")?;
    
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
                println!("Message received: {}", content);
                
                match serde_json::from_str::<ClientMessage>(&content) {
                    Ok(client_message) => {
                        if let Err(e) = handle_client_message(
                            &client, 
                            &game_state, 
                            client_message, 
                            sender_tag.clone()
                        ).await {
                            println!("Error handling client message: {}", e);
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