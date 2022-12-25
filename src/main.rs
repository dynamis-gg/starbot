mod command;

use clap::Parser;
use poise::serenity_prelude::UserId;
use sea_orm::Database;
use sea_orm_migration::MigratorTrait;
use serenity::model::id::GuildId;
use serenity::prelude::*;

#[derive(Parser, Debug)]
#[command()]
struct Args {
    #[arg(long, env = "STARBOT_DISCORD_TOKEN", required = true)]
    token: String,
    #[arg(long, env = "STARBOT_TRAIN_GUILD_ID", required = true)]
    train_guild_id: u64,
    #[arg(long, env = "DATABASE_URL", required = true)]
    db_url: url::Url,
    #[arg(long, env = "STARBOT_OWNER_ID", required = true)]
    owner_id: u64,
}

#[tokio::main]
async fn main() -> eyre::Result<()> {
    env_logger::init();

    let args = Args::parse();

    let db = Database::connect(args.db_url.as_ref()).await?;
    migration::Migrator::up(&db, None).await?;

    // Build our client.
    let data = command::Data {
        db,
        train_guild_id: GuildId(args.train_guild_id),
    };
    let framework = poise::Framework::builder()
        .options(poise::FrameworkOptions {
            commands: command::all(),
            owners: std::collections::HashSet::from([UserId(args.owner_id)]),
            prefix_options: poise::PrefixFrameworkOptions {
                prefix: Some("!".to_owned()),
                mention_as_prefix: true,
                case_insensitive_commands: true,
                ..Default::default()
            },
            ..Default::default()
        })
        .token(args.token)
        .intents(GatewayIntents::DIRECT_MESSAGES)
        .user_data_setup(move |ctx, _r, framework| {
            Box::pin(async move {
                eprintln!("Initializing...");
                let channel = UserId(args.owner_id).create_dm_channel(ctx).await?;
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
