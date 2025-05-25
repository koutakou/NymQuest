mod command_completer;
mod config;
mod discovery;
mod game_protocol;
mod game_state;
mod message_auth;
mod network;
mod renderer;
mod status_monitor;
mod ui_components;

use colored::*;
use rustyline::error::ReadlineError;
use rustyline::{Config, Editor};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use tokio::sync::mpsc;
use tokio::{task, time};
use tracing::{error, info, warn};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt, EnvFilter, Layer};

use command_completer::GameHistoryHinter;
use config::ClientConfig;
use game_protocol::{ClientMessage, Direction, ProtocolVersion, ServerMessage};
use game_state::GameState;
use network::NetworkManager;
use ui_components::{clear_screen, render_game_state, render_help_section};

/// Initialize structured logging for the client
fn init_logging() -> anyhow::Result<()> {
    // Create a rolling file appender for production logs
    let file_appender = tracing_appender::rolling::daily("logs", "nymquest-client.log");
    let (file_writer, _guard) = tracing_appender::non_blocking(file_appender);

    // Set up console output with filtering for better UX
    // Use a restrictive filter to only show our application logs, not noisy network libraries
    let console_filter = EnvFilter::new("nym_mmorpg_client=info");

    let console_layer = tracing_subscriber::fmt::layer()
        .with_target(false)
        .with_level(true)
        .compact()
        .with_filter(console_filter);

    // Set up file output with JSON formatting for production parsing
    let file_layer = tracing_subscriber::fmt::layer()
        .with_writer(file_writer)
        .json()
        .with_target(true)
        .with_current_span(false);

    // Initialize the subscriber with environment-based filtering for file logs
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));

    tracing_subscriber::registry()
        .with(console_layer)
        .with(file_layer.with_filter(filter))
        .init();

    info!("Client structured logging initialized");
    Ok(())
}

