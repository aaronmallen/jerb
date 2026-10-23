use sea_orm_migration::{prelude::*, schema::*};

pub struct AddJobSchedule;

#[async_trait::async_trait]
impl MigrationTrait for AddJobSchedule {
  async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
    manager
      .alter_table(Table::alter().table("jerb_jobs").drop_column("schedule").to_owned())
      .await
  }

  async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
    manager
      .alter_table(
        Table::alter()
          .table("jerb_jobs")
          .add_column(string_null("schedule"))
          .to_owned(),
      )
      .await
  }
}

impl MigrationName for AddJobSchedule {
  fn name(&self) -> &str {
    "m20261023_000016_add_job_schedule"
  }
}
