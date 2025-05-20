use serde::{Deserialize, Serialize};
use std::collections::HashMap;

// Player's position in the game world
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
pub struct Position {
    pub x: f32,
    pub y: f32,
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
}

impl Direction {
    // Converts a direction to a movement vector
    pub fn to_vector(&self) -> (f32, f32) {
        match self {
            Direction::Up => (0.0, -1.0),
            Direction::Down => (0.0, 1.0),
            Direction::Left => (-1.0, 0.0),
            Direction::Right => (1.0, 0.0),
        }
    }
}
