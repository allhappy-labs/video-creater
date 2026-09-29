# Editor Redesign Gap Closure — Design

Date: 2026-09-16. Status: approved for autonomous execution (the user asked to close every open
gap from the editor redesign and to work without check-ins).

Source of the gap inventory:
- `docs/superpowers/plans/2026-09-13-editor-redesign-handoff.md`
- `docs/product-backlog.md` rows VC-015 to VC-028
- The Linux status review that followed the hand-off

## Goal

Make the redesigned editor fully working on Linux, and back that claim with native evidence from this Linux host. A gap is closed when two things hold:
- the behavior works;
- a test or a recorded native run proves it.

The native run is recorded under `output/` and summarized in the updated hand-off.

## Decisions (made autonomously, recorded here for review)

1. **Scope is everything that can be built and verified on this Linux host.** macOS-only items stay open, with an explicit reason. They are listed under Out of scope.
2. **Native evidence counts only when observed.** A step the smoke skips or can't run is reported as not run, never as passed.
3. **The Codex app-server native run uses the installed `codex` CLI and its existing auth.** Keep it to the smallest flows that prove the contract: one safe edit, and one review edit that is dismissed.
4. **Temporal is exercised against a local dev server.** Use the official `temporalio` container image, or the MIT-licensed `temporal` CLI binary downloaded into `/tmp`. No system install.
5. **Local tools stay under `/tmp`.** Missing Linux tools (gnome-keyring, xdotool, xmllint and similar) are extracted from Ubuntu `.deb` packages into `/tmp/vc-smoke-tools`, without sudo. The license policy still applies: LGPL/MIT/BSD tooling is fine for tests, and nothing GPL may be linked into the app.
6. **Master quality is the highest-quality encode of the chosen format**, not a separate format:
   - H.264/H.265: lower CRF plus a slower preset, or the equivalent bitrate ladder for the encoder in use.
   - WebM: higher-quality VP8/VP9 settings.
   - ProRes (macOS): unchanged.
7. **Exports to a user-chosen folder are recorded as export artifacts with absolute paths.** The restricted reveal command may reveal those exact recorded paths even when they are outside the project folder. It still refuses every unrecorded path.
8. **Result frame capture on Linux uses the Rust canonical frame sampler.** That is `precompose::render_canonical_frame_rgba`, which has already been checked for parity against GES. The AVFoundation-only path stays as it is on macOS. Linux captures are written as PNGs to the same `renders/<jobId>/preview-qa/preview-frames/` layout.
9. **Commands that write the project run off the main thread through one per-project FIFO executor,** so edit order is preserved. The frontend write queue stays as the first line of ordering.
10. **Stale Temporal jobs are reconciled by describing their workflow.** A closed, failed, terminated, timed-out or unknown workflow fails the job with a plain reason. A queued job whose start failed, or which has no run id and is older than a grace period, is failed with "The workflow never started." An unreachable Temporal server leaves jobs untouched, since that is not proof of death, and shows a plain "Workflow service unreachable" detail.
11. **Job progress is an optional `progress` field (0–1) on job summaries.**
    - It is written at most about twice a second.
    - It is bookkeeping, so it is excluded from undo, as jobs already are.
    - Sources: GES render position/duration, transcription worker progress where the worker reports it, and provider progress where the provider API reports it.
12. **Audio clip speed** comes with pitch preservation:
    - GES uses `scaletempo` (LGPL, gst-plugins-good), provided it is in the curated runtime. If it isn't, it is added to the runtime build.
    - The preview uses `playbackRate` with `preservesPitch`.
    - Audio Properties gets a Speed tab.
13. **Detach audio** is a project action. It creates an audio clip from the video clip's embedded audio on an audio track, linked to the video clip, and mutes the video clip's own audio. It is one undo step.
14. **Reverse** is added only if GES/GStreamer can render reversed clips reliably through a precomposed reversed intermediate on this runtime. A feasibility spike comes first. If the spike fails, Reverse stays a documented deviation, with the spike's evidence.
15. **Speed combined with transitions** is fixed and render-verified. That includes the frozen first moments GES showed for sped-up clips. The preview's audio crossfade follows clip speed once audio speed exists.
16. **Agent fidelity:**
    - Each conversation turn records its chat session id in the backend history.
    - MCP undo cancels the batch's running generations, as editor undo does.
    - Status or output updates on generated assets that the batch did not create stop blocking undo, because they are bookkeeping for those assets.
    - Undo snapshots are stored compressed, with a retained-size cap in addition to the 20-entry cap.
