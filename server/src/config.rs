use anyhow::{anyhow, Result};
use std::env;
use std::time::Duration;
use tracing::{info, warn};

/// Game configuration constants with validation and environment override support
///
/// Environment Variables:
/// - NYMQUEST_WORLD_MAX_X: Maximum X coordinate boundary (default: 100.0)
/// - NYMQUEST_WORLD_MIN_X: Minimum X coordinate boundary (default: -100.0)  
/// - NYMQUEST_WORLD_MAX_Y: Maximum Y coordinate boundary (default: 100.0)
/// - NYMQUEST_WORLD_MIN_Y: Minimum Y coordinate boundary (default: -100.0)
/// - NYMQUEST_MOVEMENT_SPEED: Player movement speed multiplier (default: 14.0)
/// - NYMQUEST_PLAYER_COLLISION_RADIUS: Minimum distance between players (default: 7.0)
/// - NYMQUEST_MAX_PLAYER_NAME_LENGTH: Maximum player name length (default: 50)
/// - NYMQUEST_MAX_CHAT_MESSAGE_LENGTH: Maximum chat message length (default: 500)
/// - NYMQUEST_HEARTBEAT_INTERVAL_SECONDS: Heartbeat request interval (default: 30)
/// - NYMQUEST_HEARTBEAT_TIMEOUT_SECONDS: Player inactivity timeout (default: 90)
/// - NYMQUEST_MAX_PLAYERS: Maximum concurrent players (default: 100)
/// - NYMQUEST_ATTACK_COOLDOWN_SECONDS: Attack cooldown period (default: 3)
/// - NYMQUEST_INITIAL_PLAYER_HEALTH: Starting player health (default: 100)
/// - NYMQUEST_ATTACK_DAMAGE: Base attack damage (default: 20)
/// - NYMQUEST_ATTACK_RANGE: Attack range in world units (default: 28.0)
/// - NYMQUEST_BASE_DAMAGE: Base damage amount (default: 10)
/// - NYMQUEST_CRIT_CHANCE: Critical hit chance (0.0 to 1.0) (default: 0.15)
/// - NYMQUEST_CRIT_MULTIPLIER: Critical hit damage multiplier (default: 2.0)
/// - NYMQUEST_ENABLE_PERSISTENCE: Enable game state persistence (default: true)
/// - NYMQUEST_PERSISTENCE_DIR: Directory for saving game state (default: "./game_data")
/// - NYMQUEST_MESSAGE_RATE_LIMIT: Messages per second limit per connection (default: 10)
/// - NYMQUEST_MESSAGE_BURST_SIZE: Maximum burst size for rate limiting (default: 20)
/// - NYMQUEST_MESSAGE_PROCESSING_INTERVAL_MS: Minimum interval between processing messages in milliseconds (default: 100)
/// - NYMQUEST_ENABLE_MESSAGE_PROCESSING_PACING: Enable message processing pacing for enhanced privacy (default: false)
/// - NYMQUEST_STATE_BROADCAST_INTERVAL_SECONDS: Interval for broadcasting game state (default: 5)
/// - NYMQUEST_INACTIVE_PLAYER_CLEANUP_INTERVAL_SECONDS: Interval for cleaning up inactive players (default: 45)
/// - NYMQUEST_REPLAY_PROTECTION_WINDOW_SIZE: Number of sequence numbers to track for replay prevention (default: 64)
/// - NYMQUEST_REPLAY_PROTECTION_ADAPTIVE: Enable adaptive replay protection window sizing (default: true)
/// - NYMQUEST_REPLAY_PROTECTION_MIN_WINDOW: Minimum window size for adaptive replay protection (default: 32)
/// - NYMQUEST_REPLAY_PROTECTION_MAX_WINDOW: Maximum window size for adaptive replay protection (default: 96)
/// - NYMQUEST_REPLAY_PROTECTION_ADJUSTMENT_COOLDOWN: Cooldown period in seconds between window size adjustments (default: 60)
/// - NYMQUEST_MESSAGE_PROCESSING_JITTER_PERCENT: Jitter percentage to apply to message processing pacing (0-100) (default: 25)
#[derive(Debug, Clone)]
pub struct GameConfig {
    /// Maximum X coordinate boundary for the game world
    pub world_max_x: f32,
    /// Minimum X coordinate boundary for the game world  
    pub world_min_x: f32,
    /// Maximum Y coordinate boundary for the game world
    pub world_max_y: f32,
    /// Minimum Y coordinate boundary for the game world
    pub world_min_y: f32,
    /// Movement speed multiplier for player movements
    pub movement_speed: f32,
    /// Minimum distance between players (collision radius)
    pub player_collision_radius: f32,
    /// Maximum length allowed for player names
    pub max_player_name_length: usize,
    /// Maximum length allowed for chat messages
    pub max_chat_message_length: usize,
    /// Heartbeat interval in seconds
    pub heartbeat_interval_seconds: u64,
    /// Heartbeat timeout in seconds (how long to wait before considering a player inactive)
    pub heartbeat_timeout_seconds: u64,
    /// Maximum number of players allowed in the game
    pub max_players: usize,
    /// Attack cooldown in seconds
    pub attack_cooldown_seconds: u64,
    /// Initial player health
    pub initial_player_health: u32,
    /// Attack damage amount
    pub attack_damage: u32,
    /// Attack range in world units
    pub attack_range: f32,
    /// Base damage amount
    pub base_damage: u32,
    /// Critical hit chance (0.0 to 1.0)
    pub crit_chance: f32,
    /// Critical hit damage multiplier
    pub crit_multiplier: f32,
    /// Enable game state persistence
    pub enable_persistence: bool,
    /// Directory for storing persistent game data
    pub persistence_dir: String,
    /// Rate limit for messages per second per connection (DoS protection)
    pub message_rate_limit: f32,
    /// Maximum burst size for rate limiting (number of tokens in bucket)
    pub message_burst_size: u32,
    /// Minimum interval between processing messages in milliseconds (privacy protection)
    pub message_processing_interval_ms: u64,
    /// Enable message processing pacing for enhanced privacy
    pub enable_message_processing_pacing: bool,
    /// Interval for broadcasting game state in seconds
    pub state_broadcast_interval_seconds: u64,
    /// Interval for cleaning up inactive players in seconds
    pub inactive_player_cleanup_interval_seconds: u64,
    /// Replay protection window size (number of sequence numbers to track for replay prevention)
    pub replay_protection_window_size: u8,
    /// Enable adaptive replay protection window sizing
    pub replay_protection_adaptive: bool,
    /// Minimum window size for adaptive replay protection
    pub replay_protection_min_window: u8,
    /// Maximum window size for adaptive replay protection
    pub replay_protection_max_window: u8,
    /// Cooldown period in seconds between window size adjustments
    pub replay_protection_adjustment_cooldown: u64,
    /// Jitter percentage to apply to message processing pacing (0-100)
    pub message_processing_jitter_percent: u8,
}

