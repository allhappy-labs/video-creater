# Gap Closure 01 — Responsive Backend and Job Reliability Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.
>
> **Detail level:** task-level with bite-sized TDD steps. Each task names the files it owns, the exact test commands, and its commit message.

**Goal:** Close three gaps from the editor redesign on Linux:

- **VC-027:** no Tauri command that reads or writes a project, or that waits on the storage or project lease, runs on the main thread. Commands that write the project run through one per-project FIFO executor, so edit order is preserved while a render holds the lease.
- **VC-028:** unfinished Temporal jobs are reconciled with their workflows. A start that fails fails its job at once. A closed or missing workflow fails its job with a plain reason. An unreachable server leaves jobs untouched and says "Workflow service unreachable".
- **VC-021:** running renders (in-process and Temporal worker) and xAI video generations report progress from 0 to 1, at most about twice a second. The Background tasks row and pill show it.

**Architecture:**

- **Project command queue (lib, new).** `video_creater_lib::project::command_queue` keys a FIFO per canonical project directory. Each key gets one worker thread, which exits once its queue is empty. `submit` enqueues synchronously and returns a future (`QueuedProjectCommand<T>`) that the async Tauri command awaits. Work keeps taking the storage and project leases inside its closure, so a write waits behind a render on the worker thread and never on the main thread. A panic in one closure fails that command only. A nested submit for the same key from its own worker runs inline, so it can't deadlock.
- **Command threading (main.rs).** Project-writing commands become `async` and go through the queue (`run_project_write_command`). Commands that read the project, decode media, reach the network, or take the storage lease use the existing `run_blocking_command`. Renders stay on `run_blocking_command` and never enter the queue. A source audit test fails if a synchronous `#[tauri::command]` in `main.rs` reaches a lease or project I/O.
- **Job progress (lib, new).** `video_creater_lib::project::job_progress` writes lease-free progress snapshots to `logs/job-progress/<jobId>.json`, atomically and throttled to 500 ms. `load_job_progress_from_split_project_folder` reads them without any lease. Render attempts own a `JobProgressReporter` (`render_pipeline::cancel`). The GES bus loop reports position over duration, scaled to 0–0.95. The attempt guard removes the file when the render ends. The editor joins the snapshots onto job task records.
- **Job failure reason.** `JobSummary.failure_reason` (TS `ProjectJobSummary.failureReason`) is set by a new bookkeeping action, `recordJobFailure`. It fails a job only while that job is still unfinished. `updateJobStatus` to any non-failed status clears the reason.
- **Temporal reconciliation (lib, new).** `video_creater_lib::workflows::temporal_reconcile` does three things:
  - A pure planner maps each candidate job and its `WorkflowObservation` to a failure reason or no change.
  - A `TemporalWorkflowDescriber` trait has a fake for tests and a real `temporalio_client` implementation.
  - The async command `reconcile_temporal_jobs_in_split_project_folder` describes the workflows first, then re-plans against a freshly loaded project inside the project command queue.
- **Editor.** The jobs slice polls progress every 500 ms while a running task exists. The progress poll is separate from the folder reload, which blocks for the whole render. The slice reconciles Temporal jobs when the editor opens and every 30 s while a Temporal-backed task is active. The export, speech and generation services fail a job whose Temporal start fails.

**Tech Stack:** Rust 1.87, Tauri 2.11 (async commands, `tauri::async_runtime`), `temporalio-client` 0.4.0, `gstreamer` 0.25 / GES, React 19, Zustand, Vitest, Playwright, Docker (Temporal dev server).

**Spec:** `docs/superpowers/specs/2026-09-16-editor-redesign-gap-closure-design.md`, Decisions 2, 4, 5, 9, 10 and 11, and the architecture notes "Job progress" and "FIFO executor".
**Backlog rows:** VC-021, VC-027, VC-028 in `docs/product-backlog.md`.
**Depends on:** nothing in 02–06. Workstream 06 later records the native Linux evidence for this plan's Temporal steps.

## Design refinements (recorded for review)

These refine the approved Decisions after reading the code. None of them changes the user-visible outcome.

1. **Progress doesn't travel through the job-status update path.** The approved note said emitters update the job through that path. Two facts rule it out:
   - A render holds the process-wide storage lease (`settings::storage::acquire_storage_mutation_lease`) and the cross-process project lease (`project::mutation::acquire_split_project_mutation_lease`) for its whole duration. See `render_media_to_split_project_folder_for_timeline` in `src-tauri/src/render_pipeline/project_export.rs`. `load_split_project` also takes the project lease (`project/split.rs`). The editor's folder reload therefore returns only after the render ends, whether the render runs in the app or in the Temporal worker.
   - Every `apply_project_actions_to_split_project` call advances `contentRevision` and runs a metadata transaction. Writing progress twice a second would make the editor's `expectedRevision` saves conflict constantly.

   Progress is therefore a lease-free snapshot joined onto job summaries in the editor. It is never stored in `job.json` and never enters undo history. The field the user sees is still `TaskRecord.progress`.
2. **A failed job's reason is persisted with a bookkeeping action,** `recordJobFailure`, and not with a new `updateJobStatus` field. `updateJobStatus` is part of the strict Codex action schema (`update_job_status_action_schema` in `codex/app_server.rs`) and appears in about 55 places across the Rust sources and tests. Following the `updateJobProviderRequest` precedent, the new action isn't offered to Codex or MCP. It is classified as a jobs change in `codex/conversation/risk.rs`, and it is non-undoable in the editor.
3. **The queue's scope.** Only commands whose work is mainly a canonical project write enter the FIFO queue. Commands dominated by decoding, network or reads use the blocking pool. They still take the lease around their own writes, and the lease serializes those writes. `cancel_render_job_in_split_project_folder` requests cancellation before it queues the status write. Queued behind the render, the cancellation would never reach it.
4. **Progress sources.**
   - **Transcription:** the speech worker (`crates/speech-worker/src/protocol.rs`) answers with one JSON line and has no progress channel, so transcription stays indeterminate.
   - **Providers:** only xAI video reports numeric progress (`"progress": 0..100` in `tests/xai_generation_provider.rs`). The fal, Replicate, Google, MiniMax, OpenAI and ElevenLabs status payloads decoded today carry none.
5. **Workflow status mapping.** `Running`, `Paused`, `ContinuedAsNew` and `Unspecified` leave the job alone. `Completed`, `Failed`, `Canceled`, `Terminated`, `TimedOut` and not-found fail it, each with its own reason (Task 5). A cancelled workflow becomes a failed job whose reason says it was cancelled.

## Global Constraints

- **Shell.** Prefix every shell command with `rtk`. `rg` isn't installed, so use `rtk grep -rn`. Run long commands (cargo, Playwright, `verify:frontend`) in the foreground.
- **Commits.**
  - Use Conventional Commits, with the message given in each task.
  - End every message with the trailer `Co-Authored-By: Claude Opus 5 (1M context) <noreply@anthropic.com>`.
  - Stage only the files the task names (`rtk git add <paths>`), never `-A`.
  - Never push.
- **Size.** New files stay under 600 lines. Editor files (`src/editor/**`) must stay under 600 lines (`scripts/editor-source-policy.test.ts`). `src/editor/services/generation-service.ts` is already 591 lines, see Task 10. Existing oversized files (`src-tauri/src/main.rs`, `src/lib/project.ts`, `project/action.rs`, `workflows/mod.rs`) only get thin wiring. Logic goes in new modules.
- **UI styling.** Theme tokens only: no hex colors, and no `white/` or `black/` opacity fragments.
- **Rust/TS lockstep.** A new project action lands in one commit with all of these:
  - the Rust variant and its apply function,
  - the TS `ProjectAction` union member and its `applyProjectActionLocally` case,
  - the editor's non-undoable set,
  - the risk classification in `codex/conversation/risk.rs` and in the fixture `conversation-fixture-proposals.ts`.

  User edit actions also update the Codex schema and MCP support. `recordJobFailure` is bookkeeping and follows the `updateJobProviderRequest` precedent: it is not offered to Codex or MCP.
