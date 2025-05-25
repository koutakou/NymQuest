use anyhow::{anyhow, Result};
use std::env;
use std::time::Duration;
use tracing::{info, warn};

/// Client configuration constants with validation and environment override support
#[derive(Debug, Clone)]
pub struct ClientConfig {
    /// Maximum length allowed for player names
    pub max_player_name_length: usize,
    /// Maximum length allowed for chat messages
    pub max_chat_message_length: usize,
    /// Connection timeout in milliseconds for network operations
    pub connection_timeout_ms: u64,
    /// Initial timeout for acknowledgements in milliseconds
    pub initial_ack_timeout_ms: u64,
    /// Subsequent timeout for acknowledgements in milliseconds
    pub subsequent_ack_timeout_ms: u64,
    /// Maximum number of retries for sending messages
    pub max_retries: usize,
    /// Render frame rate limit (FPS)
    pub max_fps: u32,
    /// Command history size limit
    pub max_command_history: usize,
    /// Chat history size limit
    pub max_chat_history: usize,
    /// Enable debug mode for additional logging
    pub debug_mode: bool,
    /// Path to server address file
    pub server_address_file: String,
    /// Movement speed for player movement
    pub movement_speed: f32,
    /// Minimum interval between message sends in milliseconds (privacy protection)
    pub message_pacing_interval_ms: u64,
    /// Enable message pacing for enhanced privacy (adds jitter to prevent timing analysis)
    pub enable_message_pacing: bool,
    /// Maximum jitter percentage to apply to message pacing (0-100)
    pub message_pacing_jitter_percent: u8,
    /// Replay protection window size (number of sequence numbers to track for replay prevention)
    pub replay_protection_window_size: u8,
}

impl Default for ClientConfig {
    fn default() -> Self {
        Self {
            max_player_name_length: 50,
            max_chat_message_length: 500,
            connection_timeout_ms: 10000,
            initial_ack_timeout_ms: 5000,
            subsequent_ack_timeout_ms: 2000,
            max_retries: 3,
            max_fps: 60,
            max_command_history: 1000,
            max_chat_history: 500,
            debug_mode: false,
            server_address_file: "server_address.txt".to_string(),
            movement_speed: 14.0, // Same as server default
            message_pacing_interval_ms: 100,
            enable_message_pacing: true, // Enabled by default for better privacy
            message_pacing_jitter_percent: 25, // Add up to 25% random jitter to message pacing
            replay_protection_window_size: 64, // Default window size for tracking sequence numbers
        }
    }
}

impl ClientConfig {
    /// Load configuration with environment variable overrides and validation
    pub fn load() -> Result<Self> {
        let mut config = Self::default();

        // Load environment overrides with validation
        config.max_player_name_length = Self::load_env_usize(
            "NYMQUEST_CLIENT_MAX_PLAYER_NAME_LENGTH",
            config.max_player_name_length,
        )?;
        config.max_chat_message_length = Self::load_env_usize(
            "NYMQUEST_CLIENT_MAX_CHAT_MESSAGE_LENGTH",
            config.max_chat_message_length,
        )?;
        config.connection_timeout_ms = Self::load_env_u64(
            "NYMQUEST_CLIENT_CONNECTION_TIMEOUT_MS",
            config.connection_timeout_ms,
        )?;
        config.initial_ack_timeout_ms = Self::load_env_u64(
            "NYMQUEST_CLIENT_INITIAL_ACK_TIMEOUT_MS",
            config.initial_ack_timeout_ms,
        )?;
        config.subsequent_ack_timeout_ms = Self::load_env_u64(
            "NYMQUEST_CLIENT_SUBSEQUENT_ACK_TIMEOUT_MS",
            config.subsequent_ack_timeout_ms,
        )?;
        config.max_retries =
            Self::load_env_usize("NYMQUEST_CLIENT_MAX_RETRIES", config.max_retries)?;
        config.max_fps = Self::load_env_u32("NYMQUEST_CLIENT_MAX_FPS", config.max_fps)?;
        config.max_command_history = Self::load_env_usize(
            "NYMQUEST_CLIENT_MAX_COMMAND_HISTORY",
            config.max_command_history,
        )?;
        config.max_chat_history =
            Self::load_env_usize("NYMQUEST_CLIENT_MAX_CHAT_HISTORY", config.max_chat_history)?;
        config.debug_mode = Self::load_env_bool("NYMQUEST_CLIENT_DEBUG_MODE", config.debug_mode)?;
        config.server_address_file = Self::load_env_string(
            "NYMQUEST_CLIENT_SERVER_ADDRESS_FILE",
            config.server_address_file,
        );
        config.movement_speed =
            Self::load_env_f32("NYMQUEST_CLIENT_MOVEMENT_SPEED", config.movement_speed)?;
        config.message_pacing_interval_ms = Self::load_env_u64(
            "NYMQUEST_CLIENT_MESSAGE_PACING_INTERVAL_MS",
            config.message_pacing_interval_ms,
        )?;
        config.enable_message_pacing = Self::load_env_bool(
            "NYMQUEST_CLIENT_ENABLE_MESSAGE_PACING",
            config.enable_message_pacing,
        )?;
        config.message_pacing_jitter_percent = Self::load_env_u8(
            "NYMQUEST_CLIENT_MESSAGE_PACING_JITTER_PERCENT",
            config.message_pacing_jitter_percent,
        )?;
        config.replay_protection_window_size = Self::load_env_u8(
            "NYMQUEST_CLIENT_REPLAY_PROTECTION_WINDOW_SIZE",
            config.replay_protection_window_size,
        )?;

        // Validate configuration
        config.validate()?;

        info!("Client configuration loaded and validated");
        info!(
            "Network timeouts: {}ms connection, {}ms initial ack, {}ms subsequent ack",
            config.connection_timeout_ms,
            config.initial_ack_timeout_ms,
            config.subsequent_ack_timeout_ms
        );
        info!(
            "Limits: {} retries, {}fps, {} cmd history, {} chat history",
            config.max_retries, config.max_fps, config.max_command_history, config.max_chat_history
        );
        info!(
            "String limits: {} name length, {} chat length",
            config.max_player_name_length, config.max_chat_message_length
        );
        info!("Debug mode: {}", config.debug_mode);
        info!("Server address file: {}", config.server_address_file);
        info!("Movement speed: {}", config.movement_speed);
        info!(
            "Message pacing interval: {}ms",
            config.message_pacing_interval_ms
        );
        info!("Enable message pacing: {}", config.enable_message_pacing);
        info!(
            "Message pacing jitter: {}%",
            config.message_pacing_jitter_percent
        );
        info!(
            "Replay protection window size: {}",
            config.replay_protection_window_size
        );

        Ok(config)
    }

