use chrono::{TimeZone, Timelike, Utc};
use colored::*;
use std::io::{self, Write};

use crate::game_protocol::{Player, Position};
use crate::game_state::{ChatMessage, GameState};
use crate::status_monitor::{ConnectionHealth, PrivacyLevel};

/// Modern Unicode box drawing characters for a sleek interface
const DOUBLE_HORIZONTAL: &str = "═";
#[allow(dead_code)] // Part of complete box drawing character set for future UI enhancements
const DOUBLE_VERTICAL: &str = "║";
#[allow(dead_code)] // Part of complete box drawing character set for future UI enhancements
const DOUBLE_TOP_LEFT: &str = "╔";
#[allow(dead_code)] // Part of complete box drawing character set for future UI enhancements
const DOUBLE_TOP_RIGHT: &str = "╗";
#[allow(dead_code)] // Part of complete box drawing character set for future UI enhancements
const DOUBLE_BOTTOM_LEFT: &str = "╚";
#[allow(dead_code)] // Part of complete box drawing character set for future UI enhancements
const DOUBLE_BOTTOM_RIGHT: &str = "╝";
#[allow(dead_code)] // Part of complete box drawing character set for future UI enhancements
const DOUBLE_CROSS: &str = "╬";
#[allow(dead_code)] // Part of complete box drawing character set for future UI enhancements
const DOUBLE_T_DOWN: &str = "╦";
#[allow(dead_code)] // Part of complete box drawing character set for future UI enhancements
const DOUBLE_T_UP: &str = "╩";
#[allow(dead_code)] // Part of complete box drawing character set for future UI enhancements
const DOUBLE_T_RIGHT: &str = "╠";
#[allow(dead_code)] // Part of complete box drawing character set for future UI enhancements
const DOUBLE_T_LEFT: &str = "╣";

const SINGLE_HORIZONTAL: &str = "─";
#[allow(dead_code)] // Part of complete box drawing character set for future UI enhancements
const SINGLE_VERTICAL: &str = "│";
#[allow(dead_code)] // Part of complete box drawing character set for future UI enhancements
const SINGLE_TOP_LEFT: &str = "┌";
#[allow(dead_code)] // Part of complete box drawing character set for future UI enhancements
const SINGLE_TOP_RIGHT: &str = "┐";
#[allow(dead_code)] // Part of complete box drawing character set for future UI enhancements
const SINGLE_BOTTOM_LEFT: &str = "└";
#[allow(dead_code)] // Part of complete box drawing character set for future UI enhancements
const SINGLE_BOTTOM_RIGHT: &str = "┘";

const LIGHT_HORIZONTAL: &str = "┄";
#[allow(dead_code)] // Part of complete box drawing character set for future UI enhancements
const LIGHT_VERTICAL: &str = "┊";

/// Modern UI symbols and indicators
const ICON_SHIELD: &str = "🛡️";
#[allow(dead_code)] // Part of complete icon set for future UI enhancements
const ICON_HEART: &str = "❤️";
const ICON_LOCATION: &str = "📍";
const ICON_USERS: &str = "👥";
const ICON_CHAT: &str = "💬";
const ICON_NETWORK: &str = "🌐";
const ICON_TIME: &str = "🕐";
const ICON_PRIVACY: &str = "🔒";
const ICON_WARNING: &str = "⚠️";
const ICON_SUCCESS: &str = "✅";
const ICON_INFO: &str = "ℹ️";
const ICON_ARROW_RIGHT: &str = "→";
#[allow(dead_code)] // Part of complete icon set for future UI enhancements
const ICON_PACING: &str = "⏱️";
const ICON_BULLET: &str = "•";

/// Health bar and progress indicators
const HEALTH_FULL: &str = "█";
#[allow(dead_code)] // Part of complete health bar character set for future UI enhancements
const HEALTH_THREE_QUARTERS: &str = "▉";
#[allow(dead_code)] // Part of complete health bar character set for future UI enhancements
const HEALTH_HALF: &str = "▌";
#[allow(dead_code)] // Part of complete health bar character set for future UI enhancements
const HEALTH_QUARTER: &str = "▎";
const HEALTH_EMPTY: &str = "░";

