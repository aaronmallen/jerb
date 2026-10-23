mod entity;
mod migration;
mod paused;
mod schedule;

use std::time::{Duration, SystemTime, UNIX_EPOCH};

use async_trait::async_trait;
use entity::{ActiveModel, Column, Entity, Status};
use migration::Migrator;
use sea_orm::{
  ActiveModelTrait,
  ActiveValue::Set,
  ColumnTrait, Condition, ConnectionTrait, Database, DatabaseConnection, DbBackend, EntityTrait, ExprTrait,
  PaginatorTrait, QueryFilter, QueryOrder, QuerySelect, QueryTrait, Select, TransactionTrait, TryInsertResult,
  UpdateMany,
  sea_query::{Expr, IntoCondition, LockBehavior, LockType, OnConflict},
};
use sea_orm_migration::MigratorTrait;
use serde_json::Value;
use sha2::{Digest, Sha256};

use super::{Backend, Error, Job, Outcome, Stats};

/// A [`Backend`] on a SQL database, built on SeaORM.
pub struct Sql {
  db: DatabaseConnection,
}

impl Sql {
  /// Connects to the database at `url` and creates or updates jerb's tables.
  pub async fn connect(url: &str) -> Result<Self, Error> {
    let db = Database::connect(url).await?;
    Migrator::up(&db, None).await?;
    Ok(Self {
      db,
    })
  }
}

#[async_trait]
impl Backend for Sql {
  async fn claim(&self, queues: &[&str]) -> Result<Option<Job>, Error> {
    if self.db.get_database_backend() == DbBackend::MySql {
      return claim_locked(&self.db, queues).await;
    }
    let claimed = claiming(millis(SystemTime::now()))
      .filter(
        Column::Id.in_subquery(
          next(queues)
            .select_only()
            .column(Column::Id)
            .lock_with_behavior(LockType::Update, LockBehavior::SkipLocked)
            .into_query(),
        ),
      )
      .exec_with_returning(&self.db)
      .await?;
    Ok(claimed.into_iter().next().map(job))
  }

  async fn completed(&self, limit: u64) -> Result<Vec<Job>, Error> {
    let completed = Entity::find()
      .filter(Column::Status.eq(Status::Finished))
      .order_by_desc(Column::FinishedAt)
      .order_by_desc(Column::Id)
      .limit(limit)
      .all(&self.db)
      .await?;
    Ok(completed.into_iter().map(job).collect())
  }

  async fn dead(&self) -> Result<Vec<Job>, Error> {
    let dead = Entity::find()
      .filter(Column::Status.eq(Status::Failed))
      .order_by_asc(Column::Id)
      .all(&self.db)
      .await?;
    Ok(dead.into_iter().map(job).collect())
  }

  async fn delete(&self, id: i64) -> Result<(), Error> {
    let result = Entity::delete_many()
      .filter(Column::Id.eq(id))
      .filter(Column::Status.ne(Status::Running))
      .exec(&self.db)
      .await?;
    if result.rows_affected == 0 && Entity::find_by_id(id).one(&self.db).await?.is_some() {
      return Err(Error::Running(id));
    }
    Ok(())
  }

  async fn fail(&self, id: i64, error: &str, backtrace: &str) -> Result<(), Error> {
    settle(&self.db, id, Status::Failed, Some((error, backtrace))).await
  }

  async fn finish(&self, id: i64) -> Result<(), Error> {
    settle(&self.db, id, Status::Finished, None).await
  }

  async fn fire(&self, schedule: &str, at: i64, queue: &str, name: &str, args: Value) -> Result<Option<i64>, Error> {
    let txn = self.db.begin().await?;
    schedule::Entity::insert(schedule::ActiveModel {
      enabled: Set(true),
      fired_at: Set(i64::MIN),
      name: Set(schedule.to_owned()),
    })
    .on_conflict(
      OnConflict::column(schedule::Column::Name)
        .do_nothing_on([schedule::Column::Name])
        .to_owned(),
    )
    .exec_without_returning(&txn)
    .await?;
    let fired = schedule::Entity::update_many()
      .col_expr(schedule::Column::FiredAt, Expr::value(at))
      .filter(schedule::Column::Name.eq(schedule))
      .filter(schedule::Column::FiredAt.lt(at))
      .filter(schedule::Column::Enabled.eq(true))
      .exec(&txn)
      .await?;
    if fired.rows_affected == 0 {
      return Ok(None);
    }
    let id = insert_for(&txn, schedule, queue, name, args).await?;
    txn.commit().await?;
    Ok(Some(id))
  }

  async fn fire_now(&self, schedule: &str, queue: &str, name: &str, args: Value) -> Result<i64, Error> {
    insert_for(&self.db, schedule, queue, name, args).await
  }

