use std::{
  error::Error,
  sync::Arc,
  time::{Duration, SystemTime},
};

use async_trait::async_trait;
use serde_json::Value;

use crate::{
  Enqueue, Error as JerbError, QueueWorkers, ScheduleStatus, Settings,
  backend::{Backend, Job, Stats},
  busy::Busy,
};

/// Enqueues jobs onto the store [`start`](crate::start) connected to, on the queue the settings give each job.
#[derive(Clone)]
pub struct Client {
  backend: Arc<dyn Backend>,
  busy: Arc<Busy>,
  settings: Settings,
}

impl Client {
  pub(crate) fn new(backend: Arc<dyn Backend>, settings: Settings) -> Self {
    Self {
      backend,
      busy: Arc::new(Busy::new(&settings)),
      settings,
    }
  }

  /// Lists up to `limit` finished jobs, the most recently finished first. Leaves out failed, pending and running jobs.
  /// Errors on a backend that does not support it.
  pub async fn completed(&self, limit: u64) -> Result<Vec<Job>, Box<dyn Error + Send + Sync>> {
    Ok(self.backend.completed(limit).await?)
  }

  /// Removes the job `id`, whether it is pending, scheduled, waiting to retry or dead, so it never runs, and frees its
  /// unique key. Errors and leaves the job alone when a worker is running it.
  pub async fn delete(&self, id: i64) -> Result<(), Box<dyn Error + Send + Sync>> {
    Ok(self.backend.delete(id).await?)
  }

  /// An axum router that serves a page listing the queues, schedules, scheduled jobs, retries and dead set of the store
  /// this client enqueues onto. The app nests it in its own router at the path it picks.
  #[cfg(feature = "dashboard")]
  pub fn dashboard(&self) -> axum::Router {
    crate::dashboard::router(self.backend.clone(), self.settings.clone())
  }

  /// Enqueues the job of the schedule `name` with its args now, even when the schedule is off, and returns the job id.
  /// The next cron fire is not affected. Errors when the settings name no such schedule, and on a backend that does not
  /// support it.
  pub async fn fire_now(&self, name: &str) -> Result<i64, Box<dyn Error + Send + Sync>> {
    let schedule = self
      .settings
      .schedules()
      .get(name)
      .ok_or_else(|| JerbError::UnknownSchedule(name.to_owned()))?;
    Ok(
      self
        .backend
        .fire_now(
          name,
          self.settings.job(schedule.job()).queue(),
          schedule.job(),
          schedule.args().clone(),
        )
        .await?,
    )
  }

  /// Makes a scheduled, waiting-to-retry or dead job due now, so the next claim runs it. Errors for a job that is
  /// running, finished or unknown, and on a backend that does not support it.
  pub async fn run_now(&self, id: i64) -> Result<(), Box<dyn Error + Send + Sync>> {
    Ok(self.backend.run_now(id).await?)
  }

  /// Tells whether the schedule `name` enqueues on its fires. A schedule never switched off is on.
  pub async fn schedule_enabled(&self, name: &str) -> Result<bool, Box<dyn Error + Send + Sync>> {
    Ok(self.backend.schedule_enabled(name).await?)
  }

  /// Lists each schedule in the settings with its cron expression, whether it is on, its last and next fire and how
  /// its last job ended, sorted by name. Errors on a backend that does not support it.
  pub async fn schedules(&self) -> Result<Vec<ScheduleStatus>, Box<dyn Error + Send + Sync>> {
    let mut names: Vec<_> = self.settings.schedules().keys().collect();
    names.sort();
    let mut statuses = Vec::new();
    for name in names {
      statuses.push(ScheduleStatus::new(
        name,
        &self.settings.schedules()[name],
        self.backend.schedule_enabled(name).await?,
        self.backend.last_run(name).await?,
      )?);
    }
    Ok(statuses)
  }

  /// Switches the schedule `name` on or off, for every process on the store. An off schedule enqueues nothing, and
  /// turning it on again waits for its next fire. Fails on a store that cannot hold the switch, such as Redis.
  pub async fn set_schedule_enabled(&self, name: &str, enabled: bool) -> Result<(), Box<dyn Error + Send + Sync>> {
    Ok(self.backend.set_schedule_enabled(name, enabled).await?)
  }

