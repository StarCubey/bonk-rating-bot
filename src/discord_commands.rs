mod admin_commands;

use anyhow::Result;
use serenity::all::{
    CommandInteraction, CreateInteractionResponse, CreateInteractionResponseMessage,
    EditInteractionResponse,
};

pub async fn help(ctx: &serenity::all::Context, interaction: &CommandInteraction) -> Result<()> {
    let config = {
        let data = ctx.data.read().await;
        data.get::<super::ConfigKey>().cloned()
    };

    let mut admin = false;
    if let Some(config) = config {
        admin = config.bot_admin.contains(&interaction.user.id.get());
    }

    interaction
        .create_response(
            &ctx.http,
            response_message(
                concat!(
                    "A single slash command for all of your sgrBot needs!\n\n",
                    "__Commands:__\n",
                    "**help, h:** The help menu that you're currently reading.\n",
                    "**ping:** Pong!",
                )
                .to_string()
                    + if admin {
                        "\n**a**: Runs an admin command."
                    } else {
                        ""
                    },
            ),
        )
        .await?;

    Ok(())
}

pub async fn ping(
    ctx: &serenity::all::Context,
    interaction: &CommandInteraction,
    args: Vec<&str>,
) -> Result<()> {
    if help_check(
        ctx,
        interaction,
        &args,
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

pub async fn a(
    ctx: &serenity::all::Context,
    interaction: &CommandInteraction,
    args: Vec<&str>,
) -> Result<()> {
    if help_check(ctx, interaction, &args, "Just a test :3").await? {
        return Ok(());
    }

    let config = {
        let data = ctx.data.read().await;
        data.get::<super::ConfigKey>().cloned()
    };

    if let Some(config) = config {
        if config.bot_admin.contains(&interaction.user.id.get()) {
            match args.get(0) {
                Some(&subcommand) => match subcommand {
                    "open" | "o" => admin_commands::open(ctx, interaction, args).await?,
                    "shutdown" | "sd" => admin_commands::shutdown(ctx, interaction, args).await?,
                    "closeall" | "ca" => admin_commands::closeall(ctx, interaction, args).await?,
                    _ => {
                        interaction
                            .create_response(
                                &ctx.http,
                                response_message(format!(
                                    "Unknown command \"{}\". Run \"help\" for a list of commands.",
                                    subcommand
                                )),
                            )
                            .await?;
                    }
                },
                None => {
                    admin_commands::admin_help(ctx, interaction).await?;
                }
            }
        } else {
            interaction
                .create_response(&ctx.http, response_message("You aren't an admin, silly!"))
                .await?;
        }
    } else {
        interaction
            .create_response(
                &ctx.http,
                response_message("Error: Missing or invalid config data."),
            )
            .await?;
    }

    Ok(())
}

pub async fn help_check(
    ctx: &serenity::all::Context,
    interaction: &CommandInteraction,
    args: &Vec<&str>,
    help_message: &str,
) -> Result<bool> {
    if let Some(subcommand) = args.get(0) {
        if let "help" | "h" | "?" = *subcommand {
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
