#![allow(unused)]

mod interaction;
mod model;
mod train;

use eyre::WrapErr;
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

use interaction::Response;

struct Bot {
    db: DbConn,
    train_guild_id: GuildId,
}

#[async_trait]
impl EventHandler for Bot {
    async fn interaction_create(&self, ctx: Context, interaction: Interaction) {
        match interaction {
            Interaction::ApplicationCommand(command) => {
                let result = match command.data.name.as_str() {
                    "train" => train::handle_command(&self.db, &ctx, &command).await,
                    name => Err(eyre::eyre!("Unknown command name: {}", name)),
                };

                let content = match result {
                    Ok(Response::Ephemeral(s)) => s,
                    Err(e) => {
                        format!("Error occurred processing command: {}", e)
                    }
                };

                if let Err(why) = command
                    .create_interaction_response(&ctx.http, |response| {
                        response
                            .kind(InteractionResponseType::ChannelMessageWithSource)
                            .interaction_response_data(|message| {
                                message.content(content).flags(MessageFlags::EPHEMERAL)
                            })
                    })
                    .await
                {
                    eprintln!("Error responding to slash command: {}", why);
                }
            }
            Interaction::MessageComponent(_) => {}
            _ => {}
        }
    }

    async fn guild_create(&self, ctx: Context, guild: Guild, is_new: bool) {
        println!(
            "Joined {} guild {} ({})",
            if is_new { "new" } else { "old" },
            guild.id,
            guild.name
        );
        if guild.id == self.train_guild_id {
            eprintln!("Initializing train commands");
            if let Err(e) = train::init(&self.db, &ctx, &guild).await {
                eprintln!("Error initializing train commands: {}", e);
            }
        } else {
            eprintln!("Not the train guild; skipping train commands")
        }
    }

    async fn ready(&self, ctx: Context, ready: Ready) {
        println!("{} is connected!", ready.user.name);
    }
}

#[tokio::main]
async fn main() -> eyre::Result<()> {
    // Configure the client with your Discord bot token in the environment.
    let token = env::var("DISCORD_TOKEN")
        .expect("Expected a token in the environment variable DISCORD_TOKEN");
    let app_id = env::var("DISCORD_APPLICATION_ID")
        .map_err(eyre::Report::new)
        .and_then(|s| s.parse().map_err(eyre::Report::new))
        .expect("Expected a token in the environment variable DISCORD_APPLICATION_ID");
    let train_guild_id: u64 = env::var("TRAIN_GUILD_ID")
        .map_err(eyre::Report::new)
        .and_then(|s| s.parse().map_err(eyre::Report::new))
        .expect("Expected a token in the environment variable TRAIN_GUILD_ID");
    let db_url = env::var("DATABASE_URL")
        .expect("Expected a database URL in the environment variable DATABASE_URL");

    let db = Database::connect(&*db_url).await?;

    // Build our client.
    let mut client = Client::builder(token, GatewayIntents::GUILDS)
        .application_id(app_id)
        .event_handler(Bot {
            db,
            train_guild_id: GuildId(train_guild_id),
        })
        .await
        .wrap_err("Error creating client")?;

    // Finally, start a single shard, and start listening to events.
    //
    // Shards will automatically attempt to reconnect, and will perform
    // exponential backoff until it reconnects.
    Ok(client.start().await?)
}
