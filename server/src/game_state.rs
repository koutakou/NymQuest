use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use uuid::Uuid;
use std::time::{SystemTime, UNIX_EPOCH};
use rand::{thread_rng, Rng};
use tracing::{warn, error, debug, info};

use nym_sdk::mixnet::AnonymousSenderTag;
use crate::game_protocol::{Player, Position};

/// Type alias for a player ID and its associated sender tag
pub type PlayerTag = (String, AnonymousSenderTag);

/// Configuration constants for heartbeat mechanism
pub const HEARTBEAT_TIMEOUT_SECONDS: u64 = 30; // Remove players after 30 seconds of inactivity
pub const HEARTBEAT_INTERVAL_SECONDS: u64 = 10; // Send heartbeat requests every 10 seconds

/// GameState manages the entire game state including players and connections
pub struct GameState {
    /// Map of player IDs to Player objects
    players: Mutex<HashMap<String, Player>>,
    /// List of active connections (player_id, sender_tag)
    connections: Mutex<Vec<PlayerTag>>,
    /// Map of player IDs to their last heartbeat timestamp
    last_heartbeat: Mutex<HashMap<String, u64>>,
}

impl GameState {
    /// Create a new empty GameState
    pub fn new() -> Self {
        GameState {
            players: Mutex::new(HashMap::new()),
            connections: Mutex::new(Vec::new()),
            last_heartbeat: Mutex::new(HashMap::new()),
        }
    }

    /// Add a new player to the game
    pub fn add_player(&self, name: String, sender_tag: AnonymousSenderTag) -> String {
        // Generate a unique ID for the player
        let player_id = Uuid::new_v4().to_string();
        
        // Generate a position that's not occupied by another player
        let available_position = {
            match self.players.lock() {
                Ok(state) => self.generate_available_position(&state),
                Err(e) => {
                    warn!("Failed to access players for position generation: {}", e);
                    // Return a default position if mutex is poisoned
                    Position::new(0.0, 0.0)
                }
            }
        };
        
        // Get current time for initializing attack cooldown
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        
        // Generate a privacy-preserving display ID using a more anonymous scheme
        // Use a random alphanumeric code to improve privacy protection
        let display_id = {
            let mut rng = thread_rng();
            // Generate a random prefix from a set of common words
            let prefixes = ["Hero", "Warrior", "Knight", "Scout", "Ranger", "Mage", "Nomad", "Shadow"];
            let prefix = prefixes[rng.gen_range(0..prefixes.len())];
            
            // Add a 3-digit random number
            let suffix = rng.gen_range(100..999);
            
            format!("{}{}", prefix, suffix)
        };
        
        // Ensure the display ID is unique
        let unique_display_id = {
            match self.players.lock() {
                Ok(mut state) => {
                    let mut current_id = format!("Player{}", state.len() + 1);
                    let mut attempts = 0;
                    
                    // Ensure the display ID is unique
                    while state.values().any(|p| p.display_id == current_id) && attempts < 100 {
                        attempts += 1;
                        current_id = format!("Player{}", state.len() + attempts);
                    }
                    
                    current_id
                },
                Err(e) => {
                    warn!("Failed to access players for display ID generation: {}", e);
                    // Fallback to a UUID-based display ID if mutex is poisoned
                    format!("Player_{}", Uuid::new_v4().simple().to_string()[..8].to_uppercase())
                }
            }
        };
            
        // Create a new player with the available position
        let player = Player {
            id: player_id.clone(),
            display_id: unique_display_id,
            name,
            position: available_position,
            health: 100,
            last_attack_time: now,
        };
        
        // Add the player to the game state
        match self.players.lock() {
            Ok(mut players) => {
                players.insert(player_id.clone(), player);
            },
            Err(e) => {
                error!("Failed to add player to game state: {}", e);
                return player_id; // Return the ID even if we couldn't add the player
            }
        }
        
        // Store this active connection
        match self.connections.lock() {
            Ok(mut connections) => {
                connections.push((player_id.clone(), sender_tag));
            },
            Err(e) => {
                error!("Failed to store connection: {}", e);
            }
        }
        
        // Initialize last heartbeat timestamp
        match self.last_heartbeat.lock() {
            Ok(mut last_heartbeats) => {
                last_heartbeats.insert(player_id.clone(), now);
            },
            Err(e) => {
                error!("Failed to initialize last heartbeat timestamp: {}", e);
            }
        }
        
        player_id
    }

