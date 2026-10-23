use sea_orm_migration::{prelude::*, schema::*};

pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
  async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
    manager
      .drop_table(Table::drop().table("jerb_paused_queues").to_owned())
      .await
  }

  async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
    manager
      .create_table(
        Table::create()
          .table("jerb_paused_queues")
          .col(string("queue").primary_key())
          .to_owned(),
      )
      .await
  }
}

impl MigrationName for Migration {
  fn name(&self) -> &str {
    "m20261023_000009_create_paused_queues"
  }
}
