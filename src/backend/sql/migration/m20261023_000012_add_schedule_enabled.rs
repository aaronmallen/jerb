use sea_orm_migration::{prelude::*, schema::*};

pub struct AddScheduleEnabled;

#[async_trait::async_trait]
impl MigrationTrait for AddScheduleEnabled {
  async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
    manager
      .alter_table(Table::alter().table("jerb_schedules").drop_column("enabled").to_owned())
      .await
  }

  async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
    manager
      .alter_table(
        Table::alter()
          .table("jerb_schedules")
          .add_column(boolean("enabled").default(true))
          .to_owned(),
      )
      .await
  }
}

impl MigrationName for AddScheduleEnabled {
  fn name(&self) -> &str {
    "m20261023_000012_add_schedule_enabled"
  }
}
