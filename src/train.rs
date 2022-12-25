use chrono::{Duration, Utc};
use futures::{stream::FuturesUnordered, StreamExt};
use poise::serenity_prelude::{ChannelId, MessageId};
use sea_orm::{ColumnTrait, EntityTrait, ModelTrait, QueryFilter};
use std::collections::{BTreeSet, HashMap};
use std::convert::AsRef;
use std::fmt::Write;

use crate::command::Context;
use entity::{
    dashboard, monitor,
    train::{self, Status},
    Expac, World,
};

pub mod command;

async fn refresh_dashboard(
    ctx: Context<'_>,
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
        m.content("Where a time is present, it indicates force (cap) time.")
            .embed(|e| {
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
                            use train::Model as Train;
                            let status = train.map_or(Status::Unknown, |t| t.status);

                            let text = match (status, train) {
                                (
                                    Status::Scouted,
                                    Some(Train {
                                        scout_map: Some(url),
                                        ..
                                    }),
                                ) => format!("[Scouted]({})", url),
                                (Status::Scouted, _) => "Scouted".to_owned(),
                                (
                                    Status::Waiting,
                                    Some(Train {
                                        last_run: Some(last_run),
                                        ..
                                    }),
                                ) => format!(
                                    "<t:{}:R>",
                                    (*last_run + Duration::hours(6)).timestamp()
                                ),
                                (Status::Running, _) => "**Running**".to_owned(),
                                _ => "Unknown".to_owned(),
                            };
                            writeln!(col, "{} {}", status.emoji(), text,).unwrap();
                        }
                        (world, col, true)
                    }))
            })
    })
    .await?;
    Ok(())
}

async fn refresh_monitor(
    ctx: Context<'_>,
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
        m.content("")
            .embed(|e| train.format_embed(e))
            .components(|c| train.format_components(c))
    })
    .await?;
    Ok(())
}

// Prints errors to stderr and reports only success/failure.
pub async fn refresh_dashboards(ctx: Context<'_>) -> bool {
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
pub async fn refresh_monitors(ctx: Context<'_>, train: &train::Model) -> bool {
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