17. **NLE interchange:**
    - FCPXML dips use a Fade To Color transition, with a verified effect uid.
    - Wipes export as real wipe transitions in XMEML ("Wipe") and FCPXML where an effect id can be verified from public reference exports. Otherwise they stay cuts with a note.
    - Non-primary FCPXML lanes are written as connected secondary storylines, so their transitions survive.
    - Evidence is DTD/schema validation of the exported XML. A real Premiere or Resolve import is out of scope (no apps on Linux).
18. **The flattened-group preview gap is closed.** Lower-track clips covered by a flattened frame are counted as covered.
19. **`codex/context.rs` is split into modules under 600 lines,** with no behavior change.
20. **Lottie precompose with transition handles is render-verified** with the Lottie worker on Linux.

## Workstreams

Each workstream becomes one implementation plan under
`docs/superpowers/plans/2026-09-16-gap-closure-NN-*.md`.

| # | Workstream | Gaps |
| --- | --- | --- |
| 01 | Responsive backend and job reliability | VC-027 main-thread writes, VC-028 stale Temporal jobs, VC-021 job progress |
| 02 | Export completeness | VC-019 destination / name / fps / Master, VC-020 artifacts, save-range naming, retry quality |
| 03 | Agent fidelity and Linux result frames | VC-023 frames on Linux, VC-022 sessions / MCP undo cancel / non-batch asset status / compact snapshots, VC-026 context split |
| 04 | Media editing features | VC-017 audio speed, VC-018 detach audio, Reverse (spike first), VC-015 speed+transitions, flattened-group preview, Lottie handles |
| 05 | NLE interchange | VC-024 dips, wipes, secondary storylines, audio lanes, DTD validation |
| 06 | Native Linux evidence | packaged `.deb` smoke, Temporal smoke steps, MCP sidecar self-tests, Secret Service credentials, denoise re-run, GTK-level Ctrl+Z native menu check, native Codex app-server run of AI flows 1 and 2 |

Workstream 06 runs last, after 01–05 have landed, so the evidence covers the final code.

## Architecture notes

- **Rust/TS lockstep.** New project actions (audio speed, detach audio, reverse) land in one commit together with:
  - the TS union member,
  - `applyProjectActionLocally`,
  - the Codex schema,
  - MCP tool support,
  - the risk classification (safe, matching the existing clip edits).
- **Job progress.** Emitters update the job through the existing job-status update path. Polling already merges job state without touching history.
- **FIFO executor.** A small `ProjectCommandQueue` keyed by the canonical project dir. Commands submit closures and await the results. Renders continue to take the storage lease inside their closures, so writes queue behind the render instead of freezing the UI.
- **Reveal.** The allowed-path rule adds recorded absolute export artifact paths. Everything else stays the same.
- **Canonical capture.** A new Linux path in `capture_canonical_preview_frame_in_split_project_folder` renders RGBA via the sampler, encodes a PNG with the existing image dependencies, and records the capture job exactly as today.

## Testing and evidence

- **Per workstream:** TDD, unit and integration tests, `verify:frontend`, the Rust suites touched, and the GES-gated tests with `VIDEO_CREATER_RENDER_RUNTIME_ROOT`.
- **Native evidence (workstream 06).** Each item records its output folder and its observed result in the hand-off. Anything that can't run is stated with the missing prerequisite.

## Out of scope (cannot be built or verified on this Linux host)

- VC-016 native AVFoundation transitions: macOS-only exporter. The GES fallback works.
- macOS visual baseline refresh and the macOS ⌘Z single-undo check: they need a Mac.
- Opening exported XML in Premiere Pro or DaVinci Resolve: the apps aren't available. DTD/schema validation is the substitute evidence.
- Paid generation provider end-to-end runs. Mock providers and the recorded provider tests stay the evidence; this is not a listed gap.
