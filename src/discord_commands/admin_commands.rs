use anyhow::{anyhow, Context, Result};
use serenity::all::{ChannelId, CommandDataOptionValue, CommandInteraction, CreateMessage};

use crate::bonk_bot::{room_maker::RoomParameters, BonkBotKey};
use crate::DatabaseValue;

use super::super::leaderboard::LeaderboardSettings;
use super::{edit_message, help_check, loading_message, response_message};

pub async fn admin_help(
    ctx: &serenity::all::Context,
    interaction: &CommandInteraction,
) -> Result<()> {
    interaction
        .create_response(
            &ctx.http,
            response_message(concat!(
                "Here's a list of admin commands.\n\n",
                "__Commands:__\n",
                "**admins <add/remove/list>:** Edits the list of admins who have access to the \"a\" command.\n",
                "**leaderboard, lb <create/remove/list>:** Creates a leaderboard from a config file and a specified Discord channel.\n",
                "**leaderboard, lb edit <leaderboard abbreviation>:** Modifies the leaderboard config file and channel. May break the leaderboard.\n",
                "**leaderboard, lb match_channel <get/set/clear> <leaderboard abbreviation>:** Sets the channel where matches are posted.\n",
                "**roomlog <get/set/clear>:** Edits the room log channel where room links are posted.\n",
                "**open, o:** Creates a room from a room config file!\n",
                "**closeall, ca:** Closes all rooms.\n",
                "**shutdown, sd:** Shuts down the bot. This is the reccomended way to do it.\n",
            )),
        )
        .await?;

    Ok(())
}

pub async fn admins(
    ctx: &serenity::all::Context,
    interaction: &CommandInteraction,
    args: Vec<&str>,
) -> Result<()> {
    if help_check(
        ctx,
        interaction,
        &args,
        concat!(
            "Edits the list of admins who have access to the \"a\" command.",
            " A user must be specified with the \"user:\" option for add and remove."
        ),
    )
    .await?
    {
        return Ok(());
    }

    let db = {
        let data = ctx.data.read().await;
        data.get::<crate::DatabaseKey>().cloned()
    }
    .ok_or(anyhow!("Failed to connect to database."))?;

    if let Some(&option) = args.get(0) {
        match option {
            "add" | "a" => {
                let user = &interaction
                    .data
                    .options
                    .iter()
                    .find(|o| o.name == "user")
                    .context("User not selected.")?
                    .value;

                if let CommandDataOptionValue::User(user) = user {
                    let user = user.get() as i64;

                    sqlx::query("INSERT INTO admins (id) VALUES ($1)")
                        .bind(user)
                        .execute(db.db.as_ref())
                        .await?;

                    interaction
                        .create_response(
                            &ctx.http,
                            response_message(format!("<@{}> is now an admin!", user)),
                        )
                        .await?;
                }
            }
            "remove" | "r" => {
                let user = &interaction
                    .data
                    .options
                    .iter()
                    .find(|o| o.name == "user")
                    .context("User not selected.")?
                    .value;

                if let CommandDataOptionValue::User(user) = user {
                    let user = user.get() as i64;

                    sqlx::query("DELETE FROM admins WHERE id = $1")
                        .bind(user)
                        .execute(db.db.as_ref())
                        .await?;

                    interaction
                        .create_response(
                            &ctx.http,
                            response_message(format!("<@{}> is no longer admin.", user)),
                        )
                        .await?;
                }
            }
            "list" | "ls" => {
                let users: Vec<(i64,)> = sqlx::query_as("SELECT id FROM admins")
                    .fetch_all(db.db.as_ref())
                    .await?;

                let users: Vec<u64> = users.iter().map(|r| r.0 as u64).collect();

                let mut output = "Admin list:".to_string();
                output.push_str(
                    &users
                        .iter()
                        .map(|u| format!("\n<@{}>", u))
                        .collect::<Vec<String>>()
                        .concat(),
                );

                interaction
                    .create_response(&ctx.http, response_message(output))
                    .await?;
            }
            _ => {
                return Err(anyhow!("Invalid argument."));
            }
        }
    } else {
        return Err(anyhow!("Missing argument for \"a admins\" command."));
    }

    Ok(())
}

