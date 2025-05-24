# Combat System

NymQuest features a simple but engaging combat system that allows players to engage in player-versus-player combat while maintaining privacy.

## Combat Mechanics

- **Attack Range**: Players can attack others within 28.0 units of distance
- **Cooldown System**: 3-second cooldown between attacks
- **Health**: Players start with 100 health points
- **Damage**: Base attack deals 10 damage points
- **Critical Hits**: 15% chance to land a critical hit doing double damage (20 points)
- **Respawn**: Defeated players respawn with full health at a random position

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

## Future Combat Enhancements

The following combat features are planned for future updates:

- Different weapon types with varying damage and range properties
- Armor and defensive items
- Special abilities and cooldowns
- Environmental obstacles and cover mechanics
- Team-based combat modes
