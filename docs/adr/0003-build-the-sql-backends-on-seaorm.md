---
id: "0003"
title: Build the SQL backends on SeaORM
status: active
created: 2026-10-23
area:
  - backends
issue: "JERB-4"
tags:
  - seaorm
  - sql
  - postgres
  - mysql
  - sqlite
  - migrations
---

# ADR 0003: Build the SQL backends on SeaORM

![Active][status]

## Context

Jerb stores jobs in Redis, Postgres, MySQL or SQLite, each behind its own Cargo feature, and any mix may be on. The
three SQL backends do the same work: insert a job, claim the next one, record the result. They differ in how they
claim. Postgres, and MySQL from 8.0, take `SKIP LOCKED`, so two workers never claim one row. SQLite has no row
locks and needs its own claim query. Each backend also needs migrations to create and change the job tables.

## Decision

The Postgres, MySQL and SQLite backends share one layer built on [SeaORM]. The job tables have one set of entities
and one set of migrations, written with SeaORM's migration crate, for all three. Queries go through SeaORM's query
builder, which renders each dialect's SQL, and the claim query branches on the backend where the dialects part.

The `postgres`, `mysql` and `sqlite` features turn on the matching SeaORM driver feature. Redis does not go through
this layer. It gets its own adapter.

## Alternatives

**[sqlx] alone.** SeaORM runs on sqlx, so this drops a layer. But sqlx takes SQL as text, so we would write each
query and each migration once per dialect and keep three copies in step.

**Hand-written queries per backend.** Each backend owns its SQL and its driver. This gives the most control over the
claim query, but it means three copies of every query and every migration, and nothing to catch one drifting from
the others.

## Consequences

- One change to the job schema is one migration, not three.
- The claim query still differs per dialect. SeaORM renders the SQL but does not hide that SQLite has no
  `SKIP LOCKED`, so each dialect needs its own claim tests.
- SeaORM and the query builder under it add to compile times and to the dependency tree of every app with a SQL
  feature on.
- SeaORM needs an async runtime feature, so its feature set has to follow whatever runtime jerb picks.
- Jerb carries two storage paths: the SQL layer and the Redis adapter. Behaviour such as claim timeouts and the dead
  set lands in both.

[SeaORM]: https://www.sea-ql.org/SeaORM/
[sqlx]: https://docs.rs/sqlx
[status]: https://img.shields.io/badge/Active-green?style=for-the-badge
