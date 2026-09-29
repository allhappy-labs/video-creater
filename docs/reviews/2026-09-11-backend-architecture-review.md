# Backend architecture review — 2026-09-11

Review team: three GPT-5.6 Sol agents at medium effort for scoped research/checks; one GPT-6 Astra agent at high effort for evidence review and prioritization. No paid provider calls or application source changes.

Commit: `19b59f347304b38f3a0d4ced29a9496613afe02a`. Findings were checked against current source, including callers and existing mitigations. This is a static architecture audit, not proof of runtime failure. No source changes, provider requests, or app-server sessions were made by the synthesis reviewer.

The highest-priority problem is inconsistent enforcement of Rust-owned state: action validation exists, but ordinary persistence still lacks package-level atomicity; whole snapshots additionally bypass validation and conflict detection. Render jobs and subprocesses have similar ownership gaps. Fix these boundaries before splitting large modules or adding more provider integrations.

## Architecture map

- The React editor calls Tauri commands in `src-tauri/src/main.rs`; commands resolve paths and invoke project, render, generation, and Codex modules.
- Canonical state is a split filesystem package. `project/action.rs` validates actions; `project/split.rs` loads and writes manifest, timeline, media, jobs, reports, indexes, and sidecars. Per-project mutation leases serialize participating writers.
- Storage cleanup has a separate process-wide coordinator. Some public writers participate in both coordinators; others participate only in the project coordinator.
- Render/export owns graphics, precompose, encoder, compatibility review, and report publication. Cancellation is an in-memory registry. Generation has a separate, more developed project-scoped cancellation/completion design.
- Codex proposal generation uses a custom stdio app-server transport. MCP exposes a separate local tool dispatcher. Providers perform request/poll/download operations through individual adapters.

## Prioritized findings

Severity describes potential impact; likelihood describes the trigger in ordinary use, not a measured incident rate. Confidence is confidence in the source-level finding. None of these failures were runtime reproduced in this review.

| # | Finding | Severity | Likelihood | Confidence |
|---|---|---|---|---|
| 1 | Dynamic sidecar paths bypass containment validation | High | Low in ordinary UI; reachable with malformed state/IPC | High |
| 2 | Ordinary action and whole-project saves are not package-atomic | High | Medium under interrupted/failed writes | High |
| 3 | Whole snapshots overwrite newer canonical work | High | Medium with background work and undo/Codex saves | High |
| 4 | Render cancellation and recovery use globally ambiguous job IDs | High | Low–medium; requires reused IDs across projects | High |
| 5 | Render cancellation does not arbitrate terminal completion | High | Medium around late cancellation | High |
| 6 | Subprocess supervision can wait indefinitely | Medium–high | Conditional on a hung/noisy child | High for missing safeguards |
| 7 | Codex transport treats request response as completed turn | High if pinned protocol uses asynchronous turn events | Protocol-dependent; not runtime verified | High for client behavior; medium for consequence |
| 8 | Cleanup and canonical writers use different exclusion boundaries | Medium | Low–medium; concurrent cleanup/write required | High for coordination gap |

### 1. Validate the complete model before deriving sidecar paths

`save_split_project_to_folder` accepts a renderer-supplied `VideoProject` and forwards it to the writer without a complete validation gate (`src-tauri/src/main.rs:1393`, `src-tauri/src/main.rs:1406`). The writer joins transcript IDs, template IDs, generated asset IDs, report IDs, job IDs, and export IDs into filenames/directories (`src-tauri/src/project/split.rs:1886`, `:1898`, `:1910`, `:1924`, `:1935`, `:1952`). `write_json_file` creates parents and persists the target (`src-tauri/src/project/split.rs:7583`). The existing write-path containment helper (`src-tauri/src/project/split.rs:1770`) is not applied to these paths.

An absolute or traversal-bearing ID can create/replace JSON outside the package. This is a malformed-state/IPC integrity boundary failure; it is not evidence of a remotely exploitable entry point. Normal UI-generated IDs reduce ordinary likelihood.

