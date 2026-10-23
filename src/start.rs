use std::{
  any::{Any, TypeId},
  backtrace::Backtrace,
  collections::HashMap,
  sync::Arc,
  time::{Duration, SystemTime},
};

#[cfg(feature = "redis")]
use crate::backend::Redis;
#[cfg(any(feature = "mysql", feature = "postgres", feature = "sqlite"))]
use crate::backend::Sql;
use crate::{
  BackendType, Client, Error, JobSettings, Next, Router, Run, Schedule, Settings,
  backend::{Backend, Job},
  busy::Busy,
  handler::HandlerFuture,
  queues::Queues,
  scheduler,
};

/// Checks `router` against `settings`, connects to the store the settings pick and spawns onto the running tokio
/// runtime a scheduler for each of the settings' schedules, as many shared workers as their concurrency and as many
/// workers as each queue in their `workers` names. Returns a [`Client`] that enqueues onto the same
/// store.
///
/// Fails when a job takes a `State` type other than the router's, when the settings name a job no fn defines, when
/// the settings pick a store whose Cargo feature is off, when the store cannot be reached, or when a schedule's cron
/// expression does not parse.
pub async fn start<S: Any + Send + Sync>(settings: Settings, router: Router<S>) -> Result<Client, Error> {
  check(&settings, &router)?;
  let backend = connect(&settings).await?;
  scheduler::start(backend.clone(), &settings)?;
  let router = Arc::new(router);
  let shared = Arc::new(settings.clone());
  let client = Client::new(backend.clone(), settings.clone());
  let spawn = |weights: HashMap<String, u32>, count: usize| {
    if weights.is_empty() {
      return;
    }
    for _ in 0..count {
      tokio::spawn(work(
        backend.clone(),
        router.clone(),
        shared.clone(),
        client.busy(),
        weights.clone(),
      ));
    }
  };
  spawn(
    settings
      .queues()
      .iter()
      .filter(|(name, _)| !settings.workers().contains_key(*name))
      .map(|(name, &weight)| (name.clone(), weight))
      .collect(),
    settings.concurrency(),
  );
  for (name, &count) in settings.workers() {
    spawn(HashMap::from([(name.clone(), 1)]), count);
  }
  Ok(client)
}

fn check<S: Any>(settings: &Settings, router: &Router<S>) -> Result<(), Error> {
  let state = TypeId::of::<S>();
  if let Some(route) = router
    .routes()
    .values()
    .find(|route| route.state().is_some_and(|taken| taken != state))
  {
    return Err(Error::State(route.name()));
  }
  let mut named = settings
    .jobs()
    .keys()
    .chain(settings.schedules().values().map(Schedule::job));
  match named.find(|name| !router.routes().contains_key(name.as_str())) {
    Some(name) => Err(Error::UnknownJob(name.clone())),
    None => Ok(()),
  }
}

async fn connect(settings: &Settings) -> Result<Arc<dyn Backend>, Error> {
  match settings.backend() {
    #[cfg(feature = "mysql")]
    BackendType::Mysql => Ok(Arc::new(Sql::connect(settings.url()).await?)),
    #[cfg(feature = "postgres")]
    BackendType::Postgres => Ok(Arc::new(Sql::connect(settings.url()).await?)),
    #[cfg(feature = "redis")]
    BackendType::Redis => Ok(Arc::new(Redis::connect(settings.url()).await?)),
    #[cfg(feature = "sqlite")]
    BackendType::Sqlite => Ok(Arc::new(Sql::connect(settings.url()).await?)),
    #[cfg(not(all(feature = "mysql", feature = "postgres", feature = "redis", feature = "sqlite")))]
    other => Err(Error::Unsupported(other)),
  }
}

async fn run<S: Any + Send + Sync>(backend: &dyn Backend, router: &Router<S>, options: &JobSettings, job: Job) {
  let result = match router.routes().get(job.name().as_str()) {
    Some(route) => {
      let run = Run::new()
        .with_args(job.args().clone())
        .with_attempt(job.attempt())
        .with_id(job.id().to_string());
      let call = route.call(run.clone(), router.state());
      let timeout = options.timeout();
      let job: HandlerFuture = Box::pin(async move {
        tokio::time::timeout(timeout, call)
          .await
          .unwrap_or_else(|elapsed| Err(elapsed.into()))
      });
      Next::new(router.middleware(), route.name(), &run, job).run().await
    }
    None => Err(format!("no fn defines job {}", job.name()).into()),
  };
  let backtrace = || Backtrace::force_capture().to_string();
  let _ = match result {
    Ok(()) => backend.finish(job.id()).await,
    Err(error) if job.attempt() < options.retries() => {
      let backoff = Duration::from_secs(u64::from(job.attempt()).pow(4) + 15);
      backend
        .retry(job.id(), &error.to_string(), &backtrace(), SystemTime::now() + backoff)
        .await
    }
    Err(error) => backend.fail(job.id(), &error.to_string(), &backtrace()).await,
  };
}

