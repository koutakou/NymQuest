mod game_protocol;
mod game_state;
mod handlers;
mod utils;
mod message_auth;
mod config;
mod persistence;

use game_protocol::{Player, Position, ClientMessage};
use game_state::GameState;
use handlers::{handle_client_message, send_heartbeat_requests, cleanup_inactive_players, init_rate_limiter, cleanup_rate_limiter, broadcast_shutdown_notification};
use utils::save_server_address;
use message_auth::{AuthKey, AuthenticatedMessage};
use config::GameConfig;
use persistence::GameStatePersistence;

use nym_sdk::mixnet::{MixnetClient, MixnetClientBuilder, StoragePaths, AnonymousSenderTag};
use std::path::PathBuf;
use futures::StreamExt;
use std::time::Duration;
use std::sync::Arc;
use anyhow::Result;
use tracing::{info, warn, error, debug};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};
use tokio::time::interval;
use std::time::Instant;

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
    // Filter out h2 close_notify warnings which are common with mixnet connections
    let filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new("info,h2=error"));
    
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
    
    // Load game configuration
    let game_config = match GameConfig::load() {
        Ok(config) => {
            info!("Game configuration loaded successfully");
            config
        },
        Err(e) => {
            error!("Failed to load game configuration: {}", e);
            return Err(e);
        }
    };
    
    // Initialize rate limiter
    init_rate_limiter(&game_config);
    
    // Log key configuration values for debugging
    info!("World boundaries: ({:.1}, {:.1}) to ({:.1}, {:.1})", 
          game_config.world_min_x, game_config.world_min_y,
          game_config.world_max_x, game_config.world_max_y);
    info!("Player limits: {} max players, {} max name length",
          game_config.max_players, game_config.max_player_name_length);
    info!("Combat settings: {} damage, {}s cooldown", 
          game_config.attack_damage, game_config.attack_cooldown_seconds);
    info!("Heartbeat: {}s interval, {}s timeout",
          game_config.heartbeat_interval_seconds, game_config.heartbeat_timeout_seconds);
    
    // Configure Nym client with permanent storage location
    let config_dir = dirs_next::data_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("nymquest").join("server").join("nym_storage");
    
    // Create directories if they don't exist
    std::fs::create_dir_all(&config_dir)?;
    
    info!("Using permanent Nym storage location: {:?}", config_dir);
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
    
    info!("Server is ready and listening for connections");
    
    // Initialize game state persistence
    let persistence_enabled = std::env::var("NYMQUEST_ENABLE_PERSISTENCE")
        .map(|v| v.to_lowercase() == "true")
        .unwrap_or(true); // Default to enabled
    
    let persistence_dir = std::env::var("NYMQUEST_PERSISTENCE_DIR")
        .unwrap_or_else(|_| "./game_data".to_string());
    
    let persistence = GameStatePersistence::new(&persistence_dir, persistence_enabled);
    
    // Initialize persistence directory
    if let Err(e) = persistence.initialize().await {
        error!("Failed to initialize game state persistence: {}", e);
        return Err(e);
    }
    
    // Create backup of existing state before starting
    if let Err(e) = persistence.backup_current_state().await {
        warn!("Failed to create state backup: {}", e);
    }
    
    // Initialize game state with loaded configuration
    let game_state = Arc::new(GameState::new_with_config(game_config.clone()));
    
    // Try to recover previous game state
    match persistence.load_state(&game_config).await {
        Ok(Some(mut persisted_state)) => {
            info!("Attempting to recover previous game state");
            
            // Clean up stale players (offline for more than 5 minutes)
            let cleanup_threshold = 300; // 5 minutes in seconds
            persistence.cleanup_stale_players(&mut persisted_state, cleanup_threshold);
            
            // Restore player data (excluding network connections)
            let mut recovered_count = 0;
            for (player_id, persisted_player) in persisted_state.players {
                // Create a new Player from persisted data
                let player = Player {
                    id: player_id.clone(), // Use the player_id as the internal ID
                    display_id: persisted_player.display_id,
                    name: persisted_player.name,
                    position: persisted_player.position,
                    health: persisted_player.health,
                    last_attack_time: persisted_player.last_attack_time,
                };
                
                // Validate position is still within current world boundaries
                let (clamped_x, clamped_y) = game_config.clamp_position(player.position.x, player.position.y);
                let adjusted_player = if clamped_x != player.position.x || clamped_y != player.position.y {
                    warn!("Adjusted player {} position from ({}, {}) to ({}, {}) due to boundary changes",
                          player_id, player.position.x, player.position.y, clamped_x, clamped_y);
                    Player {
                        id: player.id.clone(),
                        position: Position::new(clamped_x, clamped_y),
                        ..player
                    }
                } else {
                    player
                };
                
                // Add player to game state (they will need to reconnect to establish network connection)
                game_state.restore_player(player_id, adjusted_player);
                recovered_count += 1;
            }
            
            info!("Recovered {} players from previous session", recovered_count);
            if recovered_count > 0 {
                info!("Recovered players will be visible once they reconnect through the mixnet");
            }
        },
        Ok(None) => {
            info!("Starting with fresh game state");
        },
        Err(e) => {
            warn!("Failed to load previous game state, starting fresh: {}", e);
        }
    }
    
    // Set up shutdown signal handler
    let (shutdown_tx, mut shutdown_rx) = tokio::sync::mpsc::channel::<()>(1);
    
    // Set up ctrl+c handler for graceful shutdown
    tokio::spawn(async move {
        if let Err(e) = tokio::signal::ctrl_c().await {
            error!("Failed to listen for ctrl+c: {}", e);
            return;
        }
        info!("Received shutdown signal, initiating graceful shutdown...");
        let _ = shutdown_tx.send(()).await;
    });
    
    // Main event loop
    info!("Server ready to receive connections");
    let mut heartbeat_interval = interval(Duration::from_secs(game_config.heartbeat_interval_seconds));
    let mut cleanup_interval = interval(Duration::from_secs(game_config.inactive_player_cleanup_interval_seconds));
    
    // Add persistence interval (save state every 2 minutes)
    let mut persistence_interval = interval(Duration::from_secs(120));
    
    // Add rate limiter cleanup interval (cleanup every 5 minutes)
    let mut rate_limiter_cleanup_interval = interval(Duration::from_secs(300));
    
    // Message processing pacing for privacy protection
    let mut last_message_processed: Option<Instant> = None;
    
    // Skip the first tick to avoid immediate execution
    heartbeat_interval.tick().await;
    cleanup_interval.tick().await;
    persistence_interval.tick().await;
    rate_limiter_cleanup_interval.tick().await;

    // Main event loop with background task scheduling
    loop {
        tokio::select! {
            // Handle shutdown signal
            _ = shutdown_rx.recv() => {
                info!("Processing shutdown sequence...");
                
                // Final state persistence
                let players = game_state.get_players();
                info!("Saving final game state...");
                if let Err(e) = persistence.save_state(&players, &game_config).await {
                    error!("Failed to save final game state during shutdown: {}", e);
                } else {
                    info!("Final game state saved successfully");
                }
                
                // Send shutdown notification to all players with 5 second countdown
                info!("Notifying connected players of server shutdown...");
                if let Err(e) = broadcast_shutdown_notification(
                    &client, 
                    &game_state, 
                    "Server is shutting down. You will be disconnected shortly.", 
                    5, // 5 second countdown before forced disconnect
                    &auth_key
                ).await {
                    error!("Failed to send shutdown notification: {}", e);
                }
                
                // Wait a moment to allow clients to receive the notification
                info!("Waiting for notification delivery...");
                tokio::time::sleep(Duration::from_secs(1)).await;
                
                // Clean disconnect from mixnet
                info!("Disconnecting from Nym mixnet...");
                break;
            },
            // Handle incoming messages from clients
            received_message = client.next() => {
                match received_message {
                    Some(message) => {
                        // Process the message
                        if let Err(e) = process_incoming_message(&client, &game_state, message.message, message.sender_tag, &auth_key, &game_config, &mut last_message_processed).await {
                            error!("Error processing incoming message: {}", e);
                        }
                    }
                    None => {
                        error!("Message stream ended unexpectedly");
                        break;
                    }
                }
            },
            
            // Send heartbeat requests to all connected players periodically
            _ = heartbeat_interval.tick() => {
                if let Err(e) = send_heartbeat_requests(&client, &game_state, &auth_key).await {
                    error!("Failed to send heartbeat requests: {}", e);
                }
            },
            
            // Clean up inactive players periodically
            _ = cleanup_interval.tick() => {
                if let Err(e) = cleanup_inactive_players(&client, &game_state, &auth_key).await {
                    error!("Failed to cleanup inactive players: {}", e);
                }
            },
            
            // Save game state to disk periodically
            _ = persistence_interval.tick() => {
                let players = game_state.get_players();
                if let Err(e) = persistence.save_state(&players, &game_config).await {
                    error!("Failed to save game state: {}", e);
                } else if !players.is_empty() {
                    debug!("Periodically saved game state with {} players", players.len());
                }
            },
            
            // Clean up rate limiter periodically
            _ = rate_limiter_cleanup_interval.tick() => {
                cleanup_rate_limiter();
                debug!("Performed rate limiter cleanup");
            }
        }
    }
    
    info!("Server is shutting down gracefully...");
    
    // Final cleanup of rate limiter
    let _ = cleanup_rate_limiter();
    
    // Disconnect from the mixnet (ensuring data is flushed)
    client.disconnect().await;
    info!("Successfully disconnected from Nym mixnet");
    
    info!("Server shutdown complete");
    
    // The client will be dropped naturally when main exits
    // No need to explicitly disconnect when using shared references
    Ok(())
}

/// Process an incoming message from a client
async fn process_incoming_message(
    client: &MixnetClient,
    game_state: &Arc<GameState>,
    received_message: impl Into<Vec<u8>>,
    sender_tag: Option<AnonymousSenderTag>,
    auth_key: &AuthKey,
    game_config: &GameConfig,
    last_message_processed: &mut Option<Instant>
) -> Result<()> {
    let message_content = received_message.into();
    
    // Apply message processing pacing for privacy protection (prevent timing correlation attacks)
    if game_config.enable_message_processing_pacing {
        if let Some(last_processed) = *last_message_processed {
            let elapsed = last_processed.elapsed();
            let min_interval = Duration::from_millis(game_config.message_processing_interval_ms);
            
            if elapsed < min_interval {
                let wait_time = min_interval - elapsed;
                debug!("Applying message processing pacing: waiting {:?} for privacy protection", wait_time);
                tokio::time::sleep(wait_time).await;
            }
        }
        *last_message_processed = Some(Instant::now());
    }
    
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