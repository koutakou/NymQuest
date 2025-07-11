mod command_completer;
mod config;
mod discovery;
mod game_protocol;
mod game_state;
mod message_auth;
mod message_padding;
mod message_replay;
mod mixnet_health;
mod network;
mod renderer;
mod status_monitor;
mod ui_components;
mod world_lore;

// Application constants
const USER_INPUT_CHANNEL_BUFFER: usize = 256;
const TYPING_STATE_CHANNEL_BUFFER: usize = 32;
const HEARTBEAT_CHECK_INTERVAL_MS: u64 = 1000;
const DEFAULT_PACING_INTERVAL_MS: u64 = 100;

use crate::world_lore::Faction;

use colored::*;
use rustyline::error::ReadlineError;
use rustyline::{Config, Editor};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use tokio::sync::mpsc;
use tokio::{task, time};
use tracing::{debug, error, info, warn};
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

    // Set up file output with JSON formatting for production parsing
    // This will capture all logs to file only, not to console
    let file_layer = tracing_subscriber::fmt::layer()
        .with_writer(file_writer)
        .json()
        .with_target(true)
        .with_level(true)
        .with_current_span(false);

    // Initialize the subscriber with environment-based filtering for file logs
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));

    // Only use the file layer, remove console layer to prevent logs from appearing in the command input area
    tracing_subscriber::registry()
        .with(file_layer.with_filter(filter))
        .init();

    // This log entry will only appear in the log file, not in the console
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

    // Create a channel for user input commands (increased buffer for better performance)
    let (tx, mut rx) = mpsc::channel::<String>(USER_INPUT_CHANNEL_BUFFER);

    // Create a dedicated channel for controlling typing state (increased buffer)
    let (typing_tx, mut typing_rx) = mpsc::channel::<bool>(TYPING_STATE_CHANNEL_BUFFER);
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

