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
