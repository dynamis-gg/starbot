use chrono::{DateTime, Duration, Utc};
use eyre::{bail, ensure, eyre, WrapErr};
use sea_orm::entity::prelude::*;
use sea_orm::ActiveValue::NotSet;
use sea_orm::{ConnectionTrait, DeriveActiveEnum, EnumIter, Set};
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
use strum_macros::{AsRefStr, Display, EnumString, IntoStaticStr};
use url::Url;

use super::{Expac, World};

#[derive(Clone, Debug, DeriveEntityModel)]
#[sea_orm(table_name = "trains")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i64,
    pub world: World,
    pub expac: Expac,
    pub status: Status,
    pub scout_map: Option<String>,
    pub last_run: Option<DateTime<Utc>>,
}

impl Model {
    pub fn begin(&mut self) {
        self.status = Status::Running;
    }
    pub fn scouted(&mut self, scout_map: Option<Url>) {
        self.scout_map = scout_map.map(|u| u.into());
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
        self.scout_map = Some(scout_map.into());
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
                row.create_button(|button| Self::create_scout_link(url, button))
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

    fn create_scout_link<'b>(url: &String, button: &'b mut CreateButton) -> &'b mut CreateButton {
        button
            .style(ButtonStyle::Link)
            .label("Scouted Map")
            .url(url)
    }
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}

pub async fn find_or_create(
    tx: &impl ConnectionTrait,
    world: World,
    expac: Expac,
) -> eyre::Result<Model> {
    match Entity::find()
        .filter(Column::World.eq(world).and(Column::Expac.eq(expac)))
        .one(tx)
        .await?
    {
        Some(existing) => Ok(existing),
        None => {
            let new = ActiveModel {
                id: NotSet,
                world: Set(world),
                expac: Set(expac),
                status: Set(Status::Unknown),
                ..Default::default()
            };
            Ok(new.insert(tx).await?)
        }
    }
}

#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug, DeriveActiveEnum, EnumIter)]
#[sea_orm(rs_type = "i8", db_type = "Integer")]
#[repr(i8)]
pub enum Status {
    Unknown = 0,
    Waiting = 1,
    Scouted = 2,
    Running = 3,
}