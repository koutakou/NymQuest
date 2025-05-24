# System Architecture

This document describes the architecture of the NymQuest system, including both the client and server components.

## Overall Architecture

NymQuest consists of two main components that communicate via the Nym mixnet:

1. **Server**: Manages game state and player connections
2. **Client**: Provides user interface and player input handling

All communication between clients and the server is routed through the Nym mixnet to ensure privacy and metadata protection.

## Server Architecture

The server is built with a production-ready architecture designed for reliability, scalability, and privacy:

### Core Components

- **Game State Manager**: Maintains the current state of the game world and players
- **Player Manager**: Handles player connections, registration, and authentication
- **Message Processor**: Processes incoming messages from clients
- **Network Layer**: Manages communication with the Nym mixnet

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

## Client Architecture

The client is designed to provide an engaging user experience while maintaining privacy:

### Core Components

- **User Interface**: Terminal-based UI with intuitive controls and visual elements
- **Input Handler**: Processes user commands and translates them to game actions
- **Game State Renderer**: Visualizes the game state in the terminal
- **Network Manager**: Handles communication with the server through the Nym mixnet
- **Status Monitor**: Tracks connection health and privacy metrics

### Key Features

- **Enhanced Terminal Interface**: Features bordered sections, intuitive health bars, and color-coded statuses
- **Automatic Heartbeat Responses**: Responds to server heartbeat requests to maintain connection
- **Graceful Disconnection**: Sends proper disconnect message when exiting
- **Real-Time Status Monitoring**: Comprehensive privacy and connection health monitoring system

## Technical Stack

- **Rust**: Core programming language for both client and server
- **Nym SDK**: Privacy infrastructure for anonymous communications
- **Tokio**: Asynchronous runtime for handling concurrent operations
- **Serde**: Serialization/deserialization of game messages
- **Colored**: Terminal text coloring for improved UI

## Data Flow

1. **Client Input**: User enters commands in the client terminal
2. **Message Creation**: Client converts commands to protocol messages
3. **Privacy Routing**: Messages are sent through the Nym mixnet
4. **Server Processing**: Server receives and processes messages
5. **State Update**: Game state is updated based on message contents
6. **Broadcast**: Updates are broadcast to all connected clients
7. **Client Rendering**: Clients update their display based on received state

This architecture ensures that all communications are privacy-protected while maintaining an engaging and responsive game experience.