/// Constants for critical hit detection in messages
const CRITICAL_HIT_MARKER: &str = "CRITICAL HIT";
const CRITICAL_HIT_ALT_MARKER: &str = "critical hit";

/// Terminal width for responsive design
const TERMINAL_WIDTH: usize = 120;
const PANEL_WIDTH: usize = 58; // Half width for side-by-side panels

/// Clear the terminal screen with modern escape sequences
pub fn clear_screen() {
    print!("\x1B[2J\x1B[H");
    io::stdout().flush().unwrap();
}

/// Draw a modern header with gradient effect
pub fn draw_header() {
    let title = "NYM QUEST";
    let subtitle = "Privacy-First MMORPG";

    println!();
    println!("{}", DOUBLE_HORIZONTAL.repeat(TERMINAL_WIDTH).cyan().bold());

    // Calculate centering for title
    let title_padding = (TERMINAL_WIDTH - title.len()) / 2;
    let subtitle_padding = (TERMINAL_WIDTH - subtitle.len()) / 2;

    println!(
        "{}{}{}",
        " ".repeat(title_padding),
        title.bright_magenta().bold(),
        " ".repeat(TERMINAL_WIDTH - title_padding - title.len())
    );

    println!(
        "{}{}{}",
        " ".repeat(subtitle_padding),
        subtitle.bright_cyan(),
        " ".repeat(TERMINAL_WIDTH - subtitle_padding - subtitle.len())
    );

    println!("{}", DOUBLE_HORIZONTAL.repeat(TERMINAL_WIDTH).cyan().bold());
    println!();
}

/// Draw a modern panel with title
pub fn draw_panel(title: &str, content: &[String], width: usize, style: PanelStyle) {
    let panel_width = width.clamp(40, TERMINAL_WIDTH - 4); // Ensure minimum width and prevent overflow

    // Choose border style based on panel type
    let (h_char, title_color) = match style {
        PanelStyle::Primary => (DOUBLE_HORIZONTAL, Color::BrightCyan),
        PanelStyle::Secondary => (SINGLE_HORIZONTAL, Color::Cyan),
        PanelStyle::Accent => (SINGLE_HORIZONTAL, Color::Yellow),
        PanelStyle::Info => (LIGHT_HORIZONTAL, Color::Blue),
    };

    // Top border with title
    let title_display = format!(" {} ", title);
    let title_len = strip_ansi_codes(&title_display).chars().count();

    if title_len >= panel_width - 2 {
        // Full line if title is too long
        println!("{}", h_char.repeat(panel_width).color(title_color));
        println!("{}", title_display.color(title_color).bold());
    } else {
        let available_space = panel_width - title_len;
        let left_padding = available_space / 2;
        let right_padding = available_space - left_padding;

        println!(
            "{}{}{}",
            h_char.repeat(left_padding).color(title_color),
            title_display.color(title_color).bold(),
            h_char.repeat(right_padding).color(title_color)
        );
    }

    // Content without side borders - much cleaner
    for line in content {
        println!(" {}", line); // Simple indentation for content
    }

    // Bottom border
    println!("{}", h_char.repeat(panel_width).color(title_color));
}

/// Panel styling options
#[derive(Clone, Copy)]
pub enum PanelStyle {
    Primary,   // Double borders, bright cyan
    Secondary, // Single borders, cyan
    Accent,    // Single borders, yellow
    Info,      // Light borders, blue
}

/// Draw a status bar with indicators
pub fn draw_status_bar(left_text: &str, right_text: &str) {
    let left_len = strip_ansi_codes(left_text).len();
    let right_len = strip_ansi_codes(right_text).len();
    let padding = TERMINAL_WIDTH.saturating_sub(left_len + right_len);

    println!("{}{}{}", left_text, " ".repeat(padding), right_text);
}

