use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};
use tokio::fs;
use tracing::{info, warn};
use uuid::Uuid;

use crate::config::GameConfig;
use crate::game_protocol::{Player, Position};

/// Persistable game state structure that excludes sensitive runtime data
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PersistedGameState {
    /// Map of player IDs to their persistent game data
    pub players: HashMap<String, PersistedPlayer>,
    /// Timestamp when this state was last saved
    pub last_saved: u64,
    /// Game configuration used when this state was saved
    pub config_snapshot: GameConfigSnapshot,
    /// Server session identifier to detect state from different server instances
    pub session_id: String,
}

/// Player data that can be safely persisted (excludes network-sensitive data)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PersistedPlayer {
    pub id: String,
    pub display_id: String,
    pub name: String,
    pub position: Position,
    pub health: u32,
    pub last_attack_time: u64,
    /// Experience points earned through gameplay
    pub experience: u32,
    /// Player level based on experience
    pub level: u8,
    /// Timestamp when player was last active (for cleanup purposes)
    pub last_active: u64,
}

/// Snapshot of relevant game configuration for validation during recovery
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GameConfigSnapshot {
    pub world_max_x: f32,
    pub world_min_x: f32,
    pub world_max_y: f32,
    pub world_min_y: f32,
    pub initial_player_health: u32,
    pub max_players: usize,
}

impl From<&GameConfig> for GameConfigSnapshot {
    fn from(config: &GameConfig) -> Self {
        Self {
            world_max_x: config.world_max_x,
            world_min_x: config.world_min_x,
            world_max_y: config.world_max_y,
            world_min_y: config.world_min_y,
            initial_player_health: config.initial_player_health,
            max_players: config.max_players,
        }
    }
}

/// Game state persistence manager
pub struct GameStatePersistence {
    /// Path to the persistence directory
    persist_dir: PathBuf,
    /// Current session identifier
    session_id: String,
    /// Whether persistence is enabled
    enabled: bool,
}

impl GameStatePersistence {
    /// Create a new persistence manager
    pub fn new<P: AsRef<Path>>(persist_dir: P, enabled: bool) -> Self {
        let session_id = Uuid::new_v4().to_string();
        Self {
            persist_dir: persist_dir.as_ref().to_path_buf(),
            session_id,
            enabled,
        }
    }

    /// Initialize persistence directory and validate setup
    pub async fn initialize(&self) -> Result<()> {
        if !self.enabled {
            info!("Game state persistence is disabled");
            return Ok(());
        }

        // Create persistence directory if it doesn't exist
        if !self.persist_dir.exists() {
            fs::create_dir_all(&self.persist_dir)
                .await
                .map_err(|e| anyhow!("Failed to create persistence directory: {}", e))?;
            info!(
                "Created persistence directory: {}",
                self.persist_dir.display()
            );
        }

        // Validate directory permissions
        let temp_file = self.persist_dir.join("write_test.tmp");
        match fs::write(&temp_file, b"test").await {
            Ok(_) => {
                if let Err(e) = fs::remove_file(&temp_file).await {
                    warn!("Failed to clean up test file: {}", e);
                }
                info!(
                    "Persistence directory is writable: {}",
                    self.persist_dir.display()
                );
                Ok(())
            }
            Err(e) => Err(anyhow!("Persistence directory is not writable: {}", e)),
        }
    }

