# NymQuest: Privacy-Focused Multiplayer Game

NymQuest is a privacy-preserving multiplayer game that leverages the Nym mixnet to ensure secure, anonymous communications between players. This innovative game demonstrates the practical application of privacy-enhancing technologies in interactive entertainment. The game features an enhanced terminal UI with intuitive visualizations and improved player experience.

## Project Overview

This project showcases how the Nym network can be used to build privacy-preserving applications beyond traditional financial or messaging use cases. The game features:

- **Private Communications**: All game data is transmitted through the Nym mixnet, preventing network observers from linking players to their actions
- **Enhanced Terminal Interface**: Lightweight client with an improved UI featuring bordered sections, intuitive health bars, distance visualization, and color-coded player statuses for a more engaging gaming experience
- **Real-Time Multiplayer**: Move around a 2D world, chat with other players, use emotes for non-verbal communication, and engage in simple combat
- **Anonymous Identity**: Players can create characters without revealing their real identity
- **Emote System**: Express yourself with visual emotes that enhance social interaction while maintaining privacy
- **Heartbeat System**: Automatic detection and removal of inactive players to maintain game state consistency
- **Graceful Disconnection**: Proper cleanup when players leave the game

## Architecture

The project consists of two main components:

### Server
- Manages the game state and player connections
- Processes player commands (movement, attacks, chat)
- Broadcasts game state updates to all connected players
- Handles player registration and disconnection
- **Background Task Scheduling**: Asynchronous event loop with concurrent task execution for optimal performance
- **Automated Heartbeat Management**: Periodically sends heartbeat requests to maintain active connections
- **Inactive Player Cleanup**: Automatically detects and removes disconnected players to maintain game state consistency
- **Production-Ready Architecture**: Non-blocking concurrent processing ensures server responsiveness under load

### Client
- Connects to the server through the Nym mixnet
- Renders the game state in a terminal interface with colors
- Processes user input for movement and actions
- Displays a mini-map of the game world showing player positions
- **Automatic Heartbeat Responses**: Responds to server heartbeat requests to maintain connection
- **Graceful Disconnection**: Sends proper disconnect message when exiting
- **Real-Time Status Monitoring**: Comprehensive privacy and connection health monitoring system that provides:
  - **Connection Health Indicators**: Real-time monitoring of mixnet connection quality (Excellent, Good, Fair, Poor, Critical)
  - **Privacy Protection Levels**: Visual indicators showing current anonymity status (Fully Protected, Protected, Degraded, Compromised)
  - **Message Delivery Tracking**: Live tracking of message lifecycle from sent to delivered/failed with latency measurements
  - **Network Statistics**: Real-time metrics including average latency, packet loss rates, and success rates
  - **Anonymity Set Monitoring**: Information about the current anonymity set size for privacy awareness
  - **Status Dashboard**: Interactive UI displaying all privacy and connection metrics in an organized dashboard
  - **Privacy-Preserving Metrics**: All monitoring respects privacy principles and doesn't compromise user anonymity

## Technical Stack

- **Rust**: Core programming language for both client and server
- **Nym SDK**: Privacy infrastructure for anonymous communications
- **Tokio**: Asynchronous runtime for handling concurrent operations
- **Serde**: Serialization/deserialization of game messages
- **Colored**: Terminal text coloring for improved UI

## Technical Implementation

### Background Task Scheduling

The server implements a production-ready concurrent event loop using Tokio's `select!` macro to handle multiple asynchronous operations simultaneously:

- **Message Processing**: Handles incoming player messages through the Nym mixnet while maintaining anonymity
- **Heartbeat Management**: Sends periodic heartbeat requests to all connected players at configurable intervals
- **Inactive Player Cleanup**: Automatically removes players who fail to respond to heartbeat requests within the timeout period
- **Game State Persistence**: Automatically saves and recovers game state to ensure continuity across server restarts

This architecture ensures:
- **Non-blocking Operations**: Server remains responsive to new connections and messages
- **Privacy Preservation**: All communications continue to flow through the Nym mixnet
- **Scalable Performance**: Concurrent task execution prevents any single operation from blocking others
- **Production Stability**: Automatic cleanup prevents memory leaks from abandoned connections
- **Data Durability**: Game state persists across server restarts with automatic recovery

### Game State Persistence

