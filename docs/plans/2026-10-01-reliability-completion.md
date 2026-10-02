# Reliability completion implementation plan

> **For agentic workers:** Use subagent-driven development for independent bounded
> slices and native execution for the primary-owned RPC integration. Maintain TASKS.md.

**Goal:** Implement the remaining locally executable reliability follow-ups approved
in the conversation, then verify and review them without claiming unavailable platforms.

**Architecture:** Keep Rust canonical state, current editor gates and cross-process
lock ordering. Add durable credential-free mutation outcome receipts beside host session
state, bound retained histories before accepting new work, and publish owned exports
through retained directory descriptors. Preserve recovery rather than guessing failure.

**Tech stack:** Existing Rust/libc/serde/atomic file persistence, React/TypeScript,
repository Vitest/Playwright and native GStreamer integration tests; no new service.

**Spec:** This document plus
`docs/development/architecture-risk-followups-2026-10-01.md` and the user's explicit
instruction to implement all follow-ups and continue. The user selected autonomous
execution and Sol/Luna delegation; no additional design approval is requested.

## Global constraints

- Original request identity survives uncertainty; no automatic replay under a new ID.
- Success followed by media failure stays committed. Stale revisions reject atomically.
- Authorization, CSRF, takeover, cancellation bypass, history and EDL-first validation remain.
- No accepted edit/outcome/active artifact is evicted to satisfy a limit.
- Format changes are explicit, additive and versioned; old canonical project files remain valid.
- No push, merge, production/external CI change, paid call or private upload.
- Every shell starts with `rtk`; use focused RED→GREEN, then applicable broader gates.

## Review focus

1. Host dies between canonical mutation commit and outcome publication: retain uncertain
   receipt, distinguish interruption from failure and prohibit blind replay.
2. New session/tab or cleared client metadata: authorize server recovery by stable trusted
   identity; never expose another user's outcomes or persist raw credentials/RPC bodies.
3. Retention pressure: reserve before acceptance, preserve current leases/permits and
   retain replay receipts when compacting terminal history.
4. Parent/stage/destination replacement: descriptor-relative ownership and quarantine
   must preserve replacement files and reject unsupported safe publication.
5. Actual encoding spans editor expiry: advancing native progress/output evidence plus
   renewal, editing, cancellation, takeover and restart recovery; bound process cleanup.

## Task 1 — Native encoding and interruption evidence

Owner: Sol `long_encode_recovery`; files `src-tauri/tests/web_host_render_followups.rs`
and new helpers/process tests under `tests/web_host_render_followups/`.

- [x] Require >31 seconds of actual progressing encode position/output under authenticated
  host operations, using synthetic media and one inherited CPU; no artificial encode delays.
- [x] Kill a progressing worker process, restart the same package, read interrupted state,
  explicitly recover under editor ownership, reject duplicate/superseded replay and render
  a subsequent attempt with valid streams. Kill/reap children on every failure.
- [x] Run focused native feature tests, then actual-host browser gates after integration.

## Task 2 — Filesystem-owned export publication and recovery

Owner: Sol `filesystem_ownership`; `project/export_destination{.rs,/prepared.rs}`
and focused tests/new siblings. Primary adapts `project_export/named_export.rs`.

- [x] Reproduce moved-parent and process-exit stage leaks before implementation.
- [x] Use retained directory descriptors, exclusive private stages, durable owner metadata
  and advisory activity locks. Publish no-clobber, quarantine before conditional cleanup,
  restore replacements safely or preserve forensic evidence. Recover only verified dead stages.
- [x] Expose `PreparedExportOutput::publication_owner()` and retained owner rollback;
  preserve existing materialized path/wire fields. Fail closed on unsupported platforms.
- [x] Cover source/stage/parent swaps, competing names, cancellation, dead/live recovery,
  link/copy parity and owned rollback with focused tests and real named exports.

## Task 3 — Render and session retention contracts

Owner: Sol `retention_contract`; `project_export/admission.rs`, new retention modules/tests,
`web_host/session.rs` and session tests. Primary owns limiter/cache changes.

- [x] RED: reject input/package limits before writes, preserve replay at capacity, release
  byte reservations on error/panic and preserve session state when persistence fails.
