use rand::{thread_rng, Rng};
use std::collections::HashMap;
use std::sync::{Mutex, RwLock};
use std::time::{SystemTime, UNIX_EPOCH};
use tracing::{debug, error, info, warn};
use uuid::Uuid;

use crate::config::GameConfig;
use crate::game_protocol::{Player, Position};
use crate::world_lore::Faction;
use nym_sdk::mixnet::AnonymousSenderTag;

/// Type alias for a player ID and its associated sender tag
pub type PlayerTag = (String, AnonymousSenderTag);

/// GameState manages the entire game state including players and connections
pub struct GameState {
    /// Map of player IDs to Player objects
    players: RwLock<HashMap<String, Player>>,
    /// List of active connections (player_id, sender_tag)
    connections: Mutex<Vec<PlayerTag>>,
    /// Map of player IDs to their last heartbeat timestamp
    last_heartbeat: Mutex<HashMap<String, u64>>,
    /// Reverse lookup: display_id (lowercase) -> player_id for O(1) lookups
    display_id_to_player_id: RwLock<HashMap<String, String>>,
    /// Game configuration
    config: GameConfig,
}

impl GameState {
    /// Create a new empty GameState with default configuration
    #[allow(dead_code)]
    pub fn new() -> Self {
        let config = GameConfig::default();
        GameState {
            players: RwLock::new(HashMap::with_capacity(256)), // Increased capacity
            connections: Mutex::new(Vec::with_capacity(256)),
            last_heartbeat: Mutex::new(HashMap::with_capacity(256)),
            display_id_to_player_id: RwLock::new(HashMap::with_capacity(256)),
            config,
        }
    }

    /// Create a new GameState with specific configuration
    pub fn new_with_config(config: GameConfig) -> Self {
        GameState {
            players: RwLock::new(HashMap::with_capacity(256)), // Increased capacity
            connections: Mutex::new(Vec::with_capacity(256)),
            last_heartbeat: Mutex::new(HashMap::with_capacity(256)),
            display_id_to_player_id: RwLock::new(HashMap::with_capacity(256)),
            config,
        }
    }

    /// Get a reference to the game configuration
    pub fn get_config(&self) -> &GameConfig {
        &self.config
    }

