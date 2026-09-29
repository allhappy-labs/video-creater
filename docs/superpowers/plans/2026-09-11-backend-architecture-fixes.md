# Backend Architecture Fixes Implementation Plan

> **For agentic workers:** Use superpowers:subagent-driven-development. User authorized implementing every review finding via Sol subagents.

**Goal:** Address all eight prioritized backend findings and five supplemental concerns with tested, integrated fixes.
**Architecture:** Retain Rust-owned project state and existing typed actions. Consolidate canonical commit safety, scoped render lifecycle, bounded subprocess/provider I/O, and project-scoped proposal capabilities.
**Tech Stack:** Rust, Tauri, React/TypeScript, filesystem packages, native worker subprocesses.
**Spec:** docs/reviews/2026-09-11-backend-architecture-review.md (fix directions authorized by user).

## Global Constraints

- All implementers and reviewers use GPT-5.6 Sol at medium effort, no nested subagents.
- Preserve unrelated work; changes live in fix/backend-architecture-review isolated worktree.
- One implementation agent at a time; read-only research/review may run concurrently.
- Rust owns canonical project validation and mutation. No paid requests/private media uploads.
- Keep cancellation signalling out-of-band; never wait for the long render lease before signalling.
- Use Conventional Commits; no pushes, merges, deployment, or release operations.
- Run meaningful targeted regression tests; record blocked checks honestly. No macOS/native packaged claims from Linux.
- Prefix shell commands with rtk; when missing define temporary rtk() { "$@"; } pass-through.
- Reuse /home/olhapi/projects/video-creater/src-tauri/target for Cargo checks; serialize Cargo execution, cap jobs at 2, monitor disk. Do not delete unrelated caches.

### Task 1: Canonical persistence and conflict safety

Own project/split.rs, project/model.rs as needed, search/mod.rs, main.rs save/cleanup boundaries, frontend save callers and corresponding tests. Address findings 1,2,3,8 and supplemental search freshness.
- [x] Add regression coverage for unsafe sidecar IDs, transaction failure recovery, stale snapshots, cleanup/writer coordination and stale search content.
- [x] Validate filename-derived IDs and containment before writes; route initial and existing canonical saves/actions through package transactions without recursive transaction calls.
- [x] Use persisted content revision/CAS or narrow actions so whole-project UI undo/Codex saves cannot overwrite worker changes; adapt all real callers and conflict handling without losing unsaved edits.
- [x] Coordinate cleanup with all participating canonical writers using consistent lock ordering without blocking cancellation; consider process boundaries and document limitations.
- [x] Make stored search freshness depend on indexed canonical content, retaining legacy compatibility via rebuild.
- [x] Run focused tests, format touched files, self-review and commit.

### Task 2: Render identity, lifecycle and worker supervision

Own render_pipeline/cancel.rs, project_export.rs, graphics-related cancellation propagation, precompose/compatibility.rs, main.rs render command sections, project/action.rs terminal updates and tests. Address findings 4,5 and compatibility portion of 6.
- [x] Add tests for same-ID separate-project renders, stale/terminal cancellation, late-stage cancellation, hung/noisy fake workers.
- [x] Scope render registry by canonical project and attempt; reject duplicate active registration and use same identity for recovery.
- [x] Arbitrate cancellation/completion once; prevent rewriting terminal/non-render jobs and propagate cancellation across graphics/review/publication. Preserve retry semantics through explicit new attempts where needed.
- [x] Introduce/reuse bounded subprocess supervision with concurrent capped stdout/stderr drainage, parent deadlines, cancellation and kill/reap descendants. Expose helper for Task 3.
- [x] Run focused tests, format touched files, self-review and commit.

### Task 3: Agent protocol, project scope and proposal sandbox

Own codex/app_server.rs, mcp_server.rs, tools.rs/context.rs relevant boundaries, main.rs Codex sections and tests. Address finding 7, app-server portion 6, supplemental MCP and workspace-write concerns.
- [x] Inspect pinned Codex 0.141.0 launch/schema locally and test handshake without paid turn. Use official docs only if necessary.
- [x] Implement bounded protocol pump for response/event/server-request routing, terminal result aggregation, deadline/cancel and safe denial of unsupported approval requests (no silent hang).
- [x] Drain/cap stderr with supervisor support; use read-only proposal sandbox and explicitly scoped project reads/scratch only if needed.
- [x] Bind MCP mutation projectDir to canonical launch root, preserving intended tools via server-owned context; regression test cross-project denial and allowed same-root actions.
- [x] Run fake-child protocol tests and focused integration tests; format, self-review and commit.

### Task 4: Provider download and retry hardening

Own generation adapters/shared download helper, main.rs retry download sections and relevant tests. Address supplemental unbounded downloads and retry URL policy.
- [x] Add capped streaming atomic downloads for all adapters/retry (also in-memory/base64 paths where applicable); reject declared or observed oversized bodies, clean partials on cancel/error.
- [x] Validate retry URL provenance/scheme/redirects and reject unintended local/private targets while retaining explicit configured self-hosted provider endpoints. Do not introduce unreviewed secret logging or arbitrary provider allowlists.
- [x] Test local fixtures for redirects, limits, cancellation cleanup and valid download behavior; no external provider calls.
- [x] Run focused tests, format, self-review and commit.

### Task 5: Integrated review and verification

- [x] Independent Sol spec/quality review after each task; fix concrete defects before dependent work.
- [x] Run integrated Rust targeted suites, frontend checks for caller changes, formatting and source-policy checks.
- [x] Run final Sol review with full branch diff and all findings coverage; address all material issues.
- [x] Update implementation evidence mapping each finding to fix/tests and disclose platform limits; retain branch/worktree for user review.
