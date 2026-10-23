use getset::{CopyGetters, Getters};
use serde_json::Value;

#[derive(Clone, CopyGetters, Debug, Default, Getters)]
pub struct Run {
  #[getset(get = "pub")]
  args: Value,
  #[getset(get_copy = "pub")]
  attempt: u32,
  #[getset(get = "pub")]
  id: String,
}

impl Run {
  pub const fn new() -> Self {
    Self {
      args: Value::Null,
      attempt: 0,
      id: String::new(),
    }
  }

  pub fn with_args(mut self, args: Value) -> Self {
    self.args = args;
    self
  }

  pub const fn with_attempt(mut self, attempt: u32) -> Self {
    self.attempt = attempt;
    self
  }

  pub fn with_id(mut self, id: impl Into<String>) -> Self {
    self.id = id.into();
    self
  }
}
