use std::{
  collections::BTreeMap,
  fmt::{self, Debug, Formatter},
  sync::Arc,
};

use getset::Getters;

use crate::{Middleware, Route};

#[derive(Clone, Getters)]
pub struct Router<S = ()> {
  #[getset(get = "pub(crate)")]
  middleware: Vec<Arc<dyn Middleware>>,
  #[getset(get = "pub")]
  routes: BTreeMap<&'static str, Route>,
  #[getset(get = "pub")]
  state: S,
}

impl Router {
  pub fn new() -> Self {
    Self {
      middleware: Vec::new(),
      routes: inventory::iter::<Route>
        .into_iter()
        .map(|route| (route.name(), *route))
        .collect(),
      state: (),
    }
  }
}

impl<S> Router<S> {
  /// Wraps every job run in `middleware`, inside any middleware added before it.
  pub fn with_middleware(mut self, middleware: impl Middleware) -> Self {
    self.middleware.push(Arc::new(middleware));
    self
  }

  pub fn with_state<T>(self, state: T) -> Router<T> {
    Router {
      middleware: self.middleware,
      routes: self.routes,
      state,
    }
  }
}

impl<S: Debug> Debug for Router<S> {
  fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
    f.debug_struct("Router")
      .field("middleware", &self.middleware.len())
      .field("routes", &self.routes)
      .field("state", &self.state)
      .finish()
  }
}

impl Default for Router {
  fn default() -> Self {
    Self::new()
  }
}

#[cfg(test)]
mod tests {
  use std::error::Error;

  use async_trait::async_trait;

  use super::*;
  use crate::{Json, Next, Run, State};

  struct Pass;

  #[async_trait]
  impl Middleware for Pass {
    async fn call(&self, _name: &str, _run: &Run, next: Next<'_>) -> Result<(), Box<dyn Error + Send + Sync>> {
      next.run().await
    }
  }

  #[crate::job]
  async fn deliver_report(Json(_): Json<String>) -> Result<(), Box<dyn Error + Send + Sync>> {
    Ok(())
  }

  #[crate::job]
  async fn prune_sessions(State(_): State<u8>) -> Result<(), Box<dyn Error + Send + Sync>> {
    Ok(())
  }

  mod router {
    use super::*;

    mod new {
      use super::*;

      #[test]
      fn it_lists_every_job_by_its_fn_name() {
        let router = Router::new();

        assert!(router.routes().contains_key("deliver_report"));
        assert!(router.routes().contains_key("prune_sessions"));
      }
    }

    mod with_state {
      use pretty_assertions::assert_eq;

      use super::*;

      #[test]
      fn it_keeps_the_jobs() {
        let router = Router::new();
        let names: Vec<_> = router.routes().keys().copied().collect();

        let router = router.with_state(7_u8);

        assert_eq!(router.routes().keys().copied().collect::<Vec<_>>(), names);
      }

      #[test]
      fn it_keeps_the_middleware() {
        let router = Router::new().with_middleware(Pass).with_state(7_u8);

        assert_eq!(router.middleware().len(), 1);
      }
    }
  }
}
