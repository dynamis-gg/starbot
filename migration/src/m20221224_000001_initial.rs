use sea_orm_migration::prelude::*;

use entity::train::Column as Trains;
use entity::monitor::Column as Monitors;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let trains = Table::create()
            .table(entity::Table::Trains)
            .col(
                ColumnDef::new(Trains::Id)
                    .integer()
                    .primary_key(),
            )
            .col(ColumnDef::new(Trains::World).text().not_null())
            .col(ColumnDef::new(Trains::Expac).integer().not_null())
            .col(ColumnDef::new(Trains::Status).integer().not_null())
            .col(ColumnDef::new(Trains::LastRun).integer())
            .col(ColumnDef::new(Trains::ScoutMap).text())
            .to_owned();
        /*
        manager
            .create_table(trains)
            .await?;
            */
        println!("{}", manager.get_database_backend().build(&trains));

        let monitors = Table::create()
            .table(entity::Table::Monitors)
            .col(
                ColumnDef::new(Monitors::Id).integer().primary_key(),
                )
            .col(ColumnDef::new(Monitors::MessageId).integer().not_null())
            
            .col(ColumnDef::new(Monitors::ChannelId).integer().not_null())
            .col(ColumnDef::new(Monitors::TrainId).integer().not_null())
            .to_owned();
        /*
        manager
            .create_table(monitors)
            .await?;
            */
        println!("{}", manager.get_database_backend().build(&monitors));

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(entity::Table::Trains).to_owned())
            .await?;
        manager
            .drop_table(Table::drop().table(entity::Table::Monitors).to_owned()).await?;
        
        Ok(())
    }
}
