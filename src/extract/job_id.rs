use std::error::Error;

use crate::{FromRun, Run};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct JobId(pub String);

impl<S> FromRun<S> for JobId {
  fn from_run(run: &Run, _state: &S) -> Result<Self, Box<dyn Error + Send + Sync>> {
    Ok(Self(run.id().clone()))
  }
}