impl Default for GameConfig {
    fn default() -> Self {
        Self {
            world_max_x: 100.0,
            world_min_x: -100.0,
            world_max_y: 100.0,
            world_min_y: -100.0,
            movement_speed: 14.0,
            player_collision_radius: 7.0,
            max_player_name_length: 50,
            max_chat_message_length: 500,
            heartbeat_interval_seconds: 30,
            heartbeat_timeout_seconds: 90,
            max_players: 100,
            attack_cooldown_seconds: 3,
            initial_player_health: 100,
            attack_damage: 20,
            attack_range: 28.0,
            base_damage: 10,
            crit_chance: 0.15,    // 15% chance
            crit_multiplier: 2.0, // 2x damage
            enable_persistence: true,
            persistence_dir: "./game_data".to_string(),
            message_rate_limit: 10.0,
            message_burst_size: 20,
            message_processing_interval_ms: 100,
            enable_message_processing_pacing: true,
            state_broadcast_interval_seconds: 5,
            inactive_player_cleanup_interval_seconds: 45,
            replay_protection_window_size: 64, // Default window size for replay protection
            replay_protection_adaptive: true,
            replay_protection_min_window: 32,
            replay_protection_max_window: 96,
            replay_protection_adjustment_cooldown: 60,
            message_processing_jitter_percent: 25, // Default 25% jitter for privacy protection
        }
    }
}