- **Licenses.** LGPL GStreamer, GES and WebKitGTK are fine. Nothing GPL may be linked into or loaded by the app. The Temporal dev server is test-only tooling, either the MIT `temporal` CLI or the `temporalio` image, and is never bundled.
- **Cargo environment.** Every cargo command runs with `TAURI_CONFIG='{"bundle":{"externalBin":[],"resources":[]}}'`. Use `--test-threads=1` for integration and bin tests.
- **GES tests.**
  - Set `VIDEO_CREATER_RENDER_RUNTIME_ROOT=$HOME/.local/share/com.olhapi.video-creater/render-runtime/linux-3451661e032bbcf9d529a468ad23f69a`.
  - Its symlinks point into `/tmp/vc-deb-root`. If they are broken, rebuild with `rtk pnpm build:linux-media-runtime --output <that dir>`.
  - `project_export`-style renders also need `VIDEO_CREATER_COMPATIBILITY_DECODER=/home/olhapi/projects/video-creater/src-tauri/target/debug/video-creater-compatibility-decoder`. Build it with `rtk cargo build --manifest-path src-tauri/Cargo.toml -p video-creater-compatibility-decoder`.
- **Temporal dev server.** Run it from the official image or from the CLI binary under `/tmp`. Don't install anything system-wide.
- **Evidence.** Never claim evidence you didn't observe. A test that was skipped, ignored or not run is reported as not run. Ignored tests count only when run with `-- --ignored` and seen passing.
- **Unused code.** `knip.jsonc` has no temporary ignore block (`scripts/knip-temporary-block.test.ts`). Every new TS export must be consumed by the end of Task 11.

## Parallelization and file ownership

| Wave | Tasks | Notes |
| --- | --- | --- |
| A | Task 1, Task 2, Task 3 | Run in parallel in separate worktrees. Task 1 alone owns `src/lib/project.ts`, `src/editor/store/project-slice.ts` and the `JobSummary` literal sites. Tasks 2 and 3 both add one line to `src-tauri/src/project/mod.rs`, so merge Task 2 first, then Task 3. |
| B | Task 4, Task 5, Task 9 | Run in parallel. Task 4 (after 2) owns `src-tauri/src/main.rs`. Task 5 (after 1) owns `src-tauri/src/workflows/mod.rs` and possibly `src-tauri/Cargo.toml`. Task 9 (after 1) owns `src/lib/project.ts`. |
| C | Task 6, Task 7, Task 8, Task 10 | Task 6 (after 3, 4, 5) owns `main.rs`, so it is sequential with Task 4. Task 7 (after 5) owns `tests/temporal_reconcile.rs` and the docs. Task 8 (after 3, 5) owns `workflows/mod.rs` and `generation/xai.rs`. Task 10 (after 9) owns the editor store and services. Tasks 6, 7, 8 and 10 don't share files. |
| D | Task 11 | After 6 and 10. Owns the fixtures and e2e specs. |
| E | Task 12, then Task 13 | Sequential, after everything. |

Sequential hot spots:
- `src-tauri/src/main.rs`: Task 4, then Task 6.
- `src/lib/project.ts`: Task 1, then Task 9.
- `src-tauri/src/workflows/mod.rs`: Task 5, then Task 8.
- `src-tauri/Cargo.toml`: Task 5 only, and only if its verification step requires it.
- `package.json` and `knip.jsonc`: no task changes them.

Parallel cargo runs contend for the lock on `src-tauri/target` and serialize there. Plan for the wait.

Cross-workstream conflicts. Serialize with the other gap-closure plans:
- **`main.rs`:** 02 changes export inputs and reveal, and 03 changes result frame capture.
- **`project.ts` and `project/action.rs`:** 04 adds new actions.
- **`gstreamer_backend.rs`:** 04 adds `scaletempo`.
- **`codex/context.rs`:** 03 splits it, and Task 1 edits one `JobSummary` literal there. Land Task 1 before 03's split, or add the field inside the split modules.
- **`task-records.ts` and `export-service.ts`:** 02 changes retry quality.

---

## File Map

### Create
- **`src-tauri/src/project/command_queue.rs`** (Task 2): `ProjectCommandQueue`, `project_command_queue()`, `QueuedProjectCommand<T>` (`Future` plus `wait()`), and unit tests.
- **`src-tauri/src/project/job_progress.rs`** (Task 3):
  - `JobProgressSnapshot`, `JobProgressReporter`, `read_job_progress_snapshots` and `remove_settled_job_progress`
  - `JOB_PROGRESS_DIR = "logs/job-progress"` and `JOB_PROGRESS_MIN_INTERVAL = 500 ms`
  - unit tests
- **`src-tauri/src/workflows/temporal_reconcile.rs`** (Task 5):
  - `WorkflowObservation`, `DescribeOutcome`, `TemporalWorkflowDescriber`
  - `plan_temporal_job_reconciliation`, `reconcile_temporal_jobs_with_describer`
  - `TemporalClientDescriber` and `connect_temporal_client_from_environment`, both behind `#[cfg(feature = "temporal-worker")]`
  - `TemporalJobReconciliationResult`
  - unit tests
- **`src-tauri/src/command_thread_audit.rs`** (Task 4): a `#[cfg(test)]` source audit of `main.rs` command threading.
- **`src-tauri/tests/temporal_reconcile.rs`** (Tasks 5 and 7): fake-describer integration tests over a split project fixture, plus `#[ignore]` dev-server tests.
- **`src-tauri/tests/project_action/job_failure.rs`** (Task 1): `recordJobFailure` and failure-reason clearing.
- **`src-tauri/tests/render_progress_ges.rs`** (Task 3): a GES render reports progress readable without the lease.
- **`src/editor/store/job-monitors.ts` and `job-monitors.test.ts`** (Task 10): the progress poller and the Temporal reconciler timers.
- **`src/lib/jobs/start-failure.ts` and `start-failure.test.ts`** (Task 10): `workflowNeverStartedReason` and `workflowStartFailureActions(job, updatedAt)`.

### Modify
- **Rust model and actions (Task 1):**
  - `src-tauri/src/project/model.rs` (`JobSummary.failure_reason`)
  - `src-tauri/src/project/action.rs` (`RecordJobFailure`, `record_job_failure`; `update_job_status` clears the reason)
  - `src-tauri/src/codex/conversation/risk.rs`
  - every `JobSummary { … }` literal from `rtk grep -rn "JobSummary {" src-tauri/src src-tauri/tests`, which today are:
    - `workflows/mod.rs`, `render_pipeline/project_export.rs`, `render_pipeline/codex_e2e.rs`, `codex/context.rs`
    - `project/split.rs`, `project/split/agent_undo_content.rs`, `project/action.rs`
    - `bin/video-creater-compatibility-evidence.rs`, `bin/video-creater-native-export-evidence.rs`
    - `tests/codex_app_server.rs`, `tests/codex_conversation/context.rs`, `tests/codex_conversation/undo_bookkeeping.rs`, `tests/project_split.rs`
    - `tests/project_export_prores_appkit.rs`, `tests/project_export_nested_effect_appkit.rs`
  - `src-tauri/tests/project_action.rs` (the `#[path]` module line)
- **TS lockstep (Task 1):** `src/lib/project.ts`, `src/lib/project.test.ts`, `src/editor/store/project-slice.ts`, `src/lib/runtime/fixtures/conversation-fixture-proposals.ts`.
- **Queue and progress wiring (Tasks 2 and 3):**
  - `src-tauri/src/project/mod.rs`
  - `src-tauri/src/project/mutation.rs` (expose `pub(crate) fn project_mutation_key`)
  - `src-tauri/src/render_pipeline/cancel.rs` (reporter on the attempt state; `RenderCancellationToken::report_progress`)
  - `src-tauri/src/render_pipeline/gstreamer_backend.rs` (position and duration in `wait_for_gst_pipeline_with_cancellation`)
- **`src-tauri/src/main.rs` (Tasks 4 and 6):**
  - async conversions and `_blocking` helpers
  - `run_project_write_command`
  - the `mod command_thread_audit` line
  - the new commands `load_job_progress_from_split_project_folder` and `reconcile_temporal_jobs_in_split_project_folder`
  - `remove_settled_job_progress` on load
  - the imports in the test module
- **`src-tauri/src/workflows/mod.rs` (Tasks 5 and 8):** `pub mod temporal_reconcile;` and the xAI progress reporter at the `run_xai_video_generation_submission_with_client_cancellable` call site.
- **`src-tauri/src/generation/xai.rs` and `src-tauri/tests/xai_generation_provider.rs` (Task 8).**
- **Frontend lib (Task 9):**
  - `src/lib/project.ts`: `JobProgressSnapshot`, `TemporalJobReconciliation` and the two wrappers
  - `src/lib/jobs/task-records.ts` and `task-records.test.ts`
  - `src/lib/runtime/fixtures/task-fixtures.test.ts` and `export-fixtures.test.ts`, only for the new `TaskRuntime` fields
- **Store and services (Task 10):**
  - `src/editor/store/jobs-slice.ts` and `jobs-slice.test.ts`
  - `src/editor/services/export-service.ts`, `speech-service.ts`, `generation-service.ts` and their tests