  async fn last_run(&self, schedule: &str) -> Result<Option<(SystemTime, Outcome)>, Error> {
    let last = Entity::find()
      .filter(Column::Schedule.eq(schedule))
      .order_by_desc(Column::Id)
      .one(&self.db)
      .await?;
    Ok(last.and_then(|last| {
      let at = u64::try_from(last.enqueued_at?).ok()?;
      let outcome = match last.status {
        Status::Failed => Outcome::Failed,
        Status::Finished => Outcome::Ok,
        Status::Pending | Status::Running => Outcome::Running,
      };
      Some((UNIX_EPOCH + Duration::from_millis(at), outcome))
    }))
  }

  async fn pause(&self, queue: &str) -> Result<(), Error> {
    paused::Entity::insert(paused::ActiveModel {
      queue: Set(queue.to_owned()),
    })
    .on_conflict_do_nothing_on([paused::Column::Queue])
    .exec_without_returning(&self.db)
    .await?;
    Ok(())
  }

  async fn paused(&self) -> Result<Vec<String>, Error> {
    Ok(
      paused::Entity::find()
        .select_only()
        .column(paused::Column::Queue)
        .order_by_asc(paused::Column::Queue)
        .into_tuple()
        .all(&self.db)
        .await?,
    )
  }

  async fn push(&self, queue: &str, name: &str, args: Value) -> Result<i64, Error> {
    insert(&self.db, queue, name, args, None).await
  }

  async fn push_at(&self, queue: &str, name: &str, args: Value, at: SystemTime) -> Result<i64, Error> {
    insert(&self.db, queue, name, args, Some(millis(at))).await
  }

  async fn push_unique(&self, queue: &str, name: &str, args: Value) -> Result<i64, Error> {
    let key = format!(
      "{:x}",
      Sha256::new()
        .chain_update(name)
        .chain_update([0])
        .chain_update(args.to_string())
        .finalize()
    );
    loop {
      let job = ActiveModel {
        unique_key: Set(Some(key.clone())),
        ..pending(queue, name, args.clone())
      };
      if let TryInsertResult::Inserted(pushed) = Entity::insert(job)
        .on_conflict_do_nothing_on([Column::UniqueKey])
        .exec(&self.db)
        .await?
      {
        return Ok(pushed.last_insert_id);
      }
      if let Some(id) = Entity::find()
        .select_only()
        .column(Column::Id)
        .filter(Column::UniqueKey.eq(&key))
        .into_tuple()
        .one(&self.db)
        .await?
      {
        return Ok(id);
      }
    }
  }

  async fn resume(&self, queue: &str) -> Result<(), Error> {
    paused::Entity::delete_by_id(queue).exec(&self.db).await?;
    Ok(())
  }

  async fn retries(&self) -> Result<Vec<Job>, Error> {
    waiting(&self.db, Column::Attempt.gt(0)).await
  }

  async fn retry(&self, id: i64, error: &str, backtrace: &str, at: SystemTime) -> Result<(), Error> {
    let result = Entity::update_many()
      .col_expr(Column::Attempt, Expr::col(Column::Attempt).add(1))
      .col_expr(Column::Backtrace, Expr::value(backtrace))
      .col_expr(Column::Error, Expr::value(error))
      .col_expr(Column::FinishedAt, Expr::value(millis(SystemTime::now())))
      .col_expr(Column::RunAt, Expr::value(millis(at)))
      .col_expr(Column::Status, Expr::value(Status::Pending))
      .filter(Column::Id.eq(id))
      .filter(Column::Status.eq(Status::Running))
      .exec(&self.db)
      .await?;
    if result.rows_affected == 0 {
      return Err(Error::NotRunning(id));
    }
    Ok(())
  }

