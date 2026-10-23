---
id: "0008"
title: Parse cron schedules with croner
status: active
created: 2026-10-23
area:
  - scheduler
issue: "JERB-9"
tags:
  - cron
  - dependencies
---

# ADR 0008: Parse cron schedules with croner

![Active][status]

## Context

Jerb runs recurring jobs listed in the config file, as sidekiq-scheduler does. Each schedule is a cron expression,
and the scheduler needs a parser that reads one and finds the next time it fires.

A crontab and a sidekiq-scheduler file both write a schedule as five fields, such as `0 0 * * *`.

## Decision

The scheduler parses cron expressions with [croner]. A schedule takes five fields, or six with seconds in front, and
the scheduler asks croner for the next fire after a given time.

## Alternatives

**The [cron] crate.** Its parser needs six or seven fields, with seconds first and an optional year last, and rejects
a five field line. Every schedule copied from a crontab or a sidekiq-scheduler file would need a leading `0`. It also
has no `L`, `W` or `#`, so "the last Friday of the month" has no expression.

## Consequences

- Schedules read the way they do in a crontab, and the `L`, `W` and `#` forms work.
- Croner and its own dependencies land in every build of jerb.
- Croner finds the next fire in the time zone of the time we pass it. This record does not settle which zone
  schedules run in; the spec leaves that open.
- A croner release that changes how it reads an expression changes when our schedules fire.

[cron]: https://crates.io/crates/cron
[croner]: https://crates.io/crates/croner
[status]: https://img.shields.io/badge/Active-green?style=for-the-badge