    /// Save game state to disk
    pub async fn save_state(
        &self,
        players: &HashMap<String, Player>,
        config: &GameConfig,
    ) -> Result<()> {
        if !self.enabled {
            return Ok(());
        }

        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        // Convert active players to persistable format
        let persisted_players: HashMap<String, PersistedPlayer> = players
            .iter()
            .map(|(id, player)| {
                let persisted = PersistedPlayer {
                    id: id.clone(),
                    display_id: player.display_id.clone(),
                    name: player.name.clone(),
                    position: player.position,
                    health: player.health,
                    last_attack_time: player.last_attack_time,
                    experience: player.experience,
                    level: player.level,
                    last_active: now, // Mark as active during save
                };
                (id.clone(), persisted)
            })
            .collect();

        let state = PersistedGameState {
            players: persisted_players,
            last_saved: now,
            config_snapshot: GameConfigSnapshot::from(config),
            session_id: self.session_id.clone(),
        };

        let serialized = serde_json::to_string_pretty(&state)
            .map_err(|e| anyhow!("Failed to serialize game state: {}", e))?;

        // Write to temporary file first for atomic operation
        let temp_file = self.get_temp_file_path();
        let final_file = self.get_state_file_path();

        fs::write(&temp_file, serialized)
            .await
            .map_err(|e| anyhow!("Failed to write temporary state file: {}", e))?;

        // Atomic rename to final location
        fs::rename(&temp_file, &final_file)
            .await
            .map_err(|e| anyhow!("Failed to finalize state file: {}", e))?;

        info!(
            "Game state saved successfully: {} players, session {}",
            state.players.len(),
            self.session_id
        );

        Ok(())
    }

    /// Load game state from disk
    pub async fn load_state(&self, config: &GameConfig) -> Result<Option<PersistedGameState>> {
        if !self.enabled {
            return Ok(None);
        }

        let state_file = self.get_state_file_path();

        if !state_file.exists() {
            info!("No existing game state found");
            return Ok(None);
        }

        let content = fs::read_to_string(&state_file)
            .await
            .map_err(|e| anyhow!("Failed to read state file: {}", e))?;

        let state: PersistedGameState = serde_json::from_str(&content)
            .map_err(|e| anyhow!("Failed to deserialize game state: {}", e))?;

        // Validate state compatibility with current configuration
        self.validate_state_compatibility(&state, config)?;

        info!(
            "Loaded game state: {} players from session {} (saved {}s ago)",
            state.players.len(),
            state.session_id,
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs()
                .saturating_sub(state.last_saved)
        );

        Ok(Some(state))
    }

    /// Clean up old or stale player data during recovery
    pub fn cleanup_stale_players(
        &self,
        state: &mut PersistedGameState,
        max_offline_duration: u64,
    ) -> usize {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        let initial_count = state.players.len();

        state
            .players
            .retain(|_id, player| now.saturating_sub(player.last_active) <= max_offline_duration);

        let removed_count = initial_count - state.players.len();

        if removed_count > 0 {
            info!("Cleaned up {} stale players during recovery", removed_count);
        }

        removed_count
    }

    #[allow(dead_code)]
    pub fn session_id(&self) -> &str {
        &self.session_id
    }

    #[allow(dead_code)]
    pub fn is_enabled(&self) -> bool {
        self.enabled
    }

    /// Create a backup of the current state file
    pub async fn backup_current_state(&self) -> Result<()> {
        if !self.enabled {
            return Ok(());
        }

        let state_file = self.get_state_file_path();
        if !state_file.exists() {
            return Ok(());
        }

        let backup_file = self.get_backup_file_path();

        fs::copy(&state_file, &backup_file)
            .await
            .map_err(|e| anyhow!("Failed to create backup: {}", e))?;

        info!("Created state backup: {}", backup_file.display());
        Ok(())
    }

    /// Validate that loaded state is compatible with current configuration
    fn validate_state_compatibility(
        &self,
        state: &PersistedGameState,
        current_config: &GameConfig,
    ) -> Result<()> {
        let config_snapshot = &state.config_snapshot;

        // Check if world boundaries are compatible
        if config_snapshot.world_max_x != current_config.world_max_x
            || config_snapshot.world_min_x != current_config.world_min_x
            || config_snapshot.world_max_y != current_config.world_max_y
            || config_snapshot.world_min_y != current_config.world_min_y
        {
            warn!("World boundaries changed since last save - players may need repositioning");
        }

        // Check if player health settings are compatible
        if config_snapshot.initial_player_health != current_config.initial_player_health {
            warn!(
                "Player health configuration changed: saved={}, current={}",
                config_snapshot.initial_player_health, current_config.initial_player_health
            );
        }

        // Validate player positions are still within current world boundaries
        for (player_id, player) in &state.players {
            if !current_config.is_position_valid(player.position.x, player.position.y) {
                warn!(
                    "Player {} position ({}, {}) is outside current world boundaries",
                    player_id, player.position.x, player.position.y
                );
            }
        }

        Ok(())
    }

