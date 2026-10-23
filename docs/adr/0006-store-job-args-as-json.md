---
id: "0006"
title: Store job args as JSON
status: active
created: 2026-10-23
area:
  - api
  - backends
issue: "JERB-7"
tags:
  - args
  - json
  - wire-format
---

# ADR 0006: Store job args as JSON

![Active][status]

## Context

A job's args leave the process that enqueues it and come back in the worker that runs it, through whichever of the
four backends the app picked. Every backend has to store them the same way, or a job enqueued against one store
would not decode the same against another.

Job fns take their args through a `Json<T>` extractor, and `#[jerb::job]` generates a typed `enqueue` for any args
type that serializes. Schedules in the config file carry args too, and those must decode into the same `T` the job
takes.

## Decision

Jerb stores a job's args as one JSON value. The typed `enqueue` encodes `T` to JSON, every backend stores that value
as is, and the `Json<T>` extractor decodes it in the worker. Schedule args from the config file go through the same
conversion to JSON before they reach a backend.

## Alternatives

- **MessagePack.** Smaller and faster to encode, but binary. A stored job would not read as text in a SQL column,
  a Redis key or the dashboard.
- **A plain `String` each job parses**, as in the first sketch. Jerb would not know the args' shape, so it could not
  generate a typed `enqueue`, and each job would carry its own parsing code and its own format.

## Consequences

- Args read as text in every backend's store and in the dashboard.
- Args take more space and time to encode than a binary format.
- An args type must survive a round trip through JSON. Bytes, map keys that are not strings and integers too large
  for some JSON readers need care. A type that does not round trip fails when the worker decodes it, not when the app
  enqueues it.
- Changing an args type can break jobs already stored under the old shape.

[status]: https://img.shields.io/badge/Active-green?style=for-the-badge