- [x] Logical input bytes: 64 MiB each, 256 MiB across admitted workers. Durable package
  metadata: 512 MiB / 1,024 entries. Reject only new unaccepted work with typed Busy.
- [x] Compact owned terminal input/results into versioned receipts retaining package,
  attempt, fingerprint and outcome. Read protocol-1 files. Never compact other active,
  pinned, superseded or unrecovered attempts; prune only verified terminal pins under lease.
- [x] Sessions: 1,024 live / 2 MiB, transactional issue/revoke/rotate/CSRF changes,
  prune expired/revoked entries while preserving exact expiry and valid credentials.
- [x] Limiter: 2,048 entries, prune expired idle windows only, retain active permits.
- [x] RPC response cache: 64 MiB total / 8 MiB per response; retain bounded replay receipts
  rather than reexecute after an oversized response; durable journal guards aged eviction.

## Task 4 — Durable mutation outcomes and recovery integration

Primary ownership: new `web_host/request_outcome*` modules/tests, `rpc.rs`, `idempotency.rs`,
`http.rs`, `mod.rs`, host contract tests and remote transport/outcome/UI integration.

- [x] RED: execute a mutation, recreate the engine/store, recover known outcome without
  dispatch; same-ID different fingerprint rejects. Interrupted pending stays uncertain.
- [x] Add a bounded, versioned, atomic local journal keyed by hashed trusted identity plus
  original request ID. Reserve/fsync pending before dispatch; persist compact terminal
  metadata before reporting a durable outcome. Never persist raw body/lease/CSRF credentials.
- [x] A process-lifetime journal owner lock prevents a second host from treating running
  operations as interrupted. On real restart pending becomes interrupted, never failed.
- [x] Expose authenticated outcome lookup/list/acknowledgement through existing HTTP
  identity/session/CSRF boundaries; known creation identifies its accepted catalog project.
- [x] Timestamp new client request identities. Retention rejects expired new-format requests
  instead of treating aged records as new; old identities keep explicit receipt compatibility.
- [x] Bound compact records and bytes; pressure rejects new unaccepted mutations. Retain
  unknown/interrupted and legacy identities rather than silently evicting them.
- [x] Client restores server-side outstanding guards after reconnect/new tab, resolves known
  outcomes by original identity, and reads canonical state before resuming project editing.
  Creation resumes only on known terminal outcome. No guessed failure or automatic mutation retry.
- [x] Cover lost ACK, restart, expiry, conflict, concurrent/replayed calls, principal isolation,
  corrupt/unavailable journal, media failure and stale reconciliation; actual browser creation/edit flow.

## Task 5 — Platform gates, integration and final review

- [x] Run final frontend unit/type/lint/build/browser/visual gates, portable Rust and host
  contracts, native integration, Clippy host/MCP/packaged Linux profiles, formatting/diff review.
- [x] Inspect local availability for Tailscale Serve, host restart and Mac/device checks.
  Prepare reproducible platform acceptance steps; perform only authorized available local actions.
- [x] Preserve assessment documents/unrelated work, maintain evidence and blockers in TASKS.md,
  commit small Conventional Commit slices, independently review the final source/diff.

## Execution ledger

- Starting state: clean `604a3894` on existing unintegrated feature branch. No new worktree
  is needed for this continuation; preserving the current checkout also preserves assessment files.
- Task 2 RED: owned stage remained after original parent moved; process exit left dead stage.
- Tasks 1–3 delegated with disjoint ownership; Cargo runs are serialized by the primary.
- Platform unavailability/required external authorization cannot be changed by implementation;
  keep those gates explicit rather than claiming everything is operationally verified.
- Final local Linux acceptance: desktop package payload/ELF audit passed; extracted
  provider-free smoke passed 17 checks, followed by 6 native keyboard/menu checks. Both
  runs restored the pre-existing sample and reaped owned processes. Headless-host package
  passed 282 files, all 281 listed hashes and isolated Codex initialization/reap.
- Packages use clean source `7b5775623c26`; the final evidence commit is documentation-only.
  Mac/signing/device and live authorized Serve/deployed restart remain unverified external
  gates. The completion report contains reproduction commands, hashes and explicit deferrals.
