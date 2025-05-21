mod game_protocol;
mod game_state;
mod network;
mod renderer;

use std::io::{self, Write};
use std::sync::{Arc, Mutex};
use tokio::sync::mpsc;
use tokio::task;
use tokio::time::{self, Duration};
use colored::*;

use game_protocol::{ClientMessage, ServerMessage, Direction};
use game_state::GameState;
use network::NetworkManager;
use renderer::render_game_state;

/// Main entry point for the NYM MMORPG Client
#[tokio::main]
async fn main() -> anyhow::Result<()> {
    println!("=== NYM MMORPG Client ===");
    
    // Initialize the game state
    let game_state: Arc<Mutex<GameState>> = Arc::new(Mutex::new(GameState::new()));
    
    // Initialize network connection
    let mut network = match NetworkManager::new().await {
        Ok(network) => network,
        Err(e) => {
            println!("Error: {}", e);
            return Ok(());
        }
    };
    
    // Create a channel for user input commands
    let (tx, mut rx) = mpsc::channel::<String>(100);
    let tx_clone = tx.clone();
    
    // Create a dedicated channel for controlling typing state
    let (typing_tx, mut typing_rx) = mpsc::channel::<bool>(10);
    
    // Spawn a task to handle user input
    task::spawn(async move {
        loop {
            // Signal that typing has started
            let _ = typing_tx.send(true).await;
            
            let mut input = String::new();
            io::stdin().read_line(&mut input).unwrap();
            let input = input.trim().to_string();
            
            // Signal that typing has ended
            let _ = typing_tx.send(false).await;
            
            if input.is_empty() {
                continue;
            }
            
            if let Err(_) = tx_clone.send(input).await {
                break;
            }
        }
    });
    
    // Render initial state
    render_game_state(&game_state.lock().unwrap());
    
    // Main event loop
    loop {
        tokio::select! {
            // Process user commands
            Some(command) = rx.recv() => {
                process_user_command(&mut network, &game_state, command).await?;
                render_game_state(&game_state.lock().unwrap());
            },
            // Process incoming messages from the server
            Some(server_message) = network.receive_message() => {
                process_server_message(&game_state, server_message);
            },
            // Refresh the display periodically, but not during typing
            _ = time::sleep(Duration::from_secs(2)) => {
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
    
    // This code is unreachable due to the infinite loop above
    // But we'll keep it as a reference for proper shutdown sequence
    println!("Disconnected. Goodbye!");
    
    Ok(())
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
            println!("Disconnecting from Nym network...");
            let network_instance = std::mem::replace(network, unsafe { std::mem::zeroed() });
            tokio::spawn(async move {
                network_instance.disconnect().await;
            });
            println!("Disconnected. Goodbye!");
            
            std::process::exit(0);
        },
        _ => {
            println!("Unknown command: {}", command_parts[0]);
        }
    }
    
    Ok(())
}

/// Process a message received from the server
fn process_server_message(game_state: &Arc<Mutex<GameState>>, server_message: ServerMessage) {
    let mut state = game_state.lock().unwrap();
    
    match server_message {
        ServerMessage::RegisterAck { player_id } => {
            state.set_player_id(player_id);
            println!("Registration successful!");
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