use anyhow::{Context, Result};
use serenity::all::CommandInteraction;

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
                "**open, o:** Creates a room from a room config file!\n",
                "**closeall, ca:** Closes all rooms.\n",
                "**shutdown, sd:** Shuts down the bot. This is the reccomended way to do it.\n",
            )),
        )
        .await?;

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
        match bonk_bot.open_room(room_parameters).await {
            Ok(room_link) => {
                interaction
                    .edit_response(
                        &ctx.http,
                        edit_message(format!("Room opened: {}", room_link)),
                    )
                    .await?;
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
