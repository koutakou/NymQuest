use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::time::Duration;

// Player's position in the game world
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
pub struct Position {
    pub x: f32,
    pub y: f32,
}

impl Position {
    // Create a new position from coordinates
    pub fn new(x: f32, y: f32) -> Self {
        Self { x, y }
    }
    
    // Add a movement vector to this position
    pub fn apply_movement(&mut self, move_vector: (f32, f32), speed: f32) {
        self.x += move_vector.0 * speed;
        self.y += move_vector.1 * speed;
        
        // Clamp the position to world boundaries
        self.x = self.x.clamp(-100.0, 100.0);
        self.y = self.y.clamp(-100.0, 100.0);
    }
    
    // Calculate distance to another position
    pub fn distance_to(&self, other: &Position) -> f32 {
        let dx = self.x - other.x;
        let dy = self.y - other.y;
        (dx * dx + dy * dy).sqrt()
    }
}

// Player representation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Player {
    pub id: String,          // Internal server ID (UUID) - not exposed to other clients
    pub display_id: String,  // Public privacy-preserving identifier (e.g. "Player1")
    pub position: Position,
    pub health: u32,
    pub name: String,
    pub last_attack_time: u64,
}

// Message types that the client can send to the server
#[derive(Debug, Serialize, Deserialize)]
pub enum ClientMessage {
    // Message to register in the game
    Register { name: String, seq_num: u64 },
    // Message to move in the game world
    Move { direction: Direction, seq_num: u64 },
    // Message to attack another player (using display_id for privacy)
    Attack { target_display_id: String, seq_num: u64 },
    // Message to send chat to all players
    Chat { message: String, seq_num: u64 },
    // Message to leave the game
    Disconnect { seq_num: u64 },
    // Acknowledge receipt of a server message
    Ack { server_seq_num: u64, original_type: ServerMessageType },
}

// Type of server message (used for acknowledgements)
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
pub enum ServerMessageType {
    RegisterAck,
    GameState,
    Event,
    ChatMessage,
    Error,
    Ack,
}

// Message types that the server can send to the client
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ServerMessage {
    // Confirms registration and provides the player ID
    RegisterAck { player_id: String, seq_num: u64 },
    // Game state update
    GameState { 
        players: HashMap<String, Player>,
        seq_num: u64,
        // We could add other game elements here
    },
    // Event notification (attack, etc.)
    Event { message: String, seq_num: u64 },
    // Chat message from another player
    ChatMessage { sender_name: String, message: String, seq_num: u64 },
    // Error message
    Error { message: String, seq_num: u64 },
    // Acknowledge receipt of a client message
    Ack { client_seq_num: u64, original_type: ClientMessageType },
}

// Type of client message (used for acknowledgements)
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
pub enum ClientMessageType {
    Register,
    Move,
    Attack,
    Chat,
    Disconnect,
    Ack,
}

impl ServerMessage {
    // Get the message type
    pub fn get_type(&self) -> ServerMessageType {
        match self {
            ServerMessage::RegisterAck { .. } => ServerMessageType::RegisterAck,
            ServerMessage::GameState { .. } => ServerMessageType::GameState,
            ServerMessage::Event { .. } => ServerMessageType::Event,
            ServerMessage::ChatMessage { .. } => ServerMessageType::ChatMessage,
            ServerMessage::Error { .. } => ServerMessageType::Error,
            ServerMessage::Ack { .. } => ServerMessageType::Ack,
        }
    }
    
    // Get the sequence number
    pub fn get_seq_num(&self) -> u64 {
        match self {
            ServerMessage::RegisterAck { seq_num, .. } => *seq_num,
            ServerMessage::GameState { seq_num, .. } => *seq_num,
            ServerMessage::Event { seq_num, .. } => *seq_num,
            ServerMessage::ChatMessage { seq_num, .. } => *seq_num,
            ServerMessage::Error { seq_num, .. } => *seq_num,
            ServerMessage::Ack { client_seq_num, .. } => *client_seq_num,
        }
    }
}

impl ClientMessage {
    // Get the message type
    pub fn get_type(&self) -> ClientMessageType {
        match self {
            ClientMessage::Register { .. } => ClientMessageType::Register,
            ClientMessage::Move { .. } => ClientMessageType::Move,
            ClientMessage::Attack { .. } => ClientMessageType::Attack,
            ClientMessage::Chat { .. } => ClientMessageType::Chat,
            ClientMessage::Disconnect { .. } => ClientMessageType::Disconnect,
            ClientMessage::Ack { .. } => ClientMessageType::Ack,
        }
    }
    
    // Get the sequence number
    pub fn get_seq_num(&self) -> u64 {
        match self {
            ClientMessage::Register { seq_num, .. } => *seq_num,
            ClientMessage::Move { seq_num, .. } => *seq_num,
            ClientMessage::Attack { seq_num, .. } => *seq_num,
            ClientMessage::Chat { seq_num, .. } => *seq_num,
            ClientMessage::Disconnect { seq_num, .. } => *seq_num,
            ClientMessage::Ack { server_seq_num, .. } => *server_seq_num,
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
    // Converts a direction to a movement vector
    pub fn to_vector(&self) -> (f32, f32) {
        match self {
            Direction::Up => (0.0, -1.0),
            Direction::Down => (0.0, 1.0),
            Direction::Left => (-1.0, 0.0),
            Direction::Right => (1.0, 0.0),
            Direction::UpLeft => (-0.7, -0.7),    // Normalized diagonal vectors
            Direction::UpRight => (0.7, -0.7),
            Direction::DownLeft => (-0.7, 0.7),
            Direction::DownRight => (0.7, 0.7),
        }
    }
    
    // Parse a direction from a string
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