  /// Counts the jobs by state, and the jobs that finished or ended dead in each of the last 24 clock hours up to `now`.
  /// Errors on a backend that does not support it.
  pub async fn stats(&self, now: SystemTime) -> Result<Stats, Box<dyn Error + Send + Sync>> {
    Ok(self.backend.stats(now).await?)
  }

  /// Stores a new job named `name` with `args` that no worker runs before `at`, and returns its id.
  pub async fn enqueue_at(&self, at: SystemTime, name: &str, args: Value) -> Result<i64, Box<dyn Error + Send + Sync>> {
    Ok(
      self
        .backend
        .push_at(self.settings.job(name).queue(), name, args, at)
        .await?,
    )
  }

  /// Stores a new job named `name` with `args` that no worker runs until `delay` has passed, and returns its id.
  pub async fn enqueue_in(
    &self,
    delay: Duration,
    name: &str,
    args: Value,
  ) -> Result<i64, Box<dyn Error + Send + Sync>> {
    self.enqueue_at(SystemTime::now() + delay, name, args).await
  }

  /// Stops workers claiming jobs on `queue` until [`resume`](Self::resume). The queue keeps its jobs, and the pause
  /// holds across processes and restarts. Errors on a backend that does not support it.
  pub async fn pause(&self, queue: &str) -> Result<(), Box<dyn Error + Send + Sync>> {
    Ok(self.backend.pause(queue).await?)
  }

  /// Lists the paused queues, sorted by name.
  pub async fn paused(&self) -> Result<Vec<String>, Box<dyn Error + Send + Sync>> {
    Ok(self.backend.paused().await?)
  }

  /// Lists each queue with its configured and busy workers in this process, sorted by name.
  pub fn workers(&self) -> Vec<QueueWorkers> {
    self.busy.report(&self.settings)
  }

  pub(crate) fn busy(&self) -> Arc<Busy> {
    self.busy.clone()
  }

  /// Lets workers claim jobs on `queue` again.
  pub async fn resume(&self, queue: &str) -> Result<(), Box<dyn Error + Send + Sync>> {
    Ok(self.backend.resume(queue).await?)
  }

  /// Returns the job `id` in any state, or `None` when no such job exists. Errors on a backend that does not support
  /// it.
  pub async fn find(&self, id: i64) -> Result<Option<Job>, Box<dyn Error + Send + Sync>> {
    Ok(self.backend.find(id).await?)
  }

  /// Lists the pending jobs whose time has come, oldest first. Errors on a backend that does not support it.
  pub async fn queued(&self) -> Result<Vec<Job>, Box<dyn Error + Send + Sync>> {
    Ok(self.backend.queued().await?)
  }

  /// Lists the jobs a worker holds, oldest first. Errors on a backend that does not support it.
  pub async fn running(&self) -> Result<Vec<Job>, Box<dyn Error + Send + Sync>> {
    Ok(self.backend.running().await?)
  }

  /// Lists the pending jobs pushed for a later time that has not come, soonest first. Leaves out jobs waiting to run
  /// again after a failed run.
  pub async fn scheduled(&self) -> Result<Vec<Job>, Box<dyn Error + Send + Sync>> {
    Ok(self.backend.scheduled().await?)
  }

  /// Lists the jobs in the dead set, oldest first. Errors on a backend that does not support it.
  pub async fn dead(&self) -> Result<Vec<Job>, Box<dyn Error + Send + Sync>> {
    Ok(self.backend.dead().await?)
  }

  /// Lists the pending jobs that wait to run again after a failed run and whose time has not come, soonest first.
  /// Errors on a backend that does not support it.
  pub async fn retrying(&self) -> Result<Vec<Job>, Box<dyn Error + Send + Sync>> {
    Ok(self.backend.retries().await?)
  }
}

#[async_trait]
impl Enqueue for Client {
  async fn enqueue(&self, name: &str, args: Value) -> Result<i64, Box<dyn Error + Send + Sync>> {
    let job = self.settings.job(name);
    if job.unique() {
      Ok(self.backend.push_unique(job.queue(), name, args).await?)
    } else {
      Ok(self.backend.push(job.queue(), name, args).await?)
    }
  }
}

#[cfg(all(test, feature = "sqlite"))]
mod tests {
  use serde_json::json;

  use super::*;
  use crate::backend::Sql;

