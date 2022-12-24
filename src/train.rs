use std::collections::HashMap;

use eyre::{bail, ensure, eyre, WrapErr};
use serenity::async_trait;
use serenity::builder::CreateApplicationCommandOption;
use serenity::model::application::command::{CommandOptionType, CommandType};
use serenity::model::application::interaction::application_command::ApplicationCommandInteraction;
use serenity::model::application::interaction::{Interaction, InteractionResponseType};
use serenity::model::channel::ChannelType;
use serenity::model::guild::Guild;
use serenity::model::id::GuildId;
use serenity::model::permissions::Permissions;
use serenity::model::prelude::interaction::application_command::{
    CommandDataOption, CommandDataOptionValue,
};
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
        "add" => add_train(database, ctx, interaction, opt).await,
        "remove" => remove_train(database, ctx, interaction, opt).await,
        name => bail!("Unexpected SubCommand name: {}", name),
    }
}

async fn add_train(
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
    let Some(GuildId(guild_id)) = interaction.guild_id else{
        bail! ("Command must be inside a server");
    };
    let guild_id_s = guild_id as i64;

    let mut tx = database.begin().await?;
    let existing = sqlx::query!(
        "SELECT COUNT(*) as count FROM trains WHERE guild_id = ? AND world = ? AND expac = ?",
        guild_id_s,
        world,
        expac,
    )
    .fetch_one(&mut tx)
    .await?
    .count;

    ensure!(
        existing == 0,
        "A {} train on {} already exists.",
        expac,
        world
    );

    tx.commit().await?;
    Ok("Success".to_owned())
}

async fn remove_train(
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
    let Some(GuildId(guild_id)) = interaction.guild_id else{
        bail! ("Command must be inside a server");
    };
    let guild_id_s = guild_id as i64;

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