    /// Validate the configuration values for consistency and safety
    pub fn validate(&self) -> Result<()> {
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

        // Validate network timeouts
        if self.connection_timeout_ms == 0 || self.connection_timeout_ms > 300000 {
            return Err(anyhow!(
                "Invalid connection timeout: {} (must be 1-300000ms)",
                self.connection_timeout_ms
            ));
        }

        if self.initial_ack_timeout_ms == 0 || self.initial_ack_timeout_ms > 60000 {
            return Err(anyhow!(
                "Invalid initial ack timeout: {} (must be 1-60000ms)",
                self.initial_ack_timeout_ms
            ));
        }

        if self.subsequent_ack_timeout_ms == 0 || self.subsequent_ack_timeout_ms > 60000 {
            return Err(anyhow!(
                "Invalid subsequent ack timeout: {} (must be 1-60000ms)",
                self.subsequent_ack_timeout_ms
            ));
        }

        if self.subsequent_ack_timeout_ms >= self.initial_ack_timeout_ms {
            warn!(
                "Subsequent ack timeout ({}) should be less than initial timeout ({})",
                self.subsequent_ack_timeout_ms, self.initial_ack_timeout_ms
            );
        }

        // Validate retry count
        if self.max_retries == 0 || self.max_retries > 10 {
            return Err(anyhow!(
                "Invalid max retries: {} (must be 1-10)",
                self.max_retries
            ));
        }

        // Validate FPS
        if self.max_fps == 0 || self.max_fps > 144 {
            return Err(anyhow!("Invalid max FPS: {} (must be 1-144)", self.max_fps));
        }

        // Validate replay protection window size
        if self.replay_protection_window_size < 16 || self.replay_protection_window_size > 128 {
            return Err(anyhow!(
                "Invalid replay protection window size: {} (must be 16-128)",
                self.replay_protection_window_size
            ));
        }

        if self.max_fps > 60 {
            warn!(
                "High FPS setting ({}) may consume more CPU resources",
                self.max_fps
            );
        }

        // Validate history limits
        if self.max_command_history == 0 || self.max_command_history > 10000 {
            return Err(anyhow!(
                "Invalid max command history: {} (must be 1-10000)",
                self.max_command_history
            ));
        }

        if self.max_chat_history == 0 || self.max_chat_history > 10000 {
            return Err(anyhow!(
                "Invalid max chat history: {} (must be 1-10000)",
                self.max_chat_history
            ));
        }

        // Validate movement speed
        if self.movement_speed <= 0.0 {
            return Err(anyhow!(
                "Invalid movement speed: {} (must be greater than 0)",
                self.movement_speed
            ));
        }

        // Validate message pacing interval
        if self.message_pacing_interval_ms == 0 || self.message_pacing_interval_ms > 10000 {
            return Err(anyhow!(
                "Invalid message pacing interval: {} (must be 1-10000ms)",
                self.message_pacing_interval_ms
            ));
        }

        // Validate jitter percentage
        if self.message_pacing_jitter_percent > 100 {
            return Err(anyhow!(
                "Invalid message pacing jitter: {}% (must be 0-100%)",
                self.message_pacing_jitter_percent
            ));
        }

        Ok(())
    }

