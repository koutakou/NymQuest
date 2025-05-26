# Privacy Features

NymQuest is designed with privacy as a core principle. This document outlines the privacy features implemented in the game.

## Network-Level Privacy

- **Nym Mixnet Integration**: All game communications are routed through the Nym network for metadata privacy
- **Decentralized Architecture**: No central server needs to be trusted with player data
- **Metadata Protection**: The timing and frequency of game actions are obfuscated

## Anonymity Features

- **Anonymous Authentication**: Players can participate without revealing their real identity
- **Anonymous Player Identification**: Uses anonymous sender tags for player tracking
- **Enhanced Display IDs**: Players are assigned randomized display IDs using a word-number combination (e.g., Warrior123) rather than sequential numbering to improve anonymity

## Message Pacing

A key privacy enhancement is the configurable message pacing system that was implemented to reduce timing correlation attack vulnerabilities:

- **Client-side Message Pacing**: 
  - Introduces controlled delays between message sends
  - Configurable interval (default: 100ms)
  - Can be enabled/disabled via environment variables

- **Server-side Message Processing Pacing**:
  - Introduces controlled delays between message processing
  - Configurable interval (default: 100ms)
  - Configurable jitter percentage (default: 25%)
  - Adds randomized timing variation to further prevent timing correlation attacks
  - Can be enabled/disabled via environment variables

- **Privacy Benefits**:
  - **Timing Correlation Resistance**: Controlled delays prevent attackers from correlating messages by timing
  - **Traffic Analysis Protection**: Reduces patterns that could be used for traffic analysis
  - **Configurable Trade-offs**: Allows balancing privacy enhancement with responsiveness

## Security Measures

- **Message Authentication**: All messages are cryptographically authenticated using HMAC-SHA256 to prevent tampering
- **Adaptive Replay Protection**: 
  - Messages are verified with sequence numbers and authentication tags to prevent replay attacks
  - Dynamic window sizing that automatically adjusts based on network conditions
  - Larger windows for networks with high out-of-order message rates
  - Smaller windows for optimal performance on reliable networks
  - Privacy-preserving adaptation based on connection patterns, not user identity
- **Session Integrity Protection**: Prevents identity conflicts by requiring clients to disconnect before registering again

## Privacy-Preserving Rate Limiting

- **Token Bucket Algorithm**: Manages message rates per connection without tracking identities
- **Privacy-preserving**: Rate limits are applied per mixnet connection tag, not user identity
- **Anonymity Preservation**: No identity tracking in rate limiting implementation

## Privacy-Aware Monitoring

- **Privacy-Compliant Metrics**: All collected data respects privacy principles and doesn't compromise user anonymity
- **Anonymity Set Awareness**: Displays estimated anonymity set size for privacy context
- **Privacy Level Indicators**: Monitors anonymity protection status and provides visual feedback

## Configuration Options

To enhance privacy protection through message pacing:

**Client Configuration:**
```bash
# Enable message pacing for privacy protection (default: false)
export NYMQUEST_CLIENT_ENABLE_MESSAGE_PACING=true

# Minimum interval between message sends in milliseconds (default: 100ms)
export NYMQUEST_CLIENT_MESSAGE_PACING_INTERVAL_MS=100
```

**Server Configuration:**
```bash
# Enable message processing pacing for privacy protection (default: true)
export NYMQUEST_ENABLE_MESSAGE_PROCESSING_PACING=true

# Minimum interval between processing messages in milliseconds (default: 100ms)
export NYMQUEST_MESSAGE_PROCESSING_INTERVAL_MS=100

# Jitter percentage to apply to message processing (0-100) (default: 25)
export NYMQUEST_MESSAGE_PROCESSING_JITTER_PERCENT=25

# Replay protection window size (default: 64)
export NYMQUEST_REPLAY_PROTECTION_WINDOW_SIZE=64

# Enable adaptive replay protection window sizing (default: true)
export NYMQUEST_REPLAY_PROTECTION_ADAPTIVE=true

# Minimum window size for adaptive replay protection (default: 32)
export NYMQUEST_REPLAY_PROTECTION_MIN_WINDOW=32

# Maximum window size for adaptive replay protection (default: 96)
export NYMQUEST_REPLAY_PROTECTION_MAX_WINDOW=96

# Cooldown period in seconds between window size adjustments (default: 60)
export NYMQUEST_REPLAY_PROTECTION_ADJUSTMENT_COOLDOWN=60
```

By default, message pacing is disabled to maintain game responsiveness but can be enabled when enhanced privacy is required.
