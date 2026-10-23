mod backend_type;
mod job_settings;
mod schedule;

use std::collections::HashMap;

pub use backend_type::BackendType;
use getset::{CopyGetters, Getters};
pub use job_settings::JobSettings;
pub use schedule::Schedule;
use serde::Deserialize;

/// The shape of the jerb config file.
///
/// Jerb never reads the file. The app loads it in whatever way it likes and deserializes it into `Settings`.
///
/// ```
/// let settings: jerb::Settings = toml::from_str(
///   r#"
///     backend = "sqlite"
///     url = "sqlite://jerb.sqlite?mode=rwc"
///     concurrency = 10
///
///     [queues]
///     critical = 5
///     default = 1
///
///     [jobs.send_email]
///     queue = "critical"
///     retries = 3
///     timeout = 30
///
///     [schedules.nightly_digest]
///     cron = "0 0 * * *"
///     job = "send_email"
///     args = { to = "team" }
///   "#,
/// )
/// .unwrap();
///
/// assert_eq!(settings.job("send_email").retries(), 3);
/// ```
#[derive(Clone, CopyGetters, Debug, Deserialize, Getters, PartialEq)]
pub struct Settings {
  /// The store jerb keeps its jobs in.
  #[get_copy = "pub"]
  backend: BackendType,
  /// How many jobs run at once in this process. Defaults to 5.
  #[get_copy = "pub"]
  #[serde(default = "default_concurrency")]
  concurrency: usize,
  #[get = "pub(crate)"]
  #[serde(default)]
  jobs: HashMap<String, JobSettings>,
  /// Each queue workers pull from, with its weight: workers pull from each queue in proportion to it. Defaults to the
  /// `default` queue at weight 1.
  #[get = "pub"]
  #[serde(default = "default_queues")]
  queues: HashMap<String, u32>,
  /// The recurring jobs, keyed by schedule name.
  #[get = "pub"]
  #[serde(default)]
  schedules: HashMap<String, Schedule>,
  /// The URL jerb connects to the store with.
  #[get = "pub"]
  url: String,
  /// How many workers run each queue named here, and only that queue. A queue never runs more jobs at once than its
  /// count. Queues with no entry share the `concurrency` workers by weight. A count of 0 stops the queue.
  #[get = "pub"]
  #[serde(default)]
  workers: HashMap<String, usize>,
}

impl Settings {
  /// The options the job named `name` runs with, or [`JobSettings::default`] when the config file has no entry for it.
  pub fn job(&self, name: &str) -> JobSettings {
    self.jobs.get(name).cloned().unwrap_or_default()
  }
}

const fn default_concurrency() -> usize {
  5
}

fn default_queues() -> HashMap<String, u32> {
  HashMap::from([("default".into(), 1)])
}

#[cfg(test)]
mod tests {
  use super::*;

  fn settings(source: &str) -> Settings {
    toml::from_str(source).unwrap()
  }

  mod settings {
    use super::*;

    const FULL: &str = r#"
      backend = "postgres"
      url = "postgres://localhost/jerb"
      concurrency = 20

      [queues]
      critical = 5
      default = 1

      [workers]
      critical = 2

      [jobs.send_email]
      queue = "critical"
      retries = 3
      timeout = 30

      [jobs.resize_image]
      retries = 0

      [schedules.nightly_digest]
      cron = "0 0 * * *"
      job = "send_email"
      args = { to = "team", count = 2 }
    "#;

    mod deserialize {
      use pretty_assertions::assert_eq;
      use serde_json::json;

      use super::*;

      #[test]
      fn it_reads_the_backend() {
        assert_eq!(settings(FULL).backend(), BackendType::Postgres);
      }

      #[test]
      fn it_reads_the_concurrency() {
        assert_eq!(settings(FULL).concurrency(), 20);
      }

      #[test]
      fn it_reads_the_queues_and_their_weights() {
        assert_eq!(
          settings(FULL).queues(),
          &HashMap::from([("critical".into(), 5), ("default".into(), 1)])
        );
      }

      #[test]
      fn it_reads_the_schedules() {
        let settings = settings(FULL);
        let digest = &settings.schedules()["nightly_digest"];

        assert_eq!(
          (digest.cron().as_str(), digest.job().as_str(), digest.args()),
          ("0 0 * * *", "send_email", &json!({ "to": "team", "count": 2 }))
        );
      }

      #[test]
      fn it_reads_the_workers_per_queue() {
        assert_eq!(settings(FULL).workers(), &HashMap::from([("critical".into(), 2)]));
      }

      #[test]
      fn it_reads_the_url() {
        assert_eq!(settings(FULL).url(), "postgres://localhost/jerb");
      }
    }

    mod job {
      use std::time::Duration;

      use pretty_assertions::assert_eq;

      use super::*;

      #[test]
      fn it_fills_keys_an_entry_leaves_out_from_the_defaults() {
        let job = settings(FULL).job("resize_image");

        assert_eq!(
          (job.queue().as_str(), job.retries(), job.timeout()),
          (
            JobSettings::default().queue().as_str(),
            0,
            JobSettings::default().timeout()
          )
        );
      }

      #[test]
      fn it_reads_an_entry() {
        let job = settings(FULL).job("send_email");

        assert_eq!(
          (job.queue().as_str(), job.retries(), job.timeout()),
          ("critical", 3, Duration::from_secs(30))
        );
      }

      #[test]
      fn it_returns_the_defaults_for_a_job_with_no_entry() {
        assert_eq!(settings(FULL).job("unknown"), JobSettings::default());
      }
    }
  }
}