/// Create a modern health bar
pub fn create_health_bar(health: u32, max_health: u32, width: usize) -> String {
    let percentage = health as f32 / max_health as f32;
    let filled_width = (percentage * width as f32) as usize;
    let empty_width = width - filled_width;

    let color = if percentage > 0.7 {
        Color::Green
    } else if percentage > 0.3 {
        Color::Yellow
    } else {
        Color::Red
    };

    format!(
        "{}{}{}",
        HEALTH_FULL.repeat(filled_width).color(color).bold(),
        HEALTH_EMPTY
            .repeat(empty_width)
            .color(Color::Black)
            .on_black(),
        format!(" {}%", (percentage * 100.0) as u8).color(color)
    )
}

/// Strip ANSI color codes from a string for accurate length calculation
fn strip_ansi_codes(input: &str) -> String {
    let mut result = String::new();
    let mut chars = input.chars().peekable();

    while let Some(ch) = chars.next() {
        if ch == '\x1b' {
            if chars.peek() == Some(&'[') {
                chars.next(); // consume '['
                for escape_ch in chars.by_ref() {
                    if escape_ch.is_ascii_alphabetic() {
                        break;
                    }
                }
            }
        } else {
            result.push(ch);
        }
    }

    result
}

/// Format a player name based on their relation to the current player
#[allow(dead_code)] // Part of complete UI API for future enhancements
pub fn format_player_name(player: &Player, current_player_id: &Option<String>) -> ColoredString {
    if Some(&player.id) == current_player_id.as_ref() {
        player.name.bright_green().bold()
    } else {
        player.name.bright_yellow()
    }
}

/// Format health as a modern colored health bar
pub fn format_health(health: u32, bar_width: usize) -> String {
    create_health_bar(health, 100, bar_width)
}

/// Format a chat message with modern styling
pub fn format_chat_message(msg: &ChatMessage, timestamp_format: bool) -> String {
    let timestamp_display = if timestamp_format {
        format!("[{}] ", format_timestamp(msg.timestamp).bright_black())
    } else {
        String::new()
    };

    let sender_formatted = if msg.sender == "System" || msg.sender == "Server" {
        format!("{}: ", msg.sender.bright_magenta().bold())
    } else {
        format!("{}: ", msg.sender.bright_cyan().bold())
    };

    let content_formatted = if msg.content.contains(CRITICAL_HIT_MARKER)
        || msg.content.contains(CRITICAL_HIT_ALT_MARKER)
    {
        msg.content.bright_red().bold().to_string()
    } else {
        msg.content.normal().to_string()
    };

    format!(
        "{}{}{}",
        timestamp_display, sender_formatted, content_formatted
    )
}

/// Format a timestamp as a readable time string (HH:MM:SS)
pub fn format_timestamp(timestamp: u64) -> String {
    if let Some(datetime) = Utc.timestamp_millis_opt(timestamp as i64).single() {
        format!(
            "{:02}:{:02}:{:02}",
            datetime.hour(),
            datetime.minute(),
            datetime.second()
        )
    } else {
        "??:??:??".to_string()
    }
}

/// Calculate distance between two positions
pub fn calculate_distance(pos1: &Position, pos2: &Position) -> f32 {
    ((pos1.x - pos2.x).powi(2) + (pos1.y - pos2.y).powi(2)).sqrt()
}

/// Format distance between players with modern indicators
pub fn format_distance(distance: f32) -> ColoredString {
    if distance < 5.0 {
        format!("{:.1}m", distance).bright_red().bold()
    } else if distance < 15.0 {
        format!("{:.1}m", distance).bright_yellow()
    } else {
        format!("{:.1}m", distance).white()
    }
}

/// Get player attack range status with modern indicators
pub fn get_attack_range_indicator(distance: f32) -> ColoredString {
    if distance <= 28.0 {
        format!("{} IN RANGE", ICON_SUCCESS).bright_green().bold()
    } else {
        format!("{} OUT OF RANGE", ICON_WARNING).bright_red()
    }
}

