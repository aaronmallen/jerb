# Architecture decision records

Each record holds one decision: what we chose, why, and what it costs. Read [writing ADRs] before you add
one.

| # | Title | Status | Created |
| --- | --- | --- | --- |
| [0001] | Split jerb into a Cargo workspace with jerb-macros | ![Active][active] | 2026-10-23 |
| [0002] | Put every store behind one backend trait | ![Active][active] | 2026-10-23 |
| [0003] | Build the SQL backends on SeaORM | ![Active][active] | 2026-10-23 |
| [0004] | Run on tokio | ![Active][active] | 2026-10-23 |
| [0005] | Use the redis crate as the Redis client | ![Active][active] | 2026-10-23 |
| [0006] | Store job args as JSON | ![Active][active] | 2026-10-23 |
| [0007] | Register jobs at link time with inventory | ![Active][active] | 2026-10-23 |
| [0008] | Parse cron schedules with croner | ![Active][active] | 2026-10-23 |
| [0009] | Build the dashboard on axum | ![Active][active] | 2026-10-23 |

[0001]: 0001-split-jerb-into-a-cargo-workspace-with-jerb-macros.md
[0002]: 0002-put-every-store-behind-one-backend-trait.md
[0003]: 0003-build-the-sql-backends-on-seaorm.md
[0004]: 0004-run-on-tokio.md
[0005]: 0005-use-the-redis-crate-as-the-redis-client.md
[0006]: 0006-store-job-args-as-json.md
[0007]: 0007-register-jobs-at-link-time-with-inventory.md
[0008]: 0008-parse-cron-schedules-with-croner.md
[0009]: 0009-build-the-dashboard-on-axum.md
[active]: https://img.shields.io/badge/Active-green?style=for-the-badge
[writing ADRs]: ../writing-adrs.md