/// Main entry point for the NYM MMORPG Client
#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Initialize logging first
    if let Err(e) = init_logging() {
        eprintln!("Failed to initialize logging: {}", e);
        return Err(e);
    }

    info!("=== NYM MMORPG Client Starting ===");
    println!("{}", "=== NYM MMORPG Client ===".green().bold());

    // Load client configuration
    let config = ClientConfig::load()?;

    // Initialize the game state
    let game_state: Arc<Mutex<GameState>> = Arc::new(Mutex::new(GameState::new()));

    // Get a reference to the status monitor for the network manager
    let status_monitor = {
        let gs = game_state.lock().unwrap();
        Arc::clone(&gs.status_monitor)
    };

    // Initialize network connection
    let mut network = match NetworkManager::new(&config, status_monitor).await {
        Ok(network) => network,
        Err(e) => {
            error!("Error initializing network connection: {}", e);
            return Ok(());
        }
    };

    // Create a channel for user input commands
    let (tx, mut rx) = mpsc::channel::<String>(100);

    // Create a dedicated channel for controlling typing state
    let (typing_tx, mut typing_rx) = mpsc::channel::<bool>(10);
    let typing_tx_clone = typing_tx.clone();

    // Initialize rustyline editor with history
    let history_path = dirs_next::home_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".nym_mmorpg_history");

    info!("Command history saved to: {}", history_path.display());

    // Clone necessary values for the input handling task
    let tx_clone = tx.clone();
    let game_state_clone = Arc::clone(&game_state);
    let _config_clone = config.clone();

    // Spawn a task to handle user input with command history
    task::spawn(async move {
        // Initialize rustyline editor with custom config and hinter
        let config = Config::builder()
            .auto_add_history(true)
            .history_ignore_space(true)
            .completion_type(rustyline::CompletionType::List)
            .build();

        let mut rl = match Editor::with_config(config) {
            Ok(mut editor) => {
                // Set our custom hinter
                editor.set_helper(Some(GameHistoryHinter::new()));
                editor
            }
            Err(e) => {
                error!("Error initializing editor: {}", e);
                return;
            }
        };

        // Load command history if it exists
        if let Err(e) = rl.load_history(&history_path) {
            // Only print a warning if the file exists but couldn't be loaded
            // It's normal for it not to exist on the first run
            if history_path.exists() {
                warn!("Failed to load command history: {}", e);
            }
        }

        // Input handling loop
        loop {
            // Signal that typing has started
            let _ = typing_tx_clone.send(true).await;

            // Show game state when prompt appears
            // Handle the mutex lock more gracefully to prevent poisoning
            if let Ok(state) = game_state_clone.lock() {
                render_game_state(&state);
            } else {
                // If mutex is poisoned, print error and continue
                error!("Failed to access game state. Please restart the client.");
            }

            // Use rustyline to get input with history navigation
            let readline = rl.readline("> ");

            // Signal that typing has ended
            let _ = typing_tx_clone.send(false).await;

            match readline {
                Ok(line) => {
                    let input = line.trim().to_string();

                    if input.is_empty() {
                        continue;
                    }

                    // Add valid input to history
                    rl.add_history_entry(input.clone());

                    // Save history periodically
                    if let Err(e) = rl.save_history(&history_path) {
                        warn!("Failed to save history: {}", e);
                    }

                    // Send the command to main thread
                    if (tx_clone.send(input).await).is_err() {
                        break;
                    }
                }
                Err(ReadlineError::Interrupted) => {
                    // Ctrl-C pressed
                    info!("CTRL-C pressed, use 'exit' to quit properly");
                }
                Err(ReadlineError::Eof) => {
                    // Ctrl-D pressed, exit properly
                    info!("EOF (CTRL-D) detected, exiting...");
                    let exit_command = "exit".to_string();
                    if (tx_clone.send(exit_command).await).is_err() {
                        break;
                    }
                }
                Err(err) => {
                    error!("Error reading input: {}", err);
                    break;
                }
            }
        }
    });

    // Render initial state
    match game_state.lock() {
        Ok(state) => {
            render_game_state(&state);
        }
        Err(e) => {
            error!("Failed to render initial game state: {}", e);
            info!("Continuing without initial render...");
        }
    }

    // We'll use a simple event handler approach
    let result = run_event_loop(&mut network, &game_state, &mut rx, &mut typing_rx, &config).await;

    // This would only be reached if the event loop had a break condition
    // which our implementation doesn't currently have
    match result {
        Ok(_) => {
            info!("Disconnected. Goodbye!");
            Ok(())
        }
        Err(e) => {
            error!("Error in event loop: {}", e);
            Err(e)
        }
    }
}

