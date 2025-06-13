use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::f32::consts::FRAC_1_SQRT_2;

use crate::world_lore::{Faction, WorldRegion};

/// Current protocol version - increment when making breaking changes
pub const PROTOCOL_VERSION: u16 = 1;

/// Minimum supported protocol version for backward compatibility
pub const MIN_SUPPORTED_VERSION: u16 = 1;

/// Protocol version information exchanged during connection setup
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ProtocolVersion {
    pub current: u16,
    pub min_supported: u16,
}

impl Default for ProtocolVersion {
    fn default() -> Self {
        Self {
            current: PROTOCOL_VERSION,
            min_supported: MIN_SUPPORTED_VERSION,
        }
    }
}

impl ProtocolVersion {
    /// Check if this version is compatible with another version
    #[allow(dead_code)] // Part of complete protocol API for future use
    pub fn is_compatible_with(&self, other: &ProtocolVersion) -> bool {
        // We can communicate if our ranges overlap
        self.min_supported <= other.current && other.min_supported <= self.current
    }

    /// Get the negotiated version to use (highest common version)
    #[allow(dead_code)] // Part of complete protocol API for future use
    pub fn negotiate_with(&self, other: &ProtocolVersion) -> Option<u16> {
        if !self.is_compatible_with(other) {
            return None;
        }
        // Use the lower of the two current versions
        Some(self.current.min(other.current))
    }
}

// Player's position in the game world
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
pub struct Position {
    pub x: f32,
    pub y: f32,
}

impl Position {
    /// Create a new position from coordinates
    #[allow(dead_code)] // Part of complete protocol API for future use
    pub fn new(x: f32, y: f32) -> Self {
        Self { x, y }
    }

    // Add a movement vector to this position
    #[allow(dead_code)]
    pub fn apply_movement(&mut self, move_vector: (f32, f32), speed: f32) {
        self.x += move_vector.0 * speed;
        self.y += move_vector.1 * speed;

        // Note: Boundary clamping is now handled by GameConfig::clamp_position
        // This allows for configurable world boundaries rather than hardcoded values
    }

    /// Calculate distance to another position
    #[allow(dead_code)] // Part of complete protocol API for future use
    pub fn distance_to(&self, other: &Position) -> f32 {
        let dx = self.x - other.x;
        let dy = self.y - other.y;
        (dx * dx + dy * dy).sqrt()
    }
}

// Player representation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Player {
    pub id: String,         // Internal server ID (UUID) - not exposed to other clients
    pub display_id: String, // Public privacy-preserving identifier (e.g. "Player1")
    pub position: Position,
    pub health: u32,
    pub name: String,
    pub last_attack_time: u64,
    pub experience: u32,  // Experience points earned through gameplay
    pub level: u8,        // Player level based on experience
    pub faction: Faction, // The player's chosen faction
}

// Type of client message (used for acknowledgements)
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
pub enum ClientMessageType {
    Register,
    Move,
    Attack,
    Chat,
    Emote,
    Disconnect,
    Heartbeat,
    Ack,
    Whisper,
}

// Message types that the client can send to the server
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ClientMessage {
    // Message to register in the game with protocol version negotiation
    Register {
        name: String,
        faction: Faction, // Player's selected faction
        seq_num: u64,
        protocol_version: ProtocolVersion,
    },
    // Message to move in the game world
    Move {
        direction: Direction,
        seq_num: u64,
    },
    // Message to attack another player (using display_id for privacy)
    Attack {
        target_display_id: String,
        seq_num: u64,
    },
    // Message to send chat to all players
    Chat {
        message: String,
        seq_num: u64,
    },
    // Message to perform an emote
    Emote {
        emote_type: EmoteType,
        seq_num: u64,
    },
    // Message to leave the game
    Disconnect {
        seq_num: u64,
    },
    // Heartbeat message to keep the connection alive
    Heartbeat {
        seq_num: u64,
    },
    // Acknowledge receipt of a server message
    Ack {
        server_seq_num: u64,
        original_type: ServerMessageType,
    },
    // Private message (whisper) to another player
    Whisper {
        target_display_id: String,
        message: String,
        seq_num: u64,
    },
}