    /// Remove a player from the game
    pub fn remove_player(&self, tag: &AnonymousSenderTag) -> Option<String> {
        let mut connection_index = None;
        let mut player_id_to_remove = None;
        
        // Find the player's ID and index in the connections list
        {
            match self.connections.lock() {
                Ok(connections) => {
                    for (i, (id, conn_tag)) in connections.iter().enumerate() {
                        if conn_tag.to_string() == tag.to_string() {
                            player_id_to_remove = Some(id.clone());
                            connection_index = Some(i);
                            break;
                        }
                    }
                },
                Err(e) => {
                    warn!("Failed to access connections for player removal: {}", e);
                }
            }
        }
        
        // Remove the player if found
        if let Some(index) = connection_index {
            // Remove from active connections
            match self.connections.lock() {
                Ok(mut connections) => {
                    connections.remove(index);
                },
                Err(e) => {
                    error!("Failed to remove connection: {}", e);
                }
            }
            
            // Remove from game state
            if let Some(id) = &player_id_to_remove {
                match self.players.lock() {
                    Ok(mut players) => {
                        players.remove(id);
                    },
                    Err(e) => {
                        error!("Failed to remove player from game state: {}", e);
                    }
                }
            }
            
            // Remove from last heartbeat timestamps
            if let Some(id) = &player_id_to_remove {
                match self.last_heartbeat.lock() {
                    Ok(mut last_heartbeats) => {
                        last_heartbeats.remove(id);
                    },
                    Err(e) => {
                        error!("Failed to remove last heartbeat timestamp: {}", e);
                    }
                }
            }
        }
        
        player_id_to_remove
    }

    /// Get a player ID from a sender tag
    pub fn get_player_id(&self, tag: &AnonymousSenderTag) -> Option<String> {
        match self.connections.lock() {
            Ok(connections) => {
                for (id, conn_tag) in connections.iter() {
                    if conn_tag.to_string() == tag.to_string() {
                        return Some(id.clone());
                    }
                }
                None
            },
            Err(e) => {
                warn!("Failed to access connections for player ID retrieval: {}", e);
                None
            }
        }
    }

    /// Get a clone of all players
    pub fn get_players(&self) -> HashMap<String, Player> {
        match self.players.lock() {
            Ok(players) => players.clone(),
            Err(e) => {
                warn!("Failed to access players for retrieval: {}", e);
                HashMap::new()
            }
        }
    }

    /// Get a specific player by their internal ID
    pub fn get_player(&self, player_id: &str) -> Option<Player> {
        match self.players.lock() {
            Ok(players) => players.get(player_id).cloned(),
            Err(e) => {
                warn!("Failed to access players for retrieval: {}", e);
                None
            }
        }
    }
    
    /// Find a player's internal ID from their display ID
    pub fn get_player_id_by_display_id(&self, display_id: &str) -> Option<String> {
        match self.players.lock() {
            Ok(players) => {
                for (id, player) in players.iter() {
                    if player.display_id == display_id {
                        return Some(id.clone());
                    }
                }
                None
            },
            Err(e) => {
                warn!("Failed to access players for display ID lookup: {}", e);
                None
            }
        }
    }

    /// Update a player's position
    pub fn update_player_position(&self, player_id: &str, new_position: Position) -> bool {
        let position_tolerance: f32 = 2.0; // Minimum distance between players
        
        // Check if the new position would collide with any other player
        match self.players.lock() {
            Ok(state) => {
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
            },
            Err(e) => {
                warn!("Failed to access players for position update: {}", e);
                false
            }
        }
    }
    