- **Fixtures and e2e (Task 11):**
  - `src/lib/runtime/fixtures/task-fixtures.ts`, `export-fixtures.ts` and their tests
  - `e2e/editor-tasks.spec.ts`, `e2e/editor-export-tasks.spec.ts`
- **Docs:**
  - `docs/development/runtime-and-verification.md` (Task 7)
  - `docs/product-backlog.md` (Task 13)

---

### Task 1: Job failure reason and the `recordJobFailure` action (Rust/TS lockstep)

**Owns:** `project/model.rs`, `project/action.rs`, `codex/conversation/risk.rs`, all `JobSummary` literal sites listed in the File Map, `tests/project_action.rs`, `tests/project_action/job_failure.rs` (new), `src/lib/project.ts`, `src/lib/project.test.ts`, `src/editor/store/project-slice.ts`, `src/lib/runtime/fixtures/conversation-fixture-proposals.ts`. **Parallel with:** Tasks 2 and 3.

- [ ] **Write failing Rust tests** in `src-tauri/tests/project_action/job_failure.rs`. Register the file in `tests/project_action.rs` with `#[path = "project_action/job_failure.rs"] mod job_failure;`, following the `transitions` modules.
  - `record_job_failure_fails_a_queued_job_with_its_reason`: status `Failed`, `failure_reason == Some("The workflow never started.")`, `updated_at` set.
  - `record_job_failure_leaves_a_finished_job_untouched`: a `Completed` or `Cancelled` job is unchanged, and the call returns `Ok`.
  - `record_job_failure_rejects_a_blank_reason_and_unknown_job` (`JobNotFound`).
  - `update_job_status_to_running_clears_the_failure_reason`.
  - `record_job_failure_round_trips_as_camel_case_json`: the action is `{"type":"recordJobFailure","jobId":…,"reason":…,"updatedAt":…,"runId":null}`, and a job serializes `failureReason`, which is omitted when `None`.
  - **Run:** `TAURI_CONFIG='{"bundle":{"externalBin":[],"resources":[]}}' rtk cargo test --manifest-path src-tauri/Cargo.toml --test project_action job_failure -- --test-threads=1`. Expect a compile failure.
- [ ] **Implement the Rust model:**
  - Add `#[serde(default, skip_serializing_if = "Option::is_none")] pub failure_reason: Option<String>` to `JobSummary` in `model.rs`.
  - Add `ProjectAction::RecordJobFailure { job_id: String, reason: String, updated_at: String, #[serde(default, skip_serializing_if = "Option::is_none")] run_id: Option<String> }`, with `record_job_failure` in `action.rs`. It sets `Failed` only for `Queued | Running | Progress | Blocked`.
  - `update_job_status` sets `failure_reason = None` when the new status isn't `Failed`.
  - `risk.rs` adds the variant to the `RecordJob | UpdateJobStatus | UpdateJobProviderRequest` arm ("changesJobs").
- [ ] **Fix every `JobSummary { … }` literal** by adding `failure_reason: None`. Build all targets:
  - `TAURI_CONFIG='…' rtk cargo test --manifest-path src-tauri/Cargo.toml --no-run --all-targets`
  - `rtk grep -rn "JobSummary {" src-tauri/src src-tauri/tests` must show no literal without the field.
- [ ] **Write failing TS tests** in `src/lib/project.test.ts`:
  - `applyProjectActionLocally` with `recordJobFailure` fails a queued job and sets `failureReason`.
  - It leaves a completed job unchanged.
  - `updateJobStatus` to `running` drops `failureReason`.

  Run `rtk pnpm vitest run src/lib/project.test.ts`. Expect failures.
- [ ] **Implement the TS side:**
  - `project.ts`: add `failureReason?: string | null` to `ProjectJobSummary`, the union member `{ type: "recordJobFailure"; jobId: string; reason: string; updatedAt: string; runId?: string | null }`, and the `applyProjectActionLocally` case. Also clear the reason in the `updateJobStatus` case.
  - `project-slice.ts`: add `"recordJobFailure"` to `nonUndoableActionTypes`.
  - `conversation-fixture-proposals.ts`: add `recordJobFailure: jobs` to `reviewReasons`.
- [ ] **Verify:**
  - `TAURI_CONFIG='…' rtk cargo test --manifest-path src-tauri/Cargo.toml --test project_action -- --test-threads=1`
  - `TAURI_CONFIG='…' rtk cargo test --manifest-path src-tauri/Cargo.toml --test project_split -- --test-threads=1`
  - `TAURI_CONFIG='…' rtk cargo test --manifest-path src-tauri/Cargo.toml --test codex_app_server -- --test-threads=1`, which confirms the Codex schema is unchanged
  - `rtk pnpm vitest run src/lib/project.test.ts src/editor/store/project-slice.test.ts src/lib/runtime/fixtures/conversation-fixtures.test.ts`
  - `rtk pnpm lint`
- [ ] **Commit:** `feat(jobs): record why a background job failed`

### Task 2: Per-project FIFO command queue (lib)

**Owns:** `src-tauri/src/project/command_queue.rs` (new), `src-tauri/src/project/mod.rs`, `src-tauri/src/project/mutation.rs`. **Parallel with:** Tasks 1 and 3. Merge before Task 3 because of `mod.rs`.

- [ ] **Expose the key.** Rename the private `mutation_key` in `mutation.rs` to `pub(crate) fn project_mutation_key(project_dir: &Path) -> Result<PathBuf, String>`, and update its callers in that file.
- [ ] **Write failing unit tests** in `command_queue.rs` (`#[cfg(test)] mod tests`). Use a test-local `block_on` built on `std::task::Wake` and thread parking.
  - `commands_for_one_project_complete_in_submission_order`: five submissions from five threads, gated by a `Barrier`. The recorded order equals the order of the `submit` calls.
  - `submit_returns_while_a_render_holds_the_project_lease`:
    1. A helper thread holds `acquire_split_project_mutation_lease(dir)`.
    2. Submit a closure that takes the same lease and records a write.
    3. `submit` returns within 50 ms, and the result isn't ready after 200 ms.
    4. Release the lease. `wait()` returns `Ok` within 2 s.
  - `different_projects_run_concurrently`: project A is blocked on the lease, and project B's command still completes.
  - `a_panicking_command_fails_alone`: it returns `Err("<label> task panicked")`, and the next command for that key succeeds.
  - `nested_submit_for_the_same_project_runs_inline`: no deadlock within 2 s.
  - `worker_exits_when_its_queue_drains`: `active_project_keys_for_test()` is empty after `wait()`.
  - **Run:** `TAURI_CONFIG='…' rtk cargo test --manifest-path src-tauri/Cargo.toml --lib project::command_queue -- --test-threads=1`. Expect failures.
- [ ] **Implement:**
  - **API:** `pub fn project_command_queue() -> &'static ProjectCommandQueue` (a `OnceLock`), and `pub fn submit<T: Send + 'static>(&self, project_dir: &Path, label: &'static str, work: impl FnOnce() -> Result<T, String> + Send + 'static) -> Result<QueuedProjectCommand<T>, String>`.
  - **Queue state:** a `Mutex<HashMap<PathBuf, VecDeque<Job>>>`. A named `std::thread` (`project-commands`) is spawned when a key's deque goes from empty to non-empty. The worker drains the deque with `catch_unwind(AssertUnwindSafe(..))` and removes the key when the deque is empty, under the lock.
  - **Result delivery:** a one-shot slot (`Arc<(Mutex<Option<Result<T,String>>>, Condvar)>` plus an `Option<Waker>`). `QueuedProjectCommand` implements `Future` and has a blocking `wait()`.
  - **Nesting:** a thread-local "current key" makes a nested submit run `work` inline and return a ready command.
  - **Docs:** a doc comment explains why ordering holds. Submission order is the order `submit` is called. The editor's write queue (`project-slice.ts` `enqueue`) keeps its own submissions sequential.
  - **Registration:** add `pub mod command_queue;` to `project/mod.rs`.
- [ ] **Verify:**
  - Re-run the lib test command.
  - `TAURI_CONFIG='…' rtk cargo test --manifest-path src-tauri/Cargo.toml --lib project::mutation -- --test-threads=1`
  - `rtk wc -l src-tauri/src/project/command_queue.rs` is under 600.
- [ ] **Commit:** `feat(project): add a per-project FIFO command queue`

### Task 3: Lease-free job progress and GES render progress (lib)

