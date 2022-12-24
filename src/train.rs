use std::collections::HashMap;
use std::convert::AsRef;

use eyre::{bail, ensure, eyre, WrapErr};
use serenity::async_trait;
use serenity::builder::{CreateComponents,CreateApplicationCommandOption};
use serenity::model::application::command::{CommandOptionType, CommandType};
use serenity::model::application::interaction::application_command::{
    ApplicationCommandInteraction, CommandDataOption, CommandDataOptionValue,
};
use serenity::model::prelude::*;
use serenity::prelude::*;
use sqlx::{query, query_as};
use strum::IntoEnumIterator;
use strum_macros::{EnumIter, EnumString, IntoStaticStr, AsRefStr, Display};
use url::Url;
use time::{OffsetDateTime, Duration};

pub async fn init(database: &sqlx::SqlitePool, ctx: &Context, guild: &Guild) {
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

#[derive(Copy, Clone, PartialEq, Eq, Hash, Display, Debug, sqlx::Type)]
enum TrainStatus {
    Unknown,
    Waiting,
    Scouted,
    Running,
}

#[derive(Copy, Clone, PartialEq, Eq, Hash, Display, Debug)]
#[derive(sqlx::Type, EnumIter, EnumString, AsRefStr)]
enum World {
    Halicarnassus,
    Maduin,
    Marilith,
    Seraph,
}

#[derive(Copy, Clone, PartialEq, Hash, Display, Debug)]
#[derive(sqlx::Type, EnumIter, EnumString, AsRefStr)]
#[repr(i8)]
enum Expac {
    #[strum(to_string="A Realm Reborn")]
    ARR = 2,
    #[strum(to_string="Heavensward")]
    HW = 3,
    #[strum(to_string="Stormblood")]
    StB = 5,
    #[strum(to_string="Shadowbringers")]
    ShB = 6,
    #[strum(to_string="Endwalker")]
    EW = 7,
}

#[derive(Clone, Debug)]
struct Train {
    guild_id: GuildId,
    world: World,
    expac: Expac,
    channel_id: ChannelId,
    message_id: MessageId,
    status: TrainStatus,
    scout_map: Option<Url>,
    last_run: Option<OffsetDateTime>,
}

impl Train {
    fn begin(&mut self) {
        self.status = TrainStatus::Running;
    }
    fn scouted(&mut self, scout_map: Option<Url>){
        self.scout_map = scout_map;
        self.status = TrainStatus::Scouted;
    }
    fn done(&mut self) {
        self.scout_map = None;
        self.status = TrainStatus::Waiting;
        self.last_run = Some(OffsetDateTime::now_utc());
    }
    fn lost(&mut self) {
        self.status = TrainStatus::Unknown;
    }
    fn set_scout_map(&mut self, scout_map: Url) {
        self.scout_map = Some(scout_map);
    }
    fn clear_scout_map(&mut self) {
        self.scout_map = None;
    }
    fn set_last_run(&mut self, time: OffsetDateTime) {
        self.last_run = Some(time)
    }
    fn clear_last_run(&mut self) {
        self.last_run = None;
    }

    async fn from_db(
        &self,
        executor: impl sqlx::SqliteExecutor<'_>,
        guild_id: GuildId,
        world: World,
        expac: Expac,
    ) -> eyre::Result<Train> {
        let guild_id_s = guild_id.0 as i64;
        #[derive(sqlx::FromRow)]
        struct Row {
            channel_id: i64,
            message_id: i64,
            status: TrainStatus,
            scout_map: Option<String>,
            last_run: Option<OffsetDateTime>,
        }
        let row = query_as::<_, Row>(
            "SELECT channel_id, message_id, status, scout_map, last_run
             FROM trains WHERE guild_id = ? AND world = ? AND expac = ?",
        )
        .bind(guild_id_s)
        .bind(world)
        .bind(expac)
        .fetch_one(executor)
        .await?;

        Ok(Train {
            guild_id: guild_id,
            world: world,
            expac: expac,
            channel_id: ChannelId(row.channel_id as u64),
            message_id: MessageId(row.message_id as u64),
            status: row.status,
            scout_map: row.scout_map.map(|s| s.parse()).transpose()?,
            last_run: row.last_run,
        })
    }

    async fn write_to_db(&self, executor: impl sqlx::SqliteExecutor<'_>) -> eyre::Result<()> {
        let guild_id = self.guild_id.0 as i64;
        let channel_id = self.channel_id.0 as i64;
        let message_id = self.message_id.0 as i64;
        let scout_map = self.scout_map.as_ref().map(|u| u.as_str());
        query!("REPLACE INTO trains (guild_id, world, expac, channel_id, message_id, status, scout_map, last_run)
                VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
            guild_id,
            self.world,
            self.expac,
            channel_id,
            message_id,
            self.status,
            scout_map,
            self.last_run,
        ) 
        .execute(executor)
        .await?;
        Ok(())
    }

    fn message_content(&self) -> String {
        let mut content = format!("**{} {} Train**\n\nStatus: {}", self.world, self.expac, self.status);
        if let Some(end_time) = self.last_run {
            content+= &*format!("\nLast run completed at: <t:{}:f>", end_time.unix_timestamp());
            if self.status == TrainStatus::Waiting {
                let force_time = end_time + 6*Duration::HOUR;
                if force_time < OffsetDateTime::now_utc() {
                    content += &*format!("\nForce at: <t:{}:f>", force_time.unix_timestamp());
                } else {
                    content+=&*format!("\n**Forced at:** <t:{}:f>", force_time.unix_timestamp());
                }
            }
        }
        content
    }

    fn message_components(&self) -> CreateComponents{
        let mut components = CreateComponents::default();
        components
    }
}

async fn add_train(
    tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
    ctx: &Context,
    interaction: &ApplicationCommandInteraction,
    guild_id: GuildId,
    expac: &String,
    world: &String,
    channel: &PartialChannel,
) -> eyre::Result<String> {
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

    let mut train = Train {
        guild_id,
        world: world.parse()?,
        expac: expac.parse()?,
        channel_id: channel.id,
        message_id: MessageId(0),
        status: TrainStatus::Unknown,
        scout_map: None,
        last_run: None,
    };
    let m = channel.id.send_message(&ctx.http, |m|
        m.content(train.message_content()).set_components(train.message_components())
    ).await?;
    train.message_id = m.id;
    train.write_to_db( tx).await?;

    Ok("Success!".to_owned())
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