  async fn reclaim(&self, timeout: &(dyn for<'a> Fn(&'a str) -> Duration + Sync)) -> Result<(), Error> {
    let names: Vec<String> = Entity::find()
      .select_only()
      .column(Column::Name)
      .distinct()
      .filter(Column::Status.eq(Status::Running))
      .into_tuple()
      .all(&self.db)
      .await?;
    for name in names {
      Entity::update_many()
        .col_expr(Column::Status, Expr::value(Status::Pending))
        .filter(Column::Status.eq(Status::Running))
        .filter(Column::Name.eq(&name))
        .filter(Column::ClaimedAt.lte(millis(
          SystemTime::now().checked_sub(timeout(&name)).unwrap_or(UNIX_EPOCH),
        )))
        .exec(&self.db)
        .await?;
    }
    Ok(())
  }

  async fn run_now(&self, id: i64) -> Result<(), Error> {
    let dead = Entity::update_many()
      .col_expr(Column::Backtrace, Expr::value(Option::<String>::None))
      .col_expr(Column::Error, Expr::value(Option::<String>::None))
      .col_expr(Column::RunAt, Expr::value(Option::<i64>::None))
      .col_expr(Column::Status, Expr::value(Status::Pending))
      .filter(Column::Id.eq(id))
      .filter(Column::Status.eq(Status::Failed))
      .exec(&self.db)
      .await?;
    if dead.rows_affected > 0 {
      return Ok(());
    }
    let waiting = Entity::update_many()
      .col_expr(Column::RunAt, Expr::value(Option::<i64>::None))
      .filter(Column::Id.eq(id))
      .filter(Column::Status.eq(Status::Pending))
      .exec(&self.db)
      .await?;
    if waiting.rows_affected == 0 {
      return Err(Error::NotWaiting(id));
    }
    Ok(())
  }

  async fn schedule_enabled(&self, schedule: &str) -> Result<bool, Error> {
    let found = schedule::Entity::find_by_id(schedule).one(&self.db).await?;
    Ok(found.is_none_or(|found| found.enabled))
  }

  async fn scheduled(&self) -> Result<Vec<Job>, Error> {
    waiting(&self.db, Column::Attempt.eq(0)).await
  }

  async fn set_schedule_enabled(&self, schedule: &str, enabled: bool) -> Result<(), Error> {
    schedule::Entity::insert(schedule::ActiveModel {
      enabled: Set(enabled),
      fired_at: Set(i64::MIN),
      name: Set(schedule.to_owned()),
    })
    .on_conflict(
      OnConflict::column(schedule::Column::Name)
        .update_column(schedule::Column::Enabled)
        .to_owned(),
    )
    .exec_without_returning(&self.db)
    .await?;
    Ok(())
  }

  async fn stats(&self, now: SystemTime) -> Result<Stats, Error> {
    let at = millis(now);
    let count = |status: Status, cond: Condition| async move {
      Entity::find()
        .filter(Column::Status.eq(status))
        .filter(cond)
        .count(&self.db)
        .await
    };
    let waiting = |attempt: Condition| attempt.add(Column::RunAt.gte(at));
    let counts = [
      count(Status::Failed, Condition::all()).await?,
      count(Status::Finished, Condition::all()).await?,
      count(
        Status::Pending,
        Condition::any().add(Column::RunAt.is_null()).add(Column::RunAt.lt(at)),
      )
      .await?,
      count(Status::Pending, waiting(Condition::all().add(Column::Attempt.gt(0)))).await?,
      count(Status::Running, Condition::all()).await?,
      count(Status::Pending, waiting(Condition::all().add(Column::Attempt.eq(0)))).await?,
    ];
    let since = Stats::new(now, counts, []).hours()[0].start();
    let ended: Vec<(Status, Option<i64>)> = Entity::find()
      .select_only()
      .column(Column::Status)
      .column(Column::FinishedAt)
      .filter(Column::Status.is_in([Status::Failed, Status::Finished]))
      .filter(Column::FinishedAt.gte(millis(since)))
      .filter(Column::FinishedAt.lte(at))
      .into_tuple()
      .all(&self.db)
      .await?;
    let ended = ended.into_iter().filter_map(|(status, at)| {
      let at = UNIX_EPOCH + Duration::from_millis(u64::try_from(at?).ok()?);
      Some((at, status == Status::Failed))
    });
    Ok(Stats::new(now, counts, ended))
  }

  async fn size(&self, queue: &str) -> Result<u64, Error> {
    Ok(
      Entity::find()
        .filter(Column::Status.eq(Status::Pending))
        .filter(Column::Queue.eq(queue))
        .filter(due())
        .count(&self.db)
        .await?,
    )
  }

  async fn find(&self, id: i64) -> Result<Option<Job>, Error> {
    Ok(Entity::find_by_id(id).one(&self.db).await?.map(job))
  }

  async fn queued(&self) -> Result<Vec<Job>, Error> {
    let queued = Entity::find()
      .filter(Column::Status.eq(Status::Pending))
      .filter(due())
      .order_by_asc(Column::Id)
      .all(&self.db)
      .await?;
    Ok(queued.into_iter().map(job).collect())
  }

  async fn running(&self) -> Result<Vec<Job>, Error> {
    let running = Entity::find()
      .filter(Column::Status.eq(Status::Running))
      .order_by_asc(Column::Id)
      .all(&self.db)
      .await?;
    Ok(running.into_iter().map(job).collect())
  }
}

fn due() -> Condition {
  Condition::any()
    .add(Column::RunAt.is_null())
    .add(Column::RunAt.lt(millis(SystemTime::now())))
}

async fn claim_locked(db: &DatabaseConnection, queues: &[&str]) -> Result<Option<Job>, Error> {
  let txn = db.begin().await?;
  let Some(next) = next(queues)
    .lock_with_behavior(LockType::Update, LockBehavior::SkipLocked)
    .one(&txn)
    .await?
  else {
    return Ok(None);
  };
  let now = millis(SystemTime::now());
  claiming(now).filter(Column::Id.eq(next.id)).exec(&txn).await?;
  txn.commit().await?;
  Ok(Some(job(entity::Model {
    claimed_at: Some(now),
    finished_at: None,
    ..next
  })))
}