**Owns:** `src-tauri/src/project/job_progress.rs` (new), `src-tauri/src/project/mod.rs` (one line; merge after Task 2), `src-tauri/src/render_pipeline/cancel.rs`, `src-tauri/src/render_pipeline/gstreamer_backend.rs`, `src-tauri/tests/render_progress_ges.rs` (new). **Parallel with:** Tasks 1 and 2.

- [ ] **Verify the storage location before relying on it:**
  - `save_split_project_metadata_transactionally` (`project/split.rs`) swaps only reported sidecar paths, so it must leave `logs/job-progress/` alone.
  - Add the test `progress_snapshots_survive_a_metadata_save` in `job_progress.rs`: write a snapshot, call `save_split_project` on a fixture split project, then read the snapshot back.
  - If this fails, move the directory to `renders/.job-progress/` and record why in the commit body.
- [ ] **Write failing unit tests** in `job_progress.rs`:
  - The reporter clamps to 0..=1, ignores NaN, and never goes backwards.
  - Throttling: `report_at(0.1, t0)` writes, `report_at(0.2, t0+100ms)` doesn't, `report_at(0.3, t0+600ms)` writes, and `report_at(1.0, t0+650ms)` always writes.
  - Writes are atomic: temp file plus rename in the same directory, validated with `project::split::validate_split_project_write_path`.
  - No write happens when `<root>/video-creater.project.json` (`project::storage::PROJECT_FILE_NAME`) is missing. That protects the fake roots in the `cancel.rs` tests.
  - `read_job_progress_snapshots`:
    - skips non-`.json` files, symlinks, files over 4 KiB and malformed JSON;
    - returns at most 256 entries;
    - never takes a lease. Prove it with a test that reads while another thread holds `acquire_split_project_mutation_lease` and `acquire_storage_mutation_lease`.
  - `clear()` removes the file.
  - `remove_settled_job_progress(dir, &project)` removes snapshots whose job is missing or finished and keeps the unfinished ones.
  - **Run:** `TAURI_CONFIG='…' rtk cargo test --manifest-path src-tauri/Cargo.toml --lib project::job_progress -- --test-threads=1`. Expect failures.
- [ ] **Implement `job_progress.rs`:**
  - `#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)] #[serde(rename_all = "camelCase")] pub struct JobProgressSnapshot { job_id: String, progress: f64, updated_at: String }`.
  - `JobProgressReporter { project_dir, job_id, last: Mutex<Option<(Instant, f64)>> }`, which also derives `Debug`.
  - Register it in `project/mod.rs`.
- [ ] **Write failing tests in `cancel.rs`:**
  - `render_attempt_progress_is_written_for_a_split_project_and_cleared_on_drop`, using a tempdir with a manifest file.
  - `render_attempt_progress_is_a_no_op_for_a_non_project_root`.
- [ ] **Implement in `cancel.rs`:**
  - `RenderCancellationState` gains `progress: JobProgressReporter`, built in `register_render_attempt` from `key.project_root()` and `key.job_id()`.
  - Add `pub fn report_progress(&self, fraction: f64)` to `RenderCancellationToken`.
  - `unregister_render_attempt` calls `progress.clear()`.
- [ ] **Verify the GStreamer API names** before using them. `ElementExtManual::query_position::<gst::ClockTime>()` and `query_duration::<gst::ClockTime>()` return `Option<T>` in gstreamer 0.25.2. Check with `rtk grep -n "fn query_position\|fn query_duration" ~/.cargo/registry/src/*/gstreamer-0.25.2/src/element.rs`.
- [ ] **Implement in `gstreamer_backend.rs`.** In `wait_for_gst_pipeline_with_cancellation`, after each `None` from `bus.timed_pop_filtered` (that is, every `GST_RENDER_CANCEL_POLL_INTERVAL`):
  - when `cancellation` is `Some`, and position and duration are both known with `duration > 0`,
  - call `token.report_progress(0.95 * position / duration)`.

  GES encoding covers 0–0.95. 1.0 is never shown before the job completes, because validation and report writing follow the encode.
- [ ] **Write the GES integration test** `src-tauri/tests/render_progress_ges.rs`:
  - `in_process_render_reports_progress_readable_without_the_lease`:
    1. Build a split project with a generated source of at least 4 s. Reuse the fixture helpers that `tests/project_export.rs` uses for `render_media_to_split_project_folder`, and copy only the helper you need if it is private.
    2. Render on a thread.
    3. Every 50 ms, call `read_job_progress_snapshots` while the render thread is unfinished. Each read must return in under 100 ms.
    4. After the join, assert at least one observed value is in `(0, 0.95]`.
    5. Assert the values never decrease.
    6. Assert the snapshot file is gone.
  - Skip with an explicit `eprintln!("SKIPPED: …")` only when the render runtime can't resolve. Report a skip as not run.
- [ ] **Run the GES test** (foreground):
  ```bash
  TAURI_CONFIG='{"bundle":{"externalBin":[],"resources":[]}}' \
  VIDEO_CREATER_RENDER_RUNTIME_ROOT=$HOME/.local/share/com.olhapi.video-creater/render-runtime/linux-3451661e032bbcf9d529a468ad23f69a \
  VIDEO_CREATER_COMPATIBILITY_DECODER=/home/olhapi/projects/video-creater/src-tauri/target/debug/video-creater-compatibility-decoder \
    rtk cargo test --manifest-path src-tauri/Cargo.toml --test render_progress_ges -- --test-threads=1 --nocapture
  ```
  Confirm the output has no `SKIPPED` line and lists the observed progress values. If GES returns no position while it renders, record that in the commit body. Then fall back to reporting the elapsed wall-clock time over the plan duration, capped at 0.9, and keep the same test.
- [ ] **Regression runs** with the same environment:
  - `--test render_transitions_ges`
  - `--test project_export`
  - `--lib render_pipeline::`
  - `rtk cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets -- -D warnings`
- [ ] **Commit:** `feat(render): report render progress without the project lease`

### Task 4: Move lease-bound commands off the main thread (main.rs)

**Owns:** `src-tauri/src/main.rs` and `src-tauri/src/command_thread_audit.rs` (new). **After:** Task 2. **Sequential with:** Task 6, which also owns `main.rs`.

Inventory. A script over `main.rs` found the synchronous commands below that reach a lease or project I/O. Every item here must end up async.

| Route | Commands |
| --- | --- |
| **Queue** (`run_project_write_command`) | `save_split_project_to_folder`, `save_project_to_folder`, `import_media_to_project`, `apply_project_action_to_split_project_folder`, `apply_project_actions_to_split_project_folder`, `update_project_settings_in_split_project_folder`, `migrate_single_file_project_to_split`, `create_matte_in_split_project_folder`, `rename_project_speaker`, `recolor_project_speaker`, `assign_project_media_speaker`, `apply_agent_session_action_to_split_project_folder`, `complete_mock_generated_asset_in_split_project_folder`, `cancel_generate_media_in_process`, `export_nle_xml_to_split_project_folder`, `cancel_render_job_in_split_project_folder` (see below); plus the already-async `apply_codex_conversation_proposal` and `undo_latest_codex_conversation_edit` (switch `spawn_blocking` to the queue) |
| **Blocking pool** (`run_blocking_command`) | `load_project_from_folder`, `load_settings_acceptance_project`, `validate_split_project_folder`, `load_app_server_conversation_history_from_split_project_folder`, `load_agent_sessions_from_split_project_folder`, `search_project_media`, `rebuild_project_search_index`, `extract_visual_frame_cache_in_split_project_folder`, `caption_visual_frame_cache_in_split_project_folder`, `retry_generated_asset_output_download_in_split_project_folder`, `analyze_media_for_edit_in_split_project_folder`, `cancel_generate_media_provider_request_in_split_project_folder`, `reveal_export_artifact_in_split_project_folder`, `call_codex_local_tool`, `preview_storage_cleanup`, `run_storage_cleanup` (whose `start_storage_cleanup` takes the storage lease before it spawns) |
| **Already async but loading on the async runtime** | `run_generate_media_in_process`: move its leading `load_split_project` into `spawn_blocking` |
| **Stay synchronous** (audit allowlist, with reasons) | `get_project_speaker_registry` (`load_speaker_registry` reads the registry file without a lease), `load_render_pipeline_report_from_split_project_folder` (reads `renders/<job>/pipeline-report.json` without a lease), `list_shader_background_templates` (reads templates, no lease), `get_storage_health` and `refresh_storage_inventory` (inventory walks take no lease) |

