use chrono::{Duration, Utc};
use eyre::{bail, eyre};
use sea_orm::{
    ActiveModelTrait, ColumnTrait, EntityTrait, NotSet, QueryFilter, Set, TransactionTrait,
};

use super::{refresh_dashboard, refresh_dashboards, refresh_monitor, refresh_monitors};
use crate::command::{argument, Context};
use entity::{
    dashboard, monitor, train,
    Expac, World,
};

/// Hunt train commands.
#[poise::command(
    slash_command,
    subcommands("scout", "start", "done", "create_monitor", "create_dashboard")
)]
pub async fn train(_ctx: Context<'_>) -> eyre::Result<()> {
    Err(eyre!("unsupported"))
}

/// Add a new hunt train monitor dashboard
#[poise::command(slash_command)]
pub async fn create_dashboard(ctx: Context<'_>) -> eyre::Result<()> {
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
    ctx: Context<'_>,
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
    let monitor = monitor.insert(&tx).await?;
    // Commit before updating the message.
    tx.commit().await?;

    refresh_monitor(ctx, monitor, &train).await?;

    Ok(())
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
    ctx: Context<'_>,
    #[description = "World server"] world: World,
    #[description = "Expansion"] expac: Expac,
    #[description = "Link to a map or a message with flag locations (leave blank to clear existing map)"] map_link: Option<String>,
) -> eyre::Result<()> {
    if let Some(ref url) = map_link {
        let _ = url.parse::<::url::Url>()?;
    }

    let db = &ctx.data().db;
    let mut train = train::find_or_create(db, world, expac).await?;
    train.scout(map_link);
    let train = train::ActiveModel::from(train).update(db).await?;

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
    ctx: Context<'_>,
    #[description = "World server"] world: World,
    #[description = "Expansion"] expac: Expac,
    #[description = "Link to a map or a message with flag locations"] map_link: Option<String>,
) -> eyre::Result<()> {
    if let Some(ref url) = map_link {
        let _ = url.parse::<::url::Url>()?;
    }

    let db = &ctx.data().db;
    let mut train = train::find_or_create(db, world, expac).await?;
    // This isn't ideal, but there is no better way to handle this
    // without either putting too much logic in the Model or running
    // into ownership issues.
    if map_link.is_some() {
        train.scout_map = map_link;
    }
    train.start();
    let train = train::ActiveModel::from(train).update(db).await?;

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
    ctx: Context<'_>,
    #[description = "World server"] world: World,
    #[description = "Expansion"] expac: Expac,
    #[description = "Discord timestamp when it was finished, defaults to now"]
    completion_time: Option<argument::Timestamp>,
    #[description = "Discord timestamp when it will be forced, mutually exclusive with `completion_time`"]
    force_time: Option<argument::Timestamp>,
) -> eyre::Result<()> {
    let last_run_time = match (completion_time, force_time) {
        (Some(_), Some(_)) => bail!("Cannot provide both completion_time and force_time"),
        (Some(completed), _) => completed.0,
        (_, Some(force)) => force.0 - Duration::hours(6),
        _ => Utc::now(),
    };

    let db = &ctx.data().db;
    let mut train = train::find_or_create(db, world, expac).await?;
    train.done(last_run_time);
    let train = train::ActiveModel::from(train).update(db).await?;

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