fn claiming(now: i64) -> UpdateMany<Entity> {
  Entity::update_many()
    .col_expr(Column::ClaimedAt, Expr::value(now))
    .col_expr(Column::FinishedAt, Expr::value(Option::<i64>::None))
    .col_expr(Column::Status, Expr::value(Status::Running))
    .col_expr(Column::UniqueKey, Expr::value(Option::<String>::None))
}

async fn insert(
  db: &impl ConnectionTrait,
  queue: &str,
  name: &str,
  args: Value,
  run_at: Option<i64>,
) -> Result<i64, Error> {
  let job = ActiveModel {
    run_at: Set(run_at),
    ..pending(queue, name, args)
  }
  .insert(db)
  .await?;
  Ok(job.id)
}

async fn insert_for(
  db: &impl ConnectionTrait,
  schedule: &str,
  queue: &str,
  name: &str,
  args: Value,
) -> Result<i64, Error> {
  let job = ActiveModel {
    schedule: Set(Some(schedule.to_owned())),
    ..pending(queue, name, args)
  }
  .insert(db)
  .await?;
  Ok(job.id)
}

fn job(model: entity::Model) -> Job {
  Job::new(model.id, model.queue, model.name, model.args)
    .with_attempt(model.attempt.cast_unsigned())
    .with_backtrace(model.backtrace)
    .with_error(model.error)
    .with_times(model.enqueued_at, model.claimed_at, model.finished_at)
    .with_run_at(
      model
        .run_at
        .and_then(|at| u64::try_from(at).ok())
        .map(|at| UNIX_EPOCH + Duration::from_millis(at)),
    )
}

fn millis(at: SystemTime) -> i64 {
  at.duration_since(UNIX_EPOCH)
    .map_or(0, |since| i64::try_from(since.as_millis()).unwrap_or(i64::MAX))
}

fn next(queues: &[&str]) -> Select<Entity> {
  Entity::find()
    .filter(Column::Status.eq(Status::Pending))
    .filter(Column::Queue.is_in(queues.iter().copied()))
    .filter(
      Column::Queue.not_in_subquery(
        paused::Entity::find()
          .select_only()
          .column(paused::Column::Queue)
          .into_query(),
      ),
    )
    .filter(due())
    .order_by_asc(Column::Id)
    .limit(1)
}

fn pending(queue: &str, name: &str, args: Value) -> ActiveModel {
  ActiveModel {
    args: Set(args),
    enqueued_at: Set(Some(millis(SystemTime::now()))),
    name: Set(name.to_owned()),
    queue: Set(queue.to_owned()),
    status: Set(Status::Pending),
    ..Default::default()
  }
}

async fn settle(db: &DatabaseConnection, id: i64, status: Status, failure: Option<(&str, &str)>) -> Result<(), Error> {
  let result = Entity::update_many()
    .col_expr(Column::Status, Expr::value(status))
    .col_expr(
      Column::Backtrace,
      Expr::value(failure.map(|(_, backtrace)| backtrace.to_owned())),
    )
    .col_expr(Column::Error, Expr::value(failure.map(|(error, _)| error.to_owned())))
    .col_expr(Column::FinishedAt, Expr::value(millis(SystemTime::now())))
    .filter(Column::Id.eq(id))
    .filter(Column::Status.eq(Status::Running))
    .exec(db)
    .await?;
  if result.rows_affected == 0 {
    return Err(Error::NotRunning(id));
  }
  Ok(())
}

async fn waiting(db: &DatabaseConnection, attempt: impl IntoCondition) -> Result<Vec<Job>, Error> {
  let waiting = Entity::find()
    .filter(Column::Status.eq(Status::Pending))
    .filter(attempt)
    .filter(Column::RunAt.gte(millis(SystemTime::now())))
    .order_by_asc(Column::RunAt)
    .order_by_asc(Column::Id)
    .all(db)
    .await?;
  Ok(waiting.into_iter().map(job).collect())
}

#[cfg(test)]
mod tests {
  use super::*;

  async fn backend() -> Sql {
    Sql::connect("sqlite::memory:").await.unwrap()
  }

  async fn stored(backend: &Sql, id: i64) -> entity::Model {
    Entity::find_by_id(id).one(&backend.db).await.unwrap().unwrap()
  }

  mod conformance {
    use super::*;

    #[cfg(feature = "mysql")]
    mod mysql {
      use super::*;

      async fn backend() -> Option<Sql> {
        let url = std::env::var("JERB_TEST_MYSQL_URL").ok()?;
        let name = format!(
          "jerb_test_{}_{}",
          std::process::id(),
          SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos()
        );
        Database::connect(&url)
          .await
          .unwrap()
          .execute_unprepared(&format!("CREATE DATABASE {name}"))
          .await
          .unwrap();
        Some(Sql::connect(&format!("{url}/{name}")).await.unwrap())
      }

      crate::backend::conformance::conformance!(match backend().await {
        Some(backend) => backend,
        None => return,
      });
    }

    #[cfg(feature = "postgres")]
    mod postgres {
      use super::*;

