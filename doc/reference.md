# Reference

## Bonk.io Commands

```
!help: Lists commands.
!ping: Pong!
!queue: Checks the queue. Spots will be held for some time when players leave.
!pick, !p <name>: When prompted, this command chooses and opponent to play against.
!ready, !r: Indicaates that you're ready to play before a game.
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
leaderboard, lb <create/remove>: Creates a leaderboard from a config file and a specified Discord channel.
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
password = ""
unlisted = true
```

## Leaderbaord Config Template

The "/sgr a lb create" command takes a TOML config file as an argument.

```toml
name = "Classic 1v1"
abbreviation = "c1"
# "whr", "glicko"
algorithm = "whr"
mean_rating = 1500
rating_scale = 173.72
unrated_deviation = 350

# Optional (depending on rating system)

whr_w = 0.0215
glicko_rp_days = 1
glicko_c = 0.0467
```
