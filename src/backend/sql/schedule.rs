use sea_orm::entity::prelude::*;

#[derive(Clone, Debug, DeriveEntityModel, PartialEq)]
#[sea_orm(table_name = "jerb_schedules")]
pub struct Model {
  pub enabled: bool,
  pub fired_at: i64,
  #[sea_orm(primary_key, auto_increment = false)]
  pub name: String,
}

impl ActiveModelBehavior for ActiveModel {}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}
