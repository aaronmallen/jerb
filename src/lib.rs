//! A background job runner, in the spirit of Sidekiq, that works with more than one backend rather than Redis alone.
//!
//! Write a job as an async fn and mark it with [`job`]. Build a [`Router`], which finds every job, and a [`Settings`],
//! which the app deserializes from its own config file. Pass both to [`start`], which connects to the store, starts
//! the workers and returns a [`Client`] that enqueues jobs.
//!
//! ```no_run
//! use std::{error::Error, time::Duration};
//!
//! use jerb::{Json, Router, Settings};
//!
//! #[jerb::job]
//! async fn send_email(Json(to): Json<String>) -> Result<(), Box<dyn Error + Send + Sync>> {
//!   println!("sending an email to {to}");
//!   Ok(())
//! }
//!
//! #[tokio::main]
//! async fn main() -> Result<(), Box<dyn Error + Send + Sync>> {
//!   let settings: Settings = toml::from_str(
//!     r#"
//!       backend = "sqlite"
//!       url = "sqlite://jerb.sqlite?mode=rwc"
//!       concurrency = 10
//!
//!       [jobs.send_email]
//!       retries = 3
//!       timeout = 30
//!     "#,
//!   )?;
//!
//!   let client = jerb::start(settings, Router::new()).await?;
//!
//!   send_email::enqueue(&client, "ada@example.com".into()).await?;
//!
//!   // Give a worker time to run the job. A real app keeps running.
//!   tokio::time::sleep(Duration::from_secs(1)).await;
//!   Ok(())
//! }
//! ```
//!
//! A job takes its args through [`Json`] and shared state, such as a connection pool, through [`State`]. Hand the
//! state to the router with [`Router::with_state`] and wrap every run in a [`Middleware`] with
//! [`Router::with_middleware`].
//!
//! # Features
//!
//! Each store sits behind a Cargo feature, and `sqlite` is the default.
//!
//! | Feature | Adds |
//! | --- | --- |
//! | `sqlite` | The SQLite backend. On by default. |
//! | `postgres` | The Postgres backend. |
//! | `mysql` | The MySQL backend, 8.0 or later. |
//! | `redis` | The Redis backend. |
//! | `dashboard` | `Client::dashboard`, an axum router that serves a page of queues, schedules and jobs. |
//!
//! [`start`] fails with [`Error::Unsupported`] when the settings pick a store whose feature is off.

extern crate self as jerb;

pub mod backend;
mod busy;
mod client;
#[cfg(feature = "dashboard")]
mod dashboard;
mod enqueue;
mod error;
mod extract;
mod handler;
mod middleware;
mod queues;
mod route;
mod router;
mod run;
mod schedule_status;
mod scheduler;
mod settings;
mod start;

pub use busy::QueueWorkers;
pub use client::Client;
pub use enqueue::Enqueue;
pub use error::Error;
pub use extract::{Attempt, FromRun, JobId, Json, State};
pub use handler::Handler;
#[doc(hidden)]
pub use inventory as __inventory;
pub use jerb_macros::job;
pub use middleware::{Middleware, Next};
pub use route::{Route, call as __call};
pub use router::Router;
pub use run::Run;
pub use schedule_status::ScheduleStatus;
#[doc(hidden)]
pub use serde_json as __serde_json;
pub use settings::{BackendType, JobSettings, Schedule, Settings};
pub use start::start;
