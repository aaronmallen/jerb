use sea_orm_migration::{prelude::*, schema::*};

pub struct AddBacktrace;

#[async_trait::async_trait]
impl MigrationTrait for AddBacktrace {
  async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
    manager
      .alter_table(Table::alter().table("jerb_jobs").drop_column("backtrace").to_owned())
      .await
  }

  async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
    manager
      .alter_table(
        Table::alter()
          .table("jerb_jobs")
          .add_column(text_null("backtrace"))
          .to_owned(),
      )
      .await
  }
}

impl MigrationName for AddBacktrace {
  fn name(&self) -> &str {
    "m20261023_000008_add_backtrace"
  }
}
