use std::time::Duration;

use getset::{CopyGetters, Getters};
use serde::Deserialize;

/// The options one job runs with, keyed by job name under `[jobs]` in the config file.
///
/// A key left out of an entry, or a job with no entry at all, takes the value from [`JobSettings::default`]: the
/// `default` queue, 25 retries, a timeout of 1800 seconds and not unique.
#[derive(Clone, CopyGetters, Debug, Deserialize, Eq, Getters, PartialEq)]
#[serde(default)]
pub struct JobSettings {
  /// The queue the job is pushed to.
  #[get = "pub"]
  queue: String,
  /// How many times a failed run is retried before the job moves to the dead set.
  #[get_copy = "pub"]
  retries: u32,
  timeout: u64,
  /// Whether an enqueue is dropped while a job with the same name and args is already pending.
  #[get_copy = "pub"]
  unique: bool,
}

impl JobSettings {
  /// How long one run may take, set in the config file in seconds.
  pub const fn timeout(&self) -> Duration {
    Duration::from_secs(self.timeout)
  }
}

impl Default for JobSettings {
  fn default() -> Self {
    Self {
      queue: "default".into(),
      retries: 25,
      timeout: 1800,
      unique: false,
    }
  }
}