// Type of server message (used for acknowledgements)
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
pub enum ServerMessageType {
    RegisterAck,
    GameState,
    Event,
    ChatMessage,
    Error,
    HeartbeatRequest,
    Ack,
    PlayerLeft,
    PlayerUpdate,
    ServerShutdown,
    WhisperMessage,
}

// Message types that the server can send to the client
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ServerMessage {
    // Server is shutting down, clients should disconnect
    ServerShutdown {
        message: String,
        seq_num: u64,
        shutdown_in_seconds: u8,
    },
    // Confirms registration and provides the player ID with protocol version negotiation
    RegisterAck {
        player_id: String,
        seq_num: u64,
        // World boundaries for client synchronization
        world_boundaries: WorldBoundaries,
        // Negotiated protocol version to use for this session
        negotiated_version: u16,
    },
    // Game state update
    GameState {
        players: HashMap<String, Player>,
        seq_num: u64,
        // We could add other game elements here
    },
    // Event notification (attack, etc.)
    Event {
        message: String,
        seq_num: u64,
    },
    // Chat message from another player
    ChatMessage {
        sender_name: String,
        message: String,
        seq_num: u64,
    },
    // Error message
    Error {
        message: String,
        seq_num: u64,
    },
    // Heartbeat request
    HeartbeatRequest {
        seq_num: u64,
    },
    // Acknowledge receipt of a client message
    Ack {
        client_seq_num: u64,
        original_type: ClientMessageType,
    },
    // Player departed
    PlayerLeft {
        display_id: String,
        seq_num: u64,
    },
    // Player status update
    PlayerUpdate {
        display_id: String,
        position: Position,
        health: u32,
        seq_num: u64,
    },
    // Private message (whisper) from another player
    WhisperMessage {
        sender_name: String,
        message: String,
        seq_num: u64,
    },
}

// Helper implementation for ServerMessage to get metadata easily
impl ServerMessage {
    #[allow(dead_code)]
    /// Get the message type
    #[allow(dead_code)] // Part of complete protocol API for future use
    pub fn get_type(&self) -> ServerMessageType {
        match self {
            ServerMessage::ServerShutdown { .. } => ServerMessageType::ServerShutdown,
            ServerMessage::RegisterAck { .. } => ServerMessageType::RegisterAck,
            ServerMessage::GameState { .. } => ServerMessageType::GameState,
            ServerMessage::Event { .. } => ServerMessageType::Event,
            ServerMessage::ChatMessage { .. } => ServerMessageType::ChatMessage,
            ServerMessage::Error { .. } => ServerMessageType::Error,
            ServerMessage::HeartbeatRequest { .. } => ServerMessageType::HeartbeatRequest,
            ServerMessage::Ack { .. } => ServerMessageType::Ack,
            ServerMessage::PlayerLeft { .. } => ServerMessageType::PlayerLeft,
            ServerMessage::PlayerUpdate { .. } => ServerMessageType::PlayerUpdate,
            ServerMessage::WhisperMessage { .. } => ServerMessageType::WhisperMessage,
        }
    }

    #[allow(dead_code)]
    pub fn get_seq_num(&self) -> u64 {
        match self {
            ServerMessage::ServerShutdown { seq_num, .. } => *seq_num,
            ServerMessage::RegisterAck { seq_num, .. } => *seq_num,
            ServerMessage::GameState { seq_num, .. } => *seq_num,
            ServerMessage::Event { seq_num, .. } => *seq_num,
            ServerMessage::ChatMessage { seq_num, .. } => *seq_num,
            ServerMessage::Error { seq_num, .. } => *seq_num,
            ServerMessage::HeartbeatRequest { seq_num, .. } => *seq_num,
            ServerMessage::Ack { client_seq_num, .. } => *client_seq_num,
            ServerMessage::PlayerLeft { seq_num, .. } => *seq_num,
            ServerMessage::PlayerUpdate { seq_num, .. } => *seq_num,
            ServerMessage::WhisperMessage { seq_num, .. } => *seq_num,
        }
    }
}

