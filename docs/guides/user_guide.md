# User Guide

This guide provides instructions on how to play NymQuest and use the available commands.

## Getting Started

After starting the client and connecting to the server, you first need to register with a username:
```
/register YourName
```

## Available Commands

### Registration and Login
- Register: `/register YourName` or `/r YourName`
- Disconnect: `/quit` or `/exit` or `/q`

### Movement
- Full command: `/move up` (or `/m up`, `/go up`), `/move down`, `/move left`, `/move right`
- Direct shortcuts: `/up` (or `/u`, `/n`), `/down` (or `/d`, `/s`), `/left` (or `/l`, `/w`), `/right` (or `/r`, `/e`)
- Diagonal movement: `/ne`, `/nw`, `/se`, `/sw`

### Combat
- Attack: `/attack player_display_id` or `/a player_display_id` (use the ID in [brackets], not the player name)

### Communication
- Chat: `/chat Hello everyone!` or `/c Hello everyone!` or `/say Hello everyone!`
- Emotes: `/emote wave` or `/em dance` 
  - Available emotes: wave, bow, laugh, dance, salute, shrug, cheer, clap

### Help and Information
- Help: `/help` or `/h` or `/?`

## Experience and Leveling

NymQuest features a progression system that rewards combat activity:

- **Gaining Experience**: You earn XP when you successfully attack other players
- **Experience Points**: Each point of damage you deal awards 1 XP
- **Bonus Experience**: Defeating a player (reducing their health to zero) awards 20 bonus XP
- **Level Progression**: To reach the next level, you need (current level × 100) XP
  - Level 2 requires 100 XP
  - Level 3 requires 200 XP
  - And so on
- **Level Benefits**:
  - Each level above 1 provides +2 damage to your attacks
  - Each level above 1 provides +5 maximum health
  - Your current level and XP progress are displayed in your player status panel

## User Interface Elements

The game interface is divided into several sections:

### World View
- Shows your character and other players in the game world
- Player positions are shown with their name and health status
- Your position is highlighted
- Other players' display IDs are shown in [brackets]

### Status Panel
- Shows your current health
- Displays your current level and experience points
- Displays your position coordinates
- Indicates your connection status and privacy level

### Message Log
- Shows game events, chat messages, and system notifications
- Different message types are color-coded for easy reading

### Mini-Map
- Provides a visual representation of player positions
- Your position is highlighted
- Other players are shown as dots

## Privacy and Connection Status

The status panel shows important information about your connection:

- **Connection Health**: Shows the quality of your Nym mixnet connection (Excellent, Good, Fair, Poor, Critical)
- **Privacy Protection**: Indicates your current anonymity status (Fully Protected, Protected, Degraded, Compromised)
- **Network Statistics**: Shows metrics like latency and message delivery success rates

## Security Features

NymQuest incorporates several security mechanisms to protect your gameplay experience:

- **Message Authentication**: All communications are authenticated using HMAC-SHA256, preventing message tampering
- **Message Expiration**: Messages automatically expire after a set time period (varies by message type), preventing delayed replay attacks
- **Replay Protection**: A sliding window approach prevents message replay attacks within the active time window
- **Message Pacing**: Communication timing is randomized to prevent timing correlation attacks
- **Privacy-Preserving IDs**: Players are identified by display IDs rather than actual usernames to other players

## Tips for Playing

- Keep an eye on your connection health status
- Use emotes for non-verbal communication when privacy is a priority
- Don't stand too close to other players if you want to avoid combat
- Use the help command if you forget any commands
- Watch the message log for important game events and announcements
