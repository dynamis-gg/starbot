pub mod monitor;
pub mod train;

use sea_orm::entity::prelude::*;
use sea_orm::{DeriveActiveEnum, EnumIter};
use strum_macros::{AsRefStr, Display, EnumString, IntoStaticStr};

#[derive(
    Copy, Clone, PartialEq, Eq, Hash, Debug, DeriveActiveEnum, EnumIter, EnumString, AsRefStr,
)]
#[sea_orm(rs_type = "String", db_type = "String(None)")]
pub enum World {
    #[sea_orm(string_value = "Halicarnassus")]
    Halicarnassus,
    #[sea_orm(string_value = "Maduin")]
    Maduin,
    #[sea_orm(string_value = "Marilith")]
    Marilith,
    #[sea_orm(string_value = "Seraph")]
    Seraph,
}

#[derive(Copy, Clone, PartialEq, Hash, Debug, DeriveActiveEnum, EnumIter, EnumString, AsRefStr)]
#[sea_orm(rs_type = "i8", db_type = "Integer")]
#[repr(i8)]
pub enum Expac {
    #[strum(to_string = "A Realm Reborn")]
    ARR = 2,
    #[strum(to_string = "Heavensward")]
    HW = 3,
    #[strum(to_string = "Stormblood")]
    StB = 4,
    #[strum(to_string = "Shadowbringers")]
    ShB = 5,
    #[strum(to_string = "Endwalker")]
    EW = 6,
}
