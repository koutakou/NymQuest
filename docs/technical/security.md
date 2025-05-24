# Security Features

This document outlines the security features implemented in NymQuest to protect player privacy and game integrity.

## Network Privacy

### Nym Mixnet Integration

All communications between clients and the server are routed through the Nym mixnet, providing:
- **Metadata Protection**: Prevents observers from linking messages to their senders
- **Traffic Analysis Resistance**: Obfuscates the timing and volume of communications
- **Decentralized Routing**: Messages travel through multiple nodes to prevent tracing

## Message Authentication

### HMAC-SHA256 Authentication

All messages are cryptographically authenticated using HMAC-SHA256:
- **Integrity Protection**: Ensures messages are not tampered with in transit
- **Origin Validation**: Verifies that messages come from authenticated sources
- **Transparent Verification**: Authentication occurs automatically without user interaction

## Anti-Replay Protection

### Sliding Window Mechanism

To prevent replay attacks, NymQuest implements:
- **Sequence Numbers**: Each message contains a unique, incrementing sequence number
- **Window Tracking**: Server maintains a window of recently seen sequence numbers
- **Automatic Rejection**: Messages with invalid or repeated sequence numbers are discarded

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

### Server-Side Message Processing Pacing
- **Processing delays**: Introduces controlled intervals between processing messages
- **Pattern disruption**: Further obfuscates timing patterns at the server level
- **Configurable intervals**: Default 100ms, configurable between 1-10000ms

### Privacy Benefits
- **Timing correlation resistance**: Controlled delays prevent attackers from correlating messages by timing
- **Traffic analysis protection**: Reduces patterns that could be used for traffic analysis
- **Configurable trade-offs**: Allows balancing privacy enhancement with responsiveness

## Session Integrity

### Connection Management
- **Automatic heartbeat system**: Ensures only active connections are maintained
- **Graceful disconnection**: Proper cleanup when players leave
- **Session validation**: Prevents identity conflicts and session hijacking

### Error Handling
- **Privacy-preserving errors**: Error messages provide information without compromising privacy
- **Secure authentication verification**: Improved error handling for authentication failures
- **Robust input validation**: Prevents malformed messages from affecting the system
