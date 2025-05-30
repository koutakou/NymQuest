# Security Features

This document outlines the security features implemented in NymQuest to protect player privacy and game integrity.

## Network Privacy

### Nym Mixnet Integration

All communications between clients and the server are routed through the Nym mixnet, providing:
- **Metadata Protection**: Prevents observers from linking messages to their senders
- **Traffic Analysis Resistance**: Obfuscates the timing and volume of communications
- **Decentralized Routing**: Messages travel through multiple nodes to prevent tracing

## Message Authentication

### HMAC-SHA256 Authentication with Forward Secrecy

All messages are cryptographically authenticated using HMAC-SHA256 with automatic key rotation:
- **Integrity Protection**: Ensures messages are not tampered with in transit
- **Origin Validation**: Verifies that messages come from authenticated sources
- **Transparent Verification**: Authentication occurs automatically without user interaction
- **Forward Secrecy**: Regular key rotation prevents past communications from being compromised if a key is exposed

### Key Rotation System

The authentication system implements automatic key rotation:
- **Time-based Rotation**: Keys are automatically rotated every 24 hours
- **Historical Key Retention**: Previous keys are securely maintained for a limited time to verify older messages
- **Timestamp Binding**: Authentication tags are bound to specific key versions using timestamps
- **Seamless Transition**: Key rotation occurs without disrupting active sessions

## Anti-Replay Protection

### Sliding Window Mechanism

To prevent replay attacks, NymQuest implements:
- **Sequence Numbers**: Each message contains a unique, incrementing sequence number
- **Window Tracking**: Server maintains a window of recently seen sequence numbers
- **Automatic Rejection**: Messages with invalid or repeated sequence numbers are discarded
- **Configurable Window Size**: The replay protection window size is configurable via environment variables

### Configurable Replay Protection

The replay protection window size can be adjusted to balance security and resource usage:
- **Client configuration**: Set `NYMQUEST_CLIENT_REPLAY_PROTECTION_WINDOW_SIZE` (default: 64)
- **Server configuration**: Set `NYMQUEST_REPLAY_PROTECTION_WINDOW_SIZE` (default: 64)
- **Valid range**: Values must be between 16 and 128
- **Resource implications**: Larger windows provide better protection against sophisticated replay attacks but consume more memory
- **Runtime adaptability**: Changes take effect on application restart

## DoS Protection & Rate Limiting

NymQuest implements a comprehensive rate limiting system to prevent abuse while maintaining privacy:

### Server-Side Rate Limiting
- **Token bucket algorithm**: Manages message rates per connection without tracking identities
- **Privacy-preserving**: Rate limits are applied per mixnet connection tag, not user identity
- **Configurable limits**: Environment variables control message rates and burst capacity
- **Graceful handling**: Rate-limited clients receive informative error messages
- **Memory efficient**: Automatic cleanup of old rate limiting buckets

### Client-Side Awareness
- **Proactive throttling**: Client maintains its own token bucket (8 msg/sec, 15 burst)
- **Automatic backoff**: Delays sending when approaching limits
- **Seamless user experience**: Rate limiting works transparently

### Security Benefits
- **DoS prevention**: Protects against message flooding attacks
- **Resource conservation**: Prevents server overload from rapid message bursts
- **Fair usage**: Ensures all players have equal access to server resources
- **Anonymity preservation**: No identity tracking in rate limiting implementation

## Message Pacing

The game includes configurable message pacing to enhance privacy by preventing timing correlation attacks:

### Client-Side Message Pacing
- **Controlled delays**: Introduces configurable intervals between message sends
- **Timing obfuscation**: Prevents attackers from correlating actions based on timing patterns
- **Configurable intervals**: Default 100ms, configurable between 1-10000ms
- **Adaptive jitter**: Adds random timing variation (up to 25% by default) to further prevent timing analysis
- **Enabled by default**: Client-side message pacing is now enabled by default for comprehensive protection
- **Enhanced implementation**: Unified pacing mechanism ensures consistent timing obfuscation throughout the message lifecycle

### Server-Side Message Processing Pacing
- **Processing delays**: Introduces controlled intervals between processing messages
- **Pattern disruption**: Further obfuscates timing patterns at the server level
- **Configurable intervals**: Default 100ms, configurable between 1-10000ms
- **Enabled by default**: Server-side pacing is now enabled by default for comprehensive protection

### Privacy Benefits
- **Timing correlation resistance**: Controlled delays prevent attackers from correlating messages by timing
- **Traffic analysis protection**: Reduces patterns that could be used for traffic analysis
- **Configurable trade-offs**: Allows balancing privacy enhancement with responsiveness

## Enhanced Message Padding

NymQuest uses an advanced message padding system to protect against traffic analysis and size correlation attacks:

### Adaptive Size Normalization
- **Dynamic size buckets**: Messages are padded to standardized size buckets (128, 256, 512, 1024, 2048, 4096 bytes)
- **Variable jitter range**: Applies 2-8% size jitter to prevent bucket size fingerprinting
- **Multiple entropy sources**: Utilizes four distinct entropy mechanisms that rotate automatically:
  - Message count-based entropy
  - Time-based entropy
  - Combined entropy (message count + time)
  - True randomness
- **Unpredictable rotation**: Jitter strategies rotate at varying intervals (50-150 messages)
- **Enhanced randomization**: Uses thread-safe random number generation for higher quality padding

### Security Features
- **Size validation**: Enforces maximum message size constraints with proper error handling
- **Efficient implementation**: Minimizes performance impact while maximizing privacy protection
- **Consistent application**: Applied uniformly to both client-to-server and server-to-client communications
- **Adaptive logging**: Reduces log volume for large messages while maintaining visibility

### Privacy Benefits
- **Prevents message type identification**: Makes it difficult to identify message types based on size
- **Thwarts statistical analysis**: Multiple entropy sources and variable rotation intervals resist pattern recognition
- **Machine learning resistance**: Enhanced jitter strategies make ML-based traffic analysis more difficult
- **Cross-platform consistency**: Client and server implementations are synchronized for consistent protection

## Session Integrity

### Connection Management
- **Automatic heartbeat system**: Ensures only active connections are maintained
- **Graceful disconnection**: Proper cleanup when players leave
- **Session validation**: Prevents identity conflicts and session hijacking

### Error Handling
- **Privacy-preserving errors**: Error messages provide information without compromising privacy
- **Secure authentication verification**: Improved error handling for authentication failures
- **Robust input validation**: Prevents malformed messages from affecting the system
