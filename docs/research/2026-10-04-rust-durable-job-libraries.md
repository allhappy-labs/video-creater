# Rust durable job libraries: can one replace the custom admission code and the Temporal backend?

Research date: 2026-10-04. Scope: maintained Rust libraries (and bundle-able sidecars) that could replace (a) the custom in-process durable render admission in `src-tauri/src/render_pipeline/project_export/` and (b) the development-only Temporal backend in `src-tauri/src/workflows/`. Sources are primary only: the crates.io API, the projects' GitHub repositories (metadata via the GitHub API, README, LICENSE, source at the default branch head), and release assets. No builds, tests or benchmarks were run. Nothing in the repository was changed apart from adding this note.

Marking convention: **[V]** = verified today against the URL that follows. **[I]** = my inference or judgement, not a sourced fact. "Recent downloads" is the crates.io `recent_downloads` field (last 90 days). "Releases in 12 months" counts versions published on crates.io since 2025-10-04. Issue counts are open/closed issues excluding pull requests, from the GitHub search API.

## 1. Answer

No library beats the current custom code for render and export admission: that code stores job state inside the project folder with hardened file handling, byte budgets and a project-identity check, and none of the candidates provides those. [I] For the Temporal-only paths (transcription, provider generation, Codex edit, NLE export), the best library fit under the hard constraints is **apalis 1.0.0-rc.10 with `apalis-sqlite` and `apalis-workflow`** (MIT, embedded SQLite, in-process, idempotency keys, heartbeat-based orphan recovery, stepped workflows); **duroxide 0.1.30** (MIT, Microsoft, embedded SQLite, Temporal-style replay, durable cancellation and progress status) is functionally closer to Temporal but is labelled "Preview" and is ten months old. [I] Both are effectively single-maintainer and pre-1.0, and both add `sqlx` plus a bundled SQLite to an app that currently links neither, so the lowest-risk option remains extending the existing custom admission pattern to the remaining job kinds and deleting the Temporal feature. [I] Every server-based option is either non-permissive (Restate, Golem, Zizq: BSL 1.1; Obelisk: AGPL-3.0) or a 40 MB+ sidecar (Temporal CLI), and every Postgres-only crate is excluded by the storage constraint. [V, sources in section 3]

## 2. Comparison table

All rows verified against the crates.io API and the GitHub API on 2026-10-04 unless a cell says otherwise; URLs are in section 3.

| Candidate | Latest (date) | Licence | Storage | Shape | Releases, 12 mo | Last commit | Recent / total downloads | Meets hard constraints? |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| apalis + apalis-sqlite + apalis-workflow | 1.0.0-rc.10 (2026-09-15; sqlite 2026-10-02; workflow 0.1.0-rc.10) | MIT (apalis: MIT OR Apache-2.0) | SQLite via sqlx 0.9, also JSON file, Redis, Postgres, MySQL | In-process | 19 / 19 / 17 | 2026-09-21 (sqlite 2026-10-02) | 340,904 / 1,230,073 (sqlite 34,870 / 58,589) | Yes |
| duroxide | 0.1.30 (2026-07-29) | MIT | SQLite built in (sqlx 0.8, bundled), Postgres via duroxide-pg | In-process | 31 | 2026-09-10 | 29,302 / 37,300 | Yes |
| effectum | 0.7.0 (2024-07-23) | MIT OR Apache-2.0 | SQLite only (rusqlite 0.31, bundled) | In-process | 0 | 2024-07-23 | 4,710 / 22,651 | Yes, but unmaintained |
| fang | 0.11.0 (2026-07-02) | MIT | Postgres, SQLite, MySQL (sqlx 0.8 / diesel) | In-process | 1 | 2026-07-02 | 7,329 / 109,457 | Yes |
| obelisk | 0.41.5 stable, 0.42.0-rc.4 (2026-09-26) | AGPL-3.0-only | SQLite or Postgres | Server binary, WASM components | 49 | 2026-10-04 | 296 / 31,618 | No (licence) |
| underway | 0.2.0 (2025-07-16) | MIT OR Apache-2.0 | Postgres only | In-process | 0 | 2026-08-07 | 6,026 / 23,386 | No (storage) |
| rexecutor (+ rexecutor-sqlx) | 0.1.1 (2025-11-08) | MIT OR Apache-2.0 | Postgres only (in-memory for tests) | In-process | 1 | 2026-04-23 | 48 / 1,806 | No (storage) |
| background-jobs | 0.20.0 (2026-01-13) | AGPL-3.0 | sled, Postgres, in-memory (not examined) | In-process | 1 | not checked | 3,260 / 120,727 | No (licence) |
| sqlxmq | 0.6.0 (2025-05-25) | MIT OR Apache-2.0 | Postgres only | In-process | 0 | 2025-05-25 | 15,765 / 192,233 | No (storage) |
| graphile_worker | 0.13.6 (2026-09-29) | MIT | Postgres only | In-process | 21 | 2026-10-03 | 29,301 / 65,203 | No (storage) |
| Restate (restate-sdk + server) | SDK 0.12.1 (2026-09-22); server v1.7.13 (2026-10-01) | SDK MIT; server BSL 1.1 | Server-owned | Sidecar, ~43–49 MB compressed | 7 | 2026-10-02 | 603,756 / 885,684 | No (server licence) |
| Temporal (temporalio-sdk + CLI dev server) | SDK 1.0.0 (2026-09-04); CLI v1.9.1 (2026-09-14) | SDK MIT; CLI and server MIT | Dev server: SQLite file | Sidecar, ~43–45 MB compressed | 9 | 2026-10-02 | 2,293,348 / 2,471,647 | Licence yes; needs sidecar |
| DBOS (`dbos` crate) | 0.5.0 (2026-09-09) | MIT | Postgres only | In-process | 3 | 2026-10-02 | 1,550 / 1,550 | No (storage) |
| flawless | 1.0.0-beta.3 (2024-12-09) | SDK crate BSD-2-Clause-Patent; server not verified | not verified | Server + WASM | 0 | no public repo found | 288 / 23,988 | Not verifiable; stale |
| Golem (golem-rust + server) | SDK 2.1.0 stable, 3.0.0-rc2 (2026-10-02); server v1.5.10 (2026-08-24) | SDK Apache-2.0; server BSL 1.1 | Server-owned | Sidecar, 141–180 MB uncompressed | 19 | 2026-10-04 | 2,338 / 32,877 | No (server licence, size) |
| Resonate (resonate-sdk + server) — not on the original list | SDK 0.6.0 (2026-07-22); server v0.9.8 (2026-06-04) | Apache-2.0 both | Server has a SQLite crate; SDK "local" mode is in-memory | Sidecar, ~10–11 MB compressed | 5 | SDK 2026-08-24 | 1,891 / 2,249 | Licence yes; needs sidecar |
| ora — not on the original list | 0.13.0 stable, 1.0.0-rc.3 (2026-09-25) | MIT OR Apache-2.0 | not verified | gRPC scheduler (tonic), optional embedded server | 13 | 2026-09-27 | 572 / 22,450 | Not verified |
| zizq — not on the original list | client 0.7.0 (2026-09-08) | Client MIT; server BSL 1.1, non-commercial grant | Server-owned | Sidecar | 8 | 2026-09-28 | 82 / 189 | No (server licence) |
| aide-de-camp-sqlite — not on the original list | 0.2.0 (2022-12-18) | MIT OR Apache-2.0 | SQLite | In-process | 0 | not checked | 24 / 4,866 | Abandoned |

