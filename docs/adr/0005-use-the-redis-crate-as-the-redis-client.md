---
id: "0005"
title: Use the redis crate as the Redis client
status: active
created: 2026-10-23
area:
  - backends
issue: "JERB-6"
tags:
  - redis
  - dependencies
---

# ADR 0005: Use the redis crate as the Redis client

![Active][status]

## Context

Jerb can store jobs in Redis behind the `redis` feature. Redis does not go through the SQL layer; it gets its own
adapter that implements the backend trait. That adapter keeps delayed jobs in sorted sets scored by run time and must
claim a job in one atomic step, so two workers never take the same one. It runs on tokio, as the rest of jerb does.

## Decision

The Redis adapter talks to Redis through the [redis] crate, on its async API with tokio. The `redis` feature turns
on the crate and the crate features the adapter needs, such as tokio support.

## Alternatives

**[fred].** An async client with its own pooling, reconnects and a typed method per command, grouped behind
features. It covers what the adapter needs as well. It lost because the adapter sends a small, fixed set of commands,
so fred's extra layers buy jerb little, while the redis crate is the client most Rust code that talks to Redis
already uses.

## Consequences

- The redis crate and its dependencies land in every build with the `redis` feature on, and in no other build.
- The crate has no pool of its own, so the adapter picks how it holds connections.
- A redis crate release that changes its async API or its feature names means work in the adapter and in jerb's
  feature list.

[fred]: https://crates.io/crates/fred
[redis]: https://crates.io/crates/redis
[status]: https://img.shields.io/badge/Active-green?style=for-the-badge
