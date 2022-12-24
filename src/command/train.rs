use chrono::Utc;
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
#[poise::command(slash_command, subcommands("scout", "start", "done", "create_monitor"))]
pub async fn train(_ctx: super::Context<'_>) -> eyre::Result<()> {
    Err(eyre!("unsupported"))
}

/// Add a new hunt train monitor post
///
/// Creates a new message in the current channel to monitor a train's status.
/// All monitors for the same train (world + expac) share underlying data.
#[poise::command(slash_command, ephemeral)]
pub async fn create_monitor(
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
    let mut msg = ctx
        .channel_id()
        .send_message(ctx.discord(), |m| {
            m.content(format!("Initializing {} {} Train...", world, expac))
        })
        .await?;
    let monitor = monitor::ActiveModel {
        id: NotSet,
        train_id: Set(train.id),
        channel_id: Set(ctx.channel_id().0 as i64),
        message_id: Set(msg.id.0 as i64),
        ..Default::default()
    };
    monitor.insert(&tx).await?;
    // Commit before updating the message.
    tx.commit().await?;

    msg.edit(ctx.discord(), |m| {
        m.content("")
            .embed(|e| train.format_embed(e))
            .components(|c| train.format_components(c))
    })
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

#[must_use]
async fn refresh_monitors(ctx: super::Context<'_>, train: &train::Model) -> eyre::Result<()> {
    let db = &ctx.data().db;
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
        msg?.edit(ctx.discord(), |m| {
            m.embed(|e| train.format_embed(e))
                .components(|c| train.format_components(c))
        })
        .await?;
    }
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
    let active = self::train::ActiveModel {
        status: Set(Status::Scouted),
        scout_map: Set(map_link),
        ..self::train::ActiveModel::from(train.clone())
    };
    let train = active.update(db).await?;
    let reply = ctx
        .say(format!(
            "{} {} Train marked as scouted. Updating monitors...",
            expac, world
        ))
        .await?;

    refresh_monitors(ctx, &train).await?;
    reply
        .edit(ctx, |m| {
            m.content(format!(
                "Success! {} {} Train marked as scouted!",
                world, expac
            ))
        })
        .await?;
    Ok(())
}

/// Mark a train as being run.
#[poise::command(slash_command, ephemeral)]
pub async fn start(
    ctx: super::Context<'_>,
    #[description = "World server"] world: World,
    #[description = "Expansion"] expac: Expac,
    #[description = "Link to a map or a message with flag locations"] map_link: Option<String>,
) -> eyre::Result<()> {
    let db = &ctx.data().db;
    if let Some(ref url) = map_link {
        let _ = url.parse::<::url::Url>()?;
    }

    let train = self::train::find_or_create(db, world, expac).await?;
    let mut active = self::train::ActiveModel {
        status: Set(Status::Running),
        last_run: Set(None),
        ..self::train::ActiveModel::from(train.clone())
    };
    if let Some(ref url) = map_link {
        active.scout_map = Set(map_link)
    }
    let train = active.update(db).await?;

    let reply = ctx
        .say(format!(
            "{} {} Train marked as running. Updating monitors...",
            expac, world
        ))
        .await?;

    refresh_monitors(ctx, &train).await?;
    reply
        .edit(ctx, |m| {
            m.content(format!(
                "Success! {} {} Train marked as running!",
                expac, world
            ))
        })
        .await?;

    Ok(())
}

/// Mark a train as being complete.
#[poise::command(slash_command, ephemeral)]
pub async fn done(
    ctx: super::Context<'_>,
    #[description = "World server"] world: World,
    #[description = "Expansion"] expac: Expac,
    #[description = "Discord timestamp when it was finished, defaults to now"]
    completion_time: Option<super::argument::Timestamp>,
) -> eyre::Result<()> {
    let db = &ctx.data().db;

    let train = self::train::find_or_create(db, world, expac).await?;
    let completion_time = completion_time.unwrap_or_else(|| super::argument::Timestamp(Utc::now()));
    let mut active = self::train::ActiveModel {
        status: Set(Status::Waiting),
        scout_map: Set(None),
        last_run: Set(Some(completion_time.0)),
        ..self::train::ActiveModel::from(train.clone())
    };
    let train = active.update(db).await?;

    let reply = ctx
        .say(format!(
            "{} {} Train marked as complete at {}. Updating monitors...",
            expac, world, completion_time
        ))
        .await?;

    refresh_monitors(ctx, &train).await?;
    reply
        .edit(ctx, |m| {
            m.content(format!(
                "Success! {} {} Train marked as complete at {}!",
                expac, world, completion_time,
            ))
        })
        .await?;

    Ok(())
}
