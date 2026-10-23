# Jerb

Jerb is a background job library for Rust. It runs async functions on Tokio workers,
stores jobs in a database or Redis, and handles retries and recurring schedules.
Use it to move work out of a request handler or run tasks on a schedule while
keeping job code in your application.

## Features

- Typed job arguments and automatic registration with `#[jerb::job]`.
- SQLite, PostgreSQL, MySQL and Redis backends.
- Weighted queues and dedicated workers per queue.
- Delayed jobs, cron schedules, retries with backoff and per-job timeouts.
- Shared application state, middleware and an optional web dashboard.

## Quick start

Jerb requires Rust 1.94 or later. Depend on `jerb` alone; it re-exports the macros
from `jerb-macros`.

```sh
cargo add jerb
```

Add these dependencies to your application's `Cargo.toml`:

```toml
[dependencies]
jerb = "0.1"
tokio = { version = "1", features = ["macros", "rt-multi-thread", "signal"] }
toml = "1"
```

Define a job, start the workers and enqueue it:

```rust
use std::error::Error;

use jerb::{Json, Router, Settings};

#[jerb::job]
async fn greet(Json(name): Json<String>) -> Result<(), Box<dyn Error + Send + Sync>> {
    println!("Hello, {name}!");
    Ok(())
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error + Send + Sync>> {
    let settings: Settings = toml::from_str(
        r#"
        backend = "sqlite"
        url = "sqlite://jerb.sqlite?mode=rwc"
        "#,
    )?;

    let client = jerb::start(settings, Router::new()).await?;
    greet::enqueue(&client, "Ada".to_owned()).await?;

    tokio::signal::ctrl_c().await?;
    Ok(())
}
```

`Router::new()` collects the jobs marked with `#[jerb::job]`. The macro adds a
typed `greet::enqueue` function that returns the stored job's ID. Jobs without
arguments take `()` when enqueued.

`jerb::start` connects to the backend and spawns workers and schedulers on the
current Tokio runtime. Keep that runtime alive while jobs run. SQL backends apply
their schema migrations when they connect.

## Configuration

Your application loads and deserializes `Settings`; Jerb does not read a config
file itself. TOML is one option:

```toml
backend = "sqlite"
url = "sqlite://jerb.sqlite?mode=rwc"
concurrency = 5

[queues]
default = 1
bulk = 2

[workers]
urgent = 2

[jobs.greet]
queue = "urgent"
retries = 3
timeout = 30
unique = true

[schedules.morning_greeting]
cron = "0 9 * * *"
job = "greet"
args = "Ada"
```

Queues without a `[workers]` entry share the `concurrency` workers according to
their weights. Each `[workers]` entry adds dedicated workers for that queue;
a count of `0` stops it. Job and schedule names must match registered functions.

By default, Jerb runs five shared workers on the `default` queue. Each job allows
25 retries, has a 1,800-second timeout and has uniqueness turned off. With
`unique = true`, normal enqueue calls reuse an existing pending job with the same
name and arguments.

## Backends

SQLite is enabled by default. To use another backend, select its Cargo feature
and set `backend` and `url` in your settings:

```toml
jerb = { version = "0.1", default-features = false, features = ["postgres"] }
```

| Feature    | `backend`  | Example URL                     |
| ---------- | -----------| ------------------------------- |
| `sqlite`   | `sqlite`   | `sqlite://jerb.sqlite?mode=rwc` |
| `postgres` | `postgres` | `postgres://localhost/jerb`     |
| `mysql`    | `mysql`    | `mysql://localhost/jerb`        |
| `redis`    | `redis`    | `redis://localhost/`            |

The MySQL backend requires MySQL 8.0 or later.

Redis supports job execution, delayed jobs, retries, unique jobs and cron
schedules. Some management features, including queue pausing, job deletion,
statistics and schedule switches, require a SQL backend.

## Working with jobs

Use `Json<T>` for stored arguments, `State<T>` for application state, and `JobId`
or `Attempt` for details about the current run. Supply state with
`Router::new().with_state(state)` and add middleware with `.with_middleware(...)`.

The returned `Client` supports delayed jobs through `enqueue_in` and `enqueue_at`.
It also provides methods to inspect jobs, pause and resume queues, retry a job
with `run_now`, and trigger a schedule with `fire_now`. Support for these
operations depends on the backend.

Enable the `dashboard` Cargo feature to expose `client.dashboard()`, an Axum
router you can mount in your application. It shows queues, schedules and job
status, with controls for managing jobs and schedules.

```rust
let app = axum::Router::new().nest("/jerb", client.dashboard());
```

## Development

The repository uses [mise] for its tools and checks:

```sh
mise install
mise run lint
mise run test
```

See the [architecture decisions] for the design and the [changelog] for changes.

## License

Jerb uses the [MIT license].

[architecture decisions]: docs/adr
[changelog]: CHANGELOG.md
[mise]: https://mise.jdx.dev
[MIT license]: LICENSE
