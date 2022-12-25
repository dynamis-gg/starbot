use chrono::{Duration, Utc};
use eyre::{bail, eyre};
use futures::{stream::FuturesUnordered, StreamExt};
use poise::serenity_prelude::{ChannelId, MessageId};
use sea_orm::{
    ActiveModelTrait, ColumnTrait, EntityTrait, ModelTrait, NotSet, QueryFilter, Set,
    TransactionTrait,
};
use std::collections::{BTreeSet, HashMap};
use std::convert::AsRef;
use std::fmt::Write;

use entity::{
    dashboard, monitor,
    train::{self, Status},
    Expac, World,
};

/// Hunt train commands.
#[poise::command(
    slash_command,
    subcommands("scout", "start", "done", "create_monitor", "create_dashboard")
)]
pub async fn train(_ctx: super::Context<'_>) -> eyre::Result<()> {
    Err(eyre!("unsupported"))
}

/// Add a new hunt train monitor dashboard
#[poise::command(slash_command)]
pub async fn create_dashboard(ctx: super::Context<'_>) -> eyre::Result<()> {
    if ctx.guild_id() != Some(ctx.data().train_guild_id) {
        bail!("Not allowed in this guild/in DM");
    }
    let tx = ctx.data().db.begin().await?;

    // Send an "initializing" message first so that we can get its ID
    // and commit it before we make it look like the command succeeded.
    let msg = ctx
        .say("Initializing dashboard...")
        .await?
        .into_message()
        .await?;

    let dashboard = dashboard::ActiveModel {
        id: NotSet,
        channel_id: Set(ctx.channel_id().0 as i64),
        message_id: Set(msg.id.0 as i64),
        ..Default::default()
    };
    let dashboard = dashboard.insert(&tx).await?;
    let trains = train::Entity::find()
        .filter(train::Column::World.ne(World::Testing))
        .all(&tx)
        .await?;

    // Commit before updating the message.
    tx.commit().await?;

    refresh_dashboard(ctx, dashboard, trains).await
}

/// Add a new hunt train monitor post
///
/// Creates a new message in the current channel to monitor a train's status.
/// All monitors for the same train (world + expac) share underlying data.
#[poise::command(slash_command)]
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
        .say(format!(
            "Initializing monitor for {} {} Train...",
            world, expac
        ))
        .await?
        .into_message()
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

    Ok(())
}

async fn refresh_dashboard(
    ctx: super::Context<'_>,
    dashboard: dashboard::Model,
    trains: impl AsRef<[train::Model]>,
) -> eyre::Result<()> {
    let db = &ctx.data().db;
    let channel_id = ChannelId(dashboard.channel_id as u64);
    let message_id = MessageId(dashboard.message_id as u64);
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
            if let Err(e) = dashboard.delete(db).await {
                eprintln!("Warning: Unable to delete stale message from our DB: {}", e);
            }
            return Ok(());
        }
    }

    let mut expacs = BTreeSet::new();
    let mut worlds = BTreeSet::new();
    let mut train_map = HashMap::<(Expac, World), &train::Model>::new();
    for t in trains.as_ref() {
        train_map.insert((t.expac, t.world), t);
        expacs.insert(t.expac);
        worlds.insert(t.world);
    }

    msg?.edit(ctx.discord(), |m| {
        m.content("").embed(|e| {
            e.title("Train Dashboard")
                .timestamp(Utc::now())
                .field(
                    "Expansion",
                    format!(
                        "__{}__",
                        expacs
                            .iter()
                            .rev()
                            .map(Expac::as_ref)
                            .collect::<Vec<_>>()
                            .join("__\n__")
                    ),
                    true,
                )
                .fields(worlds.into_iter().map(|world| {
                    let mut col = String::new();
                    for &expac in expacs.iter().rev() {
                        let train = train_map.get(&(expac, world));
                        let status = train.map_or(Status::Unknown, |t| t.status);
                        writeln!(
                            col,
                            "{} {}",
                            status.emoji(),
                            match status {
                                Status::Unknown => "Unknown".to_owned(),
                                Status::Scouted =>
                                    if let Some(train::Model {
                                        scout_map: Some(url),
                                        ..
                                    }) = train
                                    {
                                        format!("[Scouted]({})", url)
                                    } else {
                                        "Scouted".to_owned()
                                    },
                                Status::Waiting =>
                                    if let Some(train::Model {
                                        last_run: Some(last_run),
                                        ..
                                    }) = train
                                    {
                                        format!(
                                            "<t:{}:R>",
                                            (*last_run + Duration::hours(6)).timestamp()
                                        )
                                    } else {
                                        "Waiting".to_owned()
                                    },
                                Status::Running => "Running".to_owned(),
                            }
                        )
                        .unwrap();
                    }
                    (world, col, true)
                }))
        })
    })
    .await?;
    Ok(())
}

