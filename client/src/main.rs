mod game_protocol;
mod game_state;
mod network;
mod renderer;

use std::sync::{Arc, Mutex};
use tokio::sync::mpsc;
use tokio::{task, time};
use colored::*;
use rustyline::error::ReadlineError;
use rustyline::Editor;
use std::path::PathBuf;

use game_protocol::{ClientMessage, ServerMessage, Direction, Position};
use game_state::GameState;
use network::NetworkManager;
use renderer::{render_game_state, clear_screen};

/// Main entry point for the NYM MMORPG Client
#[tokio::main]
async fn main() -> anyhow::Result<()> {
    println!("{}", "=== NYM MMORPG Client ===".green().bold());
    
    // Initialize the game state
    let game_state: Arc<Mutex<GameState>> = Arc::new(Mutex::new(GameState::new()));
    
    // Initialize network connection
    let mut network = match NetworkManager::new().await {
        Ok(network) => network,
        Err(e) => {
            println!("{} {}", "Error:".red().bold(), e);
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
        
    println!("{} {}", "Command history saved to:".cyan(), history_path.display());
    
    // Clone necessary values for the input handling task
    let tx_clone = tx.clone();
    let game_state_clone = Arc::clone(&game_state);
    
    // Spawn a task to handle user input with command history
    task::spawn(async move {
        // Initialize rustyline editor
        let mut rl = match Editor::<()>::new() {
            Ok(editor) => editor,
            Err(e) => {
                println!("Error initializing editor: {}", e);
                return;
            }
        };
        
        // Load command history if it exists
        if let Err(e) = rl.load_history(&history_path) {
            // Only print a warning if the file exists but couldn't be loaded
            // It's normal for it not to exist on the first run
            if history_path.exists() {
                println!("Warning: Failed to load command history: {}", e);
            }
        }
        
        // Input handling loop
        loop {
            // Signal that typing has started
            let _ = typing_tx_clone.send(true).await;
            
            // Show game state when prompt appears
            render_game_state(&game_state_clone.lock().unwrap());
            
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
                        println!("Warning: Failed to save history: {}", e);
                    }
                    
                    // Send the command to main thread
                    if let Err(_) = tx_clone.send(input).await {
                        break;
                    }
                },
                Err(ReadlineError::Interrupted) => {
                    // Ctrl-C pressed
                    println!("CTRL-C pressed, use 'exit' to quit properly");
                },
                Err(ReadlineError::Eof) => {
                    // Ctrl-D pressed, exit properly
                    println!("EOF (CTRL-D) detected, exiting...");
                    let exit_command = "exit".to_string();
                    let _ = tx_clone.send(exit_command).await;
                    break;
                },
                Err(err) => {
                    println!("Error reading input: {}", err);
                    break;
                }
            }
        }
    });
    
    // Render initial state
    render_game_state(&game_state.lock().unwrap());
    
    // We'll use a simple event handler approach
    let result = run_event_loop(&mut network, &game_state, &mut rx, &mut typing_rx).await;
    
    // This would only be reached if the event loop had a break condition
    // which our implementation doesn't currently have
    match result {
        Ok(_) => {
            println!("Disconnected. Goodbye!");
            Ok(())
        },
        Err(e) => {
            println!("{} {}", "Error in event loop:".red().bold(), e);
            Err(e)
        }
    }
}