**Fix:** Validate all filename-derived IDs as safe single segments before any filesystem write, and enforce containment/symlink checks at every dynamic write. Apply this before staging as well: a transaction alone does not contain unsafe paths. **Regression:** table-test every ID-bearing collection and require rejection before any canonical or outside-package file changes.

### 2. Make action commits and snapshot replacement atomic at the package level

`save_split_project` calls the in-place writer (`src-tauri/src/project/split.rs:1824`). That writer persists manifest, timeline, media, and sidecars in sequence, then deletes stale files (`src-tauri/src/project/split.rs:1879`, `:1981`). Each JSON file is individually persisted atomically, but the package is not. Crucially, normal `apply_project_actions_to_split_project_with_lease` also calls this same writer (`src-tauri/src/project/split.rs:2752`, `:2765`). The staging/journal/backup transaction helper exists at `src-tauri/src/project/split.rs:2227` and is used by settings updates at `:2740`.

An interrupted write, ENOSPC, permission error, or process termination can leave mixed revisions even after all actions validated successfully in memory. Existing pending-transaction recovery does not create a journal for these in-place saves.

**Fix:** Route all canonical replacement through the transaction path; keep the in-place writer private to validated staging. Handle initial package creation through sibling staging/promotion. **Regression:** inject failure after each write/promotion checkpoint and require reopening to yield the complete old or new state.

Action batches are validation-atomic in memory, but they are not currently transactionally persisted. This finding covers ordinary action writes as well as whole snapshots.

### 3. Replace unversioned whole-project saves in live workflows

The whole-save command accepts a snapshot without reloading or comparing a persisted revision (`src-tauri/src/main.rs:1393`). Its writer removes sidecars absent from the supplied snapshot (`src-tauri/src/project/split.rs:1981`). Live callers include undo/redo (`src/components/workspace/editor-workspace.tsx:5046`) and the asynchronous Codex completion path (`src/components/workspace/editor-workspace.tsx:7530`, `:7537`). Background generation refreshes canonical state independently (`src/components/workspace/editor-workspace.tsx:5017`).

If a worker/import/action commits after the caller captured its snapshot, a later serialized save can erase the newer jobs, assets, reports, or edits. Mutual exclusion orders writes; it does not detect obsolete input. Making the save transactional alone would merely make this obsolete replacement atomic.

**Fix:** Use narrow actions for Codex metadata and revision-aware history operations; otherwise require a persisted revision and compare-and-swap. **Regression:** commit B after capturing A, submit stale A, and require a conflict with B intact.

### 4. Scope render identity to project and execution attempt

The render registry is keyed only by `String job_id` (`src-tauri/src/render_pipeline/cancel.rs:23`, `:36`, `:50`). The cancel command resolves a project root but signals only by ID (`src-tauri/src/main.rs:3571`). Recovery uses the same ambiguous active check (`src-tauri/src/render_pipeline/project_export.rs:324`). IDs are supplied at IPC boundaries (`src-tauri/src/main.rs:3237`, `:3282`).

Project A and project B can both contain `render-1`: cancelling A can signal B, and an active B can suppress interrupted-job recovery in A. The global storage lock normally serializes renders, so this does not require or demonstrate two simultaneous renders. It requires a wrong-project cancellation or recovery lookup while one is active.

**Fix:** Use canonical project identity plus job ID and execution-attempt identity everywhere; reject duplicate active registration. **Regression:** same job ID in two roots must produce isolated cancellation and recovery results.

### 5. Make cancellation and completion competing lifecycle outcomes

The cancel command ignores whether an active token was found and always writes `Cancelled` (`src-tauri/src/main.rs:3577`). It does not validate job kind or cancellable state. `update_job_status` prevents transitions away from `Cancelled` but permits `Completed -> Cancelled` and `Failed -> Cancelled` (`src-tauri/src/project/action.rs:5845`, `:5855`). Graphics rendering receives no cancellation token (`src-tauri/src/render_pipeline/project_export.rs:1342`), nor does post-encoder review extraction (`:1473`), and completion/report publication has no final cancellation claim (`:1558`).

A late cancellation can rewrite a completed export's history. During graphics/review, work can continue and publish success while the cancel command waits for the project lease, then change only the final job status. Exact latency and artifact behavior need runtime tests.

