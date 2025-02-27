# Reference

## Bonk.io Commands

```
!help: Lists commands.
!ping: Pong!
!queue: Checks the queue. Spots will be held for some time when players leave.
!pick, !p <name>: When prompted, this command chooses and opponent to play against.
```

## Discord Base Commands

All commands start with /sgr with the command written in the command field.

```
A single slash command for all of your sgrBot needs!

Commands:
help, h: The help menu that you're currently reading.
ping: Pong!
a: Runs an admin command.
```

## Discord Admin Commands

All commands are prefixed with /sgr a. Commands that target specific users, channels, or config files will use those optional arguments from the /sgr command.

```
Here's a list of admin commands.

Commands:
admins <add/remove/list>: Edits the list of admins who have access to the "a" command.
roomlog <get/set/clear>: Edits the room log channel where room links are posted.
open, o: Creates a room from a room config file!
closeall, ca: Closes all rooms.
shutdown, sd: Shuts down the bot. This is the reccomended way to do it.
```

## Room Config Template

The "/sgr a open" command takes a TOML config file as an argument. Below is an example config file.

```toml
# Required

name = "Test room"
max_players = 8
min_level = 1
# "Football", "Simple", "DeathArrows", "Arrows", "Grapple", "VTOL", "Classic"
mode = "Classic"
rounds = 8

# Optional

headless = true
password = "Super secret password"
unlisted = true
```
