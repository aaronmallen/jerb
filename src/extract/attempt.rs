use std::error::Error;

use crate::{FromRun, Run};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Attempt(pub u32);

impl<S> FromRun<S> for Attempt {
  fn from_run(run: &Run, _state: &S) -> Result<Self, Box<dyn Error + Send + Sync>> {
    Ok(Self(run.attempt()))
  }
}
