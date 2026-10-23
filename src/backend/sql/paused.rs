use sea_orm::entity::prelude::*;

#[derive(Clone, Debug, DeriveEntityModel, PartialEq)]
#[sea_orm(table_name = "jerb_paused_queues")]
pub struct Model {
  #[sea_orm(primary_key, auto_increment = false)]
  pub queue: String,
}

impl ActiveModelBehavior for ActiveModel {}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}