- [ ] **Write the failing audit test** in `src-tauri/src/command_thread_audit.rs`, declared in `main.rs` as `#[cfg(test)] mod command_thread_audit;`. It parses `include_str!("main.rs")`:
  1. Cut the source at `#[cfg(test)]\nmod tests`.
  2. Split it into top-level items at lines starting with `fn `, `async fn `, `pub fn `, `#[` or `struct `.
  3. Collect each `#[tauri::command]` function and whether it is `async`.
  4. For every synchronous command, walk the calls it makes into same-file helpers to depth 3, skipping helpers marked `#[tauri::command]`.
  5. Fail if the walk reaches any of these tokens:
     - leases: `acquire_split_project_mutation_lease`, `acquire_storage_mutation_lease`, `mutation_coordinator`
     - project reads and writes: `load_split_project`, `save_split_project`, `apply_project_action_to_split_project`, `apply_project_actions_to_split_project`, `replace_split_project_if_revision`, `update_project_settings_in_split_project`, `migrate_single_file_project`, `validate_split_project`
     - imports and mattes: `import_media_files`, `create_matte`
     - agent sessions and history: `apply_agent_session_action`, `load_agent_session_manifest`, `load_app_server_conversation_history`
     - speakers: `rename_speaker`, `recolor_speaker`, `assign_media_speaker`
     - search and frame caches: `query_project_search_for_project_dir`, `rebuild_project_search_index_for_project_dir`, `cache_visual_frames`, `caption_cached_visual_frames_with_fal`
     - Codex turns: `codex_project_for_turn`

     The allowlist above is excluded, each entry with its reason string.
  - Add a second test that runs the same audit over a synthetic source string, a sync command calling `load_split_project`, and expects it to be flagged.
  - **Run:** `TAURI_CONFIG='…' rtk cargo test --manifest-path src-tauri/Cargo.toml --bin video-creater command_thread_audit -- --test-threads=1`. Expect it to list every inventory command.
- [ ] **Add `run_project_write_command`** next to `run_blocking_command`:
  - Signature: `async fn run_project_write_command<T: Send + 'static>(label: &'static str, project_dir: &str, work: impl FnOnce(PathBuf) -> Result<T, String> + Send + 'static) -> Result<T, String>`.
  - It resolves the directory with `resolve_project_dir`, then `project_command_queue().submit(..)?.await`.
  - Generalize the error type of `run_blocking_command` (for example `E: From<String>`), or add a sibling for `SettingsOperationsCommandError`, so the storage cleanup commands keep their error shape.
- [ ] **Convert the commands,** keeping names, arguments and results unchanged:
  - **Bodies.** Move each body into a synchronous `<name>_blocking` function. That matches the existing `load_split_project_from_folder_blocking` and `prepare_project_preview_blocking`.
  - **State parameters.** Commands that take `tauri::State` switch to `app: tauri::AppHandle<R>` with `<R: tauri::Runtime>`, and read state inside the closure through `app.state::<T>()`, following `load_split_project_from_folder`.
  - **`cancel_render_job_in_split_project_folder`.**
    1. Call `request_render_cancellation_by_locator` synchronously in the async body, before any await.
    2. Only then queue the `UpdateJobStatus { Cancelled }` write, or the `NotFound` validation load.

    Add a comment explaining that, queued behind the render, the cancellation would never reach it.
  - **Test module.** Update the `use super::{…}` list in `main.rs` `mod tests` to the `_blocking` names for:
    - `call_codex_local_tool`
    - `cancel_generate_media_in_process`, `cancel_generate_media_provider_request_in_split_project_folder`
    - `cancel_render_job_in_split_project_folder`
    - `complete_mock_generated_asset_in_split_project_folder`
    - `export_nle_xml_to_split_project_folder`
    - `load_app_server_conversation_history_from_split_project_folder`
    - `retry_generated_asset_output_download_in_split_project_folder`
    - `validate_split_project_folder`

    The IPC tests built with `tauri::test::mock_builder` and `get_ipc_response` (storage cleanup preview, settings) keep invoking by command name.
- [ ] **Add a queue-routing test** in `main.rs` tests: `project_writes_queue_behind_a_held_project_lease_in_order`.
  1. Hold the project lease on a helper thread.
  2. From a second thread, call `apply_project_actions_to_split_project_folder` through `tauri::async_runtime::block_on` three times, spawned in order with renames "A", "B", "C" of one media item.
  3. Release the lease.
  4. Assert the final name is "C" and the three results carry increasing `contentRevision`.
- [ ] **Verify:**
  - `TAURI_CONFIG='…' rtk cargo test --manifest-path src-tauri/Cargo.toml --bin video-creater -- --test-threads=1`. All bin suites must pass, including `command_thread_audit` and the existing `mod tests`.
  - `TAURI_CONFIG='…' rtk cargo test --manifest-path src-tauri/Cargo.toml --test codex_conversation -- --test-threads=1`
  - `rtk cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets -- -D warnings`
  - `rtk pnpm vitest run src/lib/project.test.ts`: the TS contract is unchanged
- [ ] **Commit:** `fix(desktop): keep project commands off the main thread while renders hold the lease`

  The body lists the inventory and the allowlist with its reasons.

### Task 5: Temporal job reconciliation (lib) with a fake describer

**Owns:** `src-tauri/src/workflows/temporal_reconcile.rs` (new), `src-tauri/src/workflows/mod.rs` (the `pub mod` line), `src-tauri/tests/temporal_reconcile.rs` (new), and `src-tauri/Cargo.toml` only if the timeout step requires it. **After:** Task 1. **Parallel with:** Tasks 4 and 9.

- [ ] **Verify the Temporal API names** in the vendored `temporalio-client` 0.4.0 sources before using them:
  - `Client::get_workflow_handle::<UntypedWorkflow>(workflow_id)` exists (`src/lib.rs`).
  - `WorkflowHandle::describe(WorkflowDescribeOptions::default())` exists (`src/workflow_handle.rs`).
  - `WorkflowExecutionDescription::status() -> WorkflowExecutionStatus` exists, with the variants `Unspecified, Running, Completed, Failed, Canceled, Terminated, ContinuedAsNew, TimedOut, Paused`.
  - `WorkflowInteractionError::{NotFound, Rpc(tonic::Status), …}` exists (`src/errors.rs`), and `TEMPORAL_ADDRESS` is read by `temporalio-common` `envconfig.rs`.

  Checks:
  - `rtk grep -n "pub async fn describe" ~/.cargo/registry/src/*/temporalio-client-0.4.0/src/workflow_handle.rs`
  - `rtk grep -rn "pub enum WorkflowExecutionStatus" -A12 src-tauri/target/debug/build/temporalio-common-*/out/temporal.api.enums.v1.rs`
- [ ] **Write failing planner tests** in `temporal_reconcile.rs`. `plan_temporal_job_reconciliation(project, observations, now, grace, generation_backend_in_process) -> Vec<(job_id, reason)>` works as follows:
  - **Candidates.** A job is a candidate when all of these hold:
    - its status is `Queued | Running | Progress | Blocked`;
    - `start_request` is present;
    - its kind isn't `captureCanonicalPreviewFrame`;
    - its run id is `None`, or doesn't start with `in-process/`, `render-attempt/` or `mock-run-`;
    - it isn't a `generate_media` job without a run id while the generation backend is in-process, because `reconcile_interrupted_generation_jobs_on_project_open` owns those.
  - **Mapping for jobs with a run id:**

    | Observation | Result |
    | --- | --- |
    | `Running`, `Paused`, `ContinuedAsNew`, `Unspecified` | no change |
    | `Completed` | "The workflow finished without reporting this task's result." |
    | `Failed` | "The workflow failed." |
    | `Canceled` | "The workflow was cancelled." |
    | `Terminated` | "The workflow was stopped." |
    | `TimedOut` | "The workflow timed out." |
    | `NotFound` | "The workflow service has no record of this task." |
  - **Jobs without a run id.** `NotFound` fails the job with "The workflow never started." when `updated_at` is older than `grace` (2 minutes). A job inside the grace period is unchanged. Any other observation is unchanged, because the start may still record its run id.
  - **An `Unreachable` observation changes nothing** for any job.
  - Tests: one per row above, plus the grace boundary, the mock, in-process and render-attempt exclusions, and the in-process generation exclusion.
  - **Run:** `TAURI_CONFIG='…' rtk cargo test --manifest-path src-tauri/Cargo.toml --lib workflows::temporal_reconcile -- --test-threads=1`. Expect failures.
