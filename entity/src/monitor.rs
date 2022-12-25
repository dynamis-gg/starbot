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
    #[sea_orm(
        belongs_to = "super::train::Entity",
        from = "Column::TrainId",
        to = "super::train::Column::Id"
    )]
    Train,
}

impl Related<super::train::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Train.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}
