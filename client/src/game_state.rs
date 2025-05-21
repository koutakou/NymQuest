use std::collections::HashMap;
use std::time::Instant;

use crate::game_protocol::Player;

/// Structure to hold client state
pub struct GameState {
    pub player_id: Option<String>,
    pub players: HashMap<String, Player>,
    pub is_typing: bool,
    pub last_update: Instant,
}

impl GameState {
    /// Create a new empty game state
    pub fn new() -> Self {
        Self {
            player_id: None,
            players: HashMap::new(),
            is_typing: false,
            last_update: Instant::now(),
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
}
