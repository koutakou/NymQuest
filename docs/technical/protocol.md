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

To prevent replay attacks, the protocol implements a sliding window mechanism:

1. Each message includes a sequence number
2. The server maintains a window of recently seen sequence numbers
3. Messages with sequence numbers outside the window or already seen are rejected
4. The window advances as new valid messages are received

This system ensures that captured messages cannot be replayed by an attacker.
