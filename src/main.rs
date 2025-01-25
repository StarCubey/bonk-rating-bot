mod bonk_bot;
mod discord_commands;

use std::sync::Arc;

use anyhow::{Context, Result};
use bonk_bot::{BonkBotKey, BonkBotValue};
use dotenv;
use serenity::{
    all::{
        ActivityData, Command, CommandInteraction, CreateCommand, CreateCommandOption,
        CreateInteractionResponse, CreateInteractionResponseMessage, EditInteractionResponse,
        EventHandler, GatewayIntents, Interaction, Ready,
    },
    async_trait,
    prelude::TypeMapKey,
};

struct Handler;

pub struct DatabaseKey;

impl TypeMapKey for DatabaseKey {
    type Value = DatabaseValue;
}

#[derive(Clone)]
pub struct DatabaseValue {
    db: Arc<sqlx::PgPool>,
}

#[async_trait]
impl EventHandler for Handler {
    async fn ready(&self, ctx: serenity::all::Context, _ready: Ready) {
        // if let Ok(commands) = Command::get_global_commands(&ctx.http).await {
        //     dbg!(commands);
        // }
        // let res =
        //     Command::delete_global_command(&ctx.http, CommandId::new(/*id*/)).await;
        // if let Err(res) = res {
        //     println!("{:?}", res);
        // }

        let activity = ActivityData::playing("bonk.io /sgr help");
        ctx.set_activity(Some(activity));

        if let Err(e) = Command::create_global_command(
            &ctx.http,
            CreateCommand::new("sgr")
                .description("A command prefix")
                .add_option(
                    CreateCommandOption::new(
                        serenity::all::CommandOptionType::String,
                        "command",
                        "Type \"help\" for a list of commands.",
                    )
                    .required(true),
                )
                .add_option(CreateCommandOption::new(
                    serenity::all::CommandOptionType::User,
                    "user",
                    "Add a user here if applicable.",
                ))
                .add_option(CreateCommandOption::new(
                    serenity::all::CommandOptionType::Channel,
                    "channel",
                    "Add a channel here if applicable.",
                ))
                .add_option(CreateCommandOption::new(
                    serenity::all::CommandOptionType::Attachment,
                    "attachment",
                    "Add an attachment here if applicable.",
                )),
        )
        .await
        {
            println!("{:?}", e);
        }

        let mut data = ctx.data.write().await;

        data.insert::<BonkBotKey>(BonkBotValue::new());

        let db = sqlx::postgres::PgPool::connect(
            &dotenv::var("DATABASE_URL").expect("Missing database URL."),
        )
        .await
        .expect("Failed to connect to databse.");

        let res = sqlx::migrate!("./migrations").run(&db).await;

        if let Err(e) = res {
            println!("{e}");
        };

        data.insert::<DatabaseKey>(DatabaseValue { db: Arc::new(db) });
    }

    async fn interaction_create(&self, ctx: serenity::all::Context, interaction: Interaction) {
        if let Interaction::Command(command) = interaction {
            let res: Result<()> = async {
                if command.data.name == "sgr" {
                    let args = &command.data.options;

                    if args.len() < 1 {
                        discord_commands::help(&ctx, &command).await?;
                        return Ok(());
                    }

                    let args = args
                        .get(0)
                        .context("Missing option.")?
                        .value
                        .as_str()
                        .context("Failed to convert option data to string.")?;

                    let args = args.split(' ').collect::<Vec<&str>>();

                    parse_command(&ctx, &command, &args).await?;
                }
                Ok(())
            }
            .await;

            if let Err(e) = res {
                //uncomment if you want command error messages to print to console.
                //println!("sgr slash command parse error: {e}");

                let message = CreateInteractionResponseMessage::new()
                    .content(format!("Command failed: {e}"))
                    .ephemeral(true);
                match command
                    .create_response(&ctx.http, CreateInteractionResponse::Message(message))
                    .await
                {
                    Err(_) => {
                        let _ = command
                            .edit_response(
                                &ctx.http,
                                EditInteractionResponse::new()
                                    .content(format!("Command failed: {e}")),
                            )
                            .await;
                    }
                    _ => (),
                }
            }
        }
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    let token = dotenv::var("DISCORD_TOKEN")?;

    let intents = GatewayIntents::GUILD_MESSAGES
        | GatewayIntents::DIRECT_MESSAGES
        | GatewayIntents::MESSAGE_CONTENT;

    let mut c = serenity::all::Client::builder(&token, intents)
        .event_handler(Handler)
        .await?;

    c.start().await?;

    Ok(())
}

async fn parse_command(
    ctx: &serenity::all::Context,
    interaction: &CommandInteraction,
    args: &Vec<&str>,
) -> Result<()> {
    match args.get(0) {
        Some(subcommand) => {
            let mut args = args.clone();
            args.remove(0);

            match *subcommand {
                "help" | "h" | "?" => discord_commands::help(ctx, interaction).await?,
                "ping" => discord_commands::ping(ctx, interaction, args).await?,
                "a" => discord_commands::a(ctx, interaction, args).await?,
                _ => {
                    let message = CreateInteractionResponseMessage::new()
                        .content(format!(
                            "Unknown command \"{}\". Run \"help\" for a list of commands.",
                            subcommand,
                        ))
                        .ephemeral(true);
                    let response = CreateInteractionResponse::Message(message);
                    interaction.create_response(&ctx.http, response).await?;
                }
            }
        }
        None => {
            discord_commands::help(ctx, interaction).await?;
        }
    }

    Ok(())
}
