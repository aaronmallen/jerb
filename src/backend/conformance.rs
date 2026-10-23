macro_rules! conformance {
  ($backend:expr) => {
    use std::{
      sync::Arc,
      time::{Duration, SystemTime, UNIX_EPOCH},
    };

    use serde_json::json;

    use $crate::backend::{Backend, Error};

    mod claim {
      use pretty_assertions::assert_eq;

      use super::*;

      #[tokio::test]
      async fn it_claims_a_pushed_job() {
        let backend = $backend;
        let id = backend.push("default", "send_email", json!({"to": "a"})).await.unwrap();

        let job = backend.claim(&["default"]).await.unwrap().unwrap();

        assert_eq!(
          (job.id(), job.queue().as_str(), job.name().as_str(), job.args()),
          (id, "default", "send_email", &json!({"to": "a"}))
        );
      }

      #[tokio::test]
      async fn it_claims_a_job_once() {
        let backend = $backend;
        backend.push("default", "send_email", json!(null)).await.unwrap();
        backend.claim(&["default"]).await.unwrap();

        assert_eq!(backend.claim(&["default"]).await.unwrap(), None);
      }

      #[tokio::test]
      async fn it_claims_the_oldest_job_first() {
        let backend = $backend;
        let first = backend.push("default", "send_email", json!(null)).await.unwrap();
        backend.push("default", "send_email", json!(null)).await.unwrap();

        let job = backend.claim(&["default"]).await.unwrap().unwrap();

        assert_eq!(job.id(), first);
      }

      #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
      async fn it_never_gives_two_workers_the_same_job() {
        let backend = Arc::new($backend);
        let mut pushed = Vec::new();
        for _ in 0..20 {
          pushed.push(backend.push("default", "send_email", json!(null)).await.unwrap());
        }

        let workers: Vec<_> = (0..40)
          .map(|_| {
            let backend = Arc::clone(&backend);
            tokio::spawn(async move { backend.claim(&["default"]).await.unwrap().map(|job| job.id()) })
          })
          .collect();
        let mut claimed = Vec::new();
        for worker in workers {
          claimed.extend(worker.await.unwrap());
        }
        claimed.sort_unstable();

        assert_eq!(claimed, pushed);
      }

      #[tokio::test]
      async fn it_returns_none_when_no_job_is_pending() {
        let backend = $backend;

        assert_eq!(backend.claim(&["default"]).await.unwrap(), None);
      }

      #[tokio::test]
      async fn it_skips_jobs_on_other_queues() {
        let backend = $backend;
        backend.push("mailers", "send_email", json!(null)).await.unwrap();

        assert_eq!(backend.claim(&["default"]).await.unwrap(), None);
      }
    }

    mod dead {
      use pretty_assertions::assert_eq;

      use super::*;

      #[tokio::test]
      async fn it_keeps_the_error() {
        let backend = $backend;
        let id = backend.push("default", "send_email", json!(null)).await.unwrap();
        backend.claim(&["default"]).await.unwrap();
        backend.fail(id, "boom", "trace").await.unwrap();

        let dead = backend.dead().await.unwrap();

        assert_eq!(dead[0].error().as_deref(), Some("boom"));
      }

      #[tokio::test]
      async fn it_keeps_the_backtrace() {
        let backend = $backend;
        let id = backend.push("default", "send_email", json!(null)).await.unwrap();
        backend.claim(&["default"]).await.unwrap();
        backend.fail(id, "boom", "trace").await.unwrap();

        let dead = backend.dead().await.unwrap();

        assert_eq!(dead[0].backtrace().as_deref(), Some("trace"));
      }

      #[tokio::test]
      async fn it_leaves_out_a_job_that_passes_on_a_retry() {
        let backend = $backend;
        let id = backend.push("default", "send_email", json!(null)).await.unwrap();
        backend.claim(&["default"]).await.unwrap();
        backend.retry(id, "boom", "trace", SystemTime::now() - Duration::from_secs(1)).await.unwrap();
        backend.claim(&["default"]).await.unwrap();
        backend.finish(id).await.unwrap();

        assert_eq!(backend.dead().await.unwrap(), Vec::new());
      }

      #[tokio::test]
      async fn it_leaves_out_a_job_waiting_to_retry() {
        let backend = $backend;
        let id = backend.push("default", "send_email", json!(null)).await.unwrap();
        backend.claim(&["default"]).await.unwrap();
        backend.retry(id, "boom", "trace", SystemTime::now() + Duration::from_secs(60)).await.unwrap();

        assert_eq!(backend.dead().await.unwrap(), Vec::new());
      }

      #[tokio::test]
      async fn it_lists_a_job_that_failed_after_its_retries() {
        let backend = $backend;
        let id = backend.push("mailers", "send_email", json!({"to": "a"})).await.unwrap();
        backend.claim(&["mailers"]).await.unwrap();
        backend.retry(id, "boom", "trace", SystemTime::now() - Duration::from_secs(1)).await.unwrap();
        backend.claim(&["mailers"]).await.unwrap();
        backend.fail(id, "boom", "trace").await.unwrap();

        let dead = backend.dead().await.unwrap();

        assert_eq!(
          dead
            .iter()
            .map(|job| (job.id(), job.attempt(), job.queue().as_str(), job.name().as_str(), job.args()))
            .collect::<Vec<_>>(),
          vec![(id, 1, "mailers", "send_email", &json!({"to": "a"}))]
        );
      }

      #[tokio::test]
      async fn it_lists_failed_jobs_oldest_first() {
        let backend = $backend;
        let first = backend.push("default", "send_email", json!("a")).await.unwrap();
        let second = backend.push("default", "send_email", json!("b")).await.unwrap();
        backend.claim(&["default"]).await.unwrap();
        backend.claim(&["default"]).await.unwrap();
        backend.fail(second, "boom", "trace").await.unwrap();
        backend.fail(first, "boom", "trace").await.unwrap();

        let dead: Vec<_> = backend.dead().await.unwrap().iter().map(|job| job.id()).collect();

        assert_eq!(dead, vec![first, second]);
      }
    }

    mod fail {
      use pretty_assertions::assert_eq;

      use super::*;

      #[tokio::test]
      async fn it_errors_when_the_job_is_not_running() {
        let backend = $backend;
        let id = backend.push("default", "send_email", json!(null)).await.unwrap();

        assert!(matches!(backend.fail(id, "boom", "trace").await, Err(Error::NotRunning(n)) if n == id));
      }

      #[tokio::test]
      async fn it_leaves_the_job_unclaimable() {
        let backend = $backend;
        let id = backend.push("default", "send_email", json!(null)).await.unwrap();
        backend.claim(&["default"]).await.unwrap();
        backend.fail(id, "boom", "trace").await.unwrap();

        assert_eq!(backend.claim(&["default"]).await.unwrap(), None);
      }

      #[tokio::test]
      async fn it_stops_the_job_running() {
        let backend = $backend;
        let id = backend.push("default", "send_email", json!(null)).await.unwrap();
        backend.claim(&["default"]).await.unwrap();
        backend.fail(id, "boom", "trace").await.unwrap();

        assert!(matches!(backend.finish(id).await, Err(Error::NotRunning(n)) if n == id));
      }
    }

    mod finish {
      use pretty_assertions::assert_eq;

      use super::*;

      #[tokio::test]
      async fn it_errors_when_the_job_is_not_running() {
        let backend = $backend;
        let id = backend.push("default", "send_email", json!(null)).await.unwrap();

        assert!(matches!(backend.finish(id).await, Err(Error::NotRunning(n)) if n == id));
      }

      #[tokio::test]
      async fn it_leaves_the_job_unclaimable() {
        let backend = $backend;
        let id = backend.push("default", "send_email", json!(null)).await.unwrap();
        backend.claim(&["default"]).await.unwrap();
        backend.finish(id).await.unwrap();

        assert_eq!(backend.claim(&["default"]).await.unwrap(), None);
      }

      #[tokio::test]
      async fn it_stops_the_job_running() {
        let backend = $backend;
        let id = backend.push("default", "send_email", json!(null)).await.unwrap();
        backend.claim(&["default"]).await.unwrap();
        backend.finish(id).await.unwrap();

        assert!(matches!(backend.fail(id, "boom", "trace").await, Err(Error::NotRunning(n)) if n == id));
      }
    }

    mod fire {
      use pretty_assertions::assert_eq;

      use super::*;

      #[tokio::test]
      async fn it_enqueues_a_later_fire() {
        let backend = $backend;
        backend.fire("digest", 10, "default", "send_email", json!(null)).await.unwrap();

        assert!(backend.fire("digest", 11, "default", "send_email", json!(null)).await.unwrap().is_some());
      }

      #[tokio::test]
      async fn it_enqueues_a_fire_once() {
        let backend = $backend;
        backend.fire("digest", 10, "default", "send_email", json!(null)).await.unwrap();

        assert_eq!(
          backend.fire("digest", 10, "default", "send_email", json!(null)).await.unwrap(),
          None
        );
      }

      #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
      async fn it_enqueues_a_fire_once_however_many_processes_call_it() {
        let backend = Arc::new($backend);

        let callers: Vec<_> = (0..20)
          .map(|_| {
            let backend = Arc::clone(&backend);
            tokio::spawn(async move { backend.fire("digest", 10, "default", "send_email", json!(null)).await.unwrap() })
          })
          .collect();
        let mut fired = Vec::new();
        for caller in callers {
          fired.extend(caller.await.unwrap());
        }

        assert_eq!(fired.len(), 1);
      }

      #[tokio::test]
      async fn it_enqueues_each_schedule_on_its_own() {
        let backend = $backend;
        backend.fire("digest", 10, "default", "send_email", json!(null)).await.unwrap();

        assert!(backend.fire("sweep", 10, "default", "sweep", json!(null)).await.unwrap().is_some());
      }

      #[tokio::test]
      async fn it_enqueues_the_job() {
        let backend = $backend;
        let id = backend.fire("digest", 10, "mailers", "send_email", json!({"to": "a"})).await.unwrap();

        let job = backend.claim(&["mailers"]).await.unwrap().unwrap();

        assert_eq!(
          (Some(job.id()), job.queue().as_str(), job.name().as_str(), job.args()),
          (id, "mailers", "send_email", &json!({"to": "a"}))
        );
      }

      #[tokio::test]
      async fn it_skips_a_fire_older_than_the_last() {
        let backend = $backend;
        backend.fire("digest", 10, "default", "send_email", json!(null)).await.unwrap();
        backend.claim(&["default"]).await.unwrap();

        backend.fire("digest", 9, "default", "send_email", json!(null)).await.unwrap();

        assert_eq!(backend.claim(&["default"]).await.unwrap(), None);
      }
    }

    mod push {
      use pretty_assertions::assert_ne;

      use super::*;

      #[tokio::test]
      async fn it_gives_each_job_its_own_id() {
        let backend = $backend;
        let first = backend.push("default", "send_email", json!(null)).await.unwrap();
        let second = backend.push("default", "send_email", json!(null)).await.unwrap();

        assert_ne!(first, second);
      }
    }

    mod push_at {
      use pretty_assertions::assert_eq;

      use super::*;

      #[tokio::test]
      async fn it_claims_a_job_pushed_for_now_while_the_delayed_one_waits() {
        let backend = $backend;
        backend
          .push_at("default", "send_email", json!("later"), SystemTime::now() + Duration::from_secs(60))
          .await
          .unwrap();
        let now = backend.push("default", "send_email", json!("now")).await.unwrap();

        let job = backend.claim(&["default"]).await.unwrap().unwrap();

        assert_eq!(
          (job.id(), backend.claim(&["default"]).await.unwrap()),
          (now, None)
        );
      }

      #[tokio::test]
      async fn it_claims_a_job_whose_time_has_passed() {
        let backend = $backend;
        let id = backend
          .push_at("default", "send_email", json!(null), SystemTime::now() - Duration::from_secs(1))
          .await
          .unwrap();

        let job = backend.claim(&["default"]).await.unwrap().unwrap();

        assert_eq!(job.id(), id);
      }

      #[tokio::test]
      async fn it_claims_the_job_once_its_time_comes() {
        let backend = $backend;
        let at = SystemTime::now() + Duration::from_millis(100);
        let id = backend.push_at("default", "send_email", json!(null), at).await.unwrap();
        tokio::time::sleep(at.duration_since(SystemTime::now()).unwrap_or_default() + Duration::from_millis(5)).await;

        let job = backend.claim(&["default"]).await.unwrap().unwrap();

        assert_eq!(job.id(), id);
      }

      #[tokio::test]
      async fn it_does_not_claim_the_job_before_its_time() {
        let backend = $backend;
        backend
          .push_at("default", "send_email", json!(null), SystemTime::now() + Duration::from_secs(60))
          .await
          .unwrap();

        assert_eq!(backend.claim(&["default"]).await.unwrap(), None);
      }
    }

    mod push_unique {
      use pretty_assertions::{assert_eq, assert_ne};

      use super::*;

      #[tokio::test]
      async fn it_drops_a_push_while_the_same_job_is_pending() {
        let backend = $backend;
        let first = backend.push_unique("default", "send_email", json!({"to": "a"})).await.unwrap();
        let second = backend.push_unique("default", "send_email", json!({"to": "a"})).await.unwrap();

        assert_eq!(second, first);
        assert_eq!(backend.claim(&["default"]).await.unwrap().map(|job| job.id()), Some(first));
        assert_eq!(backend.claim(&["default"]).await.unwrap(), None);
      }

      #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
      async fn it_drops_pushes_that_race_each_other() {
        let backend = Arc::new($backend);

        let pushes: Vec<_> = (0..20)
          .map(|_| {
            let backend = Arc::clone(&backend);
            tokio::spawn(async move { backend.push_unique("default", "send_email", json!(null)).await.unwrap() })
          })
          .collect();
        let mut ids = Vec::new();
        for push in pushes {
          ids.push(push.await.unwrap());
        }
        ids.dedup();

        assert_eq!(ids.len(), 1);
        assert!(backend.claim(&["default"]).await.unwrap().is_some());
        assert_eq!(backend.claim(&["default"]).await.unwrap(), None);
      }

      #[tokio::test]
      async fn it_ignores_key_order_in_the_args() {
        let backend = $backend;
        let first = backend.push_unique("default", "send_email", json!({"a": 1, "b": 2})).await.unwrap();

        let second = backend.push_unique("default", "send_email", json!({"b": 2, "a": 1})).await.unwrap();

        assert_eq!(second, first);
      }

      #[tokio::test]
      async fn it_keeps_a_job_with_another_name() {
        let backend = $backend;
        let first = backend.push_unique("default", "send_email", json!(null)).await.unwrap();

        let second = backend.push_unique("default", "send_sms", json!(null)).await.unwrap();

        assert_ne!(second, first);
      }

      #[tokio::test]
      async fn it_keeps_a_job_with_other_args() {
        let backend = $backend;
        let first = backend.push_unique("default", "send_email", json!("a")).await.unwrap();

        let second = backend.push_unique("default", "send_email", json!("b")).await.unwrap();

        assert_ne!(second, first);
      }

      #[tokio::test]
      async fn it_leaves_plain_pushes_alone() {
        let backend = $backend;
        let first = backend.push("default", "send_email", json!(null)).await.unwrap();

        let second = backend.push_unique("default", "send_email", json!(null)).await.unwrap();

        assert_ne!(second, first);
      }

      #[tokio::test]
      async fn it_pushes_again_once_the_job_finishes() {
        let backend = $backend;
        let first = backend.push_unique("default", "send_email", json!(null)).await.unwrap();
        backend.claim(&["default"]).await.unwrap();
        backend.finish(first).await.unwrap();

        let second = backend.push_unique("default", "send_email", json!(null)).await.unwrap();

        assert_eq!(backend.claim(&["default"]).await.unwrap().map(|job| job.id()), Some(second));
      }

      #[tokio::test]
      async fn it_pushes_again_once_the_job_is_claimed() {
        let backend = $backend;
        let first = backend.push_unique("default", "send_email", json!(null)).await.unwrap();
        backend.claim(&["default"]).await.unwrap();

        let second = backend.push_unique("default", "send_email", json!(null)).await.unwrap();

        assert_ne!(second, first);
      }
    }

    mod times {
      use pretty_assertions::assert_eq;

      use super::*;

      /// Whether `at` falls between `before` and now, to the millisecond the stores keep.
      fn is_fresh(at: Option<SystemTime>, before: SystemTime) -> bool {
        at.is_some_and(|at| at + Duration::from_millis(1) >= before && at <= SystemTime::now())
      }

      #[tokio::test]
      async fn it_stamps_the_enqueue_time_at_push_and_the_start_time_at_claim() {
        let backend = $backend;
        let before = SystemTime::now();
        backend.push("default", "send_email", json!(null)).await.unwrap();

        let job = backend.claim(&["default"]).await.unwrap().unwrap();

        assert_eq!(
          (
            is_fresh(job.enqueued_at(), before),
            is_fresh(job.started_at(), before),
            job.enqueued_at() <= job.started_at(),
            job.finished_at()
          ),
          (true, true, true, None)
        );
      }

      #[tokio::test]
      async fn it_stamps_the_enqueue_time_of_a_scheduled_job() {
        let backend = $backend;
        let before = SystemTime::now();
        backend
          .push_at("default", "send_email", json!(null), before + Duration::from_secs(60))
          .await
          .unwrap();

        let job = backend.scheduled().await.unwrap().remove(0);

        assert_eq!(
          (is_fresh(job.enqueued_at(), before), job.started_at(), job.finished_at()),
          (true, None, None)
        );
      }

      #[tokio::test]
      async fn it_stamps_the_finish_time_at_fail() {
        let backend = $backend;
        let id = backend.push("default", "send_email", json!(null)).await.unwrap();
        let started = backend.claim(&["default"]).await.unwrap().unwrap().started_at();
        let before = SystemTime::now();
        backend.fail(id, "boom", "trace").await.unwrap();

        let job = backend.dead().await.unwrap().remove(0);

        assert_eq!(
          (is_fresh(job.finished_at(), before), job.started_at() == started),
          (true, true)
        );
      }

      #[tokio::test]
      async fn it_keeps_the_backtrace_of_the_last_failed_run() {
        let backend = $backend;
        let id = backend.push("default", "send_email", json!(null)).await.unwrap();
        backend.claim(&["default"]).await.unwrap();
        backend.retry(id, "boom", "trace", SystemTime::now() + Duration::from_secs(60)).await.unwrap();

        let job = backend.retries().await.unwrap().remove(0);

        assert_eq!(job.backtrace().as_deref(), Some("trace"));
      }

      #[tokio::test]
      async fn it_stamps_the_finish_time_at_retry() {
        let backend = $backend;
        let id = backend.push("default", "send_email", json!(null)).await.unwrap();
        backend.claim(&["default"]).await.unwrap();
        let before = SystemTime::now();
        backend
          .retry(id, "boom", "trace", before + Duration::from_secs(60))
          .await
          .unwrap();

        let job = backend.retries().await.unwrap().remove(0);

        assert_eq!(is_fresh(job.finished_at(), before), true);
      }

      #[tokio::test]
      async fn it_clears_the_finish_time_at_the_next_claim() {
        let backend = $backend;
        let id = backend.push("default", "send_email", json!(null)).await.unwrap();
        backend.claim(&["default"]).await.unwrap();
        backend
          .retry(id, "boom", "trace", SystemTime::now() - Duration::from_secs(1))
          .await
          .unwrap();

        let job = backend.claim(&["default"]).await.unwrap().unwrap();

        assert_eq!(job.finished_at(), None);
      }
    }

    mod retries {
      use pretty_assertions::assert_eq;

      use super::*;

      #[tokio::test]
      async fn it_leaves_out_a_job_whose_time_has_come() {
        let backend = $backend;
        let id = backend.push("default", "send_email", json!(null)).await.unwrap();
        backend.claim(&["default"]).await.unwrap();
        backend.retry(id, "boom", "trace", SystemTime::now() - Duration::from_secs(1)).await.unwrap();

        assert_eq!(backend.retries().await.unwrap(), Vec::new());
      }

      #[tokio::test]
      async fn it_leaves_out_a_scheduled_job() {
        let backend = $backend;
        backend
          .push_at("default", "send_email", json!(null), SystemTime::now() + Duration::from_secs(60))
          .await
          .unwrap();

        assert_eq!(backend.retries().await.unwrap(), Vec::new());
      }

      #[tokio::test]
      async fn it_lists_a_job_waiting_to_retry() {
        let backend = $backend;
        let at = UNIX_EPOCH + Duration::from_secs(4_000_000_000);
        let id = backend.push("mailers", "send_email", json!({"to": "a"})).await.unwrap();
        backend.claim(&["mailers"]).await.unwrap();
        backend.retry(id, "boom", "trace", at).await.unwrap();

        let retries = backend.retries().await.unwrap();

        assert_eq!(
          retries
            .iter()
            .map(|job| (
              job.id(),
              job.attempt(),
              job.queue().as_str(),
              job.name().as_str(),
              job.args(),
              job.error().as_deref(),
              job.run_at()
            ))
            .collect::<Vec<_>>(),
          vec![(id, 1, "mailers", "send_email", &json!({"to": "a"}), Some("boom"), Some(at))]
        );
      }

      #[tokio::test]
      async fn it_lists_the_soonest_first() {
        let backend = $backend;
        let later = backend.push("default", "send_email", json!("a")).await.unwrap();
        let sooner = backend.push("default", "send_email", json!("b")).await.unwrap();
        backend.claim(&["default"]).await.unwrap();
        backend.claim(&["default"]).await.unwrap();
        backend.retry(later, "boom", "trace", SystemTime::now() + Duration::from_secs(120)).await.unwrap();
        backend.retry(sooner, "boom", "trace", SystemTime::now() + Duration::from_secs(60)).await.unwrap();

        let retries: Vec<_> = backend.retries().await.unwrap().iter().map(|job| job.id()).collect();

        assert_eq!(retries, vec![sooner, later]);
      }
    }

    mod retry {
      use pretty_assertions::assert_eq;

      use super::*;

      #[tokio::test]
      async fn it_claims_the_job_once_its_time_comes() {
        let backend = $backend;
        let id = backend.push("default", "send_email", json!(null)).await.unwrap();
        backend.claim(&["default"]).await.unwrap();
        backend.retry(id, "boom", "trace", SystemTime::now() - Duration::from_secs(1)).await.unwrap();

        let job = backend.claim(&["default"]).await.unwrap().unwrap();

        assert_eq!(job.id(), id);
      }

      #[tokio::test]
      async fn it_counts_each_attempt() {
        let backend = $backend;
        let id = backend.push("default", "send_email", json!(null)).await.unwrap();
        let mut attempts = vec![backend.claim(&["default"]).await.unwrap().unwrap().attempt()];
        for _ in 0..2 {
          backend.retry(id, "boom", "trace", SystemTime::now() - Duration::from_secs(1)).await.unwrap();
          attempts.push(backend.claim(&["default"]).await.unwrap().unwrap().attempt());
        }

        assert_eq!(attempts, vec![0, 1, 2]);
      }

      #[tokio::test]
      async fn it_does_not_claim_the_job_before_its_time() {
        let backend = $backend;
        let id = backend.push("default", "send_email", json!(null)).await.unwrap();
        backend.claim(&["default"]).await.unwrap();
        backend.retry(id, "boom", "trace", SystemTime::now() + Duration::from_secs(60)).await.unwrap();

        assert_eq!(backend.claim(&["default"]).await.unwrap(), None);
      }

      #[tokio::test]
      async fn it_errors_when_the_job_is_not_running() {
        let backend = $backend;
        let id = backend.push("default", "send_email", json!(null)).await.unwrap();

        assert!(matches!(
          backend.retry(id, "boom", "trace", SystemTime::now()).await,
          Err(Error::NotRunning(n)) if n == id
        ));
      }
    }

    mod reclaim {
      use pretty_assertions::assert_eq;

      use super::*;

      #[tokio::test]
      async fn it_leaves_a_job_within_its_timeout() {
        let backend = $backend;
        backend.push("default", "send_email", json!(null)).await.unwrap();
        backend.claim(&["default"]).await.unwrap();

        backend.reclaim(&|_| Duration::from_secs(60)).await.unwrap();

        assert_eq!(backend.claim(&["default"]).await.unwrap(), None);
      }

      #[tokio::test]
      async fn it_leaves_a_settled_job() {
        let backend = $backend;
        let id = backend.push("default", "send_email", json!(null)).await.unwrap();
        backend.claim(&["default"]).await.unwrap();
        backend.finish(id).await.unwrap();

        backend.reclaim(&|_| Duration::ZERO).await.unwrap();

        assert_eq!(backend.claim(&["default"]).await.unwrap(), None);
      }

      #[tokio::test]
      async fn it_returns_a_job_held_past_its_timeout() {
        let backend = $backend;
        let id = backend.push("default", "send_email", json!({"to": "a"})).await.unwrap();
        backend.claim(&["default"]).await.unwrap();

        backend.reclaim(&|_| Duration::ZERO).await.unwrap();
        let job = backend.claim(&["default"]).await.unwrap().unwrap();

        assert_eq!((job.id(), job.args()), (id, &json!({"to": "a"})));
      }

      #[tokio::test]
      async fn it_takes_each_job_timeout_from_its_name() {
        let backend = $backend;
        backend.push("default", "send_email", json!(null)).await.unwrap();
        let sms = backend.push("default", "send_sms", json!(null)).await.unwrap();
        backend.claim(&["default"]).await.unwrap();
        backend.claim(&["default"]).await.unwrap();

        backend
          .reclaim(&|name| if name == "send_sms" { Duration::ZERO } else { Duration::from_secs(60) })
          .await
          .unwrap();

        assert_eq!(backend.claim(&["default"]).await.unwrap().map(|job| job.id()), Some(sms));
        assert_eq!(backend.claim(&["default"]).await.unwrap(), None);
      }
    }

    mod scheduled {
      use pretty_assertions::assert_eq;

      use super::*;

      #[tokio::test]
      async fn it_leaves_out_a_job_waiting_to_retry() {
        let backend = $backend;
        let id = backend.push("default", "send_email", json!(null)).await.unwrap();
        backend.claim(&["default"]).await.unwrap();
        backend.retry(id, "boom", "trace", SystemTime::now() + Duration::from_secs(60)).await.unwrap();

        assert_eq!(backend.scheduled().await.unwrap(), Vec::new());
      }

      #[tokio::test]
      async fn it_leaves_out_a_job_whose_time_has_come() {
        let backend = $backend;
        backend.push("default", "send_email", json!(null)).await.unwrap();
        backend
          .push_at("default", "send_email", json!(null), SystemTime::now() - Duration::from_secs(1))
          .await
          .unwrap();

        assert_eq!(backend.scheduled().await.unwrap(), Vec::new());
      }

      #[tokio::test]
      async fn it_lists_a_job_pushed_for_later() {
        let backend = $backend;
        let at = UNIX_EPOCH + Duration::from_secs(4_000_000_000);
        let id = backend.push_at("mailers", "send_email", json!({"to": "a"}), at).await.unwrap();

        let scheduled = backend.scheduled().await.unwrap();

        assert_eq!(
          scheduled
            .iter()
            .map(|job| (job.id(), job.queue().as_str(), job.name().as_str(), job.args(), job.run_at()))
            .collect::<Vec<_>>(),
          vec![(id, "mailers", "send_email", &json!({"to": "a"}), Some(at))]
        );
      }

      #[tokio::test]
      async fn it_lists_the_soonest_first() {
        let backend = $backend;
        let later = backend
          .push_at("default", "send_email", json!(null), SystemTime::now() + Duration::from_secs(120))
          .await
          .unwrap();
        let sooner = backend
          .push_at("default", "send_email", json!(null), SystemTime::now() + Duration::from_secs(60))
          .await
          .unwrap();

        let scheduled: Vec<_> = backend.scheduled().await.unwrap().iter().map(|job| job.id()).collect();

        assert_eq!(scheduled, vec![sooner, later]);
      }
    }

    mod size {
      use pretty_assertions::assert_eq;

      use super::*;

      #[tokio::test]
      async fn it_counts_a_job_whose_time_has_passed() {
        let backend = $backend;
        backend
          .push_at("default", "send_email", json!(null), SystemTime::now() - Duration::from_secs(1))
          .await
          .unwrap();

        assert_eq!(backend.size("default").await.unwrap(), 1);
      }

      #[tokio::test]
      async fn it_counts_the_pending_jobs_on_the_queue() {
        let backend = $backend;
        backend.push("default", "send_email", json!(null)).await.unwrap();
        backend.push("default", "send_email", json!(null)).await.unwrap();
        backend.push("mailers", "send_email", json!(null)).await.unwrap();

        assert_eq!(backend.size("default").await.unwrap(), 2);
      }

      #[tokio::test]
      async fn it_leaves_out_a_claimed_job() {
        let backend = $backend;
        backend.push("default", "send_email", json!(null)).await.unwrap();
        backend.claim(&["default"]).await.unwrap();

        assert_eq!(backend.size("default").await.unwrap(), 0);
      }

      #[tokio::test]
      async fn it_leaves_out_a_job_whose_time_has_not_come() {
        let backend = $backend;
        backend
          .push_at("default", "send_email", json!(null), SystemTime::now() + Duration::from_secs(60))
          .await
          .unwrap();

        assert_eq!(backend.size("default").await.unwrap(), 0);
      }
    }
  };
}

pub(crate) use conformance;
