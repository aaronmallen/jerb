use sea_orm_migration::{prelude::*, schema::*};

pub struct AddRunAt;

#[async_trait::async_trait]
impl MigrationTrait for AddRunAt {
  async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
    manager
      .alter_table(Table::alter().table("jerb_jobs").drop_column("run_at").to_owned())
      .await
  }

  async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
    manager
      .alter_table(
        Table::alter()
          .table("jerb_jobs")
          .add_column(big_integer_null("run_at"))
          .to_owned(),
      )
      .await
  }
}

impl MigrationName for AddRunAt {
  fn name(&self) -> &str {
    "m20261023_000002_add_run_at"
  }
}
