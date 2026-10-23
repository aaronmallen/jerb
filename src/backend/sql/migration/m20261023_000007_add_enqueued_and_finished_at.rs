use sea_orm_migration::{prelude::*, schema::*};

pub struct AddEnqueuedAndFinishedAt;

#[async_trait::async_trait]
impl MigrationTrait for AddEnqueuedAndFinishedAt {
  async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
    for column in ["enqueued_at", "finished_at"] {
      manager
        .alter_table(Table::alter().table("jerb_jobs").drop_column(column).to_owned())
        .await?;
    }
    Ok(())
  }

  async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
    for column in ["enqueued_at", "finished_at"] {
      manager
        .alter_table(
          Table::alter()
            .table("jerb_jobs")
            .add_column(big_integer_null(column))
            .to_owned(),
        )
        .await?;
    }
    Ok(())
  }
}

impl MigrationName for AddEnqueuedAndFinishedAt {
  fn name(&self) -> &str {
    "m20261023_000007_add_enqueued_and_finished_at"
  }
}