/// Render modern mini-map with enhanced visuals
pub fn render_mini_map(state: &GameState, current_position: Option<&Position>) {
    const MAP_SIZE: usize = 19;

    // Get current player position
    let current_player_pos = match current_position {
        Some(pos) => pos,
        None => match state.current_player() {
            Some(player) => &player.position,
            None => return,
        },
    };

    // Create map grid
    let mut map = vec![vec![' '; MAP_SIZE]; MAP_SIZE];

    // Fill with terrain dots
    #[allow(clippy::needless_range_loop)]
    for y in 1..MAP_SIZE - 1 {
        for x in 1..MAP_SIZE - 1 {
            map[y][x] = '·';
        }
    }

    // Add coordinate markers
    for i in 1..MAP_SIZE - 1 {
        if i % 4 == 0 {
            map[MAP_SIZE - 1][i] = '┼';
            map[i][0] = '┼';
        }
    }

    // Place players
    let current_player_id = state.player_id.clone();

    // Get world boundaries for accurate positioning
    let (min_x, max_x, min_y, max_y) = if let Some(boundaries) = state.get_world_boundaries() {
        (
            boundaries.min_x,
            boundaries.max_x,
            boundaries.min_y,
            boundaries.max_y,
        )
    } else {
        // Fallback to default values if boundaries aren't available
        (0.0, 100.0, 0.0, 100.0)
    };

    for (id, player) in &state.players {
        // Normalize positions using actual world boundaries
        let norm_x = (player.position.x - min_x) / (max_x - min_x);
        let norm_y = (player.position.y - min_y) / (max_y - min_y);

        let map_x = (norm_x * (MAP_SIZE - 2) as f32) as usize + 1;
        let map_y = (norm_y * (MAP_SIZE - 2) as f32) as usize + 1;

        let map_x = map_x.clamp(1, MAP_SIZE - 2);
        let map_y = map_y.clamp(1, MAP_SIZE - 2);

        let symbol = if Some(id) == current_player_id.as_ref() {
            '@'
        } else {
            '●'
        };

        map[map_y][map_x] = symbol;
    }

    // Build styled map content
    let mut map_content = Vec::new();

    // World info
    map_content.push(format!(
        "{} World: X[{:.0},{:.0}] Y[{:.0},{:.0}]",
        ICON_LOCATION, min_x, max_x, min_y, max_y
    ));
    map_content.push(format!(
        "{} Position: ({:.1}, {:.1})",
        ICON_ARROW_RIGHT, current_player_pos.x, current_player_pos.y
    ));
    map_content.push("".to_string());

    // Map display
    for row in &map {
        let mut line = String::new();
        for &c in row {
            match c {
                '@' => line.push_str(&c.to_string().bright_green().bold().to_string()),
                '●' => line.push_str(&c.to_string().bright_yellow().to_string()),
                '·' => line.push_str(&c.to_string().blue().dimmed().to_string()),
                '┼' => line.push_str(&c.to_string().cyan().to_string()),
                _ => line.push(c),
            }
        }
        map_content.push(line);
    }

    // Legend
    map_content.push("".to_string());
    map_content.push(format!(
        "{} You  {} Others  {} Terrain",
        "@".bright_green().bold(),
        "●".bright_yellow(),
        "·".blue().dimmed()
    ));

    draw_panel(
        "🗺️  WORLD MAP",
        &map_content,
        PANEL_WIDTH,
        PanelStyle::Secondary,
    );
}

