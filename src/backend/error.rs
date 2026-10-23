/// An error from a [`Backend`](super::Backend).
#[derive(Debug, thiserror::Error)]
pub enum Error {
  /// The store failed.
  #[cfg(any(feature = "mysql", feature = "postgres", feature = "sqlite"))]
  #[error(transparent)]
  Database(#[from] sea_orm::DbErr),
  /// A job the store holds has args that are not JSON.
  #[cfg(feature = "redis")]
  #[error(transparent)]
  Json(#[from] serde_json::Error),
  /// The job is not claimed, so it cannot finish or fail.
  #[error("job {0} is not running")]
  NotRunning(i64),
  /// A worker holds the job, so it cannot be deleted.
  #[error("job {0} is running")]
  Running(i64),
  /// The job is not scheduled, waiting to retry or dead, so it cannot run now.
  #[error("job {0} is not scheduled, waiting to retry or dead")]
  NotWaiting(i64),
  /// Redis failed.
  #[cfg(feature = "redis")]
  #[error(transparent)]
  Redis(#[from] redis::RedisError),
  /// The backend does not support the operation.
  #[error("the backend does not support {0}")]
  Unsupported(&'static str),
}