/// Process a command entered by the user
async fn process_user_command(
    input: &str,
    network: &mut NetworkManager,
    game_state: &Arc<Mutex<GameState>>,
    config: &ClientConfig,
) -> anyhow::Result<()> {
    let command_parts: Vec<&str> = input.split_whitespace().collect();

    if command_parts.is_empty() {
        return Ok(());
    }

    // Get command without leading slash if present
    let cmd = command_parts[0].trim_start_matches('/');

    match cmd {
        // Registration command
        "register" | "r" => {
            // Check if the player is already registered
            if let Ok(state) = game_state.lock() {
                if state.is_registered() {
                    info!("You are already registered. Please disconnect first before registering again.");
                    return Ok(());
                }
            } else {
                error!("Failed to access game state for registration check. Please restart the client.");
                return Ok(());
            }

            if command_parts.len() < 2 {
                info!("Please provide a name to register with");
                return Ok(());
            }

            let name = command_parts[1..].join(" ").trim().to_string();

            // Create register message (sequence number handled by NetworkManager)
            let register_msg = ClientMessage::Register {
                name,
                protocol_version: ProtocolVersion::default(),
                seq_num: 0, // Placeholder, will be replaced by NetworkManager
            };

            // Send the message
            network.send_message(register_msg).await?;

            info!("Registration request sent...");
        }
        // Movement commands
        "move" | "m" | "go" => {
            // Check if player is registered
            let player_id = {
                if let Ok(state) = game_state.lock() {
                    state.get_player_id().map(|id| id.to_string())
                } else {
                    error!("Failed to access game state for player ID. Please restart the client.");
                    return Ok(());
                }
            };

            if player_id.is_none() {
                info!("You need to register first!");
                return Ok(());
            }

            if command_parts.len() < 2 {
                info!("Please specify a direction (up, down, left, right, etc.)");
                return Ok(());
            }

            let direction_str = &command_parts[1].to_lowercase();

            if let Some(direction) = Direction::from_str(direction_str) {
                // Create move message with placeholder sequence number
                let move_msg = ClientMessage::Move {
                    direction,
                    seq_num: 0, // Will be set by NetworkManager
                };

                // Send the message
                network.send_message(move_msg).await?;

                // Get current position to display movement prediction
                let move_vector = direction.to_vector();

                // Create a block to limit the scope of the mutex lock
                {
                    let _predicted_pos = match game_state.lock() {
                        Ok(mut state) => {
                            // Check if we have a player ID and clone it to avoid borrow issues
                            if let Some(player_id) = state.player_id.clone() {
                                // Get world boundaries first (before mutable borrow)
                                let boundaries = state.get_world_boundaries().cloned();

                                // Now get a mutable reference to the player
                                if let Some(player) = state.players.get_mut(&player_id) {
                                    let mut new_pos = player.position;

                                    // Use the configured movement speed
                                    let movement_speed = config.movement_speed;
                                    new_pos.apply_movement(move_vector, movement_speed);

                                    // Apply world boundaries if available from server
                                    if let Some(boundaries) = boundaries {
                                        boundaries.clamp_position_mut(&mut new_pos);
                                    }

                                    clear_screen();
                                    info!("Moving {:?}", direction);
                                    info!(
                                        "Current position: ({:.1}, {:.1})",
                                        player.position.x, player.position.y
                                    );
                                    info!(
                                        "Predicted position: ({:.1}, {:.1})",
                                        new_pos.x, new_pos.y
                                    );

                                    // Update position locally for responsive feedback
                                    // This will be corrected when we receive the next GameState update
                                    player.position = new_pos;
                                    Some(new_pos)
                                } else {
                                    None
                                }
                            } else {
                                None
                            }
                        }
                        Err(e) => {
                            // Handle poisoned mutex gracefully
                            error!("Failed to access game state for movement prediction: {}", e);
                            info!("Movement will proceed without local prediction.");
                            None
                        }
                    };
                }

            // Message already sent above, no need to send it again
            } else {
                info!("Invalid direction! Valid options: up, down, left, right, upleft, upright, downleft, downright");
            }
        }
        // Direct movement shortcuts - these are more ergonomic than typing "/move <direction>"
        "up" | "u" | "north" | "n" => {
            // Use helper function to handle movement in direction
            handle_movement_direction(network, game_state, Direction::Up, config).await?
        }
        "down" | "d" | "south" | "s" => {
            handle_movement_direction(network, game_state, Direction::Down, config).await?
        }
        "left" | "l" | "west" | "w" => {
            handle_movement_direction(network, game_state, Direction::Left, config).await?
        }
        "right" | "east" | "e" => {
            handle_movement_direction(network, game_state, Direction::Right, config).await?
        }
        // Diagonal movement shortcuts
        "ne" | "northeast" => {
            handle_movement_direction(network, game_state, Direction::UpRight, config).await?
        }
        "nw" | "northwest" => {
            handle_movement_direction(network, game_state, Direction::UpLeft, config).await?
        }
        "se" | "southeast" => {
            handle_movement_direction(network, game_state, Direction::DownRight, config).await?
        }
        "sw" | "southwest" => {
            handle_movement_direction(network, game_state, Direction::DownLeft, config).await?
        }
        // Attack command
        "attack" | "a" => {
            // Check if player is registered
            if let Ok(state) = game_state.lock() {
                if !state.is_registered() {
                    info!("You need to register first before you can attack.");
                    return Ok(());
                }
            } else {
                error!("Failed to access game state for registration check. Please restart the client.");
                return Ok(());
            }

            if command_parts.len() < 2 {
                info!("Usage: attack <player_display_id>");
                return Ok(());
            }

            let target_display_id = command_parts[1].to_string();
            let attack_msg = ClientMessage::Attack {
                target_display_id: target_display_id.clone(),
                seq_num: 0, // Will be set by NetworkManager
            };

            network.send_message(attack_msg).await?;
            info!("Attack request sent to player '{}'...", target_display_id);
        }
        // Chat command
        "chat" | "c" | "say" => {
            // Check if player is registered
            if let Ok(state) = game_state.lock() {
                if !state.is_registered() {
                    info!("You need to register first before you can chat.");
                    return Ok(());
                }
            } else {
                error!("Failed to access game state for registration check. Please restart the client.");
                return Ok(());
            }

            if command_parts.len() < 2 {
                info!("Usage: chat <message>");
                return Ok(());
            }

            let message_text = command_parts[1..].join(" ");
            let chat_msg = ClientMessage::Chat {
                message: message_text,
                seq_num: 0, // Will be set by NetworkManager
            };

            network.send_message(chat_msg).await?;
            info!("Chat message sent...");
        }
        // Emote command
        "emote" | "em" => {
            // Check if player is registered
            if let Ok(state) = game_state.lock() {
                if !state.is_registered() {
                    info!("You need to register first before you can use emotes.");
                    return Ok(());
                }
            } else {
                error!("Failed to access game state for registration check. Please restart the client.");
                return Ok(());
            }

            if command_parts.len() < 2 {
                info!("Usage: emote <type>\nAvailable emotes: wave, bow, laugh, dance, salute, shrug, cheer, clap");
                return Ok(());
            }

            let emote_name = command_parts[1].to_lowercase();
            if let Some(emote_type) = game_protocol::EmoteType::from_str(&emote_name) {
                let emote_msg = ClientMessage::Emote {
                    emote_type,
                    seq_num: 0, // Will be set by NetworkManager
                };

                network.send_message(emote_msg).await?;
                info!("Emote '{}' sent...", emote_name);
            } else {
                info!("Invalid emote type! Available emotes: wave, bow, laugh, dance, salute, shrug, cheer, clap");
            }
        }
        // Exit commands
        "exit" | "quit" | "q" => {
            // Perform proper network disconnection which will send the disconnect message
            if let Err(e) = network.disconnect().await {
                error!("Error during disconnection: {}", e);
            }

            info!("Goodbye!");
            std::process::exit(0);
        }
        // Help command
        "help" | "h" | "?" => {
            render_help_section();
            return Ok(());
        }
        // Message pacing commands for privacy protection
        "pacing" | "pace" => {
            if command_parts.len() < 2 {
                // Display current pacing status
                let pacing_info = network.get_message_pacing();
                info!(
                    "Message pacing status: {}",
                    if pacing_info.0 {
                        "Enabled".green()
                    } else {
                        "Disabled".red()
                    }
                );
                if pacing_info.0 {
                    info!(
                        "Base interval: {}ms, Last jitter: {}ms",
                        pacing_info.1, pacing_info.2
                    );
                }
                info!("Usage: /pacing [on|off] [interval_ms]");
                info!("Example: /pacing on 150");
                return Ok(());
            }

            let subcommand = command_parts[1].to_lowercase();
            match subcommand.as_str() {
                "on" | "enable" => {
                    // Get interval if provided
                    let interval = if command_parts.len() >= 3 {
                        match command_parts[2].parse::<u64>() {
                            Ok(val) => val,
                            Err(_) => {
                                info!("Invalid interval value. Using default 100ms.");
                                100
                            }
                        }
                    } else {
                        100 // Default interval
                    };

                    // Enable message pacing with specified interval
                    network.set_message_pacing(true, interval);
                    info!(
                        "Message pacing {} with {}ms interval",
                        "enabled".green(),
                        interval
                    );
                }
                "off" | "disable" => {
                    // Disable message pacing
                    network.set_message_pacing(false, 0);
                    info!("Message pacing {}", "disabled".red());
                }
                "status" => {
                    // Display current pacing status
                    let pacing_info = network.get_message_pacing();
                    info!(
                        "Message pacing status: {}",
                        if pacing_info.0 {
                            "Enabled".green()
                        } else {
                            "Disabled".red()
                        }
                    );
                    if pacing_info.0 {
                        info!(
                            "Base interval: {}ms, Last jitter: {}ms",
                            pacing_info.1, pacing_info.2
                        );
                    }
                }
                _ => {
                    info!("Unknown pacing subcommand: {}", subcommand);
                    info!("Usage: /pacing [on|off] [interval_ms]");
                }
            }
        }
        _ => {
            info!("Unknown command: {}", command_parts[0]);
            info!("Type {} for available commands", "/help".cyan());
        }
    }

    Ok(())
}

