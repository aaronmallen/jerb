use std::time::{Duration, SystemTime, UNIX_EPOCH};

use async_trait::async_trait;
use redis::{AsyncCommands, Client, FromRedisValue, Script, aio::ConnectionManager};
use serde_json::Value;

use super::{Backend, Error, Job, Outcome, Stats};

type Stored = (
  String,
  String,
  String,
  u32,
  Option<String>,
  Option<String>,
  Option<i64>,
  Option<i64>,
  Option<i64>,
);

/// A [`Backend`] on Redis, built on the redis crate.
pub struct Redis {
  connection: ConnectionManager,
  prefix: String,
}

impl Redis {
  /// Connects to the Redis server at `url`.
  pub async fn connect(url: &str) -> Result<Self, Error> {
    Self::open(url, "jerb:").await
  }

  async fn open(url: &str, prefix: &str) -> Result<Self, Error> {
    Ok(Self {
      connection: Client::open(url)?.get_connection_manager().await?,
      prefix: prefix.to_owned(),
    })
  }

  async fn waiting(&self) -> Result<Vec<Job>, Error> {
    let mut waiting: Vec<(i64, u64)> = self
      .connection
      .clone()
      .zrangebyscore_withscores(format!("{}scheduled", self.prefix), millis(SystemTime::now()), "+inf")
      .await?;
    waiting.sort_unstable_by_key(|&(id, at)| (at, id));
    self
      .load(waiting.into_iter().map(|(id, at)| (id, Some(at))).collect())
      .await
  }

  async fn store<T: FromRedisValue>(
    &self,
    queue: &str,
    name: &str,
    args: &Value,
    at: Option<SystemTime>,
    unique: bool,
    fire: Option<(&str, i64)>,
  ) -> Result<T, Error> {
    let (schedule, fired_at) = fire.unwrap_or_default();
    let args = args.to_string();
    Ok(
      Script::new(include_str!("redis/push.lua"))
        .arg(&self.prefix)
        .arg(queue)
        .arg(name)
        .arg(&args)
        .arg(at.map(millis).map_or_else(String::new, |at| at.to_string()))
        .arg(if unique {
          format!("{name}\0{args}")
        } else {
          String::new()
        })
        .arg(schedule)
        .arg(fired_at)
        .arg(millis(SystemTime::now()))
        .invoke_async(&mut self.connection.clone())
        .await?,
    )
  }

  async fn load(&self, ids: Vec<(i64, Option<u64>)>) -> Result<Vec<Job>, Error> {
    if ids.is_empty() {
      return Ok(Vec::new());
    }
    let mut connection = self.connection.clone();
    let mut pipe = redis::pipe();
    for (id, _) in &ids {
      pipe.hmget(
        format!("{}job:{id}", self.prefix),
        &[
          "queue",
          "name",
          "args",
          "attempt",
          "error",
          "backtrace",
          "enqueued_at",
          "started_at",
          "finished_at",
        ],
      );
    }
    let fields: Vec<Stored> = pipe.query_async(&mut connection).await?;
    ids
      .into_iter()
      .zip(fields)
      .map(
        |((id, run_at), (queue, name, args, attempt, error, backtrace, enqueued, started, finished))| {
          Ok(
            job((id, queue, name, args, attempt))?
              .with_error(error)
              .with_backtrace(backtrace)
              .with_times(enqueued, started, finished)
              .with_run_at(run_at.map(|at| UNIX_EPOCH + Duration::from_millis(at))),
          )
        },
      )
      .collect()
  }

  async fn settle(&self, id: i64, outcome: &str, error: &str, backtrace: &str, at: SystemTime) -> Result<(), Error> {
    let settled: bool = Script::new(include_str!("redis/settle.lua"))
      .arg(&self.prefix)
      .arg(id)
      .arg(outcome)
      .arg(error)
      .arg(backtrace)
      .arg(millis(at))
      .arg(millis(SystemTime::now()))
      .invoke_async(&mut self.connection.clone())
      .await?;
    if !settled {
      return Err(Error::NotRunning(id));
    }
    Ok(())
  }
}

