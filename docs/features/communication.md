# Communication System

NymQuest provides multiple ways for players to communicate while maintaining privacy through the Nym mixnet.

## Chat System

The chat system allows players to send text messages to all other players in the game:

- **Global Chat**: Messages are broadcast to all connected players
- **Privacy-Preserving**: All chat messages are routed through the Nym mixnet for metadata protection
- **Message Authentication**: Messages are authenticated to prevent tampering

### Chat Commands

To send a chat message, use one of the following commands:
```
/chat Hello everyone!
/c Hello everyone!
/say Hello everyone!
```

## Emote System

The emote system allows for non-verbal communication through visual actions:

- **Visual Feedback**: Emotes are shown in the message log with descriptive text
- **Social Interaction**: Enhances player interaction while maintaining privacy
- **Privacy-Preserving**: Emotes are processed through the same privacy-preserving channels as other messages

### Available Emotes

The following emotes are available:
- `/emote wave` or `/em wave` - Wave to other players
- `/emote bow` or `/em bow` - Bow respectfully
- `/emote laugh` or `/em laugh` - Laugh out loud
- `/emote dance` or `/em dance` - Perform a dance
- `/emote salute` or `/em salute` - Give a formal salute
- `/emote shrug` or `/em shrug` - Shrug your shoulders
- `/emote cheer` or `/em cheer` - Cheer enthusiastically
- `/emote clap` or `/em clap` - Applaud with appreciation
- `/emote thumbsup` or `/em thumbs` - Give a thumbs up sign

## Privacy Considerations

- All communication (both chat and emotes) is protected by the Nym mixnet
- Message pacing can be enabled to prevent timing correlation attacks
- No personal identifiers are attached to messages beyond the in-game display ID
- Message contents are not encrypted end-to-end by default, so avoid sharing sensitive information

## Messaging Limits

To prevent spam and ensure fair usage, the following limits apply:

- **Rate Limiting**: Default of 10 messages per second per connection
- **Burst Capacity**: Up to 20 messages can be sent in rapid succession before rate limiting applies
- **Client-Side Awareness**: Client maintains its own token bucket (8 msg/sec, 15 burst) to prevent hitting server limits

These limits apply to all message types, including chat messages and emotes.