/// Handle registration command processing
async fn handle_register_command(
    command_parts: &[&str],
    network: &mut NetworkManager,
    game_state: &Arc<Mutex<GameState>>,
) -> anyhow::Result<()> {
    // Check if the player is already registered
    if let Ok(mut state) = game_state.lock() {
        if state.is_registered() {
            // Show in UI instead of just logging
            state.add_system_message(
                "System".to_string(),
                "You are already registered. Please disconnect first before registering again."
                    .to_string(),
            );
            render_game_state(&state);
            return Ok(());
        }

        // Update status monitor with registration status
        if let Ok(mut monitor) = state.status_monitor.lock() {
            monitor.update_game_state_info("Registration in progress...".to_string());
        }

        // Show the status in the UI
        render_game_state(&state);
    } else {
        // Direct console output for critical errors
        println!(
            "{}",
            "Failed to access game state for registration check. Please restart the client.".red()
        );
        return Ok(());
    }

    // Parse command: /register <name> <faction>
    if command_parts.len() < 3 {
        if let Ok(mut state) = game_state.lock() {
            state.add_system_message(
                "System".to_string(),
                "Usage: /register <name> <faction>\nAvailable factions:\n - nyms (The Nyms Coalition)\n - corporate/corp (Corporate Hegemony)\n - cipher/collective (Cipher Collective)\n - monks/algorithm (Algorithm Monks)\n - independent/indie (Independent Operators)".to_string(),
            );

            // Reset the registration status
            if let Ok(mut monitor) = state.status_monitor.lock() {
                monitor.update_game_state_info(
                    "Registration failed - incomplete information".to_string(),
                );
            }

            render_game_state(&state);
        }
        return Ok(());
    }

    let name = command_parts[1].trim().to_string();
    let faction_input = command_parts[2].trim().to_lowercase();

    // Parse faction selection
    let faction = match faction_input.as_str() {
        "nyms" => Faction::Nyms,
        "corporate" | "corp" | "hegemony" => Faction::CorporateHegemony,
        "cipher" | "collective" | "ciphercollective" => Faction::CipherCollective,
        "monks" | "algorithm" | "algorithmmonks" => Faction::AlgorithmMonks,
        "independent" | "indie" | "free" => Faction::Independent,
        _ => {
            if let Ok(mut state) = game_state.lock() {
                state.add_system_message(
                    "System".to_string(),
                    "Invalid faction. Available options:\n - nyms (privacy advocates)\n - corporate/corp (corporate power)\n - cipher/collective (data liberation)\n - monks/algorithm (digital mystics)\n - independent/indie (free agents)".to_string(),
                );

                if let Ok(mut monitor) = state.status_monitor.lock() {
                    monitor.update_game_state_info(
                        "Registration failed - invalid faction".to_string(),
                    );
                }

                render_game_state(&state);
            }
            return Ok(());
        }
    };

    // Create register message (sequence number handled by NetworkManager)
    let register_msg = ClientMessage::Register {
        name: name.clone(),
        faction: faction.clone(), // Add selected faction to registration message
        protocol_version: ProtocolVersion::default(),
        seq_num: 0, // Placeholder, will be replaced by NetworkManager
    };

    // Show registration attempt message in UI
    if let Ok(mut state) = game_state.lock() {
        state.add_system_message(
            "System".to_string(),
            format!("Attempting to register as '{}' of the {} faction. Please wait...\nEach faction has unique advantages in the cypherpunk world.",
                name, format!("{:?}", faction).replace("CorporateHegemony", "Corporate Hegemony").replace("CipherCollective", "Cipher Collective").replace("AlgorithmMonks", "Algorithm Monks")),
        );
        render_game_state(&state);
    }

    // Send the message
    network.send_message(register_msg).await?;

    // Direct console output to show status
    println!("{}", "Registration request sent...".cyan());
    Ok(())
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
            handle_register_command(&command_parts, network, game_state).await?;
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
        "left" | "l" | "west" => {
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
        // Whisper command
        "whisper" | "wh" | "msg" | "tell" => {
            // Check if player is registered
            if let Ok(state) = game_state.lock() {
                if !state.is_registered() {
                    info!("You need to register first before you can send whispers.");
                    return Ok(());
                }
            } else {
                error!("Failed to access game state for registration check. Please restart the client.");
                return Ok(());
            }

            if command_parts.len() < 3 {
                info!("Usage: whisper <player_display_id> <message>");
                return Ok(());
            }

            let target_display_id = command_parts[1].to_string();
            let message_text = command_parts[2..].join(" ");

            // Verify the target player exists
            let player_id = if let Ok(state) = game_state.lock() {
                state.get_player_id_by_display_id(&target_display_id)
            } else {
                None
            };

            if player_id.is_none() {
                info!(
                    "Player with display name '{}' not found.",
                    target_display_id
                );
                return Ok(());
            }

            // Create the whisper message
            let whisper_msg = ClientMessage::Whisper {
                target_display_id: target_display_id.clone(),
                message: message_text.clone(),
                seq_num: 0, // Will be set by NetworkManager
            };

            network.send_message(whisper_msg).await?;
            info!("Whisper to '{}' sent...", target_display_id);
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
                // Get a lock on the game state to check faction
                let faction_specific_help = match game_state.lock().ok().and_then(|state| state.player_faction()) {
                    Some(Faction::Nyms) => "Your Nyms Coalition specialty: encrypt, ghost (enhanced privacy animations)",
                    Some(Faction::CorporateHegemony) => "Your Corporate specialty: surveillance, datadrop (corporate-style animations)",
                    Some(Faction::CipherCollective) => "Your Cipher Collective specialty: hack, decrypt (data liberation animations)",
                    Some(Faction::AlgorithmMonks) => "Your Algorithm Monks specialty: encrypt, decrypt (mystical pattern animations)",
                    Some(Faction::Independent) => "Your Independent specialty: resist, glitch (unique rogue animations)",
                    None => "Register with a faction to unlock specialty emotes"
                };

                info!("Usage: emote <type>\nStandard emotes: wave, bow, laugh, dance, salute, shrug, cheer, clap, thumbsup\nCypherpunk emotes: hack, encrypt, decrypt, surveillance, resist, ghost, datadrop, glitch\n{}", 
                    faction_specific_help);
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
                info!("Invalid emote type!\nStandard emotes: wave, bow, laugh, dance, salute, shrug, cheer, clap, thumbsup\nCypherpunk emotes: hack, encrypt, decrypt, surveillance, resist, ghost, datadrop, glitch\n(Your faction specialty: {}) ", 
                    match game_state.lock().ok().and_then(|state| state.player_faction()) {
                        Some(Faction::Nyms) => "encrypt, ghost (enhanced privacy effects)",
                        Some(Faction::CorporateHegemony) => "surveillance, datadrop (corporate style)",
                        Some(Faction::CipherCollective) => "hack, decrypt (data liberation effects)",
                        Some(Faction::AlgorithmMonks) => "encrypt, decrypt (mystical patterns)",
                        Some(Faction::Independent) => "resist, glitch (unique animations)",
                        None => "register to unlock faction specialty emotes"
                    }
                );
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
        // Alias for whisper command
        "reply" | "re" => {
            if command_parts.len() < 2 {
                info!("Usage: reply <message>");
                return Ok(());
            }

            // Get the last whisper sender from game state
            let (last_whisper_sender, player_id) = if let Ok(state) = game_state.lock() {
                let sender = state.get_last_whisper_sender().map(|s| s.to_string());
                let player_id = sender
                    .as_ref()
                    .and_then(|name| state.get_player_id_by_display_id(name));
                (sender, player_id)
            } else {
                error!("Failed to access game state. Please restart the client.");
                return Ok(());
            };

            if let Some(sender) = last_whisper_sender {
                // Check if we can get the connection tag for the player
                let connection_tag = if let (Some(pid), Ok(state)) = (&player_id, game_state.lock())
                {
                    state.get_connection_tag(pid)
                } else {
                    None
                };

                // Construct the whisper message
                let message_text = command_parts[1..].join(" ");
                let whisper_msg = ClientMessage::Whisper {
                    target_display_id: sender.clone(),
                    message: message_text.clone(),
                    seq_num: 0, // Will be set by NetworkManager
                };

                // Send the message
                network.send_message(whisper_msg).await?;

                // Log success message with connection tag info if available
                if let Some(tag) = connection_tag {
                    info!("Reply to '{}' (connection: {}) sent...", sender, tag);
                } else {
                    info!("Reply to '{}' sent...", sender);
                }
            } else {
                info!("No one to reply to! You haven't received any whispers yet.");
            }

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
                                info!(
                                    "Invalid interval value. Using default {}ms.",
                                    DEFAULT_PACING_INTERVAL_MS
                                );
                                DEFAULT_PACING_INTERVAL_MS
                            }
                        }
                    } else {
                        DEFAULT_PACING_INTERVAL_MS // Default interval
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
    let mut check_interval =
        time::interval(time::Duration::from_millis(HEARTBEAT_CHECK_INTERVAL_MS));

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

/// Process a message from the server
/// Returns true if the message was a chat-like message that should force a UI refresh
fn process_server_message(
    game_state: &Arc<Mutex<GameState>>,
    server_message: ServerMessage,
) -> bool {
    match server_message {
        ServerMessage::ServerShutdown {
            message,
            shutdown_in_seconds,
            seq_num: _,
        } => {
            let warning = format!(
                "⚠️ SERVER SHUTDOWN IN {} SECONDS: {}",
                shutdown_in_seconds, message
            );

            if let Ok(mut state) = game_state.lock() {
                // Add to system messages
                state.add_system_message("SERVER SHUTDOWN".to_string(), warning.clone());

                // Update status monitor for display in UI
                if let Ok(mut monitor) = state.status_monitor.lock() {
                    monitor.update_game_state_info(format!(
                        "SERVER SHUTDOWN IN {} SECONDS",
                        shutdown_in_seconds
                    ));
                    monitor.update_connection_info(message.clone());
                }
            }

            // Print directly to console (bypassing logging) to ensure visibility
            // This will show even with logging disabled
            println!("{}", "⚠️ SERVER SHUTDOWN NOTICE ⚠️".red().bold());
            println!("{}", warning.red().bold());

            // Force a UI refresh to show the shutdown message
            if let Ok(state) = game_state.lock() {
                render_game_state(&state);
            }

            // Sleep briefly to ensure the message is visible before exit
            std::thread::sleep(std::time::Duration::from_millis(1000));

            // Immediate exit without sending a disconnect message
            std::process::exit(0);
        }
        ServerMessage::RegisterAck {
            player_id,
            world_boundaries,
            negotiated_version: _,
            seq_num: _,
        } => {
            if let Ok(mut state) = game_state.lock() {
                state.set_player_id(player_id.clone());
                state.set_world_boundaries(world_boundaries);
                state.add_system_message(
                    "System".to_string(),
                    "Registration successful! Welcome to NymQuest!".to_string(),
                );
                info!(
                    "Registration successful! Your player ID is: {}",
                    player_id.green()
                );
            } else {
                error!("Failed to update game state with registration info");
            }
            true // Force UI refresh after successful registration
        }
        ServerMessage::GameState {
            players,
            seq_num: _,
        } => {
            if let Ok(mut state) = game_state.lock() {
                state.update_players(players.clone());

                // Update the status monitor with game state info for UI display
                if let Ok(mut monitor) = state.status_monitor.lock() {
                    monitor.update_game_state_info(format!(
                        "Game state updated - {} players online",
                        players.len()
                    ));
                }
            } else {
                error!("Failed to update game state with players info");
            }
            true // Force UI refresh when game state updates
        }
        ServerMessage::ChatMessage {
            sender_name,
            message,
            seq_num: _,
        } => {
            if let Ok(mut state) = game_state.lock() {
                state.add_chat_message(sender_name.clone(), message.clone());
            } else {
                error!("Failed to add chat message to game state");
            }
            info!("{}: {}", sender_name.cyan(), message);
            true
        }
        ServerMessage::WhisperMessage {
            sender_name,
            message,
            seq_num: _,
        } => {
            if let Ok(mut state) = game_state.lock() {
                state.add_whisper_message(sender_name.clone(), message.clone());
            } else {
                error!("Failed to add whisper message to game state");
            }
            info!(
                "{} {} {}",
                "[Whisper from".magenta(),
                sender_name.magenta().bold(),
                "]".magenta()
            );
            info!("{}", message.magenta().italic());
            true
        }
        ServerMessage::Event {
            message,
            seq_num: _,
        } => {
            if let Ok(mut state) = game_state.lock() {
                state.add_system_message("System".to_string(), message.clone());
            } else {
                error!("Failed to add event message to game state");
            }
            info!("Event: {}", message.yellow());
            true
        }
        ServerMessage::Error {
            message,
            seq_num: _,
        } => {
            if let Ok(mut state) = game_state.lock() {
                state.add_system_message("System Error".to_string(), message.clone());
            } else {
                error!("Failed to add error message to game state");
            }
            error!("Error: {}", message.red());
            true
        }
        ServerMessage::Ack { .. } => {
            // Acknowledgments are handled in the NetworkManager
            false
        }
        ServerMessage::HeartbeatRequest { .. } => {
            // Heartbeat requests are handled in the NetworkManager
            false
        }
        ServerMessage::PlayerLeft { .. } => {
            // Player left messages update the game state but don't need special UI refresh
            false
        }
        ServerMessage::PlayerUpdate {
            display_id,
            position,
            health,
            seq_num: _,
        } => {
            if let Ok(mut state) = game_state.lock() {
                // Find the player with matching display_id and update their position and health
                let player_id_to_update = state.get_player_id_by_display_id(&display_id);

                if let Some(player_id) = player_id_to_update {
                    if let Some(player) = state.players.get_mut(&player_id) {
                        player.position = position;
                        player.health = health;
                        state.update_timestamp(); // Mark state as updated

                        debug!(
                            "Updated player {}: position ({}, {}), health {}",
                            display_id, position.x, position.y, health
                        );

                        // Force a UI refresh to update the minimap
                        return true;
                    }
                }
            } else {
                error!("Failed to update player position for {}", display_id);
            }
            false
        }
    }
}