      async fn backend() -> Option<Sql> {
        let url = std::env::var("JERB_TEST_POSTGRES_URL").ok()?;
        let name = format!(
          "jerb_test_{}_{}",
          std::process::id(),
          SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos()
        );
        Database::connect(&url)
          .await
          .unwrap()
          .execute_unprepared(&format!("CREATE DATABASE {name}"))
          .await
          .unwrap();
        let (server, _) = url.rsplit_once('/').unwrap();
        Some(Sql::connect(&format!("{server}/{name}")).await.unwrap())
      }

      crate::backend::conformance::conformance!(match backend().await {
        Some(backend) => backend,
        None => return,
      });
    }

    mod sqlite {
      use super::*;

      crate::backend::conformance::conformance!(backend().await);
    }
  }

  mod sql {
    use super::*;

    mod completed {
      use pretty_assertions::assert_eq;

      use super::*;

      async fn finished(backend: &Sql) -> i64 {
        let id = backend.push("default", "send_email", Value::Null).await.unwrap();
        backend.claim(&["default"]).await.unwrap();
        backend.finish(id).await.unwrap();
        tokio::time::sleep(Duration::from_millis(5)).await;
        id
      }

      #[tokio::test]
      async fn it_lists_the_newest_finished_job_first_and_stops_at_the_limit() {
        let backend = backend().await;
        finished(&backend).await;
        let second = finished(&backend).await;
        let third = finished(&backend).await;

        let completed: Vec<_> = backend.completed(2).await.unwrap().iter().map(|job| job.id()).collect();

        assert_eq!(completed, vec![third, second]);
      }

      #[tokio::test]
      async fn it_leaves_out_failed_pending_and_running_jobs() {
        let backend = backend().await;
        let failed = backend.push("default", "send_email", Value::Null).await.unwrap();
        backend.claim(&["default"]).await.unwrap();
        backend.fail(failed, "boom", "trace").await.unwrap();
        backend.push("default", "send_email", Value::Null).await.unwrap();
        backend.push("default", "send_email", Value::Null).await.unwrap();
        backend.claim(&["default"]).await.unwrap();

        assert_eq!(backend.completed(10).await.unwrap(), Vec::new());
      }
    }

    mod listing {
      use pretty_assertions::assert_eq;

      use super::*;

      async fn ids(jobs: Result<Vec<Job>, Error>) -> Vec<i64> {
        jobs.unwrap().iter().map(Job::id).collect()
      }

      #[tokio::test]
      async fn it_lists_each_job_in_its_own_state_only() {
        let backend = backend().await;
        let later = SystemTime::now() + Duration::from_secs(60);
        let running = backend.push("default", "a", Value::Null).await.unwrap();
        backend.claim(&["default"]).await.unwrap();
        let queued = backend.push("default", "b", Value::Null).await.unwrap();
        let scheduled = backend.push_at("default", "c", Value::Null, later).await.unwrap();
        let finished = backend.push("other", "d", Value::Null).await.unwrap();
        backend.claim(&["other"]).await.unwrap();
        backend.finish(finished).await.unwrap();
        let dead = backend.push("other", "e", Value::Null).await.unwrap();
        backend.claim(&["other"]).await.unwrap();
        backend.fail(dead, "boom", "trace").await.unwrap();

        assert_eq!(ids(backend.running().await).await, vec![running]);
        assert_eq!(ids(backend.queued().await).await, vec![queued]);
        assert_eq!(ids(backend.scheduled().await).await, vec![scheduled]);
      }

      #[tokio::test]
      async fn it_lists_a_due_scheduled_job_as_queued() {
        let backend = backend().await;
        let id = backend
          .push_at("default", "a", Value::Null, SystemTime::now() - Duration::from_secs(1))
          .await
          .unwrap();

        assert_eq!(ids(backend.queued().await).await, vec![id]);
        assert_eq!(ids(backend.scheduled().await).await, Vec::<i64>::new());
      }

      #[tokio::test]
      async fn it_lists_oldest_first() {
        let backend = backend().await;
        let first = backend.push("default", "a", Value::Null).await.unwrap();
        let second = backend.push("other", "a", Value::Null).await.unwrap();

        assert_eq!(ids(backend.queued().await).await, vec![first, second]);
      }

      #[tokio::test]
      async fn it_finds_a_job_in_any_state() {
        let backend = backend().await;
        let pending = backend.push("default", "a", Value::Null).await.unwrap();
        let running = backend.push("other", "a", Value::Null).await.unwrap();
        backend.claim(&["other"]).await.unwrap();
        let dead = backend.push("third", "a", Value::Null).await.unwrap();
        backend.claim(&["third"]).await.unwrap();
        backend.fail(dead, "boom", "trace").await.unwrap();
        let finished = backend.push("fourth", "a", Value::Null).await.unwrap();
        backend.claim(&["fourth"]).await.unwrap();
        backend.finish(finished).await.unwrap();

        for id in [pending, running, dead, finished] {
          assert_eq!(backend.find(id).await.unwrap().map(|job| job.id()), Some(id));
        }
      }

