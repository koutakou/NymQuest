use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use uuid::Uuid;
use std::time::{SystemTime, UNIX_EPOCH};
use rand::{thread_rng, Rng};

use nym_sdk::mixnet::AnonymousSenderTag;
use crate::game_protocol::{Player, Position};

/// Type alias for a player ID and its associated sender tag
pub type PlayerTag = (String, AnonymousSenderTag);

/// GameState manages the entire game state including players and connections
pub struct GameState {
    /// Map of player IDs to Player objects
    players: Mutex<HashMap<String, Player>>,
    /// List of active connections (player_id, sender_tag)
    connections: Mutex<Vec<PlayerTag>>,
}

impl GameState {
    /// Create a new empty GameState
    pub fn new() -> Self {
        GameState {
            players: Mutex::new(HashMap::new()),
            connections: Mutex::new(Vec::new()),
        }
    }

    /// Add a new player to the game
    pub fn add_player(&self, name: String, sender_tag: AnonymousSenderTag) -> String {
        // Generate a unique ID for the player
        let player_id = Uuid::new_v4().to_string();
        
        // Generate a position that's not occupied by another player
        let available_position = {
            let state = self.players.lock().unwrap();
            self.generate_available_position(&state)
        };
        
        // Get current time for initializing attack cooldown
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        
        // Generate a privacy-preserving display ID
        // This will be a simple format like "Player1", "Player2", etc.
        let display_id = {
            let mut state = self.players.lock().unwrap();
            format!("Player{}", state.len() + 1)
        };
            
        // Create a new player with the available position
        let player = Player {
            id: player_id.clone(),
            display_id,
            name,
            position: available_position,
            health: 100,
            last_attack_time: now,
        };
        
        // Add the player to the game state
        self.players.lock().unwrap().insert(player_id.clone(), player);
        
        // Store this active connection
        self.connections.lock().unwrap().push((player_id.clone(), sender_tag));
        
        player_id
    }

    /// Remove a player from the game
    pub fn remove_player(&self, tag: &AnonymousSenderTag) -> Option<String> {
        let mut connection_index = None;
        let mut player_id_to_remove = None;
        
        // Find the player's ID and index in the connections list
        {
            let connections = self.connections.lock().unwrap();
            for (i, (id, conn_tag)) in connections.iter().enumerate() {
                if conn_tag.to_string() == tag.to_string() {
                    player_id_to_remove = Some(id.clone());
                    connection_index = Some(i);
                    break;
                }
            }
        }
        
        // Remove the player if found
        if let Some(index) = connection_index {
            // Remove from active connections
            self.connections.lock().unwrap().remove(index);
            
            // Remove from game state
            if let Some(id) = &player_id_to_remove {
                self.players.lock().unwrap().remove(id);
            }
        }
        
        player_id_to_remove
    }

    /// Get a player ID from a sender tag
    pub fn get_player_id(&self, tag: &AnonymousSenderTag) -> Option<String> {
        let connections = self.connections.lock().unwrap();
        for (id, conn_tag) in connections.iter() {
            if conn_tag.to_string() == tag.to_string() {
                return Some(id.clone());
            }
        }
        None
    }

    /// Get a clone of all players
    pub fn get_players(&self) -> HashMap<String, Player> {
        self.players.lock().unwrap().clone()
    }

    /// Get a specific player by their internal ID
    pub fn get_player(&self, player_id: &str) -> Option<Player> {
        self.players.lock().unwrap().get(player_id).cloned()
    }
    
    /// Find a player's internal ID from their display ID
    pub fn get_player_id_by_display_id(&self, display_id: &str) -> Option<String> {
        let players = self.players.lock().unwrap();
        
        for (id, player) in players.iter() {
            if player.display_id == display_id {
                return Some(id.clone());
            }
        }
        
        None
    }

    /// Update a player's position
    pub fn update_player_position(&self, player_id: &str, new_position: Position) -> bool {
        let position_tolerance: f32 = 2.0; // Minimum distance between players
        
        // Check if the new position would collide with any other player
        let position_is_available = {
            let state = self.players.lock().unwrap();
            state.iter().all(|(id, other_player)| {
                // Skip checking against ourselves
                if id == player_id {
                    return true;
                }
                
                let dx = other_player.position.x - new_position.x;
                let dy = other_player.position.y - new_position.y;
                let distance_squared = dx * dx + dy * dy;
                
                // Consider the position available if squared distance > tolerance squared
                distance_squared > position_tolerance * position_tolerance
            })
        };
        
        // Only update if position is available
        if position_is_available {
            let mut state = self.players.lock().unwrap();
            if let Some(player) = state.get_mut(player_id) {
                player.position = new_position;
                return true;
            }
        }
        
        false
    }
    
    /// Apply damage to a player
    pub fn apply_damage(&self, target_id: &str, damage: u32) -> bool {
        let mut target_defeated = false;
        
        let mut state = self.players.lock().unwrap();
        if let Some(target) = state.get_mut(target_id) {
            if target.health <= damage {
                target.health = 0;
                target_defeated = true;
                
                // Reset the defeated player
                let mut rng = thread_rng();
                target.position.x = rng.gen_range(-100.0..100.0);
                target.position.y = rng.gen_range(-100.0..100.0);
                target.health = 100;
            } else {
                target.health -= damage;
            }
        }
        
        target_defeated
    }

    /// Update a player's last attack time
    pub fn update_attack_time(&self, player_id: &str, time: u64) {
        let mut state = self.players.lock().unwrap();
        if let Some(player) = state.get_mut(player_id) {
            player.last_attack_time = time;
        }
    }

    /// Check if a player can attack (not on cooldown)
    pub fn can_attack(&self, player_id: &str, current_time: u64, cooldown: u64) -> bool {
        let state = self.players.lock().unwrap();
        if let Some(player) = state.get(player_id) {
            return current_time - player.last_attack_time >= cooldown;
        }
        false
    }

    /// Get all active connections
    pub fn get_connections(&self) -> Vec<PlayerTag> {
        self.connections.lock().unwrap().clone()
    }

    /// Function to generate a random position that is not already occupied by another player
    fn generate_available_position(&self, players: &HashMap<String, Player>) -> Position {
        let mut rng = thread_rng();
        let position_tolerance: f32 = 2.0; // Minimum distance between players
        
        // Maximum number of attempts to find a free position
        const MAX_ATTEMPTS: usize = 100;
        
        for _ in 0..MAX_ATTEMPTS {
            // Generate a random position
            let position = Position {
                x: rng.gen_range(-100.0..100.0),
                y: rng.gen_range(-100.0..100.0),
            };
            
            // Check if this position is far enough from all other players
            let position_is_available = players.values().all(|player| {
                let dx = player.position.x - position.x;
                let dy = player.position.y - position.y;
                let distance_squared = dx * dx + dy * dy;
                
                // Consider position available if squared distance > tolerance squared
                distance_squared > position_tolerance * position_tolerance
            });
            
            if position_is_available {
                return position;
            }
        }
        
        // If we can't find an available position after max attempts, just return a random one
        // This is a fallback that should rarely be needed
        Position {
            x: rng.gen_range(-100.0..100.0),
            y: rng.gen_range(-100.0..100.0),
        }
    }
}
