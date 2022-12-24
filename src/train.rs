use chrono::{DateTime, Duration, Utc};
use eyre::{bail, ensure, eyre, WrapErr};
use sea_orm::{
    ActiveModelTrait, ConnectionTrait, DatabaseTransaction, DbConn, Iterable, NotSet, Set,
    TransactionTrait,
};
use serenity::async_trait;
use serenity::builder::{CreateApplicationCommandOption, CreateButton, CreateComponents};
use serenity::model::application::command::{CommandOptionType, CommandType};
use serenity::model::application::interaction::application_command::{
    ApplicationCommandInteraction, CommandDataOption, CommandDataOptionValue,
};
use serenity::model::prelude::component::{Button, ButtonStyle};
use serenity::model::prelude::*;
use serenity::prelude::*;
use sqlx::{query, query_as, SqliteExecutor};
use std::collections::HashMap;
use std::convert::AsRef;
use std::ops::Deref;
use strum::IntoEnumIterator;
use strum_macros::{AsRefStr, Display, EnumIter, EnumString, IntoStaticStr};
use url::Url;

use crate::interaction::Response;
use crate::model::train::{self, Status};
use crate::model::{monitor, Expac, World};

pub async fn init(database: &DbConn, ctx: &Context, guild: &Guild) -> eyre::Result<()> {
    let mut world_option = CreateApplicationCommandOption::default();
    world_option
        .name("world")
        .description("World server for this train")
        .kind(CommandOptionType::String);
    for world in World::iter() {
        world_option.add_string_choice(world, world.as_ref());
    }

    let mut expac_option = CreateApplicationCommandOption::default();
    expac_option
        .name("expac")
        .description("Expansion pack for the train")
        .kind(CommandOptionType::String);
    for expac in Expac::iter() {
        expac_option.add_string_choice(expac, expac.as_ref());
    }

    guild
        .id
        .create_application_command(&ctx.http, |command| {
            command
                .name("train")
                .description("Manage trains")
                .kind(CommandType::ChatInput)
                .default_member_permissions(Permissions::MANAGE_ROLES)
                .create_option(|option| {
                    option
                        .name("add")
                        .description("Add a new train to track")
                        .kind(CommandOptionType::SubCommand)
                        .add_sub_option(world_option.clone())
                        .add_sub_option(expac_option.clone())
                })
        })
        .await?;
    Ok(())
}

pub async fn handle_command(
    db: &DbConn,
    ctx: &Context,
    interaction: &ApplicationCommandInteraction,
) -> eyre::Result<Response> {
    ensure!(
        interaction.data.options.len() == 1,
        "Expected command to have 1 option"
    );
    let opt = &interaction.data.options[0];
    ensure!(
        opt.kind == CommandOptionType::SubCommand,
        "Expected command option to be a SubCommand"
    );
    match &*opt.name {
        "add" => handle_add(db, ctx, interaction, opt).await,
        name => bail!("Unexpected SubCommand name: {}", name),
    }
}

async fn handle_add(
    db: &DbConn,
    ctx: &Context,
    interaction: &ApplicationCommandInteraction,
    sub_command_option: &CommandDataOption,
) -> eyre::Result<Response> {
    let params: HashMap<_, _> = sub_command_option
        .options
        .iter()
        .map(|o| (&*o.name, o))
        .collect();
    let Some(CommandDataOptionValue::String(world)) =
        params.get("world").and_then(|o| o.resolved.as_ref())
    else {
        bail!("Invalid world: {:#?}", params.get("world"))
    };
    let world: World = world.parse()?;
    let Some(CommandDataOptionValue::String(expac)) =
        params.get("expac").and_then(|o| o.resolved.as_ref())
    else {
        bail!("Invalid expac: {:#?}", params.get("expac"))
    };
    let expac: Expac = expac.parse()?;
    let Some(guild_id) = interaction.guild_id else{
        bail! ("Command must be inside a server");
    };

    let tx = db.begin().await?;
    let resp = add_train(&tx, ctx, interaction, guild_id, expac, world).await?;
    tx.commit().await?;
    Ok(resp)
}

async fn add_train(
    tx: &impl ConnectionTrait,
    ctx: &Context,
    interaction: &ApplicationCommandInteraction,
    guild_id: GuildId,
    expac: Expac,
    world: World,
) -> eyre::Result<Response> {
    let train = train::find_or_create(tx, world, expac).await?;
    let m = interaction
        .channel_id
        .send_message(&ctx.http, |m| {
            m.add_embed(|embed| train.format_embed(embed))
            // .components(train.format_components)
        })
        .await?;
    let message = monitor::ActiveModel {
        id: NotSet,
        train_id: Set(train.id),
        channel_id: Set(interaction.channel_id.0),
        message_id: Set(m.id.0),
        ..Default::default()
    };
    message.insert(tx).await?;

    Ok(Response::Ephemeral(format!(
        "Success! A {} train on {} has been created!",
        expac, world,
    )))
}