- [ ] **Implement the planner and the async driver.**
  - **Describer trait.** `pub trait TemporalWorkflowDescriber: Send + Sync { fn describe<'a>(&'a self, workflow_id: &'a str) -> Pin<Box<dyn Future<Output = DescribeOutcome> + Send + 'a>>; }`, with `DescribeOutcome::{Observed(WorkflowObservation), NotFound, Unreachable(String)}`.
  - **Driver.** `reconcile_temporal_jobs_with_describer(project_dir, describer, now, grace, backend_in_process) -> Result<TemporalJobReconciliationResult, String>`:
    1. Load the project through an injected `load` closure (the command passes a `load_split_project` call on the blocking pool), so the module stays free of Tauri and tests stay synchronous.
    2. Describe each distinct `workflow_id`, sequentially.
    3. If any outcome is `Unreachable`, return `{ project: None, failed_job_ids: [], service_reachable: false, detail: Some("Workflow service unreachable") }` without writing anything.
    4. Otherwise run an `apply` closure. The command passes one that runs inside the project command queue: it reloads the project, re-plans, and applies `ProjectAction::RecordJobFailure` for each result through `apply_project_actions_to_split_project`.
  - **Result type.** `#[serde(rename_all = "camelCase")] pub struct TemporalJobReconciliationResult { pub project: Option<VideoProject>, pub failed_job_ids: Vec<String>, pub service_reachable: bool, pub detail: Option<String> }`.
  - **Real client (`temporal-worker` feature).**
    - `connect_temporal_client_from_environment()` uses the same `ClientOptions::load_from_config` and `Connection::connect` as `start_temporal_workflow` in `main.rs`.
    - `TemporalClientDescriber` maps `WorkflowInteractionError::NotFound` to `NotFound`.
    - It maps `Rpc` statuses and connect errors to `Unreachable(message)`, because none of them proves the workflow is gone.
  - **Registration:** add `pub mod temporal_reconcile;` to `workflows/mod.rs`.
- [ ] **Verify the unreachable timeout.** Add `unreachable_server_is_reported_quickly` in `tests/temporal_reconcile.rs`:
  - With `TEMPORAL_ADDRESS=http://127.0.0.1:1` set in-process, `connect_temporal_client_from_environment()` must error within 5 s.
  - If `RetryOptions` makes it slower, bound the connect with `tokio::time::timeout(Duration::from_secs(3), …)`.
  - That needs the tokio `time` feature. Add `"time"` to the optional `tokio` features in `src-tauri/Cargo.toml` (MIT), and say so in the commit body.
- [ ] **Write the fake-describer integration tests** in `src-tauri/tests/temporal_reconcile.rs` over a real split project in a tempdir, built with `save_split_project`:
  - `a_terminated_workflow_fails_its_running_export_job_with_a_plain_reason`
  - `a_queued_job_never_started_fails_after_the_grace_period`
  - `a_running_workflow_leaves_the_job_untouched`
  - `an_unreachable_service_writes_nothing_and_reports_unreachable`: assert the project's `contentRevision` is unchanged
  - `a_job_that_finished_between_describe_and_apply_is_not_failed`: the fake completes the job during `describe`
- [ ] **Verify:**
  - `TAURI_CONFIG='…' rtk cargo test --manifest-path src-tauri/Cargo.toml --test temporal_reconcile -- --test-threads=1`
  - `--test temporal_workflows`
  - `rtk cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets -- -D warnings`
- [ ] **Commit:** `feat(workflows): reconcile unfinished Temporal jobs with their workflows`

### Task 6: Progress and reconciliation commands (main.rs)

**Owns:** `src-tauri/src/main.rs`. **After:** Tasks 3, 4 and 5.

- [ ] **Write failing tests** in `main.rs` `mod tests`:
  - `load_job_progress_returns_snapshots_while_the_project_lease_is_held`: another thread holds both leases, and `tauri::async_runtime::block_on(load_job_progress_from_split_project_folder(dir))` returns the written snapshot within 1 s.
  - `project_load_removes_progress_for_finished_jobs`: `load_split_project_from_folder_blocking` deletes the snapshot of a completed job and keeps a running one.
  - `reconcile_command_without_temporal_feature_reports_unavailable`, under `#[cfg(not(feature = "temporal-worker"))]`.
  - `reconcile_command_serializes_camel_case_result`: IPC through `mock_builder` with `generate_handler![super::reconcile_temporal_jobs_in_split_project_folder]` returns the shape `{ project, failedJobIds, serviceReachable, detail }`. Point `TEMPORAL_ADDRESS` at `http://127.0.0.1:1` so it takes the unreachable path.
  - **Run:** `TAURI_CONFIG='…' rtk cargo test --manifest-path src-tauri/Cargo.toml --bin video-creater job_progress reconcile -- --test-threads=1`. Expect failures.
- [ ] **Implement:**
  - **Progress command.** `async fn load_job_progress_from_split_project_folder(project_dir: String) -> Result<Vec<JobProgressSnapshot>, String>` uses `run_blocking_command` and `read_job_progress_snapshots`. It takes no lease.
  - **Reconcile command.** `async fn reconcile_temporal_jobs_in_split_project_folder<R: tauri::Runtime>(app: tauri::AppHandle<R>, project_dir: String, updated_at: String) -> Result<TemporalJobReconciliationResult, String>`:
    - Read `generation_execution_backend` from `AppPreferencesState` (see `get_app_preferences`).
    - Connect, describe, and apply the failures through `project_command_queue()`.
    - `cfg(not(feature = "temporal-worker"))` returns `service_reachable: false` with the detail "Workflow service unreachable".
  - **Load cleanup.** In `load_split_project_from_folder_impl_with_lease`, call `remove_settled_job_progress(&project_dir, &project)` after recovery.
  - **Registration.** Register both commands in `generate_handler!` in `run()`.
- [ ] **Verify:**
  - `TAURI_CONFIG='…' rtk cargo test --manifest-path src-tauri/Cargo.toml --bin video-creater -- --test-threads=1`
  - `rtk cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets -- -D warnings`
- [ ] **Commit:** `feat(desktop): expose job progress and Temporal job reconciliation to the editor`

### Task 7: Real run against a local Temporal dev server

**Owns:** `src-tauri/tests/temporal_reconcile.rs` (the ignored tests), `docs/development/runtime-and-verification.md`. **After:** Task 5. **Parallel with:** Tasks 6, 8 and 10.

- [ ] **Start a dev server without a system install.** First try the official image:
  ```bash
  rtk docker run --rm -d --name vc-temporal-dev -p 7233:7233 -p 8233:8233 temporalio/temporal:latest server start-dev --ip 0.0.0.0
  rtk docker logs vc-temporal-dev
  ```
  Confirm the logs contain `Temporal Server:` and that `localhost:7233` accepts connections. If the image can't be pulled or started, use the MIT CLI in `/tmp` instead:
  ```bash
  mkdir -p /tmp/vc-temporal-cli && rtk curl -fL -o /tmp/vc-temporal-cli/temporal.tar.gz "https://temporal.download/cli/archive/latest?platform=linux&arch=amd64"
  tar -xzf /tmp/vc-temporal-cli/temporal.tar.gz -C /tmp/vc-temporal-cli
  /tmp/vc-temporal-cli/temporal server start-dev --headless > /tmp/vc-temporal-cli/dev.log 2>&1 &
  ```
  Verify the archive URL and the `--headless` flag with `/tmp/vc-temporal-cli/temporal server start-dev --help` before relying on them. Record which option ran.
- [ ] **Write the ignored tests.** `#[ignore = "needs a Temporal dev server at TEMPORAL_ADDRESS"]`, using a `tokio::runtime::Runtime`:
  - `dev_server_running_workflow_without_a_worker_is_left_running`:
    1. Start `VideoCreaterExportMediaWorkflow` on a unique task queue with no worker, through `temporal_start_workflow_with_client` and a job built with `temporal_job_summary` plus a start request.
    2. Record the job with its run id.
    3. Reconcile with `TemporalClientDescriber`. The job is still `running`.
  - `dev_server_terminated_workflow_fails_the_job`: terminate through `get_workflow_handle::<UntypedWorkflow>(id).terminate(WorkflowTerminateOptions::default())`, reconcile, and expect `failed` with "The workflow was stopped.".
  - `dev_server_unknown_workflow_fails_a_stale_queued_job`: a queued job with no run id, `updated_at` 10 minutes ago, and a workflow id that was never started. Expect "The workflow never started.".
  - `closed_port_leaves_jobs_untouched`: `TEMPORAL_ADDRESS=http://127.0.0.1:1`. This one needs no server and isn't ignored.
