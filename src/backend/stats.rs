use std::time::{Duration, SystemTime, UNIX_EPOCH};

use getset::{CopyGetters, Getters};

/// How many jobs ended in one clock hour.
#[derive(Clone, Copy, CopyGetters, Debug, Eq, PartialEq)]
pub struct HourStats {
  /// The jobs that ended dead in the hour.
  #[get_copy = "pub"]
  dead: u64,
  /// The jobs that finished in the hour.
  #[get_copy = "pub"]
  finished: u64,
  /// The start of the hour.
  #[get_copy = "pub"]
  start: SystemTime,
}

impl HourStats {
  pub(crate) const fn new(start: SystemTime) -> Self {
    Self {
      dead: 0,
      finished: 0,
      start,
    }
  }
}

/// Counts of the jobs a backend holds.
#[derive(Clone, CopyGetters, Debug, Eq, Getters, PartialEq)]
pub struct Stats {
  /// The jobs in the dead set.
  #[get_copy = "pub"]
  dead: u64,
  /// The jobs that finished and have not been deleted.
  #[get_copy = "pub"]
  finished: u64,
  /// The last 24 clock hours, oldest first, ending with the hour that holds `now`. Only a job that ends dead counts as
  /// dead, not each failed attempt that retried.
  #[get = "pub"]
  hours: Vec<HourStats>,
  /// The pending jobs whose time has come.
  #[get_copy = "pub"]
  pending: u64,
  /// The jobs that wait to run again after a failed run.
  #[get_copy = "pub"]
  retries: u64,
  /// The jobs a worker holds.
  #[get_copy = "pub"]
  running: u64,
  /// The jobs pushed for a later time that has not come.
  #[get_copy = "pub"]
  scheduled: u64,
}

impl Stats {
  pub(crate) const HOUR: Duration = Duration::from_secs(3600);

  /// Builds the stats from the state counts, in the order dead, finished, pending, retries, running, scheduled, and
  /// from the end time and outcome (`true` for dead) of each job that ended in the window.
  pub(crate) fn new(now: SystemTime, counts: [u64; 6], ended: impl IntoIterator<Item = (SystemTime, bool)>) -> Self {
    let hour = Self::HOUR.as_secs();
    let current = now
      .duration_since(UNIX_EPOCH)
      .map_or(0, |since| since.as_secs() / hour * hour);
    let mut hours: Vec<HourStats> = (0..24)
      .map(|ago| HourStats::new(UNIX_EPOCH + Duration::from_secs(current.saturating_sub((23 - ago) * hour))))
      .collect();
    for (at, dead) in ended {
      let Ok(since) = at.duration_since(hours[0].start) else {
        continue;
      };
      let Some(bucket) = hours.get_mut(usize::try_from(since.as_secs() / hour).unwrap_or(usize::MAX)) else {
        continue;
      };
      if dead {
        bucket.dead += 1;
      } else {
        bucket.finished += 1;
      }
    }
    let [dead, finished, pending, retries, running, scheduled] = counts;
    Self {
      dead,
      finished,
      hours,
      pending,
      retries,
      running,
      scheduled,
    }
  }
}
