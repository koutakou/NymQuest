use nym_sdk::mixnet::{MixnetClientBuilder, MixnetMessageSender, StoragePaths, Recipient, IncludedSurbs};
use std::str::FromStr;
use std::path::PathBuf;
use std::fs;
use std::io::{self, Write};
use futures::StreamExt;
use std::sync::{Arc, Mutex};
use tokio::sync::mpsc;
use tokio::task;
use tokio::time::{self, Duration};
use std::collections::HashMap;
use colored::*;
use uuid::Uuid; // Import the uuid crate

mod game_protocol;
use game_protocol::{Player, ClientMessage, ServerMessage, Direction};

// Structure to hold client state
struct GameState {
    player_id: Option<String>,
    players: HashMap<String, Player>,
    is_typing: bool,
    last_update: std::time::Instant,
}

impl GameState {
    fn new() -> Self {
        Self {
            player_id: None,
            players: HashMap::new(),
            is_typing: false,
            last_update: std::time::Instant::now(),
        }
    }
}

// Clear the terminal screen
fn clear_screen() {
    print!("{esc}[2J{esc}[1;1H", esc = 27 as char);
    io::stdout().flush().unwrap();
}

// Render a mini-map of the game world
fn render_mini_map(state: &GameState, player_pos: &game_protocol::Position) {
    const MAP_SIZE: usize = 11; // Must be odd to have center for player
    
    println!("{}", "Mini-map: (You are '@', others are 'O'):".cyan().bold());
    
    // Create an empty map grid with borders
    let mut map = vec![vec![' '; MAP_SIZE]; MAP_SIZE];
    
    // Draw border
    for i in 0..MAP_SIZE {
        map[0][i] = '-';
        map[MAP_SIZE-1][i] = '-';
        map[i][0] = '|';
        map[i][MAP_SIZE-1] = '|';
    }
    
    // Map corners
    map[0][0] = '+';
    map[0][MAP_SIZE-1] = '+';
    map[MAP_SIZE-1][0] = '+';
    map[MAP_SIZE-1][MAP_SIZE-1] = '+';
    
    // Define world bounds for the minimap
    // Adjusted world coordinates based on actual game values
    let world_min_x = 0.0;
    let world_min_y = -100.0;
    let world_max_x = 100.0;
    let world_max_y = 100.0;
    
    // Calculate scale factors to map world coordinates to minimap coordinates
    let scale_x = (MAP_SIZE - 2) as f32 / (world_max_x - world_min_x) as f32;
    let scale_y = (MAP_SIZE - 2) as f32 / (world_max_y - world_min_y) as f32;
    
    // Place all players on the map based on their absolute position
    for (id, player) in &state.players {
        let x = player.position.x;
        let y = player.position.y;
        
        // Convert world coordinates to map coordinates
        let map_x = 1 + ((x - world_min_x as f32) * scale_x) as usize;
        let map_y = 1 + ((y - world_min_y as f32) * scale_y) as usize;
        
        // Only place if within bounds and not on border
        if map_x > 0 && map_x < MAP_SIZE-1 && map_y > 0 && map_y < MAP_SIZE-1 {
            // Use different symbols for current player vs other players
            if Some(id) == state.player_id.as_ref() {
                map[map_y][map_x] = '@';
            } else {
                map[map_y][map_x] = 'O';
            }
        }
    }
    
    // Toujours afficher le joueur actuel à sa position (player_pos)
    println!("Position du joueur: x={}, y={}", player_pos.x, player_pos.y);
    
    let player_map_x = 1 + ((player_pos.x - world_min_x as f32) * scale_x) as usize;
    let player_map_y = 1 + ((player_pos.y - world_min_y as f32) * scale_y) as usize;
    
    println!("Position calculée sur la map: x={}, y={}", player_map_x, player_map_y);
    
    // S'assurer que la position est dans les limites
    if player_map_x > 0 && player_map_x < MAP_SIZE-1 && player_map_y > 0 && player_map_y < MAP_SIZE-1 {
        println!("Le joueur est dans les limites de la map!");
        map[player_map_y][player_map_x] = '@';
    } else {
        println!("Le joueur est HORS LIMITES de la map!");
    }
    
    // Draw the map with colors
    for row in map {
        let colored_row: String = row.iter().map(|&c| {
            match c {
                '@' => c.to_string().green().bold().to_string(),
                'O' => c.to_string().yellow().to_string(),
                '|' | '-' | '+' => c.to_string().cyan().to_string(),
                _ => c.to_string()
            }
        }).collect();
        println!("  {}", colored_row);
    }
}

