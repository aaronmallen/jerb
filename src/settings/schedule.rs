use getset::Getters;
use serde::Deserialize;
use serde_json::Value;

/// A recurring job, keyed by schedule name under `[schedules]` in the config file.
#[derive(Clone, Debug, Deserialize, Getters, PartialEq)]
pub struct Schedule {
  /// The args the job is enqueued with on each fire, `null` when the entry sets none.
  #[get = "pub"]
  #[serde(default)]
  args: Value,
  /// The cron expression the schedule fires on.
  #[get = "pub"]
  cron: String,
  /// The name of the job each fire enqueues.
  #[get = "pub"]
  job: String,
}
