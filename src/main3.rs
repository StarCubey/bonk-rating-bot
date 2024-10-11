mod bonk_bot;
mod discord_commands;

use anyhow::{Context, Result};
use dotenv;
use serenity::{
    all::{
        ActivityData, Command, CommandInteraction, CreateCommand, CreateCommandOption,
        CreateInteractionResponse, CreateInteractionResponseMessage, EventHandler, GatewayIntents,
        Interaction, Ready,
    },
    async_trait,
};

struct Handler;

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
                .add_option(CreateCommandOption::new(
                    serenity::all::CommandOptionType::String,
                    "command",
                    "Type \"help\" for a list of commands.",
                )),
        )
        .await
        {
            println!("{:?}", e);
        }
    }

    async fn interaction_create(&self, ctx: serenity::all::Context, interaction: Interaction) {
        let res: Result<()> = async {
            if let Interaction::Command(command) = interaction {
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
            }
            Ok(())
        }
        .await;

        if let Err(e) = res {
            println!("sgr slash command parse error: {e}");
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
                "help" | "h" => discord_commands::help(ctx, interaction).await?,
                "open" | "o" => discord_commands::open(ctx, interaction, args).await?,
                "shutdown" | "sd" => discord_commands::shutdown(ctx, interaction).await?,
                "ping" => discord_commands::ping(ctx, interaction, args).await?,
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
