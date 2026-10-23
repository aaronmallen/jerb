#[cfg(test)]
mod conformance;
mod error;
mod job;
mod outcome;
#[cfg(feature = "redis")]
mod redis;
#[cfg(any(feature = "mysql", feature = "postgres", feature = "sqlite"))]
mod sql;
mod stats;

use std::time::{Duration, SystemTime};

use async_trait::async_trait;
pub use error::Error;
pub use job::Job;
pub use outcome::Outcome;
#[cfg(feature = "redis")]
pub use redis::Redis;
use serde_json::Value;
#[cfg(any(feature = "mysql", feature = "postgres", feature = "sqlite"))]
pub use sql::Sql;
pub use stats::{HourStats, Stats};

/// A store that holds jobs until a worker claims and runs them.
#[async_trait]
pub trait Backend: Send + Sync {
  /// Claims the oldest pending job on any of `queues` whose time has come, or returns `None` when there is none. No
  /// two calls ever claim the same job.
  async fn claim(&self, queues: &[&str]) -> Result<Option<Job>, Error>;

  /// Lists up to `limit` finished jobs, the most recently finished first. Leaves out failed, pending and running jobs.
  async fn completed(&self, limit: u64) -> Result<Vec<Job>, Error>;

  /// Lists the jobs in the dead set, oldest first.
  async fn dead(&self) -> Result<Vec<Job>, Error>;

  /// Removes the job `id` from the store, whether it is pending, scheduled, waiting to retry or dead, and frees its
  /// unique key. Does nothing when no such job exists. Errors with [`Error::Running`] and leaves the job alone when a
  /// worker holds it.
  async fn delete(&self, id: i64) -> Result<(), Error>;

  /// Moves a claimed job to the dead set and keeps `error` and `backtrace` with it.
  async fn fail(&self, id: i64, error: &str, backtrace: &str) -> Result<(), Error>;

  /// Marks a claimed job finished.
  async fn finish(&self, id: i64) -> Result<(), Error>;

  /// Stores a new pending job on `queue` for the fire of `schedule` at `at`, in Unix seconds, and returns its id.
  /// Returns `None` when this fire or a later one of `schedule` is already stored, so each fire enqueues once however
  /// many processes call it. Also returns `None` while `schedule` is off.
  async fn fire(&self, schedule: &str, at: i64, queue: &str, name: &str, args: Value) -> Result<Option<i64>, Error>;

  /// Stores a new pending job on `queue` for `schedule` now, whether `schedule` is on or off and without touching the
  /// fires it has stored, and returns its id.
  async fn fire_now(&self, schedule: &str, queue: &str, name: &str, args: Value) -> Result<i64, Error>;

  /// Tells when the newest job of `schedule` was enqueued and how it stands. Returns `None` when `schedule` has no job.
  async fn last_run(&self, schedule: &str) -> Result<Option<(SystemTime, Outcome)>, Error>;

  /// Stops claims from returning jobs on `queue` until it resumes. The jobs stay stored. Does nothing when `queue` is
  /// already paused.
  async fn pause(&self, queue: &str) -> Result<(), Error>;

  /// Lists the paused queues, sorted by name.
  async fn paused(&self) -> Result<Vec<String>, Error>;

  /// Stores a new pending job on `queue` and returns its id.
  async fn push(&self, queue: &str, name: &str, args: Value) -> Result<i64, Error>;

  /// Stores a new job on `queue` that no claim returns before `at`, and returns its id.
  async fn push_at(&self, queue: &str, name: &str, args: Value, at: SystemTime) -> Result<i64, Error>;

  /// Stores a new pending job on `queue` and returns its id, unless a job with the same `name` and `args` is already
  /// pending. Then it stores nothing and returns the pending job's id.
  async fn push_unique(&self, queue: &str, name: &str, args: Value) -> Result<i64, Error>;

  /// Lets claims return jobs on `queue` again. Does nothing when `queue` is not paused.
  async fn resume(&self, queue: &str) -> Result<(), Error>;

  /// Lists the pending jobs that wait to run again after a failed run and whose time has not come, soonest first.
  async fn retries(&self) -> Result<Vec<Job>, Error>;

  /// Puts a claimed job back as pending with `error` and `backtrace` kept and one more attempt counted, so that no claim returns it
  /// before `at`.
  async fn retry(&self, id: i64, error: &str, backtrace: &str, at: SystemTime) -> Result<(), Error>;

  /// Returns to pending each running job claimed at least `timeout` of its name ago, so a later claim runs it again
  /// when its worker died mid-run.
  async fn reclaim(&self, timeout: &(dyn for<'a> Fn(&'a str) -> Duration + Sync)) -> Result<(), Error>;

  /// Makes a scheduled, waiting-to-retry or dead job pending and due now, so the next claim runs it. A dead job keeps
  /// its attempt count and loses its error. Returns [`Error::NotWaiting`] for a running, finished or unknown job.
  async fn run_now(&self, id: i64) -> Result<(), Error>;

  /// Tells whether `schedule` enqueues on its fires. A schedule never switched off is on.
  async fn schedule_enabled(&self, schedule: &str) -> Result<bool, Error>;

  /// Lists the pending jobs pushed for a later time that has not come, soonest first. Leaves out jobs waiting to run
  /// again after a failed run.
  async fn scheduled(&self) -> Result<Vec<Job>, Error>;

  /// Switches `schedule` on or off. An off schedule enqueues nothing, and turning it on again waits for its next fire.
  async fn set_schedule_enabled(&self, schedule: &str, enabled: bool) -> Result<(), Error>;

  /// Counts the pending jobs on `queue` whose time has come.
  async fn size(&self, queue: &str) -> Result<u64, Error>;

  /// Returns the job `id` in any state, or `None` when no such job exists.
  async fn find(&self, id: i64) -> Result<Option<Job>, Error>;

  /// Lists the pending jobs whose time has come, oldest first.
  async fn queued(&self) -> Result<Vec<Job>, Error>;

  /// Lists the jobs a worker holds, oldest first.
  async fn running(&self) -> Result<Vec<Job>, Error>;

  /// Counts the jobs by state as of `now`, and the jobs that finished or ended dead in each of the last 24 clock hours.
  async fn stats(&self, now: SystemTime) -> Result<Stats, Error>;
}
