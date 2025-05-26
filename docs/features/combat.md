# Combat System

NymQuest features a simple but engaging combat system that allows players to engage in player-versus-player combat while maintaining privacy.

## Combat Mechanics

- **Attack Range**: Players can attack others within 28.0 units of distance
- **Cooldown System**: 3-second cooldown between attacks
- **Health**: Players start with 100 health points
- **Damage**: Base attack deals 10 damage points
- **Critical Hits**: 15% chance to land a critical hit doing double damage (20 points)
- **Respawn**: Defeated players respawn with full health at a random position
- **Experience**: Players earn XP for successful attacks
- **Levels**: Players progress through levels as they gain experience

## How to Attack

To attack another player, use the attack command followed by the player's display ID (shown in brackets):

```
/attack player_display_id
```

or the shorter version:

```
/a player_display_id
```

For example, if you see a player with the display ID [Warrior123], you would type:

```
/attack Warrior123
```

## Combat Feedback

The game provides feedback on combat actions:

- When you attack a player, you'll see a message indicating whether the attack hit and how much damage was dealt
- If you land a critical hit, this will be clearly indicated
- When your health is low, the health indicator will change color to warn you
- When a player is defeated, a system message announces this to all players
- You'll receive notifications when other players attack you

## Combat Strategy

- Maintain distance from other players if you want to avoid combat
- Track your health and retreat when necessary
- Be aware of your attack cooldown timer
- Use movement strategically to position yourself for attacks or escape

## Experience and Leveling System

### Experience Points (XP)

- **Earning XP**: Players earn experience points by attacking other players
- **XP for Damage**: Each point of damage dealt awards 1 XP
- **Bonus XP**: Defeating a player (reducing their health to zero) awards 20 bonus XP

### Player Levels

- **Starting Level**: All players begin at Level 1 with 0 XP
- **Level Progression**: Players need (level × 100) XP to advance to the next level
  - Level 2 requires 100 XP
  - Level 3 requires 200 XP
  - And so on

### Level Benefits

- **Damage Bonus**: +2 damage per level above level 1
- **Health Bonus**: +5 maximum health per level above level 1
- **Visual Indicator**: Your current level is displayed in your player stats

## Future Combat Enhancements

The following combat features are planned for future updates:

- Different weapon types with varying damage and range properties
- Armor and defensive items
- Special abilities and cooldowns
- Environmental obstacles and cover mechanics
- Team-based combat modes