**Fix:** Model an attempt's state explicitly; cancellation and completion must atomically claim a terminal outcome, and publication must follow that outcome. Propagate cancellation through graphics/review. Keep cancellation signalling out-of-band: **do not acquire the project's long-held render lease before signalling**, or cancellation will wait for the work it must stop. Validate active identity/state through the registry and reconcile persistence under the worker's lease. **Regression:** cancel before/after completion and within controllable slow graphics/review stages; require one consistent terminal outcome and no accidental cancellation of another job kind.

### 6. Share a bounded process supervisor across workers and app-server

Compatibility workers pipe stdout/stderr, poll for exit, then read output (`src-tauri/src/precompose/compatibility.rs:351`, `:401`, `:422`). The 300-second budget is passed to the child (`:388`); there is no independent parent deadline. Post-render extraction does not pass the render token. Separately, app-server stderr is piped but never drained (`src-tauri/src/codex/app_server.rs:164`), and stdout requests have no deadline (`:191`).

A hung child can hold export completion and the project lease indefinitely. A child that fills an undrained pipe can deadlock. Normal compatibility output is small, so pipe saturation is conditional, not a demonstrated routine failure; the missing parent deadline is independently concrete.

**Fix:** Continuously drain capped output, retain a bounded diagnostic tail, enforce parent deadlines and cancellation, and terminate/reap children and relevant descendants on every exit path. **Regression:** fake children that hang, fill stderr/stdout, or ignore graceful shutdown must produce bounded completion and no orphan process.

### 7. Implement the actual app-server turn lifecycle

`send_request` discards messages whose IDs do not match the current outgoing request (`src-tauri/src/codex/app_server.rs:207`, `:222`). `start_codex_video_edit_turn` immediately parses the `turn/start` response as the proposal (`src-tauri/src/codex/app_server.rs:309`). There is no retained notification stream, terminal-turn wait, or server-request responder; nevertheless requests use `approvalPolicy: on-request` (`src-tauri/src/codex/app_server.rs:453`, `:489`). The UI exposes this path and reports no proposal when extraction fails (`src/components/workspace/editor-workspace.tsx:7530`, `:7543`).

If the bundled protocol acknowledges turn start and delivers final output later, the proposal is lost; server-initiated requests can also leave the session waiting. The package pins Codex 0.141.0 (`package.json:97`, `scripts/build-codex-sidecar.mjs:28`), but this review did not execute that binary or inspect its generated protocol schema. Treat the consequence as a strong protocol compatibility concern, not a reproduced production outage. The configured `--stdio` invocation also needs verification against that pinned binary.

**Fix:** Test the pinned binary's handshake/launch contract, then implement a message pump distinguishing responses, events, and server requests; wait for terminal completion and gather structured output. Support cancellation/deadlines via finding 6. **Regression:** interleave acknowledgements, output notifications, server requests, and terminal completion; test against the pinned runtime without paid generation where possible.

### 8. Put cleanup and project writes under one ownership contract

Cleanup holds the process-wide storage coordinator across revalidation/deletion (`src-tauri/src/settings/storage.rs:24`; `src-tauri/src/main.rs:4335`). Ordinary action IPC uses only the per-project lease (`src-tauri/src/main.rs:2052`; `src-tauri/src/project/split.rs:2744`); cleanup does not acquire that lease. These locks cannot exclude each other's operations.

A concurrent package rewrite can race cleanup's quarantines/removals, leading to failed saves or inconsistent artifact references. Cleanup does revalidate fingerprints and quarantine identity, which narrows the claim: this audit does not establish arbitrary deletion or guaranteed deletion of changed content.

**Fix:** Centralize artifact/package mutation ownership and consistent lock ordering, including external worker/MCP processes where applicable. Prefer a project-scoped coordinator over globally serializing unrelated long jobs. **Regression:** pause cleanup after revalidation, attempt a conflicting writer, and assert deterministic serialization and consistent files/metadata.

## Supplemental findings and claims not promoted

