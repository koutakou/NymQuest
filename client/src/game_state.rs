use std::collections::{HashMap, VecDeque};
use std::sync::{Arc, Mutex};
use std::time::{Instant, SystemTime, UNIX_EPOCH};

use crate::game_protocol::{Player, WorldBoundaries};
use crate::status_monitor::StatusMonitor;

/// Represents a chat message with sender, content and timestamp
pub struct ChatMessage {
    pub sender: String,
    pub content: String,
    pub timestamp: u64,
}

/// Structure to hold client state
pub struct GameState {
    pub player_id: Option<String>,
    pub players: HashMap<String, Player>,
    pub is_typing: bool,
    pub last_update: Instant,
    /// Chat history with most recent messages at the end
    pub chat_history: VecDeque<ChatMessage>,
    /// Maximum number of chat messages to store in history
    pub max_chat_history: usize,
    /// World boundaries received from server during registration
    pub world_boundaries: Option<WorldBoundaries>,
    pub status_monitor: Arc<Mutex<StatusMonitor>>,
}

impl GameState {
    /// Create a new empty game state
    pub fn new() -> Self {
        Self {
            player_id: None,
            players: HashMap::new(),
            is_typing: false,
            last_update: Instant::now(),
            chat_history: VecDeque::with_capacity(50),
            max_chat_history: 50,
            world_boundaries: None,
            status_monitor: Arc::new(Mutex::new(StatusMonitor::new())),
        }
    }

    /// Check if the player is registered
    pub fn is_registered(&self) -> bool {
        self.player_id.is_some()
    }

    /// Get the current player if registered
    pub fn current_player(&self) -> Option<&Player> {
        self.player_id.as_ref().and_then(|id| self.players.get(id))
    }

    /// Set the player ID when registration is successful
    pub fn set_player_id(&mut self, id: String) {
        self.player_id = Some(id);
        self.update_timestamp();
    }

    /// Get the player ID if registered
    pub fn get_player_id(&self) -> Option<&String> {
        self.player_id.as_ref()
    }

    /// Update the game state with new player data
    pub fn update_players(&mut self, players: HashMap<String, Player>) {
        self.players = players;
        self.update_timestamp();
    }

    /// Update the last update timestamp
    pub fn update_timestamp(&mut self) {
        self.last_update = Instant::now();
    }

    /// Set the typing state
    pub fn set_typing(&mut self, is_typing: bool) {
        self.is_typing = is_typing;
    }

    /// Add a new chat message to the history
    pub fn add_chat_message(&mut self, sender: String, content: String) {
        // Get current timestamp in milliseconds
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64;

        // Create a new chat message
        let message = ChatMessage {
            sender,
            content,
            timestamp,
        };

        // Add to history (most recent at the end)
        self.chat_history.push_back(message);

        // Ensure we don't exceed the maximum history size
        while self.chat_history.len() > self.max_chat_history {
            self.chat_history.pop_front();
        }
    }

    /// Get a slice of the most recent chat messages
    pub fn recent_chat_messages(&self, count: usize) -> Vec<&ChatMessage> {
        let history_len = self.chat_history.len();
        let start_idx = history_len.saturating_sub(count);

        self.chat_history.iter().skip(start_idx).collect()
    }

    /// Set world boundaries received from server
    pub fn set_world_boundaries(&mut self, boundaries: WorldBoundaries) {
        self.world_boundaries = Some(boundaries);
    }

    /// Get world boundaries if available
    pub fn get_world_boundaries(&self) -> Option<&WorldBoundaries> {
        self.world_boundaries.as_ref()
    }
}
