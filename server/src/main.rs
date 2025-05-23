mod game_protocol;
mod game_state;
mod handlers;
mod utils;
mod message_auth;

use game_protocol::{Player, Position, ClientMessage, ServerMessage};
use game_state::GameState;
use handlers::{handle_client_message, broadcast_game_state, send_heartbeat_requests, cleanup_inactive_players};
use utils::save_server_address;
use message_auth::{AuthKey, AuthenticatedMessage};

use nym_sdk::mixnet::{MixnetClient, MixnetClientBuilder, StoragePaths, AnonymousSenderTag, MixnetMessageSender};
use std::path::PathBuf;
use futures::StreamExt;
use std::time::{SystemTime, UNIX_EPOCH, Duration};
use std::sync::Arc;
use anyhow::Result;
use tracing::{info, warn, error, debug};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt, EnvFilter, Layer};
use tokio::time::{interval, timeout};

// For thread-safe handling of received message tracking
#[macro_use]
extern crate lazy_static;

/// Initialize structured logging for the server
fn init_logging() -> Result<()> {
    // Create a rolling file appender for production logs
    let file_appender = tracing_appender::rolling::daily("logs", "nymquest-server.log");
    let (file_writer, _guard) = tracing_appender::non_blocking(file_appender);
    
    // Set up console output with pretty formatting for development
    let console_layer = tracing_subscriber::fmt::layer()
        .with_target(false)
        .compact();
    
    // Set up file output with JSON formatting for production parsing
    let file_layer = tracing_subscriber::fmt::layer()
        .with_writer(file_writer)
        .json()
        .with_target(true)
        .with_current_span(false);
    
    // Initialize the subscriber with environment-based filtering
    // Default to INFO level, can be overridden with RUST_LOG env var
    let filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new("info"));
    
    tracing_subscriber::registry()
        .with(filter)
        .with(console_layer)
        .with(file_layer)
        .init();
    
    info!("Structured logging initialized");
    Ok(())
}

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize logging first
    if let Err(e) = init_logging() {
        eprintln!("Failed to initialize logging: {}", e);
        return Err(e);
    }
    
    info!("=== Nym Quest Server Starting ===");
    
    // Configure Nym client
    let config_dir = PathBuf::from("/tmp/nym_mmorpg_server");
    let storage_paths = StoragePaths::new_from_dir(&config_dir)?;
    
    info!("Initializing Nym mixnet client");
    let client = MixnetClientBuilder::new_with_default_storage(storage_paths)
        .await?
        .build()?;
    
    let mut client = client.connect_to_mixnet().await?;
    
    let server_address = client.nym_address().to_string();
    info!(
        server_address = %server_address,
        "Server successfully connected to Nym mixnet"
    );
    
    // Generate a new authentication key for this server session
    let auth_key = AuthKey::new_random();
    info!("Generated authentication key for secure message verification");
    
    // Write server address and authentication key to a file that the client can read
    match save_server_address(&server_address, &auth_key, "../client/server_address.txt") {
        Ok(()) => info!("Server address and auth key saved to client file"),
        Err(e) => {
            error!(
                error = %e,
                "Failed to save server address to client file"
            );
            return Err(e);
        }
    }
    
    info!("Server ready - waiting for players to connect");
    
    // Initialize shared game state
    let game_state = Arc::new(GameState::new());
    
    // Set up periodic heartbeat tasks
    let mut heartbeat_interval = interval(Duration::from_secs(game_state::HEARTBEAT_INTERVAL_SECONDS));
    let mut cleanup_interval = interval(Duration::from_secs(game_state::HEARTBEAT_TIMEOUT_SECONDS / 3)); // Check more frequently than timeout
    
    info!(
        "Heartbeat system initialized - requests every {}s, timeout after {}s", 
        game_state::HEARTBEAT_INTERVAL_SECONDS,
        game_state::HEARTBEAT_TIMEOUT_SECONDS
    );
    
    // Main event loop with heartbeat management
    loop {
        tokio::select! {
            // Handle incoming messages from clients
            received_message = client.next() => {
                match received_message {
                    Some(message) => {
                        // Process the message
                        if let Err(e) = process_incoming_message(&client, &game_state, message.message, message.sender_tag, &auth_key).await {
                            error!("Error processing incoming message: {}", e);
                        }
                    }
                    None => {
                        info!("Client stream ended, shutting down");
                        break;
                    }
                }
            }
            
            // Send periodic heartbeat requests
            _ = heartbeat_interval.tick() => {
                if let Err(e) = send_heartbeat_requests(&client, &game_state, &auth_key).await {
                    error!("Error sending heartbeat requests: {}", e);
                }
            }
            
            // Cleanup inactive players
            _ = cleanup_interval.tick() => {
                if let Err(e) = cleanup_inactive_players(&client, &game_state, &auth_key).await {
                    error!("Error cleaning up inactive players: {}", e);
                }
            }
        }
    }
    
    info!("Server shutting down");
    client.disconnect().await;
    
    Ok(())
}

/// Process an incoming message from a client
async fn process_incoming_message(
    client: &MixnetClient,
    game_state: &Arc<GameState>,
    received_message: impl Into<Vec<u8>>,
    sender_tag: Option<AnonymousSenderTag>,
    auth_key: &AuthKey
) -> Result<()> {
    let message_content = received_message.into();
    
    // Skip empty messages
    if message_content.is_empty() {
        debug!("Received empty message, skipping");
        return Ok(());
    }
    
    let sender_tag = match sender_tag {
        Some(tag) => tag,
        None => {
            debug!("Received message without sender tag, skipping");
            return Ok(());
        }
    };
    
    match String::from_utf8(message_content) {
        Ok(content) => {
            debug!(
                message_size = content.len(),
                "Processing incoming message"
            );
            
            // Try to deserialize as an authenticated message
            match serde_json::from_str::<AuthenticatedMessage<ClientMessage>>(&content) {
                Ok(authenticated_message) => {
                    // Verify message authenticity
                    match authenticated_message.verify(auth_key) {
                        Ok(true) => {
                            // Message is authentic, extract the actual client message
                            let client_message = authenticated_message.message;
                            debug!(
                                message_type = ?client_message,
                                "Processing authenticated client message"
                            );
                            
                            if let Err(e) = handle_client_message(
                                client, 
                                game_state, 
                                client_message, 
                                sender_tag,
                                auth_key
                            ).await {
                                error!(
                                    error = %e,
                                    "Failed to handle client message"
                                );
                            }
                        },
                        Ok(false) => {
                            warn!(
                                "Received message with invalid authentication - possible security threat"
                            );
                        },
                        Err(e) => {
                            error!(
                                error = %e,
                                "Error verifying message authenticity"
                            );
                        }
                    }
                },
                Err(e) => {
                    debug!(
                        error = %e,
                        "Failed to deserialize message as authenticated format"
                    );
                }
            }
        }
        Err(e) => {
            debug!(
                error = %e,
                "Received non-UTF8 message content"
            );
        }
    }
    
    Ok(())
}