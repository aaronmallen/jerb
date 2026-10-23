use std::sync::Arc;

use axum::{Router, extract::State, http::StatusCode, response::Html, routing::get};
use chrono::{DateTime, SecondsFormat, Utc};

use crate::{
  Settings,
  backend::{Backend, Error, Job},
};

#[derive(Clone)]
struct Dashboard {
  backend: Arc<dyn Backend>,
  settings: Arc<Settings>,
}

pub(crate) fn router(backend: Arc<dyn Backend>, settings: Settings) -> Router {
  Router::new().route("/", get(index)).with_state(Dashboard {
    backend,
    settings: Arc::new(settings),
  })
}

fn escape(text: &str) -> String {
  text.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;")
}

async fn index(State(dashboard): State<Dashboard>) -> Result<Html<String>, (StatusCode, String)> {
  page(&*dashboard.backend, &dashboard.settings)
    .await
    .map(Html)
    .map_err(|error| (StatusCode::INTERNAL_SERVER_ERROR, error.to_string()))
}

fn jobs(title: &str, jobs: &[Job]) -> String {
  let mut html = format!("<h2>{title}</h2><table><tr><th>Id<th>Queue<th>Job<th>Args<th>Attempt<th>Error<th>Run at");
  for job in jobs {
    html += &format!(
      "<tr><td>{}<td>{}<td>{}<td>{}<td>{}<td>{}<td>{}",
      job.id(),
      escape(job.queue()),
      escape(job.name()),
      escape(&job.args().to_string()),
      job.attempt(),
      escape(job.error().as_deref().unwrap_or_default()),
      job
        .run_at()
        .map(|at| DateTime::<Utc>::from(at).to_rfc3339_opts(SecondsFormat::Secs, true))
        .unwrap_or_default(),
    );
  }
  html + "</table>"
}

async fn page(backend: &dyn Backend, settings: &Settings) -> Result<String, Error> {
  let mut html = String::from(concat!(
    "<!doctype html><meta charset=utf-8><title>Jerb</title><h1>Jerb</h1>",
    "<h2>Queues</h2><table><tr><th>Queue<th>Weight<th>Size",
  ));
  let mut queues: Vec<_> = settings.queues().iter().collect();
  queues.sort();
  for (name, weight) in queues {
    html += &format!("<tr><td>{}<td>{weight}<td>{}", escape(name), backend.size(name).await?);
  }
  html += "</table><h2>Schedules</h2><table><tr><th>Schedule<th>Cron<th>Job<th>Args";
  let mut schedules: Vec<_> = settings.schedules().iter().collect();
  schedules.sort_by_key(|&(name, _)| name);
  for (name, schedule) in schedules {
    html += &format!(
      "<tr><td>{}<td>{}<td>{}<td>{}",
      escape(name),
      escape(schedule.cron()),
      escape(schedule.job()),
      escape(&schedule.args().to_string()),
    );
  }
  html += "</table>";
  html += &jobs("Scheduled", &backend.scheduled().await?);
  html += &jobs("Retries", &backend.retries().await?);
  html += &jobs("Dead", &backend.dead().await?);
  Ok(html)
}

#[cfg(all(test, feature = "sqlite"))]
mod tests {
  use std::time::{Duration, UNIX_EPOCH};

  use axum::{
    body::{Body, to_bytes},
    http::Request,
  };
  use serde_json::json;
  use tower::ServiceExt;

  use super::*;
  use crate::{Client, backend::Sql};

  async fn section(title: &str) -> String {
    let backend = Arc::new(Sql::connect("sqlite::memory:").await.unwrap());
    let retry = backend.push("default", "resize_image", json!(null)).await.unwrap();
    backend.claim(&["default"]).await.unwrap();
    backend
      .retry(retry, "boom", "trace", UNIX_EPOCH + Duration::from_secs(4_000_000_000))
      .await
      .unwrap();
    let dead = backend.push("default", "charge_card", json!(1)).await.unwrap();
    backend.claim(&["default"]).await.unwrap();
    backend.fail(dead, "<card declined>", "trace").await.unwrap();
    backend.push("default", "send_email", json!("ada")).await.unwrap();
    backend
      .push_at(
        "mailers",
        "send_digest",
        json!({"to": "<team>"}),
        UNIX_EPOCH + Duration::from_secs(4_000_000_000),
      )
      .await
      .unwrap();
    let settings = toml::from_str(
      r#"
        backend = "sqlite"
        url = "sqlite::memory:"

        [queues]
        default = 3
        mailers = 1

        [schedules.nightly_digest]
        cron = "0 0 * * *"
        job = "send_digest"
      "#,
    )
    .unwrap();
    let response = Client::new(backend, settings)
      .dashboard()
      .oneshot(Request::get("/").body(Body::empty()).unwrap())
      .await
      .unwrap();
    let page = String::from_utf8(to_bytes(response.into_body(), usize::MAX).await.unwrap().to_vec()).unwrap();
    let start = page.find(&format!("<h2>{title}</h2>")).unwrap();
    let end = start + page[start..].find("</table>").unwrap();
    page[start..end].to_owned()
  }

  mod router {
    use pretty_assertions::assert_eq;

    use super::*;

    #[tokio::test]
    async fn it_lists_the_dead_set() {
      assert_eq!(
        section("Dead").await,
        concat!(
          "<h2>Dead</h2><table><tr><th>Id<th>Queue<th>Job<th>Args<th>Attempt<th>Error<th>Run at",
          "<tr><td>2<td>default<td>charge_card<td>1<td>0<td>&lt;card declined&gt;<td>"
        )
      );
    }

    #[tokio::test]
    async fn it_lists_the_queues_with_their_sizes() {
      assert_eq!(
        section("Queues").await,
        "<h2>Queues</h2><table><tr><th>Queue<th>Weight<th>Size<tr><td>default<td>3<td>1<tr><td>mailers<td>1<td>0"
      );
    }

    #[tokio::test]
    async fn it_lists_the_retries() {
      assert_eq!(
        section("Retries").await,
        concat!(
          "<h2>Retries</h2><table><tr><th>Id<th>Queue<th>Job<th>Args<th>Attempt<th>Error<th>Run at",
          "<tr><td>1<td>default<td>resize_image<td>null<td>1<td>boom<td>2096-10-02T07:06:40Z"
        )
      );
    }

    #[tokio::test]
    async fn it_lists_the_scheduled_jobs() {
      assert_eq!(
        section("Scheduled").await,
        concat!(
          "<h2>Scheduled</h2><table><tr><th>Id<th>Queue<th>Job<th>Args<th>Attempt<th>Error<th>Run at",
          "<tr><td>4<td>mailers<td>send_digest<td>{\"to\":\"&lt;team&gt;\"}<td>0<td><td>2096-10-02T07:06:40Z"
        )
      );
    }

    #[tokio::test]
    async fn it_lists_the_schedules() {
      assert_eq!(
        section("Schedules").await,
        concat!(
          "<h2>Schedules</h2><table><tr><th>Schedule<th>Cron<th>Job<th>Args",
          "<tr><td>nightly_digest<td>0 0 * * *<td>send_digest<td>null"
        )
      );
    }
  }
}