// Render the current game state to the terminal
fn render_game_state(state: &GameState) {
    clear_screen();
    println!("{}\n", "=== NYM MMORPG Client ===".green().bold());
    
    // Display last update time
    let elapsed = state.last_update.elapsed();
    let update_status = if elapsed.as_secs() < 2 {
        "Data is current".green()
    } else if elapsed.as_secs() < 10 {
        "Data updated recently".yellow()
    } else {
        "Data may be outdated".red()
    };
    println!("{} ({} seconds ago)", update_status, elapsed.as_secs());
    
    match &state.player_id {
        Some(id) => {
            println!("You are registered with ID: {}", id.blue());
            
            if let Some(player) = state.players.get(id) {
                println!("Your name: {}", player.name.green().bold());
                println!("Position: ({:.1}, {:.1})", player.position.x, player.position.y);
                
                // Colorize health based on value
                let health_str = format!("{}", player.health);
                let health_colored = if player.health > 70 {
                    health_str.green()
                } else if player.health > 30 {
                    health_str.yellow()
                } else {
                    health_str.red()
                };
                println!("Health: {}", health_colored);
                
                println!("{}", "-----------------------------------".cyan());
                
                // Render mini-map
                render_mini_map(state, &player.position);
                println!("{}", "-----------------------------------".cyan());
            }
            
            println!("Players in game: {}", state.players.len().to_string().yellow().bold());
            
            for (id, player) in &state.players {
                if Some(id) != state.player_id.as_ref() {
                    let distance_x = if let Some(current_player) = state.player_id.as_ref().and_then(|pid| state.players.get(pid)) {
                        ((player.position.x - current_player.position.x).powi(2) + 
                         (player.position.y - current_player.position.y).powi(2)).sqrt()
                    } else {
                        f32::MAX
                    };
                    
                    // Color players based on distance (closer = more noticeable)
                    let player_name = if distance_x < 3.0 {
                        player.name.red().bold()
                    } else if distance_x < 7.0 {
                        player.name.yellow()
                    } else {
                        player.name.normal()
                    };
                    
                    println!("  {}: At ({:.1}, {:.1}), Health: {}", 
                        player_name,
                        player.position.x, player.position.y, 
                        player.health);
                }
            }
        },
        None => {
            println!("Not registered yet. Use 'register <name>' to join the game.");
        }
    }
    
    println!("\nCommands:");
    println!("  register <name> - Register with the given name");
    if state.player_id.is_some() {
        println!("  move <direction> - Move your character (up, down, left, right)");
        println!("  attack <player_id> - Attack another player");
        println!("  chat <message> - Send a message to all players");
    }
    println!("  exit - Quit the game");
    print!("\n> ");
    io::stdout().flush().unwrap();
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    println!("=== NYM MMORPG Client ===");
    
    // Initialize the game state with specific type annotation to emphasize it's an Arc
    let game_state: Arc<Mutex<GameState>> = Arc::new(Mutex::new(GameState::new()));
    
    // Read server address from file
    let server_address = match fs::read_to_string("server_address.txt").or_else(|_| fs::read_to_string("../client/server_address.txt")) {
        Ok(address) => address.trim().to_string(),
        Err(_) => {
            println!("Error: Cannot read server address from server_address.txt");
            println!("Make sure the server is running and you have access to the address file.");
            return Ok(());
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
    
    let mut client = client.connect_to_mixnet().await?;
    
    println!("Connected to Nym network!");
    
    // Create a channel for user input commands
    let (tx, mut rx) = mpsc::channel::<String>(100);
    let tx_clone = tx.clone();
    
    // Create a dedicated channel for controlling typing state
    let (typing_tx, mut typing_rx) = mpsc::channel::<bool>(10);
    
    // Spawn a task to handle user input without capturing game_state
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
                let command_parts: Vec<&str> = command.split_whitespace().collect();
                
                if command_parts.is_empty() {
                    continue;
                }
                
                match command_parts[0] {
                    "register" => {
                        if game_state.lock().unwrap().player_id.is_none() {
                            if command_parts.len() < 2 {
                                println!("Usage: register <n>");
                                continue;
                            }
                            
                            let name = command_parts[1..].join(" ");
                            let register_msg = ClientMessage::Register { name };
                            let message = serde_json::to_string(&register_msg)?;
                            
                            // Send registration request to server
                            let recipient = Recipient::from_str(&server_address)
                                .map_err(|e| anyhow::anyhow!("Invalid server address: {}", e))?;
                            client.send_message(recipient, message.into_bytes(), IncludedSurbs::default()).await?;
                            println!("Registration request sent...");
                        } else {
                            println!("You are already registered!");
                        }
                    },
                    "move" => {
                        // Check if player is registered
                        if game_state.lock().unwrap().player_id.is_none() {
                            println!("You need to register first before you can move.");
                            continue;
                        }
                        
                        if command_parts.len() < 2 {
                            println!("Usage: move <direction> (up, down, left, right)");
                            continue;
                        }
                        
                        // Parse direction
                        let direction = match command_parts[1].to_lowercase().as_str() {
                            "up" => Direction::Up,
                            "down" => Direction::Down,
                            "left" => Direction::Left,
                            "right" => Direction::Right,
                            _ => {
                                println!("Invalid direction. Use up, down, left, or right.");
                                continue;
                            }
                        };
                        
                        // Create move message
                        let move_msg = ClientMessage::Move { direction };
                        let message = serde_json::to_string(&move_msg)?;
                        
                        // Send move request to server
                        let recipient = Recipient::from_str(&server_address)
                            .map_err(|e| anyhow::anyhow!("Invalid server address: {}", e))?;
                        client.send_message(recipient, message.into_bytes(), IncludedSurbs::default()).await?;
                        println!("Move request sent...");
                    },
                    "attack" => {
                        // Check if player is registered
                        if game_state.lock().unwrap().player_id.is_none() {
                            println!("You need to register first before you can attack.");
                            continue;
                        }
                        
                        if command_parts.len() < 2 {
                            println!("Usage: attack <player_id>");
                            continue;
                        }
                        
                        let target_id = command_parts[1].to_string();
                        let attack_msg = ClientMessage::Attack { target_id };
                        let message = serde_json::to_string(&attack_msg)?;
                        
                        // Send attack request to server
                        let recipient = Recipient::from_str(&server_address)
                            .map_err(|e| anyhow::anyhow!("Invalid server address: {}", e))?;
                        client.send_message(recipient, message.into_bytes(), IncludedSurbs::default()).await?;
                        println!("Attack request sent...");
                    },
                    "chat" => {
                        // Check if player is registered
                        if game_state.lock().unwrap().player_id.is_none() {
                            println!("You need to register first before you can chat.");
                            continue;
                        }
                        
                        if command_parts.len() < 2 {
                            println!("Usage: chat <message>");
                            continue;
                        }
                        
                        let message_text = command_parts[1..].join(" ");
                        let chat_msg = ClientMessage::Chat { message: message_text };
                        let message = serde_json::to_string(&chat_msg)?;
                        
                        // Send chat message to server
                        let recipient = Recipient::from_str(&server_address)
                            .map_err(|e| anyhow::anyhow!("Invalid server address: {}", e))?;
                        client.send_message(recipient, message.into_bytes(), IncludedSurbs::default()).await?;
                        println!("Chat message sent...");
                    },
                    "exit" => {
                        // Send disconnect message if registered
                        if game_state.lock().unwrap().player_id.is_some() {
                            let disconnect_msg = ClientMessage::Disconnect;
                            let message = serde_json::to_string(&disconnect_msg)?;
                            
                            // Create recipient from server address
                            if let Ok(recipient) = Recipient::from_str(&server_address) {
                                let _ = client.send_message(recipient, message.into_bytes(), IncludedSurbs::default()).await;
                                println!("Disconnect message sent to server");
                            } else {
                                println!("Failed to parse server address for disconnect message");
                            }
                        }
                        
                        break;
                    },
                    _ => {
                        println!("Unknown command: {}", command_parts[0]);
                    }
                }
                
                render_game_state(&game_state.lock().unwrap());
            },
            // Process incoming messages from the server
            Some(received_message) = client.next() => {
                if received_message.message.is_empty() {
                    continue;
                }
                
                match String::from_utf8(received_message.message.clone()) {
                    Ok(message_str) => {
                        match serde_json::from_str::<ServerMessage>(&message_str) {
                            Ok(server_message) => {
                                let mut state = game_state.lock().unwrap();
                                
                                match server_message {
                                    ServerMessage::RegisterAck { player_id } => {
                                        state.player_id = Some(player_id);
                                        state.last_update = std::time::Instant::now();
                                        println!("Registration successful!");
                                        render_game_state(&state);
                                    },
                                    ServerMessage::GameState { players } => {
                                        // Debugging output to see what players are actually received
                                        println!("{} {}", "Received game state with".cyan().bold(), 
                                                 format!("{} players", players.len()).yellow().bold());
                                        
                                        // Print player IDs to help debug
                                        for player_id in players.keys() {
                                            println!("  - Player ID: {}", player_id.cyan());
                                        }
                                        
                                        state.players = players;
                                        state.last_update = std::time::Instant::now();
                                        
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
                                
                                drop(state);
                            },
                            Err(e) => println!("Error deserializing server message: {}", e),
                        }
                    },
                    Err(e) => println!("Error parsing message: {}", e),
                }
            },
            // Refresh the display only when game state changes, not during typing
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
                state.is_typing = is_typing;
            }
        }
    }
    
    // Clean up and disconnect
    println!("Disconnecting from Nym network...");
    client.disconnect().await;
    println!("Disconnected. Goodbye!");
    
    Ok(())
}
