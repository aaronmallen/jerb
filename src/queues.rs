use std::{cmp::Reverse, collections::HashMap};

use crate::backend::{Backend, Error, Job};

pub(crate) struct Queues(Vec<Queue>);

impl Queues {
  pub(crate) fn new(weights: &HashMap<String, u32>) -> Self {
    let mut queues: Vec<Queue> = weights
      .iter()
      .map(|(name, &weight)| Queue {
        current: 0,
        name: name.clone(),
        weight: weight.into(),
      })
      .collect();
    queues.sort_by(|a, b| a.name.cmp(&b.name));
    Self(queues)
  }

  pub(crate) async fn claim(&mut self, backend: &dyn Backend) -> Result<Option<Job>, Error> {
    for index in order(&mut self.0) {
      if let Some(job) = backend.claim(&[self.0[index].name.as_str()]).await? {
        return Ok(Some(job));
      }
    }
    Ok(None)
  }
}

struct Queue {
  current: i64,
  name: String,
  weight: i64,
}

fn order(queues: &mut [Queue]) -> Vec<usize> {
  let total: i64 = queues.iter().map(|queue| queue.weight).sum();
  for queue in queues.iter_mut() {
    queue.current += queue.weight;
  }
  let mut order: Vec<usize> = (0..queues.len()).collect();
  order.sort_by_key(|&index| Reverse(queues[index].current));
  if let Some(&first) = order.first() {
    queues[first].current -= total;
  }
  order
}

#[cfg(all(test, feature = "sqlite"))]
mod tests {
  use serde_json::json;

  use super::*;
  use crate::backend::Sql;

  async fn backend(pushed: &[(&str, usize)]) -> Sql {
    let backend = Sql::connect("sqlite::memory:").await.unwrap();
    for &(queue, count) in pushed {
      for _ in 0..count {
        backend.push(queue, "send_email", json!(null)).await.unwrap();
      }
    }
    backend
  }

  fn queues(weights: &[(&str, u32)]) -> Queues {
    Queues::new(&weights.iter().map(|&(name, weight)| (name.into(), weight)).collect())
  }

  mod queues {
    use super::*;

    mod claim {
      use pretty_assertions::assert_eq;

      use super::*;

      #[tokio::test]
      async fn it_claims_from_each_queue_in_proportion_to_its_weight() {
        let backend = backend(&[("critical", 100), ("default", 100), ("low", 100)]).await;
        let mut queues = queues(&[("critical", 5), ("default", 3), ("low", 1)]);
        let mut claimed: HashMap<String, usize> = HashMap::new();

        for _ in 0..90 {
          let job = queues.claim(&backend).await.unwrap().unwrap();
          *claimed.entry(job.queue().clone()).or_default() += 1;
        }

        assert_eq!(
          claimed,
          HashMap::from([("critical".into(), 50), ("default".into(), 30), ("low".into(), 10)])
        );
      }

      #[tokio::test]
      async fn it_falls_back_to_the_other_queues_when_the_pick_is_empty() {
        let backend = backend(&[("default", 1)]).await;
        let mut queues = queues(&[("critical", 5), ("default", 1)]);

        let job = queues.claim(&backend).await.unwrap().unwrap();

        assert_eq!(job.queue(), "default");
      }

      #[tokio::test]
      async fn it_returns_none_when_every_queue_is_empty() {
        let backend = backend(&[]).await;
        let mut queues = queues(&[("critical", 5), ("default", 1)]);

        assert_eq!(queues.claim(&backend).await.unwrap(), None);
      }
    }
  }
}
