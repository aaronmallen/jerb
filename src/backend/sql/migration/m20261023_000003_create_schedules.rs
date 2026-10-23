use sea_orm_migration::{prelude::*, schema::*};

pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
  async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
    manager
      .drop_table(Table::drop().table("jerb_schedules").to_owned())
      .await
  }

  async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
    manager
      .create_table(
        Table::create()
          .table("jerb_schedules")
          .col(string("name").primary_key())
          .col(big_integer("fired_at"))
          .to_owned(),
      )
      .await
  }
}

impl MigrationName for Migration {
  fn name(&self) -> &str {
    "m20261023_000003_create_schedules"
  }
}
