#![allow(unused)]

mod command;
mod model;

use eyre::WrapErr;
use poise::serenity_prelude::UserId;
use sea_orm::{Database, DbConn};
use serenity::async_trait;
use serenity::model::application::interaction::{
    Interaction, InteractionResponseType, MessageFlags,
};
use serenity::model::gateway::Ready;
use serenity::model::guild::Guild;
use serenity::model::id::GuildId;
use serenity::prelude::*;
use std::env;

#[tokio::main]
async fn main() -> eyre::Result<()> {
    env_logger::init();

    // Configure the client with your Discord bot token in the environment.
    let token = env::var("DISCORD_TOKEN")
        .wrap_err("Expected a token in the environment variable DISCORD_TOKEN")?;
    let app_id: u64 = env::var("DISCORD_APPLICATION_ID")
        .map_err(eyre::Report::new)
        .and_then(|s| s.parse().map_err(eyre::Report::new))
        .wrap_err("Expected a token in the environment variable DISCORD_APPLICATION_ID")?;
    let train_guild_id = env::var("TRAIN_GUILD_ID")
        .map_err(eyre::Report::new)
        .and_then(|s| s.parse().map_err(eyre::Report::new))
        .wrap_err("Expected a token in the environment variable TRAIN_GUILD_ID")
        .map(GuildId)?;
    let db_url = env::var("DATABASE_URL")
        .wrap_err("Expected a database URL in the environment variable DATABASE_URL")?;
    let owner_id = env::var("OWNER_ID")
        .map_err(eyre::Report::new)
        .and_then(|s| s.parse().map_err(eyre::Report::new))
        .wrap_err("Expected an owner ID in the environment variable OWNER_ID")
        .map(UserId)?;

    let db = Database::connect(&*db_url).await?;

    // Build our client.
    let data = command::Data { db, train_guild_id };
    let framework = poise::Framework::builder()
        .options(poise::FrameworkOptions {
            commands: command::all(),
            owners: std::collections::HashSet::from([owner_id]),
            prefix_options: poise::PrefixFrameworkOptions {
                prefix: Some("!".to_owned()),
                mention_as_prefix: true,
                case_insensitive_commands: true,
                ..Default::default()
            },
            ..Default::default()
        })
        .token(token)
        .intents(GatewayIntents::DIRECT_MESSAGES)
        .user_data_setup(move |ctx, _r, framework| {
            Box::pin(async move {
                eprintln!("Initializing...");
                let channel = owner_id.create_dm_channel(ctx).await?;
                channel
                    .send_message(ctx, |m| m.content("Greetings, owner! I wish only to hear your words, share your feelings, know your thoughts."))
                    .await?;
                data.train_guild_id
                    .set_application_commands(&ctx.http, |b| {
                        *b = poise::builtins::create_application_commands(
                            &*framework.options().commands,
                        );
                        b
                    })
                    .await?;
                eprintln!(
                    "Set application commands for guild {}",
                    data.train_guild_id.0
                );
                Ok(data)
            })
        });

    framework.run().await?;
    Ok(())
}
