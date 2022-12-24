pub mod monitor;
pub mod train;

use poise::serenity_prelude as serenity;
use poise::SlashArgument;
use sea_orm::entity::prelude::*;
use sea_orm::sea_query::{self, DynIden, SeaRc};
use sea_orm::{EnumIter, Iterable};
use strum_macros::{AsRefStr, Display, EnumString, FromRepr, IntoStaticStr};

#[derive(Copy, Clone, PartialEq, Eq, Hash, Display, Debug, EnumIter, EnumString, AsRefStr)]
#[strum(ascii_case_insensitive)]
pub enum World {
    Halicarnassus,
    Maduin,
    Marilith,
    Seraph,
}

#[poise::async_trait]
impl SlashArgument for World {
    async fn extract(
        ctx: &serenity::Context,
        interaction: poise::ApplicationCommandOrAutocompleteInteraction<'_>,
        value: &poise::serenity_prelude::json::Value,
    ) -> Result<Self, poise::SlashArgError> {
        let choice = value
            .as_u64()
            .ok_or(poise::SlashArgError::CommandStructureMismatch(
                "expected u64",
            ))?;
        Self::iter()
            // TODO: technically this can overflow on 32-bit systems
            .nth(choice as usize)
            .ok_or(poise::SlashArgError::CommandStructureMismatch(
                "argument out of range",
            ))
    }

    fn create(builder: &mut serenity::CreateApplicationCommandOption) {
        builder.kind(poise::serenity_prelude::CommandOptionType::Integer);
    }

    fn choices() -> Vec<poise::CommandParameterChoice> {
        Self::iter()
            .map(|world| poise::CommandParameterChoice {
                name: world.to_string(),
                localizations: std::collections::HashMap::from([(
                    "en-US".to_owned(),
                    world.to_string(),
                )]),
            })
            .collect()
    }
}

#[derive(Debug, Iden)]
pub struct WorldEnum;

impl ActiveEnum for World {
    type Value = String;

    fn name() -> sea_orm::DynIden {
        SeaRc::new(WorldEnum)
    }

    fn to_value(&self) -> Self::Value {
        self.to_string()
    }

    fn try_from_value(v: &Self::Value) -> Result<Self, DbErr> {
        v.parse::<Self>().map_err(|e| DbErr::Type(e.to_string()))
    }

    fn db_type() -> ColumnDef {
        ColumnType::String(None).def()
    }
}

impl From<World> for sea_orm::Value {
    fn from(world: World) -> Self {
        world.into_value().into()
    }
}

impl sea_orm::TryGetable for World {
    fn try_get(res: &QueryResult, pre: &str, col: &str) -> Result<Self, sea_orm::TryGetError> {
        let value = <<Self as ActiveEnum>::Value as sea_orm::TryGetable>::try_get(res, pre, col)?;
        <Self as sea_orm::ActiveEnum>::try_from_value(&value).map_err(sea_orm::TryGetError::DbErr)
    }
}

impl sea_query::ValueType for World {
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
#[derive(
    Copy, Clone, PartialEq, Hash, Display, Debug, EnumIter, EnumString, AsRefStr, FromRepr,
)]
#[repr(i8)]
#[strum(ascii_case_insensitive)]
pub enum Expac {
    #[strum(to_string = "A Realm Reborn", serialize = "ARR")]
    ARR = 2,
    #[strum(to_string = "Heavensward", serialize = "HW")]
    HW = 3,
    #[strum(to_string = "Stormblood", serialize = "SB", serialize = "StB")]
    StB = 4,
    #[strum(to_string = "Shadowbringers", serialize = "ShB")]
    ShB = 5,
    #[strum(to_string = "Endwalker", serialize = "EW")]
    EW = 6,
}

#[derive(Debug, Iden)]
pub struct ExpacEnum;

#[poise::async_trait]
impl SlashArgument for Expac {
    async fn extract(
        ctx: &serenity::Context,
        interaction: poise::ApplicationCommandOrAutocompleteInteraction<'_>,
        value: &poise::serenity_prelude::json::Value,
    ) -> Result<Self, poise::SlashArgError> {
        let choice = value
            .as_u64()
            .ok_or(poise::SlashArgError::CommandStructureMismatch(
                "expected u64",
            ))?;
        Self::iter()
            // TODO: technically this can overflow on 32-bit systems
            .nth(choice as usize)
            .ok_or(poise::SlashArgError::CommandStructureMismatch(
                "argument out of range",
            ))
    }

    fn create(builder: &mut serenity::CreateApplicationCommandOption) {
        builder.kind(poise::serenity_prelude::CommandOptionType::Integer);
    }

    fn choices() -> Vec<poise::CommandParameterChoice> {
        Self::iter()
            .map(|expac| poise::CommandParameterChoice {
                name: expac.to_string(),
                localizations: std::collections::HashMap::from([(
                    "en-US".to_owned(),
                    expac.to_string(),
                )]),
            })
            .collect()
    }
}

impl ActiveEnum for Expac {
    type Value = i8;

    fn name() -> sea_orm::DynIden {
        SeaRc::new(ExpacEnum)
    }

    fn to_value(&self) -> Self::Value {
        *self as Self::Value
    }

    fn try_from_value(v: &Self::Value) -> Result<Self, DbErr> {
        Self::from_repr(*v).ok_or_else(|| DbErr::Type(format!("invalid Expac value: {}", v)))
    }

    fn db_type() -> ColumnDef {
        ColumnType::Integer.def()
    }
}

impl From<Expac> for sea_orm::Value {
    fn from(expac: Expac) -> Self {
        expac.into_value().into()
    }
}

impl sea_orm::TryGetable for Expac {
    fn try_get(res: &QueryResult, pre: &str, col: &str) -> Result<Self, sea_orm::TryGetError> {
        let value = <<Self as ActiveEnum>::Value as sea_orm::TryGetable>::try_get(res, pre, col)?;
        <Self as sea_orm::ActiveEnum>::try_from_value(&value).map_err(sea_orm::TryGetError::DbErr)
    }
}

impl sea_query::ValueType for Expac {
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