    /// Apply damage to a player
    /// 
    /// If health drops to zero, the player is reset with full health (100 HP) at a random position.
    /// Returns true if the player was defeated (health reached zero).
    pub fn apply_damage(&self, target_id: &str, damage: u32) -> bool {
        let mut target_defeated = false;
        
        match self.players.lock() {
            Ok(mut state) => {
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
            },
            Err(e) => {
                warn!("Failed to access players for damage application: {}", e);
            }
        }
        
        target_defeated
    }

    /// Update a player's last attack time
    pub fn update_attack_time(&self, player_id: &str, time: u64) {
        match self.players.lock() {
            Ok(mut state) => {
                if let Some(player) = state.get_mut(player_id) {
                    player.last_attack_time = time;
                }
            },
            Err(e) => {
                warn!("Failed to access players for attack time update: {}", e);
            }
        }
    }

    /// Check if a player can attack (not on cooldown)
    /// 
    /// Combat system uses a cooldown period (typically 3 seconds) between attacks.
    /// Returns true if enough time has passed since the player's last attack.
    pub fn can_attack(&self, player_id: &str, current_time: u64, cooldown: u64) -> bool {
        match self.players.lock() {
            Ok(state) => {
                if let Some(player) = state.get(player_id) {
                    return current_time - player.last_attack_time >= cooldown;
                }
                false
            },
            Err(e) => {
                warn!("Failed to access players for attack cooldown check: {}", e);
                false
            }
        }
    }

    /// Get all active connections
    pub fn get_connections(&self) -> Vec<PlayerTag> {
        match self.connections.lock() {
            Ok(connections) => connections.clone(),
            Err(e) => {
                warn!("Failed to access connections for retrieval: {}", e);
                Vec::new()
            }
        }
    }

    /// Update the last heartbeat timestamp for a player
    pub fn update_heartbeat(&self, player_id: &str) {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
            
        match self.last_heartbeat.lock() {
            Ok(mut last_heartbeats) => {
                last_heartbeats.insert(player_id.to_string(), now);
                debug!("Updated heartbeat for player {}", player_id);
            },
            Err(e) => {
                error!("Failed to update heartbeat for player {}: {}", player_id, e);
            }
        }
    }

    /// Get inactive players that haven't sent a heartbeat within the timeout period
    /// Returns a list of player IDs that should be removed
    pub fn get_inactive_players(&self) -> Vec<String> {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
            
        match self.last_heartbeat.lock() {
            Ok(last_heartbeats) => {
                last_heartbeats.iter()
                    .filter_map(|(player_id, &last_heartbeat_time)| {
                        if now.saturating_sub(last_heartbeat_time) > HEARTBEAT_TIMEOUT_SECONDS {
                            Some(player_id.clone())
                        } else {
                            None
                        }
                    })
                    .collect()
            },
            Err(e) => {
                error!("Failed to access last heartbeat timestamps: {}", e);
                Vec::new()
            }
        }
    }

    /// Remove multiple players by their IDs (used for cleanup of inactive players)
    pub fn remove_players_by_ids(&self, player_ids: &[String]) -> Vec<String> {
        let mut removed_players = Vec::new();
        
        for player_id in player_ids {
            // First find the sender tag for this player
            if let Some(sender_tag) = self.get_sender_tag_by_player_id(player_id) {
                if let Some(removed_id) = self.remove_player(&sender_tag) {
                    removed_players.push(removed_id);
                    info!("Removed inactive player: {}", player_id);
                }
            }
        }
        
        removed_players
    }

    /// Helper method to get sender tag by player ID
    fn get_sender_tag_by_player_id(&self, player_id: &str) -> Option<AnonymousSenderTag> {
        match self.connections.lock() {
            Ok(connections) => {
                connections.iter()
                    .find(|(id, _)| id == player_id)
                    .map(|(_, tag)| tag.clone())
            },
            Err(e) => {
                error!("Failed to access connections to find sender tag: {}", e);
                None
            }
        }
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
