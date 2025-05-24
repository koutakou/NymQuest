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
- **Real-Time Status Monitoring**: Comprehensive privacy and connection health monitoring system

## Technical Stack

- **Rust**: Core programming language for both client and server
- **Nym SDK**: Privacy infrastructure for anonymous communications
- **Tokio**: Asynchronous runtime for handling concurrent operations
- **Serde**: Serialization/deserialization of game messages
- **Colored**: Terminal text coloring for improved UI

## Future Roadmap

- Expanded game world with environmental features
- Enhanced combat system with items and abilities
- Persistent player progression
- Web-based client interface
- Mobile client support