impl GameConfig {
    /// Load configuration with environment variable overrides and validation
    pub fn load() -> Result<Self> {
        let mut config = Self::default();

        // Load environment overrides with validation
        config.world_max_x = Self::load_env_f32("NYMQUEST_WORLD_MAX_X", config.world_max_x)?;
        config.world_min_x = Self::load_env_f32("NYMQUEST_WORLD_MIN_X", config.world_min_x)?;
        config.world_max_y = Self::load_env_f32("NYMQUEST_WORLD_MAX_Y", config.world_max_y)?;
        config.world_min_y = Self::load_env_f32("NYMQUEST_WORLD_MIN_Y", config.world_min_y)?;

        config.movement_speed =
            Self::load_env_f32("NYMQUEST_MOVEMENT_SPEED", config.movement_speed)?;
        config.player_collision_radius = Self::load_env_f32(
            "NYMQUEST_PLAYER_COLLISION_RADIUS",
            config.player_collision_radius,
        )?;

        config.max_player_name_length = Self::load_env_usize(
            "NYMQUEST_MAX_PLAYER_NAME_LENGTH",
            config.max_player_name_length,
        )?;
        config.max_chat_message_length = Self::load_env_usize(
            "NYMQUEST_MAX_CHAT_MESSAGE_LENGTH",
            config.max_chat_message_length,
        )?;
        config.heartbeat_interval_seconds = Self::load_env_u64(
            "NYMQUEST_HEARTBEAT_INTERVAL_SECONDS",
            config.heartbeat_interval_seconds,
        )?;
        config.heartbeat_timeout_seconds = Self::load_env_u64(
            "NYMQUEST_HEARTBEAT_TIMEOUT_SECONDS",
            config.heartbeat_timeout_seconds,
        )?;
        config.max_players = Self::load_env_usize("NYMQUEST_MAX_PLAYERS", config.max_players)?;
        config.attack_cooldown_seconds = Self::load_env_u64(
            "NYMQUEST_ATTACK_COOLDOWN_SECONDS",
            config.attack_cooldown_seconds,
        )?;
        config.initial_player_health = Self::load_env_u32(
            "NYMQUEST_INITIAL_PLAYER_HEALTH",
            config.initial_player_health,
        )?;
        config.attack_damage = Self::load_env_u32("NYMQUEST_ATTACK_DAMAGE", config.attack_damage)?;
        config.attack_range = Self::load_env_f32("NYMQUEST_ATTACK_RANGE", config.attack_range)?;
        config.base_damage = Self::load_env_u32("NYMQUEST_BASE_DAMAGE", config.base_damage)?;
        config.crit_chance = Self::load_env_f32("NYMQUEST_CRIT_CHANCE", config.crit_chance)?;
        config.crit_multiplier =
            Self::load_env_f32("NYMQUEST_CRIT_MULTIPLIER", config.crit_multiplier)?;

        // Persistence settings
        config.enable_persistence = Self::parse_env_bool("NYMQUEST_ENABLE_PERSISTENCE", true);
        config.persistence_dir =
            env::var("NYMQUEST_PERSISTENCE_DIR").unwrap_or_else(|_| "./game_data".to_string());

        // Rate limiting settings
        config.message_rate_limit =
            Self::load_env_f32("NYMQUEST_MESSAGE_RATE_LIMIT", config.message_rate_limit)?;
        config.message_burst_size =
            Self::load_env_u32("NYMQUEST_MESSAGE_BURST_SIZE", config.message_burst_size)?;
        config.message_processing_interval_ms = Self::load_env_u64(
            "NYMQUEST_MESSAGE_PROCESSING_INTERVAL_MS",
            config.message_processing_interval_ms,
        )?;
        config.enable_message_processing_pacing = Self::load_env_bool(
            "NYMQUEST_ENABLE_MESSAGE_PROCESSING_PACING",
            config.enable_message_processing_pacing,
        )?;
        config.state_broadcast_interval_seconds = Self::load_env_u64(
            "NYMQUEST_STATE_BROADCAST_INTERVAL_SECONDS",
            config.state_broadcast_interval_seconds,
        )?;
        config.inactive_player_cleanup_interval_seconds = Self::load_env_u64(
            "NYMQUEST_INACTIVE_PLAYER_CLEANUP_INTERVAL_SECONDS",
            config.inactive_player_cleanup_interval_seconds,
        )?;
        config.replay_protection_window_size = Self::load_env_u8(
            "NYMQUEST_REPLAY_PROTECTION_WINDOW_SIZE",
            config.replay_protection_window_size,
        )?;
        config.replay_protection_adaptive = Self::load_env_bool(
            "NYMQUEST_REPLAY_PROTECTION_ADAPTIVE",
            config.replay_protection_adaptive,
        )?;
        config.replay_protection_min_window = Self::load_env_u8(
            "NYMQUEST_REPLAY_PROTECTION_MIN_WINDOW",
            config.replay_protection_min_window,
        )?;
        config.replay_protection_max_window = Self::load_env_u8(
            "NYMQUEST_REPLAY_PROTECTION_MAX_WINDOW",
            config.replay_protection_max_window,
        )?;
        config.replay_protection_adjustment_cooldown = Self::load_env_u64(
            "NYMQUEST_REPLAY_PROTECTION_ADJUSTMENT_COOLDOWN",
            config.replay_protection_adjustment_cooldown,
        )?;
        config.message_processing_jitter_percent = Self::load_env_u8(
            "NYMQUEST_MESSAGE_PROCESSING_JITTER_PERCENT",
            config.message_processing_jitter_percent,
        )?;

        // Validate rate limiting settings
        if config.message_rate_limit <= 0.0 {
            return Err(anyhow!(
                "Message rate limit must be positive, got: {}",
                config.message_rate_limit
            ));
        }
        if config.message_burst_size == 0 {
            return Err(anyhow!(
                "Message burst size must be positive, got: {}",
                config.message_burst_size
            ));
        }

        info!(
            "Rate limiting: {:.1} msg/sec, burst: {}",
            config.message_rate_limit, config.message_burst_size
        );

        // Validate configuration
        config.validate()?;

        info!("Game configuration loaded and validated");
        info!(
            "World boundaries: ({}, {}) to ({}, {})",
            config.world_min_x, config.world_min_y, config.world_max_x, config.world_max_y
        );
        info!("Movement speed: {}", config.movement_speed);
        info!(
            "Heartbeat: {}s interval, {}s timeout",
            config.heartbeat_interval_seconds, config.heartbeat_timeout_seconds
        );
        info!(
            "Max players: {}, Max name length: {}, Max chat length: {}",
            config.max_players, config.max_player_name_length, config.max_chat_message_length
        );
        info!(
            "Replay protection window size: {}",
            config.replay_protection_window_size
        );

        Ok(config)
    }

