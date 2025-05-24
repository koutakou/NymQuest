use colored::*;
use std::io::{self, Write};
use chrono::{Utc, TimeZone, Timelike};

use crate::game_protocol::Position;
use crate::game_state::GameState;

/// Constants for critical hit detection in messages
const CRITICAL_HIT_MARKER: &str = "CRITICAL HIT";
const CRITICAL_HIT_ALT_MARKER: &str = "critical hit";

/// Clear the terminal screen
#[allow(dead_code)] // Alternative rendering function for future use
pub fn clear_screen() {
    print!("{esc}[2J{esc}[1;1H", esc = 27 as char);
    io::stdout().flush().unwrap();
}

/// Render a mini-map of the game world
/// 
/// This function creates a scaled-down representation of the game world with the following features:
/// - The current player is represented as '@' (green)
/// - Other players are represented as 'O' (yellow)
/// - Game world coordinates (-100 to 100) are scaled to fit the mini-map
#[allow(dead_code)] // Alternative rendering function for future use
pub fn render_mini_map(state: &GameState, _player_pos: &Position) {
    const MAP_SIZE: usize = 15; // Size of the mini-map (15x15 characters)
    const WORLD_MIN_X: f32 = -100.0;
    const WORLD_MAX_X: f32 = 100.0;
    const WORLD_MIN_Y: f32 = -100.0;
    const WORLD_MAX_Y: f32 = 100.0;

    println!("{}", "Mini-map: (You are '@', others are 'O'):".cyan().bold());
    
    // Debug info about world dimensions
    println!("World boundaries: X: [{:.1}, {:.1}], Y: [{:.1}, {:.1}]", 
             WORLD_MIN_X, WORLD_MAX_X, WORLD_MIN_Y, WORLD_MAX_Y);
    
    // Create an empty map grid with borders and light grid
    let mut map = vec![vec![' '; MAP_SIZE]; MAP_SIZE];
    
    // Draw light grid for better cell visualization
    #[allow(clippy::needless_range_loop)]
    for y in 1..MAP_SIZE-1 {
        for x in 1..MAP_SIZE-1 {
            map[y][x] = '·'; // Light dot for grid points
        }
    }
    
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
    
    // Calculate scale factors to map world coordinates to minimap coordinates
    let _scale_x = (MAP_SIZE - 2) as f32 / (WORLD_MAX_X - WORLD_MIN_X);
    let _scale_y = (MAP_SIZE - 2) as f32 / (WORLD_MAX_Y - WORLD_MIN_Y);
    
    // Get current player from state
    let current_player_id = state.player_id.clone();
    
    // Debug current player position if available
    if let Some(current_player) = state.current_player() {
        println!("Current player at: ({:.1}, {:.1})", 
                 current_player.position.x, current_player.position.y);
    }
    
    // Place all players on the map
    for (id, player) in &state.players {
        // Convert world coordinates to map coordinates
        let norm_x = (player.position.x - WORLD_MIN_X) / (WORLD_MAX_X - WORLD_MIN_X);
        let norm_y = (player.position.y - WORLD_MIN_Y) / (WORLD_MAX_Y - WORLD_MIN_Y);
        
        let map_x = (norm_x * (MAP_SIZE - 2) as f32) as usize + 1;
        let map_y = (norm_y * (MAP_SIZE - 2) as f32) as usize + 1;
        
        // Safety check for map boundaries
        let map_x = map_x.clamp(1, MAP_SIZE - 2);
        let map_y = map_y.clamp(1, MAP_SIZE - 2);
        
        // Choose character based on whether this is the current player
        let symbol = if Some(id) == current_player_id.as_ref() {
            '@' // Current player
        } else {
            'O' // Other player
        };
        
        // Place on map
        map[map_y][map_x] = symbol;
        
        // Debug each player placement
        println!("Player {}: World: ({:.1}, {:.1}) → Map: ({}, {}){}", 
                 player.name, player.position.x, player.position.y, map_x, map_y,
                 if Some(id) == current_player_id.as_ref() { " (YOU)" } else { "" });
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
    
    // Display cardinal directions for orientation
    println!("    N   ");
    println!("  W + E ");
    println!("    S   ");
    
    // Add information about movement scale
    println!();
    println!("{}:", "Movement Help".cyan().bold());
    println!("  Each {} command moves exactly one cell", "'move <direction>'".green().bold());
    println!("  Valid directions: {}, {}, {}, {}", 
             "cardinal (n,s,e,w)".yellow(), 
             "diagonal (nw,ne,sw,se)".yellow(),
             "arrows (up,down,left,right)".yellow(),
             "shortcuts (u,d,l,r)".yellow());
}

/// Format a timestamp as a readable time string (HH:MM:SS)
#[allow(dead_code)] // Alternative rendering function for future use
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
#[allow(dead_code)] // Alternative rendering function for future use
pub fn render_chat_history(state: &GameState, max_messages: usize) {
    println!("{}", "===== Chat History =====".cyan().bold());
    
    // Get recent messages (limited to max_messages)
    let messages = state.recent_chat_messages(max_messages);
    
    if messages.is_empty() {
        println!("{}", "No messages yet.".italic());
    } else {
        for msg in messages {
            let time_str = format_timestamp(msg.timestamp);
            
            // Check if message contains critical hit information
            let is_critical = msg.content.contains(CRITICAL_HIT_MARKER) || 
                             msg.content.contains(CRITICAL_HIT_ALT_MARKER);
            
            // Format the content with special handling for critical hits
            let content_formatted = if is_critical {
                // Highlight critical hit messages with bright red and bold text
                msg.content.bright_red().bold()
            } else {
                msg.content.white()
            };
            
            // Format based on sender type
            match msg.sender.as_str() {
                "System" => {
                    println!("{} {}", time_str.yellow(), content_formatted);
                },
                "System Error" => {
                    println!("{} {}", time_str.yellow(), content_formatted.red().bold());
                },
                sender if Some(sender) == state.player_id.as_deref() => {
                    // Current player's messages
                    println!("{} {} {}", 
                             time_str.blue(), 
                             "[You]:".blue().bold(), 
                             content_formatted);
                },
                _ => {
                    // Other players' messages
                    println!("{} {} {}", 
                             time_str.green(), 
                             format!("[{}]:", msg.sender).green().bold(), 
                             content_formatted);
                }
            }
        }
    }
    println!("{}", "======================".cyan());
}

/// Render the current game state to the terminal
#[allow(dead_code)] // Alternative rendering function for future use
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
                    
                    println!("  {} [{}]: At ({:.1}, {:.1}), Health: {}", 
                        player_name,
                        player.display_id.cyan(), // Display the anonymized ID in cyan
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
        println!("  move <direction> - Move your character one cell on the map");
        println!("    (use cardinal: n,s,e,w or diagonal: ne,nw,se,sw or arrows: up,down,left,right)");
        println!("  attack <player_display_id> - Attack another player using their display ID (shown in [brackets])");
        println!("  chat <message> - Send a message to all players");
    }
    println!("  exit - Quit the game");
    
    // Render chat history (showing last 10 messages)
    render_chat_history(state, 10);
    
    print!("> ");
    io::stdout().flush().unwrap();
}
