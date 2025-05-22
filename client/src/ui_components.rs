use colored::*;
use std::io::{self, Write};
use std::collections::HashMap;
use chrono::{DateTime, Utc, TimeZone, Timelike};

use crate::game_protocol::{Player, Position};
use crate::game_state::{GameState, ChatMessage};

/// Constants for formatting UI elements
const UI_BORDER_HORIZONTAL: &str = "─";
const UI_BORDER_VERTICAL: &str = "│";
const UI_CORNER_TOP_LEFT: &str = "┌";
const UI_CORNER_TOP_RIGHT: &str = "┐";
const UI_CORNER_BOTTOM_LEFT: &str = "└";
const UI_CORNER_BOTTOM_RIGHT: &str = "┘";
const UI_CONNECTOR_LEFT: &str = "├";
const UI_CONNECTOR_RIGHT: &str = "┤";
const UI_CONNECTOR_TOP: &str = "┬";
const UI_CONNECTOR_BOTTOM: &str = "┴";
const UI_INTERSECTION: &str = "┼";

/// Constants for critical hit detection in messages
const CRITICAL_HIT_MARKER: &str = "CRITICAL HIT";
const CRITICAL_HIT_ALT_MARKER: &str = "critical hit";

/// Define game world boundaries
const WORLD_MIN_X: f32 = -100.0;
const WORLD_MAX_X: f32 = 100.0;
const WORLD_MIN_Y: f32 = -100.0;
const WORLD_MAX_Y: f32 = 100.0;

/// Clear the terminal screen
pub fn clear_screen() {
    print!("{esc}[2J{esc}[1;1H", esc = 27 as char);
    io::stdout().flush().unwrap();
}

/// Draw a horizontal separator with an optional title
pub fn draw_separator(title: Option<&str>, width: usize) {
    match title {
        Some(title_text) => {
            let text = format!(" {} ", title_text);
            let padding_left = (width - text.len()) / 2;
            let padding_right = width - text.len() - padding_left;
            
            let left_border = UI_BORDER_HORIZONTAL.repeat(padding_left);
            let right_border = UI_BORDER_HORIZONTAL.repeat(padding_right);
            
            println!("{}{}{}", left_border.cyan(), text.cyan().bold(), right_border.cyan());
        },
        None => {
            println!("{}", UI_BORDER_HORIZONTAL.repeat(width).cyan());
        }
    }
}

/// Draw a bordered box with a title
pub fn draw_box(title: &str, content: &[String], width: usize) {
    // Top border with title
    let title_display = format!(" {} ", title);
    let remaining_width = width - title_display.len() - 2; // -2 for corners
    let left_border_len = 1;
    let right_border_len = remaining_width;
    
    print!("{}", UI_CORNER_TOP_LEFT.cyan());
    print!("{}", UI_BORDER_HORIZONTAL.repeat(left_border_len).cyan());
    print!("{}", title_display.cyan().bold());
    println!("{}{}", UI_BORDER_HORIZONTAL.repeat(right_border_len).cyan(), UI_CORNER_TOP_RIGHT.cyan());
    
    // Content with side borders
    for line in content {
        // Calculate padding safely, ensuring we don't overflow
        let display_len = line.chars().count(); // Use character count instead of byte length
        let padding = if display_len + 2 <= width {
            width - display_len - 2 // -2 for borders
        } else {
            0 // No padding if content is too long
        };
        
        // Truncate line if it's too long
        let display_line = if display_len > width - 4 {
            // Get a substring that fits within the box (leaving room for ellipsis)
            let truncated = line.chars().take(width - 7).collect::<String>();
            format!("{truncated}...")
        } else {
            line.to_string()
        };
        
        println!("{} {}{} {}", 
                UI_BORDER_VERTICAL.cyan(), 
                display_line,
                " ".repeat(padding),
                UI_BORDER_VERTICAL.cyan());
    }
    
    // Bottom border
    println!("{}{}{}", 
            UI_CORNER_BOTTOM_LEFT.cyan(),
            UI_BORDER_HORIZONTAL.repeat(width - 2).cyan(),
            UI_CORNER_BOTTOM_RIGHT.cyan());
}

/// Format a player name based on their relation to the current player
pub fn format_player_name(player: &Player, current_player_id: &Option<String>) -> ColoredString {
    if Some(&player.id) == current_player_id.as_ref() {
        player.name.green().bold()
    } else {
        player.name.yellow()
    }
}

