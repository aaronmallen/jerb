mod m20261023_000002_add_run_at;
mod m20261023_000003_create_schedules;
mod m20261023_000004_add_unique_key;
mod m20261023_000005_add_attempt;
mod m20261023_000006_add_claimed_at;
mod m20261023_000007_add_enqueued_and_finished_at;
mod m20261023_000008_add_backtrace;
mod m20261023_000009_create_paused_queues;
mod m20261023_000012_add_schedule_enabled;
mod m20261023_000016_add_job_schedule;

use m20261023_000002_add_run_at::AddRunAt;
use m20261023_000004_add_unique_key::AddUniqueKey;
use m20261023_000005_add_attempt::AddAttempt;
use m20261023_000006_add_claimed_at::AddClaimedAt;
use m20261023_000007_add_enqueued_and_finished_at::AddEnqueuedAndFinishedAt;
use m20261023_000008_add_backtrace::AddBacktrace;
use m20261023_000012_add_schedule_enabled::AddScheduleEnabled;
use m20261023_000016_add_job_schedule::AddJobSchedule;
use sea_orm_migration::{prelude::*, schema::*};

pub struct Migrator;

#[async_trait::async_trait]
impl MigratorTrait for Migrator {
  fn migration_table_name() -> DynIden {
    "jerb_migrations".into_iden()
  }

  fn migrations() -> Vec<Box<dyn MigrationTrait>> {
    vec![
      Box::new(CreateJobs),
      Box::new(AddRunAt),
      Box::new(m20261023_000003_create_schedules::Migration),
      Box::new(AddUniqueKey),
      Box::new(AddAttempt),
      Box::new(AddClaimedAt),
      Box::new(AddEnqueuedAndFinishedAt),
      Box::new(AddBacktrace),
      Box::new(m20261023_000009_create_paused_queues::Migration),
      Box::new(AddScheduleEnabled),
      Box::new(AddJobSchedule),
    ]
  }
}

struct CreateJobs;

#[async_trait::async_trait]
impl MigrationTrait for CreateJobs {
  async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
    manager.drop_table(Table::drop().table("jerb_jobs").to_owned()).await
  }

  async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
    manager
      .create_table(
        Table::create()
          .table("jerb_jobs")
          .col(big_pk_auto("id"))
          .col(string("queue"))
          .col(string("name"))
          .col(json("args"))
          .col(string("status"))
          .col(text_null("error"))
          .to_owned(),
      )
      .await?;
    manager
      .create_index(
        Index::create()
          .name("jerb_jobs_claim")
          .table("jerb_jobs")
          .col("status")
          .col("queue")
          .col("id")
          .to_owned(),
      )
      .await
  }
}

impl MigrationName for CreateJobs {
  fn name(&self) -> &str {
    "m20261023_000001_create_jobs"
  }
}