    /// Validate player name according to configuration
    #[allow(dead_code)] // Part of complete configuration API for future use
    pub fn validate_player_name(&self, name: &str) -> Result<()> {
        if name.is_empty() {
            return Err(anyhow!("Player name cannot be empty"));
        }

        if name.len() > self.max_player_name_length {
            return Err(anyhow!(
                "Player name too long: {} characters (max: {})",
                name.len(),
                self.max_player_name_length
            ));
        }

        // Check for invalid characters (control characters, etc.)
        if name.chars().any(|c| c.is_control()) {
            return Err(anyhow!("Player name contains invalid characters"));
        }

        // Check for excessive whitespace
        if name.trim().is_empty() {
            return Err(anyhow!("Player name cannot be only whitespace"));
        }

        if name.trim().len() != name.len() {
            return Err(anyhow!(
                "Player name cannot have leading or trailing whitespace"
            ));
        }

        Ok(())
    }

    /// Validate chat message according to configuration
    #[allow(dead_code)] // Part of complete configuration API for future use
    pub fn validate_chat_message(&self, message: &str) -> Result<()> {
        if message.is_empty() {
            return Err(anyhow!("Chat message cannot be empty"));
        }

        if message.len() > self.max_chat_message_length {
            return Err(anyhow!(
                "Chat message too long: {} characters (max: {})",
                message.len(),
                self.max_chat_message_length
            ));
        }

        // Check for excessive control characters (allow newlines in chat)
        let control_count = message
            .chars()
            .filter(|c| c.is_control() && *c != '\n' && *c != '\t')
            .count();
        if control_count > 0 {
            return Err(anyhow!("Chat message contains invalid characters"));
        }

        // Check for excessive whitespace
        if message.trim().is_empty() {
            return Err(anyhow!("Chat message cannot be only whitespace"));
        }

        Ok(())
    }

    /// Get connection timeout as Duration
    #[allow(dead_code)] // Part of complete configuration API for future use
    pub fn connection_timeout(&self) -> Duration {
        Duration::from_millis(self.connection_timeout_ms)
    }

    /// Get initial ack timeout as Duration
    #[allow(dead_code)] // Part of complete configuration API for future use
    pub fn initial_ack_timeout(&self) -> Duration {
        Duration::from_millis(self.initial_ack_timeout_ms)
    }

    /// Get subsequent ack timeout as Duration
    #[allow(dead_code)] // Part of complete configuration API for future use
    pub fn subsequent_ack_timeout(&self) -> Duration {
        Duration::from_millis(self.subsequent_ack_timeout_ms)
    }

    /// Get frame duration for FPS limiting
    #[allow(dead_code)] // Part of complete configuration API for future use
    pub fn frame_duration(&self) -> Duration {
        Duration::from_millis(1000 / self.max_fps as u64)
    }

    // Helper functions for loading environment variables with validation
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
                .parse::<usize>()
                .map_err(|e| anyhow!("Invalid usize value for {}: {} ({})", var_name, val, e)),
            Err(_) => Ok(default),
        }
    }

    fn load_env_bool(var_name: &str, default: bool) -> Result<bool> {
        match env::var(var_name) {
            Ok(val) => {
                let trimmed = val.trim().to_lowercase();
                match trimmed.as_str() {
                    "true" | "1" | "yes" | "on" => Ok(true),
                    "false" | "0" | "no" | "off" => Ok(false),
                    _ => {
                        warn!(
                            "Invalid boolean value for {}: '{}', using default: {}",
                            var_name, val, default
                        );
                        Ok(default)
                    }
                }
            }
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

    fn load_env_string(var_name: &str, default: String) -> String {
        env::var(var_name).unwrap_or(default)
    }

    fn load_env_f32(var_name: &str, default: f32) -> Result<f32> {
        match env::var(var_name) {
            Ok(val) => val
                .parse()
                .map_err(|e| anyhow!("Invalid f32 value for {}: {} ({})", var_name, val, e)),
            Err(_) => Ok(default),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config_validation() {
        let config = ClientConfig::default();
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_player_name_validation() {
        let config = ClientConfig::default();

        // Valid names
        assert!(config.validate_player_name("Alice").is_ok());
        assert!(config.validate_player_name("Player123").is_ok());
        assert!(config.validate_player_name("Test User").is_ok());

        // Invalid names
        assert!(config.validate_player_name("").is_err());
        assert!(config.validate_player_name("   ").is_err());
        assert!(config.validate_player_name(" Alice").is_err());
        assert!(config.validate_player_name("Alice ").is_err());
        assert!(config.validate_player_name(&"a".repeat(1000)).is_err());
    }

    #[test]
    fn test_chat_message_validation() {
        let config = ClientConfig::default();

        // Valid messages
        assert!(config.validate_chat_message("Hello world!").is_ok());
        assert!(config.validate_chat_message("Multi\nline\nmessage").is_ok());

        // Invalid messages
        assert!(config.validate_chat_message("").is_err());
        assert!(config.validate_chat_message("   ").is_err());
        assert!(config.validate_chat_message(&"a".repeat(10000)).is_err());
    }

    #[test]
    fn test_timeout_durations() {
        let config = ClientConfig::default();
        assert_eq!(config.connection_timeout().as_millis(), 10000);
        assert_eq!(config.initial_ack_timeout().as_millis(), 5000);
        assert_eq!(config.subsequent_ack_timeout().as_millis(), 2000);
    }
}