## 3. Per-candidate notes

Common repository facts for the risk columns:

- `src-tauri/Cargo.lock` contains no `sqlx`, `sqlx-core`, `sqlx-sqlite`, `rusqlite`, `libsqlite3-sys`, `diesel`, `sled`, `redb` or `wasmtime` package entries. [V: grep of `src-tauri/Cargo.lock`, zero matches] So no candidate creates an sqlx *version conflict*; any SQLite-backed candidate adds sqlx (or rusqlite) and a bundled SQLite as a new dependency tree. [I]
- The lockfile already has `tokio 1.52.3`, `tonic 0.14.6`, `prost 0.14.4` and `0.11.9`, `reqwest 0.12.28` and `0.13.4`, `tower 0.5.3`, and `temporalio-* 0.5.0`. [V: `src-tauri/Cargo.lock`]
- The local toolchain is `rustc 1.98.1`; `src-tauri/Cargo.toml:9` declares `rust-version = "1.87"`. [V: `rustc --version`, `src-tauri/Cargo.toml`] The task brief said "Rust 1.98"; the declared MSRV in the manifest is lower than the toolchain. Any candidate with an MSRV above 1.87 would require raising the manifest value. [I]

### apalis, apalis-sqlite, apalis-workflow

- Versions: `apalis` 1.0.0-rc.10 published 2026-09-15; latest stable is still 0.7.4. `apalis-sqlite` 1.0.0-rc.10 published 2026-10-02 (crate created 2025-10-17, no stable release). `apalis-workflow` 0.1.0-rc.10 published 2026-09-15 (crate created 2025-09-13, no stable release). [V: https://crates.io/api/v1/crates/apalis , https://crates.io/api/v1/crates/apalis-sqlite , https://crates.io/api/v1/crates/apalis-workflow]
- The brief named `apalis-sql` as the SQLite backend. That is out of date: `apalis-sql` is now "SQL utilities" (last version 1.0.0-rc.9, 2026-05-06) and the SQLite backend is the separate `apalis-sqlite` crate and repository. [V: https://crates.io/api/v1/crates/apalis-sql , https://github.com/apalis-dev/apalis-sqlite]
- Cadence: 19 versions of `apalis` and 19 of `apalis-sqlite` in the last 12 months, all release candidates. [V: crates.io API, as above] The 1.0 line has therefore been in RC for the whole period; I found no stated date for a final 1.0. [I]
- Last commit: `apalis-dev/apalis` 2026-09-21; `apalis-dev/apalis-sqlite` 2026-10-02. [V: https://github.com/apalis-dev/apalis/commits/main , https://github.com/apalis-dev/apalis-sqlite/commits/main]
- Maintainers: one crates.io owner (`geofmureithi`) on both crates; top human contributors on the main repo are geofmureithi 437 commits, autotaker 27, jayvdb 7. On apalis-sqlite: geofmureithi 102, others 1 each. [V: https://crates.io/api/v1/crates/apalis/owners , https://github.com/apalis-dev/apalis/graphs/contributors] Bus factor is one. [I]
- Licence: `apalis` MIT OR Apache-2.0; `apalis-core`, `apalis-sqlite`, `apalis-workflow` MIT. [V: crates.io API `versions[0].license`; https://github.com/apalis-dev/apalis/blob/main/LICENSE]
- Storage: `apalis-sqlite` depends on `sqlx ^0.9.0` with only the `sqlite` feature and default features off; tokio (`rt`, `net`) or async-std are optional runtime features. A JSON/CSV file backend also exists as `apalis-file-storage` 0.1.0-rc.10 (1,841 total downloads). [V: https://crates.io/api/v1/crates/apalis-sqlite/1.0.0-rc.10/dependencies , https://github.com/apalis-dev/apalis-sqlite/blob/main/Cargo.toml , https://crates.io/api/v1/crates/apalis-file-storage]
- Runtime and shape: in-process library; README states "Runtime agnostic - Works with tokio, async-std". [V: https://github.com/apalis-dev/apalis/blob/main/README.md]
- Features against our requirements:
  - Retries, scheduling, priorities, heartbeat and orphan recovery: listed in the apalis-sqlite README ("jobs held by a dead worker are automatically re-enqueued"). The re-enqueue query sets the job back to `Pending` and increments `attempts`. Default heartbeat interval is 30 s with 2 missed heartbeats. [V: https://github.com/apalis-dev/apalis-sqlite/blob/main/README.md , `queries/backend/reenqueue_orphaned.sql`, `src/config.rs` in that repo] This is "re-run after timeout", not "fail cleanly at next start" as our code does. [I]
  - Idempotent admission: `TaskBuilder::idempotency_key(..)` exists in apalis-core; apalis-sqlite has a migration dated 2026-05-06 adding `idempotency_key` with `UNIQUE INDEX ... ON Jobs(job_type, idempotency_key)`, and the insert uses `ON CONFLICT(job_type, idempotency_key) DO NOTHING`. [V: https://github.com/apalis-dev/apalis/blob/main/apalis-core/src/task/builder.rs , `migrations/20260506101935_idempotency_key.sql` and `queries/task/sink.sql` in https://github.com/apalis-dev/apalis-sqlite] A duplicate push is silently dropped; returning the original admission record to the caller (our "replay") would be our code. [I]
  - Cancellation of a running task: `TaskContext::cancel()` / `is_cancelled()` and `WorkerContext::cancel_task(..)` exist in apalis-core; apalis-sqlite has a `Killed` status and a `kill.sql` query. [V: `apalis-core/src/task/context.rs`, `apalis-core/src/worker/context.rs` in https://github.com/apalis-dev/apalis ; `queries/task/kill.sql` in apalis-sqlite] Cancellation is cooperative and in-process; I did not find a durable cross-process cancel request API. [I]
  - Progress: the README sequence diagram mentions "Report task progress", but a source search of apalis-core for a progress API found only comments. [V: README; grep of `apalis-core/src` at rc.10] Treat progress reporting as not provided. [I]
  - Multi-step workflows: `apalis-workflow` offers `SteppedFlow` (sequential, delay, filter_map, fold, repeater) and `GraphFlow` (DAG), with SQLite listed as a supported backend; "Subflow" is marked incomplete. README claims "durable and resumable workflows". [V: https://github.com/apalis-dev/apalis/blob/main/apalis-workflow/README.md]
  - Deterministic replay: none; it is a queue with step chaining. [I, from the README's model]
  - Concurrency limit: `WorkerBuilder::concurrency(n)`. [V: apalis README]
- Published benchmarks: none current. The changelog says benchmarks for storages were added in 0.4.1, but neither repository has a `benches/` directory or benchmark figures in its README at rc.10. [V: https://github.com/apalis-dev/apalis/blob/main/CHANGELOG.md ; directory listings of both repos]
- Issue health: apalis 3 open / 184 closed; apalis-sqlite 0 open / 8 closed. [V: GitHub search API, `repo:apalis-dev/apalis type:issue`]
- Risks: still RC; MSRV `rust-version = "1.85"` in the workspace manifest (below our 1.87); brings sqlx 0.9. [V: https://github.com/apalis-dev/apalis/blob/main/Cargo.toml] The SQLite backend and the workflow crate are each about one year old. [V: crate `created_at`]

### duroxide

- Version: 0.1.30, published 2026-07-29; crate created 2025-11-30. [V: https://crates.io/api/v1/crates/duroxide]
- Cadence: 31 versions in the last 12 months (the crate's whole life). Last commit on `main` 2026-09-10; repository pushed 2026-10-03. [V: crates.io API; https://github.com/microsoft/duroxide/commits/main]
- Maintainers: crates.io owners `affandar`, `microsoft-oss-releases`, team `microsoft:duroxide`. Contributors: affandar 255 commits, cursoragent 17, pinodeca 5, tjgreen42 4. [V: https://crates.io/api/v1/crates/duroxide/owners , https://github.com/microsoft/duroxide/graphs/contributors] Organisation-backed but one dominant author. [I]
- Licence: MIT. [V: https://github.com/microsoft/duroxide/blob/main/LICENSE , crates.io API]
- Status: the README says "Preview: This project is currently in preview" and that releases are published through Microsoft-managed pipelines. [V: https://github.com/microsoft/duroxide/blob/main/README.md]
- Storage: feature `sqlite` enables `sqlx 0.8` (features `runtime-tokio-native-tls`, `sqlite`, `macros`, `migrate`, `chrono`) and `libsqlite3-sys >=0.28` with `bundled`. File databases use WAL. A `Provider` trait allows custom storage. [V: https://github.com/microsoft/duroxide/blob/main/Cargo.toml , `src/providers/sqlite.rs`]
- Runtime and shape: "Embeddable — runs in-process on Tokio. No separate server to operate." [V: README]
- Features against our requirements:
  - Deterministic replay, activities, durable timers, external events, sub-orchestrations, retries with backoff and per-attempt timeouts. [V: README]
  - Cancellation: `client.cancel_instance(id, reason)`; in-flight activities get a cooperative signal (`ctx.is_cancelled()`), are forcibly aborted after `activity_cancellation_grace_period` (default 30 s), and child orchestrations are cancelled too. [V: https://github.com/microsoft/duroxide/blob/main/docs/ORCHESTRATION-GUIDE.md , section "Cancel an Orchestration"]
  - Progress: `ctx.set_custom_status(..)` plus client-side `wait_for_status_change`, and per-instance durable key/value state. [V: same guide]
  - Idempotent admission: the client doc comment states "Reusing an instance ID that already exists will fail". [V: `src/client/mod.rs` line 238 at head] Mapping that error to "replay the existing admission" would be our code. [I]
  - Concurrency: `RuntimeOptions { orchestration_concurrency, worker_concurrency, .. }`. [V: `src/runtime/mod.rs`]
- Published performance data: the repository carries `stress-test-results.md`. The newest entry (commit 7893d1d, 2025-11-17) reports file-backed SQLite at 16.11 orchestrations/s (1/1 config) and 27.95 orchestrations/s (2/2 config), with one lock-contention failure in 850 runs. [V: https://github.com/microsoft/duroxide/blob/main/stress-test-results.md] That is far above our load of a handful of jobs, but the file has not been updated since November 2025. [I]
- Issue health: 24 open / 6 closed. [V: GitHub search API, `repo:microsoft/duroxide type:issue`]
- Risks: 0.1.x preview; no declared MSRV (`rust-toolchain.toml` says `stable`); sqlx 0.8 with a native-tls runtime feature; orchestration code must follow replay rules (`ctx.join`, `ctx.utcnow()`, no `tokio::select!`). [V: `Cargo.toml`, `rust-toolchain.toml`, README] The native-tls feature probably pulls a TLS dependency that a SQLite-only build does not need. [I, not checked by building]

### effectum

- Version 0.7.0, 2024-07-23; no releases in the last 12 months; last commit 2024-07-23. Single owner and author (dimfeld, 94 commits). 2 open / 5 closed issues. [V: https://crates.io/api/v1/crates/effectum , https://github.com/dimfeld/effectum]
- Licence MIT OR Apache-2.0 (GitHub detects Apache-2.0; both licence files are present). [V: crates.io API; repo root listing]
- Storage: SQLite only, `rusqlite 0.31` with default feature `bundled-sqlite`, `deadpool-sqlite`; tokio. In-process. [V: https://github.com/dimfeld/effectum/blob/master/effectum/Cargo.toml , https://crates.io/api/v1/crates/effectum/0.7.0/dependencies]
- Features (README "Released" list): priorities, scheduled and recurring jobs, retries with exponential backoff, checkpoints (`checkpoint_json`, `checkpoint_blob`), `heartbeat`, immediate retry of jobs that were running when the process restarted, cancel or modify *pending* jobs, `max_concurrency`. [V: https://github.com/dimfeld/effectum/blob/master/README.md , `effectum/src/job.rs`, `effectum/src/add_job.rs`, `effectum/src/worker.rs`] No dedup key, no running-job cancellation, no workflow steps were found. [I, from the README and the public function list]
- Benchmarks: a `stress_test` directory exists; no published figures. [V: repo root listing]
- Risk: unmaintained for 26 months; pinned to rusqlite 0.31. [V: dates above] The feature set is the closest in spirit to our custom code, so it is useful as a design reference rather than a dependency. [I]

### fang

- Version 0.11.0, 2026-07-02; one release in 12 months; last commit 2026-07-02. Owners ayrat555 and pxp9 (81 and 50 commits). MIT. MSRV 1.77. 6 open / 23 closed issues. [V: https://crates.io/api/v1/crates/fang , https://github.com/ayrat555/fang]
- Storage: "It can use PostgreSQL, SQLite or MySQL"; feature `asynk-sqlite`; depends on `sqlx ^0.8` (feature `any`) and optionally diesel. Tokio. In-process. [V: https://github.com/ayrat555/fang/blob/master/README.md , https://crates.io/api/v1/crates/fang/0.11.0/dependencies]
- Features: cron scheduling, unique tasks ("Tasks are not duplicated in the queue if they are unique"), retries. [V: README] Running-job cancellation, progress and multi-step workflows are not mentioned in the README. [I]
- Benchmarks: none found in the README. [V]

### obelisk (obeli.sk)

- Stable 0.41.5, pre-release 0.42.0-rc.4 (2026-09-26); GitHub release v0.42.0 on 2026-10-04; 49 crate versions in 12 months; last commit 2026-10-04; one human contributor (tomasol, 6,402 commits). 1 open / 45 closed issues. MSRV 1.96. [V: https://crates.io/api/v1/crates/obelisk , https://github.com/obeli-sk/obelisk]
- Licence: **AGPL-3.0-only**. [V: crates.io API; https://github.com/obeli-sk/obelisk/blob/main/LICENSE] Excluded by the licence constraint, including as a bundled sidecar under this project's "no GPL/AGPL" rule. [I]
- Shape: "A single binary executing deterministic workflows, activities, and webhook endpoints, persisting steps in execution log using SQLite or PostgreSQL"; built on the WASM Component Model (depends on wasmtime 48). Release archives are 38–47 MB compressed. [V: README; https://crates.io/api/v1/crates/obelisk/0.42.0-rc.4/dependencies ; https://github.com/obeli-sk/obelisk/releases/latest]

### underway

- 0.2.0, 2025-07-16; no releases in 12 months, but last commit 2026-08-07. Owner maxcountryman (140 commits). MIT OR Apache-2.0. 13 open / 20 closed issues. [V: https://crates.io/api/v1/crates/underway , https://github.com/maxcountryman/underway]
- "Durable background workflows on Postgres"; depends on `sqlx ^0.8.2` with `postgres`. [V: README; https://crates.io/api/v1/crates/underway/0.2.0/dependencies] Excluded: Postgres only.

### rexecutor

- 0.1.1, 2025-11-08; one release in 12 months; last commit 2026-04-23; single author (Johnabell, 62 commits); 8 open / 0 closed issues; total downloads 1,806. MIT OR Apache-2.0 per crates.io (GitHub detects no licence file type). MSRV 1.85. [V: https://crates.io/api/v1/crates/rexecutor , https://github.com/Johnabell/rexecutor]
- Backends: an in-memory backend "primarily provided for testing purposes" and `rexecutor-sqlx`, which depends on `sqlx 0.8.*` with `postgres`. No `rexecutor-sqlite` crate exists. Has uniqueness criteria for jobs. [V: README; https://crates.io/api/v1/crates/rexecutor-sqlx/0.1.1/dependencies ; https://crates.io/api/v1/crates/rexecutor-sqlite returns not found] Excluded: Postgres only.

### background-jobs

- 0.20.0, 2026-01-13; one release in 12 months. Licence **AGPL-3.0** (also `background-jobs-core`, `background-jobs-sled`). Repository is on git.asonix.dog, which I did not fetch. [V: https://crates.io/api/v1/crates/background-jobs , https://crates.io/api/v1/crates/background-jobs-sled] Excluded by licence. If the brief assumed this crate was permissive, that was wrong.

### sqlxmq

- 0.6.0, 2025-05-25; no releases in 12 months; last commit 2025-05-25. MIT OR Apache-2.0. 9 open / 26 closed issues. "A job queue built on sqlx and PostgreSQL". [V: https://crates.io/api/v1/crates/sqlxmq , https://github.com/Diggsey/sqlxmq] Excluded: Postgres only.

### graphile_worker (Rust)

- 0.13.6, 2026-09-29; 21 releases in 12 months; last commit 2026-10-03; leo91000 408 commits; MIT; 2 open / 13 closed issues. "A PostgreSQL-backed job queue"; relies on `SKIP LOCKED` and `LISTEN/NOTIFY`; depends on `sqlx ^0.9.0` with `postgres`. [V: https://crates.io/api/v1/crates/graphile_worker , https://github.com/leo91000/graphile_worker_rs/blob/main/README.md] Excluded: Postgres only. The repository description says "High performance"; the README sections I read contain no benchmark figures. [V]

### Restate

- `restate-sdk` 0.12.1, 2026-09-22; 7 releases in 12 months; MIT; MSRV 1.90; last commit 2026-10-02; 7 open / 31 closed issues. The SDK is a service endpoint (hyper) that a Restate server invokes; it is not usable without the server. [V: https://crates.io/api/v1/crates/restate-sdk , https://github.com/restatedev/sdk-rust/blob/main/README.md]
- Server licence: **Business Source License 1.1** with an additional use grant that forbids offering a public Restate platform service. [V: https://github.com/restatedev/restate/blob/main/LICENSE] BSL is on our exclusion list, so the server cannot be bundled under the stated policy even though the grant would probably allow our use. [I]
- Sidecar size: `restate-server` v1.7.13 archives are 49.3 MB (x86_64 Linux musl, .tar.xz) and 43.4 MB (aarch64 macOS, .tar.xz). [V: https://github.com/restatedev/restate/releases/tag/v1.7.13]

### Temporal Rust SDK and bundled dev server

- The SDK reached **1.0.0** on 2026-09-04 (`temporalio-sdk`, `temporalio-client`); this repository pins 0.5.0. The crate was first published on 2026-02-19 (0.1.0-alpha.1) and has 9 versions in 12 months: 0.1.0-alpha.1, 0.2.0 through 0.8.0, and 1.0.0 ( 0.6.0 on 2026-08-04, 0.7.0 on 2026-08-17, 0.8.0 on 2026-09-02). MSRV 1.92 for `temporalio-sdk`, 1.88 for `temporalio-client`. [V: https://crates.io/api/v1/crates/temporalio-sdk , https://crates.io/api/v1/crates/temporalio-client , https://github.com/temporalio/sdk-rust/releases , `src-tauri/Cargo.toml:140-145`]
- Licence: MIT (crates.io shows "non-standard" because the crate ships a licence file; the file is the MIT licence). [V: https://github.com/temporalio/sdk-rust/blob/main/LICENSE.txt]
- Maintenance: last commit 2026-10-02; contributors Sushisource 484, chris-olszewski 118, yuandrew 37, bergundy 37; 60 open / 379 closed issues. [V: https://github.com/temporalio/sdk-rust]
- Sidecar: the `temporal` CLI ("Temporal command-line interface and development server") is MIT, v1.9.1 released 2026-09-14; archives are 45.3 MB (linux amd64) and 43.2 MB (darwin arm64) compressed. `temporal server start-dev --db-filename <path>` persists state to a file. The Temporal server itself is MIT. [V: https://github.com/temporalio/cli , https://github.com/temporalio/cli/releases/tag/v1.9.1 , `internal/temporalcli/commands.yaml` in that repo, https://github.com/temporalio/temporal]
- This is the only option that keeps the existing ~18k lines of workflow code, at the cost of shipping and supervising a ~45 MB (compressed) server process and a local gRPC port. [I] Whether Temporal supports the dev server for production use is not stated in the sources I read. [not verified]

### DBOS

- A Rust library now exists: crate `dbos` 0.5.0 (2026-09-09), first published 2026-07-13, MIT, MSRV 1.95, 3 versions, 1,550 total downloads; repo `dbos-inc/dbos-transact-rust`, last commit 2026-10-02, devhawk 104 commits, 26 open / 4 closed issues. [V: https://crates.io/api/v1/crates/dbos , https://github.com/dbos-inc/dbos-transact-rust]
- Storage: "lightweight durable workflows built on top of Postgres ... no ... external dependencies except Postgres"; depends on `sqlx ^0.9` with `postgres`; the only system-database implementation file is `sysdb/postgres.rs`. [V: README; https://crates.io/api/v1/crates/dbos/0.5.0/dependencies ; code search for "sqlite" in the repo] Excluded: Postgres only.

### flawless

- Crate `flawless` 1.0.0-beta.3, published 2024-12-09; no releases in 12 months; licence BSD-2-Clause-Patent; no repository URL on crates.io; `flawless-run/flawless` does not exist on GitHub. [V: https://crates.io/api/v1/crates/flawless ; GitHub API 404] The crate is a "toolkit for writing durable execution workflows" that targets a separate flawless server. [V: crate description] I could not confirm the server's licence, storage or current status from flawless.dev (the page loaded but contained no licence statement I could extract). [not verified] Treat as stale. [I]

### Golem

- `golem-rust` 2.1.0 stable, 3.0.0-rc2 on 2026-10-02; Apache-2.0; 19 releases in 12 months. [V: https://crates.io/api/v1/crates/golem-rust]
- Server: **Business Source License 1.1** (licensor Golem Cloud, Inc.). v1.5.10 (2026-08-24) single-binary assets are 141 MB (aarch64 macOS) to 180 MB (x86_64 Linux), uncompressed executables. [V: https://github.com/golemcloud/golem/blob/main/LICENSE , https://github.com/golemcloud/golem/releases/tag/v1.5.10] Excluded by licence and size; it is a WASM agent platform rather than a job library. [I]

### Others found by searching crates.io

Searches run against `https://crates.io/api/v1/crates?q=...&sort=recent-downloads` for "durable execution", "durable workflow", "durable task", "sqlite job queue", "sqlite queue", "task queue sqlite", "background jobs sqlite" and "workflow engine embedded". [V] Beyond the candidates above they surfaced:

- **Resonate**: `resonate-sdk` 0.6.0 (2026-07-22), Apache-2.0, 5 releases, last commit 2026-08-24, 2 open / 5 closed issues. The server is Apache-2.0, "a single binary", contains a `resonate-server-sqlite` crate, and v0.9.8 archives are 10.3–11.2 MB compressed. The SDK's `Resonate::local()` mode is "in-memory, no server required", so durability needs the server. [V: https://crates.io/api/v1/crates/resonate-sdk , https://github.com/resonatehq/resonate-sdk-rs/blob/main/README.md , https://github.com/resonatehq/resonate , https://github.com/resonatehq/resonate/releases/tag/v0.9.8] The only permissive sidecar smaller than Temporal's, but adoption is tiny (2,249 total downloads). [I]
- **ora**: 0.13.0 stable and 1.0.0-rc.3 (2026-09-25), MIT OR Apache-2.0, 13 releases, tamasfe 109 commits, 3 stars, no README in the repository root. Depends on tonic and has an optional `ora-server` dependency. [V: https://crates.io/api/v1/crates/ora , https://github.com/tamasfe/ora] Storage backends and features not verified.
- **zizq**: Rust client 0.7.0 MIT; the server licence is BSL 1.1 with a non-commercial additional use grant. [V: https://crates.io/api/v1/crates/zizq , https://github.com/zizq-labs/zizq/blob/main/LICENSE] Excluded.
- **wfaas** 1.1.0 (Apache-2.0, 860,921 recent downloads) and **a3s-flow** 1.1.0 (MIT): both describe themselves as workflow engines; wfaas has no storage dependency in its manifest. [V: https://crates.io/api/v1/crates/wfaas , https://crates.io/api/v1/crates/a3s-flow] Not examined further; wfaas appears to be in-memory. [I]
- **aide-de-camp-sqlite** 0.2.0 (2022-12-18), **gaffer** 0.2.0 (2021), **backie** 0.9.0 (2023, Postgres), **hammerwork** 1.15.5 (2025-08-29), **liteq** 1.1.4 (162 total downloads), **apalis-libsql** 0.1.0 (41 total downloads): stale or negligible adoption. [V: crates.io API for each name]
- `queued` is SSPL-1.0; `pgmq`, `pgboss`, `faktory`, `sidekiq`, `hatchet-sdk`, `inngest` need an external server or Postgres. [V: crates.io API licence and description fields] Excluded.

## 4. The existing custom code and what a library would replace

Characterisation only; this was not an audit.

- `src-tauri/src/render_pipeline/project_export/admission.rs` (1,615 lines) plus `admission_retention.rs` (429 lines) and three test files (1,477 lines). [V: `wc -l`] The module header describes it as "Durable local render admission. Editor ownership covers validation and the queued write; workers own immutable inputs, cancellation and artifact leases after admission returns." [V: `admission.rs:1-2`]
- Storage is plain files in the project folder: one attempt directory per `(job_id, attempt_id)` named by a SHA-256 digest, records written with `O_EXCL`/`O_NOFOLLOW`, `sync_all`, `renameat` and a parent-directory sync, and a `flock`-ed pin file that marks a live worker. [V: `admission.rs:89-91, 186, 387-414, 587-650`]
- Capacity: `ADMITTED_WORKER_LIMIT = 4` plus an in-flight serialized-byte budget (`MAX_IN_FLIGHT_BYTES` 256 MiB, `MAX_INPUT_BYTES` 64 MiB, `MAX_ATTEMPTS` 1,024); over-capacity returns a `busy` error instead of queueing. A replayed identical request "does not consume another slot". [V: `admission.rs:513-548, 948`; `admission_retention.rs:6-10`] The reserved cancellation capacity lives in the web host RPC limiter, not in this module. [V: `web_host/idempotency.rs:98`, `web_host/request_outcome_tests.rs:300`]
- Recovery: `recover_media_render_attempt` checks the pin; if no worker holds it and the job is still active, it marks the canonical project job `Failed` ("Render stopped when the host closed. Retry the render.") through a `ProjectAction`. Status polling never mutates jobs. [V: `admission.rs:1340-1375`]
- Temporal side: `src-tauri/src/workflows/` is 18,066 lines in total (the brief said about 17k), with six workflows (generate media, export NLE XML, render draft, transcribe media, Codex edit, export media) and roughly 29 activities behind the `temporal-worker` feature. [V: `wc -l`; attribute grep of `workflows/mod.rs`] `src/editor/services/speech-service.ts` starts transcription only through `startTemporalWorkflow`. [V: that file, lines 6-20, 44]

What a library **would** replace: the queue table, the slot limit, retry bookkeeping, dedup-by-key on insert, heartbeat/orphan detection, and (for duroxide or Temporal) step history and progress status. [I]

What a library **would not** replace: [I]

- Project-folder locality. Every embedded candidate wants one SQLite database owned by the worker runtime. Keeping job state inside each project folder means either one database per project (workers started per open project, WAL files in the project folder) or moving job state to app data and losing the property that a project carries its own job history.
- The hardened file handling and package-identity checks (`PackageIdentity`, `O_NOFOLLOW`, pin files). A library writing SQLite does none of this.
- The canonical job status in the project model (`ProjectAction::UpdateJobStatus`) and the rule that a terminal canonical state wins over a late worker result.
- Byte budgets, retention and terminal receipts.
- Actual cancellation of GStreamer/GES pipelines, sherpa-onnx helper processes and provider HTTP polling. All candidates offer a cooperative flag at best; the kill path stays ours.
- The reserved cancellation lane in the RPC limiter.
- "Reject when busy" semantics: queue libraries queue; they do not refuse.

## 5. Ranked recommendation

All ranking is my judgement. [I]

1. **Keep custom (explicit option, and my first choice for render/export).** The admission module already meets every stated requirement with zero new dependencies and state in the project folder. The gap is not durability; it is that transcription, generation, Codex edit and NLE export only have a Temporal path. Generalising the existing admission pattern (attempt directory, pin, capacity permit, recovery function) to those job kinds avoids a new storage engine and keeps one recovery model.
2. **apalis 1.0.0-rc.10 + apalis-sqlite + apalis-workflow** — top library choice. Permissive, in-process, embedded SQLite, idempotency key enforced by a unique index, orphan re-enqueue, stepped and DAG workflows, concurrency limit, the widest adoption of any qualifying candidate (340,904 recent downloads). Weaknesses: still RC after a year, one maintainer, no progress API, orphan recovery re-runs rather than fails, no current benchmarks.
3. **duroxide 0.1.30** — closest functional match to what Temporal gives us (replay, durable cancel with grace period, custom status for progress, duplicate instance IDs rejected) and embeddable with bundled SQLite. Ranked below apalis because it is self-declared preview, ten months old, 24 open against 6 closed issues, and imposes deterministic-orchestration rules that 1–3 step local jobs do not need.
4. **Temporal SDK 1.0.0 with a bundled `temporal` dev server** — only worth it if preserving the existing workflow code is the priority. Licence is fine (MIT), but it adds a ~45 MB compressed Go sidecar per platform, a local gRPC listener, process supervision, and an SDK upgrade across four releases (0.5.0 to 0.6.0, 0.7.0, 0.8.0, 1.0.0).

Not recommended: effectum (unmaintained), fang (qualifies, but offers less than apalis with lower activity), Resonate (permissive and small, but a sidecar with negligible adoption), everything Postgres-only, and everything AGPL or BSL.

### Migration-size estimate for the top library choice (apalis)

Rough, from reading module outlines only; I did not read `workflows/mod.rs` in depth. [I]

- **New:** a job-runtime module (suggested `src-tauri/src/jobs/`) owning the `SqlitePool`, migrations (`SqliteStorage::setup`), worker startup/shutdown on the tokio runtime, an idempotency-key derivation from the request, a lookup-on-conflict path to return the original admission, a progress channel, and a startup pass that converts orphaned `Running` rows into failed canonical jobs if we keep "fail cleanly" rather than "re-run". Order of 1,500–2,500 lines with tests.
- **Changed:** `src-tauri/Cargo.toml` (add `apalis`, `apalis-sqlite`, `apalis-workflow`, transitively `sqlx 0.9` and bundled SQLite; drop six `temporalio-*` crates and the `temporal-worker` feature); the six workflows and ~29 activities in `src-tauri/src/workflows/mod.rs` re-expressed as task handlers or `SteppedFlow` steps, with the Temporal client, worker and `temporal_reconcile` code deleted; `src/editor/services/speech-service.ts` and `src/lib/jobs/temporal-fallback` on the frontend; the Tauri/web-host commands that start and poll workflows. Net effect is likely a large deletion (most of 18k lines) and a few thousand lines of ported activity bodies.
- **Unchanged if we follow option 1 for renders:** `render_pipeline/project_export/admission*.rs`. Replacing it as well would additionally require re-deciding where the database lives relative to the project folder.
- **Tests that must be re-proven:** exactly-once admission and identical-request replay (`admission_tests.rs`); the four-slot limit and no-slot-on-replay (`admission_capacity_tests.rs`); byte budgets and retention (`admission_retention_tests.rs`); the reserved cancellation lane (`web_host/request_outcome_tests.rs`, `web_host/rpc_limiter_tests.rs`); crash-mid-job recovery to a terminal failed state; prompt cancellation of a running GES render and of a helper process; reconciliation behaviour now covered by `workflows/temporal_reconcile/tests.rs` and `workflows/bundle_publication_tests.rs`; packaged-build smoke for transcription, which has no non-Temporal path today.
- **Licence and packaging checks to add:** SQLite (public domain) and sqlx (MIT OR Apache-2.0, not re-verified here) into the third-party notices; binary-size delta measured on both platforms.

## 6. Not verified

- flawless: server licence, storage, repository location and whether the project is still active.
- background-jobs: repository activity, storage backends and features (git.asonix.dog not fetched); licence taken from crates.io only.
- ora, wfaas, a3s-flow: storage backends and features.
- Restate and Golem server storage engines and whether either can run with a purely local embedded store; only licence and size were checked.
- Whether the Temporal `start-dev` server is supported by Temporal for production or end-user distribution.
- Uncompressed on-disk size of the Temporal, Restate, Resonate and Obelisk binaries (only archive sizes were read; Golem's assets are raw executables).
- apalis: what happens when an orphaned job has exhausted `max_attempts`; whether a conflicting idempotent push can return the existing task; per-step crash semantics of `apalis-workflow` (only the README claim "durable and resumable" was read); absence of a progress API is based on a source grep, not on documentation.
- duroxide: behaviour of an activity that was running at crash time (I assume it is re-dispatched after the lock timeout); the duplicate-instance rule comes from one doc comment; whether the `runtime-tokio-native-tls` sqlx feature actually links OpenSSL on Linux.
- That sqlx's `sqlite` feature statically bundles SQLite in 0.9 (assumed from the feature name; sqlx's README was not fetched). duroxide and effectum bundling was verified in their manifests.
- Any performance comparison between candidates: no project publishes comparable benchmarks. The only figures found are duroxide's own stress results from November 2025.
- Open-issue *content* for all candidates: only open/closed counts were collected; no issues were read.
- Compile-time and binary-size impact of any candidate on this app: no builds were run.
- MSRV of apalis-sqlite, duroxide, effectum, underway, sqlxmq and graphile_worker: no `rust-version` is declared on crates.io for those crates (apalis's workspace declares 1.85).
- The migration-size numbers in section 5 are estimates, not measurements.
