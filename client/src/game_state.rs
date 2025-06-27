use std::collections::{HashMap, VecDeque};
use std::sync::{Arc, Mutex};
use std::time::{Instant, SystemTime, UNIX_EPOCH};

use crate::game_protocol::{Player, WorldBoundaries};
use crate::status_monitor::StatusMonitor;
use crate::world_lore::Faction;

/// Represents a chat message with sender, content, timestamp, and message type
/// Uses Arc<String> for efficient sharing of strings
pub struct ChatMessage {
    pub sender: Arc<String>,
    pub content: String,
    pub timestamp: u64,
    pub message_type: MessageType,
}

/// Type of message for display purposes
#[derive(Clone, Copy, PartialEq)]
pub enum MessageType {
    /// Regular public chat message
    Chat,
    /// Private whisper message
    Whisper,
    /// System message
    System,
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
    /// Last whisper sender name - for reply functionality
    pub last_whisper_sender: Option<Arc<String>>,
}

impl GameState {
    /// Create a new empty game state
    pub fn new() -> Self {
        Self {
            player_id: None,
            players: HashMap::with_capacity(200), // Increased capacity for better performance
            is_typing: false,
            last_update: Instant::now(),
            chat_history: VecDeque::with_capacity(100), // Increased capacity
            max_chat_history: 100,
            world_boundaries: None,
            status_monitor: Arc::new(Mutex::new(StatusMonitor::new())),
            last_whisper_sender: None,
        }
    }

    /// Check if the player is registered
    pub fn is_registered(&self) -> bool {
        self.player_id.is_some()
    }

    /// Get the player's faction if registered
    pub fn player_faction(&self) -> Option<Faction> {
        if let Some(player_id) = &self.player_id {
            if let Some(player) = self.players.get(player_id) {
                return Some(player.faction.clone());
            }
        }
        None
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
            sender: Arc::new(sender),
            content,
            timestamp,
            message_type: MessageType::Chat,
        };

        // Add to history (most recent at the end)
        self.chat_history.push_back(message);

        // Ensure we don't exceed the maximum history size
        while self.chat_history.len() > self.max_chat_history {
            self.chat_history.pop_front();
        }
    }

    /// Add a system message to the history
    pub fn add_system_message(&mut self, sender: String, content: String) {
        // Get current timestamp in milliseconds
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64;

        // Create a new system message
        let message = ChatMessage {
            sender: Arc::new(sender),
            content,
            timestamp,
            message_type: MessageType::System,
        };

        // Add to history (most recent at the end)
        self.chat_history.push_back(message);

        // Ensure we don't exceed the maximum history size
        while self.chat_history.len() > self.max_chat_history {
            self.chat_history.pop_front();
        }
    }

    /// Add a whisper message to the history and update the last whisper sender
    pub fn add_whisper_message(&mut self, sender: String, content: String) {
        // Get current timestamp in milliseconds
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64;

        // Create shared sender reference
        let sender_arc = Arc::new(sender.clone());

        // Create a new whisper message
        let message = ChatMessage {
            sender: sender_arc.clone(),
            content,
            timestamp,
            message_type: MessageType::Whisper,
        };

        // Add to history (most recent at the end)
        self.chat_history.push_back(message);

        // Update the last whisper sender for reply functionality
        self.last_whisper_sender = Some(sender_arc);

        // Ensure we don't exceed the maximum history size
        while self.chat_history.len() > self.max_chat_history {
            self.chat_history.pop_front();
        }
    }

    /// Get the last whisper sender if any
    pub fn get_last_whisper_sender(&self) -> Option<&Arc<String>> {
        self.last_whisper_sender.as_ref()
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

    /// Get player ID by display ID/name (case insensitive)
    pub fn get_player_id_by_display_id(&self, display_id: &str) -> Option<String> {
        // Case insensitive comparison
        let lowercase_target = display_id.to_lowercase();

        // Find the player with the matching display_id and return their ID
        for (id, player) in &self.players {
            if player.display_id.to_lowercase() == lowercase_target {
                return Some(id.clone());
            }
        }
        None
    }

    /// Get connection tag for a player by ID
    pub fn get_connection_tag(&self, player_id: &str) -> Option<String> {
        // Find the player with the matching ID and return their name as the connection tag
        // Note: In the actual implementation, we may need a different field depending on how connections are tracked
        self.players
            .get(player_id)
            .map(|player| player.name.clone())
    }
}
