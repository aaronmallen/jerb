---
id: "0009"
title: Build the dashboard on axum
status: active
created: 2026-10-23
area:
  - dashboard
issue: "JERB-10"
tags:
  - axum
  - dashboard
  - dependencies
  - web
---

# ADR 0009: Build the dashboard on axum

![Active][status]

## Context

Jerb ships a web dashboard that lists queues, scheduled jobs, retries and the dead set. It needs a web framework to
route requests and serve pages. Jerb runs on tokio ([ADR 0004][0004]), and its job API already borrows the
extractor shape of [axum]: a job is a plain async fn whose params are `State<S>`, `Json<T>` and the like.

## Decision

The dashboard builds on axum. Its pages and endpoints are axum handlers on the app's tokio runtime. How the
dashboard mounts in the app is a separate question, which the spec leaves open.

## Alternatives

- **[actix-web].** It runs its handlers on a runtime of its own, a single threaded one per worker thread, rather
  than on the tokio runtime that jerb spawns onto. Its handlers would also read unlike the job API they sit beside.
- **A small server of jerb's own.** Jerb would write, test and keep up its own routing and request handling, code
  that has nothing to do with running jobs.

## Consequences

- The dashboard and the job API share one handler shape, so a reader of one can read the other.
- The dashboard runs on the runtime jerb already needs, with no glue between them.
- Jerb takes on axum and its own dependencies. axum has not reached 1.0, and its minor releases can break the API,
  so each upgrade may mean changes to the dashboard.

[0004]: 0004-run-on-tokio.md
[actix-web]: https://crates.io/crates/actix-web
[axum]: https://crates.io/crates/axum
[status]: https://img.shields.io/badge/Active-green?style=for-the-badge
