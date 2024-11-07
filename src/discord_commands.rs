use anyhow::Result;
use serenity::all::{
    CommandInteraction, CreateInteractionResponse, CreateInteractionResponseMessage,
};

use crate::bonk_bot::BonkBotKey;

pub async fn help(ctx: &serenity::all::Context, interaction: &CommandInteraction) -> Result<()> {
    let message = CreateInteractionResponseMessage::new()
        .content(concat!(
            "A single slash command for all of your sgrBot needs!\n\n",
            "__Commands:__\n",
            "**help, h:** The help menu that you're currently reading.\n",
            "**open, o:** Creates a room!\n",
            "**shutdown, sd:** Shuts down the bot. This is the reccomended way to do it.\n",
            "**ping:** Pong!",
        ))
        .ephemeral(true);
    let response = CreateInteractionResponse::Message(message);
    interaction.create_response(&ctx.http, response).await?;

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

    let mut data = ctx.data.write().await;
    if let Some(bonk_bot) = data.get_mut::<BonkBotKey>() {
        match bonk_bot.open_room().await {
            Ok(()) => {
                println!("Room opened!");
            }
            Err(e) => {
                println!("Failed to make room: {}", e.to_string());
            }
        }
    }

    Ok(())
}

pub async fn shutdown(
    ctx: &serenity::all::Context,
    interaction: &CommandInteraction,
) -> Result<()> {
    let message = CreateInteractionResponseMessage::new()
        .content("Goodbye!")
        .ephemeral(true);
    let response = CreateInteractionResponse::Message(message);
    interaction.create_response(&ctx.http, response).await?;

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

    let message = CreateInteractionResponseMessage::new().content("Pong!");
    let response = CreateInteractionResponse::Message(message);
    interaction.create_response(&ctx.http, response).await?;

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
            let message = CreateInteractionResponseMessage::new()
                .content(help_message)
                .ephemeral(true);
            let response = CreateInteractionResponse::Message(message);
            interaction.create_response(&ctx.http, response).await?;

            return Ok(true);
        }
    }

    Ok(false)
}
