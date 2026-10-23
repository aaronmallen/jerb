use std::{error::Error, sync::Arc};

use async_trait::async_trait;

use crate::{Run, handler::HandlerFuture};

/// Code that wraps each job run, added with [`Router::with_middleware`](crate::Router::with_middleware).
///
/// Code before `next.run().await` runs before the job, and code after it sees the job's result. The first middleware
/// added runs outermost.
#[async_trait]
pub trait Middleware: Send + Sync + 'static {
  /// Wraps the run of the job named `name`, whose id is `run.id()`.
  async fn call(&self, name: &str, run: &Run, next: Next<'_>) -> Result<(), Box<dyn Error + Send + Sync>>;
}

/// The rest of the middleware and the job itself.
pub struct Next<'a> {
  job: HandlerFuture,
  middleware: &'a [Arc<dyn Middleware>],
  name: &'a str,
  run: &'a Run,
}

impl<'a> Next<'a> {
  pub(crate) fn new(middleware: &'a [Arc<dyn Middleware>], name: &'a str, run: &'a Run, job: HandlerFuture) -> Self {
    Self {
      job,
      middleware,
      name,
      run,
    }
  }

  /// Runs the rest of the middleware and the job, and returns the job's result.
  pub async fn run(self) -> Result<(), Box<dyn Error + Send + Sync>> {
    match self.middleware.split_first() {
      Some((first, rest)) => {
        let (name, run) = (self.name, self.run);
        first.call(name, run, Next::new(rest, name, run, self.job)).await
      }
      None => self.job.await,
    }
  }
}

#[cfg(test)]
mod tests {
  use std::sync::Mutex;

  use super::*;

  type Seen = Arc<Mutex<Vec<String>>>;

  struct Log(&'static str, Seen);

  #[async_trait]
  impl Middleware for Log {
    async fn call(&self, name: &str, run: &Run, next: Next<'_>) -> Result<(), Box<dyn Error + Send + Sync>> {
      self
        .1
        .lock()
        .unwrap()
        .push(format!("{} before {name} {}", self.0, run.id()));
      let result = next.run().await;
      self.1.lock().unwrap().push(format!("{} after {result:?}", self.0));
      result
    }
  }

  fn job(seen: &Seen, result: Result<(), &'static str>) -> HandlerFuture {
    let seen = seen.clone();
    Box::pin(async move {
      seen.lock().unwrap().push("job".into());
      result.map_err(Into::into)
    })
  }

  mod next {
    use super::*;

    mod run {
      use pretty_assertions::assert_eq;

      use super::*;

      async fn ran(result: Result<(), &'static str>) -> Vec<String> {
        let seen = Seen::default();
        let middleware: Vec<Arc<dyn Middleware>> = vec![
          Arc::new(Log("outer", seen.clone())),
          Arc::new(Log("inner", seen.clone())),
        ];
        let run = Run::new().with_id("7");

        let _ = Next::new(&middleware, "send_email", &run, job(&seen, result))
          .run()
          .await;

        seen.lock().unwrap().clone()
      }

      #[tokio::test]
      async fn it_runs_each_middleware_around_the_job_in_order() {
        assert_eq!(
          ran(Ok(())).await,
          vec![
            "outer before send_email 7",
            "inner before send_email 7",
            "job",
            "inner after Ok(())",
            "outer after Ok(())",
          ]
        );
      }

      #[tokio::test]
      async fn it_runs_the_job_when_there_is_no_middleware() {
        let seen = Seen::default();
        let run = Run::new();

        let result = Next::new(&[], "send_email", &run, job(&seen, Err("boom"))).run().await;

        assert_eq!(
          (result.unwrap_err().to_string(), seen.lock().unwrap().clone()),
          ("boom".to_string(), vec!["job".to_string()])
        );
      }

      #[tokio::test]
      async fn it_shows_the_middleware_the_job_error() {
        assert_eq!(ran(Err("boom")).await[3], "inner after Err(\"boom\")");
      }
    }
  }
}
