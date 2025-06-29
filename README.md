# NymQuest: Privacy-Focused Multiplayer Game in a Cypherpunk World

NymQuest is a privacy-preserving multiplayer game set in a rich cypherpunk world that leverages the Nym mixnet to ensure secure, anonymous communications between players. This innovative game demonstrates the practical application of privacy-enhancing technologies in interactive entertainment while immersing players in a dystopian future where privacy is both scarce and valuable.

## Quick Start

1. **Prerequisites**: Rust and Cargo (latest stable version)
2. **Build and Run**:
   ```bash
   # Build and run server
   cd server
   cargo run --release
   
   # In a new terminal, build and run client
   cd ../client
   cargo run --release
   
   # Register in the client
   /register YourName
   ```
### Server Discovery

The client automatically discovers the server using a cross-platform mechanism:

- **Automatic Discovery**: Server saves connection info to platform-specific data directories
- **Custom Location**: Set `NYMQUEST_SERVER_ADDRESS_FILE=/path/to/file` environment variable
- **Cross-Platform**: Works on Linux (XDG), Windows (AppData), and macOS
- **Production Ready**: No hardcoded paths, works regardless of binary locations

## Releases

### Download Pre-built Binaries

Pre-built binaries for all supported platforms are available from the [GitHub Releases](../../releases) page:

- **Linux (x86_64)**: `nymquest-linux-x86_64.tar.gz`
- **Windows (x86_64)**: `nymquest-windows-x86_64.zip`  
- **macOS (Intel)**: `nymquest-macos-x86_64.tar.gz`
- **macOS (Apple Silicon)**: `nymquest-macos-aarch64.tar.gz`

Each release includes both the server and client binaries along with documentation.

### Building from Source

For detailed build instructions including cross-platform compilation, see [BUILD.md](./BUILD.md).

## Key Features

- **Cypherpunk World**: Rich dystopian setting with factions, regions, and thematic elements
- **Private Communications**: All game data transmitted through the Nym mixnet
- **Real-Time Multiplayer**: Move around a 2D world, chat, use emotes, and engage in combat
- **Enhanced Terminal Interface**: Intuitive UI with health bars and visual indicators
- **Status Monitoring**: Real-time connection health and privacy protection level indicators
- **Message Pacing**: Configurable delays to prevent timing correlation attacks
- **Regional Security Levels**: Different areas of the game world feature varying levels of surveillance and security
- **Cryptographic Items**: Collect and use cypherpunk-themed items with gameplay effects

## Documentation

Comprehensive documentation is available in the [`docs/`](./docs/) directory:

- **[Overview](./docs/overview.md)**: Project overview, architecture, and key features
- **Guides**:
  - **[Installation Guide](./docs/guides/installation.md)**: How to install and run NymQuest
  - **[User Guide](./docs/guides/user_guide.md)**: How to play the game and use commands
- **Features**:
  - **[Combat System](./docs/features/combat.md)**: Details about the combat mechanics
  - **[Movement System](./docs/features/movement.md)**: How player movement works
  - **[Privacy Features](./docs/features/privacy.md)**: Privacy benefits and implementations
  - **[Communication System](./docs/features/communication.md)**: Chat and emote systems
  - **[Worldbuilding](./docs/worldbuilding.md)**: Cypherpunk theme, factions, regions, and lore
- **Technical Documentation**:
  - **[Architecture](./docs/technical/architecture.md)**: System architecture and components
  - **[Protocol](./docs/technical/protocol.md)**: Communication protocol and versioning
  - **[Security](./docs/technical/security.md)**: Security features and implementations
  - **[Status Monitoring](./docs/technical/status_monitoring.md)**: Connection health and privacy monitoring
  - **[Message Pacing](./docs/technical/message_pacing.md)**: Timing correlation attack prevention
  - **[Connection Management](./docs/technical/connection_management.md)**: Heartbeat system and connection handling
  - **[State Persistence](./docs/technical/state_persistence.md)**: Game state saving and recovery
  - **[Mixnet Health Monitoring](./docs/technical/mixnet_health_monitoring.md)**: Mixnet connection health monitoring
  - **[Mixnet-Cypherpunk Integration](./docs/technical/mixnet_cypherpunk_integration.md)**: Integration of mixnet with cypherpunk worldbuilding

## Technology Stack

- **Rust**: Core programming language for both client and server
- **Nym SDK**: Privacy infrastructure for anonymous communications
- **Tokio**: Asynchronous runtime for handling concurrent operations
- **Serde**: Serialization/deserialization of game messages

## Screenshots

![Server Screenshot](https://github.com/user-attachments/assets/50db5ee3-9a82-44d1-befc-8b5c0665e1b8)
![Client 1 Screenshot](https://github.com/user-attachments/assets/6c5989fb-2a9a-4bd3-aa21-68447115deb5)
![Client 2 Screenshot](https://github.com/user-attachments/assets/ae1ce486-3695-4fe2-8957-ec00f1b60dc4)
