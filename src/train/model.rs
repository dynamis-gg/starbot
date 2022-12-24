use chrono::{DateTime, Duration, Utc};
use eyre::{bail, ensure, eyre, WrapErr};
use serenity::async_trait;
use serenity::builder::{
    CreateApplicationCommandOption, CreateButton, CreateComponents, CreateEmbed,
};
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

#[derive(Clone, Debug)]
pub struct Train {
    pub guild_id: GuildId,
    pub world: World,
    pub expac: Expac,
    pub channel_id: ChannelId,
    pub message_id: MessageId,
    pub status: Status,
    pub scout_map: Option<Url>,
    pub last_run: Option<DateTime<Utc>>,
}

impl Train {
    pub fn begin(&mut self) {
        self.status = Status::Running;
    }
    pub fn scouted(&mut self, scout_map: Option<Url>) {
        self.scout_map = scout_map;
        self.status = Status::Scouted;
    }
    pub fn done(&mut self) {
        self.scout_map = None;
        self.status = Status::Waiting;
        self.last_run = Some(Utc::now());
    }
    pub fn reset(&mut self) {
        self.status = Status::Unknown;
        self.scout_map = None;
        self.last_run = None;
    }
    pub fn set_scout_map(&mut self, scout_map: Url) {
        self.scout_map = Some(scout_map);
    }
    pub fn clear_scout_map(&mut self) {
        self.scout_map = None;
    }
    pub fn set_last_run(&mut self, time: DateTime<Utc>) {
        self.last_run = Some(time)
    }
    pub fn clear_last_run(&mut self) {
        self.last_run = None;
    }

    pub async fn from_db(
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
            status: Status,
            scout_map: Option<String>,
            last_run: Option<DateTime<Utc>>,
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

    pub async fn write_to_db(&self, executor: impl sqlx::SqliteExecutor<'_>) -> eyre::Result<()> {
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

    pub fn format_embed<'a>(&self, embed: &'a mut CreateEmbed) -> &'a mut CreateEmbed {
        let mut content = format!("Status: {}", self.status);
        if let Some(end_time) = self.last_run {
            content += &*format!("\nLast run completed at: <t:{}:F>", end_time.timestamp());
            if self.status == Status::Waiting {
                let force_time = end_time + (Duration::hours(6));
                if force_time < Utc::now() {
                    content += &*format!("\nForce at: <t:{}:F>", force_time.timestamp());
                } else {
                    content += &*format!("\n**Forced at:** <t:{}:F>", force_time.timestamp());
                }
            }
        }
        embed
            .title(format!("{} {} Train", self.world, self.expac))
            .description(content)
    }

    pub fn format_components(&self) -> CreateComponents {
        let mut components = CreateComponents::default();
        components.create_action_row(|row| {
            match self.status {
                Status::Waiting => {
                    row.create_button(Self::create_scout_button);
                    row.create_button(Self::create_run_button);
                }
                Status::Scouted => {
                    row.create_button(Self::create_run_button);
                }
                Status::Running => {
                    row.create_button(Self::create_done_button);
                }
                Status::Unknown => {
                    row.create_button(Self::create_scout_button);
                    row.create_button(Self::create_run_button);
                    row.create_button(Self::create_done_button);
                }
            };
            row
        });
        if let Some(url) = &self.scout_map {
            components.create_action_row(|row| {
                row.create_button(|button| Self::create_scout_link(&url, button))
            });
        }
        components
    }

    fn create_scout_button<'b>(button: &'b mut CreateButton) -> &'b mut CreateButton {
        button
            .style(ButtonStyle::Primary)
            .label("Scout")
            .custom_id("scout")
    }
    fn create_run_button<'b>(button: &'b mut CreateButton) -> &'b mut CreateButton {
        button
            .style(ButtonStyle::Primary)
            .label("Start")
            .custom_id("run")
    }
    fn create_done_button<'b>(button: &'b mut CreateButton) -> &'b mut CreateButton {
        button
            .style(ButtonStyle::Success)
            .label("Complete")
            .custom_id("done")
    }

    fn create_scout_link<'b>(url: &Url, button: &'b mut CreateButton) -> &'b mut CreateButton {
        button
            .style(ButtonStyle::Link)
            .label("Scouted Map")
            .url(url)
    }
}

#[derive(Copy, Clone, PartialEq, Eq, Hash, Display, Debug, sqlx::Type)]
pub enum Status {
    Unknown,
    Waiting,
    Scouted,
    Running,
}

#[derive(
    Copy, Clone, PartialEq, Eq, Hash, Display, Debug, sqlx::Type, EnumIter, EnumString, AsRefStr,
)]
pub enum World {
    Halicarnassus,
    Maduin,
    Marilith,
    Seraph,
}

#[derive(
    Copy, Clone, PartialEq, Hash, Display, Debug, sqlx::Type, EnumIter, EnumString, AsRefStr,
)]
#[repr(i8)]
pub enum Expac {
    #[strum(to_string = "A Realm Reborn")]
    ARR = 2,
    #[strum(to_string = "Heavensward")]
    HW = 3,
    #[strum(to_string = "Stormblood")]
    StB = 5,
    #[strum(to_string = "Shadowbringers")]
    ShB = 6,
    #[strum(to_string = "Endwalker")]
    EW = 7,
}