/// Process a command entered by the user
async fn process_user_command(
    network: &mut NetworkManager,
    game_state: &Arc<Mutex<GameState>>, 
    command: String
) -> anyhow::Result<()> {
    let command_parts: Vec<&str> = command.split_whitespace().collect();
    
    if command_parts.is_empty() {
        return Ok(());
    }
    
    match command_parts[0] {
        "register" => {
            if command_parts.len() < 2 {
                println!("{}", "Please provide a name to register with".yellow());
                return Ok(());
            }
            
            let name = command_parts[1..].join(" ").trim().to_string();
            
            // Create register message (sequence number handled by NetworkManager)
            let register_msg = ClientMessage::Register { 
                name, 
                seq_num: 0 // Placeholder, will be replaced by NetworkManager
            };
            
            // Send the message
            network.send_message(register_msg).await?;
            
            println!("{}", "Registration request sent...".green());
        },
        "move" => {
            // Check if player is registered
            let player_id = {
                let state = game_state.lock().unwrap();
                state.get_player_id().map(|id| id.to_string())
            };
            
            if player_id.is_none() {
                println!("{}", "You need to register first!".red());
                return Ok(());
            }
            
            if command_parts.len() < 2 {
                println!("{}", "Please specify a direction (up, down, left, right, etc.)".yellow());
                return Ok(());
            }
            
            let direction_str = &command_parts[1].to_lowercase();
            
            if let Some(direction) = Direction::from_str(direction_str) {
                // Create move message with placeholder sequence number
                let move_msg = ClientMessage::Move {
                    direction,
                    seq_num: 0  // Will be set by NetworkManager
                };
                
                // Send the message
                network.send_message(move_msg).await?;
                
                // Get current position to display movement prediction
                let move_vector = direction.to_vector();
            
            // Create a block to limit the scope of the mutex lock
            {
                let mut state = game_state.lock().unwrap();
                
                // First, check if we have a player ID and clone it to avoid borrow issues
                if let Some(player_id) = state.player_id.clone() {
                    // Now get a mutable reference to the player
                    if let Some(player) = state.players.get_mut(&player_id) {
                        // Calculate and display predicted new position
                        let mut predicted_pos = player.position;
                        
                        // Use the same mini_map_cell_size as the server (14.0 units)
                        // This ensures one movement command = one cell on the mini-map
                        let mini_map_cell_size = 14.0;
                        predicted_pos.apply_movement(move_vector, mini_map_cell_size);
                        
                        clear_screen();
                        println!("Moving {:?}", direction);
                        println!("Current position: ({:.1}, {:.1})", player.position.x, player.position.y);
                        println!("Predicted position: ({:.1}, {:.1})", predicted_pos.x, predicted_pos.y);
                        
                        // Update position locally for responsive feedback
                        // This will be corrected when we receive the next GameState update
                        player.position = predicted_pos;
                    }
                }
            }
            
            // Message already sent above, no need to send it again
            } else {
                println!("{}", "Invalid direction! Valid options: up, down, left, right, upleft, upright, downleft, downright".red());
            }
        },
        "attack" => {
            // Check if player is registered
            if !game_state.lock().unwrap().is_registered() {
                println!("You need to register first before you can attack.");
                return Ok(());
            }
            
            if command_parts.len() < 2 {
                println!("Usage: attack <player_display_id>");
                return Ok(());
            }
            
            let target_display_id = command_parts[1].to_string();
            let attack_msg = ClientMessage::Attack { 
                target_display_id: target_display_id.clone(),
                seq_num: 0  // Will be set by NetworkManager
            };
            
            network.send_message(attack_msg).await?;
            println!("Attack request sent to player '{}'...", target_display_id);
        },
        "chat" => {
            // Check if player is registered
            if !game_state.lock().unwrap().is_registered() {
                println!("You need to register first before you can chat.");
                return Ok(());
            }
            
            if command_parts.len() < 2 {
                println!("Usage: chat <message>");
                return Ok(());
            }
            
            let message_text = command_parts[1..].join(" ");
            let chat_msg = ClientMessage::Chat { 
                message: message_text,
                seq_num: 0  // Will be set by NetworkManager
            };
            
            network.send_message(chat_msg).await?;
            println!("Chat message sent...");
        },
        "exit" => {
            // Send disconnect message if registered
            if game_state.lock().unwrap().is_registered() {
                let disconnect_msg = ClientMessage::Disconnect { seq_num: 0 };
                if let Err(e) = network.send_message(disconnect_msg).await {
                    println!("Failed to send disconnect message: {}", e);
                } else {
                    println!("Disconnect message sent to server");
                }
            }
            
            // Perform proper network disconnection
            if let Err(e) = network.disconnect().await {
                println!("Error during disconnection: {}", e);
            }
            
            println!("Goodbye!");
            std::process::exit(0);
        },
        _ => {
            println!("Unknown command: {}", command_parts[0]);
        }
    }
    
    Ok(())
}