// Helper implementation for ClientMessage to get metadata easily
impl ClientMessage {
    /// Get the message type
    #[allow(dead_code)] // Part of complete protocol API for future use
    pub fn get_type(&self) -> ClientMessageType {
        match self {
            ClientMessage::Register { .. } => ClientMessageType::Register,
            ClientMessage::Move { .. } => ClientMessageType::Move,
            ClientMessage::Attack { .. } => ClientMessageType::Attack,
            ClientMessage::Chat { .. } => ClientMessageType::Chat,
            ClientMessage::Emote { .. } => ClientMessageType::Emote,
            ClientMessage::Disconnect { .. } => ClientMessageType::Disconnect,
            ClientMessage::Heartbeat { .. } => ClientMessageType::Heartbeat,
            ClientMessage::Ack { .. } => ClientMessageType::Ack,
            ClientMessage::Whisper { .. } => ClientMessageType::Whisper,
        }
    }

    /// Get the sequence number
    #[allow(dead_code)] // Part of complete protocol API for future use
    pub fn get_seq_num(&self) -> u64 {
        match self {
            ClientMessage::Register { seq_num, .. } => *seq_num,
            ClientMessage::Move { seq_num, .. } => *seq_num,
            ClientMessage::Attack { seq_num, .. } => *seq_num,
            ClientMessage::Chat { seq_num, .. } => *seq_num,
            ClientMessage::Emote { seq_num, .. } => *seq_num,
            ClientMessage::Disconnect { seq_num, .. } => *seq_num,
            ClientMessage::Heartbeat { seq_num, .. } => *seq_num,
            ClientMessage::Ack { server_seq_num, .. } => *server_seq_num,
            ClientMessage::Whisper { seq_num, .. } => *seq_num,
        }
    }
}

// Movement direction
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum Direction {
    Up,
    Down,
    Left,
    Right,
    UpLeft,
    UpRight,
    DownLeft,
    DownRight,
}

impl Direction {
    /// Convert direction to a movement vector
    ///
    /// This function takes a direction and returns a movement vector.
    /// The movement vector is a tuple of two f32 values representing the x and y components of the movement.
    #[allow(dead_code)] // Part of complete protocol API for future use
    pub fn to_vector(self) -> (f32, f32) {
        // 1/sqrt(2) ≈ 0.7071 is the correct normalization factor for diagonal movement
        // This ensures that diagonal movement has the same speed as cardinal movement
        let diag = FRAC_1_SQRT_2;

        match self {
            Direction::Up => (0.0, -1.0),
            Direction::Down => (0.0, 1.0),
            Direction::Left => (-1.0, 0.0),
            Direction::Right => (1.0, 0.0),
            Direction::UpLeft => (-diag, -diag), // Properly normalized diagonal vectors
            Direction::UpRight => (diag, -diag),
            Direction::DownLeft => (-diag, diag),
            Direction::DownRight => (diag, diag),
        }
    }

    #[allow(dead_code)]
    pub fn from_str(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "up" | "u" | "north" | "n" => Some(Direction::Up),
            "down" | "d" | "south" | "s" => Some(Direction::Down),
            "left" | "l" | "west" | "w" => Some(Direction::Left),
            "right" | "r" | "east" | "e" => Some(Direction::Right),
            "upleft" | "ul" | "northwest" | "nw" => Some(Direction::UpLeft),
            "upright" | "ur" | "northeast" | "ne" => Some(Direction::UpRight),
            "downleft" | "dl" | "southwest" | "sw" => Some(Direction::DownLeft),
            "downright" | "dr" | "southeast" | "se" => Some(Direction::DownRight),
            _ => None,
        }
    }
}

// World boundaries with cypherpunk-themed properties
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorldBoundaries {
    pub min_x: f32,
    pub max_x: f32,
    pub min_y: f32,
    pub max_y: f32,
    /// Name of the region (e.g., "Neon Harbor", "The Deep Net")
    pub name: String,
    /// Security level in the region, affects encounter risks
    pub security_level: String,
    /// Surveillance density (0.0 to 1.0) affecting privacy
    pub surveillance_density: f32,
    /// The region type from worldbuilding lore
    pub region_type: String,
}