    /// Add a new player to the game
    pub fn add_player(
        &self,
        name: String,
        faction: Faction,
        sender_tag: AnonymousSenderTag,
    ) -> String {
        // Validate player name length according to configuration
        if name.len() > self.config.max_player_name_length {
            warn!(
                "Player name too long: {} characters (max: {})",
                name.len(),
                self.config.max_player_name_length
            );
            // Truncate the name to fit within limits
        }

        // Check player count limit
        let current_player_count = match self.players.read() {
            Ok(players) => players.len(),
            Err(_) => {
                error!("Failed to access players for count check");
                return String::new();
            }
        };

        if current_player_count >= self.config.max_players {
            warn!(
                "Maximum player limit reached: {}/{}",
                current_player_count, self.config.max_players
            );
            return String::new();
        }

        // Generate a unique ID for the player
        let player_id = Uuid::new_v4().to_string();

        // Generate a position that's not occupied by another player
        let available_position = {
            match self.players.read() {
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
        let _display_id = {
            let mut rng = thread_rng();
            // Generate a random prefix from a set of common words
            let prefixes = [
                "Hero", "Warrior", "Knight", "Scout", "Ranger", "Mage", "Nomad", "Shadow",
            ];
            let prefix = prefixes[rng.gen_range(0..prefixes.len())];

            // Add a 3-digit random number
            let suffix = rng.gen_range(100..999);

            format!("{}{}", prefix, suffix)
        };

        // Ensure the display ID is unique
        let unique_display_id = {
            match self.players.read() {
                Ok(state) => {
                    let mut current_id = format!("Player{}", state.len() + 1);
                    let mut attempts = 0;

                    // Ensure the display ID is unique
                    while state.values().any(|p| p.display_id == current_id) && attempts < 100 {
                        attempts += 1;
                        current_id = format!("Player{}", state.len() + attempts);
                    }

                    current_id
                }
                Err(e) => {
                    warn!("Failed to access players for display ID generation: {}", e);
                    // Fallback to a UUID-based display ID if mutex is poisoned
                    format!(
                        "Player_{}",
                        Uuid::new_v4().simple().to_string()[..8].to_uppercase()
                    )
                }
            }
        };

        // Create a new player with the available position
        let player = Player {
            id: player_id.clone(),
            display_id: unique_display_id,
            name: name
                .chars()
                .take(self.config.max_player_name_length)
                .collect(), // Ensure name is within limits
            position: available_position,
            health: self.config.initial_player_health, // New players at level 1 don't get bonus health
            last_attack_time: now.saturating_sub(self.config.attack_cooldown_seconds), // Allow immediate first attack
            experience: 0, // New players start with 0 experience
            level: 1,      // New players start at level 1
            faction,       // Store the player's chosen faction
        };

        // Add the player to the game state
        let display_id_for_lookup = player.display_id.to_lowercase();
        match self.players.write() {
            Ok(mut players) => {
                players.insert(player_id.clone(), player);
            }
            Err(e) => {
                error!("Failed to add player to game state: {}", e);
                return player_id; // Return the ID even if we couldn't add the player
            }
        }

        // Add reverse lookup mapping for O(1) display_id -> player_id lookups
        match self.display_id_to_player_id.write() {
            Ok(mut reverse_lookup) => {
                reverse_lookup.insert(display_id_for_lookup, player_id.clone());
            }
            Err(e) => {
                error!("Failed to add reverse lookup mapping: {}", e);
            }
        }

        // Store this active connection
        match self.connections.lock() {
            Ok(mut connections) => {
                connections.push((player_id.clone(), sender_tag));
            }
            Err(e) => {
                error!("Failed to store connection: {}", e);
            }
        }

        // Initialize last heartbeat timestamp
        match self.last_heartbeat.lock() {
            Ok(mut last_heartbeats) => {
                last_heartbeats.insert(player_id.clone(), now);
            }
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
                }
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
                }
                Err(e) => {
                    error!("Failed to remove connection: {}", e);
                }
            }

            // Remove from game state and get display_id for reverse lookup cleanup
            let display_id_for_cleanup = if let Some(id) = &player_id_to_remove {
                let display_id = match self.players.read() {
                    Ok(players) => players.get(id).map(|p| p.display_id.to_lowercase()),
                    Err(e) => {
                        error!("Failed to read player for display_id cleanup: {}", e);
                        None
                    }
                };

                match self.players.write() {
                    Ok(mut players) => {
                        players.remove(id);
                    }
                    Err(e) => {
                        error!("Failed to remove player from game state: {}", e);
                    }
                }

                display_id
            } else {
                None
            };

            // Remove from reverse lookup HashMap
            if let Some(display_id) = display_id_for_cleanup {
                match self.display_id_to_player_id.write() {
                    Ok(mut reverse_lookup) => {
                        reverse_lookup.remove(&display_id);
                    }
                    Err(e) => {
                        error!("Failed to remove from reverse lookup: {}", e);
                    }
                }
            }

            // Remove from last heartbeat timestamps
            if let Some(id) = &player_id_to_remove {
                match self.last_heartbeat.lock() {
                    Ok(mut last_heartbeats) => {
                        last_heartbeats.remove(id);
                    }
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
        // Clone connections to reduce lock scope
        let connections_clone = match self.connections.lock() {
            Ok(connections) => connections.clone(),
            Err(e) => {
                warn!(
                    "Failed to access connections for player ID retrieval: {}",
                    e
                );
                return None;
            }
        };

        // Search through connections without holding the lock
        let tag_string = tag.to_string();
        for (id, conn_tag) in connections_clone.iter() {
            if conn_tag.to_string() == tag_string {
                return Some(id.clone());
            }
        }
        None
    }

    /// Get a clone of all players
    pub fn get_players(&self) -> HashMap<String, Player> {
        match self.players.read() {
            Ok(players) => players.clone(),
            Err(e) => {
                warn!("Failed to access players for retrieval: {}", e);
                HashMap::new()
            }
        }
    }

    /// Get a specific player by their internal ID
    pub fn get_player(&self, player_id: &str) -> Option<Player> {
        match self.players.read() {
            Ok(players) => players.get(player_id).cloned(),
            Err(e) => {
                warn!("Failed to access players for retrieval: {}", e);
                None
            }
        }
    }

    /// Get all players except the one with the specified ID
    /// Used for collision detection and other player-to-player interactions
    pub fn get_all_players_except(&self, player_id: &str) -> Vec<Player> {
        match self.players.read() {
            Ok(players) => players
                .iter()
                .filter(|(id, _)| *id != player_id)
                .map(|(_, player)| player.clone())
                .collect(),
            Err(e) => {
                error!("Failed to get players except {}: {}", player_id, e);
                Vec::new()
            }
        }
    }

    /// Find a player's internal ID from their display ID
    /// This lookup is case-insensitive to improve user experience
    /// Optimized O(1) lookup for player ID by display ID (case insensitive)
    pub fn get_player_id_by_display_id(&self, display_id: &str) -> Option<String> {
        let lowercase_target = display_id.to_lowercase();
        match self.display_id_to_player_id.read() {
            Ok(reverse_lookup) => reverse_lookup.get(&lowercase_target).cloned(),
            Err(e) => {
                warn!("Failed to access reverse lookup for display ID: {}", e);
                // Fallback to linear search if reverse lookup is poisoned
                match self.players.read() {
                    Ok(players) => {
                        for (id, player) in players.iter() {
                            if player.display_id.to_lowercase() == lowercase_target {
                                return Some(id.clone());
                            }
                        }
                        None
                    }
                    Err(_) => None,
                }
            }
        }
    }

    /// Update a player's position
    pub fn update_player_position(&self, player_id: &str, new_position: Position) -> bool {
        // Validate position using configuration boundaries
        let (clamped_x, clamped_y) = self.config.clamp_position(new_position.x, new_position.y);
        let clamped_position = Position::new(clamped_x, clamped_y);

        match self.players.write() {
            Ok(mut players) => {
                if let Some(player) = players.get_mut(player_id) {
                    player.position = clamped_position;
                    debug!(
                        "Updated position for player {} to ({:.1}, {:.1})",
                        player_id, clamped_position.x, clamped_position.y
                    );
                    true
                } else {
                    debug!("Player {} not found for position update", player_id);
                    false
                }
            }
            Err(e) => {
                error!("Failed to update player position: {}", e);
                false
            }
        }
    }

    /// Apply damage to a player and return true if they were defeated
    /// Also awards experience points to the attacker
    pub fn apply_damage(&self, target_id: &str, attacker_id: &str, damage: u32) -> bool {
        // Get attacker level and calculate damage bonus
        let attacker_level = match self.players.read() {
            Ok(players) => {
                if let Some(attacker) = players.get(attacker_id) {
                    attacker.level
                } else {
                    1 // Default to level 1 if attacker not found
                }
            }
            Err(e) => {
                error!("Failed to get attacker level: {}", e);
                1 // Default to level 1 on error
            }
        };

        // Calculate damage with level-based bonus
        let (damage_bonus, _) = Self::calculate_level_bonuses(attacker_level);
        let modified_damage = damage + damage_bonus;
        let actual_damage = modified_damage.min(self.config.attack_damage + damage_bonus); // Limit damage to configured max + bonus

        // First, determine if we need a new position by checking if player will be defeated
        let needs_respawn = match self.players.read() {
            Ok(players) => {
                if let Some(player) = players.get(target_id) {
                    player.health <= actual_damage
                } else {
                    return false; // Player doesn't exist
                }
            }
            Err(e) => {
                error!("Failed to check player health: {}", e);
                return false;
            }
        };

        // Generate new position if needed (while no locks are held)
        let new_position = if needs_respawn {
            match self.players.read() {
                Ok(players) => Some(self.generate_available_position(&players)),
                Err(e) => {
                    error!("Failed to generate respawn position: {}", e);
                    return false;
                }
            }
        } else {
            None
        };

        // Now apply the damage and update position if needed
        // First apply damage and get result
        let was_defeated = match self.players.write() {
            Ok(mut players) => {
                if let Some(player) = players.get_mut(target_id) {
                    if let Some(respawn_pos) = new_position {
                        // Player defeated - reset position and health (with level bonuses)
                        player.position = respawn_pos;
                        let (_, health_bonus) = Self::calculate_level_bonuses(player.level);
                        player.health = self.config.initial_player_health + health_bonus;
                        info!(
                            "Player {} was defeated and respawned at {:?}",
                            target_id, player.position
                        );
                        true
                    } else {
                        player.health -= actual_damage;
                        info!(
                            "Player {} took {} damage, health now: {}",
                            target_id, actual_damage, player.health
                        );
                        false
                    }
                } else {
                    warn!("Attempted to damage non-existent player: {}", target_id);
                    return false;
                }
            }
            Err(e) => {
                error!("Failed to apply damage: {}", e);
                return false;
            }
        };

        // Award XP to the attacker
        if attacker_id != target_id {
            // No XP for self-damage
            match self.players.write() {
                Ok(mut players) => {
                    if let Some(attacker) = players.get_mut(attacker_id) {
                        // Award more XP for defeating a player
                        let xp_gained = if was_defeated {
                            // Bonus XP for defeating a player
                            20 + actual_damage
                        } else {
                            // Base XP for dealing damage
                            actual_damage
                        };

                        attacker.experience += xp_gained;

                        // Check for level up - simple level formula: each level needs level*100 XP
                        let next_level = attacker.level + 1;
                        let xp_needed_for_level = (next_level as u32) * 100;

                        if attacker.experience >= xp_needed_for_level {
                            attacker.level = next_level;
                            info!("Player {} reached level {}!", attacker_id, next_level);
                        }

                        info!(
                            "Player {} gained {} XP for attacking {}",
                            attacker_id, xp_gained, target_id
                        );
                    }
                }
                Err(e) => {
                    error!("Failed to award XP: {}", e);
                }
            }
        }

        was_defeated
    }

    /// Update a player's last attack time
    pub fn update_attack_time(&self, player_id: &str, time: u64) {
        match self.players.write() {
            Ok(mut state) => {
                if let Some(player) = state.get_mut(player_id) {
                    player.last_attack_time = time;
                }
            }
            Err(e) => {
                warn!("Failed to access players for attack time update: {}", e);
            }
        }
    }

    /// Check if a player can attack (not on cooldown)
    ///
    /// Combat system uses a cooldown period from configuration between attacks.
    /// Returns true if enough time has passed since the player's last attack.
    pub fn can_attack(&self, player_id: &str, current_time: u64) -> bool {
        match self.players.read() {
            Ok(players) => {
                if let Some(player) = players.get(player_id) {
                    let time_since_last_attack =
                        current_time.saturating_sub(player.last_attack_time);
                    time_since_last_attack >= self.config.attack_cooldown_seconds
                } else {
                    false
                }
            }
            Err(e) => {
                error!("Failed to check attack cooldown: {}", e);
                false
            }
        }
    }

    /// Get all sender tags for connected players
    pub fn get_player_tags(&self) -> Vec<AnonymousSenderTag> {
        match self.connections.lock() {
            Ok(connections) => {
                // Extract the sender tags from the (player_id, sender_tag) tuples
                connections.iter().map(|(_, tag)| *tag).collect()
            }
            Err(e) => {
                error!("Failed to get player tags: {}", e);
                Vec::new()
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
            }
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
            Ok(last_heartbeats) => last_heartbeats
                .iter()
                .filter_map(|(player_id, &last_heartbeat_time)| {
                    if now.saturating_sub(last_heartbeat_time)
                        > self.config.heartbeat_timeout_seconds
                    {
                        Some(player_id.clone())
                    } else {
                        None
                    }
                })
                .collect(),
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

    /// Restore a player from persistence (used during server recovery)
    /// This method restores player data without creating a network connection
    /// Players will need to reconnect to establish their mixnet connection
    pub fn restore_player(&self, player_id: String, player: Player) {
        match self.players.write() {
            Ok(mut players) => {
                players.insert(player_id.clone(), player.clone());
                info!(
                    "Restored player {} ({}) at position ({:.1}, {:.1}) with {} health",
                    player_id, player.name, player.position.x, player.position.y, player.health
                );
            }
            Err(e) => {
                error!("Failed to restore player {}: {}", player_id, e);
            }
        }

        // Note: We intentionally do not add to connections list here
        // Players must reconnect through the mixnet to establish their connection
        // This preserves the privacy and security properties of the system
    }

    /// Helper method to get sender tag by player ID
    fn get_sender_tag_by_player_id(&self, player_id: &str) -> Option<AnonymousSenderTag> {
        match self.connections.lock() {
            Ok(connections) => connections
                .iter()
                .find(|(id, _)| id == player_id)
                .map(|(_, tag)| *tag),
            Err(e) => {
                error!("Failed to access connections to find sender tag: {}", e);
                None
            }
        }
    }

    /// Get connection tag (sender tag) for a player by their player ID
    /// This is used for direct communication, such as sending whisper messages
    pub fn get_connection_tag(&self, player_id: &str) -> Option<AnonymousSenderTag> {
        // This is essentially the same as get_sender_tag_by_player_id but with a public interface
        self.get_sender_tag_by_player_id(player_id)
    }

    /// Calculate level-based stat bonuses for a player
    /// Returns a tuple of (damage_bonus, health_bonus)
    pub fn calculate_level_bonuses(level: u8) -> (u32, u32) {
        // Small bonuses that won't unbalance the game
        // Each level provides +2 damage and +5 health
        let level_above_base = level.saturating_sub(1) as u32; // Level 1 is base level
        let damage_bonus = level_above_base * 2;
        let health_bonus = level_above_base * 5;

        (damage_bonus, health_bonus)
    }

    /// Function to generate a random position that is not already occupied by another player
    fn generate_available_position(&self, players: &HashMap<String, Player>) -> Position {
        let mut rng = thread_rng();
        let position_tolerance: f32 = 2.0; // Minimum distance between players

        // Maximum number of attempts to find a free position
        const MAX_ATTEMPTS: usize = 100;

        for _ in 0..MAX_ATTEMPTS {
            // Generate a random position within configured world boundaries
            let position = Position {
                x: rng.gen_range(self.config.world_min_x..self.config.world_max_x),
                y: rng.gen_range(self.config.world_min_y..self.config.world_max_y),
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
            x: rng.gen_range(self.config.world_min_x..self.config.world_max_x),
            y: rng.gen_range(self.config.world_min_y..self.config.world_max_y),
        }
    }
}
