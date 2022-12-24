use eyre::{bail, eyre};
use poise::{
    command,
    serenity_prelude::{ChannelId, MessageId},
};
use sea_orm::{ActiveModelTrait, ModelTrait, NotSet, Set, TransactionTrait};

use crate::model::{
    monitor,
    train::{self, Status},
    Expac, World,
};

/// Hunt train commands.
#[poise::command(slash_command, subcommands("create", "scout"))]
pub async fn train(_ctx: super::Context<'_>) -> eyre::Result<()> {
    Err(eyre!("unsupported"))
}

/// Add a new hunt train monitor
///
/// Creates a new message in the current channel to monitor a train's status.
/// All monitors for the same train (world + expac) share underlying data.
#[poise::command(
    slash_command,
    // required_bot_permissions = "SEND_MESSAGES",
    // required_permissions = "MANAGE_MESSAGES",
    ephemeral
)]
pub async fn create(
    ctx: super::Context<'_>,
    #[description = "World server"] world: World,
    #[description = "Expansion"] expac: Expac,
) -> eyre::Result<()> {
    if ctx.guild_id() != Some(ctx.data().train_guild_id) {
        bail!("Not allowed in this guild/in DM");
    }
    let tx = ctx.data().db.begin().await?;

    let train = train::find_or_create(&tx, world, expac).await?;
    // Send an "initializing" message first so that we can get its ID
    // and commit it before we make it look like the command succeeded.
    let msg = ctx
        .send(|m| {
            m.content(format!("Initializing {} {} Train...", world, expac))
                .ephemeral(false)
        })
        .await?;
    let monitor = monitor::ActiveModel {
        id: NotSet,
        train_id: Set(train.id),
        channel_id: Set(ctx.channel_id().0 as i64),
        message_id: Set(msg.message().await?.id.0 as i64),
        ..Default::default()
    };
    monitor.insert(&tx).await?;
    // Commit before updating the message.
    tx.commit().await?;

    msg.edit(
        ctx,
        |m| m.content("").embed(|e| train.format_embed(e)), // .components(train.format_components)
    )
    .await?;

    ctx.send(|m| {
        m.content(format!(
            "Success! A monitor for the {} train on {} has been created!",
            expac, world,
        ))
    })
    .await?;

    Ok(())
}

/// Mark a train as scouted
#[poise::command(slash_command, ephemeral)]
pub async fn scout(
    ctx: super::Context<'_>,
    #[description = "World server"] world: World,
    #[description = "Expansion"] expac: Expac,
    #[description = "Link to a map or a message with flag locations"] map_link: Option<String>,
) -> eyre::Result<()> {
    if let Some(ref url) = map_link {
        let _ = url.parse::<::url::Url>()?;
    }

    let db = &ctx.data().db;
    let train = self::train::find_or_create(db, world, expac).await?;
    if train.status == Status::Scouted {
        bail!("That train is already scouted.");
    }
    let active = self::train::ActiveModel {
        status: Set(Status::Scouted),
        scout_map: Set(map_link),
        ..self::train::ActiveModel::from(train.clone())
    };
    let train = active.update(db).await?;
    let reply = ctx
        .say("Train marked as scouted. Updating monitors...")
        .await?;

    for monitor in train.find_related(self::monitor::Entity).all(db).await? {
        let channel_id = ChannelId(monitor.channel_id as u64);
        let message_id = MessageId(monitor.message_id as u64);
        let msg = channel_id.message(ctx.discord(), message_id).await;
        if let Err(serenity::Error::Http(ref http)) = msg {
            if let serenity::http::error::Error::UnsuccessfulRequest(
                serenity::http::error::ErrorResponse {
                    status_code:
                        poise::serenity_prelude::StatusCode::FORBIDDEN
                        | poise::serenity_prelude::StatusCode::NOT_FOUND,
                    ..
                },
            ) = **http
            {
                // The message must have been deleted or we no longer have permission to find it.
                // Remove from our DB, logging but not failing on error.
                if let Err(e) = monitor.delete(db).await {
                    eprintln!("Warning: Unable to delete stale message from our DB: {}", e);
                }
                continue;
            }
        }
        msg?.edit(ctx.discord(), |m| m.embed(|e| train.format_embed(e)))
            .await?;
    }
    reply
        .edit(ctx, |m| m.content("Success! Train marked as scouted!"))
        .await?;
    Ok(())
}