  async fn client(jobs: &str) -> (Client, Arc<Sql>) {
    let backend = Arc::new(Sql::connect("sqlite::memory:").await.unwrap());
    let settings = toml::from_str(&format!("backend = \"sqlite\"\nurl = \"sqlite::memory:\"\n{jobs}")).unwrap();
    (Client::new(backend.clone(), settings), backend)
  }

  mod client {
    use super::*;

    mod completed {
      use pretty_assertions::assert_eq;

      use super::*;

      #[tokio::test]
      async fn it_lists_finished_jobs() {
        let (client, backend) = client("").await;
        let id = backend.push("default", "send_email", Value::Null).await.unwrap();
        backend.claim(&["default"]).await.unwrap();
        backend.finish(id).await.unwrap();

        let completed: Vec<_> = client.completed(10).await.unwrap().iter().map(|job| job.id()).collect();

        assert_eq!(completed, vec![id]);
      }
    }

    mod enqueue {
      use pretty_assertions::assert_eq;

      use super::*;

      #[tokio::test]
      async fn it_pushes_the_job_onto_the_default_queue_when_the_settings_name_none() {
        let (client, backend) = client("").await;

        let id = client.enqueue("send_email", json!("ada")).await.unwrap();

        let job = backend.claim(&["default"]).await.unwrap().unwrap();
        assert_eq!(
          (job.id(), job.name().as_str(), job.args()),
          (id, "send_email", &json!("ada"))
        );
      }

      #[tokio::test]
      async fn it_pushes_the_job_onto_the_queue_the_settings_give_it() {
        let (client, backend) = client("[jobs.send_email]\nqueue = \"mailers\"").await;

        let id = client.enqueue("send_email", json!(null)).await.unwrap();

        let job = backend.claim(&["mailers"]).await.unwrap().unwrap();
        assert_eq!(job.id(), id);
      }

      #[tokio::test]
      async fn it_stores_a_unique_job_once_while_it_is_pending() {
        let (client, backend) = client("[jobs.send_email]\nunique = true").await;

        let first = client.enqueue("send_email", json!("ada")).await.unwrap();
        let second = client.enqueue("send_email", json!("ada")).await.unwrap();

        assert_eq!(second, first);
        assert_eq!(
          backend.claim(&["default"]).await.unwrap().map(|job| job.id()),
          Some(first)
        );
        assert_eq!(backend.claim(&["default"]).await.unwrap(), None);
      }
    }

    mod delete {
      use super::*;

      #[tokio::test]
      async fn it_removes_the_job() {
        let (client, backend) = client("").await;
        let id = client.enqueue("send_email", json!(null)).await.unwrap();

        client.delete(id).await.unwrap();

        assert_eq!(backend.claim(&["default"]).await.unwrap(), None);
      }

      #[tokio::test]
      async fn it_errors_for_a_running_job() {
        let (client, backend) = client("").await;
        client.enqueue("send_email", json!(null)).await.unwrap();
        let id = backend.claim(&["default"]).await.unwrap().unwrap().id();

        assert!(client.delete(id).await.is_err());
      }
    }

    mod enqueue_at {
      use pretty_assertions::assert_eq;

      use super::*;

      #[tokio::test]
      async fn it_holds_the_job_until_its_time() {
        let (client, backend) = client("").await;

        client
          .enqueue_at(SystemTime::now() + Duration::from_secs(60), "send_email", json!(null))
          .await
          .unwrap();

        assert_eq!(backend.claim(&["default"]).await.unwrap(), None);
      }

      #[tokio::test]
      async fn it_pushes_the_job_onto_the_queue_the_settings_give_it() {
        let (client, backend) = client("[jobs.send_email]\nqueue = \"mailers\"").await;

        let id = client
          .enqueue_at(SystemTime::now() - Duration::from_secs(1), "send_email", json!(null))
          .await
          .unwrap();

        let job = backend.claim(&["mailers"]).await.unwrap().unwrap();
        assert_eq!(job.id(), id);
      }
    }

    mod dead_and_retrying {
      use pretty_assertions::assert_eq;

      use super::*;