      #[tokio::test]
      async fn it_finds_nothing_for_an_unknown_id() {
        assert_eq!(backend().await.find(99).await.unwrap(), None);
      }
    }

    mod delete {
      use pretty_assertions::assert_eq;

      use super::*;

      #[tokio::test]
      async fn it_removes_a_dead_job() {
        let backend = backend().await;
        let id = backend.push("default", "send_email", Value::Null).await.unwrap();
        backend.claim(&["default"]).await.unwrap();
        backend.fail(id, "boom", "trace").await.unwrap();

        backend.delete(id).await.unwrap();

        assert_eq!(backend.dead().await.unwrap(), Vec::new());
      }

      #[tokio::test]
      async fn it_removes_a_pending_job_so_it_never_runs() {
        let backend = backend().await;
        let id = backend.push("default", "send_email", Value::Null).await.unwrap();

        backend.delete(id).await.unwrap();

        assert_eq!(backend.claim(&["default"]).await.unwrap(), None);
      }

      #[tokio::test]
      async fn it_removes_a_scheduled_job() {
        let backend = backend().await;
        let at = SystemTime::now() + Duration::from_secs(60);
        let id = backend.push_at("default", "send_email", Value::Null, at).await.unwrap();

        backend.delete(id).await.unwrap();

        assert_eq!(backend.scheduled().await.unwrap(), Vec::new());
      }

      #[tokio::test]
      async fn it_removes_a_job_waiting_to_retry() {
        let backend = backend().await;
        let id = backend.push("default", "send_email", Value::Null).await.unwrap();
        backend.claim(&["default"]).await.unwrap();
        backend
          .retry(id, "boom", "trace", SystemTime::now() + Duration::from_secs(60))
          .await
          .unwrap();

        backend.delete(id).await.unwrap();

        assert_eq!(backend.retries().await.unwrap(), Vec::new());
      }

      #[tokio::test]
      async fn it_errors_and_keeps_a_running_job() {
        let backend = backend().await;
        let id = backend.push("default", "send_email", Value::Null).await.unwrap();
        backend.claim(&["default"]).await.unwrap();

        assert!(matches!(backend.delete(id).await, Err(Error::Running(n)) if n == id));
        assert_eq!(stored(&backend, id).await.status, Status::Running);
      }

      #[tokio::test]
      async fn it_frees_the_unique_key() {
        let backend = backend().await;
        let first = backend.push_unique("default", "send_email", Value::Null).await.unwrap();

        backend.delete(first).await.unwrap();
        let second = backend.push_unique("default", "send_email", Value::Null).await.unwrap();

        assert_ne!(second, first);
      }

      #[tokio::test]
      async fn it_ignores_a_missing_job() {
        backend().await.delete(42).await.unwrap();
      }
    }

    mod fire_now {
      use pretty_assertions::assert_eq;

      use super::*;

      #[tokio::test]
      async fn it_enqueues_while_the_schedule_is_off_and_keeps_the_stored_fires() {
        let backend = backend().await;
        backend
          .fire("digest", 10, "default", "send_email", Value::Null)
          .await
          .unwrap();
        backend.set_schedule_enabled("digest", false).await.unwrap();

        backend
          .fire_now("digest", "default", "send_email", Value::Null)
          .await
          .unwrap();
        backend.set_schedule_enabled("digest", true).await.unwrap();

        assert_eq!(
          backend
            .fire("digest", 10, "default", "send_email", Value::Null)
            .await
            .unwrap(),
          None
        );
        assert!(
          backend
            .fire("digest", 11, "default", "send_email", Value::Null)
            .await
            .unwrap()
            .is_some()
        );
      }
    }

    mod last_run {
      use pretty_assertions::assert_eq;

      use super::*;

      #[tokio::test]
      async fn it_returns_none_for_a_schedule_with_no_job() {
        let backend = backend().await;
        backend.push("default", "send_email", Value::Null).await.unwrap();

        assert_eq!(backend.last_run("digest").await.unwrap(), None);
      }
    }

    mod pause {
      use pretty_assertions::assert_eq;

      use super::*;

      #[tokio::test]
      async fn it_holds_after_a_reconnect() {
        let path = std::env::temp_dir().join(format!("jerb-36-{}.db", std::process::id()));
        let url = format!("sqlite://{}?mode=rwc", path.display());
        let first = Sql::connect(&url).await.unwrap();
        let id = first.push("default", "send_email", Value::Null).await.unwrap();
        first.pause("default").await.unwrap();
        drop(first);

        let second = Sql::connect(&url).await.unwrap();
        let before = second.claim(&["default"]).await.unwrap();
        second.resume("default").await.unwrap();
        let after = second.claim(&["default"]).await.unwrap().map(|job| job.id());
        std::fs::remove_file(&path).unwrap();

        assert_eq!((before, after), (None, Some(id)));
      }
    }