#[async_trait]
impl Backend for Redis {
  async fn claim(&self, queues: &[&str]) -> Result<Option<Job>, Error> {
    let now = millis(SystemTime::now());
    let claimed: Option<(i64, String, String, String, u32, Option<i64>)> = Script::new(include_str!("redis/claim.lua"))
      .arg(&self.prefix)
      .arg(now)
      .arg(queues)
      .invoke_async(&mut self.connection.clone())
      .await?;
    claimed
      .map(|(id, queue, name, args, attempt, enqueued)| {
        Ok(job((id, queue, name, args, attempt))?.with_times(enqueued, Some(now), None))
      })
      .transpose()
  }

  async fn completed(&self, _limit: u64) -> Result<Vec<Job>, Error> {
    Err(Error::Unsupported("completed"))
  }

  async fn dead(&self) -> Result<Vec<Job>, Error> {
    let mut connection = self.connection.clone();
    let ids: Vec<i64> = connection.zrange(format!("{}dead", self.prefix), 0, -1).await?;
    self.load(ids.into_iter().map(|id| (id, None)).collect()).await
  }

  async fn delete(&self, _id: i64) -> Result<(), Error> {
    Err(Error::Unsupported("delete"))
  }

  async fn fail(&self, id: i64, error: &str, backtrace: &str) -> Result<(), Error> {
    self.settle(id, "failed", error, backtrace, UNIX_EPOCH).await
  }

  async fn finish(&self, id: i64) -> Result<(), Error> {
    self.settle(id, "finished", "", "", UNIX_EPOCH).await
  }

  async fn fire(&self, schedule: &str, at: i64, queue: &str, name: &str, args: Value) -> Result<Option<i64>, Error> {
    self.store(queue, name, &args, None, false, Some((schedule, at))).await
  }

  async fn fire_now(&self, _schedule: &str, _queue: &str, _name: &str, _args: Value) -> Result<i64, Error> {
    Err(Error::Unsupported("fire_now"))
  }

  async fn last_run(&self, _schedule: &str) -> Result<Option<(SystemTime, Outcome)>, Error> {
    Err(Error::Unsupported("last_run"))
  }

  async fn pause(&self, _queue: &str) -> Result<(), Error> {
    Err(Error::Unsupported("pause"))
  }

  async fn paused(&self) -> Result<Vec<String>, Error> {
    Err(Error::Unsupported("paused"))
  }

  async fn push(&self, queue: &str, name: &str, args: Value) -> Result<i64, Error> {
    self.store(queue, name, &args, None, false, None).await
  }

  async fn push_at(&self, queue: &str, name: &str, args: Value, at: SystemTime) -> Result<i64, Error> {
    self.store(queue, name, &args, Some(at), false, None).await
  }

  async fn push_unique(&self, queue: &str, name: &str, args: Value) -> Result<i64, Error> {
    self.store(queue, name, &args, None, true, None).await
  }

  async fn resume(&self, _queue: &str) -> Result<(), Error> {
    Err(Error::Unsupported("resume"))
  }

  async fn retries(&self) -> Result<Vec<Job>, Error> {
    Ok(
      self
        .waiting()
        .await?
        .into_iter()
        .filter(|job| job.attempt() > 0)
        .collect(),
    )
  }

  async fn retry(&self, id: i64, error: &str, backtrace: &str, at: SystemTime) -> Result<(), Error> {
    self.settle(id, "retry", error, backtrace, at).await
  }