The server includes a persistence system that automatically:
- **Saves game state** to disk every 2 minutes while running
- **Creates backups** before starting to prevent data loss
- **Recovers player data** on server restart (players need to reconnect for network security)
- **Validates compatibility** between saved state and current configuration
- **Cleans up stale data** by removing players offline for more than 5 minutes
- **Maintains privacy** by excluding network-sensitive information from saved data

Persistence features:
- **Atomic saves** using temporary files to prevent corruption
- **JSON format** for human-readable state files
- **Configurable storage** location via environment variables
- **Graceful degradation** when persistence is disabled
- **Position validation** to handle world boundary changes

### DoS Protection & Rate Limiting

NymQuest implements a comprehensive rate limiting system to prevent abuse while maintaining privacy:

#### Server-Side Rate Limiting
- **Token bucket algorithm**: Manages message rates per connection without tracking identities
- **Privacy-preserving**: Rate limits are applied per mixnet connection tag, not user identity
- **Configurable limits**: Environment variables control message rates and burst capacity
- **Graceful handling**: Rate-limited clients receive informative error messages
- **Memory efficient**: Automatic cleanup of old rate limiting buckets

#### Rate Limiting Configuration

The server rate limiting can be configured via environment variables:

```bash
# Message rate limit (messages per second per connection)
export NYMQUEST_MESSAGE_RATE_LIMIT=10.0

# Maximum burst size (messages that can be sent rapidly)
export NYMQUEST_MESSAGE_BURST_SIZE=20
```

Default values:
- **Rate limit**: 10 messages per second per connection
- **Burst size**: 20 messages maximum in rapid succession
- **Cleanup interval**: 5 minutes for unused buckets

#### Client-Side Awareness

The client includes rate limiting awareness to prevent hitting server limits:
- **Proactive throttling**: Client maintains its own token bucket (8 msg/sec, 15 burst)
- **Automatic backoff**: Delays sending when approaching limits
- **Seamless user experience**: Rate limiting works transparently

#### Security Benefits

- **DoS prevention**: Protects against message flooding attacks
- **Resource conservation**: Prevents server overload from rapid message bursts
- **Fair usage**: Ensures all players have equal access to server resources
- **Anonymity preservation**: No identity tracking in rate limiting implementation

### Client-Side Status Monitoring

The client includes a comprehensive status monitoring system that provides real-time visibility into connection health and privacy status while maintaining anonymity:

**Core Components:**
- **StatusMonitor Module**: Centralized tracking of connection metrics, message delivery, and privacy indicators
- **Network Integration**: Deep integration with the NetworkManager to monitor all mixnet communications
- **Thread-Safe Architecture**: Uses `Arc<Mutex<StatusMonitor>>` for safe concurrent access across network and UI threads
- **Privacy-Compliant Metrics**: All collected data respects privacy principles and doesn't compromise user anonymity

**Real-Time Tracking:**
- **Message Lifecycle Monitoring**: Tracks messages from send → in-transit → delivered/failed with precise latency measurements
- **Connection Health Assessment**: Evaluates mixnet connection quality based on response times and success rates
- **Privacy Level Indicators**: Monitors anonymity protection status and provides visual feedback
- **Network Statistics**: Calculates rolling averages for latency, packet loss, and delivery success rates
- **Anonymity Set Awareness**: Displays estimated anonymity set size for privacy context

**User Interface Integration:**
- **Status Dashboard**: Real-time display of all privacy and connection metrics
- **Visual Indicators**: Color-coded status indicators for quick assessment
- **Data Freshness Indicators**: Improved timestamp display with appropriate thresholds for mixnet delays
- **Non-Intrusive Design**: Status information enhances rather than disrupts the gaming experience

This monitoring system enhances user awareness while preserving the core privacy guarantees of the Nym mixnet.

### Configuration

All timing and persistence parameters are configurable via environment variables:
- `NYMQUEST_HEARTBEAT_INTERVAL_SECONDS`: Frequency of heartbeat requests (default: 30s)
- `NYMQUEST_HEARTBEAT_TIMEOUT_SECONDS`: Player inactivity timeout (default: 90s)
- `NYMQUEST_ENABLE_PERSISTENCE`: Enable game state persistence (default: true)
- `NYMQUEST_PERSISTENCE_DIR`: Directory for saving game state (default: "./game_data")

## Getting Started

### Prerequisites

- Rust and Cargo (latest stable version)
- Internet connection to access the Nym network

### Installation