- [ ] **Run** (foreground, server up):
  ```bash
  TAURI_CONFIG='{"bundle":{"externalBin":[],"resources":[]}}' TEMPORAL_ADDRESS=http://localhost:7233 \
    rtk cargo test --manifest-path src-tauri/Cargo.toml --test temporal_reconcile -- --ignored --test-threads=1 --nocapture
  ```
  All three ignored tests must be listed as `ok`. Save the output to `output/gap-closure-01/temporal-dev-server.txt`, which is git-ignored.
- [ ] **Stop the server:** `rtk docker stop vc-temporal-dev`, or kill the CLI process.
- [ ] **Document it.** Add a "Temporal reconciliation against a dev server" section to `docs/development/runtime-and-verification.md` with the commands above.
- [ ] **Commit:** `test(workflows): reconcile Temporal jobs against a local dev server`

### Task 8: xAI video generation progress, and the progress sources recorded

**Owns:** `src-tauri/src/generation/xai.rs`, `src-tauri/src/workflows/mod.rs` (the call site), `src-tauri/tests/xai_generation_provider.rs`. **After:** Tasks 3 and 5.

- [ ] **Verify the progress sources** and paste these findings into the commit body:
  - `rtk grep -n "progress" src-tauri/tests/xai_generation_provider.rs`: xAI status bodies carry `"progress":10` and `"progress":100`.
  - `rtk grep -rn "progress" src-tauri/src/generation/fal.rs src-tauri/src/generation/replicate.rs src-tauri/src/generation/google.rs src-tauri/src/generation/minimax.rs`: no numeric progress is decoded. fal exposes only `queue_position`.
  - `rtk grep -n "progress" src-tauri/crates/speech-worker/src/protocol.rs`: none, so transcription stays indeterminate.
- [ ] **Write a failing test** in `tests/xai_generation_provider.rs`, `xai_video_status_progress_is_reported_through_the_job_progress_reporter`:
  - The mock server answers `pending` with progress 10, then 55, then `done` with 100.
  - The test uses a split project tempdir with a manifest and a `JobProgressReporter` whose throttle is exercised through `report_at` or a zero poll interval.
  - The snapshot shows 0.55 before completion.
- [ ] **Implement:**
  - Add `#[serde(default)] progress: Option<f64>` to `XAiVideoStatusResponse`.
  - Add `run_xai_video_generation_submission_with_client_reporting(…, progress: Option<&JobProgressReporter>)`. It reports `progress / 100` on `pending`. The existing `…_cancellable` function delegates with `None`, so its signature is unchanged.
  - In `workflows/mod.rs`, at the `XAI_GROK_VIDEO_MODEL_ID` branch (around line 7190), build `JobProgressReporter::new(project_dir, &input.job_id)`, call the reporting variant, and `clear()` it when the call returns.
- [ ] **Verify:**
  - `TAURI_CONFIG='…' rtk cargo test --manifest-path src-tauri/Cargo.toml --test xai_generation_provider -- --test-threads=1`
  - `--test generation_provider`
  - `--test temporal_workflows`
  - `rtk cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets -- -D warnings`
- [ ] **Commit:** `feat(generation): report xAI video generation progress`

### Task 9: Frontend contract and task records

**Owns:** `src/lib/project.ts`, `src/lib/jobs/task-records.ts`, `src/lib/jobs/task-records.test.ts`, and the `TaskRuntime` literals in `src/lib/runtime/fixtures/task-fixtures.test.ts` and `export-fixtures.test.ts`. **After:** Task 1. **Parallel with:** Tasks 4 and 5.

- [ ] **Write failing tests** in `task-records.test.ts`. `TaskRuntime` gains the required fields `progressByJobId: ReadonlyMap<string, number>` and `workflowServiceUnreachable: boolean`.
  - A running job with a snapshot of 0.42 gets `progress: 0.42`. A queued, completed or failed job gets `null`, even with a snapshot.
  - The active in-process render record (`activeRenderRecord`) takes its progress from the map.
  - A failed job with `failureReason: "The workflow never started."` shows that reason. Without one, it keeps the kind default ("The export stopped before it finished.").
  - When `workflowServiceUnreachable` is true, a queued or running Temporal-backed record has the detail "Workflow service unreachable". In-process records keep their detail.
  - **Run:** `rtk pnpm vitest run src/lib/jobs/task-records.test.ts`. Expect failures.
- [ ] **Implement:**
  - **`project.ts`:**
    - `export interface JobProgressSnapshot { jobId: string; progress: number; updatedAt: string }`
    - `export interface TemporalJobReconciliation { project: VideoProject | null; failedJobIds: string[]; serviceReachable: boolean; detail: string | null }`
    - `loadJobProgressFromSplitProjectFolder(input: { projectDir: string }): Promise<JobProgressSnapshot[]>` calls `backendRequest("load_job_progress_from_split_project_folder", input)`.
    - `reconcileTemporalJobsInSplitProjectFolder(input: { projectDir: string; updatedAt: string }): Promise<TemporalJobReconciliation>` calls `backendRequest("reconcile_temporal_jobs_in_split_project_folder", input)`.
  - **`task-records.ts`:** fill `progress`, `failureReason` and `detail` as the tests describe. Update the doc comment on `progress`.
  - **Fixture tests:** add the two runtime fields to the `taskRecords(…)` calls in the two fixture test files.
- [ ] **Verify:**
  - `rtk pnpm vitest run src/lib/jobs src/lib/runtime/fixtures/task-fixtures.test.ts src/lib/runtime/fixtures/export-fixtures.test.ts src/lib/project.test.ts`
  - `rtk pnpm lint`

  `check:unused` runs at Task 11, once the wrappers are consumed.
- [ ] **Commit:** `feat(jobs): show job progress, failure reasons, and an unreachable workflow service in task records`

### Task 10: Jobs slice monitors and failed Temporal starts

**Owns:**
- `src/editor/store/jobs-slice.ts` and its test
- `src/editor/store/job-monitors.ts` and its test (new)
- `src/lib/jobs/start-failure.ts` and its test (new)
- `src/editor/services/export-service.ts`, `speech-service.ts`, `generation-service.ts` and their tests
- possibly `src/editor/services/generation-workflow-start.ts` (new; see the size step)

**After:** Task 9.

- [ ] **Write failing tests for `start-failure.ts`.** `workflowStartFailureActions(job, updatedAt)` returns `[{ type: "recordJobFailure", jobId, reason: "The workflow never started.", updatedAt, runId: null }]`. For `generate_media` it also returns `updateGeneratedAssetStatus` → `failed` for `generationJobAssetId(job)`.
- [ ] **Write failing tests for `job-monitors.ts`,** using fake timers and injected loaders. The file exports `createProgressMonitor` and `createTemporalReconciler`.
  - **Progress monitor:**
    - It polls every 500 ms while `hasRunningTask()` is true, and never overlaps an in-flight poll.
    - It keeps polling while a folder reload is still pending, which is the render case.
    - It stops when no task runs.
    - It disables itself on `FixtureOperationUnsupportedError` or a backend-unavailable error.
  - **Reconciler:**
    - It runs once on start and every 30 s while `hasActiveTemporalTask()` is true.
    - It passes `serviceReachable` and `detail` to its callback.
    - It hands a non-null `project` to the merge.
    - It never runs two reconciliations at once.
- [ ] **Write failing tests in `jobs-slice.test.ts`:**
  - `startPolling` starts both monitors for a pollable project.
  - Progress snapshots land in `jobProgress` and show on `tasks[i].progress` without changing `project`, and the project history length is unchanged.
  - A reconciliation result is merged through `mergeLoadedProject`, and the failed row shows its reason.
  - `workflowServiceUnreachable` flips the detail, and a later reachable result clears it.
  - `stopPolling` stops both monitors.
- [ ] **Write failing service tests:**
  - `export-service.test.ts`: a Temporal start that returns `status: "unavailable"`, or throws, applies `recordJobFailure` and still sets `lastError` to the start message.
  - `speech-service.test.ts`: the same for `startQueuedWorkflow`.
  - `generation-service.test.ts`: the same for `startTemporal`, and the generated asset becomes `failed`.
  - **Run:** `rtk pnpm vitest run src/lib/jobs/start-failure.test.ts src/editor/store/job-monitors.test.ts src/editor/store/jobs-slice.test.ts src/editor/services`. Expect failures.
