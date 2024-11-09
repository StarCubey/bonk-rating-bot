use anyhow::Result;
use serenity::all::{
    CommandInteraction, CreateInteractionResponse, CreateInteractionResponseMessage,
    EditInteractionResponse,
};

use crate::bonk_bot::BonkBotKey;

pub async fn help(ctx: &serenity::all::Context, interaction: &CommandInteraction) -> Result<()> {
    interaction
        .create_response(
            &ctx.http,
            response_message(concat!(
                "A single slash command for all of your sgrBot needs!\n\n",
                "__Commands:__\n",
                "**help, h:** The help menu that you're currently reading.\n",
                "**open, o:** Creates a room!\n",
                "**closeall, ca:** Closes all rooms.\n",
                "**shutdown, sd:** Shuts down the bot. This is the reccomended way to do it.\n",
                "**ping:** Pong!",
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
        args,
        "This command opens a bonk.io room. Use \"close\" to close all rooms.",
    )
    .await?
    {
        return Ok(());
    }

    interaction
        .create_response(&ctx.http, loading_message())
        .await?;

    let mut data = ctx.data.write().await;
    if let Some(bonk_bot) = data.get_mut::<BonkBotKey>() {
        match bonk_bot.open_room().await {
            Ok(()) => {
                interaction
                    .edit_response(&ctx.http, edit_message("Room opened!"))
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
        args,
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
        args,
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

pub async fn ping(
    ctx: &serenity::all::Context,
    interaction: &CommandInteraction,
    args: Vec<&str>,
) -> Result<()> {
    if help_check(
        ctx,
        interaction,
        args,
        "This is a ping command.\n\nUsage: ping",
    )
    .await?
    {
        return Ok(());
    }

    interaction
        .create_response(&ctx.http, response_message("Pong!"))
        .await?;

    Ok(())
}

pub async fn help_check(
    ctx: &serenity::all::Context,
    interaction: &CommandInteraction,
    args: Vec<&str>,
    help_message: &str,
) -> Result<bool> {
    if let Some(subcommand) = args.get(0) {
        if let "help" | "h" = *subcommand {
            interaction
                .create_response(&ctx.http, response_message(help_message))
                .await?;

            return Ok(true);
        }
    }

    Ok(false)
}

pub fn response_message(message: impl Into<String>) -> CreateInteractionResponse {
    let message = CreateInteractionResponseMessage::new()
        .content(message)
        .ephemeral(true);

    CreateInteractionResponse::Message(message)
}

pub fn loading_message() -> CreateInteractionResponse {
    let message = CreateInteractionResponseMessage::new().ephemeral(true);

    CreateInteractionResponse::Defer(message)
}

pub fn edit_message(message: impl Into<String>) -> EditInteractionResponse {
    EditInteractionResponse::new().content(message)
}