pub async fn leaderboard(
    ctx: &serenity::all::Context,
    interaction: &CommandInteraction,
    args: Vec<&str>,
) -> Result<()> {
    if help_check(
        ctx,
        interaction,
        &args,
        concat!("This command edits the list of leaderboards.",),
    )
    .await?
    {
        return Ok(());
    }

    let db = {
        let data = ctx.data.read().await;
        data.get::<crate::DatabaseKey>().cloned()
    }
    .ok_or(anyhow!("Failed to connect to database."))?;

    if let Some(&option) = args.get(0) {
        match option {
            "create" | "c" => {
                let attachment = &interaction
                    .data
                    .resolved
                    .attachments
                    .values()
                    .next()
                    .context("Attachment not found.")?;
                let channel = &interaction
                    .data
                    .options
                    .iter()
                    .find(|o| o.name == "channel")
                    .context("Channel not selected.")?
                    .value
                    .as_channel_id()
                    .context("Channel ID expected.")?;

                let response = reqwest::get(&attachment.url).await?;
                let file = response.text().await?;
                let settings: LeaderboardSettings = toml::de::from_str(&file)?;
                let settings_json = serde_json::to_value(&settings)?;

                let message = channel
                    .send_message(&ctx.http, CreateMessage::new().content(&settings.name))
                    .await?;

                sqlx::query(
                    "INSERT INTO leaderboard (name, abbreviation, settings, channel, messages) VALUES ($1, $2, $3, $4, $5)",
                )
                .bind(settings.name)
                .bind(&settings.abbreviation)
                .bind(settings_json)
                .bind(i64::from(*channel))
                .bind(vec![i64::from(message.id)])
                .execute(db.db.as_ref())
                .await?;

                interaction
                    .create_response(
                        &ctx.http,
                        response_message(format!("{} was created.", settings.abbreviation)),
                    )
                    .await?;
            }
            "list" | "l" | "ls" => {
                let list: Vec<(String, String)> =
                    sqlx::query_as("SELECT name, abbreviation FROM leaderboard")
                        .fetch_all(db.db.as_ref())
                        .await?;

                let list = list
                    .iter()
                    .map(|x| format!("{} ({})", x.0, x.1))
                    .collect::<Vec<String>>()
                    .join("\n");

                interaction
                    .create_response(
                        &ctx.http,
                        response_message(format!("Leaderboards:\n\n{}", list)),
                    )
                    .await?;
            }
            "remove" | "r" | "rm" => {
                if let Some(lb_abbr) = args.get(1) {
                    sqlx::query("DELETE FROM leaderboard WHERE abbreviation = $1")
                        .bind(lb_abbr)
                        .execute(db.db.as_ref())
                        .await?;

                    interaction
                        .create_response(
                            &ctx.http,
                            response_message(format!("Deleted {}.", lb_abbr)),
                        )
                        .await?;
                } else {
                    interaction
                        .create_response(
                            &ctx.http,
                            response_message(
                                "Missing argument for \"a leaderboard remove\" command.",
                            ),
                        )
                        .await?;
                }
            }
            "edit" | "e" => {
                if let Some(lb_abbr) = args.get(1) {
                    let attachment = &interaction
                        .data
                        .resolved
                        .attachments
                        .values()
                        .next()
                        .context("Attachment not found.")?;
                    let channel = &interaction
                        .data
                        .options
                        .iter()
                        .find(|o| o.name == "channel")
                        .context("Channel not selected.")?
                        .value
                        .as_channel_id()
                        .context("Channel ID expected.")?;

                    let response = reqwest::get(&attachment.url).await?;
                    let file = response.text().await?;
                    let settings: LeaderboardSettings = toml::de::from_str(&file)?;
                    let settings_json = serde_json::to_value(&settings)?;

                    let lb_id: Option<i64> =
                        sqlx::query_scalar("SELECT id FROM leaderboard WHERE abbreviation = $1")
                            .bind(lb_abbr)
                            .fetch_optional(db.db.as_ref())
                            .await?;
                    if lb_id.is_some() {
                        sqlx::query(
                            "UPDATE leaderboard SET name = $1, abbreviation = $2, settings = $3, channel = $4 WHERE abbreviation = $5",
                        )
                        .bind(settings.name)
                        .bind(&settings.abbreviation)
                        .bind(settings_json)
                        .bind(i64::from(*channel))
                        .bind(lb_abbr)
                        .execute(db.db.as_ref())
                        .await?;

                        interaction
                            .create_response(
                                &ctx.http,
                                response_message(format!("Updated {}", lb_abbr)),
                            )
                            .await?;
                    } else {
                        interaction
                            .create_response(
                                &ctx.http,
                                response_message(format!("Failed to find \"{}\"", lb_abbr)),
                            )
                            .await?;
                    }
                } else {
                    interaction
                        .create_response(
                            &ctx.http,
                            response_message(
                                "Missing argument for \"a leaderboard remove\" command.",
                            ),
                        )
                        .await?;
                }
            }
            "match_channel" | "mc" => {
                let mut args = args.clone();
                args.remove(0);
                match_channel(ctx, db, interaction, args).await?;
            }
            _ => {
                return Err(anyhow!("Invalid argument."));
            }
        }
    } else {
        return Err(anyhow!("Missing argument for \"a leaderboard\" command."));
    }

    Ok(())
}

