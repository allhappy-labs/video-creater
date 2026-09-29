# Backend architecture fixes — implementation evidence

Date: 2026-09-12  
Base: `19b59f347304b38f3a0d4ced29a9496613afe02a`  
Implementation through: `997fde80`
Branch: `fix/backend-architecture-review`

This report maps every finding in the 2026-09-11 backend architecture review to the implemented boundary and regression evidence. All eight prioritized findings and five supplemental findings are addressed. Each of Tasks 1–4 passed its scoped independent Sol spec and quality review after its fix rounds. The final whole-branch review recorded one stale-revision result defect in `task-5-final-review.md`; commit `997fde80` addressed it, and the scoped repair review in `task-5-rereview-1.md` passed both spec compliance and code quality with no findings.

## Finding coverage

| Review finding | Implemented boundary | Regression evidence |
| --- | --- | --- |
| 1. Validate the complete model before deriving sidecar paths | Split-package writes validate every filename-derived ID as one safe component and verify dynamic destinations remain inside the project before staging. | `project::split::tests` covers every unsafe sidecar ID and symlink containment. |
| 2. Make action commits and snapshot replacement atomic | Existing packages use bounded metadata journals with staged promotion, rollback, open recovery, file and directory durability barriers; initial creation uses sibling package promotion. | Split tests cover mid-write failure, committed and uncommitted recovery, post-promotion durability failure, restore failure, and bounded work with bulk artifacts. |
| 3. Replace unversioned whole-project saves | Canonical project and manifest carry `contentRevision`; snapshot replacement uses expected-revision CAS and preserves newer worker-owned records by stable key. The central split save returns the exact canonical project it committed together with its write report. Matte creation, split import, media analysis, preview comparison, and agent undo return that durable project; the frontend analysis merge retains the newest revision. UI undo/redo and Codex saves adopt canonical revisions and handle conflicts. | Split stale-snapshot and same-key worker-record tests; central save, matte, split-import, analysis, preview-comparison, and agent-undo regressions compare the returned project with an immediate reload and perform a following CAS save. The editor regression carries the analysis revision through the following queued action and Codex request. Task 5 also fixed the packaged settings acceptance caller to send its current revision. |
| 4. Scope render identity to project and attempt | Render registration and recovery use canonical project root, project ID, job ID, and explicit attempt ID; duplicate active registrations fail. | Render cancellation tests cover cross-project identity, duplicate registration, locators, and unregister safety; app command and frontend render tests cover attempt propagation. |
| 5. Make cancellation and completion competing outcomes | Atomic attempt lifecycle grants one terminal owner, rejects repeated terminal writes, signals before long leases, propagates through graphics/review, and quarantines cancelled output before publication. | Render cancel/project-export suites cover race arbitration, pre-lease and late-stage cancellation, cleanup failure, terminal revision preservation, and failure-before-cancel. |
| 6. Share a bounded process supervisor | Shared supervisor drains capped stdout/stderr, bounds stdin and the entire parent deadline, supports cancellation, and terminates/reaps Unix process groups. Compatibility and app-server use the supervised boundary. | Seven supervisor tests cover noisy streams, oversized frames/stdin, hangs, cancellation, and a TERM-ignoring descendant. Graphics and project-export checks cover callers. |
| 7. Implement the app-server turn lifecycle | The stdio pump separates responses, notifications, and server requests; safely denies unsupported approval requests; aggregates terminal proposal output; and applies one deadline/cancellation token across initialize, thread, and turn phases. | Fourteen unit tests, six app-server integration tests, eight Codex render-pipeline tests, and eleven Temporal Codex tests passed. The pinned Codex 0.141.0 initialize-only handshake passed without a paid turn. |
| 8. Put cleanup and project writes under one ownership contract | A stable sibling Unix `flock` backs the reentrant project lease across processes and package renames. Cleanup takes the storage coordinator before the project lease and revalidates under both. | Cross-process parent/child lease test and split/command coordination tests cover cleanup, settings, render recovery, media, agent, Temporal, and app-server writers. |
| Supplemental: bounded provider downloads | One shared boundary caps declared and observed response sizes, streams to unique same-directory temporary files, syncs before atomic publication, and removes partials on cancellation/error. Encoded media paths enforce envelope and decoded limits. | Eight shared download tests and local provider fixtures, including ten Google tests in Task 5. No live provider calls were made. |
| Supplemental: MCP project scope | Server-owned canonical launch root is injected into mutation dispatch; all accepted mutating method aliases reject a caller-supplied cross-root `projectDir`. | Scoped MCP integration test passed across the accepted aliases, including same-root success. |
| Supplemental: proposal workspace writes | Codex proposal sessions use a read-only sandbox and `approvalPolicy: never`; unsupported server requests receive explicit denials. | App-server sandbox and protocol-pump unit tests plus fake-child integrations. |
| Supplemental: retry URL policy | Retry derives provider policy from the same leased canonical snapshot as revision/provider/model/output/path, revalidates before publication, allows private origins only through exact configured capability, validates and pins each redirect hop, disables proxies, and scopes credentials separately. | Shared redirect/private-address tests, Google credential-scope test, provider-change race test, and fourteen retry command tests from Task 4. |
| Supplemental: search freshness | Stored indexes include a digest of serialized canonical project content; missing legacy digests trigger rebuild. | Search regression changes sidecar content without changing count/timestamp and verifies stale index rejection. |

