use sea_orm_migration::{prelude::*, schema::*};

pub struct AddClaimedAt;

#[async_trait::async_trait]
impl MigrationTrait for AddClaimedAt {
  async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
    manager
      .alter_table(Table::alter().table("jerb_jobs").drop_column("claimed_at").to_owned())
      .await
  }

  async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
    manager
      .alter_table(
        Table::alter()
          .table("jerb_jobs")
          .add_column(big_integer_null("claimed_at"))
          .to_owned(),
      )
      .await
  }
}

impl MigrationName for AddClaimedAt {
  fn name(&self) -> &str {
    "m20261023_000006_add_claimed_at"
  }
}