pub async fn match_channel(
    ctx: &serenity::all::Context,
    db: DatabaseValue,
    interaction: &CommandInteraction,
    args: Vec<&str>,
) -> Result<()> {
    if help_check(
        ctx,
        interaction,
        &args,
        concat!("This command edits a leaderboard's match channel.",),
    )
    .await?
    {
        return Ok(());
    }

    if let Some(&option) = args.get(0) {
        let abbreviation = *args.get(1).context("Missing leaderboard abbreviation.")?;

        match option {
            "get" | "g" => {
                let channel: Option<i64> = sqlx::query_scalar(
                    "SELECT match_channel FROM leaderboard WHERE abbreviation = $1",
                )
                .bind(abbreviation)
                .fetch_one(db.db.as_ref())
                .await?;
                let channel = channel.context("This leaderboard doesn't have a match channel.")?;

                interaction
                    .create_response(
                        &ctx.http,
                        response_message(format!(
                            "This leaderboard's match channel is <#{}>.",
                            channel
                        )),
                    )
                    .await?;
            }
            "set" | "s" => {
                let channel = &interaction
                    .data
                    .options
                    .iter()
                    .find(|o| o.name == "channel")
                    .context("Channel not selected.")?
                    .value;

                if let CommandDataOptionValue::Channel(channel) = channel {
                    let channel = channel.get() as i64;
                    sqlx::query(
                        "UPDATE leaderboard SET match_channel = $1 WHERE abbreviation = $2",
                    )
                    .bind(channel)
                    .bind(abbreviation)
                    .execute(db.db.as_ref())
                    .await?;

                    interaction
                        .create_response(
                            &ctx.http,
                            response_message(format!(
                                "Match channel set to <#{}>.",
                                channel as u64
                            )),
                        )
                        .await?;
                }
            }
            "clear" | "c" => {
                sqlx::query("UPDATE leaderboard SET match_channel = NULL WHERE abbreviation = $1")
                    .bind(abbreviation)
                    .execute(db.db.as_ref())
                    .await?;

                interaction
                    .create_response(&ctx.http, response_message("Match channel cleared."))
                    .await?;
            }
            _ => return Err(anyhow!("Invalid argument.")),
        }
    } else {
        return Err(anyhow!(
            "Missing  get/set/clear argument from \"a leaderboard match_channel\" command."
        ));
    }

    Ok(())
}

pub async fn roomlog(
    ctx: &serenity::all::Context,
    interaction: &CommandInteraction,
    args: Vec<&str>,
) -> Result<()> {
    if help_check(
        ctx,
        interaction,
        &args,
        concat!("This command edits the room log channel where room links are posted.",),
    )
    .await?
    {
        return Ok(());
    }

    let db = {
        let data = ctx.data.read().await;
        data.get::<crate::DatabaseKey>().cloned()
    }
    .ok_or(anyhow!("Failed to connect to database."))?;

    if let Some(&option) = args.get(0) {
        match option {
            "get" | "g" => {
                let channel: Vec<(i64,)> =
                    sqlx::query_as("SELECT id FROM channegls WHERE type = 'room log'")
                        .fetch_all(db.db.as_ref())
                        .await?;

                if channel.len() == 0 {
                    interaction
                        .create_response(&ctx.http, response_message("Room log is not set."))
                        .await?;
                } else {
                    let channel = channel.get(0).context("Missing room id.")?.0 as u64;

                    interaction
                        .create_response(
                            &ctx.http,
                            response_message(format!("The room log channel is <#{}>.", channel)),
                        )
                        .await?;
                }
            }
            "set" | "s" => {
                let rows: Vec<(i64,)> =
                    sqlx::query_as("SELECT id FROM channels WHERE type = 'room log'")
                        .fetch_all(db.db.as_ref())
                        .await?;

                let channel = &interaction
                    .data
                    .options
                    .iter()
                    .find(|o| o.name == "channel")
                    .context("Channel not selected.")?
                    .value;

                if let CommandDataOptionValue::Channel(channel) = channel {
                    let channel = channel.get() as i64;

                    if rows.len() == 0 {
                        sqlx::query("INSERT INTO channels (id, type) VALUES ($1, 'room log')")
                            .bind(channel)
                            .execute(db.db.as_ref())
                            .await?;
                    } else {
                        sqlx::query("UPDATE channels SET id = $1 WHERE type = 'room log'")
                            .bind(channel)
                            .execute(db.db.as_ref())
                            .await?;
                    }

                    interaction
                        .create_response(
                            &ctx.http,
                            response_message(format!("Room log is now <#{}>.", channel as u64)),
                        )
                        .await?;
                }
            }
            "clear" | "c" => {
                sqlx::query("DELETE FROM channels WHERE type = 'room log'")
                    .execute(db.db.as_ref())
                    .await?;

                interaction
                    .create_response(&ctx.http, response_message("Room log cleared."))
                    .await?;
            }
            _ => {
                return Err(anyhow!("Invalid argument."));
            }
        }
    } else {
        return Err(anyhow!("Missing argument for /a roomlog command."));
    }

    Ok(())
}

