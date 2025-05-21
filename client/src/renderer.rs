use colored::*;
use std::io::{self, Write};
use std::collections::HashMap;
use chrono::{DateTime, Utc, TimeZone, Timelike};

use crate::game_protocol::{Player, Position};
use crate::game_state::{GameState, ChatMessage};

/// Clear the terminal screen
pub fn clear_screen() {
    print!("{esc}[2J{esc}[1;1H", esc = 27 as char);
    io::stdout().flush().unwrap();
}

/// Render a mini-map of the game world
pub fn render_mini_map(state: &GameState, player_pos: &Position) {
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
    
    // Always display the current player at their position
    println!("Player position: x={}, y={}", player_pos.x, player_pos.y);
    
    let player_map_x = 1 + ((player_pos.x - world_min_x as f32) * scale_x) as usize;
    let player_map_y = 1 + ((player_pos.y - world_min_y as f32) * scale_y) as usize;
    
    println!("Calculated position on map: x={}, y={}", player_map_x, player_map_y);
    
    // Ensure position is within bounds
    if player_map_x > 0 && player_map_x < MAP_SIZE-1 && player_map_y > 0 && player_map_y < MAP_SIZE-1 {
        println!("Player is within map bounds!");
        map[player_map_y][player_map_x] = '@';
    } else {
        println!("Player is OUT OF BOUNDS of the map!");
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

/// Format a timestamp as a readable time string (HH:MM:SS)
fn format_timestamp(timestamp: u64) -> String {
    // Convert milliseconds timestamp to DateTime<Utc>
    let dt = match Utc.timestamp_millis_opt(timestamp as i64) {
        chrono::LocalResult::Single(dt) => dt,
        _ => return "[??:??:??]".to_string(),
    };
    
    // Format as HH:MM:SS
    format!("[{:02}:{:02}:{:02}]", dt.hour(), dt.minute(), dt.second())
}

/// Render the chat history in a dedicated area
pub fn render_chat_history(state: &GameState, max_messages: usize) {
    println!("{}", "===== Chat History =====".cyan().bold());
    
    // Get recent messages (limited to max_messages)
    let messages = state.recent_chat_messages(max_messages);
    
    if messages.is_empty() {
        println!("{}", "No messages yet.".italic());
    } else {
        for msg in messages {
            let time_str = format_timestamp(msg.timestamp);
            
            // Format based on sender type
            match msg.sender.as_str() {
                "System" => {
                    println!("{} {}", time_str.yellow(), msg.content.yellow());
                },
                "System Error" => {
                    println!("{} {}", time_str.yellow(), msg.content.red().bold());
                },
                sender if Some(sender) == state.player_id.as_deref() => {
                    // Current player's messages
                    println!("{} {} {}", 
                             time_str.blue(), 
                             format!("[You]:").blue().bold(), 
                             msg.content.white());
                },
                _ => {
                    // Other players' messages
                    println!("{} {} {}", 
                             time_str.green(), 
                             format!("[{}]:", msg.sender).green().bold(), 
                             msg.content.white());
                }
            }
        }
    }
    println!("{}", "======================".cyan());
}

/// Render the current game state to the terminal
pub fn render_game_state(state: &GameState) {
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
            println!("Not registered yet. Use 'register <n>' to join the game.");
        }
    }
    
    println!("\nCommands:");
    println!("  register <n> - Register with the given name");
    if state.player_id.is_some() {
        println!("  move <direction> - Move your character (up, down, left, right)");
        println!("  attack <player_id> - Attack another player");
        println!("  chat <message> - Send a message to all players");
    }
    println!("  exit - Quit the game");
    
    // Render chat history (showing last 10 messages)
    println!("");
    render_chat_history(state, 10);
    
    print!("\n> ");
    io::stdout().flush().unwrap();
}
