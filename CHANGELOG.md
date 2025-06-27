# Changelog

## [0.2.1] - 2025-06-27

### Performance Improvements
- **Memory & Data Structure Optimizations**: Replaced unbounded HashSet with bounded VecDeque for message replay protection, increased HashMap capacities to reduce reallocations, and added memory limits (MAX_RECEIVED_MSG_HISTORY: 1000, MAX_PENDING_ACKS: 100)
- **Rendering Performance**: Added static MINI_MAP_CACHE to eliminate repeated grid allocations, reducing ~225 character allocations per render cycle with lazy initialization for cached components
- **Network & Serialization**: Optimized channel buffer sizes and pre-allocated HashMap capacity for network data structures, improving async task scheduling efficiency
- **Algorithmic Improvements**: Implemented integer-based TokenBucket replacing float arithmetic for 50%+ performance gain, increased server GameState collection capacities to 256
- **String Allocation Optimization**: Converted ChatMessage to use Arc<String>, eliminating excessive cloning across client message handling
- **JSON Serialization Enhancement**: Replaced serde_json::to_string() with serde_json::to_vec() across 6 hotspots, improving message processing throughput

### CI/CD
- Updated continuous integration workflow configuration

## [0.2.0] - 2025-06-13

### Added
- Integrated Cypherpunk Worldbuilding with Mixnet Communication
- Implemented comprehensive mixnet health monitoring system

### Enhanced Privacy
- Enhanced message padding with thread-safe implementation
- Added dynamic message size padding for improved traffic analysis resistance
- Implemented message size normalization to prevent correlation attacks

### Fixed
- Fixed unnecessary type cast in mixnet_health module

## [0.1.4] - 2025-05-27

### Added
- Experience points and leveling system for character progression
- Key rotation mechanism with forward secrecy for enhanced message authentication
- Message expiration mechanism to prevent delayed replay attacks
- Improved security model with time-based message validity

## [0.1.3] - 2025-05-26

### Added
- Message prioritization system that enhances privacy protection against timing correlation attacks
- Variable jitter based on message type sensitivity (Critical, High, Medium, Low priority levels)
- Improved privacy protection during high server load situations
- Updated privacy documentation with details on the new prioritization system

### Improved
- Optimized server logging to reduce redundancy
- Eliminated duplicate configuration log entries
- Enhanced log readability by removing repetitive information

## [0.1.2] - 2025-05-25

### Fixed
- Fixed minimap synchronization issues with player positions
- Improved handling of PlayerUpdate messages in the client
- Modified minimap rendering to use actual world boundaries instead of hardcoded values
- Updated world info display to show correct coordinate boundaries

## [0.1.1] - 2025-05-25

### Fixed
- Movement system now uses consistent configuration values between client and server
- Added collision detection to prevent players from overlapping
- Fixed boundary handling in the player movement system

### Added
- Player collision radius configuration parameter (NYMQUEST_PLAYER_COLLISION_RADIUS)
- Comprehensive movement system documentation
- Utility functions for detecting position collisions

## [0.1.0] - Initial Release

- Initial implementation of NymQuest game
- Privacy-preserving gameplay using Nym mixnet
- Movement, combat, chat, and emote systems