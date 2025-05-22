# NymQuest: Privacy-Focused Multiplayer Game

NymQuest is a privacy-preserving multiplayer game that leverages the Nym mixnet to ensure secure, anonymous communications between players. This innovative game demonstrates the practical application of privacy-enhancing technologies in interactive entertainment. The game features an enhanced terminal UI with intuitive visualizations and improved player experience.

## Project Overview

This project showcases how the Nym network can be used to build privacy-preserving applications beyond traditional financial or messaging use cases. The game features:

- **Private Communications**: All game data is transmitted through the Nym mixnet, preventing network observers from linking players to their actions
- **Enhanced Terminal Interface**: Lightweight client with an improved UI featuring bordered sections, intuitive health bars, distance visualization, and color-coded player statuses for a more engaging gaming experience
- **Real-Time Multiplayer**: Move around a 2D world, chat with other players, and engage in simple combat
- **Anonymous Identity**: Players can create characters without revealing their real identity

## Architecture

The project consists of two main components:

### Server
- Manages the game state and player connections
- Processes player commands (movement, attacks, chat)
- Broadcasts game state updates to all connected players
- Handles player registration and disconnection

### Client
- Connects to the server through the Nym mixnet
- Renders the game state in a terminal interface with colors
- Processes user input for movement and actions
- Displays a mini-map of the game world showing player positions

## Technical Stack

- **Rust**: Core programming language for both client and server
- **Nym SDK**: Privacy infrastructure for anonymous communications
- **Tokio**: Asynchronous runtime for handling concurrent operations
- **Serde**: Serialization/deserialization of game messages
- **Colored**: Terminal text coloring for improved UI

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
   - Move: `/move up`, `/move down`, `/move left`, `/move right`
   - Chat: `/chat Hello everyone!`
   - Attack: `/attack player_display_id` (use the ID in [brackets], not the player name)
   - Disconnect: `/quit`

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

## Combat System

NymQuest features a simple but engaging combat system:

- **Attack Range**: Players can attack others within 28.0 units of distance
- **Cooldown System**: 3-second cooldown between attacks
- **Health**: Players start with 100 health points
- **Damage**: Base attack deals 10 damage points
- **Critical Hits**: 15% chance to land a critical hit doing double damage (20 points)
- **Respawn**: Defeated players respawn with full health at a random position

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