1. Clone the repository:
   ```
   git clone https://github.com/koutakou/NymQuest
   cd NymQuest
   ```

2. Build the server:
   ```
   cd server
   cargo build --release
   ```

3. Build the client:
   ```
   cd ../client
   cargo build --release
   ```

### Running the Game

1. Start the server:
   ```
   cd server
   cargo run --release
   ```
   The server will display its Nym address and save it to `client/server_address.txt`.

2. Start the client in a new terminal:
   ```
   cd client
   cargo run --release
   ```

3. In the client, register with a username:
   ```
   /register YourName
   ```

4. Use the following commands in the client:
   - Register: `/register YourName` or `/r YourName`
   - Move: `/move up` (or `/m up`, `/go up`), `/move down`, `/move left`, `/move right`
   - Direct movement: `/up` (or `/u`, `/n`), `/down` (or `/d`, `/s`), `/left` (or `/l`, `/w`), `/right` (or `/r`, `/e`)
   - Diagonal movement: `/ne`, `/nw`, `/se`, `/sw`
   - Chat: `/chat Hello everyone!` or `/c Hello everyone!` or `/say Hello everyone!`
   - Emotes: `/emote wave` or `/em dance` (options: wave, bow, laugh, dance, salute, shrug, cheer, clap)
   - Attack: `/attack player_display_id` or `/a player_display_id` (use the ID in [brackets], not the player name)
   - Help: `/help` or `/h` or `/?`
   - Disconnect: `/quit` or `/exit` or `/q`

## Privacy Benefits

NymQuest demonstrates several key privacy benefits:

1. **Network-Level Privacy**: All game communications are protected by Nym's mixnet, preventing traffic analysis
2. **Metadata Protection**: The timing and frequency of game actions are obfuscated
3. **Anonymous Authentication**: Players can participate without revealing their real identity
4. **Decentralized Architecture**: No central server needs to be trusted with player data
5. **Message Authentication**: All messages are cryptographically authenticated using HMAC-SHA256 to prevent tampering
6. **Enhanced Display IDs**: Players are assigned randomized display IDs using a word-number combination (e.g., Warrior123) rather than sequential numbering to improve anonymity
7. **Secure Authentication Verification**: Improved error handling for authentication failures with privacy-preserving error messages
8. **Protection Against Replay Attacks**: Messages are verified with sequence numbers and authentication tags to prevent replay attacks
9. **Session Integrity Protection**: Prevents identity conflicts by requiring clients to disconnect before registering again, maintaining the integrity of user sessions
10. **Heartbeat System**: Automatic detection of inactive players preserves privacy by preventing stale player data from remaining visible

## Combat System

NymQuest features a simple but engaging combat system:

- **Attack Range**: Players can attack others within 28.0 units of distance
- **Cooldown System**: 3-second cooldown between attacks
- **Health**: Players start with 100 health points
- **Damage**: Base attack deals 10 damage points
- **Critical Hits**: 15% chance to land a critical hit doing double damage (20 points)
- **Respawn**: Defeated players respawn with full health at a random position

## Connection Management & Heartbeat System

NymQuest implements a robust connection management system to ensure game state consistency:

- **Heartbeat Monitoring**: Server periodically sends heartbeat requests to all connected players
- **Automatic Response**: Clients automatically respond to heartbeat requests to maintain their connection
- **Inactive Player Detection**: Players who don't respond to heartbeat requests within the timeout period are automatically removed
- **Graceful Disconnection**: When players exit using `/quit`, `/exit`, or `/q`, a proper disconnect message is sent to the server
- **Immediate Cleanup**: Disconnected or inactive players are immediately removed from the game state and other players are notified
- **Privacy Preservation**: The heartbeat system maintains anonymity while ensuring accurate player presence information

## Future Roadmap

- Expanded game world with environmental features
- Enhanced combat system with items and abilities
- Persistent player progression
- Web-based client interface
- Mobile client support

## Screenshot
Server
<img width="1036" alt="image" src="https://github.com/user-attachments/assets/50db5ee3-9a82-44d1-befc-8b5c0665e1b8" />

Client 1
<img width="1022" alt="image" src="https://github.com/user-attachments/assets/6c5989fb-2a9a-4bd3-aa21-68447115deb5" />

Client 2
<img width="1022" alt="image" src="https://github.com/user-attachments/assets/ae1ce486-3695-4fe2-8957-ec00f1b60dc4" />
