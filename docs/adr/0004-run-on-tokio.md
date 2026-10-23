---
id: "0004"
title: Run on tokio
status: active
created: 2026-10-23
area:
  - api
  - backends
  - scheduler
issue: "JERB-5"
tags:
  - async
  - runtime
  - tokio
---

# ADR 0004: Run on tokio

![Active][status]

## Context

Jerb runs its workers and scheduler inside the app's own process, and each job is an async fn. Something has to
spawn the workers, run the job futures, sleep between polls and time out a claim. The standard library has no async
runtime, so jerb needs one, and the workers, the scheduler and the backends must share it.

## Decision

Jerb runs on [tokio]. Workers and the scheduler run as tokio tasks, and timers, polling and claim timeouts use
tokio's time. The backends do their I/O on the same runtime. `jerb::start` expects the app to run a tokio runtime
and spawns onto it.

## Alternatives

**Work on any runtime.** Jerb would reach spawning and timers through a layer of its own, and the app would pick the
runtime. It lost because that layer is code jerb has to write, test on every runtime it claims, and keep in step
with each one.

## Consequences

- An app on another runtime cannot use jerb without also running tokio.
- A job's future and its `State` must be `Send` and `'static`, since tokio's multi-thread runtime can move a task
  between threads.
- Later records can pick backend clients and a dashboard framework that run on tokio with no glue between them.

[status]: https://img.shields.io/badge/Active-green?style=for-the-badge
[tokio]: https://tokio.rs