async fn work<S: Any + Send + Sync>(
  backend: Arc<dyn Backend>,
  router: Arc<Router<S>>,
  settings: Arc<Settings>,
  busy: Arc<Busy>,
  weights: HashMap<String, u32>,
) {
  let mut queues = Queues::new(&weights);
  loop {
    let _ = backend.reclaim(&|name| settings.job(name).timeout()).await;
    match queues.claim(&*backend).await {
      Ok(Some(job)) => {
        let _busy = busy.enter(job.queue());
        run(&*backend, &router, &settings.job(job.name()), job).await;
      }
      _ => tokio::time::sleep(Duration::from_secs(1)).await,
    }
  }
}

#[cfg(test)]
mod tests {
  use std::{error::Error, sync::Mutex};

  use async_trait::async_trait;
  use serde_json::{Value, json};

  use super::*;
  use crate::{Attempt, Json, backend};

  struct Note(Arc<Mutex<Vec<String>>>);

  #[async_trait]
  impl crate::Middleware for Note {
    async fn call(&self, name: &str, run: &Run, next: Next<'_>) -> Result<(), Box<dyn Error + Send + Sync>> {
      self.0.lock().unwrap().push(format!("before {name} {}", run.id()));
      let result = next.run().await;
      self.0.lock().unwrap().push(format!("after {result:?}"));
      result
    }
  }

  #[derive(Default)]
  struct Spy(Mutex<Vec<String>>);

  #[async_trait]
  impl Backend for Spy {
    async fn claim(&self, _queues: &[&str]) -> Result<Option<Job>, backend::Error> {
      Ok(None)
    }

    async fn completed(&self, _limit: u64) -> Result<Vec<Job>, backend::Error> {
      Ok(Vec::new())
    }

    async fn dead(&self) -> Result<Vec<Job>, backend::Error> {
      Ok(Vec::new())
    }

    async fn delete(&self, _id: i64) -> Result<(), backend::Error> {
      Ok(())
    }

    async fn fail(&self, id: i64, error: &str, _backtrace: &str) -> Result<(), backend::Error> {
      self.0.lock().unwrap().push(format!("fail {id}: {error}"));
      Ok(())
    }

    async fn finish(&self, id: i64) -> Result<(), backend::Error> {
      self.0.lock().unwrap().push(format!("finish {id}"));
      Ok(())
    }

    async fn fire(
      &self,
      _schedule: &str,
      _at: i64,
      _queue: &str,
      _name: &str,
      _args: Value,
    ) -> Result<Option<i64>, backend::Error> {
      Ok(None)
    }

    async fn fire_now(&self, _schedule: &str, _queue: &str, _name: &str, _args: Value) -> Result<i64, backend::Error> {
      Ok(0)
    }

    async fn last_run(
      &self,
      _schedule: &str,
    ) -> Result<Option<(std::time::SystemTime, backend::Outcome)>, backend::Error> {
      Ok(None)
    }

    async fn pause(&self, _queue: &str) -> Result<(), backend::Error> {
      Ok(())
    }

    async fn paused(&self) -> Result<Vec<String>, backend::Error> {
      Ok(Vec::new())
    }

    async fn push(&self, _queue: &str, _name: &str, _args: Value) -> Result<i64, backend::Error> {
      Ok(0)
    }

    async fn push_at(&self, _queue: &str, _name: &str, _args: Value, _at: SystemTime) -> Result<i64, backend::Error> {
      Ok(0)
    }

    async fn push_unique(&self, _queue: &str, _name: &str, _args: Value) -> Result<i64, backend::Error> {
      Ok(0)
    }

    async fn resume(&self, _queue: &str) -> Result<(), backend::Error> {
      Ok(())
    }

    async fn retries(&self) -> Result<Vec<Job>, backend::Error> {
      Ok(Vec::new())
    }

    async fn find(&self, _id: i64) -> Result<Option<Job>, backend::Error> {
      Ok(None)
    }

    async fn queued(&self) -> Result<Vec<Job>, backend::Error> {
      Ok(Vec::new())
    }

    async fn running(&self) -> Result<Vec<Job>, backend::Error> {
      Ok(Vec::new())
    }

    async fn retry(&self, id: i64, error: &str, _backtrace: &str, at: SystemTime) -> Result<(), backend::Error> {
      let wait = at
        .duration_since(SystemTime::now())
        .unwrap_or_default()
        .as_secs_f64()
        .round();
      self.0.lock().unwrap().push(format!("retry {id} in {wait}s: {error}"));
      Ok(())
    }

    async fn reclaim(&self, _timeout: &(dyn for<'a> Fn(&'a str) -> Duration + Sync)) -> Result<(), backend::Error> {
      Ok(())
    }

    async fn run_now(&self, _id: i64) -> Result<(), backend::Error> {
      Ok(())
    }

    async fn schedule_enabled(&self, _schedule: &str) -> Result<bool, backend::Error> {
      Ok(true)
    }

    async fn scheduled(&self) -> Result<Vec<Job>, backend::Error> {
      Ok(Vec::new())
    }

    async fn set_schedule_enabled(&self, _schedule: &str, _enabled: bool) -> Result<(), backend::Error> {
      Ok(())
    }

    async fn size(&self, _queue: &str) -> Result<u64, backend::Error> {
      Ok(0)
    }

