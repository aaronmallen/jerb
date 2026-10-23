use std::time::{Duration, SystemTime, UNIX_EPOCH};

use getset::{CopyGetters, Getters};
use serde_json::Value;

/// A job a backend holds.
#[derive(Clone, CopyGetters, Debug, Getters, PartialEq)]
pub struct Job {
  /// The args the job was pushed with.
  #[get = "pub"]
  args: Value,
  /// How many times the job has run before this claim.
  #[get_copy = "pub"]
  attempt: u32,
  /// The backtrace of the last failed run, `None` when no run has failed.
  #[get = "pub"]
  backtrace: Option<String>,
  /// When the job was pushed, `None` for a job stored before jerb recorded it.
  #[get_copy = "pub"]
  enqueued_at: Option<SystemTime>,
  /// The error the last failed run returned, `None` when no run has failed.
  #[get = "pub"]
  error: Option<String>,
  /// When the last run ended in a finish, a failure or a retry, `None` when the job has not ended a run since its
  /// last claim.
  #[get_copy = "pub"]
  finished_at: Option<SystemTime>,
  /// The id the backend gave the job.
  #[get_copy = "pub"]
  id: i64,
  /// The name of the job fn.
  #[get = "pub"]
  name: String,
  /// The queue the job was pushed to.
  #[get = "pub"]
  queue: String,
  /// The time before which no claim returns the job, `None` when it was pushed to run now.
  #[get_copy = "pub"]
  run_at: Option<SystemTime>,
  /// When the job was last claimed, `None` when no worker has claimed it.
  #[get_copy = "pub"]
  started_at: Option<SystemTime>,
}

impl Job {
  pub(crate) const fn new(id: i64, queue: String, name: String, args: Value) -> Self {
    Self {
      args,
      attempt: 0,
      backtrace: None,
      enqueued_at: None,
      error: None,
      finished_at: None,
      id,
      name,
      queue,
      run_at: None,
      started_at: None,
    }
  }

  pub(crate) const fn with_attempt(mut self, attempt: u32) -> Self {
    self.attempt = attempt;
    self
  }

  pub(crate) fn with_backtrace(mut self, backtrace: Option<String>) -> Self {
    self.backtrace = backtrace;
    self
  }

  pub(crate) fn with_error(mut self, error: Option<String>) -> Self {
    self.error = error;
    self
  }

  /// Sets the enqueue, start and finish times from milliseconds since the Unix epoch.
  pub(crate) fn with_times(mut self, enqueued: Option<i64>, started: Option<i64>, finished: Option<i64>) -> Self {
    let time = |at: i64| u64::try_from(at).ok().map(|at| UNIX_EPOCH + Duration::from_millis(at));
    self.enqueued_at = enqueued.and_then(time);
    self.started_at = started.and_then(time);
    self.finished_at = finished.and_then(time);
    self
  }

  pub(crate) const fn with_run_at(mut self, run_at: Option<SystemTime>) -> Self {
    self.run_at = run_at;
    self
  }
}
