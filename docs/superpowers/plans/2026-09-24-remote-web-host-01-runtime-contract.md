# Remote Web Host 01 — Runtime Contract and Service Extraction Plan

**Goal:** Create one typed, Tauri-independent application service and classify every frontend
operation before adding a network listener.

**Spec:** [Remote Web Host Design](../specs/2026-09-24-remote-web-host-design.md)  
**Depends on:** none  
**Feeds:** plans 02–04

## Constraints

- Preserve operation names and response shapes while moving behavior.
- Rust remains the sole authority for projects, revisions, validation, jobs, credentials, agents,
  render plans, logs, and artifacts.
- Do not add HTTP, authentication, or remote exposure in this phase.
- Keep new source files below 600 lines and split by domain.
- Codex proposals remain structured and Rust-validated; canonical files are never agent-written.
- Run only fixture/fake agent tests; no paid model turn is part of this plan.

## Task 1 — Generate and enforce the operation inventory

**Create:**

- `src-tauri/src/app_service/operation.rs`
- `src-tauri/tests/app_service_operation_inventory.rs`
- `docs/development/remote-operation-inventory.md`
- `scripts/remote-operation-inventory-policy.test.ts`

**Modify:** `src-tauri/src/lib.rs`, `package.json`

- [ ] Extract the Tauri handler list and frontend `backendRequest`/`backendListen` literals into a
  checked inventory.
- [ ] Classify every operation as `remote`, `desktop-only`, `internal`, or `removed` and name its
  auth scope, mutation class, project/revision requirements, maximum request size, and browser
  replacement when desktop-only.
- [ ] Fail the policy test when a frontend operation or Tauri handler is absent from the inventory.
- [ ] Mark native dialogs, reveal-in-folder, native menu, and OS notification operations
  desktop-only; do not pretend they work remotely.
- [ ] Run `rtk pnpm test:source-quality` and the focused Rust inventory test.

**Commit:** `docs(remote): inventory the host operation boundary`

## Task 2 — Define typed service context and errors

**Create:**

- `src-tauri/src/app_service/mod.rs`
- `src-tauri/src/app_service/context.rs`
- `src-tauri/src/app_service/error.rs`
- `src-tauri/src/app_service/events.rs`

**Modify:** `src-tauri/src/lib.rs`

- [ ] Add `VideoCreaterService`, containing explicit stores and coordinators currently installed as
  Tauri-managed state.
- [ ] Add `RequestContext` with client kind, request ID, project ID, expected revision, authorization
  scopes, and optional editor-lease token. It must not contain raw credentials.
- [ ] Add a stable error union with codes for invalid input, unavailable capability, unauthorized,
  forbidden, revision conflict, lease conflict, not found, busy, cancelled, and internal failure.
- [ ] Add a backend-neutral `EventSink` trait and ordered event envelope. A fake sink records events
  deterministically.
- [ ] Prove serialization contains no internal error chain or absolute path.
- [ ] Run focused library tests and `rtk cargo fmt --manifest-path src-tauri/Cargo.toml --all -- --check`.

**Commit:** `feat(runtime): add the shared application service contract`

## Task 3 — Move the project read/write vertical slice

**Create:**

- `src-tauri/src/app_service/projects.rs`
- `src-tauri/tests/app_service_projects.rs`

**Modify:** `src-tauri/src/main.rs`

- [ ] Move split-project create/load/save/validate, project action application, preference-neutral
  project settings, agent-session history reads, and job-progress reads behind typed service methods.
- [ ] Keep Tauri commands as thin adapters with their existing names and payloads.
- [ ] Require expected revision on service mutations even where a legacy Tauri wrapper supplies a
  compatibility default; record those exceptions in the inventory.
- [ ] Prove the same project fixtures produce byte-equivalent canonical files through direct service
  calls and Tauri test adapters.
- [ ] Prove a stale revision has no partial write and emits no success event.

**Commit:** `refactor(runtime): route project operations through shared services`

## Task 4 — Move media, jobs, export, and agent vertical slices

**Create:**

- `src-tauri/src/app_service/media.rs`
- `src-tauri/src/app_service/jobs.rs`
- `src-tauri/src/app_service/exports.rs`
- `src-tauri/src/app_service/agents.rs`
- matching focused integration tests under `src-tauri/tests/`

**Modify:** `src-tauri/src/main.rs`

- [ ] Move one complete user journey at a time: inspect/import, timeline preview, transcription,
  generation, render/export, task cancellation/recovery, and agent conversation/apply/undo.
- [ ] Preserve the project command queue, mutation/artifact leases, cancellation ownership,
  credential resolution, and durable job records.
- [ ] Route settings-operation and job progress through `EventSink`, with the Tauri sink calling
  `AppHandle::emit`.
- [ ] Keep Codex app-server and Claude transport private inside the agent service. The method returns
  only the existing validated proposal/result contract.
- [ ] Prove an edit proposal contains a valid EDL before visual layers and that render completion
  still validates duration, streams, timing, artifacts, and logs.

**Commit:** `refactor(runtime): share media job export and agent services`

## Task 5 — Make Tauri a contract adapter and close the inventory

**Modify:** `src-tauri/src/main.rs`, relevant command tests, inventory documentation

- [ ] Replace direct orchestration in Tauri commands with input decoding, typed service calls, and
  output mapping.
- [ ] Confirm every inventory row has an implemented service method or explicit desktop-only path.
- [ ] Add a policy test preventing new business logic in Tauri wrapper modules.
- [ ] Run targeted Rust tests, `rtk pnpm lint`, `rtk pnpm test`, and the existing Tauri command tests.
- [ ] Record baseline versus final operation counts and any intentionally retired command.

**Commit:** `refactor(runtime): complete the Tauri service adapter`

## Exit Criteria

- No network listener exists yet.
- Direct service tests can execute the complete fixture edit-to-render journey without Tauri.
- Existing Tauri-facing contracts and frontend tests remain green.
- Every operation has an explicit remote classification.