  async fn reclaim(&self, timeout: &(dyn for<'a> Fn(&'a str) -> Duration + Sync)) -> Result<(), Error> {
    let mut connection = self.connection.clone();
    let ids: Vec<i64> = connection.zrange(format!("{}running", self.prefix), 0, -1).await?;
    if ids.is_empty() {
      return Ok(());
    }
    let mut pipe = redis::pipe();
    for id in &ids {
      pipe.hget(format!("{}job:{id}", self.prefix), "name");
    }
    let names: Vec<Option<String>> = pipe.query_async(&mut connection).await?;
    let script = Script::new(include_str!("redis/reclaim.lua"));
    let mut invocation = script.prepare_invoke();
    invocation.arg(&self.prefix);
    for (id, name) in ids.into_iter().zip(names) {
      if let Some(name) = name {
        invocation.arg(id).arg(millis(
          SystemTime::now().checked_sub(timeout(&name)).unwrap_or(UNIX_EPOCH),
        ));
      }
    }
    invocation.invoke_async::<()>(&mut connection).await?;
    Ok(())
  }

  async fn stats(&self, _now: SystemTime) -> Result<Stats, Error> {
    Err(Error::Unsupported("stats"))
  }

  async fn run_now(&self, _id: i64) -> Result<(), Error> {
    Err(Error::Unsupported("run_now"))
  }

  async fn schedule_enabled(&self, _schedule: &str) -> Result<bool, Error> {
    Ok(true)
  }

  async fn scheduled(&self) -> Result<Vec<Job>, Error> {
    Ok(
      self
        .waiting()
        .await?
        .into_iter()
        .filter(|job| job.attempt() == 0)
        .collect(),
    )
  }

  async fn set_schedule_enabled(&self, _schedule: &str, enabled: bool) -> Result<(), Error> {
    if enabled {
      Ok(())
    } else {
      Err(Error::Unsupported("switching a schedule off"))
    }
  }

  async fn find(&self, _id: i64) -> Result<Option<Job>, Error> {
    Err(Error::Unsupported("find"))
  }

  async fn queued(&self) -> Result<Vec<Job>, Error> {
    Err(Error::Unsupported("queued"))
  }

  async fn running(&self) -> Result<Vec<Job>, Error> {
    Err(Error::Unsupported("running"))
  }

  async fn size(&self, queue: &str) -> Result<u64, Error> {
    let mut connection = self.connection.clone();
    let ready: u64 = connection.zcard(format!("{}queue:{queue}", self.prefix)).await?;
    let due: Vec<i64> = connection
      .zrangebyscore(
        format!("{}scheduled", self.prefix),
        "-inf",
        format!("({}", millis(SystemTime::now())),
      )
      .await?;
    if due.is_empty() {
      return Ok(ready);
    }
    let mut pipe = redis::pipe();
    for id in &due {
      pipe.hget(format!("{}job:{id}", self.prefix), "queue");
    }
    let queues: Vec<Option<String>> = pipe.query_async(&mut connection).await?;
    Ok(ready + queues.iter().filter(|on| on.as_deref() == Some(queue)).count() as u64)
  }
}

fn job((id, queue, name, args, attempt): (i64, String, String, String, u32)) -> Result<Job, Error> {
  Ok(Job::new(id, queue, name, serde_json::from_str(&args)?).with_attempt(attempt))
}

fn millis(at: SystemTime) -> i64 {
  at.duration_since(UNIX_EPOCH)
    .map_or(0, |since| i64::try_from(since.as_millis()).unwrap_or(i64::MAX))
}

#[cfg(test)]
mod tests {
  use std::sync::atomic::{AtomicU64, Ordering};

  use super::*;

  static BACKENDS: AtomicU64 = AtomicU64::new(0);

  async fn backend() -> Option<Redis> {
    let url = std::env::var("JERB_TEST_REDIS_URL").ok()?;
    let prefix = format!(
      "jerb-test:{}:{}:{}:",
      std::process::id(),
      millis(SystemTime::now()),
      BACKENDS.fetch_add(1, Ordering::Relaxed)
    );
    Some(Redis::open(&url, &prefix).await.unwrap())
  }

  mod conformance {
    use super::*;

    crate::backend::conformance::conformance!(match backend().await {
      Some(backend) => backend,
      None => return,
    });
  }
}
