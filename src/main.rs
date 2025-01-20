mod bonk_bot;
mod discord_commands;

use std::fmt::Display;

use anyhow::{Context, Result};
use bonk_bot::{BonkBotKey, BonkBotValue};
use dotenv;
use serde::Deserialize;
use serenity::{
    all::{
        ActivityData, Command, CommandInteraction, CreateCommand, CreateCommandOption,
        CreateInteractionResponse, CreateInteractionResponseMessage, EditInteractionResponse,
        EventHandler, GatewayIntents, Interaction, Ready,
    },
    async_trait,
    prelude::TypeMapKey,
};
use sqlx::Row;

struct Handler;

pub struct ConfigKey;

impl TypeMapKey for ConfigKey {
    type Value = Config;
}

#[derive(Deserialize, Clone)]
pub struct Config {
    bot_admin: Vec<u64>,
}

pub struct DatabaseKey;

impl TypeMapKey for DatabaseKey {
    type Value = DatabaseValue;
}

pub struct DatabaseValue {
    db: sqlx::Pool<sqlx::Postgres>,
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

        if let Ok(config) = tokio::fs::read_to_string("config.toml").await {
            if let Ok(config) = toml::de::from_str(config.as_str()) {
                data.insert::<ConfigKey>(config);
            }
        }

        let db = sqlx::postgres::PgPool::connect(
            &dotenv::var("DATABASE_URL").expect("Missing database URL."),
        )
        .await
        .expect("Failed to connect to databse.");

        let res = sqlx::migrate!("./migrations").run(&db).await;

        if let Err(e) = res {
            println!("{e}");
        };

        data.insert::<DatabaseKey>(DatabaseValue { db });
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

    //c.start().await?; TODO

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
