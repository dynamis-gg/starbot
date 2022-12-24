mod model;

use chrono::{DateTime, Duration, Utc};
use eyre::{bail, ensure, eyre, WrapErr};
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
use strum::IntoEnumIterator;
use strum_macros::{AsRefStr, Display, EnumIter, EnumString, IntoStaticStr};
use url::Url;

use crate::interaction::Response;
use model::{Expac, Status, Train, World};

pub async fn init(database: &sqlx::SqlitePool, ctx: &Context, guild: &Guild) -> eyre::Result<()> {
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
                .create_option(|option| {
                    option
                        .name("remove")
                        .description("Remove a train")
                        .kind(CommandOptionType::SubCommand)
                        .add_sub_option(world_option)
                        .add_sub_option(expac_option)
                })
        })
        .await?;
    Ok(())
}

pub async fn handle_command(
    database: &sqlx::SqlitePool,
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
    let response = match &*opt.name {
        "add" => handle_add(database, ctx, interaction, opt).await,
        "remove" => handle_remove(database, ctx, interaction, opt).await,
        name => bail!("Unexpected SubCommand name: {}", name),
    }?;
    Ok(Response::Ephmeral(response))
}

async fn handle_add(
    database: &sqlx::SqlitePool,
    ctx: &Context,
    interaction: &ApplicationCommandInteraction,
    sub_command_option: &CommandDataOption,
) -> eyre::Result<String> {
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
    let Some(CommandDataOptionValue::String(expac)) =
        params.get("expac").and_then(|o| o.resolved.as_ref())
    else {
        bail!("Invalid expac: {:#?}", params.get("expac"))
    };
    let Some(guild_id) = interaction.guild_id else{
        bail! ("Command must be inside a server");
    };

    let mut tx = database.begin().await?;
    let ok = add_train(&mut tx, ctx, interaction, guild_id, expac, world).await?;
    tx.commit().await?;
    Ok(ok)
}

async fn handle_remove(
    database: &sqlx::SqlitePool,
    ctx: &Context,
    interaction: &ApplicationCommandInteraction,
    sub_command_option: &CommandDataOption,
) -> eyre::Result<String> {
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
    let Some(CommandDataOptionValue::String(expac)) =
        params.get("expac").and_then(|o| o.resolved.as_ref())
    else {
        bail!("Invalid expac: {:#?}", params.get("expac"))
    };
    let Some(guild_id) = interaction.guild_id else{
        bail! ("Command must be inside a server");
    };

    remove_train(database, ctx, interaction, guild_id, world, expac).await
}
async fn add_train<'e, Executor>(
    tx: &'e mut Executor,
    ctx: &Context,
    interaction: &ApplicationCommandInteraction,
    guild_id: GuildId,
    expac: &String,
    world: &String,
) -> eyre::Result<String>
where
    for<'a> &'a mut Executor: SqliteExecutor<'a>,
{
    let guild_id_s = guild_id.0 as i64;
    let existing = query!(
        "SELECT COUNT(*) as count FROM trains WHERE guild_id = ? AND world = ? AND expac = ?",
        guild_id_s,
        world,
        expac,
    )
    .fetch_one(&mut *tx)
    .await?
    .count;

    ensure!(
        existing == 0,
        "A {} train on {} already exists.",
        expac,
        world
    );

    let channel_id = interaction.channel_id;
    let mut train = Train {
        guild_id,
        world: world.parse()?,
        expac: expac.parse()?,
        channel_id: channel_id,
        message_id: MessageId(0),
        status: Status::Unknown,
        scout_map: None,
        last_run: None,
    };
    let m = channel_id
        .send_message(&ctx.http, |m| {
            m.add_embed(|embed| train.format_embed(embed))
            // .components(train.format_components)
        })
        .await?;
    train.message_id = m.id;
    train.write_to_db(tx).await?;

    Ok(format!(
        "Success! A {} train on {} has been created!",
        expac, world,
    ))
}

async fn remove_train(
    database: impl sqlx::SqliteExecutor<'_>,
    ctx: &Context,
    interaction: &ApplicationCommandInteraction,
    guild_id: GuildId,
    world: &String,
    expac: &String,
) -> eyre::Result<String> {
    let guild_id_s = guild_id.0 as i64;
    let affected = query!(
        "DELETE FROM trains WHERE guild_id = ? AND world = ? AND expac = ?",
        guild_id_s,
        world,
        expac,
    )
    .execute(database)
    .await?
    .rows_affected();

    ensure!(affected > 0, "No known {} train on {}", expac, world);
    Ok(format!(
        "Success! The {} train on {} has been deleted.\n\nYou may now delete the message that contained the train.",
        expac, world
    ))
}
