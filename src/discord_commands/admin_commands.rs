use anyhow::{anyhow, Context, Result};
use serenity::all::{ChannelId, CommandDataOptionValue, CommandInteraction};

use crate::bonk_bot::{room_maker::RoomParameters, BonkBotKey};

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
                "**leaderboard, lb <create/remove>:** Creates a leaderboard from a config file and a specified Discord channel.\n",
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

    if let Some(&option) = args.get(0) {
        match option {
            "create" | "c" => {
                //TODO

                interaction
                    .create_response(&ctx.http, response_message("hello"))
                    .await?;
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
                    sqlx::query_as("SELECT id FROM channels WHERE type = 'room log'")
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

    let mut data = ctx.data.write().await;
    if let Some(bonk_bot) = data.get_mut::<BonkBotKey>() {
        match bonk_bot.open_room(room_parameters.clone()).await {
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
                                format!("Room opened:\n\n{}\n{}", room_parameters.name, room_link),
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
    if help_check(
        ctx,
        interaction,
        &args,
        "Shuts down the bot. This is the reccomended way to do it.",
    )
    .await?
    {
        return Ok(());
    }

    let mut data = ctx.data.write().await;
    if let Some(bonk_bot) = data.get_mut::<BonkBotKey>() {
        match bonk_bot.close_all().await {
            Ok(()) => {
                interaction
                    .create_response(&ctx.http, response_message("Rooms closed!"))
                    .await?;
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
