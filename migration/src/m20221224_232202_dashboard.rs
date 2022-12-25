use sea_orm_migration::prelude::*;

use entity::dashboard::Column as Dashboards;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(entity::Table::Dashboards)
                    .col(
                        ColumnDef::new(Dashboards::Id)
                            .integer()
                            .primary_key(),
                    )
                    .col(ColumnDef::new(Dashboards::MessageId).integer().not_null())
                    .col(ColumnDef::new(Dashboards::ChannelId).integer().not_null())
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(entity::Table::Dashboards).to_owned())
            .await
    }
}
