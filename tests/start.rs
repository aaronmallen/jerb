use std::{
  error::Error,
  sync::{Arc, Mutex},
  time::Duration,
};

use jerb::{Json, Router, Settings, State};

type Seen = Arc<Mutex<Vec<String>>>;

#[jerb::job]
async fn greet(State(seen): State<Seen>, Json(name): Json<String>) -> Result<(), Box<dyn Error + Send + Sync>> {
  seen.lock().unwrap().push(name);
  Ok(())
}

#[jerb::job]
async fn slow(State(seen): State<Seen>) -> Result<(), Box<dyn Error + Send + Sync>> {
  seen.lock().unwrap().push("start".into());
  tokio::time::sleep(Duration::from_millis(200)).await;
  seen.lock().unwrap().push("end".into());
  Ok(())
}

fn settings(extra: &str) -> Settings {
  toml::from_str(&format!("backend = \"sqlite\"\nurl = \"sqlite::memory:\"\n{extra}")).unwrap()
}

mod start {
  use pretty_assertions::assert_eq;

  use super::*;

  #[tokio::test]
  async fn it_errors_when_a_job_takes_another_state_type() {
    let error = jerb::start(settings(""), Router::new().with_state(7_u8))
      .await
      .err()
      .unwrap();

    assert!(matches!(error, jerb::Error::State("greet")));
  }

  #[tokio::test]
  async fn it_errors_when_a_schedule_names_a_job_no_fn_defines() {
    let settings = settings("[schedules.nightly]\ncron = \"0 0 * * *\"\njob = \"missing\"");

    let error = jerb::start(settings, Router::new().with_state(Seen::default()))
      .await
      .err()
      .unwrap();

    assert!(matches!(error, jerb::Error::UnknownJob(name) if name == "missing"));
  }

  #[cfg(not(feature = "redis"))]
  #[tokio::test]
  async fn it_errors_when_the_backend_is_not_built_in() {
    let settings: Settings = toml::from_str("backend = \"redis\"\nurl = \"redis://localhost\"").unwrap();

    let error = jerb::start(settings, Router::new().with_state(Seen::default()))
      .await
      .err()
      .unwrap();

    assert!(matches!(error, jerb::Error::Unsupported(jerb::BackendType::Redis)));
  }

  #[tokio::test]
  async fn it_errors_when_the_settings_name_a_job_no_fn_defines() {
    let error = jerb::start(settings("[jobs.missing]"), Router::new().with_state(Seen::default()))
      .await
      .err()
      .unwrap();

    assert!(matches!(error, jerb::Error::UnknownJob(name) if name == "missing"));
  }

  #[tokio::test]
  async fn it_runs_an_enqueued_job_with_its_state_and_args() {
    let seen = Seen::default();
    let client = jerb::start(settings(""), Router::new().with_state(seen.clone()))
      .await
      .unwrap();

    greet::enqueue(&client, "ada".into()).await.unwrap();
    tokio::time::timeout(Duration::from_secs(10), async {
      while seen.lock().unwrap().is_empty() {
        tokio::time::sleep(Duration::from_millis(10)).await;
      }
    })
    .await
    .unwrap();

    assert_eq!(*seen.lock().unwrap(), vec!["ada".to_string()]);
  }

  #[tokio::test]
  async fn it_never_runs_more_jobs_on_a_queue_than_its_workers() {
    let seen = Seen::default();
    let settings = settings("[jobs.slow]\nqueue = \"slow\"\n[workers]\nslow = 2");
    let client = jerb::start(settings, Router::new().with_state(seen.clone()))
      .await
      .unwrap();

    for _ in 0..6 {
      slow::enqueue(&client, ()).await.unwrap();
    }
    let slow = || {
      client
        .workers()
        .into_iter()
        .find(|queue| queue.queue() == "slow")
        .unwrap()
    };
    tokio::time::timeout(Duration::from_secs(10), async {
      while seen.lock().unwrap().iter().filter(|event| *event == "end").count() < 6 || *slow().busy() > 0 {
        tokio::time::sleep(Duration::from_millis(10)).await;
      }
    })
    .await
    .unwrap();

    let (mut running, mut most) = (0, 0);
    for event in seen.lock().unwrap().iter() {
      running += if event == "start" { 1 } else { -1 };
      most = most.max(running);
    }
    assert_eq!((most, *slow().configured()), (2, 2));
  }

  #[cfg(feature = "redis")]
  #[tokio::test]
  async fn it_runs_an_enqueued_job_on_redis() {
    let Ok(url) = std::env::var("JERB_TEST_REDIS_URL") else {
      return;
    };
    let seen = Seen::default();
    let settings = toml::from_str(&format!("backend = \"redis\"\nurl = \"{url}\"")).unwrap();
    let client = jerb::start(settings, Router::new().with_state(seen.clone()))
      .await
      .unwrap();
    let name = format!("{:?}", std::time::SystemTime::now());

    greet::enqueue(&client, name.clone()).await.unwrap();
    tokio::time::timeout(Duration::from_secs(10), async {
      while !seen.lock().unwrap().contains(&name) {
        tokio::time::sleep(Duration::from_millis(10)).await;
      }
    })
    .await
    .unwrap();
  }
}