async fn refresh_monitor(
    ctx: super::Context<'_>,
    monitor: monitor::Model,
    train: &train::Model,
) -> eyre::Result<()> {
    let db = &ctx.data().db;
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
            return Ok(());
        }
    }
    msg?.edit(ctx.discord(), |m| {
        m.embed(|e| train.format_embed(e))
            .components(|c| train.format_components(c))
    })
    .await?;
    Ok(())
}

// Prints errors to stderr and reports only success/failure.
async fn refresh_dashboards(ctx: super::Context<'_>) -> bool {
    let db = &ctx.data().db;
    let dashboards = match dashboard::Entity::find().all(db).await {
        Ok(d) => d,
        Err(e) => {
            eprintln!("Warning: Unable to retrieve dashboards from DB: {}", e);
            return false;
        }
    };
    let trains = match train::Entity::find()
        .filter(train::Column::World.ne(World::Testing))
        .all(db)
        .await
    {
        Ok(t) => t,
        Err(e) => {
            eprintln!("Warning: Unable to retrieve trains from DB: {}", e);
            return false;
        }
    };

    let tasks: FuturesUnordered<_> = dashboards
        .into_iter()
        .map(|dashboard| refresh_dashboard(ctx, dashboard, &trains))
        .collect();
    tasks
        .all(|r| async move {
            match r {
                Ok(()) => true,
                Err(e) => {
                    eprintln!("Warning: Unable to update dashboard: {}", e);
                    false
                }
            }
        })
        .await
}

// Prints errors to stderr and reports only success/failure.
async fn refresh_monitors(ctx: super::Context<'_>, train: &train::Model) -> bool {
    let db = &ctx.data().db;
    let monitors = match train.find_related(self::monitor::Entity).all(db).await {
        Ok(m) => m,
        Err(e) => {
            eprintln!("Warning: Unable to retrieve monitors from DB: {}", e);
            return false;
        }
    };

    let tasks: FuturesUnordered<_> = monitors
        .into_iter()
        .map(|monitor| refresh_monitor(ctx, monitor, train))
        .collect();
    tasks
        .all(|r| async move {
            match r {
                Ok(()) => true,
                Err(e) => {
                    eprintln!("Warning: Unable to update monitor: {}", e);
                    false
                }
            }
        })
        .await
}

fn monitor_msg(base: String, success: bool) -> String {
    if success {
        format!("{}.", base)
    } else {
        format!(
            "Error: {}, but not all monitor posts could be updated.",
            base
        )
    }
}

/// Mark a train as scouted
#[poise::command(slash_command)]
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

    ctx.defer().await?;
    let success = refresh_monitors(ctx, &train).await && refresh_dashboards(ctx).await;
    let scout_text = match train.scout_map {
        Some(url) => format!("[scouted]({})", url),
        None => "scouted".to_owned(),
    };
    ctx.say(monitor_msg(
        format!("{} {} Train has been {}", world, expac, scout_text),
        success,
    ))
    .await?;

    Ok(())
}

/// Mark a train as being run.
#[poise::command(slash_command)]
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
    if map_link.is_some() {
        active.scout_map = Set(map_link)
    }
    let train = active.update(db).await?;

    ctx.defer().await?;
    let success = refresh_monitors(ctx, &train).await && refresh_dashboards(ctx).await;
    ctx.say(monitor_msg(
        format!("{} {} Train is now running", world, expac),
        success,
    ))
    .await?;

    Ok(())
}

/// Mark a train as being complete.
#[poise::command(slash_command)]
pub async fn done(
    ctx: super::Context<'_>,
    #[description = "World server"] world: World,
    #[description = "Expansion"] expac: Expac,
    #[description = "Discord timestamp when it was finished, defaults to now"]
    completion_time: Option<super::argument::Timestamp>,
    #[description = "Discord timestamp when it will be forced, mutually exclusive with `completion_time`"]
    force_time: Option<super::argument::Timestamp>,
) -> eyre::Result<()> {
    let db = &ctx.data().db;

    let train = self::train::find_or_create(db, world, expac).await?;
    let last_run_time = match (completion_time, force_time) {
        (Some(_), Some(_)) => bail!("Cannot provide both completion_time and force_time"),
        (Some(completed), _) => completed.0,
        (_, Some(force)) => force.0 - Duration::hours(6),
        _ => Utc::now(),
    };

    let active = self::train::ActiveModel {
        status: Set(Status::Waiting),
        scout_map: Set(None),
        last_run: Set(Some(last_run_time)),
        ..self::train::ActiveModel::from(train.clone())
    };
    let train = active.update(db).await?;

    ctx.defer().await?;
    let success = refresh_monitors(ctx, &train).await && refresh_dashboards(ctx).await;
    ctx.say(monitor_msg(
        format!(
            "{} {} Train completed at <t:{}:f>",
            world,
            expac,
            last_run_time.timestamp()
        ),
        success,
    ))
    .await?;

    Ok(())
}
