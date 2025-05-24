# Game State Persistence

NymQuest includes a robust persistence system that allows the game state to be saved and recovered, ensuring continuity across server restarts.

## Overview

The server persistence system automatically:
- **Saves game state** to disk at regular intervals
- **Creates backups** before starting to prevent data loss
- **Recovers player data** on server restart
- **Validates compatibility** between saved state and current configuration
- **Cleans up stale data** by removing long-inactive players
- **Maintains privacy** by excluding network-sensitive information

## Implementation Details

### Persistence Features

- **Automatic Saving**: Game state is automatically saved every 2 minutes while the server is running
- **Atomic Saves**: Uses temporary files during the write process to prevent corruption if interrupted
- **JSON Format**: State files are stored in human-readable JSON format for easy inspection if needed
- **Configurable Storage**: Storage location can be configured via environment variables
- **Graceful Degradation**: Server operates normally even if persistence is disabled
- **Position Validation**: Handles world boundary changes by validating saved positions

### Data Management

- **Backup Creation**: Before loading state, the system creates a backup of any existing state file
- **Stale Data Cleanup**: Players offline for more than 5 minutes are automatically removed during loading
- **Privacy Protection**: Network-sensitive information is excluded from the saved data
- **Compatibility Checking**: Ensures saved state is compatible with current server configuration

### Recovery Process

When the server starts:
1. The system checks for an existing state file
2. If found, it creates a backup of the current state
3. The state file is loaded and validated
4. Stale player data is removed
5. Positions are validated against current world boundaries
6. The cleaned state is applied to the server

Players need to reconnect after a server restart for network security reasons, but their game state (position, health, etc.) is preserved.

## Configuration

### Environment Variables

```bash
# Set the directory where game state files are stored
export NYMQUEST_STATE_DIRECTORY=/path/to/state/directory

# Set the base filename for state files (default: "game_state")
export NYMQUEST_STATE_FILENAME=custom_state_name

# Disable state persistence entirely (default: enabled)
export NYMQUEST_DISABLE_PERSISTENCE=true
```

### Default Settings

- **Save Interval**: 2 minutes
- **State Directory**: `./game_data/` relative to the server executable
- **Filename**: `game_state.json`
- **Backup Filename**: `game_state.json.bak`
- **Temp Filename During Save**: `game_state.json.tmp`

## Benefits

- **Data Durability**: Game state persists across server restarts
- **Continuity**: Players maintain their progress even if the server needs to be restarted
- **Administration Flexibility**: Allows for server maintenance without complete game reset
- **Disaster Recovery**: Backup system provides protection against data corruption