/// Render player stats with modern layout
pub fn render_player_stats(player: &Player, is_current: bool) {
    let title = if is_current {
        format!("{}  PLAYER STATUS", ICON_SHIELD)
    } else {
        format!("{}  OTHER PLAYER", ICON_USERS)
    };

    let name_display = if is_current {
        player.name.bright_green().bold().to_string()
    } else {
        player.name.bright_yellow().to_string()
    };

    let content = vec![
        format!("Name: {}", name_display),
        format!("ID: {}", player.display_id.bright_cyan()),
        format!(
            "Position: ({:.1}, {:.1})",
            player.position.x, player.position.y
        ),
        format!("Health: {}", format_health(player.health, 15)),
    ];

    draw_panel(&title, &content, PANEL_WIDTH, PanelStyle::Primary);
}

/// Render chat history with modern styling
pub fn render_chat_history(state: &GameState, max_messages: usize) {
    let messages = state.recent_chat_messages(max_messages);

    let mut content = Vec::new();

    if messages.is_empty() {
        content.push(
            format!("{}  No messages yet", ICON_INFO)
                .dimmed()
                .to_string(),
        );
    } else {
        for msg in messages {
            content.push(format_chat_message(msg, true));
        }
    }

    draw_panel(
        &format!("{}  CHAT HISTORY", ICON_CHAT),
        &content,
        TERMINAL_WIDTH - 4,
        PanelStyle::Info,
    );
}

/// Render nearby players with modern indicators
pub fn render_nearby_players(state: &GameState) {
    let current_player = match state.current_player() {
        Some(player) => player,
        None => return,
    };

    let mut content = Vec::new();

    let mut players_with_distance: Vec<_> = state
        .players
        .iter()
        .filter(|(id, _)| Some(*id) != state.player_id.as_ref())
        .map(|(id, player)| {
            let distance = calculate_distance(&current_player.position, &player.position);
            (id, player, distance)
        })
        .collect();

    players_with_distance.sort_by(|a, b| a.2.partial_cmp(&b.2).unwrap());

    if players_with_distance.is_empty() {
        content.push(
            format!("{}  No other players detected", ICON_INFO)
                .dimmed()
                .to_string(),
        );
    } else {
        for (_, player, distance) in players_with_distance {
            let distance_colored = format_distance(distance);
            let range_indicator = get_attack_range_indicator(distance);

            content.push(format!(
                "{}  {} {} {} {}",
                ICON_BULLET,
                player.name.bright_yellow(),
                format!("[{}]", player.display_id).cyan(),
                distance_colored,
                range_indicator
            ));
        }
    }

    draw_panel(
        &format!("{}  NEARBY PLAYERS", ICON_USERS),
        &content,
        PANEL_WIDTH,
        PanelStyle::Accent,
    );
}