    /// Get the path for the main state file
    fn get_state_file_path(&self) -> PathBuf {
        self.persist_dir.join("game_state.json")
    }

    /// Get the path for temporary state file (used during saves)
    fn get_temp_file_path(&self) -> PathBuf {
        self.persist_dir.join("game_state.tmp")
    }

    /// Get the path for backup state file
    fn get_backup_file_path(&self) -> PathBuf {
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        self.persist_dir
            .join(format!("game_state_backup_{}.json", timestamp))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game_protocol::Position;
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_persistence_initialization() {
        let temp_dir = TempDir::new().unwrap();
        let persistence = GameStatePersistence::new(temp_dir.path(), true);

        assert!(persistence.initialize().await.is_ok());
        assert!(temp_dir.path().exists());
    }

    #[tokio::test]
    async fn test_save_and_load_state() {
        let temp_dir = TempDir::new().unwrap();
        let persistence = GameStatePersistence::new(temp_dir.path(), true);
        persistence.initialize().await.unwrap();

        let config = GameConfig::default();
        let mut players = HashMap::new();

        // Create a test player
        let player = Player {
            id: "player1".to_string(),
            display_id: "TestPlayer001".to_string(),
            name: "Test Player".to_string(),
            position: Position::new(10.0, 20.0),
            health: 100,
            last_attack_time: 1234567890,
            experience: 50,
            level: 1,
        };
        players.insert("player1".to_string(), player);

        // Save state
        assert!(persistence.save_state(&players, &config).await.is_ok());

        // Load state
        let loaded_state = persistence.load_state(&config).await.unwrap();
        assert!(loaded_state.is_some());

        let state = loaded_state.unwrap();
        assert_eq!(state.players.len(), 1);
        assert!(state.players.contains_key("player1"));

        let loaded_player = &state.players["player1"];
        assert_eq!(loaded_player.name, "Test Player");
        assert_eq!(loaded_player.position.x, 10.0);
        assert_eq!(loaded_player.position.y, 20.0);
    }

    #[tokio::test]
    async fn test_disabled_persistence() {
        let temp_dir = TempDir::new().unwrap();
        let persistence = GameStatePersistence::new(temp_dir.path(), false);

        let config = GameConfig::default();
        let players = HashMap::new();

        // Operations should succeed but do nothing
        assert!(persistence.save_state(&players, &config).await.is_ok());
        let loaded = persistence.load_state(&config).await.unwrap();
        assert!(loaded.is_none());
    }

    #[tokio::test]
    async fn test_stale_player_cleanup() {
        let temp_dir = TempDir::new().unwrap();
        let persistence = GameStatePersistence::new(temp_dir.path(), true);

        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        let mut state = PersistedGameState {
            players: HashMap::new(),
            last_saved: now,
            config_snapshot: GameConfigSnapshot::from(&GameConfig::default()),
            session_id: "test-session".to_string(),
        };

        // Add fresh player
        state.players.insert(
            "fresh".to_string(),
            PersistedPlayer {
                id: "fresh".to_string(),
                display_id: "Fresh001".to_string(),
                name: "Fresh Player".to_string(),
                position: Position::new(0.0, 0.0),
                health: 100,
                last_attack_time: 0,
                experience: 25,
                level: 1,
                last_active: now,
            },
        );

        // Add stale player
        state.players.insert(
            "stale".to_string(),
            PersistedPlayer {
                id: "stale".to_string(),
                display_id: "Stale001".to_string(),
                name: "Stale Player".to_string(),
                position: Position::new(0.0, 0.0),
                health: 100,
                last_attack_time: 0,
                experience: 75,
                level: 2,
                last_active: now - 3600, // 1 hour ago
            },
        );

        let removed = persistence.cleanup_stale_players(&mut state, 1800); // 30 min threshold
        assert_eq!(removed, 1);
        assert!(state.players.contains_key("fresh"));
        assert!(!state.players.contains_key("stale"));
    }
}
