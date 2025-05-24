use anyhow::{Result, anyhow};
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
/// - NYMQUEST_MAX_PLAYER_NAME_LENGTH: Maximum player name length (default: 50)
/// - NYMQUEST_MAX_CHAT_MESSAGE_LENGTH: Maximum chat message length (default: 500)
/// - NYMQUEST_HEARTBEAT_INTERVAL_SECONDS: Heartbeat request interval (default: 30)
/// - NYMQUEST_HEARTBEAT_TIMEOUT_SECONDS: Player inactivity timeout (default: 90)
/// - NYMQUEST_MAX_PLAYERS: Maximum concurrent players (default: 100)
/// - NYMQUEST_ATTACK_COOLDOWN_SECONDS: Attack cooldown period (default: 3)
/// - NYMQUEST_INITIAL_PLAYER_HEALTH: Starting player health (default: 100)
/// - NYMQUEST_ATTACK_DAMAGE: Base attack damage (default: 20)
/// - NYMQUEST_ATTACK_RANGE: Attack range in world units (default: 28.0)
/// - NYMQUEST_ENABLE_PERSISTENCE: Enable game state persistence (default: true)
/// - NYMQUEST_PERSISTENCE_DIR: Directory for saving game state (default: "./game_data")
/// - NYMQUEST_MESSAGE_RATE_LIMIT: Messages per second limit per connection (default: 10)
/// - NYMQUEST_MESSAGE_BURST_SIZE: Maximum burst size for rate limiting (default: 20)
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
}