    /// Validate the configuration values for consistency and safety
    pub fn validate(&self) -> Result<()> {
        // Validate world boundaries
        if self.world_min_x >= self.world_max_x {
            return Err(anyhow!(
                "Invalid world boundaries: min_x ({}) must be less than max_x ({})",
                self.world_min_x,
                self.world_max_x
            ));
        }

        if self.world_min_y >= self.world_max_y {
            return Err(anyhow!(
                "Invalid world boundaries: min_y ({}) must be less than max_y ({})",
                self.world_min_y,
                self.world_max_y
            ));
        }

        // Validate movement speed
        if self.movement_speed <= 0.0 {
            return Err(anyhow!(
                "Movement speed must be positive, got: {}",
                self.movement_speed
            ));
        }

        if self.movement_speed > 100.0 {
            warn!(
                "Movement speed {} is very high, this may cause gameplay issues",
                self.movement_speed
            );
        }

        // Validate string lengths
        if self.max_player_name_length == 0 || self.max_player_name_length > 1000 {
            return Err(anyhow!(
                "Invalid max player name length: {} (must be 1-1000)",
                self.max_player_name_length
            ));
        }

        if self.max_chat_message_length == 0 || self.max_chat_message_length > 10000 {
            return Err(anyhow!(
                "Invalid max chat message length: {} (must be 1-10000)",
                self.max_chat_message_length
            ));
        }

        // Validate heartbeat configuration
        if self.heartbeat_interval_seconds == 0 {
            return Err(anyhow!("Heartbeat interval must be positive"));
        }

        if self.heartbeat_timeout_seconds <= self.heartbeat_interval_seconds {
            return Err(anyhow!(
                "Heartbeat timeout ({}) must be greater than interval ({})",
                self.heartbeat_timeout_seconds,
                self.heartbeat_interval_seconds
            ));
        }

        // Validate player limits
        if self.max_players == 0 || self.max_players > 10000 {
            return Err(anyhow!(
                "Invalid max players: {} (must be 1-10000)",
                self.max_players
            ));
        }

        // Validate combat parameters
        if self.attack_cooldown_seconds == 0 {
            return Err(anyhow!("Attack cooldown must be positive"));
        }

        if self.initial_player_health == 0 {
            return Err(anyhow!("Initial player health must be positive"));
        }

        if self.attack_damage == 0 {
            return Err(anyhow!("Attack damage must be positive"));
        }

        if self.attack_range <= 0.0 {
            return Err(anyhow!("Attack range must be positive"));
        }

        if self.base_damage == 0 {
            return Err(anyhow!("Base damage must be positive"));
        }

        if self.crit_chance < 0.0 || self.crit_chance > 1.0 {
            return Err(anyhow!(
                "Critical hit chance must be between 0.0 and 1.0, got: {}",
                self.crit_chance
            ));
        }

        if self.crit_multiplier <= 0.0 {
            return Err(anyhow!("Critical hit multiplier must be positive"));
        }

        if self.attack_damage >= self.initial_player_health {
            warn!(
                "Attack damage ({}) is very high compared to initial health ({})",
                self.attack_damage, self.initial_player_health
            );
        }

        // Validate message processing pacing
        if self.message_processing_interval_ms > 2000 {
            return Err(anyhow!(
                "Message pacing interval too large: {} (must be <= 2000ms)",
                self.message_processing_interval_ms
            ));
        }

        // Validate replay protection window size
        if self.replay_protection_window_size < 16 || self.replay_protection_window_size > 128 {
            return Err(anyhow!(
                "Invalid replay protection window size: {} (must be 16-128)",
                self.replay_protection_window_size
            ));
        }

        Ok(())
    }