/// Format health as a colored string with health bar
pub fn format_health(health: u32, max_width: usize) -> String {
    let health_percent = (health as f32 / 100.0).clamp(0.0, 1.0);
    let bar_width = (max_width as f32 * health_percent) as usize;
    
    let health_str = format!("{}", health);
    let health_colored = if health > 70 {
        health_str.green().to_string()
    } else if health > 30 {
        health_str.yellow().to_string()
    } else {
        health_str.red().to_string()
    };
    
    let bar_fill = "█".repeat(bar_width);
    let bar_empty = "░".repeat(max_width - bar_width);
    
    let bar_colored = if health > 70 {
        format!("{}{}", bar_fill.green(), bar_empty)
    } else if health > 30 {
        format!("{}{}", bar_fill.yellow(), bar_empty)
    } else {
        format!("{}{}", bar_fill.red(), bar_empty)
    };
    
    format!("{} {}", health_colored, bar_colored)
}

/// Format a chat message with timestamp and sender highlighting
pub fn format_chat_message(msg: &ChatMessage, timestamp_format: bool) -> String {
    let timestamp = if timestamp_format {
        format_timestamp(msg.timestamp)
    } else {
        "".to_string()
    };
    
    let timestamp_display = if timestamp.is_empty() {
        "".to_string()
    } else {
        format!("[{}] ", timestamp.dimmed())
    };
    
    let sender_formatted = if msg.sender == "System" {
        msg.sender.yellow().to_string()
    } else if msg.sender == "System Error" {
        msg.sender.red().bold().to_string()
    } else {
        msg.sender.green().bold().to_string()
    };
    
    let content_formatted = if msg.content.contains(CRITICAL_HIT_MARKER) || 
                             msg.content.contains(CRITICAL_HIT_ALT_MARKER) {
        msg.content.red().bold().to_string()
    } else {
        msg.content.normal().to_string()
    };
    
    format!("{}{}: {}", timestamp_display, sender_formatted, content_formatted)
}

/// Format a timestamp as a readable time string (HH:MM:SS)
pub fn format_timestamp(timestamp: u64) -> String {
    if let Some(datetime) = Utc.timestamp_millis_opt(timestamp as i64).single() {
        format!("{:02}:{:02}:{:02}", 
            datetime.hour(), 
            datetime.minute(), 
            datetime.second())
    } else {
        "??:??:??".to_string()
    }
}

/// Calculate distance between two positions
pub fn calculate_distance(pos1: &Position, pos2: &Position) -> f32 {
    ((pos1.x - pos2.x).powi(2) + (pos1.y - pos2.y).powi(2)).sqrt()
}

/// Format distance between players
pub fn format_distance(distance: f32) -> ColoredString {
    if distance < 5.0 {
        format!("{:.1}", distance).red().bold()
    } else if distance < 15.0 {
        format!("{:.1}", distance).yellow()
    } else {
        format!("{:.1}", distance).normal()
    }
}

/// Get player attack range status
pub fn get_attack_range_indicator(distance: f32) -> ColoredString {
    // Attack range is defined as 28.0 units in README
    if distance <= 28.0 {
        "IN RANGE".green().bold()
    } else {
        "OUT OF RANGE".red()
    }
}

