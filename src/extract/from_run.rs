use std::error::Error;

use crate::Run;

pub trait FromRun<S>: Sized {
  fn from_run(run: &Run, state: &S) -> Result<Self, Box<dyn Error + Send + Sync>>;
}
