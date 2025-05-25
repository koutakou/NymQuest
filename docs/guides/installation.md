# Installation Guide

This guide provides instructions for installing and running NymQuest.

## Prerequisites

- Rust and Cargo (latest stable version)
- Internet connection to access the Nym network

## Installation

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

## Running the Game

1. Start the server:
   ```
   cd server
   cargo run --release
   ```
   The server will display its Nym address and save connection info to the platform-specific data directory.

2. Start the client in a new terminal:
   ```
   cd client
   cargo run --release
   ```
   The client will automatically discover the server using the cross-platform discovery mechanism.

3. In the client, register with a username:
   ```
   /register YourName
   ```

## Server Discovery

The client uses a robust discovery mechanism to find the server:

### Automatic Discovery (Recommended)

The server automatically saves connection information to platform-specific directories:

- **Linux**: `~/.local/share/nymquest/server/nymquest_server.addr`
- **Windows**: `%APPDATA%\nymquest\server\nymquest_server.addr`
- **macOS**: `~/Library/Application Support/nymquest/server/nymquest_server.addr`

### Custom Server Location

For advanced deployments, you can specify a custom server address file location:

```bash
# Set custom location for server address file
export NYMQUEST_SERVER_ADDRESS_FILE=/path/to/custom/server_address.txt

# Start the server (will save to custom location)
cd server
cargo run --release

# Start the client (will find server at custom location)
cd client
cargo run --release
```

### Discovery Priority

The client searches for server connection information in the following order:

1. Custom location specified by `NYMQUEST_SERVER_ADDRESS_FILE` environment variable
2. Platform-specific data directory (recommended)
3. Current working directory (`nymquest_server.addr`)
4. Current working directory (`server_address.txt` - legacy compatibility)
5. Legacy relative paths (for backward compatibility)
6. Home directory fallback

## Configuration Options

### Rate Limiting Configuration

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

### Message Pacing Configuration

The game includes configurable message pacing to enhance privacy by preventing timing correlation attacks:

**Client Configuration:**
```bash
# Enable message pacing for privacy protection (default: false)
export NYMQUEST_CLIENT_ENABLE_MESSAGE_PACING=true

# Minimum interval between message sends in milliseconds (default: 100ms)
export NYMQUEST_CLIENT_MESSAGE_PACING_INTERVAL_MS=100
```

**Server Configuration:**
```bash
# Enable message processing pacing for privacy protection (default: false)
export NYMQUEST_ENABLE_MESSAGE_PROCESSING_PACING=true

# Minimum interval between processing messages in milliseconds (default: 100ms)
export NYMQUEST_MESSAGE_PROCESSING_INTERVAL_MS=100
```

By default, message pacing is disabled to maintain game responsiveness. Enable it when enhanced privacy is required.