- [ ] **Implement:**
  - **Jobs slice state.** Add `jobProgress: ReadonlyMap<string, number>` and `workflowServiceUnreachable: boolean`. Include both in the `sync` inputs and in `deriveTasks`.
  - **Wiring.** Start and stop the monitors in `startPolling` and `stopPolling`, using `loadJobProgressFromSplitProjectFolder` and `reconcileTemporalJobsInSplitProjectFolder`.
    - `hasRunningTask` means any task with status `running`.
    - `hasActiveTemporalTask` means any non-terminal record with `workflow?.backend === "temporal"`.
  - **Services.** Each service applies `workflowStartFailureActions` on a failed start. They are export-service `startTemporal`, speech-service `startQueuedWorkflow` and generation-service `startTemporal`.
  - **Size.** If `rtk wc -l src/editor/services/generation-service.ts` would exceed 600, move `startMockWorkflow` and `startTemporal` into `src/editor/services/generation-workflow-start.ts` first. That is behavior-preserving, with its existing tests still passing.
- [ ] **Verify:**
  - `rtk pnpm vitest run src/editor src/lib/jobs`
  - `rtk pnpm lint`
  - `rtk pnpm test:source-quality`, which includes the 600-line editor policy
- [ ] **Commit:** `feat(jobs): poll job progress, reconcile Temporal jobs, and fail workflows that never start`

### Task 11: Fixtures and e2e

**Owns:** `src/lib/runtime/fixtures/task-fixtures.ts`, `export-fixtures.ts` and their tests, `e2e/editor-tasks.spec.ts`, `e2e/editor-export-tasks.spec.ts`. **After:** Tasks 6 and 10. Task 6 fixes the command names and shapes that the fixtures mirror.

- [ ] **Write failing fixture tests:**
  - **`task-fixtures.test.ts`:**
    - The seeded sample now includes a queued DaVinci XML export with a start request, no run id, and `updatedAt` 10 minutes ago. Its job id is `fixture-export-nle-stale`, its kind `export_nle_xml`, and its input format `davinciFcpxml`.
    - `reconcile_temporal_jobs_in_split_project_folder` fails it with "The workflow never started." and returns `{ project, failedJobIds: ["fixture-export-nle-stale"], serviceReachable: true, detail: null }`.
    - `load_job_progress_from_split_project_folder` returns `[]`.
  - **`export-fixtures.test.ts`:** while a render is pending, `load_job_progress_from_split_project_folder` returns `{ jobId, progress: polls / exportFixtureRenderPolls }`. It returns nothing once the render has completed.
- [ ] **Implement the fixture handlers.** Add the two operations to the task and export fixture maps. Export progress reads the pending renders. The task fixture's handler composes with it, so the export map overrides it. Update the doc comments.
- [ ] **Update the e2e specs:**
  - **`e2e/editor-tasks.spec.ts`:**
    - `expectTaskRows` expects 5 rows.
    - The "DaVinci XML export" row shows "The workflow never started." and a Retry button.
    - The indicator still contains "Transcribing…".
  - **`e2e/editor-export-tasks.spec.ts`:** after "Export video", assert `indicator` has the accessible description `/^Exporting · \d+%$/` before the completion toast. On desktop, the Background tasks row shows `role="progressbar"`, named "Video export progress", with a non-zero value.
  - **Flakiness.** If the progress window is flaky at `exportFixtureRenderPolls = 3`, raise it to 4 in `export-fixtures.ts` and update its comment.
- [ ] **Verify:**
  - `rtk pnpm vitest run src/lib/runtime/fixtures`
  - `rtk pnpm exec playwright test e2e/editor-tasks.spec.ts e2e/editor-export-tasks.spec.ts e2e/editor-mobile.spec.ts`. The mobile spec checks that the top bar still fits with a failed task present.
  - `rtk pnpm check:unused`
  - `rtk pnpm lint`
- [ ] **Commit:** `test(editor): cover job progress and stale Temporal tasks in Background tasks`

### Task 12: Full verification

**Owns:** no source files. Fixes found here go into a `fix(...)` commit that names its files. **After:** Tasks 1–11.

- [ ] **Frontend gate:** `rtk pnpm verify:frontend`. Record every step's result.
- [ ] **Rust suites** (foreground), with `TAURI_CONFIG`, the render runtime root and the compatibility decoder set as in Global Constraints:
  - `rtk cargo test --manifest-path src-tauri/Cargo.toml --lib -- --test-threads=1`
  - `rtk cargo test --manifest-path src-tauri/Cargo.toml --bins -- --test-threads=1`
  - these integration targets, each with `--test-threads=1`:
    - `--test project_action`, `--test project_split`
    - `--test temporal_workflows`, `--test temporal_reconcile`
    - `--test project_export`, `--test render_pipeline`, `--test render_transitions_ges`, `--test render_progress_ges`
    - `--test codex_conversation`, `--test codex_app_server`, `--test codex_mcp_server`
    - `--test xai_generation_provider`, `--test generation_provider`
  - `rtk cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets -- -D warnings`
  - `rtk proxy rustfmt --edition 2021 --check` on every changed `.rs` file
- [ ] **Temporal dev server.** Re-run the Task 7 ignored tests if Tasks 8–11 touched `workflows/`. Otherwise cite Task 7's saved output.
- [ ] **Policy checks:**
  - No new non-test file under 600 lines has grown past it: `rtk wc -l` on every file created in this plan.
  - `rtk git status --short` is clean apart from `output/`.
- [ ] **Record the results** in a short table in the final commit body of Task 13, observed results only. A skipped or ignored test that wasn't run is listed as not run.

### Task 13: Backlog update (after verification only)

**Owns:** `docs/product-backlog.md`. **After:** Task 12 passes.

- [ ] **Update the rows.** Each row changes only if its proof in Acceptance was observed.
  - **VC-027:** `idea` becomes `implemented`. **Next action:** "Record native Linux evidence (workstream 06): edit, save and import during an in-process export without the webview freezing." Name the audit test and the queue-ordering test.
  - **VC-028:** `idea` becomes `implemented`. **Next action:** "Run the Linux desktop smoke Temporal steps (workstream 06). Describe can't prove a dead worker while its workflow is still Running; such tasks stay active until the workflow's own timeouts close it." Name the dev-server run and its output path.
  - **VC-021:** `idea` becomes `implemented`. **Next action:** "Record native evidence of a progressing export pill (workstream 06). Transcription has no worker progress channel. Only xAI video reports provider progress."
- [ ] **Update "Last updated"** to the commit date.
- [ ] **Commit:** `docs(backlog): record responsive project commands, Temporal job reconciliation, and job progress`

  Include the verification table from Task 12 in the body.

## Acceptance

| Gap | Behavior | Proof |
| --- | --- | --- |
| VC-027 | No synchronous Tauri command in `main.rs` reaches a lease or project I/O. | `command_thread_audit` passes, and its synthetic negative case is flagged (Task 4). |
| VC-027 | Project writes return to the async runtime at once and complete in submission order after a render releases the lease. Different projects don't block each other. | The `project::command_queue` unit tests (Task 2) and `project_writes_queue_behind_a_held_project_lease_in_order` (Task 4). |
| VC-027 | Cancelling a render still reaches the render while writes are queued. | Cancellation is requested before queueing, covered by the existing cancel tests in `main.rs` `mod tests`, which pass after Task 4. |
| VC-028 | A failed Temporal start fails its job with "The workflow never started.". | The service tests in Task 10. |
| VC-028 | Closed, missing and never-started workflows fail their jobs with plain reasons. Running workflows and an unreachable server leave jobs untouched, and the editor says "Workflow service unreachable". | The planner unit tests and fake-describer integration tests (Task 5), and the task-record and jobs-slice tests (Tasks 9 and 10). |
| VC-028 | The same behavior works against a real server. | The `-- --ignored` run against the local Temporal dev server, with all three tests `ok` and the output saved to `output/gap-closure-01/temporal-dev-server.txt` (Task 7). |
| VC-028 | Background tasks shows the stale-task failure. | `e2e/editor-tasks.spec.ts` at desktop and phone sizes (Task 11). |
| VC-021 | GES renders report progress readable during the render without the lease, throttled to 500 ms and never decreasing. The snapshot disappears when the render ends. | The `job_progress` and `cancel` unit tests (Task 3), and `render_progress_ges` run with the staged runtime and no `SKIPPED` line (Task 3). |
| VC-021 | xAI video progress is reported. | `xai_generation_provider` (Task 8). |
| VC-021 | The pill and the row show a percentage. Progress never enters undo history or `job.json`. | The task-record, jobs-slice and monitor tests (Tasks 9 and 10), and the `e2e/editor-export-tasks.spec.ts` "Exporting · NN%" assertion (Task 11). |
| All | Gates pass. | `verify:frontend`, the Rust suites in Task 12 and clippy, with observed results recorded in the Task 13 commit body. |

Workstream 06 must still record the native Linux smoke evidence for the Temporal steps, a progressing export pill, and editing during an export. This plan doesn't claim that evidence.
