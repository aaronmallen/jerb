# Changelog

This file records each notable change to Jerb. It follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/).

## [Unreleased]

## [0.1.0] - 2026-11-05

The first release, of `jerb` and `jerb-macros`.

### Added

- `#[jerb::job]` turns an async fn into a job and adds a typed `enqueue` fn for it. Jobs register at link time, so
  `Router::new` finds them with no list to keep.
- `Router` with `with_state` for shared state and `with_middleware` to wrap every run. The `Json`, `State`, `JobId`
  and `Attempt` extractors read a job's args, state, id and attempt count.
- `jerb::start` checks the router against the settings, connects to the store and spawns the workers and schedulers.
  It returns a `Client`.
- `Settings`, a shape to deserialize from the app's own config file. It sets the store, concurrency, weighted queues,
  a worker count for any queue, per-job retries, timeout and unique enqueues, and cron schedules.
- Backends for SQLite (the default feature), Postgres, MySQL and Redis, each behind a Cargo feature of its name.
- `Client` enqueues now, at a time or after a delay. It also lists queued, running, scheduled, retrying, dead and
  completed jobs, finds, deletes and re-runs a job, pauses and resumes a queue, reads the stats and turns a schedule
  on or off or fires it now.
- The `dashboard` feature adds `Client::dashboard`, an axum router that serves a page of the queues, schedules and
  jobs.

[0.1.0]: https://aaronmallen.dev/aaron/jerb/releases/tag/v0.1.0
[Unreleased]: https://aaronmallen.dev/aaron/jerb/compare/v0.1.0...main
