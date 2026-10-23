---
id: "0007"
title: Register jobs at link time with inventory
status: active
created: 2026-10-23
area:
  - api
issue: "JERB-8"
tags:
  - inventory
  - jobs
  - macros
  - registration
---

# ADR 0007: Register jobs at link time with inventory

![Active][status]

## Context

A job is a plain async fn marked `#[jerb::job]`. The macro names the job after its fn, and `Router::new()` has to
find every job with no registration call. The jobs can live in any module of the app, so no single place in the code
lists them. Something has to collect them across the whole program and hand them to the router.

## Decision

`#[jerb::job]` submits each job to a registry with the [inventory] crate. `Router::new()` reads that registry and
holds every job the program links, keyed by name.

## Alternatives

- **Register each job by hand**, as in `Router::new().job(send_email)`. The router could then check each job's
  `State` type against its own at build. It lost because the spec wants `Router::new()` to find every job with no
  registration call, and a job left out of the chain would never run.
- **[linkme]**, which also collects items across the program at link time. JERB-8 records that we weighed it but not
  why it lost.

## Consequences

- Adding a job takes the attribute and nothing else. No list of jobs goes stale.
- The registry cannot carry each job's `State` type, so the build cannot check it against the router's.
  `jerb::start` checks it, and returns an error when they differ. A wrong `State` shows up when the app starts, not
  when it compiles.
- A reader cannot see which jobs a router holds from the code that builds it.
- Jerb takes on `inventory` as a dependency, and inherits its limits: it works only on the platforms it supports, and
  a linker can drop a job whose crate the rest of the program never refers to.

[inventory]: https://crates.io/crates/inventory
[linkme]: https://crates.io/crates/linkme
[status]: https://img.shields.io/badge/Active-green?style=for-the-badge
