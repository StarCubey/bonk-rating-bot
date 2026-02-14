# Reference

## Bonk.io Commands

```
!help: Lists commands.
!ping: Pong!
!queue: Checks the queue. Spots will be held for some time when players leave.
!pick, !p <name>: When prompted, this command chooses and opponent to play against.
!ready, !r: Indicaates that you're ready to play before a game.
!reset: Resets the current round with the same score.
!cancel, !c: Votes to cancel the game without recording the result.
```

## Discord Base Commands

All commands start with /elo with the command written in the command field.

```
Commands:
help, h: The help menu that you're currently reading.
ping: Pong!
a: Runs an admin command.
```

## Discord Admin Commands

All commands are prefixed with /elo a. Commands that target specific users, channels, or config files will use those optional arguments from the /elo command.

```
Here's a list of admin commands.

Commands:
admins <add/remove/list>: Edits the list of admins who have access to the "a" command.
leaderboard, lb <create/remove/list>: Creates a leaderboard from a config file and a specified Discord channel.
leaderboard, lb edit <leaderboard abbreviation>: Modifies the leaderboard config file and channel. May break the leaderboard.
leaderboard, lb match_channel <leaderboard abbreviation> <get/set/clear>: Sets the channel where matches are posted.
roomlog <get/set/clear>: Edits the room log channel where room links are posted.
open, o: Creates a room from a room config file!
closeall, ca: Closes all rooms.
forcecloseall, fca: Force closese all rooms.
shutdown, sd: Shuts down the bot. This is the reccomended way to do it.
```

## Room Config Template

The "/elo a open" command takes a TOML config file as an argument. Below is an example config file.

```toml
# Required

name = "Test room"
max_players = 8
# min_level = 0 is currently not supported.
min_level = 1
# "Football", "Simple", "DeathArrows", "Arrows", "Grapple", "VTOL", "Classic"
mode = "Classic"
# "Singles", "Teams", "FFA"
queue = "Singles"
rounds = 5
# List of maps from raw map data. You can get maps from your favorites with sgrAPI.getFav(0);.
maps = [
"""
{
  "id": 123,
  "name": "Simple 1v1",
  "authorname": "GudStrat",
  "leveldata": "ILDuJAhZIawhiQEVgGkCqAmANgFwGMBxADxwEkARAMSwFlVyAlAZgDVYMWmBPATQAaqGAEsArhACiAVlTRgACwASAEwDqTACoqlAKQUqkwAKahJCAMIgAHCgTngGSKGGJklV0a-efSByAA5NjVpMT41AEYcAC0LSE0AQyJqUGiBAHoAN3Sc3JyodIB2PLyWLJLc4CIAWwA2FQBzIzkCcDo6AT4ScmpIAGdBJmqAIxZdPF9J7wAFAGofbO90gAYALzp1zbpwKd29-YPJh2RJaBP5f2ALUGp4JEpgAHlPQ69Ic+BPNk1iahZh2CQaJeUCUO6vHxOT6XahcSAGLAAFkQQA",
  "publisheddate": "2020-05-05 16:59:52",
  "vu": 72147,
  "vd": 14943,
  "remixname": "",
  "remixauthor": "",
  "remixdb": 1,
  "remixid": 0
}
""",
]

# Optional (defaults shown)

strike_num = 2
team_size = 2
team_num = 2
ffa_min = 2
ffa_max = 7
# Timers are in seconds
idle_time = 5
pick_time = 60
ready_time = 60
strike_time = 20
game_time = 600
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
# These mean rating and rating scale values are based on the elo scale.
mean_rating = 1500
# Rating scale is how much the base win probability function is stretched horizontally.
rating_scale = 173.717793
# The deviation of a new player divided by the rating scale.
unrated_deviation = 2.014761
# deviation_per_day^2 * rating_scale^2 is added to the players' deviation^2 (variance) every day.
# Exact implementation depends on algorithm.
deviation_per_day = 0.037

# Optional (defaults shown)

# Conservative rating estimate. The number of standard deviations to subtract from
# the rating when calculating the displayed rating.
cre = 0
```
