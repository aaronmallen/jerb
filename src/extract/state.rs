use std::error::Error;

use crate::{FromRun, Run};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct State<S>(pub S);

impl<S: Clone> FromRun<S> for State<S> {
  fn from_run(_run: &Run, state: &S) -> Result<Self, Box<dyn Error + Send + Sync>> {
    Ok(Self(state.clone()))
  }
}
