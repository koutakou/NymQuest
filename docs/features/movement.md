# Movement System

The NymQuest movement system provides players with the ability to navigate the game world while maintaining privacy and ensuring consistent gameplay experience.

## Key Features

- **Privacy-Preserving Movement**: All movement commands are transmitted through the Nym mixnet, ensuring anonymity and protection against traffic analysis
- **Consistent Movement Speeds**: Server and client use the same configurable movement speed parameters to ensure predictable gameplay
- **Collision Detection**: Players cannot overlap, providing a more realistic game world experience
- **Boundary Enforcement**: Players cannot move outside the defined world boundaries
- **Client-Side Prediction**: The client predicts movement results for responsive feedback, while the server maintains authoritative control

## Implementation Details

### Movement Protocol

Movement in NymQuest is implemented using the following components:

1. **Direction Enum**: The `Direction` enum defines eight possible movement directions (Up, Down, Left, Right, and diagonals)
2. **Position Struct**: The `Position` struct stores player coordinates in the game world
3. **Movement Messages**: Movement is initiated through `ClientMessage::Move` messages that specify the desired direction
4. **Server Validation**: The server validates and processes movement requests with:
   - Boundary checks to ensure players stay within the game world
   - Collision detection to prevent player overlap
   - Movement speed enforcement based on server configuration

### Configuration

Movement behavior can be customized through the following environment variables:

- `NYMQUEST_MOVEMENT_SPEED`: Defines the distance traveled per movement command (default: 14.0)
- `NYMQUEST_PLAYER_COLLISION_RADIUS`: Sets the minimum distance between players (default: 7.0)
- `NYMQUEST_WORLD_MIN_X`, `NYMQUEST_WORLD_MAX_X`, `NYMQUEST_WORLD_MIN_Y`, `NYMQUEST_WORLD_MAX_Y`: Define the game world boundaries

## Privacy Considerations

The movement system incorporates several privacy-enhancing features:

- **Mixnet Communication**: All movement commands are sent through the Nym mixnet, providing strong network-level privacy
- **Display IDs**: Players are identified by display IDs rather than real identities or network addresses
- **Consistent Message Sizes**: Movement messages have consistent sizes to prevent traffic analysis
- **Message Pacing**: Optional message pacing can be enabled to prevent timing correlation attacks

## User Interface

Players can move using the following commands:

- Arrow keys for directional movement
- WASD keys for directional movement
- Explicit `/move` command with direction parameter (e.g., `/move north`, `/move se`)

The game provides immediate feedback during movement, with clear messaging when movement is blocked due to collisions or world boundaries.
