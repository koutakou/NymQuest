# Message Pacing System

The message pacing system is a critical privacy enhancement in NymQuest designed to reduce timing correlation attack vulnerabilities and enhance overall anonymity protection.

## Overview

Message pacing introduces controlled delays between message operations to prevent timing correlation attacks, which could otherwise be used to deanonymize players by analyzing message timing patterns.

## Client-Side Implementation

### Configuration Options
- **enable_message_pacing**: Boolean flag to enable/disable client-side pacing (default: false)
- **message_pacing_interval_ms**: Milliseconds between message sends (default: 100ms, valid range: 1-10000ms)

### Implementation Details
- **NetworkManager Integration**: The pacing logic is implemented in the `NetworkManager.send_message()` function
- **Timing Control**: Introduces a controlled delay between consecutive message sends
- **Last Message Tracking**: Uses `last_message_sent` timestamp to measure elapsed time since last message
- **Configurable Intervals**: Delay intervals can be adjusted based on privacy needs vs. responsiveness requirements

### Example Configuration
```bash
# Enable message pacing for privacy protection
export NYMQUEST_CLIENT_ENABLE_MESSAGE_PACING=true

# Set the minimum interval between message sends in milliseconds
export NYMQUEST_CLIENT_MESSAGE_PACING_INTERVAL_MS=100
```

## Server-Side Implementation

### Configuration Options
- **enable_message_processing_pacing**: Boolean flag to enable/disable server-side pacing (default: false)
- **message_processing_interval_ms**: Milliseconds between processing messages (default: 100ms, valid range: 1-10000ms)

### Implementation Details
- **Message Processing Integration**: Pacing logic is implemented in the `process_incoming_message()` function
- **Controlled Processing**: Introduces delays between processing consecutive messages
- **Last Message Tracking**: Uses `last_message_processed` timestamp in the main event loop
- **Validation**: Ensures configured intervals are within reasonable bounds (1-10000ms)

### Example Configuration
```bash
# Enable message processing pacing for privacy protection
export NYMQUEST_ENABLE_MESSAGE_PROCESSING_PACING=true

# Set the minimum interval between processing messages in milliseconds
export NYMQUEST_MESSAGE_PROCESSING_INTERVAL_MS=100
```

## Privacy Benefits

The message pacing system provides several important privacy benefits:

- **Timing Correlation Resistance**: By introducing controlled delays, the system prevents attackers from correlating messages based on their timing patterns
- **Traffic Analysis Protection**: Reduces the effectiveness of traffic analysis attacks that rely on message timing
- **Configurable Privacy Levels**: Allows users to balance privacy enhancement with game responsiveness based on their needs
- **Dual-Layer Protection**: Both client and server pacing combine to provide enhanced protection

## Performance Considerations

- **Default Settings**: Both client and server pacing are disabled by default to maintain game responsiveness
- **Recommended Usage**: Enable pacing when enhanced privacy is needed or when playing in potentially hostile network environments
- **Latency Impact**: Higher pacing intervals increase overall latency and may impact game responsiveness
- **Balanced Configuration**: The default 100ms interval provides a good balance between privacy and usability

## Implementation Notes

- Pacing is applied independently on both client and server sides
- Validation ensures intervals remain within reasonable bounds
- The system degrades gracefully if only client or server pacing is enabled
- All pacing operations respect the overall privacy principles of NymQuest
