use chrono::{DateTime, Duration, Utc};
use sea_orm::entity::prelude::*;
use sea_orm::sea_query::{self, SeaRc};
use sea_orm::{ConnectionTrait, EnumIter, NotSet, Set};
use serenity::builder::{CreateButton, CreateComponents, CreateEmbed};
use serenity::model::prelude::component::ButtonStyle;
use std::fmt::Write;
use strum_macros::{Display, FromRepr};
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
        let mut content = format!("{} {}", self.status.emoji(), self.status);
        if let Some(end_time) = self.last_run {
            write!(
                content,
                "\nLast run completed at: <t:{}:f>",
                end_time.timestamp()
            )
            .unwrap();
            if self.status == Status::Waiting {
                let force_time = end_time + (Duration::hours(6));
                write!(content, "\nForced <t:{}:R>", force_time.timestamp()).unwrap();
            }
        }
        embed
            .title(format!("{} {} Train", self.world, self.expac))
            .description(content)
    }

    pub fn format_components<'a>(
        &self,
        components: &'a mut CreateComponents,
    ) -> &'a mut CreateComponents {
        /*
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
        */
        if let Some(url) = &self.scout_map {
            components.create_action_row(|row| {
                row.create_button(|button| Self::create_scout_link(url, button))
            });
        }
        components
    }

    #[allow(unused)]
    fn create_scout_button<'b>(button: &'b mut CreateButton) -> &'b mut CreateButton {
        button
            .style(ButtonStyle::Primary)
            .label("Scout")
            .custom_id("scout")
    }
    #[allow(unused)]
    fn create_run_button<'b>(button: &'b mut CreateButton) -> &'b mut CreateButton {
        button
            .style(ButtonStyle::Primary)
            .label("Start")
            .custom_id("run")
    }
    #[allow(unused)]
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
pub enum Relation {
    #[sea_orm(has_many = "super::monitor::Entity")]
    Monitor,
}

impl Related<super::monitor::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Monitor.def()
    }
}

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

#[derive(Copy, Clone, PartialEq, Eq, Hash, Display, Debug, EnumIter, FromRepr)]
#[repr(i8)]
pub enum Status {
    Unknown = 0,
    Waiting = 1,
    Scouted = 2,
    Running = 3,
}

impl Status {
    pub fn emoji(&self) -> &'static str {
        use Status::*;
        match self {
            Unknown => "❓",
            Waiting => "🕑",
            Scouted => "☑️",
            Running => "➡️",
        }
    }
}

#[derive(Debug, Iden)]
pub struct StatusEnum;

impl ActiveEnum for Status {
    type Value = i8;

    fn name() -> sea_orm::DynIden {
        SeaRc::new(StatusEnum)
    }

    fn to_value(&self) -> Self::Value {
        *self as Self::Value
    }

    fn try_from_value(v: &Self::Value) -> Result<Self, DbErr> {
        Self::from_repr(*v).ok_or_else(|| DbErr::Type(format!("invalid Status value: {}", v)))
    }

    fn db_type() -> ColumnDef {
        ColumnType::Integer.def()
    }
}

impl From<Status> for sea_orm::Value {
    fn from(expac: Status) -> Self {
        expac.into_value().into()
    }
}

impl sea_orm::TryGetable for Status {
    fn try_get(res: &QueryResult, pre: &str, col: &str) -> Result<Self, sea_orm::TryGetError> {
        let value = <<Self as ActiveEnum>::Value as sea_orm::TryGetable>::try_get(res, pre, col)?;
        <Self as sea_orm::ActiveEnum>::try_from_value(&value).map_err(sea_orm::TryGetError::DbErr)
    }
}

impl sea_query::ValueType for Status {
    fn try_from(v: Value) -> Result<Self, sea_query::ValueTypeErr> {
        let value =
            <<Self as sea_orm::ActiveEnum>::Value as sea_orm::sea_query::ValueType>::try_from(v)?;
        <Self as sea_orm::ActiveEnum>::try_from_value(&value)
            .map_err(|_| sea_orm::sea_query::ValueTypeErr)
    }

    fn type_name() -> String {
        <<Self as sea_orm::ActiveEnum>::Value as sea_orm::sea_query::ValueType>::type_name()
    }

    fn array_type() -> sea_query::ArrayType {
        unimplemented!("Array of enum is not supported.")
    }

    fn column_type() -> sea_query::ColumnType {
        <Self as sea_orm::ActiveEnum>::db_type()
            .get_column_type()
            .to_owned()
            .into()
    }
}
