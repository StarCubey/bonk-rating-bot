mod admin_commands;

use anyhow::Result;
use serenity::all::{
    CommandInteraction, CreateInteractionResponse, CreateInteractionResponseMessage,
    EditInteractionResponse,
};

pub async fn help(ctx: &serenity::all::Context, interaction: &CommandInteraction) -> Result<()> {
    let owner: u64 = dotenv::var("DISCORD_USER_ID")?.parse()?;
    let user = &interaction.user.id.get();

    let db = {
        let data = ctx.data.read().await;
        data.get::<super::DatabaseKey>().cloned()
    };

    let mut admin = *user == owner;
    if let Some(db) = db {
        let rows: Vec<(i64,)> = sqlx::query_as("SELECT id FROM admins")
            .fetch_all(db.db.as_ref())
            .await?;

        let ids: Vec<u64> = rows.iter().map(|r| r.0 as u64).collect();

        admin = admin || ids.contains(user);
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
                        "\n**a:** Runs an admin command."
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
    mut args: Vec<&str>,
) -> Result<()> {
    if help_check(ctx, interaction, &args, "Shows admin commands.").await? {
        return Ok(());
    }

    let owner: u64 = dotenv::var("DISCORD_USER_ID")?.parse()?;
    let user = &interaction.user.id.get();

    let db = {
        let data = ctx.data.read().await;
        data.get::<super::DatabaseKey>().cloned()
    };

    if let Some(db) = db {
        let rows: Vec<(i64,)> = sqlx::query_as("SELECT id FROM admins")
            .fetch_all(db.db.as_ref())
            .await?;

        let ids: Vec<u64> = rows.iter().map(|r| r.0 as u64).collect();

        if ids.contains(user) || *user == owner {
            if let Some(&subcommand) = args.get(0) {
                args.remove(0);
                match subcommand {
                    "admins" => admin_commands::admins(ctx, interaction, args).await?,
                    "leaderboard" | "lb" => {
                        admin_commands::leaderboard(ctx, interaction, args).await?
                    }
                    "roomlog" => admin_commands::roomlog(ctx, interaction, args).await?,
                    "open" | "o" => admin_commands::open(ctx, interaction, args).await?,
                    "shutdown" | "sd" => admin_commands::shutdown(ctx, interaction, args).await?,
                    "closeall" | "ca" => admin_commands::closeall(ctx, interaction, args).await?,
                    "forcecloseall" | "fca" => {
                        admin_commands::forcecloseall(ctx, interaction, args).await?
                    }
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
                }
            } else {
                admin_commands::admin_help(ctx, interaction).await?;
            }
        } else {
            interaction
                .create_response(&ctx.http, response_message("You aren't an admin!"))
                .await?;
        }
    } else {
        interaction
            .create_response(
                &ctx.http,
                response_message("Error: Can't connect to database."),
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