impl WorldBoundaries {
    /// Clamp a position to stay within world boundaries
    #[allow(dead_code)]
    pub fn clamp_position(&self, x: f32, y: f32) -> (f32, f32) {
        let clamped_x = x.clamp(self.min_x, self.max_x);
        let clamped_y = y.clamp(self.min_y, self.max_y);
        (clamped_x, clamped_y)
    }

    /// Check if a position is within world boundaries
    #[allow(dead_code)]
    pub fn is_position_valid(&self, x: f32, y: f32) -> bool {
        x >= self.min_x && x <= self.max_x && y >= self.min_y && y <= self.max_y
    }

    /// Apply boundaries to a Position, modifying it in place
    #[allow(dead_code)]
    pub fn clamp_position_mut(&self, position: &mut Position) {
        let (x, y) = self.clamp_position(position.x, position.y);
        position.x = x;
        position.y = y;
    }

    /// Check if two positions would collide given a minimum distance
    #[allow(dead_code)]
    pub fn would_positions_collide(pos1: &Position, pos2: &Position, min_distance: f32) -> bool {
        pos1.distance_to(pos2) < min_distance
    }

    /// Create WorldBoundaries from GameConfig
    pub fn from_config(config: &crate::config::GameConfig) -> Self {
        let region = if let Some(region_type) = &config.world_region {
            region_type.clone()
        } else {
            "Neon Harbor".to_string()
        };

        // Convert to WorldRegion enum if possible, otherwise use default
        let world_region = match region.as_str() {
            "Neon Harbor" => WorldRegion::NeonHarbor,
            "Deep Net" => WorldRegion::DeepNet,
            "Data Havens" => WorldRegion::DataHavens,
            "Dead Zones" => WorldRegion::DeadZones,
            "The Grid" => WorldRegion::TheGrid,
            _ => WorldRegion::NeonHarbor,
        };

        // Get lore boundaries with all properties
        let lore_boundaries = world_region.get_boundaries();

        // Create new boundaries with cypherpunk properties
        Self {
            min_x: config.world_min_x,
            max_x: config.world_max_x,
            min_y: config.world_min_y,
            max_y: config.world_max_y,
            name: lore_boundaries.name.to_string(),
            security_level: format!("{:?}", lore_boundaries.security_level),
            surveillance_density: lore_boundaries.surveillance_density,
            region_type: region,
        }
    }

    /// Calculate surveillance risk for a given position
    /// Returns a value from 0.0 (no surveillance) to 1.0 (maximum surveillance)
    #[allow(dead_code)]
    pub fn calculate_surveillance_risk(&self, x: f32, y: f32) -> f32 {
        if !self.is_position_valid(x, y) {
            return 0.0;
        }

        // Base risk from the region's surveillance density
        let mut risk = self.surveillance_density;

        // Distance from center affects risk - closer to center is higher risk in most regions
        let center_x = (self.min_x + self.max_x) / 2.0;
        let center_y = (self.min_y + self.max_y) / 2.0;

        let max_distance =
            ((self.max_x - self.min_x).powi(2) + (self.max_y - self.min_y).powi(2)).sqrt() / 2.0;
        let distance = ((x - center_x).powi(2) + (y - center_y).powi(2)).sqrt();
        let distance_factor = 1.0 - (distance / max_distance);

        // Adjust risk based on security level
        match self.security_level.as_str() {
            "Maximum" => {
                // In maximum security, it's equally surveilled everywhere
                risk *= 0.8 + (0.2 * distance_factor);
            }
            "High" => {
                // High security has more surveillance in the center
                risk *= 0.6 + (0.4 * distance_factor);
            }
            "Moderate" => {
                // Moderate security has some surveillance hotspots
                risk *= 0.4 + (0.6 * distance_factor);
            }
            "Low" => {
                // Low security has minimal surveillance mostly at the edges
                risk *= 0.2 + (0.1 * distance_factor);
            }
            _ => {
                // No security has almost no surveillance
                risk *= 0.05;
            }
        }

        risk.clamp(0.0, 1.0)
    }
}

// Types of emotes that players can perform - enhanced with cypherpunk themes
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
pub enum EmoteType {
    // Standard emotes
    Wave,
    Bow,
    Laugh,
    Dance,
    Salute,
    Shrug,
    Cheer,
    Clap,
    ThumbsUp,

