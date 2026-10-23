use sea_orm_migration::{prelude::*, schema::*};

pub struct AddUniqueKey;

#[async_trait::async_trait]
impl MigrationTrait for AddUniqueKey {
  async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
    manager
      .drop_index(Index::drop().name("jerb_jobs_unique").table("jerb_jobs").to_owned())
      .await?;
    manager
      .alter_table(Table::alter().table("jerb_jobs").drop_column("unique_key").to_owned())
      .await
  }

  async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
    manager
      .alter_table(
        Table::alter()
          .table("jerb_jobs")
          .add_column(string_null("unique_key"))
          .to_owned(),
      )
      .await?;
    manager
      .create_index(
        Index::create()
          .name("jerb_jobs_unique")
          .table("jerb_jobs")
          .col("unique_key")
          .unique()
          .to_owned(),
      )
      .await
  }
}

impl MigrationName for AddUniqueKey {
  fn name(&self) -> &str {
    "m20261023_000004_add_unique_key"
  }
}