    /// Get world boundary check function
    pub fn clamp_position(&self, x: f32, y: f32) -> (f32, f32) {
        let clamped_x = x.clamp(self.world_min_x, self.world_max_x);
        let clamped_y = y.clamp(self.world_min_y, self.world_max_y);
        (clamped_x, clamped_y)
    }

    /// Check if a position is within world boundaries
    pub fn is_position_valid(&self, x: f32, y: f32) -> bool {
        x >= self.world_min_x
            && x <= self.world_max_x
            && y >= self.world_min_y
            && y <= self.world_max_y
    }

    #[allow(dead_code)]
    /// Get heartbeat interval as Duration
    pub fn heartbeat_interval(&self) -> Duration {
        Duration::from_secs(self.heartbeat_interval_seconds)
    }

    #[allow(dead_code)]
    /// Get heartbeat timeout as Duration
    pub fn heartbeat_timeout(&self) -> Duration {
        Duration::from_secs(self.heartbeat_timeout_seconds)
    }

    #[allow(dead_code)]
    /// Get attack cooldown as Duration
    pub fn attack_cooldown(&self) -> Duration {
        Duration::from_secs(self.attack_cooldown_seconds)
    }

    /// Load a f32 value from environment variable with validation
    fn load_env_f32(name: &str, default: f32) -> Result<f32> {
        match env::var(name) {
            Ok(value) => match value.parse::<f32>() {
                Ok(parsed) => Ok(parsed),
                Err(_) => {
                    warn!(
                        "Invalid {} value: '{}', using default: {}",
                        name, value, default
                    );
                    Ok(default)
                }
            },
            Err(_) => Ok(default),
        }
    }