    async fn stats(&self, _now: SystemTime) -> Result<backend::Stats, backend::Error> {
      Err(backend::Error::Unsupported("stats"))
    }
  }

  #[crate::job]
  async fn check_name(Json(name): Json<String>) -> Result<(), Box<dyn Error + Send + Sync>> {
    if name.is_empty() { Err("no name".into()) } else { Ok(()) }
  }

  #[crate::job]
  async fn hang() -> Result<(), Box<dyn Error + Send + Sync>> {
    tokio::time::sleep(Duration::from_secs(60)).await;
    Ok(())
  }

  #[crate::job]
  async fn pass_on_retry(Attempt(attempt): Attempt) -> Result<(), Box<dyn Error + Send + Sync>> {
    if attempt == 0 { Err("first try".into()) } else { Ok(()) }
  }

  mod run {
    use pretty_assertions::assert_eq;

    use super::*;

    async fn ran(name: &str, args: Value) -> Vec<String> {
      ran_with(Job::new(7, "default".into(), name.into(), args), JobSettings::default()).await
    }

    async fn ran_with(job: Job, options: JobSettings) -> Vec<String> {
      let backend = Spy::default();

      run(&backend, &Router::new(), &options, job).await;

      backend.0.into_inner().unwrap()
    }

    #[tokio::test]
    async fn it_finishes_a_job_that_passes_on_a_retry() {
      let job = Job::new(7, "default".into(), "pass_on_retry".into(), json!(null)).with_attempt(1);

      assert_eq!(ran_with(job, JobSettings::default()).await, vec!["finish 7"]);
    }

    #[tokio::test]
    async fn it_finishes_a_job_that_succeeds() {
      assert_eq!(ran("check_name", json!("ada")).await, vec!["finish 7"]);
    }

    #[tokio::test]
    async fn it_keeps_the_backtrace_of_a_dead_job_whose_handler_returns_an_error() {
      let backend = backend::Sql::connect("sqlite::memory:").await.unwrap();
      backend.push("default", "check_name", json!("")).await.unwrap();
      let job = backend.claim(&["default"]).await.unwrap().unwrap();
      let options = toml::from_str("retries = 0").unwrap();

      run(&backend, &Router::new(), &options, job).await;

      let dead = backend.dead().await.unwrap().remove(0);
      assert!(dead.backtrace().as_deref().is_some_and(|trace| !trace.is_empty()));
      assert_eq!(dead.error().as_deref(), Some("no name"));
    }

    #[tokio::test]
    async fn it_keeps_the_backtrace_of_a_retried_job_whose_handler_returns_an_error() {
      let backend = backend::Sql::connect("sqlite::memory:").await.unwrap();
      backend.push("default", "check_name", json!("")).await.unwrap();
      let job = backend.claim(&["default"]).await.unwrap().unwrap();

      run(&backend, &Router::new(), &JobSettings::default(), job).await;

      let retry = backend.retries().await.unwrap().remove(0);
      assert!(retry.backtrace().as_deref().is_some_and(|trace| !trace.is_empty()));
      assert_eq!(retry.error().as_deref(), Some("no name"));
    }

    #[tokio::test]
    async fn it_moves_a_job_out_of_retries_to_the_dead_set() {
      let job = Job::new(7, "default".into(), "check_name".into(), json!("")).with_attempt(2);
      let options = toml::from_str("retries = 2").unwrap();

      assert_eq!(ran_with(job, options).await, vec!["fail 7: no name"]);
    }

    #[tokio::test]
    async fn it_retries_a_job_no_fn_defines() {
      assert_eq!(
        ran("missing", json!(null)).await,
        vec!["retry 7 in 15s: no fn defines job missing"]
      );
    }

    #[tokio::test]
    async fn it_retries_a_job_that_returns_an_error() {
      assert_eq!(ran("check_name", json!("")).await, vec!["retry 7 in 15s: no name"]);
    }

    #[tokio::test]
    async fn it_retries_a_job_that_runs_past_its_timeout() {
      let job = Job::new(7, "default".into(), "hang".into(), json!(null));
      let options = toml::from_str("timeout = 0").unwrap();

      assert_eq!(
        ran_with(job, options).await,
        vec!["retry 7 in 15s: deadline has elapsed"]
      );
    }

    #[tokio::test]
    async fn it_runs_the_middleware_around_the_job() {
      let notes = Arc::default();
      let router = Router::new().with_middleware(Note(Arc::clone(&notes)));
      let job = Job::new(7, "default".into(), "check_name".into(), json!(""));

      run(&Spy::default(), &router, &JobSettings::default(), job).await;

      assert_eq!(
        *notes.lock().unwrap(),
        vec!["before check_name 7", "after Err(\"no name\")"]
      );
    }

    #[tokio::test]
    async fn it_waits_longer_after_each_attempt() {
      let job = Job::new(7, "default".into(), "check_name".into(), json!("")).with_attempt(2);

      assert_eq!(
        ran_with(job, JobSettings::default()).await,
        vec!["retry 7 in 31s: no name"]
      );
    }
  }
}
