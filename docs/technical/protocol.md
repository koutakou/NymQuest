# Protocol Documentation

This document describes the communication protocol used in NymQuest.

## Protocol Overview

NymQuest uses a custom message protocol for all communications between clients and the server. All messages are transmitted through the Nym mixnet to ensure privacy and metadata protection.

## Protocol Versioning System

NymQuest implements a robust protocol versioning system that enables backward compatibility and smooth upgrades:

### Version Negotiation Process
1. **Client Connection**: When registering, clients send their supported protocol version range
2. **Server Compatibility Check**: Server validates client version against its own supported range
3. **Version Selection**: If compatible, the highest common version is negotiated
4. **Session Establishment**: Both client and server use the negotiated version for the session

### Compatibility Rules
- **Current Version**: The latest protocol version with all features enabled
- **Minimum Supported**: The oldest version that can still be served
- **Negotiation**: Uses the lower of both current versions if ranges overlap
- **Rejection**: Incompatible clients receive clear error messages

### Version Evolution
```rust
// Protocol version constants
pub const PROTOCOL_VERSION: u16 = 1;        // Current version
pub const MIN_SUPPORTED_VERSION: u16 = 1;   // Minimum supported
```

This system ensures that:
- New features can be added without breaking existing clients
- Legacy clients continue to work during gradual upgrades
- Clear compatibility feedback prevents connection issues
- Future protocol evolution is supported from day one

## Data Structures

### Player Structure

The Player structure contains all essential information about a player:

```rust
public struct Player {
    pub id: String,         // Internal server ID (UUID) - not exposed to other clients
    pub display_id: String, // Public privacy-preserving identifier (e.g. "Player1")
    pub position: Position, // Player position in the game world
    pub health: u32,        // Current health points
    pub name: String,       // Player-chosen name
    pub last_attack_time: u64, // Timestamp of the last attack (for cooldown)
    pub experience: u32,    // Experience points earned through gameplay
    pub level: u8,          // Player level based on experience
}
```

## Message Types

The protocol supports the following message types:

### Registration and Authentication
- **Register**: Client requests to join the game with a username
- **RegisterResponse**: Server confirms registration and provides player information
- **Disconnect**: Client notifies server of disconnection

### Game Actions
- **Move**: Client requests to move in a specified direction (see [Movement System](../features/movement.md))
- **Attack**: Client requests to attack another player
- **Chat**: Client sends a chat message
- **Emote**: Client performs an emote action

### System Messages
- **Heartbeat**: Server checks if client is still connected
- **HeartbeatResponse**: Client confirms it is still connected
- **GameState**: Server broadcasts the current game state to all clients
- **ErrorMessage**: Server notifies client of an error condition

## Message Authentication

All messages are authenticated using HMAC-SHA256 to ensure integrity and prevent tampering. The authentication process works as follows:

1. A message is created with all required fields
2. An HMAC-SHA256 signature is generated using the shared secret
3. The signature is attached to the message
4. The receiver verifies the signature before processing the message
5. Messages with invalid signatures are rejected

## Replay Protection

To prevent replay attacks, the protocol implements a robust sliding window mechanism on both client and server:

1. Each message includes a unique sequence number
2. Both client and server maintain a window of recently seen sequence numbers
3. Messages with sequence numbers outside the window or already seen are rejected
4. The window advances as new valid messages are received
5. The system implements a 128-bit bitmap for efficient tracking of out-of-order messages
6. The window size is configurable, with a default of 64 sequence numbers

This bidirectional replay protection ensures that captured messages cannot be replayed by an attacker in either direction, protecting both client and server from replay attacks.

## Message Pacing

To enhance privacy and prevent timing correlation attacks, the protocol implements a sophisticated message pacing system:

1. Messages are sent with configurable time intervals between them
2. Random jitter is added to the intervals to prevent predictable timing patterns
3. The client supports adjustable pacing intervals to balance privacy and responsiveness
4. The pacing system includes:
   - Base interval: Configurable minimum delay between messages
   - Random jitter: Variable delay added to the base interval
   - Monitoring: Real-time visualization of pacing effects

This message pacing system helps prevent traffic analysis and timing correlation attacks that could compromise user privacy, even when using the Nym mixnet's existing protections.
