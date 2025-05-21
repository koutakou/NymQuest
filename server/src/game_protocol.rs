use serde::{Deserialize, Serialize};
use std::collections::HashMap;

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
    pub id: String,
    pub position: Position,
    pub health: u32,
    pub name: String,
    pub last_attack_time: u64,
}

// Message types that the client can send to the server
#[derive(Debug, Serialize, Deserialize)]
pub enum ClientMessage {
    // Message to register in the game
    Register { name: String },
    // Message to move in the game world
    Move { direction: Direction },
    // Message to attack another player
    Attack { target_id: String },
    // Message to send chat to all players
    Chat { message: String },
    // Message to leave the game
    Disconnect,
}

// Message types that the server can send to the client
#[derive(Debug, Serialize, Deserialize)]
pub enum ServerMessage {
    // Confirms registration and provides the player ID
    RegisterAck { player_id: String },
    // Game state update
    GameState { 
        players: HashMap<String, Player>,
        // We could add other game elements here
    },
    // Event notification (attack, etc.)
    Event { message: String },
    // Chat message from another player
    ChatMessage { sender_name: String, message: String },
    // Error message
    Error { message: String },
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
