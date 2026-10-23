use std::{
  collections::HashMap,
  sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
  },
};

use getset::Getters;

use crate::Settings;

/// How many workers a queue has and how many of them run a job right now.
#[derive(Clone, Debug, Eq, Getters, PartialEq)]
pub struct QueueWorkers {
  /// The workers that run this queue. A queue with no entry in the settings' `workers` shares the `concurrency` workers
  /// with every other such queue, so this is `concurrency` for it.
  #[get = "pub"]
  configured: usize,
  /// How many of them run a job right now.
  #[get = "pub"]
  busy: usize,
  /// The queue's name.
  #[get = "pub"]
  queue: String,
}

/// The jobs running now, by queue, in this process.
pub(crate) struct Busy(HashMap<String, AtomicUsize>);

impl Busy {
  pub(crate) fn new(settings: &Settings) -> Self {
    Self(
      settings
        .queues()
        .keys()
        .chain(settings.workers().keys())
        .map(|name| (name.clone(), AtomicUsize::new(0)))
        .collect(),
    )
  }

  /// Counts a job on `queue` as running until the guard drops.
  pub(crate) fn enter(self: &Arc<Self>, queue: &str) -> Guard {
    if let Some(count) = self.0.get(queue) {
      count.fetch_add(1, Ordering::SeqCst);
    }
    Guard(self.clone(), queue.to_string())
  }

  pub(crate) fn report(&self, settings: &Settings) -> Vec<QueueWorkers> {
    let mut report: Vec<QueueWorkers> = self
      .0
      .iter()
      .map(|(name, count)| QueueWorkers {
        busy: count.load(Ordering::SeqCst),
        configured: settings.workers().get(name).copied().unwrap_or(settings.concurrency()),
        queue: name.clone(),
      })
      .collect();
    report.sort_by(|a, b| a.queue.cmp(&b.queue));
    report
  }
}

pub(crate) struct Guard(Arc<Busy>, String);

impl Drop for Guard {
  fn drop(&mut self) {
    if let Some(count) = self.0.0.get(&self.1) {
      count.fetch_sub(1, Ordering::SeqCst);
    }
  }
}

#[cfg(test)]
mod tests {
  use pretty_assertions::assert_eq;

  use super::*;

  #[test]
  fn it_reports_each_queue_with_its_configured_and_busy_workers() {
    let settings: Settings = toml::from_str(
      "backend = \"sqlite\"\nurl = \"x\"\nconcurrency = 4\n[queues]\ndefault = 1\n[workers]\nmailers = 2",
    )
    .unwrap();
    let busy = Arc::new(Busy::new(&settings));

    let _first = busy.enter("mailers");
    let _second = busy.enter("mailers");
    drop(busy.enter("default"));

    let report: Vec<_> = busy
      .report(&settings)
      .iter()
      .map(|q| (q.queue().clone(), *q.configured(), *q.busy()))
      .collect();
    assert_eq!(report, [("default".into(), 4, 0), ("mailers".into(), 2, 2)]);
  }
}
