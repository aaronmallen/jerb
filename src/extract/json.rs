use std::error::Error;

use serde::de::DeserializeOwned;

use crate::{FromRun, Run};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Json<T>(pub T);

impl<S, T: DeserializeOwned> FromRun<S> for Json<T> {
  fn from_run(run: &Run, _state: &S) -> Result<Self, Box<dyn Error + Send + Sync>> {
    Ok(Self(T::deserialize(run.args())?))
  }
}
