use std::error::Error;

use async_trait::async_trait;
use serde_json::Value;

/// A client that `#[jerb::job]` enqueues a job through, as in `send_email::enqueue(&client, args)`.
#[async_trait]
pub trait Enqueue: Send + Sync {
  /// Stores a new job named `name` with `args` and returns its id.
  async fn enqueue(&self, name: &str, args: Value) -> Result<i64, Box<dyn Error + Send + Sync>>;
}

#[cfg(test)]
mod tests {
  use std::{
    collections::BTreeMap,
    sync::{Arc, Mutex},
  };

  use serde_json::json;

  use super::*;
  use crate::Json;

  #[derive(Default)]
  struct Client(Arc<Mutex<Vec<(String, Value)>>>);

  #[async_trait]
  impl Enqueue for Client {
    async fn enqueue(&self, name: &str, args: Value) -> Result<i64, Box<dyn Error + Send + Sync>> {
      let mut jobs = self.0.lock().unwrap();
      jobs.push((name.into(), args));
      Ok(jobs.len() as i64)
    }
  }

  #[crate::job]
  async fn send_digest(Json(_): Json<BTreeMap<(u8, u8), u8>>) -> Result<(), Box<dyn Error + Send + Sync>> {
    Ok(())
  }

  #[crate::job]
  async fn send_email(Json(_): Json<String>) -> Result<(), Box<dyn Error + Send + Sync>> {
    Ok(())
  }

  #[crate::job]
  async fn sweep() -> Result<(), Box<dyn Error + Send + Sync>> {
    Ok(())
  }

  mod enqueue {
    use pretty_assertions::assert_eq;

    use super::*;

    #[tokio::test]
    async fn it_enqueues_the_job_by_its_fn_name_with_its_args_as_json() {
      let client = Client::default();

      send_email::enqueue(&client, "ada@example.com".into()).await.unwrap();

      assert_eq!(
        *client.0.lock().unwrap(),
        vec![("send_email".to_string(), json!("ada@example.com"))]
      );
    }

    #[tokio::test]
    async fn it_enqueues_a_job_that_takes_no_args_with_null() {
      let client = Client::default();

      sweep::enqueue(&client, ()).await.unwrap();

      assert_eq!(*client.0.lock().unwrap(), vec![("sweep".to_string(), Value::Null)]);
    }

    #[tokio::test]
    async fn it_returns_an_error_when_the_args_do_not_encode() {
      let client = Client::default();

      let error = send_digest::enqueue(&client, BTreeMap::from([((1, 2), 3)]))
        .await
        .unwrap_err();

      assert!(error.is::<serde_json::Error>());
    }
  }
}