/// Render privacy and connection status with modern indicators
pub fn render_status_dashboard(state: &GameState) {
    // Get a lock on the status monitor
    let status_monitor = match state.status_monitor.lock() {
        Ok(monitor) => monitor,
        Err(_) => {
            // Handle poisoned mutex gracefully
            let error_content = vec![format!("{} Status monitoring unavailable", ICON_WARNING)
                .bright_red()
                .to_string()];
            draw_panel(
                &format!("{} CONNECTION STATUS", ICON_NETWORK),
                &error_content,
                PANEL_WIDTH,
                PanelStyle::Primary,
            );
            return;
        }
    };

    // Health and privacy descriptions
    let (health_desc, privacy_desc) = status_monitor.status_description();

    // Get the color for the privacy description
    let privacy_color = match status_monitor.privacy_level {
        PrivacyLevel::FullyProtected => "green".to_string(),
        PrivacyLevel::Protected => "cyan".to_string(),
        PrivacyLevel::Degraded => "yellow".to_string(),
        PrivacyLevel::Compromised => "red".to_string(),
    };

    // Create colored version
    let privacy_colored = match status_monitor.privacy_level {
        PrivacyLevel::FullyProtected => privacy_desc.green().bold(),
        PrivacyLevel::Protected => privacy_desc.cyan().bold(),
        PrivacyLevel::Degraded => privacy_desc.yellow().bold(),
        PrivacyLevel::Compromised => privacy_desc.red().bold(),
    };

    // Format connection health description
    let health_colored = match status_monitor.connection_health {
        ConnectionHealth::Excellent => health_desc.green().bold(),
        ConnectionHealth::Good => health_desc.cyan().bold(),
        ConnectionHealth::Fair => health_desc.yellow().bold(),
        ConnectionHealth::Poor => health_desc.red(),
        ConnectionHealth::Critical => health_desc.red().bold(),
    };

    // Status panel with enhanced details
    let mut content = vec![
        format!("{} Privacy Level: {}", ICON_PRIVACY, privacy_colored),
        format!(
            "   {} Anonymity set: {} participants",
            ICON_USERS,
            status_monitor.anonymity_set_size.to_string().cyan()
        ),
        "".to_string(),
        format!("{} Connection: {}", ICON_NETWORK, health_colored),
    ];

    // Add pacing status if enabled
    if status_monitor.pacing_info.enabled {
        content.push(format!(
            "   {} Message pacing: {} ms",
            ICON_TIME,
            status_monitor.pacing_info.interval_ms.to_string().cyan()
        ));
    }

    // Display network statistics
    content.push("".to_string());
    content.push(format!(
        "{} Average latency: {} ms",
        ICON_INFO,
        status_monitor
            .network_stats
            .avg_latency_ms
            .to_string()
            .cyan()
    ));

    content.push(format!(
        "   {} Messages sent: {}",
        ICON_ARROW_RIGHT,
        status_monitor
            .network_stats
            .messages_sent
            .to_string()
            .cyan()
    ));

    content.push(format!(
        "   {} Messages received: {}",
        ICON_ARROW_RIGHT,
        status_monitor
            .network_stats
            .messages_delivered
            .to_string()
            .cyan()
    ));

    // Add success rate if we have sent any messages
    if status_monitor.network_stats.messages_sent > 0 {
        let success_rate = status_monitor.network_stats.delivery_success_rate();
        let success_rate_str = format!("{:.1}%", success_rate);
        let colored_rate = if success_rate > 90.0 {
            success_rate_str.green()
        } else if success_rate > 75.0 {
            success_rate_str.yellow()
        } else {
            success_rate_str.red()
        };

        content.push(format!(
            "   {} Delivery success rate: {}",
            ICON_SUCCESS, colored_rate
        ));
    }

    // Display latest game state update if available
    if let Some(game_update) = status_monitor.get_game_state_info() {
        content.push("".to_string());
        content.push(format!(
            "{} Game Status: {}",
            ICON_INFO,
            game_update.cyan().bold()
        ));
    }

    // Display latest connection update if available
    if let Some(conn_update) = status_monitor.get_connection_info() {
        content.push(format!("   {} {}", ICON_NETWORK, conn_update.yellow()));
    }

    // Format status panel
    let panel_style = match status_monitor.privacy_level {
        PrivacyLevel::FullyProtected => PanelStyle::Primary,
        PrivacyLevel::Protected => PanelStyle::Secondary,
        PrivacyLevel::Degraded => PanelStyle::Accent,
        PrivacyLevel::Compromised => PanelStyle::Info,
    };

    draw_panel(
        &format!(
            "{} PRIVACY STATUS ({})",
            ICON_PRIVACY,
            privacy_color.to_uppercase()
        ),
        &content,
        PANEL_WIDTH,
        panel_style,
    );
}

