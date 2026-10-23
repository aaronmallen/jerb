use sea_orm_migration::{prelude::*, schema::*};

pub struct AddAttempt;

#[async_trait::async_trait]
impl MigrationTrait for AddAttempt {
  async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
    manager
      .alter_table(Table::alter().table("jerb_jobs").drop_column("attempt").to_owned())
      .await
  }

  async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
    manager
      .alter_table(
        Table::alter()
          .table("jerb_jobs")
          .add_column(integer("attempt").default(0))
          .to_owned(),
      )
      .await
  }
}

impl MigrationName for AddAttempt {
  fn name(&self) -> &str {
    "m20261023_000005_add_attempt"
  }
}
