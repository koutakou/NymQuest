mod game_protocol;
mod game_state;
mod network;
mod renderer;

use std::sync::{Arc, Mutex};
use tokio::sync::mpsc;
use tokio::task;
use colored::*;
use rustyline::error::ReadlineError;
use rustyline::Editor;
use std::path::PathBuf;

use game_protocol::{ClientMessage, ServerMessage, Direction};
use game_state::GameState;
use network::NetworkManager;
use renderer::render_game_state;

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
            if !game_state.lock().unwrap().is_registered() {
                if command_parts.len() < 2 {
                    println!("Usage: register <name>");
                    return Ok(());
                }
                
                let name = command_parts[1..].join(" ");
                let register_msg = ClientMessage::Register { name };
                
                network.send_message(register_msg).await?;
                println!("Registration request sent...");
            } else {
                println!("You are already registered!");
            }
        },
        "move" => {
            // Check if player is registered
            if !game_state.lock().unwrap().is_registered() {
                println!("You need to register first before you can move.");
                return Ok(());
            }
            
            if command_parts.len() < 2 {
                println!("Usage: move <direction> (up, down, left, right)");
                return Ok(());
            }
            
            // Parse direction
            let direction = match command_parts[1].to_lowercase().as_str() {
                "up" => Direction::Up,
                "down" => Direction::Down,
                "left" => Direction::Left,
                "right" => Direction::Right,
                _ => {
                    println!("Invalid direction. Use up, down, left, or right.");
                    return Ok(());
                }
            };
            
            // Create and send move message
            let move_msg = ClientMessage::Move { direction };
            network.send_message(move_msg).await?;
            println!("Move request sent...");
        },
        "attack" => {
            // Check if player is registered
            if !game_state.lock().unwrap().is_registered() {
                println!("You need to register first before you can attack.");
                return Ok(());
            }
            
            if command_parts.len() < 2 {
                println!("Usage: attack <player_id>");
                return Ok(());
            }
            
            let target_id = command_parts[1].to_string();
            let attack_msg = ClientMessage::Attack { target_id };
            
            network.send_message(attack_msg).await?;
            println!("Attack request sent...");
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
            let chat_msg = ClientMessage::Chat { message: message_text };
            
            network.send_message(chat_msg).await?;
            println!("Chat message sent...");
        },
        "exit" => {
            // Send disconnect message if registered
            if game_state.lock().unwrap().is_registered() {
                let disconnect_msg = ClientMessage::Disconnect;
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
                process_server_message(game_state, server_message);
                // Only refresh if not typing
                let state = game_state.lock().unwrap();
                if !state.is_typing {
                    render_game_state(&state);
                }
            },
            // Check for typing state updates
            Some(is_typing) = typing_rx.recv() => {
                let mut state = game_state.lock().unwrap();
                state.set_typing(is_typing);
            }
        }
    }
}

/// Process a message received from the server
fn process_server_message(game_state: &Arc<Mutex<GameState>>, server_message: ServerMessage) {
    let mut state = game_state.lock().unwrap();
    
    match server_message {
        ServerMessage::RegisterAck { player_id } => {
            state.set_player_id(player_id);
            println!("{}", "Registration successful!".green().bold());
            render_game_state(&state);
        },
        ServerMessage::GameState { players } => {
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
        },
        ServerMessage::Event { message } => {
            println!("{} {}", "Event:".yellow().bold(), message.yellow());
        },
        ServerMessage::ChatMessage { sender_name, message } => {
            println!("[{}]: {}", sender_name.green(), message.white());
        },
        ServerMessage::Error { message } => {
            println!("{} {}", "Error:".red().bold(), message.red());
        }
    }
}