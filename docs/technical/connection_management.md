# Connection Management & Heartbeat System

NymQuest implements a robust connection management system to ensure game state consistency while preserving privacy.

## Heartbeat System Overview

The heartbeat system is a critical component that:
- Verifies player connections remain active
- Automatically detects and removes inactive players
- Maintains game state consistency
- Preserves privacy throughout the connection monitoring process

## Server-Side Implementation

### Heartbeat Dispatch
- **Periodic Requests**: Server periodically sends heartbeat requests to all connected players
- **Configurable Intervals**: Timing can be adjusted based on network conditions and privacy requirements
- **Automated Processing**: Handled within the server's background task scheduling system

### Inactive Player Detection
- **Timeout Tracking**: Players who don't respond within the configured timeout period are flagged
- **Automatic Cleanup**: Flagged inactive players are automatically removed from the game state
- **Notification**: Other players are informed when a player is removed due to inactivity
- **Resource Reclamation**: Resources associated with inactive players are properly released

## Client-Side Implementation

### Heartbeat Response
- **Automatic Reply**: Clients automatically respond to heartbeat requests without user intervention
- **Transparent Operation**: Heartbeat mechanism operates in the background without disrupting gameplay
- **Reliable Delivery**: Uses the same authenticated message system as other game communications

### Graceful Disconnection
- **Explicit Disconnect**: When players exit using `/quit`, `/exit`, or `/q`, a proper disconnect message is sent to the server
- **Immediate Processing**: Server immediately removes disconnected players from the game state
- **Notification**: Other players are informed when a player disconnects
- **Resource Cleanup**: All resources associated with the disconnected player are properly released

## Privacy Considerations

- **Anonymous Identification**: Heartbeats use the same anonymous sender tag system as other messages
- **Mixnet Routing**: All heartbeat messages are routed through the Nym mixnet for privacy protection
- **Minimal Information**: Heartbeat messages contain only the essential information needed for connection verification
- **Pacing Compatibility**: Heartbeat messages respect message pacing settings when enabled

## Benefits

- **Game State Consistency**: Ensures the game state accurately reflects currently active players
- **Resource Management**: Prevents resource leaks from abandoned connections
- **Enhanced User Experience**: Players see an accurate representation of who is actively in the game
- **Stability**: Contributes to overall system stability by cleaning up stale connections
- **Privacy Preservation**: Maintains anonymity while ensuring accurate player presence information

## Implementation Notes

- Heartbeat messages are small and efficient to minimize network overhead
- The system is designed to be resilient to occasional network issues and temporary connection problems
- Timeouts are set to reasonable values that balance responsiveness with tolerance for network delays
- All heartbeat operations integrate with the status monitoring system for comprehensive connection health tracking