/// Run the main event loop for the game
async fn run_event_loop(
    network: &mut NetworkManager,
    game_state: &Arc<Mutex<GameState>>,
    rx: &mut mpsc::Receiver<String>,
    typing_rx: &mut mpsc::Receiver<bool>,
    config: &ClientConfig,
) -> anyhow::Result<()> {
    // Create an interval for checking unacknowledged messages
    let mut check_interval = time::interval(time::Duration::from_millis(1000));

    // This loop will run until the process exits
    loop {
        tokio::select! {
            // Process user commands
            Some(command) = rx.recv() => {
                if let Err(e) = process_user_command(&command, network, game_state, config).await {
                    error!("Error processing command: {}", e);
                }
                // Don't render here as it's done in the input handler now
            },
            // Process incoming messages from the server
            Some(server_message) = network.receive_message() => {
                // Process the message and get whether it was a chat message
                let was_chat = process_server_message(game_state, server_message);

                // Always refresh the display when we receive a chat message, even if typing
                if let Ok(state) = game_state.lock() {
                    if was_chat || !state.is_typing {
                        render_game_state(&state);
                    }
                } else {
                    // Handle poisoned mutex
                    error!("Error accessing game state. Please restart the client.");
                }
            },
            // Check for typing state updates
            Some(is_typing) = typing_rx.recv() => {
                if let Ok(mut state) = game_state.lock() {
                    state.set_typing(is_typing);
                } else {
                    // Mutex poisoned, continue gracefully
                    error!("Failed to update typing state.");
                }
            },
            // Periodically check for messages that need to be resent
            _ = check_interval.tick() => {
                if let Err(e) = network.check_for_resends().await {
                    error!("Error checking for messages to resend: {}", e);
                }
            }
        }
    }
}

