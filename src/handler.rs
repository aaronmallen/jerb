use std::{error::Error, pin::Pin};

use crate::{FromRun, Run};

pub(crate) type HandlerFuture = Pin<Box<dyn Future<Output = Result<(), Box<dyn Error + Send + Sync>>> + Send>>;

pub trait Handler<T, S>: Clone + Send + Sync + 'static {
  fn call(self, run: Run, state: S) -> HandlerFuture;
}

macro_rules! impl_handler {
  ($($extractor:ident),*) => {
    impl<F, Fut, E, S, $($extractor,)*> Handler<($($extractor,)*), S> for F
    where
      F: FnOnce($($extractor,)*) -> Fut + Clone + Send + Sync + 'static,
      Fut: Future<Output = Result<(), E>> + Send + 'static,
      E: Into<Box<dyn Error + Send + Sync>>,
      S: Send + 'static,
      $($extractor: FromRun<S> + Send + 'static,)*
    {
      #[allow(non_snake_case, unused_variables)]
      fn call(self, run: Run, state: S) -> HandlerFuture {
        Box::pin(async move {
          $(let $extractor = $extractor::from_run(&run, &state)?;)*
          self($($extractor,)*).await.map_err(Into::into)
        })
      }
    }
  };
}

impl_handler!();
impl_handler!(T1);
impl_handler!(T1, T2);
impl_handler!(T1, T2, T3);
impl_handler!(T1, T2, T3, T4);

#[cfg(test)]
mod tests {
  use serde_json::json;

  use super::*;
  use crate::{Attempt, JobId, Json, Router, State};

  #[derive(Debug)]
  struct Failed;

  impl std::fmt::Display for Failed {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
      write!(f, "failed")
    }
  }

  impl Error for Failed {}

  mod handler {
    use super::*;

    mod call {
      use std::sync::{Arc, Mutex};

      use pretty_assertions::assert_eq;

      use super::*;

      type Seen = Arc<Mutex<Vec<String>>>;

      #[tokio::test]
      async fn it_passes_the_args_and_state() {
        async fn job(State(seen): State<Seen>, Json(name): Json<String>) -> Result<(), Failed> {
          seen.lock().unwrap().push(name);
          Ok(())
        }
        let router = Router::new().with_state(Seen::default());
        let run = Run::new().with_args(json!("ada"));

        job.call(run, router.state().clone()).await.unwrap();

        assert_eq!(*router.state().lock().unwrap(), vec!["ada".to_string()]);
      }

      #[tokio::test]
      async fn it_passes_the_job_id_and_attempt() {
        async fn job(
          Attempt(attempt): Attempt,
          Json(()): Json<()>,
          JobId(id): JobId,
          State(seen): State<Seen>,
        ) -> Result<(), Failed> {
          seen.lock().unwrap().push(format!("{id}:{attempt}"));
          Ok(())
        }
        let seen = Seen::default();
        let run = Run::new().with_attempt(3).with_id("abc");

        job.call(run, seen.clone()).await.unwrap();

        assert_eq!(*seen.lock().unwrap(), vec!["abc:3".to_string()]);
      }

      #[tokio::test]
      async fn it_returns_an_error_when_the_args_do_not_decode() {
        async fn job(Json(_): Json<u32>) -> Result<(), Failed> {
          Ok(())
        }
        let run = Run::new().with_args(json!("not a number"));

        let error = job.call(run, ()).await.unwrap_err();

        assert!(error.is::<serde_json::Error>());
      }

      #[tokio::test]
      async fn it_returns_the_job_error() {
        async fn job() -> Result<(), Failed> {
          Err(Failed)
        }

        let error = job.call(Run::new(), ()).await.unwrap_err();

        assert!(error.is::<Failed>());
      }
    }
  }
}