- **Unbounded provider downloads are real hardening work.** Streaming paths have no cumulative limit, while some adapters/retry buffer complete responses (`src-tauri/src/generation/fal.rs:569`; `src-tauri/src/generation/replicate.rs:893`; `src-tauri/src/generation/google.rs:923`; `src-tauri/src/main.rs:2206`). An oversized response can exhaust disk or memory. Add media-appropriate byte ceilings and streaming `.part` files to the shared I/O boundary. Lower priority here because no oversized response was observed.
- **MCP project scope is ambiguous, not an established high-severity security exploit.** The launched root supplies read context (`src-tauri/src/codex/mcp_server.rs:293`), while tools accept another `projectDir` and can write it (`src-tauri/src/codex/tools.rs:12770`, `:20388`). The design explicitly exposes a broad local filesystem control plane. Bind writes to server-owned project context if one-project capability is intended; do not claim an authorization bypass without establishing that contract. Context/mutation mismatch remains a worthwhile API correction.
- **Workspace-write is weaker than proposal-only intent, but scope was overstated.** Its cwd is the app/skill root, not necessarily the media project (`src-tauri/src/main.rs:5712`, `:5730`; `src-tauri/src/codex/app_server.rs:463`). Arbitrary root selection also requires loading the mandatory skill bundle. Do not claim every media project is writable by default. Prefer read-only proposal sessions plus a dedicated scratch directory if tools need writes.
- **Retry URL policy merits hardening, not an unqualified high-severity SSRF label.** Persisted project data can cause a user-triggered GET to any HTTP(S) URL (`src-tauri/src/main.rs:2136`, `:2182`). That can reach local/private services and follows default redirects, but no attacker-controlled readback/exfiltration or authenticated sensitive request was demonstrated. Bind URLs to provider provenance and validate redirects; assess custom/self-hosted provider requirements before imposing a blanket hostname allowlist.
- **Search freshness is conditional.** Same-count/same-timestamp external sidecar edits can evade freshness (`src-tauri/src/search/mod.rs:809`). Successful ordinary saves rebuild the index, so this is not evidence of normal action-induced stale search. A canonical content revision from finding 3 can address it.
- No evidence supports claiming macOS playback, packaged runtime, signing, physical-device, or production verification. Do not call this audit comprehensive or infer absence of other defects.

## Three architectural improvements and fix order

1. **One canonical project mutation service.** Give callers validated actions, revision-aware replacements, transactional persistence, and artifact ownership through one API; make unsafe raw writers private. Implement path validation first (#1), then package atomicity (#2), stale-write prevention (#3), and shared cleanup coordination (#8). Retain small in-memory action tests, but add failure-injection and two-writer integration tests at this boundary.
2. **One scoped job/attempt lifecycle.** Unify render identity, cancellation claims, terminal states, restart recovery, and artifact publication. Use generation's existing scoped cancellation/completion design as a reference rather than adding another registry. Fix #4 and #5 together, preserving immediate out-of-band cancellation and short canonical commit sections.
3. **One supervised external-I/O boundary.** Share subprocess deadlines, output drainage/caps, shutdown/reaping, and diagnostics; layer the app-server protocol pump on it. Fix #6 before validating #7 against the pinned runtime. Then consolidate provider streaming/byte limits and explicit project/proposal capabilities. Keep provider-specific request schemas inside adapters rather than forcing a large generic provider abstraction.

## Verification record

- Static source/caller verification completed for the findings above; no destructive reproduction was performed.
- `node scripts/source-quality-policy.mjs` and `node scripts/check-tooling-source.mjs` passed, including 86 tooling files. These checks do not validate backend runtime semantics.
- Pipeline reviewer attempted `timeout 45s cargo test --manifest-path src-tauri/Cargo.toml --offline --no-default-features --lib generation::cancel::tests -- --test-threads=1`. It exited 124 during cold dependency compilation. **No backend test pass or failure was obtained.** No additional expensive build was attempted.
- This Ubuntu VM provides no macOS/GStreamer/AVFoundation packaged-runtime or physical-device verification. Protocol compatibility, cancellation latency, process cleanup, and recovery behavior remain runtime-unverified.