## Task 5 integration correction

The packaged settings acceptance runner called the revised `save_split_project_to_folder` IPC without `expectedRevision`. Its generic bridge type bypassed the typed wrapper and therefore bypassed TypeScript's argument check. The runner now captures its acceptance project, sends `project.contentRevision ?? 0`, and has a regression asserting that exact IPC payload. The test failed before the caller change and passed afterward. This flow performs one save before restarting, so there is no later in-memory revision to update.

The first final whole-branch review (`task-5-final-review.md`) found that `save_split_project` advanced the revision only in a private clone while returning only a write report. Result-producing callers consequently returned a stale input project. Commit `997fde80` makes the central save API return one `ProjectActionWriteResult` containing both the canonical committed project and report, and canonicalizes the returned split representation before writing. Every project-returning caller consumes that value. Report-only and unit-returning callers explicitly select or discard the report, while non-split import retains its original storage path. The scoped repair review (`task-5-rereview-1.md`) passed both spec compliance and code quality with no findings.

## Fresh integrated verification

All Cargo commands used `CARGO_TARGET_DIR=/home/olhapi/projects/video-creater/src-tauri/target`, `CARGO_BUILD_JOBS=2`, `--offline`, and serialized execution. App commands additionally used `TAURI_CONFIG='{"bundle":{"externalBin":[],"resources":[]}}'`. Free disk was checked between build-heavy lanes. It remained at least 14 GiB during the original integration pass and at least 12 GiB during the final-review repair, both above the 10 GiB stop threshold.

- No-feature Rust library suites passed: split 33; cross-process mutation parent and child; search freshness 1; render cancellation 9; project export 8; process supervisor 7; app-server 14; shared download 8.
- Graphics cancellation passed 2 tests with `graphics-render`.
- The `app-runtime,gpu-render` application check passed. This is the fresh replacement for the Task 4 check invalidated by concurrent incremental-cache removal.
- Codex caller integrations passed: app-server 6, render-pipeline Codex 8, Temporal Codex 11, and scoped MCP 1.
- Google provider local fixtures passed 10; the `project_export` integration target compiled with `graphics-render`.
- Relevant frontend suites passed 530 tests: App 31, project library 69, editor workspace 330, agent panel 74, settings acceptance runner 19, and settings visual fixtures 7.
- `pnpm exec tsc --noEmit` and `pnpm exec tsc -p tsconfig.node.json --noEmit` passed.
- All Rust files changed from the review base passed `rustfmt --check --edition 2021 --config skip_children=true`; `git diff --check` passed.
- `node scripts/source-quality-policy.mjs` and `node scripts/check-tooling-source.mjs` passed; the latter checked 42 files.

### Final-review fix round 1

- Focused regressions passed for the central save result, matte creation, split import, media analysis, preview comparison, and agent undo. Each backend result equals an immediate canonical reload and its returned revision succeeds as the expected revision for the next snapshot save.
- The focused editor regression passed and verifies that a committed media-analysis revision survives the frontend merge, feeds the following queued project action, and reaches the Codex start request.
- The `app-runtime,gpu-render` application check passed after the API change. Selected integration targets (`project_split`, `project_export`, `codex_app_server`, `codex_mcp_server`, and `temporal_workflows`) compiled together with those features. This compile surfaced and then verified updates to all direct report-field consumers in `project_split`.
- The complete affected editor suite passed 331 tests, both TypeScript checks passed, and changed-file formatting, diff hygiene, source-quality policy, and tooling-source checks passed. Free disk remained at least 12 GiB during this round.

The full app bin test target ran 114 tests: 113 passed and `tests::active_project_session_updates_only_after_successful_split_project_commands` failed because its mock app omits managed `SettingsAcceptanceStateHolder`. Git blame and the base source show both the command's state requirement and this test fixture predate the review base; this branch did not change that dependency. The changed save, cleanup, retry, render-cancel, and Codex command tests passed within the same target. The unrelated fixture was preserved.

The repository-wide `cargo fmt --all -- --check` remains blocked by an existing formatting difference in `src-tauri/src/settings/acceptance.rs`, which is unchanged from the review base. Changed-file formatting passed as recorded above.

## Limits

- Verification ran on Ubuntu. It does not establish macOS AVFoundation/AppKit behavior, Xcode builds, signing, packaging, physical-device behavior, or production deployment.
- Unix process-group tests establish Linux descendant termination behavior; macOS teardown remains unverified here.
- The pinned Codex check performed initialization only. No paid Codex turn, live provider request, private-media upload, or external service mutation was performed.
- The public retry IPC still has no cancellation operation identifier. Its shared downloader is cancellation-safe and tested, but user-triggered retry cancellation would require a separate API lifecycle.
- The final whole-branch review and scoped repair rereview establish coverage of the thirteen recorded findings; they do not establish absence of unrelated defects.