      #[tokio::test]
      async fn it_lists_each_job_in_its_own_set() {
        let (client, backend) = client("").await;
        let dead = client.enqueue("send_email", json!(null)).await.unwrap();
        let retrying = client.enqueue("send_email", json!(null)).await.unwrap();
        client.enqueue("send_email", json!(null)).await.unwrap();

        backend.claim(&["default"]).await.unwrap();
        backend.fail(dead, "boom", "trace").await.unwrap();
        backend.claim(&["default"]).await.unwrap();
        backend
          .retry(retrying, "boom", "trace", SystemTime::now() + Duration::from_secs(60))
          .await
          .unwrap();

        let dead_ids: Vec<_> = client.dead().await.unwrap().iter().map(|job| job.id()).collect();
        let retrying_ids: Vec<_> = client.retrying().await.unwrap().iter().map(|job| job.id()).collect();

        assert_eq!((dead_ids, retrying_ids), (vec![dead], vec![retrying]));
      }
    }

    mod scheduled {
      use pretty_assertions::assert_eq;

      use super::*;

      #[tokio::test]
      async fn it_lists_a_later_job_and_not_a_due_one() {
        let (client, _) = client("").await;
        let later = client
          .enqueue_at(SystemTime::now() + Duration::from_secs(60), "send_email", json!(null))
          .await
          .unwrap();
        let due = client.enqueue("send_email", json!(null)).await.unwrap();

        let scheduled: Vec<_> = client.scheduled().await.unwrap().iter().map(|job| job.id()).collect();
        let queued: Vec<_> = client.queued().await.unwrap().iter().map(|job| job.id()).collect();

        assert_eq!((scheduled, queued), (vec![later], vec![due]));
      }
    }

    mod stats {
      use pretty_assertions::assert_eq;

      use super::*;

      #[tokio::test]
      async fn it_counts_the_jobs_by_state() {
        let (client, _) = client("").await;
        client.enqueue("send_email", json!(null)).await.unwrap();

        let stats = client.stats(SystemTime::now()).await.unwrap();

        assert_eq!((stats.pending(), stats.dead(), stats.hours().len()), (1, 0, 24));
      }
    }

    mod run_now {
      use pretty_assertions::assert_eq;

      use super::*;

      #[tokio::test]
      async fn it_runs_a_scheduled_job_on_the_next_claim() {
        let (client, backend) = client("").await;
        let id = client
          .enqueue_in(Duration::from_secs(60), "send_email", json!(null))
          .await
          .unwrap();

        client.run_now(id).await.unwrap();

        assert_eq!(backend.claim(&["default"]).await.unwrap().map(|job| job.id()), Some(id));
      }

      #[tokio::test]
      async fn it_errors_for_a_running_job() {
        let (client, backend) = client("").await;
        let id = client.enqueue("send_email", json!(null)).await.unwrap();
        backend.claim(&["default"]).await.unwrap();

        assert!(client.run_now(id).await.is_err());
      }
    }

    mod pause {
      use pretty_assertions::assert_eq;

      use super::*;

      #[tokio::test]
      async fn it_stops_claims_on_the_queue_and_leaves_other_queues_running() {
        let (client, backend) = client("").await;
        client.enqueue("send_email", json!(null)).await.unwrap();
        let other = backend.push("mailers", "send_email", json!(null)).await.unwrap();

        client.pause("default").await.unwrap();

        assert_eq!(backend.claim(&["default"]).await.unwrap(), None);
        assert_eq!(
          backend
            .claim(&["default", "mailers"])
            .await
            .unwrap()
            .map(|job| job.id()),
          Some(other)
        );
      }

      #[tokio::test]
      async fn it_lists_the_queue_once_when_paused_twice() {
        let (client, _) = client("").await;

        client.pause("mailers").await.unwrap();
        client.pause("mailers").await.unwrap();
        client.pause("default").await.unwrap();

        assert_eq!(client.paused().await.unwrap(), ["default", "mailers"]);
      }
    }

    mod resume {
      use pretty_assertions::assert_eq;

      use super::*;

      #[tokio::test]
      async fn it_restores_claims_and_keeps_the_jobs() {
        let (client, backend) = client("").await;
        let id = client.enqueue("send_email", json!(null)).await.unwrap();
        client.pause("default").await.unwrap();

        client.resume("default").await.unwrap();

        assert_eq!(client.paused().await.unwrap(), Vec::<String>::new());
        assert_eq!(backend.claim(&["default"]).await.unwrap().map(|job| job.id()), Some(id));
      }
    }

    mod fire_now {
      use pretty_assertions::assert_eq;

      use super::*;

      const SCHEDULE: &str = "[jobs.send_email]\nqueue = \"mailers\"\n[schedules.digest]\ncron = \"0 0 1 1 *\"\njob = \"send_email\"\nargs = { to = \"team\" }";

