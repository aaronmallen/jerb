# Jerb

A background job runner for Rust, in the spirit of Sidekiq, that works with more than one backend rather than Redis
alone.

## Stack

A Cargo workspace of two Rust crates. Name no library or framework as if we had chosen it until a record in
`docs/adr` picks it.

## Layout

The root `Cargo.toml` holds the workspace and the `jerb` crate, whose code lives in `src/`. The proc macros live in
`macros/`, and `jerb` re-exports them, so users depend on `jerb` alone.

## Tooling

- [mise] manages tools and tasks (@.config/mise.toml). Run `mise tasks` to see them.

Always run the work through a mise task rather than calling `markdownlint-cli2`, `shellcheck` or **any** other tool
yourself. The tasks in `scripts/` carry the flags and config paths this project depends on, and they are what I
run, so a bare `markdownlint-cli2` reads no config at all.

- `mise run setup` installs every tool.
- `mise run format` and `mise run lint` cover every language.

If no existing task covers what you need, say so and suggest a new script rather than working around it with a one
off command.

## Writing Rules

Review every prose output (READMEs, docs, issues, commit messages) against these rules before delivering:

1. Never use a metaphor, simile or other figure of speech which you are used to seeing in print.
2. Never use a long word where a short one will do.
3. If it is possible to cut a word out, always cut it out.
4. Never use the passive where you can use the active.
5. Never use a foreign phrase, a scientific word or a jargon word if you can think of an everyday English equivalent.
6. Never use emdash
7. Break any of these rules sooner than say anything outright barbarous.

[mise]: https://mise.jdx.dev
