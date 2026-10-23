---
id: "0002"
title: Put every store behind one backend trait
status: active
created: 2026-10-23
area:
  - backends
issue: "JERB-3"
tags:
  - backend
  - trait
  - redis
  - postgres
  - mysql
  - sqlite
---

# ADR 0002: Put every store behind one backend trait

![Active][status]

## Context

Jerb stores jobs in Redis, Postgres, MySQL or SQLite. Each store sits behind its own Cargo feature, any mix of them
may be on at once, and `sqlite` is on by default. The config file picks the store when the app starts, so the client,
workers and scheduler cannot know at compile time which store they talk to.

The stores do not claim a job the same way. Postgres and MySQL 8.0 or later lock rows with `SKIP LOCKED`, SQLite has
one writer at a time, and Redis keeps jobs in sorted sets. The three SQL stores share one layer, and Redis gets its
own adapter.

## Decision

Every store implements one backend trait. The client, workers and scheduler call the trait and never a store
directly, and the trait is the only way jobs reach a store.

Each method on the trait names what jerb needs, such as claiming a job, and leaves how to the store. The claim
method on Postgres and MySQL uses `SKIP LOCKED`, the one on SQLite relies on its single writer, and the one on Redis
works on sorted sets.

The shared SQL layer implements the trait once for Postgres, MySQL and SQLite, and lets a store run its own query where
it needs one. The Redis adapter implements it on its own. Each impl compiles only with its feature on.

At start, jerb reads the store from the config file and builds the impl for it.

## Alternatives

**An enum over the stores.** One type with a variant per store, matched in each method. It avoids picking an impl
through the trait at run time, but every match needs a feature gate on each arm, and every new store touches every
match. The claim logic still differs per store, so each arm ends up as a store's impl inside one type. It also
closes the set: only the stores jerb ships can exist.

## Consequences

- A new store is one new impl behind one new feature, plus the code at start that builds it. Nothing that calls
  the trait changes.
- One test suite can run against every store, since each one meets the same trait.
- The trait has to fit four stores that work in different ways. A method one store cannot meet well, or a feature
  only one store has, either changes the trait for all four or stays out of jerb.
- Since the config picks the store at run time, calls go through the trait at run time too, and its async methods
  must return futures that can sit behind a pointer to the trait. That adds a heap allocation per call and limits
  what a method's signature can say.
- The trait shapes every backend issue that follows, so a change to it lands in each impl at once.

[status]: https://img.shields.io/badge/Active-green?style=for-the-badge
