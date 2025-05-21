use anyhow::{Result, anyhow};
use nym_sdk::mixnet::{MixnetClientBuilder, MixnetClient, MixnetMessageSender, StoragePaths, Recipient, IncludedSurbs};
use std::str::FromStr;
use std::fs;
use std::path::PathBuf;
use uuid::Uuid;
use futures::StreamExt;

use crate::game_protocol::{ClientMessage, ServerMessage};

/// NetworkManager handles all interactions with the Nym mixnet
pub struct NetworkManager {
    client: Option<MixnetClient>,
    server_address: String,
}

impl NetworkManager {
    /// Create a new NetworkManager and connect to the Nym network
    pub async fn new() -> Result<Self> {
        // Read server address from file
        let server_address = match fs::read_to_string("server_address.txt").or_else(|_| fs::read_to_string("../client/server_address.txt")) {
            Ok(address) => address.trim().to_string(),
            Err(_) => {
                return Err(anyhow!("Cannot read server address from server_address.txt. Make sure the server is running and you have access to the address file."));
            }
        };
        
        println!("Server address: {}", server_address);
        
        // Configure Nym client with a unique directory for each instance
        // Generate a unique ID for this client to prevent connection conflicts
        let unique_id = Uuid::new_v4().to_string();
        let config_dir = PathBuf::from(format!("/tmp/nym_mmorpg_client_{}", unique_id));
        let storage_paths = StoragePaths::new_from_dir(&config_dir)?;
        
        println!("Initializing Nym client with unique ID...");
        let client = MixnetClientBuilder::new_with_default_storage(storage_paths)
            .await?
            .build()?;
        
        let client = client.connect_to_mixnet().await?;
        
        println!("Connected to Nym network!");
        
        Ok(Self {
            client: Some(client),
            server_address,
        })
    }
    
    /// Send a message to the server
    pub async fn send_message(&mut self, message: ClientMessage) -> Result<()> {
        if let Some(client) = &mut self.client {
            let message_str = serde_json::to_string(&message)?;
            
            // Create recipient from server address
            let recipient = Recipient::from_str(&self.server_address)
                .map_err(|e| anyhow!("Invalid server address: {}", e))?;
            
            client.send_message(recipient, message_str.into_bytes(), IncludedSurbs::default()).await?;
            
            Ok(())
        } else {
            Err(anyhow!("Client is not connected"))
        }
    }
    
    /// Wait for the next message from the server
    pub async fn receive_message(&mut self) -> Option<ServerMessage> {
        // Early return if client is not connected
        let client = match &mut self.client {
            Some(client) => client,
            None => return None,
        };
        
        // Wait for the next message
        let received_message = match client.next().await {
            Some(msg) => msg,
            None => return None,
        };
        
        // Check for empty messages
        if received_message.message.is_empty() {
            return None;
        }
        
        // Try to convert bytes to UTF-8 string
        let message_str = match String::from_utf8(received_message.message) {
            Ok(str) => str,
            Err(e) => {
                println!("Error parsing message: {}", e);
                return None;
            }
        };
        
        // Try to deserialize the message
        match serde_json::from_str::<ServerMessage>(&message_str) {
            Ok(server_message) => Some(server_message),
            Err(e) => {
                println!("Error deserializing server message: {}", e);
                None
            }
        }
    }
    
    /// Disconnect from the Nym network
    pub async fn disconnect(&mut self) -> Result<()> {
        if let Some(client) = self.client.take() {
            println!("Disconnecting from Nym network...");
            // Properly await the disconnection to ensure it completes
            client.disconnect().await;
            println!("Disconnected.");
            Ok(())
        } else {
            println!("Already disconnected.");
            Ok(())
        }
    }
    
    /// Get a reference to the server address
    pub fn server_address(&self) -> &str {
        &self.server_address
    }
    
    /// Check if the client is connected
    pub fn is_connected(&self) -> bool {
        self.client.is_some()
    }
}
