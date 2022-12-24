use sea_orm::entity::prelude::*;

#[derive(Clone, Debug, DeriveEntityModel)]
#[sea_orm(table_name = "monitors")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i64,
    pub message_id: i64,
    pub channel_id: i64,
    pub train_id: i64,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(has_one = "super::train::Entity")]
    Train,
}

impl Related<super::train::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Train.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}