    // Cypherpunk-themed emotes
    Hack,         // Mimics typing rapidly on an invisible keyboard
    Encrypt,      // Makes encryption gestures
    Decrypt,      // Makes decryption gestures
    Surveillance, // Looks around suspiciously as if being watched
    Resist,       // Raises fist in defiance
    Ghost,        // Mimics disappearing/becoming anonymous
    DataDrop,     // Pantomimes dropping/transferring data
    Glitch,       // Deliberately glitches/pixelates movements
}

impl EmoteType {
    /// Get a display string for the emote
    #[allow(dead_code)] // Part of complete protocol API for future use
    pub fn display_text(&self) -> &'static str {
        match self {
            // Standard emotes
            EmoteType::Wave => "waves hello",
            EmoteType::Bow => "bows respectfully",
            EmoteType::Laugh => "laughs heartily",
            EmoteType::Dance => "performs a dance",
            EmoteType::Salute => "salutes firmly",
            EmoteType::Shrug => "shrugs shoulders",
            EmoteType::Cheer => "cheers enthusiastically",
            EmoteType::Clap => "claps hands",
            EmoteType::ThumbsUp => "gives a thumbs up",

            // Cypherpunk-themed emotes
            EmoteType::Hack => "simulates frantic hacking",
            EmoteType::Encrypt => "makes encryption gestures",
            EmoteType::Decrypt => "performs decryption movements",
            EmoteType::Surveillance => "looks around suspiciously",
            EmoteType::Resist => "raises fist in digital defiance",
            EmoteType::Ghost => "fades into digital anonymity",
            EmoteType::DataDrop => "mimes a secure data transfer",
            EmoteType::Glitch => "momentarily glitches out",
        }
    }

    /// Get a visual representation of the emote for display
    #[allow(dead_code)] // Part of complete protocol API for future use
    pub fn display_icon(&self) -> &'static str {
        match self {
            // Standard emotes
            EmoteType::Wave => "👋",
            EmoteType::Bow => "🙇",
            EmoteType::Laugh => "😂",
            EmoteType::Dance => "💃",
            EmoteType::Salute => "🫡",
            EmoteType::Shrug => "🤷",
            EmoteType::Cheer => "🎉",
            EmoteType::Clap => "👏",
            EmoteType::ThumbsUp => "👍",

            // Cypherpunk-themed emotes
            EmoteType::Hack => "⌨️",
            EmoteType::Encrypt => "🔒",
            EmoteType::Decrypt => "🔓",
            EmoteType::Surveillance => "👁️",
            EmoteType::Resist => "✊",
            EmoteType::Ghost => "👻",
            EmoteType::DataDrop => "💾",
            EmoteType::Glitch => "📟",
        }
    }

    #[allow(dead_code)]
    pub fn from_str(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            // Standard emotes
            "wave" | "hello" | "hi" => Some(EmoteType::Wave),
            "bow" => Some(EmoteType::Bow),
            "laugh" | "lol" | "haha" => Some(EmoteType::Laugh),
            "dance" => Some(EmoteType::Dance),
            "salute" => Some(EmoteType::Salute),
            "shrug" => Some(EmoteType::Shrug),
            "cheer" => Some(EmoteType::Cheer),
            "clap" | "applaud" => Some(EmoteType::Clap),
            "thumbsup" | "thumbs" | "like" => Some(EmoteType::ThumbsUp),

            // Cypherpunk-themed emotes
            "hack" | "hacking" => Some(EmoteType::Hack),
            "encrypt" | "encryption" => Some(EmoteType::Encrypt),
            "decrypt" | "decryption" => Some(EmoteType::Decrypt),
            "surveillance" | "watched" | "spy" => Some(EmoteType::Surveillance),
            "resist" | "resistance" => Some(EmoteType::Resist),
            "ghost" | "vanish" | "anonymous" => Some(EmoteType::Ghost),
            "datadrop" | "data" | "transfer" => Some(EmoteType::DataDrop),
            "glitch" | "malfunction" => Some(EmoteType::Glitch),

            _ => None,
        }
    }
}