/// Helper function to handle movement in a specific direction
/// This avoids recursion in async functions which would cause compile errors
async fn handle_movement_direction(
    network: &mut NetworkManager,
    game_state: &Arc<Mutex<GameState>>,
    direction: Direction,
    config: &ClientConfig,
) -> anyhow::Result<()> {
    // Check if player is registered
    let player_id = {
        if let Ok(state) = game_state.lock() {
            state.get_player_id().map(|id| id.to_string())
        } else {
            error!("Failed to access game state for player ID. Please restart the client.");
            return Ok(());
        }
    };

    if player_id.is_none() {
        info!("You need to register first!");
        return Ok(());
    }

    // Create move message with placeholder sequence number
    let move_msg = ClientMessage::Move {
        direction,
        seq_num: 0, // Will be set by NetworkManager
    };

    // Send the message
    network.send_message(move_msg).await?;

    // Get current position to display movement prediction
    let move_vector = direction.to_vector();

    // Create a block to limit the scope of the mutex lock
    {
        let _predicted_pos = match game_state.lock() {
            Ok(mut state) => {
                // Check if we have a player ID and clone it to avoid borrow issues
                if let Some(player_id) = state.player_id.clone() {
                    // Get world boundaries first (before mutable borrow)
                    let boundaries = state.get_world_boundaries().cloned();

                    // Now get a mutable reference to the player
                    if let Some(player) = state.players.get_mut(&player_id) {
                        let mut new_pos = player.position;

                        // Use the configured movement speed from client config (should match server)
                        let movement_speed = config.movement_speed;
                        new_pos.apply_movement(move_vector, movement_speed);

                        // Apply world boundaries if available from server
                        if let Some(boundaries) = boundaries {
                            boundaries.clamp_position_mut(&mut new_pos);
                        }

                        // Note: Server will handle collision detection and may reject the move
                        // Client can only predict movement assuming no collisions

                        clear_screen();
                        info!("Moving {:?}", direction);
                        info!(
                            "Current position: ({:.1}, {:.1})",
                            player.position.x, player.position.y
                        );
                        info!("Predicted position: ({:.1}, {:.1})", new_pos.x, new_pos.y);

                        // Update position locally for responsive feedback
                        // This will be corrected when we receive the next GameState update
                        player.position = new_pos;
                        Some(new_pos)
                    } else {
                        None
                    }
                } else {
                    None
                }
            }
            Err(e) => {
                // Handle poisoned mutex gracefully
                error!("Failed to access game state for movement prediction: {}", e);
                info!("Movement will proceed without local prediction.");
                None
            }
        };
    }

    Ok(())
}