/// Render help section with modern formatting
pub fn render_help_section() {
    let commands = vec![
        format!("{} NymQuest Commands:", ICON_INFO).bright_green().bold().to_string(),
        "".to_string(),
        format!("{} /register <name>, /r <name> - Register with the server using the specified name", ICON_BULLET),
        format!("{} /move <direction>, /m <direction> - Move in the specified direction", ICON_BULLET),
        "    Valid directions: up, down, left, right, upleft, upright, downleft, downright".to_string(),
        "    Direction shortcuts: /up (or /u, /n), /down (or /d, /s), /left (or /l, /w), /right (or /r, /e)".to_string(),
        "    Diagonal movement: /ne, /nw, /se, /sw - Move diagonally".to_string(),
        format!("{} /attack <player_id>, /a <player_id> - Attack player with the given display ID", ICON_BULLET),
        format!("{} /chat <message>, /c <message>, /say <message> - Send a chat message to all players", ICON_BULLET),
        format!("{} /emote <type>, /em <type> - Perform an emote action", ICON_BULLET),
        "    Available emotes: wave, bow, laugh, dance, salute, shrug, cheer, clap".to_string(),
        format!("{} /pacing [on|off] [interval_ms], /pace - Control message pacing for privacy protection", ICON_BULLET),
        "    Examples: /pacing on 150, /pacing off, /pacing status - View or modify timing protection".to_string(),
        format!("{} /help, /h, /? - Show this help information", ICON_BULLET),
        format!("{} /quit, /exit, /q - Exit the game", ICON_BULLET),
        "".to_string(),
        format!("{} Attack range: 28.0 units | Movement speed: 5.0 units per move", ICON_INFO).bright_green().to_string(),
    ];

    draw_panel(
        "🎮  COMMANDS",
        &commands,
        TERMINAL_WIDTH - 4,
        PanelStyle::Info,
    );
}

/// Render command input prompt with modern styling
pub fn render_input_prompt() {
    println!();
    print!("{}  ", "❯".bright_magenta().bold());
    io::stdout().flush().unwrap();
}

/// Main game state renderer with modern layout
pub fn render_game_state(state: &GameState) {
    clear_screen();

    // Header
    draw_header();

    // Connection status bar
    let elapsed = state.last_update.elapsed();
    let status_text = if elapsed.as_secs() < 5 {
        format!("{} LIVE", ICON_SUCCESS).bright_green()
    } else if elapsed.as_secs() < 30 {
        format!("{} RECENT", ICON_INFO).bright_yellow()
    } else {
        format!("{} DELAYED", ICON_WARNING).bright_red()
    };

    let time_text = format!("Updated {}s ago", elapsed.as_secs())
        .bright_black()
        .to_string();
    draw_status_bar(&status_text.to_string(), &time_text);
    println!();

    match &state.player_id {
        Some(id) => {
            // Connected state
            if let Some(player) = state.players.get(id) {
                // Two-column layout for main content
                println!();

                // Row 1: Player stats and connection status
                render_player_stats(player, true);
                println!();
                render_status_dashboard(state);
                println!();

                // Row 2: Map and nearby players
                render_mini_map(state, Some(&player.position));
                println!();
                render_nearby_players(state);
                println!();

                // Full width sections
                render_chat_history(state, 8);
                println!();
                render_help_section();
            }
        }
        None => {
            // Welcome state for unregistered users
            let welcome_content = vec![
                format!("{}  Welcome to NymQuest!", ICON_SUCCESS)
                    .bright_green()
                    .bold()
                    .to_string(),
                "".to_string(),
                format!("{}  You are not registered yet", ICON_INFO)
                    .bright_yellow()
                    .to_string(),
                format!("{}  Use /register <name> to join", ICON_ARROW_RIGHT),
                "".to_string(),
                format!("{}  Privacy protected by Nym mixnet", ICON_PRIVACY)
                    .bright_cyan()
                    .to_string(),
                format!("{}  All communications are anonymous", ICON_SHIELD)
                    .bright_cyan()
                    .to_string(),
            ];

            draw_panel(
                "🌟  WELCOME TO NYMQUEST",
                &welcome_content,
                80,
                PanelStyle::Primary,
            );
            println!();

            let quick_commands = vec![
                format!("{}  /register <name> - Join with chosen name", ICON_BULLET),
                format!("{}  /help - Show all commands", ICON_BULLET),
                format!("{}  /quit - Exit game", ICON_BULLET),
            ];

            draw_panel(
                "🚀  QUICK START",
                &quick_commands,
                80,
                PanelStyle::Secondary,
            );
        }
    }

    println!();
    render_input_prompt();
}