impl Default for GameConfig {
    fn default() -> Self {
        Self {
            world_max_x: 100.0,
            world_min_x: -100.0,
            world_max_y: 100.0,
            world_min_y: -100.0,
            movement_speed: 14.0,
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
            crit_chance: 0.15, // 15% chance
            crit_multiplier: 2.0, // 2x damage
            enable_persistence: true,
            persistence_dir: "./game_data".to_string(),
            message_rate_limit: 10.0,
            message_burst_size: 20,
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
        config.movement_speed = Self::load_env_f32("NYMQUEST_MOVEMENT_SPEED", config.movement_speed)?;
        config.max_player_name_length = Self::load_env_usize("NYMQUEST_MAX_PLAYER_NAME_LENGTH", config.max_player_name_length)?;
        config.max_chat_message_length = Self::load_env_usize("NYMQUEST_MAX_CHAT_MESSAGE_LENGTH", config.max_chat_message_length)?;
        config.heartbeat_interval_seconds = Self::load_env_u64("NYMQUEST_HEARTBEAT_INTERVAL_SECONDS", config.heartbeat_interval_seconds)?;
        config.heartbeat_timeout_seconds = Self::load_env_u64("NYMQUEST_HEARTBEAT_TIMEOUT_SECONDS", config.heartbeat_timeout_seconds)?;
        config.max_players = Self::load_env_usize("NYMQUEST_MAX_PLAYERS", config.max_players)?;
        config.attack_cooldown_seconds = Self::load_env_u64("NYMQUEST_ATTACK_COOLDOWN_SECONDS", config.attack_cooldown_seconds)?;
        config.initial_player_health = Self::load_env_u32("NYMQUEST_INITIAL_PLAYER_HEALTH", config.initial_player_health)?;
        config.attack_damage = Self::load_env_u32("NYMQUEST_ATTACK_DAMAGE", config.attack_damage)?;
        config.attack_range = Self::load_env_f32("NYMQUEST_ATTACK_RANGE", config.attack_range)?;
        config.base_damage = Self::load_env_u32("NYMQUEST_BASE_DAMAGE", config.base_damage)?;
        config.crit_chance = Self::load_env_f32("NYMQUEST_CRIT_CHANCE", config.crit_chance)?;
        config.crit_multiplier = Self::load_env_f32("NYMQUEST_CRIT_MULTIPLIER", config.crit_multiplier)?;
        
        // Persistence settings
        let enable_persistence = Self::parse_env_bool("NYMQUEST_ENABLE_PERSISTENCE", true);
        let persistence_dir = env::var("NYMQUEST_PERSISTENCE_DIR")
            .unwrap_or_else(|_| "./game_data".to_string());
        
        // Rate limiting settings
        let message_rate_limit = Self::load_env_f32("NYMQUEST_MESSAGE_RATE_LIMIT", 10.0)?;
        let message_burst_size = Self::load_env_u32("NYMQUEST_MESSAGE_BURST_SIZE", 20)?;
        
        // Validate rate limiting settings
        if message_rate_limit <= 0.0 {
            return Err(anyhow!("Message rate limit must be positive, got: {}", message_rate_limit));
        }
        if message_burst_size == 0 {
            return Err(anyhow!("Message burst size must be positive, got: {}", message_burst_size));
        }
        
        info!("Rate limiting: {:.1} msg/sec, burst: {}", message_rate_limit, message_burst_size);

        // Validate configuration
        config.validate()?;
        
        info!("Game configuration loaded and validated");
        info!("World boundaries: ({}, {}) to ({}, {})", 
              config.world_min_x, config.world_min_y, 
              config.world_max_x, config.world_max_y);
        info!("Movement speed: {}", config.movement_speed);
        info!("Heartbeat: {}s interval, {}s timeout", 
              config.heartbeat_interval_seconds, config.heartbeat_timeout_seconds);
        info!("Max players: {}, Max name length: {}, Max chat length: {}", 
              config.max_players, config.max_player_name_length, config.max_chat_message_length);
        
        Ok(GameConfig {
            world_max_x: config.world_max_x,
            world_min_x: config.world_min_x,
            world_max_y: config.world_max_y,
            world_min_y: config.world_min_y,
            movement_speed: config.movement_speed,
            max_player_name_length: config.max_player_name_length,
            max_chat_message_length: config.max_chat_message_length,
            heartbeat_interval_seconds: config.heartbeat_interval_seconds,
            heartbeat_timeout_seconds: config.heartbeat_timeout_seconds,
            max_players: config.max_players,
            attack_cooldown_seconds: config.attack_cooldown_seconds,
            initial_player_health: config.initial_player_health,
            attack_damage: config.attack_damage,
            attack_range: config.attack_range,
            base_damage: config.base_damage,
            crit_chance: config.crit_chance,
            crit_multiplier: config.crit_multiplier,
            enable_persistence,
            persistence_dir,
            message_rate_limit,
            message_burst_size,
        })
    }
    
    /// Validate the configuration values for consistency and safety
    pub fn validate(&self) -> Result<()> {
        // Validate world boundaries
        if self.world_min_x >= self.world_max_x {
            return Err(anyhow!("Invalid world boundaries: min_x ({}) must be less than max_x ({})", 
                              self.world_min_x, self.world_max_x));
        }
        
        if self.world_min_y >= self.world_max_y {
            return Err(anyhow!("Invalid world boundaries: min_y ({}) must be less than max_y ({})", 
                              self.world_min_y, self.world_max_y));
        }
        
        // Validate movement speed
        if self.movement_speed <= 0.0 {
            return Err(anyhow!("Movement speed must be positive, got: {}", self.movement_speed));
        }
        
        if self.movement_speed > 100.0 {
            warn!("Movement speed {} is very high, this may cause gameplay issues", self.movement_speed);
        }
        
        // Validate string lengths
        if self.max_player_name_length == 0 || self.max_player_name_length > 1000 {
            return Err(anyhow!("Invalid max player name length: {} (must be 1-1000)", 
                              self.max_player_name_length));
        }
        
        if self.max_chat_message_length == 0 || self.max_chat_message_length > 10000 {
            return Err(anyhow!("Invalid max chat message length: {} (must be 1-10000)", 
                              self.max_chat_message_length));
        }
        
        // Validate heartbeat configuration
        if self.heartbeat_interval_seconds == 0 {
            return Err(anyhow!("Heartbeat interval must be positive"));
        }
        
        if self.heartbeat_timeout_seconds <= self.heartbeat_interval_seconds {
            return Err(anyhow!("Heartbeat timeout ({}) must be greater than interval ({})", 
                              self.heartbeat_timeout_seconds, self.heartbeat_interval_seconds));
        }
        
        // Validate player limits
        if self.max_players == 0 || self.max_players > 10000 {
            return Err(anyhow!("Invalid max players: {} (must be 1-10000)", self.max_players));
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
            return Err(anyhow!("Critical hit chance must be between 0.0 and 1.0, got: {}", self.crit_chance));
        }
        
        if self.crit_multiplier <= 0.0 {
            return Err(anyhow!("Critical hit multiplier must be positive"));
        }
        
        if self.attack_damage >= self.initial_player_health {
            warn!("Attack damage ({}) is very high compared to initial health ({})", 
                  self.attack_damage, self.initial_player_health);
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
        x >= self.world_min_x && x <= self.world_max_x && 
        y >= self.world_min_y && y <= self.world_max_y
    }
    
    /// Get heartbeat interval as Duration
    pub fn heartbeat_interval(&self) -> Duration {
        Duration::from_secs(self.heartbeat_interval_seconds)
    }
    
    /// Get heartbeat timeout as Duration
    pub fn heartbeat_timeout(&self) -> Duration {
        Duration::from_secs(self.heartbeat_timeout_seconds)
    }
    
    /// Get attack cooldown as Duration
    pub fn attack_cooldown(&self) -> Duration {
        Duration::from_secs(self.attack_cooldown_seconds)
    }
    
    // Helper functions for loading environment variables with validation
    fn load_env_f32(var_name: &str, default: f32) -> Result<f32> {
        match env::var(var_name) {
            Ok(val) => {
                val.parse::<f32>()
                    .map_err(|e| anyhow!("Invalid float value for {}: {} ({})", var_name, val, e))
            },
            Err(_) => Ok(default),
        }
    }
    
    fn load_env_u64(var_name: &str, default: u64) -> Result<u64> {
        match env::var(var_name) {
            Ok(val) => {
                val.parse::<u64>()
                    .map_err(|e| anyhow!("Invalid u64 value for {}: {} ({})", var_name, val, e))
            },
            Err(_) => Ok(default),
        }
    }
    
    fn load_env_u32(var_name: &str, default: u32) -> Result<u32> {
        match env::var(var_name) {
            Ok(val) => {
                val.parse::<u32>()
                    .map_err(|e| anyhow!("Invalid u32 value for {}: {} ({})", var_name, val, e))
            },
            Err(_) => Ok(default),
        }
    }
    
    fn load_env_usize(var_name: &str, default: usize) -> Result<usize> {
        match env::var(var_name) {
            Ok(val) => {
                val.parse::<usize>()
                    .map_err(|e| anyhow!("Invalid usize value for {}: {} ({})", var_name, val, e))
            },
            Err(_) => Ok(default),
        }
    }
    
    fn parse_env_bool(var_name: &str, default: bool) -> bool {
        match env::var(var_name) {
            Ok(val) => {
                match val.to_lowercase().as_str() {
                    "true" => true,
                    "false" => false,
                    _ => default,
                }
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
        let mut config = GameConfig::default();
        config.world_min_x = 100.0;
        config.world_max_x = -100.0;
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
