#![allow(unused)]

mod train;

use std::env;

use eyre::WrapErr;
use serenity::async_trait;
use serenity::model::application::interaction::{Interaction, InteractionResponseType};
use serenity::model::gateway::Ready;
use serenity::model::guild::Guild;
use serenity::model::id::GuildId;
use serenity::prelude::*;

struct Bot {
    database: sqlx::SqlitePool,
    train_guild_id: GuildId,
}

#[async_trait]
impl EventHandler for Bot {
    async fn interaction_create(&self, ctx: Context, interaction: Interaction) {
        if let Interaction::ApplicationCommand(command) = interaction {
            let result = match command.data.name.as_str() {
                "train" => train::handle_command(&self.database, &ctx, &command).await,
                name => Err(eyre::eyre!("Unknown command name: {}", name)),
            };

            let content = match result {
                Ok(s) => s,
                Err(e) => {
                    format!("Error occurred processing command: {}", e)
                }
            };

            if let Err(why) = command
                .create_interaction_response(&ctx.http, |response| {
                    response
                        .kind(InteractionResponseType::ChannelMessageWithSource)
                        .interaction_response_data(|message| message.content(content))
                })
                .await
            {
                eprintln!("Error responding to slash command: {}", why);
            }
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
            println!("Initializing train commands");
            train::init(&self.database, &ctx, &guild).await;
        } else {
            println!("Not the train guild; skipping train commands")
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
    let train_guild_id: u64 = env::var("TRAIN_GUILD_ID")
        .map_err(eyre::Report::new)
        .and_then(|s| s.parse().map_err(eyre::Report::new))
        .expect("Expected a token in the environment variable TRAIN_GUILD_ID");

    // Initiate a connection to the database file, creating the file if required.
    let database = sqlx::sqlite::SqlitePoolOptions::new()
        .max_connections(5)
        .connect_with(
            sqlx::sqlite::SqliteConnectOptions::new()
                .filename("database.sqlite")
                .create_if_missing(true),
        )
        .await
        .wrap_err("Couldn't connect to database")?;

    // Run migrations, which updates the database's schema to the latest version.
    sqlx::migrate!("./migrations")
        .run(&database)
        .await
        .wrap_err("Couldn't run database migrations")?;

    // Build our client.
    let mut client = Client::builder(token, GatewayIntents::empty())
        .event_handler(Bot {
            database,
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