    /// Load a boolean value from environment variable with validation
    fn load_env_bool(name: &str, default: bool) -> Result<bool> {
        match env::var(name) {
            Ok(_value) => Ok(Self::parse_env_bool(name, default)),
            Err(_) => Ok(default),
        }
    }

    fn load_env_u64(var_name: &str, default: u64) -> Result<u64> {
        match env::var(var_name) {
            Ok(val) => val
                .parse::<u64>()
                .map_err(|e| anyhow!("Invalid u64 value for {}: {} ({})", var_name, val, e)),
            Err(_) => Ok(default),
        }
    }

    fn load_env_u32(var_name: &str, default: u32) -> Result<u32> {
        match env::var(var_name) {
            Ok(val) => val
                .parse::<u32>()
                .map_err(|e| anyhow!("Invalid u32 value for {}: {} ({})", var_name, val, e)),
            Err(_) => Ok(default),
        }
    }

    fn load_env_usize(var_name: &str, default: usize) -> Result<usize> {
        match env::var(var_name) {
            Ok(val) => val
                .trim()
                .parse::<usize>()
                .map_err(|e| anyhow!("Invalid usize value for {}: {} ({})", var_name, val, e)),
            Err(_) => Ok(default),
        }
    }

    /// Load a u8 value from an environment variable
    fn load_env_u8(var_name: &str, default: u8) -> Result<u8> {
        match env::var(var_name) {
            Ok(val) => match val.trim().parse::<u8>() {
                Ok(v) => Ok(v),
                Err(e) => {
                    warn!(
                        "Invalid u8 value for {}: '{}' ({}), using default: {}",
                        var_name, val, e, default
                    );
                    Ok(default)
                }
            },
            Err(_) => Ok(default),
        }
    }

    fn parse_env_bool(var_name: &str, default: bool) -> bool {
        match env::var(var_name) {
            Ok(val) => match val.to_lowercase().as_str() {
                "true" => true,
                "false" => false,
                _ => default,
            },
            Err(_) => default,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config_validation() {
        let config = GameConfig::default();
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_invalid_world_boundaries() {
        let config = GameConfig {
            world_min_x: 100.0,
            world_max_x: -100.0,
            ..Default::default()
        };
        assert!(config.validate().is_err());
    }

    #[test]
    fn test_position_validation() {
        let config = GameConfig::default();
        assert!(config.is_position_valid(0.0, 0.0));
        assert!(config.is_position_valid(-100.0, -100.0));
        assert!(config.is_position_valid(100.0, 100.0));
        assert!(!config.is_position_valid(-101.0, 0.0));
        assert!(!config.is_position_valid(0.0, 101.0));
    }

    #[test]
    fn test_position_clamping() {
        let config = GameConfig::default();
        let (x, y) = config.clamp_position(150.0, -150.0);
        assert_eq!(x, 100.0);
        assert_eq!(y, -100.0);
    }
}