    mod fail {
      use pretty_assertions::assert_eq;

      use super::*;

      #[tokio::test]
      async fn it_stores_the_error() {
        let backend = backend().await;
        let id = backend.push("default", "send_email", Value::Null).await.unwrap();
        backend.claim(&["default"]).await.unwrap();
        backend.fail(id, "boom", "trace").await.unwrap();

        let job = stored(&backend, id).await;

        assert_eq!((job.status, job.error), (Status::Failed, Some("boom".to_owned())));
      }
    }

    mod migration {
      use pretty_assertions::assert_eq;

      use super::*;

      #[tokio::test]
      async fn it_upgrades_a_store_without_losing_jobs() {
        let db = Database::connect("sqlite::memory:").await.unwrap();
        Migrator::up(&db, Some(6)).await.unwrap();
        db.execute_unprepared(
          "INSERT INTO jerb_jobs (queue, name, args, status, attempt, claimed_at) VALUES ('default', 'send_email', '{}', 'running', 2, 5)",
        )
        .await
        .unwrap();

        Migrator::up(&db, None).await.unwrap();

        let job = Entity::find().one(&db).await.unwrap().unwrap();
        assert_eq!(
          (job.name, job.attempt, job.claimed_at, job.enqueued_at, job.finished_at),
          ("send_email".to_owned(), 2, Some(5), None, None)
        );
      }
    }

    mod stats {
      use pretty_assertions::assert_eq;

      use super::*;

      const HOUR: u64 = 3_600_000;

      /// Noon on a Unix day, in milliseconds.
      const NOON: i64 = 86_400_000 * 20_000 + 12 * HOUR as i64;

      fn at(ms: i64) -> SystemTime {
        UNIX_EPOCH + Duration::from_millis(u64::try_from(ms).unwrap())
      }

      async fn ended(backend: &Sql, status: Status, finished_at: i64) {
        let id = backend.push("default", "send_email", Value::Null).await.unwrap();
        Entity::update_many()
          .col_expr(Column::Status, Expr::value(status))
          .col_expr(Column::FinishedAt, Expr::value(finished_at))
          .filter(Column::Id.eq(id))
          .exec(&backend.db)
          .await
          .unwrap();
      }

      #[tokio::test]
      async fn it_counts_each_state_as_the_lists_do() {
        let backend = backend().await;
        let now = SystemTime::now();
        backend.push("default", "ready", Value::Null).await.unwrap();
        backend
          .push_at("default", "later", Value::Null, now + Duration::from_secs(600))
          .await
          .unwrap();
        let retrying = backend.push("default", "retry", Value::Null).await.unwrap();
        let dead = backend.push("default", "dead", Value::Null).await.unwrap();
        let done = backend.push("default", "done", Value::Null).await.unwrap();
        backend.push("default", "run", Value::Null).await.unwrap();
        for name in ["retry", "dead", "done", "run"] {
          assert!(backend.claim(&["default"]).await.unwrap().is_some(), "{name}");
        }
        backend
          .retry(retrying, "e", "b", now + Duration::from_secs(600))
          .await
          .unwrap();
        backend.fail(dead, "e", "b").await.unwrap();
        backend.finish(done).await.unwrap();

        let stats = backend.stats(now).await.unwrap();

        assert_eq!(stats.pending(), backend.size("default").await.unwrap());
        assert_eq!(stats.scheduled(), backend.scheduled().await.unwrap().len() as u64);
        assert_eq!(stats.retries(), backend.retries().await.unwrap().len() as u64);
        assert_eq!(stats.dead(), backend.dead().await.unwrap().len() as u64);
        assert_eq!((stats.finished(), stats.running()), (1, 1));
      }

      #[tokio::test]
      async fn it_returns_24_empty_hours_ending_with_the_current_one() {
        let backend = backend().await;

        let stats = backend.stats(at(NOON + 1)).await.unwrap();

        assert_eq!(stats.hours().len(), 24);
        assert!(
          stats
            .hours()
            .iter()
            .all(|hour| hour.dead() == 0 && hour.finished() == 0)
        );
        assert_eq!(stats.hours()[23].start(), at(NOON));
        assert_eq!(stats.hours()[0].start(), at(NOON - 23 * HOUR as i64));
      }

      #[tokio::test]
      async fn it_buckets_jobs_by_the_hour_they_ended() {
        let backend = backend().await;
        ended(&backend, Status::Finished, NOON + 5).await;
        ended(&backend, Status::Finished, NOON - 1).await;
        ended(&backend, Status::Failed, NOON - 1).await;
        ended(&backend, Status::Failed, NOON + 6).await;

        let hours = backend.stats(at(NOON + 10)).await.unwrap().hours().clone();

        assert_eq!((hours[23].finished(), hours[23].dead()), (1, 1));
        assert_eq!((hours[22].finished(), hours[22].dead()), (1, 1));
      }