/// Run the main event loop for the game
async fn run_event_loop(
    network: &mut NetworkManager,
    game_state: &Arc<Mutex<GameState>>,
    rx: &mut mpsc::Receiver<String>,
    typing_rx: &mut mpsc::Receiver<bool>
) -> anyhow::Result<()> {
    // Create an interval for checking unacknowledged messages
    let mut check_interval = time::interval(time::Duration::from_millis(1000));
    
    // This loop will run until the process exits
    loop {
        tokio::select! {
            // Process user commands
            Some(command) = rx.recv() => {
                if let Err(e) = process_user_command(network, game_state, command).await {
                    println!("{} {}", "Error processing command:".red().bold(), e);
                }
                // Don't render here as it's done in the input handler now
            },
            // Process incoming messages from the server
            Some(server_message) = network.receive_message() => {
                // Process the message and get whether it was a chat message
                let was_chat = process_server_message(game_state, server_message);
                
                // Always refresh the display when we receive a chat message, even if typing
                let state = game_state.lock().unwrap();
                if was_chat || !state.is_typing {
                    render_game_state(&state);
                }
            },
            // Check for typing state updates
            Some(is_typing) = typing_rx.recv() => {
                let mut state = game_state.lock().unwrap();
                state.set_typing(is_typing);
            },
            // Periodically check for messages that need to be resent
            _ = check_interval.tick() => {
                if let Err(e) = network.check_for_resends().await {
                    println!("Error checking for messages to resend: {}", e);
                }
            }
        }
    }
}

/// Process a message received from the server
/// Returns true if the message was a chat message that should refresh the display
fn process_server_message(game_state: &Arc<Mutex<GameState>>, server_message: ServerMessage) -> bool {
    let mut state = game_state.lock().unwrap();
    
    let needs_refresh = match server_message.clone() {
        ServerMessage::RegisterAck { player_id, seq_num: _ } => {
            state.set_player_id(player_id);
            println!("{}", "Registration successful!".green().bold());
            render_game_state(&state);
            false
        },
        ServerMessage::GameState { players, seq_num: _ } => {
            // Debugging output
            println!("{} {}", "Received game state with".cyan().bold(), 
                     format!("{} players", players.len()).yellow().bold());
            
            // Print player IDs
            for player_id in players.keys() {
                println!("  - Player ID: {}", player_id.cyan());
            }
            
            state.update_players(players);
            
            // Render game state immediately to update the mini-map
            render_game_state(&state);
            false
        },
        ServerMessage::Event { message, seq_num: _ } => {
            // Add events to chat history as system messages
            state.add_chat_message("System".to_string(), message.clone());
            println!("{} {}", "Event:".yellow().bold(), message.yellow());
            true
        },
        ServerMessage::ChatMessage { sender_name, message, seq_num: _ } => {
            // Add message to chat history
            state.add_chat_message(sender_name.clone(), message.clone());
            
            // Also print it for immediate visibility
            println!("[{}]: {}", sender_name.green(), message.white());
            true
        },
        ServerMessage::Error { message, seq_num: _ } => {
            // Add errors to chat history as system messages
            state.add_chat_message("System Error".to_string(), message.clone());
            println!("{} {}", "Error:".red().bold(), message.red());
            true
        },
        ServerMessage::Ack { client_seq_num: _, original_type: _ } => {
            // Acknowledgments are handled in the NetworkManager
            // We don't need to do anything here
            false
        }
    };
    
    needs_refresh
}