/// Render enhanced mini-map
pub fn render_mini_map(state: &GameState, current_position: Option<&Position>) {
    const MAP_SIZE: usize = 23; // Larger map for better visibility
    
    let content = vec![
        "Mini-map".cyan().bold().to_string(),
        format!("World boundaries: X: [{:.1}, {:.1}], Y: [{:.1}, {:.1}]", 
                WORLD_MIN_X, WORLD_MAX_X, WORLD_MIN_Y, WORLD_MAX_Y)
    ];
    
    draw_box("WORLD MAP", &content, 60);
    
    // Create an empty map grid
    let mut map = vec![vec![' '; MAP_SIZE]; MAP_SIZE];
    
    // Draw light grid for better cell visualization
    for y in 1..MAP_SIZE-1 {
        for x in 1..MAP_SIZE-1 {
            map[y][x] = '·'; // Light dot for grid points
        }
    }
    
    // Draw border
    for i in 0..MAP_SIZE {
        map[0][i] = UI_BORDER_HORIZONTAL.chars().next().unwrap();
        map[MAP_SIZE-1][i] = UI_BORDER_HORIZONTAL.chars().next().unwrap();
        map[i][0] = UI_BORDER_VERTICAL.chars().next().unwrap();
        map[i][MAP_SIZE-1] = UI_BORDER_VERTICAL.chars().next().unwrap();
    }
    
    // Map corners
    map[0][0] = UI_CORNER_TOP_LEFT.chars().next().unwrap();
    map[0][MAP_SIZE-1] = UI_CORNER_TOP_RIGHT.chars().next().unwrap();
    map[MAP_SIZE-1][0] = UI_CORNER_BOTTOM_LEFT.chars().next().unwrap();
    map[MAP_SIZE-1][MAP_SIZE-1] = UI_CORNER_BOTTOM_RIGHT.chars().next().unwrap();
    
    // Get current player from state
    let current_player_id = state.player_id.clone();
    let current_player_pos = match current_position {
        Some(pos) => pos,
        None => match state.current_player() {
            Some(player) => &player.position,
            None => return, // Exit if no position available
        }
    };
    
    // Place coordinate markers
    for i in 1..MAP_SIZE-1 {
        if i % 5 == 0 {
            // Calculate the actual world coordinates at this grid position
            let x_coord = WORLD_MIN_X + (WORLD_MAX_X - WORLD_MIN_X) * (i as f32 / (MAP_SIZE as f32 - 2.0));
            let y_coord = WORLD_MIN_Y + (WORLD_MAX_Y - WORLD_MIN_Y) * (i as f32 / (MAP_SIZE as f32 - 2.0));
            
            // Format for display in single-digit format to save space
            let x_label = format!("{:.0}", x_coord);
            let y_label = format!("{:.0}", y_coord);
            
            // X-axis markers (on bottom)
            map[MAP_SIZE-1][i] = '+';
            
            // Y-axis markers (on left)
            map[i][0] = '+';
            
            // Mark the grid with '+' symbols only, without displaying coordinate numbers on the map itself
            // This makes the map cleaner while still showing the grid points
        }
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
    }
    
    // Draw the map with colors
    for (y, row) in map.iter().enumerate() {
        let mut line = String::new();
        
        for (x, &c) in row.iter().enumerate() {
            // Special formatting for map elements
            match c {
                '@' => line.push_str(&c.to_string().green().bold().to_string()),
                'O' => line.push_str(&c.to_string().yellow().to_string()),
                '·' => {
                    // Highlight cells based on distance from player
                    let cell_x = WORLD_MIN_X + (WORLD_MAX_X - WORLD_MIN_X) * ((x as f32 - 1.0) / (MAP_SIZE as f32 - 2.0));
                    let cell_y = WORLD_MIN_Y + (WORLD_MAX_Y - WORLD_MIN_Y) * ((y as f32 - 1.0) / (MAP_SIZE as f32 - 2.0));
                    
                    let distance = ((current_player_pos.x - cell_x).powi(2) + 
                                   (current_player_pos.y - cell_y).powi(2)).sqrt();
                    
                    // Change dot color based on distance from player (attack range visualization)
                    if distance <= 5.0 {
                        line.push_str(&c.to_string().green().to_string());
                    } else if distance <= 28.0 { // Attack range
                        line.push_str(&c.to_string().yellow().dimmed().to_string());
                    } else {
                        line.push_str(&c.to_string().normal().dimmed().to_string());
                    }
                },
                '+' => line.push_str(&c.to_string().cyan().to_string()),
                _ => {
                    if c == UI_BORDER_HORIZONTAL.chars().next().unwrap() || 
                       c == UI_BORDER_VERTICAL.chars().next().unwrap() ||
                       c == UI_CORNER_TOP_LEFT.chars().next().unwrap() ||
                       c == UI_CORNER_TOP_RIGHT.chars().next().unwrap() ||
                       c == UI_CORNER_BOTTOM_LEFT.chars().next().unwrap() ||
                       c == UI_CORNER_BOTTOM_RIGHT.chars().next().unwrap() {
                        line.push_str(&c.to_string().cyan().to_string());
                    } else {
                        line.push_str(&c.to_string());
                    }
                }
            };
        }
        println!("{}", line);
    }
    
    // Add a legend
    println!("{} = You  {} = Other Players  {} = Attack Range", 
             "@".green().bold(), 
             "O".yellow(), 
             "·".yellow().dimmed());
}

/// Render chat history in an enhanced format
pub fn render_chat_history(state: &GameState, max_messages: usize) {
    let messages = state.recent_chat_messages(max_messages);
    
    let mut content = Vec::with_capacity(messages.len() + 1);
    content.push("Recent messages:".cyan().bold().to_string());
    
    if messages.is_empty() {
        content.push("No messages yet.".dimmed().to_string());
    } else {
        for msg in messages {
            content.push(format_chat_message(msg, true));
        }
    }
    
    draw_box("CHAT HISTORY", &content, 80);
}

/// Render player stats in a compact format
pub fn render_player_stats(player: &Player, is_current: bool) {
    let title = if is_current { "YOUR STATS" } else { "PLAYER STATS" };
    
    let name_display = if is_current {
        player.name.green().bold().to_string()
    } else {
        player.name.yellow().to_string()
    };
    
    let content = vec![
        format!("Name: {} [{}]", name_display, player.display_id.cyan()),
        format!("Position: ({:.1}, {:.1})", player.position.x, player.position.y),
        format!("Health: {}", format_health(player.health, 20)),
    ];
    
    draw_box(title, &content, 60);
}