      #[tokio::test]
      async fn it_keeps_the_first_and_last_instants_of_the_window() {
        let backend = backend().await;
        let first = NOON - 23 * HOUR as i64;
        ended(&backend, Status::Finished, first).await;
        ended(&backend, Status::Finished, first - 1).await;
        ended(&backend, Status::Finished, NOON + 10).await;
        ended(&backend, Status::Finished, NOON + 11).await;

        let stats = backend.stats(at(NOON + 10)).await.unwrap();

        assert_eq!(stats.hours()[0].finished(), 1);
        assert_eq!(stats.hours()[23].finished(), 1);
        assert_eq!(stats.finished(), 4);
      }

      #[tokio::test]
      async fn it_counts_a_job_as_dead_only_when_it_ends_dead() {
        let backend = backend().await;
        let id = backend.push("default", "send_email", Value::Null).await.unwrap();
        backend.claim(&["default"]).await.unwrap();
        backend.retry(id, "e", "b", SystemTime::now()).await.unwrap();
        let now = SystemTime::now();

        let stats = backend.stats(now).await.unwrap();

        assert!(
          stats
            .hours()
            .iter()
            .all(|hour| hour.dead() == 0 && hour.finished() == 0)
        );
      }

      #[tokio::test]
      async fn it_counts_a_job_that_dies_after_retries_once() {
        let backend = backend().await;
        let id = backend.push("default", "send_email", Value::Null).await.unwrap();
        backend.claim(&["default"]).await.unwrap();
        backend.retry(id, "e", "b", SystemTime::now()).await.unwrap();
        tokio::time::sleep(Duration::from_millis(5)).await;
        backend.claim(&["default"]).await.unwrap();
        backend.fail(id, "e", "b").await.unwrap();

        let stats = backend.stats(SystemTime::now()).await.unwrap();

        assert_eq!(stats.hours().iter().map(|hour| hour.dead()).sum::<u64>(), 1);
      }
    }

    mod run_now {
      use pretty_assertions::assert_eq;

      use super::*;

      #[tokio::test]
      async fn it_runs_a_dead_job_and_keeps_its_attempt_count() {
        let backend = backend().await;
        let id = backend.push("default", "send_email", Value::Null).await.unwrap();
        backend.claim(&["default"]).await.unwrap();
        backend
          .retry(id, "boom", "trace", SystemTime::now() - Duration::from_secs(1))
          .await
          .unwrap();
        backend.claim(&["default"]).await.unwrap();
        backend.fail(id, "boom", "trace").await.unwrap();

        backend.run_now(id).await.unwrap();

        assert_eq!(backend.dead().await.unwrap(), Vec::new());
        let job = backend.claim(&["default"]).await.unwrap().unwrap();
        assert_eq!(
          (job.id(), job.attempt(), job.error().clone(), job.backtrace().clone()),
          (id, 1, None, None)
        );
      }

      #[tokio::test]
      async fn it_runs_a_job_waiting_to_retry() {
        let backend = backend().await;
        let id = backend.push("default", "send_email", Value::Null).await.unwrap();
        backend.claim(&["default"]).await.unwrap();
        backend
          .retry(id, "boom", "trace", SystemTime::now() + Duration::from_secs(60))
          .await
          .unwrap();

        backend.run_now(id).await.unwrap();

        assert_eq!(backend.claim(&["default"]).await.unwrap().map(|job| job.id()), Some(id));
      }

      #[tokio::test]
      async fn it_errors_for_a_finished_job() {
        let backend = backend().await;
        let id = backend.push("default", "send_email", Value::Null).await.unwrap();
        backend.claim(&["default"]).await.unwrap();
        backend.finish(id).await.unwrap();

        assert!(matches!(backend.run_now(id).await, Err(Error::NotWaiting(found)) if found == id));
      }

      #[tokio::test]
      async fn it_errors_for_a_running_job() {
        let backend = backend().await;
        let id = backend.push("default", "send_email", Value::Null).await.unwrap();
        backend.claim(&["default"]).await.unwrap();

        assert!(matches!(backend.run_now(id).await, Err(Error::NotWaiting(found)) if found == id));
      }
    }

    mod finish {
      use pretty_assertions::assert_eq;

      use super::*;

      #[tokio::test]
      async fn it_stores_the_finished_status() {
        let backend = backend().await;
        let id = backend.push("default", "send_email", Value::Null).await.unwrap();
        backend.claim(&["default"]).await.unwrap();
        backend.finish(id).await.unwrap();

        let job = stored(&backend, id).await;

        assert_eq!((job.status, job.error), (Status::Finished, None));
      }

      #[tokio::test]
      async fn it_stamps_the_finish_time() {
        let backend = backend().await;
        let id = backend.push("default", "send_email", Value::Null).await.unwrap();
        backend.claim(&["default"]).await.unwrap();
        backend.finish(id).await.unwrap();

        let job = stored(&backend, id).await;

        assert!(job.finished_at >= job.claimed_at && job.finished_at.is_some());
      }
    }
  }
}
