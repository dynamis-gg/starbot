use eyre::{bail, eyre};
use poise::command;
use sea_orm::{ActiveModelTrait, NotSet, Set, TransactionTrait};

use crate::model::{monitor, train, Expac, World};

/// Hunt train commands.
#[poise::command(slash_command, subcommands("create"))]
pub async fn train(_ctx: super::Context<'_>) -> eyre::Result<()> {
    Err(eyre!("unsupported"))
}

/// Add a new hunt train monitor.
///
/// Creates a new message in the current channel to monitor a train's status.
/// All monitors for the same train (world + expac) share underlying data.
#[poise::command(
    slash_command,
    // required_bot_permissions = "SEND_MESSAGES",
    // required_permissions = "MANAGE_MESSAGES",
    ephemeral
)]
pub async fn create(ctx: super::Context<'_>, world: World, expac: Expac) -> eyre::Result<()> {
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
