use crate::{BackendType, backend};

/// An error from [`start`](crate::start).
#[derive(Debug, thiserror::Error)]
pub enum Error {
  /// The store failed.
  #[error(transparent)]
  Backend(#[from] backend::Error),
  /// A schedule in the settings has a cron expression croner cannot read.
  #[error("schedule {0} has a cron expression croner cannot read")]
  Cron(String, #[source] croner::errors::CronError),
  /// The job takes a `State` type other than the router's.
  #[error("job {0} takes a State type other than the router's")]
  State(&'static str),
  /// The settings name a job no `#[jerb::job]` fn defines.
  #[error("the settings name job {0}, which no fn defines")]
  UnknownJob(String),
  /// The settings name no such schedule.
  #[error("the settings name no schedule {0}")]
  UnknownSchedule(String),
  /// The settings pick a store whose Cargo feature is off.
  #[error("the {0:?} backend is not built in")]
  Unsupported(BackendType),
}
