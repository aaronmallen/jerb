use sea_orm::entity::prelude::*;

#[derive(Clone, Debug, DeriveEntityModel, PartialEq)]
#[sea_orm(table_name = "jerb_jobs")]
pub struct Model {
  pub args: Json,
  pub attempt: i32,
  pub backtrace: Option<String>,
  pub claimed_at: Option<i64>,
  pub enqueued_at: Option<i64>,
  pub error: Option<String>,
  pub finished_at: Option<i64>,
  #[sea_orm(primary_key)]
  pub id: i64,
  pub name: String,
  pub queue: String,
  pub run_at: Option<i64>,
  pub schedule: Option<String>,
  pub status: Status,
  pub unique_key: Option<String>,
}

impl ActiveModelBehavior for ActiveModel {}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

#[derive(Clone, Copy, Debug, DeriveActiveEnum, EnumIter, Eq, PartialEq)]
#[sea_orm(rs_type = "String", db_type = "String(StringLen::None)")]
pub enum Status {
  #[sea_orm(string_value = "failed")]
  Failed,
  #[sea_orm(string_value = "finished")]
  Finished,
  #[sea_orm(string_value = "pending")]
  Pending,
  #[sea_orm(string_value = "running")]
  Running,
}