pub async fn open(
    ctx: &serenity::all::Context,
    interaction: &CommandInteraction,
    args: Vec<&str>,
) -> Result<()> {
    if help_check(
        ctx,
        interaction,
        &args,
        concat!(
            "This command opens a bonk.io room from room parameters specified in a TOML file ",
            "attachment. Use \"closeall\" to close all rooms.",
        ),
    )
    .await?
    {
        return Ok(());
    }

    interaction
        .create_response(&ctx.http, loading_message())
        .await?;

    let attachment = interaction
        .data
        .resolved
        .attachments
        .values()
        .next()
        .context("Attachment not found.")?;

    let response = reqwest::get(&attachment.url).await?;
    let file = response.text().await?;
    let room_parameters: RoomParameters = toml::de::from_str(&file)?;

    let data = ctx.data.read().await;

    if let Some(bonk_bot) = data.get::<BonkBotKey>() {
        match bonk_bot.open_room(ctx, room_parameters.clone()).await {
            Ok(room_link) => {
                interaction
                    .edit_response(
                        &ctx.http,
                        edit_message(format!("Room opened: {}", room_link)),
                    )
                    .await?;

                let db = data.get::<crate::DatabaseKey>().cloned();

                if let Some(db) = db {
                    let channel: Vec<(i64,)> =
                        sqlx::query_as("SELECT id FROM channels WHERE type = 'room log'")
                            .fetch_all(db.db.as_ref())
                            .await?;

                    if let Some(channel) = channel.get(0) {
                        let channel = ChannelId::new(channel.0 as u64);
                        channel
                            .say(
                                &ctx.http,
                                format!("Room opened: {}\n{}", room_parameters.name, room_link),
                            )
                            .await?;
                    }
                }
            }
            Err(e) => {
                interaction
                    .edit_response(
                        &ctx.http,
                        edit_message(format!("Failed to make room: {}", e)),
                    )
                    .await?;
            }
        }
    }

    Ok(())
}

pub async fn closeall(
    ctx: &serenity::all::Context,
    interaction: &CommandInteraction,
    args: Vec<&str>,
) -> Result<()> {
    if help_check(ctx, interaction, &args, "Closes all rooms.").await? {
        return Ok(());
    }

    let mut data = ctx.data.write().await;
    if let Some(bonk_bot) = data.get_mut::<BonkBotKey>() {
        match bonk_bot.close_all().await {
            Ok(()) => {
                interaction
                    .create_response(&ctx.http, response_message("Rooms closed!"))
                    .await?;

                let db = data.get::<crate::DatabaseKey>().cloned();

                if let Some(db) = db {
                    let channel: Vec<(i64,)> =
                        sqlx::query_as("SELECT id FROM channels WHERE type = 'room log'")
                            .fetch_all(db.db.as_ref())
                            .await?;

                    if let Some(channel) = channel.get(0) {
                        let channel = ChannelId::new(channel.0 as u64);
                        channel.say(&ctx.http, "All rooms closed!").await?;
                    }
                }
            }
            Err(e) => {
                interaction
                    .create_response(
                        &ctx.http,
                        response_message(format!("Error while closing rooms: {}", e)),
                    )
                    .await?;
            }
        }
    }

    Ok(())
}

pub async fn shutdown(
    ctx: &serenity::all::Context,
    interaction: &CommandInteraction,
    args: Vec<&str>,
) -> Result<()> {
    if help_check(
        ctx,
        interaction,
        &args,
        concat!(
            "Shuts down the bot. This is the reccomended way to do it. ",
            "This command does some cleanup. Hopefully, no clients will be leaked."
        ),
    )
    .await?
    {
        return Ok(());
    }

    let mut data = ctx.data.write().await;
    if let Some(bonk_bot) = data.get_mut::<BonkBotKey>() {
        let _ = bonk_bot.close_all().await;
    }

    interaction
        .create_response(&ctx.http, response_message("Goodbye!"))
        .await?;

    std::process::exit(0);
}