/// Process a message received from the server
/// Returns true if the message was a chat message that should refresh the display
/// Returns false on error or non-chat messages
fn process_server_message(
    game_state: &Arc<Mutex<GameState>>,
    server_message: ServerMessage,
) -> bool {
    // Safely lock the game state and handle potential poisoning
    let mut state = match game_state.lock() {
        Ok(state) => state,
        Err(e) => {
            // Handle poisoned mutex
            error!("Error accessing game state: {}", e);
            return false; // Return false to avoid refresh
        }
    };

    match server_message {
        ServerMessage::ServerShutdown {
            message,
            shutdown_in_seconds,
            seq_num: _,
        } => {
            // Display a prominent shutdown warning
            let warning = format!(
                "⚠️ SERVER SHUTDOWN IN {} SECONDS: {}",
                shutdown_in_seconds, message
            );

            // Add to chat history as a system message
            state.add_chat_message("SERVER SHUTDOWN".to_string(), warning.clone());

            // Print a very visible warning in the console
            error!(
                "{}\n{}",
                "⚠️ SERVER SHUTDOWN NOTICE ⚠️".red().bold(),
                warning.red().bold()
            );

            // Immediate exit without sending a disconnect message
            info!("Server initiated shutdown. Exiting immediately without sending disconnect message...");
            std::process::exit(0);
        }
        ServerMessage::RegisterAck {
            player_id,
            world_boundaries,
            negotiated_version: _,
            seq_num: _,
        } => {
            state.set_player_id(player_id);
            state.set_world_boundaries(world_boundaries);
            info!("Registration successful! Received world boundaries from server.");
            render_game_state(&state);
            false
        }
        ServerMessage::GameState {
            players,
            seq_num: _,
        } => {
            // Debugging output
            info!("Received game state with {} players", players.len());

            // Print player IDs
            for player_id in players.keys() {
                info!("  - Player ID: {}", player_id.cyan());
            }

            state.update_players(players);

            // Render game state immediately to update the mini-map
            render_game_state(&state);
            false
        }
        ServerMessage::Event {
            message,
            seq_num: _,
        } => {
            // Add events to chat history as system messages
            state.add_chat_message("System".to_string(), message.clone());
            info!("Event: {}", message.yellow());
            true
        }
        ServerMessage::ChatMessage {
            sender_name,
            message,
            seq_num: _,
        } => {
            // Add message to chat history
            state.add_chat_message(sender_name.clone(), message.clone());

            // Also print it for immediate visibility
            info!("[{}]: {}", sender_name.green(), message.white());
            true
        }
        ServerMessage::Error {
            message,
            seq_num: _,
        } => {
            // Add errors to chat history as system messages
            state.add_chat_message("System Error".to_string(), message.clone());
            error!("Error: {}", message.red());
            true
        }
        ServerMessage::Ack {
            client_seq_num: _,
            original_type: _,
        } => {
            // Acknowledgments are handled in the NetworkManager
            // We don't need to do anything here
            false
        }
        ServerMessage::HeartbeatRequest { seq_num: _ } => {
            // Heartbeat requests are handled in the NetworkManager
            // We don't need to do anything here in the main loop
            false
        }
    }
}
