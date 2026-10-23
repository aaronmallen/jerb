use std::any::{Any, TypeId};

use getset::CopyGetters;

use crate::{Handler, Run, handler::HandlerFuture};

/// A job fn marked `#[jerb::job]`, as the router finds it.
#[derive(Clone, Copy, CopyGetters, Debug)]
pub struct Route {
  call: fn(Run, &(dyn Any + Send + Sync)) -> HandlerFuture,
  /// The name of the job fn.
  #[get_copy = "pub"]
  name: &'static str,
  state: Option<fn() -> TypeId>,
}

impl Route {
  #[doc(hidden)]
  pub const fn new(
    name: &'static str,
    state: Option<fn() -> TypeId>,
    call: fn(Run, &(dyn Any + Send + Sync)) -> HandlerFuture,
  ) -> Self {
    Self {
      call,
      name,
      state,
    }
  }

  /// Runs the job with `state`, which must be of the type [`Route::state`] reports when it reports one.
  pub fn call<S: Any + Send + Sync>(&self, run: Run, state: &S) -> HandlerFuture {
    (self.call)(run, state)
  }

  /// The type the job takes through `State`, or `None` when it takes no state.
  pub fn state(&self) -> Option<TypeId> {
    self.state.map(|state| state())
  }
}

inventory::collect!(Route);

#[doc(hidden)]
pub fn call<S: Clone + Send + 'static, H: Handler<T, S>, T>(
  handler: H,
  run: Run,
  state: &(dyn Any + Send + Sync),
) -> HandlerFuture {
  match state.downcast_ref::<S>() {
    Some(state) => handler.call(run, state.clone()),
    None => Box::pin(async { Err("the state is not the type the job takes".into()) }),
  }
}

#[cfg(test)]
mod tests {
  use std::{
    error::Error,
    sync::{Arc, Mutex},
  };

  use serde_json::json;

  use super::*;
  use crate::{Json, Router, State};

  type Seen = Arc<Mutex<Vec<String>>>;

  #[crate::job]
  async fn record_name(State(seen): State<Seen>, Json(name): Json<String>) -> Result<(), Box<dyn Error + Send + Sync>> {
    seen.lock().unwrap().push(name);
    Ok(())
  }

  #[crate::job]
  async fn reject_name(Json(name): Json<String>) -> Result<(), Box<dyn Error + Send + Sync>> {
    Err(name.into())
  }

  fn route(name: &str) -> Route {
    Router::new().routes()[name]
  }

  mod route {
    use super::*;

    mod call {
      use pretty_assertions::assert_eq;

      use super::*;

      #[tokio::test]
      async fn it_runs_the_job_with_its_args_and_state() {
        let seen = Seen::default();

        route("record_name")
          .call(Run::new().with_args(json!("ada")), &seen)
          .await
          .unwrap();

        assert_eq!(*seen.lock().unwrap(), vec!["ada".to_string()]);
      }

      #[tokio::test]
      async fn it_runs_a_job_that_takes_no_state_with_any_state() {
        let error = route("reject_name")
          .call(Run::new().with_args(json!("ada")), &7_u8)
          .await
          .unwrap_err();

        assert_eq!(error.to_string(), "ada");
      }

      #[tokio::test]
      async fn it_returns_an_error_when_the_state_is_another_type() {
        let error = route("record_name")
          .call(Run::new().with_args(json!("ada")), &7_u8)
          .await
          .unwrap_err();

        assert_eq!(error.to_string(), "the state is not the type the job takes");
      }
    }

    mod state {
      use pretty_assertions::assert_eq;

      use super::*;

      #[test]
      fn it_is_none_when_the_job_takes_no_state() {
        assert_eq!(route("reject_name").state(), None);
      }

      #[test]
      fn it_is_the_type_the_job_takes_through_state() {
        assert_eq!(route("record_name").state(), Some(TypeId::of::<Seen>()));
      }
    }
  }
}
