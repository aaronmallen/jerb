use std::{str::FromStr, sync::Arc};

use chrono::Utc;
use croner::Cron;

use crate::{Error, Schedule, Settings, backend::Backend};

pub(crate) fn start(backend: Arc<dyn Backend>, settings: &Settings) -> Result<(), Error> {
  let mut crons = Vec::new();
  for (name, schedule) in settings.schedules() {
    let cron = Cron::from_str(schedule.cron()).map_err(|source| Error::Cron(name.clone(), source))?;
    crons.push((name.clone(), schedule.clone(), cron));
  }
  for (name, schedule, cron) in crons {
    let queue = settings.job(schedule.job()).queue().clone();
    tokio::spawn(fire(backend.clone(), name, schedule, cron, queue));
  }
  Ok(())
}

async fn fire(backend: Arc<dyn Backend>, name: String, schedule: Schedule, cron: Cron, queue: String) {
  while let Ok(next) = cron.find_next_occurrence(&Utc::now(), false) {
    tokio::time::sleep((next - Utc::now()).to_std().unwrap_or_default()).await;
    let _ = backend
      .fire(&name, next.timestamp(), &queue, schedule.job(), schedule.args().clone())
      .await;
  }
}

#[cfg(all(test, feature = "sqlite"))]
mod tests {
  use std::time::Duration;

  use serde_json::{Value, json};

  use super::*;
  use crate::backend::Sql;

  async fn backend() -> Arc<Sql> {
    Arc::new(Sql::connect("sqlite::memory:").await.unwrap())
  }

  async fn claimed(backend: &Sql, queue: &str) -> Vec<(String, Value)> {
    let mut jobs = Vec::new();
    while let Some(job) = backend.claim(&[queue]).await.unwrap() {
      jobs.push((job.name().clone(), job.args().clone()));
    }
    jobs
  }

  fn settings(cron: &str) -> Settings {
    toml::from_str(&format!(
      r#"
        backend = "sqlite"
        url = "sqlite::memory:"

        [jobs.send_email]
        queue = "mailers"

        [schedules.digest]
        cron = "{cron}"
        job = "send_email"
        args = {{ to = "team" }}
      "#
    ))
    .unwrap()
  }

  mod start {
    use pretty_assertions::assert_eq;

    use super::*;

    #[tokio::test]
    async fn it_enqueues_the_job_with_its_args_on_each_fire() {
      let backend = backend().await;
      start(backend.clone(), &settings("* * * * * *")).unwrap();

      tokio::time::sleep(Duration::from_millis(2500)).await;

      let jobs = claimed(&backend, "mailers").await;
      assert!((2..=3).contains(&jobs.len()), "{jobs:?}");
      assert_eq!(jobs[0], ("send_email".to_string(), json!({ "to": "team" })));
    }

    #[tokio::test]
    async fn it_enqueues_each_fire_once_with_several_processes_running() {
      let backend = backend().await;
      for _ in 0..3 {
        start(backend.clone(), &settings("* * * * * *")).unwrap();
      }

      tokio::time::sleep(Duration::from_millis(2500)).await;

      let jobs = claimed(&backend, "mailers").await;
      assert!((2..=3).contains(&jobs.len()), "{jobs:?}");
    }

    #[tokio::test]
    async fn it_errors_when_a_cron_expression_does_not_parse() {
      let error = start(backend().await, &settings("not a cron")).unwrap_err();

      assert!(matches!(error, Error::Cron(name, _) if name == "digest"));
    }

    #[tokio::test]
    async fn it_enqueues_nothing_while_the_schedule_is_off_and_resumes_when_on() {
      let backend = backend().await;
      backend.set_schedule_enabled("digest", false).await.unwrap();
      start(backend.clone(), &settings("* * * * * *")).unwrap();

      tokio::time::sleep(Duration::from_millis(1500)).await;
      assert_eq!(claimed(&backend, "mailers").await, Vec::new());

      backend.set_schedule_enabled("digest", true).await.unwrap();
      tokio::time::sleep(Duration::from_millis(1500)).await;
      let jobs = claimed(&backend, "mailers").await;
      assert!((1..=2).contains(&jobs.len()), "{jobs:?}");
    }

    #[tokio::test]
    async fn it_skips_fires_missed_while_no_process_ran() {
      let backend = backend().await;
      let hour_ago = Utc::now().timestamp() - 3600;
      backend
        .fire("digest", hour_ago, "old", "send_email", Value::Null)
        .await
        .unwrap();
      start(backend.clone(), &settings("* * * * * *")).unwrap();

      tokio::time::sleep(Duration::from_millis(1500)).await;

      let jobs = claimed(&backend, "mailers").await;
      assert!((1..=2).contains(&jobs.len()), "{jobs:?}");
    }
  }
}