      #[tokio::test]
      async fn it_enqueues_the_job_with_its_args_even_when_the_schedule_is_off() {
        let (client, backend) = client(SCHEDULE).await;
        client.set_schedule_enabled("digest", false).await.unwrap();

        let id = client.fire_now("digest").await.unwrap();

        let job = backend.claim(&["mailers"]).await.unwrap().unwrap();
        assert_eq!(
          (job.id(), job.name().as_str(), job.args()),
          (id, "send_email", &json!({ "to": "team" }))
        );
      }

      #[tokio::test]
      async fn it_errors_for_an_unknown_schedule() {
        let (client, _) = client(SCHEDULE).await;

        assert!(client.fire_now("sweep").await.is_err());
      }
    }

    mod schedules {
      use chrono::{DateTime, Timelike, Utc};
      use pretty_assertions::assert_eq;

      use super::*;
      use crate::backend::Outcome;

      const SCHEDULE: &str = "[schedules.digest]\ncron = \"30 4 * * *\"\njob = \"send_email\"";

      #[tokio::test]
      async fn it_gives_the_next_fire_the_cron_expression_matches() {
        let (client, _) = client(SCHEDULE).await;

        let status = client.schedules().await.unwrap().remove(0);

        let next = DateTime::<Utc>::from(status.next_fire().unwrap());
        assert_eq!((next.hour(), next.minute(), next.second()), (4, 30, 0));
        assert!(next > Utc::now());
        assert_eq!(status.cron().as_str(), "30 4 * * *");
      }

      #[tokio::test]
      async fn it_shows_no_fire_for_a_schedule_that_never_fired() {
        let (client, _) = client(SCHEDULE).await;

        let status = client.schedules().await.unwrap().remove(0);

        assert_eq!(
          (status.last_fire(), status.last_outcome(), status.enabled()),
          (None, None, true)
        );
      }

      #[tokio::test]
      async fn it_shows_the_job_of_the_last_fire_as_running_failed_or_ok() {
        let (client, backend) = client(SCHEDULE).await;
        let mut outcomes = Vec::new();

        client.fire_now("digest").await.unwrap();
        outcomes.push(client.schedules().await.unwrap()[0].last_outcome());
        let id = backend.claim(&["default"]).await.unwrap().unwrap().id();
        outcomes.push(client.schedules().await.unwrap()[0].last_outcome());
        backend.fail(id, "boom", "trace").await.unwrap();
        outcomes.push(client.schedules().await.unwrap()[0].last_outcome());
        let id = client.fire_now("digest").await.unwrap();
        backend.claim(&["default"]).await.unwrap();
        backend.finish(id).await.unwrap();
        outcomes.push(client.schedules().await.unwrap()[0].last_outcome());

        assert_eq!(
          outcomes,
          vec![
            Some(Outcome::Running),
            Some(Outcome::Running),
            Some(Outcome::Failed),
            Some(Outcome::Ok)
          ]
        );
      }

      #[tokio::test]
      async fn it_shows_when_the_schedule_is_off() {
        let (client, _) = client(SCHEDULE).await;
        client.set_schedule_enabled("digest", false).await.unwrap();

        assert!(!client.schedules().await.unwrap()[0].enabled());
      }
    }

    mod set_schedule_enabled {
      use super::*;

      #[tokio::test]
      async fn it_is_on_until_switched_off_and_on_again() {
        let (client, _) = client("").await;
        assert!(client.schedule_enabled("digest").await.unwrap());

        client.set_schedule_enabled("digest", false).await.unwrap();
        assert!(!client.schedule_enabled("digest").await.unwrap());

        client.set_schedule_enabled("digest", true).await.unwrap();
        assert!(client.schedule_enabled("digest").await.unwrap());
      }
    }

    mod enqueue_in {
      use pretty_assertions::assert_eq;

      use super::*;

      #[tokio::test]
      async fn it_holds_the_job_until_the_delay_passes() {
        let (client, backend) = client("").await;

        let id = client
          .enqueue_in(Duration::from_millis(100), "send_email", json!(null))
          .await
          .unwrap();

        assert_eq!(backend.claim(&["default"]).await.unwrap(), None);
        tokio::time::sleep(Duration::from_millis(110)).await;
        assert_eq!(backend.claim(&["default"]).await.unwrap().map(|job| job.id()), Some(id));
      }
    }
  }
}