/// Render a list of nearby players with distance and attack status
pub fn render_nearby_players(state: &GameState) {
    // Only proceed if player is registered
    let current_player = match state.current_player() {
        Some(player) => player,
        None => return,
    };
    
    let mut nearby_players = Vec::new();
    nearby_players.push("Nearby players:".cyan().bold().to_string());
    
    let mut players_with_distance: Vec<_> = state.players.iter()
        .filter(|(id, _)| Some(*id) != state.player_id.as_ref())
        .map(|(id, player)| {
            let distance = calculate_distance(&current_player.position, &player.position);
            (id, player, distance)
        })
        .collect();
    
    // Sort by distance (closest first)
    players_with_distance.sort_by(|a, b| a.2.partial_cmp(&b.2).unwrap());
    
    if players_with_distance.is_empty() {
        nearby_players.push("No other players nearby.".dimmed().to_string());
    } else {
        for (id, player, distance) in players_with_distance {
            let distance_colored = format_distance(distance);
            let range_indicator = get_attack_range_indicator(distance);
            
            nearby_players.push(format!(
                "{} [{}] - Distance: {} units ({})",
                player.name.yellow(),
                player.display_id.cyan(),
                distance_colored,
                range_indicator
            ));
        }
    }
    
    draw_box("NEARBY PLAYERS", &nearby_players, 80);
}

/// Render help information
pub fn render_help_section() {
    let commands = vec![
        "Available Commands:".cyan().bold().to_string(),
        "/register <name> - Register with the given name".to_string(),
        "/move <direction> - Move (up, down, left, right, n, s, e, w, ne, nw, se, sw)".to_string(),
        "/attack <player_id> - Attack player with the given display ID".to_string(),
        "/chat <message> - Send a chat message to all players".to_string(),
        "/help - Show this help information".to_string(),
        "/quit - Exit the game".to_string(),
    ];
    
    draw_box("HELP", &commands, 80);
}

/// Render command input prompt
pub fn render_input_prompt() {
    print!("\n{} ", ">".cyan().bold());
    io::stdout().flush().unwrap();
}

/// Render the current game state to the terminal
pub fn render_game_state(state: &GameState) {
    clear_screen();
    
    // Game title and header
    println!("{}", "╔═══════════════════════════════════════════════════════════╗".cyan());
    println!("{} {} {}", "║".cyan(), "              NYM QUEST - PRIVACY MMORPG              ".green().bold(), "║".cyan());
    println!("{}", "╚═══════════════════════════════════════════════════════════╝".cyan());
    
    // Display last update time with more context
    let elapsed = state.last_update.elapsed();
    let update_status = if elapsed.as_secs() < 2 {
        "DATA IS CURRENT".green().bold()
    } else if elapsed.as_secs() < 10 {
        "DATA UPDATED RECENTLY".yellow()
    } else {
        "DATA MAY BE OUTDATED".red().bold()
    };
    
    println!("{} {} seconds ago", update_status, elapsed.as_secs());
    
    // Layout based on registration status
    match &state.player_id {
        Some(id) => {
            // User is registered
            println!("{}", format!("Connected via Nym Mixnet - ID: {}", id.blue()));
            
            if let Some(player) = state.players.get(id) {
                // Player stats section
                render_player_stats(player, true);
                
                // Render enhanced mini-map with player position
                render_mini_map(state, Some(&player.position));
                
                // Render nearby players with distance info
                render_nearby_players(state);
                
                // Render chat history with enhanced formatting
                render_chat_history(state, 10);
                
                // Render help section at the bottom
                render_help_section();
            }
        },
        None => {
            // User is not registered yet
            let content = vec![
                "Welcome to NymQuest!".green().bold().to_string(),
                "".to_string(),
                "You are not registered yet.".yellow().to_string(),
                "Use '/register <name>' to join the game with your chosen name.".to_string(),
                "".to_string(),
                "Your privacy is protected through the Nym mixnet technology.".cyan().to_string(),
                "All game communications are anonymous and metadata-protected.".cyan().to_string(),
            ];
            
            draw_box("WELCOME", &content, 70);
            
            // Render abbreviated help for non-registered users
            let commands = vec![
                "Available Commands:".cyan().bold().to_string(),
                "/register <name> - Register with your chosen display name".to_string(),
                "/help - Show this help information".to_string(),
                "/quit - Exit the game".to_string(),
            ];
            
            draw_box("COMMANDS", &commands, 70);
        }
    }
    
    // Command input prompt
    render_input_prompt();
}
