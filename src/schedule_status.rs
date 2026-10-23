use std::{str::FromStr, time::SystemTime};

use chrono::Utc;
use croner::Cron;
use getset::{CopyGetters, Getters};

use crate::{Error, Schedule, backend::Outcome};

/// A schedule from the settings with what the store knows of its fires.
#[derive(Clone, CopyGetters, Debug, Getters, PartialEq)]
pub struct ScheduleStatus {
  /// The cron expression the schedule fires on.
  #[get = "pub"]
  cron: String,
  /// Whether the schedule enqueues on its fires.
  #[get_copy = "pub"]
  enabled: bool,
  /// The name of the job each fire enqueues.
  #[get = "pub"]
  job: String,
  /// When the schedule last enqueued its job, by a cron fire or by `fire_now`, `None` when it never has.
  #[get_copy = "pub"]
  last_fire: Option<SystemTime>,
  /// How the job of the last fire ended, `None` when the schedule never fired.
  #[get_copy = "pub"]
  last_outcome: Option<Outcome>,
  /// The schedule name.
  #[get = "pub"]
  name: String,
  /// The next time the cron expression matches.
  #[get_copy = "pub"]
  next_fire: Option<SystemTime>,
}

impl ScheduleStatus {
  pub(crate) fn new(
    name: &str,
    schedule: &Schedule,
    enabled: bool,
    last: Option<(SystemTime, Outcome)>,
  ) -> Result<Self, Error> {
    let cron = Cron::from_str(schedule.cron()).map_err(|source| Error::Cron(name.to_owned(), source))?;
    Ok(Self {
      cron: schedule.cron().clone(),
      enabled,
      job: schedule.job().clone(),
      last_fire: last.map(|(at, _)| at),
      last_outcome: last.map(|(_, outcome)| outcome),
      name: name.to_owned(),
      next_fire: cron.find_next_occurrence(&Utc::now(), false).ok().map(SystemTime::from),
    })
  }
}
