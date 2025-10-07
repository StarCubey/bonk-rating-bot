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
leaderboard, lb <create/remove/list>: Creates a leaderboard from a config file and a specified Discord channel.
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
# Leaderboard abbreviation if a leaderboard is used for rated matches.
leaderboard = ""
```

## Leaderbaord Config Template

The "/sgr a lb create" command takes a TOML config file as an argument.

```toml
name = "Classic 1v1"
abbreviation = "c1"
# Currently only one algorithm supported.
algorithm = "OpenSkill"
# These mean rating and deviation scale values are based on the elo scale.
mean_rating = 1500
# Rating scale is the deviation of the derivative of the
# win probability function.
rating_scale = 315.09
# The deviation of a new player divided by the rating scale.
unrated_deviation = 1.1108
# deviation_per_day^2 * rating_scale^2 is added to the players' deviation^2 (variance) every day.
# Exact implementation depends on algorithm.
deviation_per_day = 0.037

# Optional

# Conservative rating estimate. The number of standard deviations to subtract from
# the rating when calculating the displayed rating.
cre = 0
```
