# NymQuest: Privacy-Focused Multiplayer Game

NymQuest is a privacy-preserving multiplayer game that leverages the Nym mixnet to ensure secure, anonymous communications between players. This innovative game demonstrates the practical application of privacy-enhancing technologies in interactive entertainment.

## Project Overview

This project showcases how the Nym network can be used to build privacy-preserving applications beyond traditional financial or messaging use cases. The game features:

- **Private Communications**: All game data is transmitted through the Nym mixnet, preventing network observers from linking players to their actions
- **Terminal-Based Interface**: Lightweight client with colored UI for an engaging gaming experience
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
   git clone https://github.com/yourusername/nym-mmorpg.git
   cd nym-mmorpg
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
   - Attack: `/attack player_id`
   - Disconnect: `/quit`

## Privacy Benefits

NYM-MMORPG demonstrates several key privacy benefits:

1. **Network-Level Privacy**: All game communications are protected by Nym's mixnet, preventing traffic analysis
2. **Metadata Protection**: The timing and frequency of game actions are obfuscated
3. **Anonymous Authentication**: Players can participate without revealing their real identity
4. **Decentralized Architecture**: No central server needs to be trusted with player data

## Future Roadmap

- Expanded game world with environmental features
- Enhanced combat system with items and abilities
- Persistent player progression
- Web-based client interface
- Mobile client support
