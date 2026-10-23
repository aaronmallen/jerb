---
id: "0001"
title: Split jerb into a Cargo workspace with jerb-macros
status: active
created: 2026-10-23
area:
  - build
issue: "JERB-2"
tags:
  - workspace
  - proc-macro
  - layout
---

# ADR 0001: Split jerb into a Cargo workspace with jerb-macros

![Active][status]

## Context

Jerb puts a `#[jerb::job]` attribute on job functions. The attribute registers each job with no call from the user
and gives it a typed enqueue call. Rust only lets a proc macro live in a crate of its own, built as
`proc-macro = true`, and such a crate can export nothing but macros.

Jerb is one crate today, with no workspace. The scripts in `scripts/` run `cargo` once from the root and reach only
what that run can see.

## Decision

Jerb becomes a Cargo workspace with two crates:

- `jerb`, the library users depend on.
- `jerb-macros`, the proc macro crate that holds `#[jerb::job]`.

`jerb` depends on `jerb-macros` and re-exports the macro, so a user adds `jerb` alone and writes `#[jerb::job]`.

## Alternatives

**A sibling path crate with no workspace.** `jerb-macros` would sit beside `jerb` and `jerb` would point at it by
path. It would keep its own lockfile and target directory, and the scripts in `scripts/` would skip it, since they
run against the root crate.

**Drop the macro.** Users would register jobs and build enqueue calls by hand. Jerb would stay one crate but lose
auto-registration and typed enqueue.

## Consequences

- Users depend on one crate and never name `jerb-macros`.
- Both crates share one lockfile and one target directory, and one `cargo` run from the root reaches both.
- The repo layout changes: code no longer lives in a single `src/`, and `.claude/CLAUDE.md` has to say so.
- Each script in `scripts/` that runs `cargo` has to cover every member, not just the root crate.
- Publishing `jerb` means publishing `jerb-macros` first, since crates.io needs every dependency there. It shows
  up as a crate of its own, though nobody should depend on it directly.

[status]: https://img.shields.io/badge/Active-green?style=for-the-badge
