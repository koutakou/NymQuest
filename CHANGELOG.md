# Changelog

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