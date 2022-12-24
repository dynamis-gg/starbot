use std::collections::HashMap;

use eyre::{bail, ensure, eyre, WrapErr};
use serenity::async_trait;
use serenity::builder::CreateApplicationCommandOption;
use serenity::model::application::command::{CommandOptionType, CommandType};
use serenity::model::application::interaction::application_command::{
    ApplicationCommandInteraction, CommandDataOption, CommandDataOptionValue,
};
use serenity::model::application::interaction::{Interaction, InteractionResponseType};
use serenity::model::channel::{ChannelType, PartialChannel};
use serenity::model::guild::Guild;
use serenity::model::id::{ChannelId, GuildId, MessageId};
use serenity::model::permissions::Permissions;
use serenity::prelude::*;

pub async fn init(database: &sqlx::SqlitePool, ctx: &Context, guild: &Guild) {
    let mut world_option = CreateApplicationCommandOption::default();
    world_option
        .name("world")
        .description("World server for this train")
        .kind(CommandOptionType::String)
        .add_string_choice("Halicarnassus", "Halicarnassus")
        .add_string_choice("Maduin", "Maduin")
        .add_string_choice("Marilith", "Marilith")
        .add_string_choice("Seraph", "Seraph");

    let mut expac_option = CreateApplicationCommandOption::default();
    expac_option
        .name("expac")
        .description("Expansion pack for the train")
        .kind(CommandOptionType::String)
        .add_string_choice("A Realm Reborn", "ARR")
        .add_string_choice("Heavensward", "HW")
        .add_string_choice("Stormblood", "StB")
        .add_string_choice("Shadowbringers", "ShB")
        .add_string_choice("Endwalker", "EW");

    guild.id.create_application_command(&ctx.http, |command| {
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
                    .create_sub_option(|option| {
                        option
                            .name("channel")
                            .description("Channel in which to post the tracking message")
                            .kind(CommandOptionType::Channel)
                            .channel_types(&[ChannelType::Text])
                    })
            })
            .create_option(|option| {
                option
                    .name("remove")
                    .description("Remove a train")
                    .kind(CommandOptionType::SubCommand)
                    .add_sub_option(world_option)
                    .add_sub_option(expac_option)
            })
    });
}

pub async fn handle_command(
    database: &sqlx::SqlitePool,
    ctx: &Context,
    interaction: &ApplicationCommandInteraction,
) -> eyre::Result<String> {
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
        "add" => handle_add(database, ctx, interaction, opt).await,
        "remove" => handle_remove(database, ctx, interaction, opt).await,
        name => bail!("Unexpected SubCommand name: {}", name),
    }
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
    let Some(CommandDataOptionValue::Channel(channel)) =
        params.get("channel").and_then(|o| o.resolved.as_ref())
    else {
        bail!("Invalid channel: {:#?}", params.get("channel"))
    };
    let Some(guild_id) = interaction.guild_id else{
        bail! ("Command must be inside a server");
    };

    let mut tx = database.begin().await?;
    let ok = add_train(&mut tx, ctx, interaction, guild_id, expac, world, channel).await?;
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

#[derive(Clone, Debug)]
struct Train {
    guild_id: GuildId,
    world: String,
    expac: String,
    message_id: MessageId,
    channel_id: ChannelId,
    status: TrainStatus,
    scout_map: Option<url::Url>,
    last_run: Option<time::OffsetDateTime>,
}

impl Train {}

#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug)]
enum TrainStatus {
    Unknown,
    Waiting,
    Scouted,
    Running,
}

async fn add_train(
    tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
    ctx: &Context,
    interaction: &ApplicationCommandInteraction,
    guild_id: GuildId,
    expac: &String,
    world: &String,
    channel: &PartialChannel,
) -> Result<String, eyre::ErrReport> {
    let guild_id_s = guild_id.0 as i64;
    let existing = sqlx::query!(
        "SELECT COUNT(*) as count FROM trains WHERE guild_id = ? AND world = ? AND expac = ?",
        guild_id_s,
        world,
        expac,
    )
    .fetch_one(tx)
    .await?
    .count;

    ensure!(
        existing == 0,
        "A {} train on {} already exists.",
        expac,
        world
    );

    let m = channel.id.send_message(&ctx.http, |m|
        m.content("Please wait... building train... it's safe to delete this message if construction fails")
    ).await?;
    let _train = Train {
        guild_id,
        world: world.clone(),
        expac: expac.clone(),
        channel_id: channel.id,
        message_id: m.id,
        status: TrainStatus::Unknown,
        scout_map: None,
        last_run: None,
    };

    Ok("Success!".to_owned())
}

async fn remove_train(
    database: &sqlx::Pool<sqlx::Sqlite>,
    ctx: &Context,
    interaction: &ApplicationCommandInteraction,
    guild_id: GuildId,
    world: &String,
    expac: &String,
) -> Result<String, eyre::ErrReport> {
    let guild_id_s = guild_id.0 as i64;
    let affected = sqlx::query!(
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
