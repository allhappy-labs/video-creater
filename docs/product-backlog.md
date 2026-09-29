# Video Creater Product Backlog

Last updated: 2026-09-24

This is the canonical product-direction backlog for Video Creater. It tracks
what is missing, planned, active, blocked, implemented, and verified across the
complete media-to-finished-video workflow.

The [Palmier parity tracker](parity.md) remains the detailed
reference-specific audit and evidence history. This backlog uses Palmier,
Descript, and Remotion as inspiration, but it is organized around Video
Creater's product loop rather than another product's controls.

## North Star

Turn source media and an intended story into a source-grounded, editable,
reviewable, and production-ready video with an agent.

The first audience focus is a transcript-first solo talking-head creator. That
is the first complete workflow, not the limit of the product. The underlying
model must continue to support multiple sources, visual media, generated
elements, conventional timeline editing, and programmable compositions.

## Core Product Loop

1. **Add media.** Import one or more video, audio, image, or generated sources.
2. **Understand the sources.** Extract transcript, word timing, speakers,
   scenes, shots, visible content, audio events, quality signals, and meaningful
   moments.
3. **Define or discover the story.**
   - **Make this story:** follow a user brief.
   - **Find the story:** propose source-grounded narratives from the media.
4. **Build the story plan.** Produce beats, selects, reasons, alternatives, and
   a real EDL using canonical source ranges.
5. **Compose and edit.** Arrange clips, audio, captions, titles, graphics,
   effects, and reviewed generated elements.
6. **Review and refine.** Inspect the actual composition, source evidence, and
   changes through transcript, timeline, viewer, or agent. Keep edits
   reversible.
7. **Produce the final video.** Validate, render, review, and export.

## Product Principles

- **Source-grounded before generative.** Build a real EDL from canonical source
  ranges before adding captions, graphics, effects, or generated media.
- **Transcript first, timeline available.** Transcript editing should be the
  approachable default for spoken content; the timeline remains the precision
  surface and source of temporal truth.
- **Agents propose structured work.** Agents do not directly mutate canonical
  project files. Trusted application code validates, classifies, and applies
  structured actions.
- **Safe work can feel fast.** Objective, local, reversible edits may apply
  automatically as one atomic revision with facts, highlights, preview, and
  Undo.
- **Consequential work pauses.** Narrative rewrites, broad timeline
  replacement, generated media, network/provider work, rendering, exporting,
  and paid actions require review.
- **Preview the truth.** Review surfaces must derive from the changed canonical
  project rather than fabricated proposal mockups.
- **Local-first where practical.** Transcription, analysis, semantic retrieval,
  and deterministic editing should work without uploading private source
  media.

## Status Model

| Status | Meaning |
| --- | --- |
| `idea` | Worth retaining, but not yet shaped enough to plan. |
| `discovery` | Product or technical questions are still being resolved. |
| `planned` | Approved behavior and an executable plan exist. |
| `in progress` | Active implementation or verification is underway. |
| `blocked` | Progress requires a named external dependency or decision. |
| `implemented` | Meaningful production behavior exists, but the full acceptance boundary is not proven. |
| `verified` | The intended workflow passed at the stated real product boundary. |
| `deferred` | Intentionally postponed; retain the reason and revisit trigger. |

`implemented` and `verified` are deliberately separate. Unit tests, browser
fixtures, or mocked Tauri state do not prove a packaged native workflow.

## Priority Model

- **P0:** required to complete or trust the core product loop.
- **P1:** materially expands creator leverage after the core loop works.
- **P2:** supporting polish, scale, or optimization.

## Ranked Backlog

Rows cite run folders under `output/` as their evidence boundary. `output/` is git-ignored, so what
survives depends on where it was written. Checked on 2026-09-24:

- **Gone with their per-plan worktrees, and not re-inspectable:** `output/gap-closure-01/`,
  `output/gap-closure-06/`, `output/gap-closure-07/`, `output/linux-release/` and
  `output/nle-xml-validation/`. No `gap-closure-0*` directory exists anywhere on this machine, and
  `git worktree list` no longer names `vc-gap-06` or `vc-native` (it holds `main` plus three
  unrelated `t3code` checkouts). The recorded results stand; the artifacts do not, and re-inspecting
  any of them means re-running.
- **Still on disk, because they are test output rather than retained artifacts:**
  `output/linux-result-frames/` and all nine `output/transition-preview-parity/*/` reports.
  `render_transitions_ges::result_frames` and `::parity` rewrite them on every run of that always-on
  target, and last did on 2026-09-24. Any row citing them can be re-evidenced by re-running one test
  target.
- **Still on disk, untouched since they were written:** the 2026-09-15 `output/linux-desktop-smoke-*`
  folders.

| ID | Priority | Status | Capability | User outcome | Inspiration | Next action |
| --- | :---: | --- | --- | --- | --- | --- |
| VC-001 | P0 | `implemented` | Proposal-focused agent workspace | Ask for an edit, see trustworthy progress, then receive an applied result or a concise review decision. | Palmier, Descript, original | AI flows 1 and 2 **passed on a packaged Linux app on 2026-09-18** with the Claude backend (`agentBackend: "claude"`, `--model haiku`, `output/gap-closure-07/agent-flows-claude/`): the safe edit auto-applied as "Applied to Timeline 1" (`0:08 → 0:07`) **with a result frame in the card** (one `img`, `naturalWidth` 1920; the PNG is retained), Show changes highlighted one clip, Undo returned the saved timeline to its pre-edit state, and a track removal waited for review and left the timeline unchanged after Dismiss — $0.0843 for both turns. **Next action:** none blocking. Claude is the preferred backend as of 2026-09-18 (Automatic resolves to Claude whenever it is ready), so the Codex-backed run of the same two flows is **optional confirmation, not evidence this capability waits on**: the flows are proven end to end on a packaged Linux app through the Claude backend, on the user's own subscription, with no API key and nothing bundled. Run it if and when that account has headroom. For the record, the 2026-09-17 Codex attempt failed twice: the pinned `@openai/codex` 0.141.0 sidecar rejected the `gpt-5.6-sol` model selected in the user's Codex config ("requires a newer version of Codex"), and the retry with a test-only `gpt-5.5` override hit the account's usage limit (until 2026-09-19 08:10). Since `635e9a37`, a turn checks the thread's model against the app-server's `model/list` and runs on the server's default model (`gpt-5.5` for 0.141.0) when the configured model isn't listed; if listing fails, it retries once after the "requires a newer version of Codex" error. The agent settings status then says "Your Codex config selects a model this app's Codex can't use; using gpt-5.5." A no-turn probe of the real 0.141.0 sidecar confirmed the switch (thread model `gpt-5.6-sol`, turn model `gpt-5.5`). Re-running `--agent-flows --agent-backend codex` would confirm that path; it is optional. The sidecar pin was not bumped, and that too is optional: `@openai/codex` 0.154.0 lists `gpt-5.6-sol`, and the app-server methods the app uses changed only additively, but it adds a `codex-code-mode-host` helper and has not been checked with a real turn. |
| VC-002 | P0 | `implemented` | Unified media understanding | Understand what was said, shown, and heard at exact source times without switching between disconnected analysis tools. | Descript, original | Productize transcript, timing, speakers, shots, visual semantics, audio events, quality, and search results as one inspectable source-intelligence model. |
| VC-003 | P0 | `implemented` | Make this story | Turn a user brief into source-backed beats, selects, an EDL, and a composed draft. | Palmier, original | Join the existing multi-source EDL builder, agent proposal path, story beats, and composition review into one complete product workflow. |
| VC-004 | P0 | `discovery` | Find the story | Receive several credible narratives discovered from uploaded media before committing to an edit. | Original | Define story-candidate output, ranking, evidence, diversity, user selection, and the boundary between deterministic retrieval and model judgment. |
| VC-005 | P0 | `idea` | Story map | See beats, source selections, reasons, alternatives, omissions, and resulting timeline sections in one editable structure. | Descript, original | Design the smallest story-map representation that can round-trip to a canonical EDL without becoming a second timeline. |
| VC-006 | P0 | `idea` | Transcript-native editing | Edit spoken video as text while preserving exact media alignment and professional timeline control. | Descript | Define canonical transcript operations for delete, restore, reorder, retake selection, pause shortening, and timeline navigation, including alignment-failure recovery, on the Captions transcript layout contract: word buttons carry `data-word-index` and `selection.transcriptRange` holds the selected word range. |
| VC-007 | P0 | `implemented` | Review truth and recovery | Know what changed, where, why, and how to inspect or undo it. | Descript, original | All six pieces of the former next action ship today, re-read against the code on 2026-09-24. **Structured impact facts:** the closed `ResultFact` union and `resultFacts()` in `src/lib/agent/result-facts.ts`, rendered as the card's `Facts` chip list (`panels/ai/card-parts.tsx`), for applied and review cards alike; the chips never render an internal id (`result-facts.test.ts`). **Affected IDs and ranges:** `CodexProposalImpact` carries `affectedItemIds`, `affectedRanges` and `previewTimestamp` from Rust (`src-tauri/src/codex/conversation/impact.rs`) on both the prepared proposal and the apply result, edge-precise rather than whole-clip (`codex_conversation::prepare`). **Post-apply preview frames:** `panels/ai/result-frames.tsx` captures up to four canonical frames at the impact timestamps, serialized and cached, with an "Earlier version" staleness badge and an Open viewer / Retry preview fallback. **Show changes:** the card button drives `useShowChanges` (`panels/ai/show-changes.ts`) into `setHighlights`, which outlines items as `clip-highlight` and bands ranges, then seeks and scrolls; highlights clear on the next undoable edit, on undo and on Esc (`timeline/timeline-highlights.test.tsx`). **Deterministic Undo validity:** `ProjectAgentUndoOutcome::{Undone,Conflict,Unavailable}` in `src-tauri/src/project/split/agent_batch.rs`, decided by a versioned content hash (`AGENT_CONTENT_HASH_VERSION = 4`) that deliberately excludes job, render-report and export bookkeeping so preview plumbing cannot invalidate an Undo, with plain-language refusals on a disabled control (`codex_conversation::{undo,undo_bookkeeping}`). **Concise failure recovery:** `panels/ai/failure-card.tsx` with four short statuses, a retry gated to the latest retryable turn, and `agentUnavailable` short-circuiting to the missing-agent state. Observed natively on a packaged Linux app on 2026-09-18 for the applied path: result frame, Show changes highlight and Undo (see VC-001, VC-022, VC-023). **Next action:** two narrow gaps, both verified open. (1) An AI card's **Undo stays enabled until it is pressed** — `undo.available` is set `true` on apply in `src/editor/store/agent-turn-actions.ts` and only flipped after the backend answers `conflict`/`unavailable`, so a user who edits after an apply learns the Undo is refused only by clicking it. The Rust answer is authoritative and correct; the card just does not pre-disable. The staleness watcher `ResultFrames` already uses is the pattern to reuse. (2) The impact carries **no affected track ids** — Rust knows the track but drops it, so `highlightRangesFor` re-derives the spanned tracks and falls back to banding *every* track for a range whose affected items are gone (a removed clip). Then take the failure-recovery and review paths to the native boundary, which 2026-09-18 did not exercise. |
| VC-008 | P0 | `blocked` | Packaged production reliability | Trust that preview, generation, export, credentials, cancellation, retry, and recovery work outside the developer machine. | Palmier | Complete Developer ID notarization/stapling and retain clean-Mac packaged evidence for the current production workflows. |
| VC-009 | P1 | `idea` | Objective automatic cleanup | Produce a cleaner first cut without reviewing every obvious mechanical edit. | Descript | Design one atomic cleanup revision for confirmed retakes, fillers, long pauses, local audio cleanup, and caption timing; uncertain detections remain suggestions. |
| VC-010 | P1 | `implemented` | Story-aware visual composition and generation | Add titles, captions, graphics, effects, and generated media because the story needs them, not as disconnected decorations. | Palmier, Remotion | Connect story beats and reviewable intent to existing graphics, HyperFrames, effects, and provider workflows; preserve explicit cost and network approval. |
| VC-011 | P1 | `discovery` | Programmable compositions | Reuse parameterized motion designs that agents and humans can safely instantiate and edit. | Remotion | Decide the component contract, parameter schema, sandbox, deterministic preview/render boundary, versioning, and conversion to canonical timeline/render state. |
| VC-012 | P1 | `idea` | Story variants | Create short, long, platform-specific, or alternative narrative cuts from one analysis without duplicating source media. | Descript, Remotion | Define branch identity, shared analysis, inherited edits, divergence, comparison, and export naming. |
| VC-013 | P1 | `idea` | Reusable workflows | Run repeatable creator processes without hiding steps, approvals, or cost. | Descript, Remotion | Define structured recipes with typed inputs, ordered operations, approval gates, cost bounds, and acceptance checks. |
| VC-015 | P1 | `implemented` | Clip transitions | Put a crossfade, dip to black, dip to white, or wipe on a cut, see it in preview, and get it in the render. | Original | Retain packaged native evidence of transitions on retimed clips. Not run in the 2026-09-18 native run: the Linux desktop smoke has no transition step, so this needs one. Gap closure 04 closed the speed follow-ups: GES sped-up clips start at their in-point and render like canonical frames through transitions (a3fa742a; `output/transition-preview-parity/speed-transitions/`), the preview audio crossfade follows clip speed (1cd6ded6), and lower-track clips under a flattened group are left to its prepared frames (b531a1e0). Lottie clips keep their transition handles (58074ae5; `output/transition-preview-parity/lottie-handles/`). |
| VC-014 | P2 | `idea` | Quality scorecard | Catch pacing, repetition, silence, caption, visual-variety, audio, and source-coverage problems before export. | Original | Identify deterministic checks first, then separate advisory model judgments with evidence and confidence. |
| VC-016 | P2 | `idea` | Native AVFoundation transitions | Deliver macOS final exports with transitions without falling back to GES. | Original | Compose transitions in the AVFoundation exporter, which rejects render plans that contain transitions so the render falls back to GES. |
| VC-017 | P2 | `implemented` | Audio clip speed | Change an audio clip's speed the way visual clips can. | Original | Retain native WebKitGTK and packaged evidence that preview audio keeps pitch at non-1× speed (workstream 06); macOS AVFoundation retimed audio falls back to GES and the macOS `audiofx` plugin addition is unverified. Implemented in gap closure 04: reviewed `scaletempo` (0d6a4e0a), the `updateAudioClipSpeed` action (9bfb8ae2), pitch-preserving retimed audio intermediates for GES (ba6a8ca5), preview playback rate (1cd6ded6) and the audio Speed tab (128622b4). |
| VC-018 | P2 | `implemented` | Detach audio | Split a video clip's sound into its own audio clip. | Original | Retain packaged native evidence of Detach audio and its single Undo (workstream 06). Implemented in gap closure 04: the `detachAudio` action (8a15ece7), Detach audio in the timeline menu, clip tools and Audio tab (ee46ffeb), and GES render plus XMEML/FCPXML export of the detached sound exactly once (563af295). |
| VC-019 | P2 | `implemented` | Export destination and output settings | Choose an export's folder, file name, frame rate, and Master quality. | Original | Implemented in gap closure 02: the Export popover saves to the project `exports/` folder or a chosen folder under the chosen name without overwriting (`Name (2).mp4`), with a frame-rate override and Master (the highest-quality encode of MP4 and WebM), in process and through Temporal. Proof: GES renders `in_process_export_*` and `in_process_master_export_renders_h264_and_webm`, the Temporal writer tests `temporal_export_media_writer_*`, and `e2e/editor-export-destination.spec.ts`. Still unproven at the native boundary: a Linux desktop smoke export to a chosen folder with Master and a frame-rate override (workstream 06); H.265 Master (no VA-API encoder on this host) and macOS AVFoundation Master (Master equals Final there) are not verified. |
| VC-020 | P2 | `implemented` | Export records and names | Reveal every finished export and find saved ranges under meaningful names. | Original | Implemented in gap closure 02: in-process video exports record an export artifact, jobs record their export settings so Retry restores format, quality (Master included), resolution, frame rate, name and folder after a restart, Show in folder reveals a recorded export saved outside the project, and Save range as media names the media `<project or timeline> mm:ss–mm:ss`. Next: native Show in folder for an export outside the project in the packaged app (workstream 06). |
| VC-021 | P2 | `implemented` | Background task progress | See how far a running task has progressed. | Original | A progressing export pill was **recorded on a packaged Linux app on 2026-09-18** (`output/gap-closure-07/export-tasks-rerun-4/`): during a 1080p High export the pill read "Exporting…" and then **"Exporting · 3%"**, and the Background tasks popover exposed `role="progressbar"` "Video export progress" with `aria-valuenow` 3. **Next action:** none for renders. Transcription has no worker progress channel. Only xAI video reports provider progress. GES renders and xAI video generations write lease-free progress snapshots the editor polls ([plan 01](superpowers/plans/2026-09-16-gap-closure-01-responsive-backend-jobs.md); `render_progress_ges`, `xai_generation_provider`). |
| VC-022 | P2 | `implemented` | Agent session and undo fidelity | Return to the chat a turn came from and undo agent batches reliably. | Original | An agent Undo was **recorded on a packaged Linux app on 2026-09-18** through the Claude backend (`output/gap-closure-07/agent-flows-claude/`): the applied card's Undo turned it into "Undone" and the saved timeline returned to its pre-edit fingerprint, with the turn recorded against its chat session in `context/agent-sessions.json`. **Next action:** an MCP-initiated Undo. The same run against the bundled Codex app-server is optional confirmation of the second backend, not a wait. Turns record their chat session; MCP Undo cancels in-process runs (including runs in the app process, through the job watch) and fal.ai/Replicate requests but not Temporal runs; other generations' progress no longer blocks Undo, though placing their output still does; undo snapshots are compressed under a 32 MiB retained cap. |
| VC-023 | P2 | `implemented` | Agent result frames on Linux | See post-apply preview frames in AI result cards on Linux. | Original | **Recorded on a packaged Linux app on 2026-09-18** (`output/gap-closure-07/agent-flows-claude/agent-flow-1/frames/`): the applied AI card's "Result preview" group held one loaded `img` of `naturalWidth` 1920 and never showed "Preview frames aren't available."; the 920,942-byte `preview-0001.png` is retained. **Next action:** none. Linux capture uses the canonical frame sampler with graphics overlays (`result_frames` tests, `output/linux-result-frames/`); image clips, text sources and unfinished generations are refused and show the card fallback. |
| VC-024 | P2 | `implemented` | NLE transition interchange | Open exported Premiere and DaVinci XML with transitions intact. | Original | Round-trip exported XML in Premiere Pro and DaVinci Resolve, which can't be done on Linux. Exports now carry: FCPXML dip to black as a Fade To Color transition with the Resolve-exported uid; audio-track transitions of every kind as audio crossfades (XMEML `Cross Fade (+3dB)`, FCPXML `Audio Crossfade`); and transitions on upper video and audio lanes as FCPXML connected storylines anchored in the covering primary clip or a gap. `rtk pnpm verify:nle-xml` validated 48 corpus files (24 XMEML, 24 FCPXML) against the pinned XMEML v5 and FCPXML 1.10 DTDs with 0 failures (`output/nle-xml-validation/2026-09-16-final/`), and the always-on structure test passes. Still open, because no public reference export verifies them (`docs/research/2026-09-16-nle-transition-interchange-references.md`): FCPXML dip to white exports as a cross dissolve, wipes export as cuts in both formats, and FCPXML transitions whose storyline would anchor on a retimed primary clip export as cuts. FCPXML captions are connected titles (lane above the video lanes and text overlays) anchored in the primary clip or gap covering their start; a caption over a retimed primary clip stays a flat connected title in the spine for the same reason. Resolve's own Dip To Color Dissolve defaults to white, so a real import should confirm that dip to black without a color param imports as black. XMEML still writes all video tracks into one `<track>` and all audio into one `<track>`. |
| VC-025 | P2 | `implemented` | Native evidence for the redesigned editor | Trust that the redesigned editor behaves in the desktop app as it does in browser fixture flows. | Original | Run the manual macOS ⌘Z single-undo check and refresh the macOS visual baselines on a Mac. The Linux items were re-run on 2026-09-18 against a freshly audited package (see the gap closure 07 section of the editor redesign hand-off): the packaged core smoke passed 19 of 19 attempted steps (`output/gap-closure-07/packaged-smoke-final/`, package SHA-256 `5d8c3e84…`), including the agent/MCP self-tests with all four readiness rows ready, the Secret Service round-trip with a private GNOME Keyring, and audio denoise with its report and manifest retained; a real X Ctrl+Z and GTK Edit → Undo both passed first time (`native-menu/`); and new steps recorded an export to a chosen folder under a chosen name, a progressing export pill and editing during a render (`export-tasks-rerun-4/`). The Temporal steps were not re-run: release builds leave out the `temporal-worker` feature. The earlier 2026-09-17 results (see the gap closure 06 section): the smoke against the audited, extracted `.deb` (`output/gap-closure-06/packaged-smoke/`, export re-run `packaged-smoke-rerun-2/`), including the agent/MCP self-tests, the Secret Service round-trip with a private GNOME Keyring, and audio denoise with its pipeline report and manifest retained; the Temporal steps on the debug build (`temporal-smoke/`, export re-run `temporal-smoke-rerun-2/` after fix 7481ae54); a real X Ctrl+Z that undoes exactly one edit and GTK Edit → Undo (`native-menu-rerun-1/`); and the MCP sidecar and Secret Service Rust tests (`rust-evidence/`). AI flows 1 and 2 through the Codex app-server are tracked by VC-001. |
| VC-026 | P2 | `verified` | Agent source maintenance | Keep agent context and chat code small enough to change safely. | Original | None. `codex/context.rs` is split into modules under 600 lines with the same 9 tests. |
| VC-027 | P2 | `implemented` | Responsive editor during renders | Keep editing, saving and importing while an export or saved range renders. | Original | **Measured on a packaged Linux app on 2026-09-18** (`output/gap-closure-07/export-tasks-rerun-4/`): during a 1080p High export the webview answered in 44 ms, 3 ms and 2 ms, and importing a 30 s H.264/AAC file succeeded while the render ran, so nothing freezes. That run also showed a timeline delete taking **20.4 s to appear** during the render against **0.8 s idle**, because the write queued behind the lease the render held from start to finish. **The lease is now split** ([hand-off](superpowers/plans/2026-09-13-editor-redesign-handoff.md#render-leases--a-project-write-no-longer-waits-for-the-encode-2026-09-18)): `SplitProjectMutationLease` covers one canonical read or write, and a render takes it for its start-phase snapshot and for each result write only, while the new `SplitProjectArtifactLease` covers `renders/`, precompose intermediates and their caches and is what a render, a preview preparation, a filmstrip pass and interrupted-render recovery hold for their whole run — so two renders stay serialized (in process by the storage lease, across processes by the artifact lease) without an editor write waiting for an encode. Preview preparation and filmstrip caching, which held the mutation lease for minutes, now hold it for their project read alone. Evidence, against real GES exports: a `createTrack` write stays queued while the encode, probe and validation complete with the mutation lease held by the test, and a clip delete lands in 0.5 s while a 150 s export encodes, which still writes 150.02 s (`project_export::render_lease`, `project::mutation::the_artifact_lease_does_not_block_a_project_write`). Project commands run off the main thread and project writes go through a per-project FIFO queue ([plan 01](superpowers/plans/2026-09-16-gap-closure-01-responsive-backend-jobs.md); `command_thread_audit`, `project_writes_queue_behind_a_held_project_lease_in_order`). **Next action:** re-run the smoke's `--export-tasks` step on a **packaged** app to measure the delete latency again — the improvement is proven by tests, not by a native measurement. A debug-build attempt could not produce one: the step's own `load_split_project_from_folder` probe takes the *storage* lease, which a render still holds from start to finish, so under a debug build's slower 1080p export it exceeds WebDriver's 30 s script timeout. Only the editor half of that measurement should be expected to fall; the saved half and the job-status poll both reload through that command and keep waiting for the render, without freezing the webview. Then decide whether the delete should also be applied optimistically, and shorten the two long mutation-lease holds that were left alone on purpose: fal.ai visual captioning and the Palmier package export (`render_job_needs_recovery` relies on the latter). |
| VC-028 | P2 | `implemented` | Stale Temporal task recovery | See a Temporal task fail when nothing is running it any more, instead of a Background tasks pill that stays on "Exporting…". | Original | The Linux desktop smoke Temporal steps ran on 2026-09-17 against a local Temporal dev server and passed (gap closure 06 section of the [hand-off](superpowers/plans/2026-09-13-editor-redesign-handoff.md)): selecting Temporal execution, a Temporal MP4 export that finished and reached "Export complete" after fix `7481ae54`, and Temporal transcription. That run used the **debug** build, because release builds leave out `temporal-worker`, so this stays `implemented` rather than `verified`. Describe can't prove a dead worker while its workflow is still Running; such tasks stay active until the workflow's own timeouts close it. Closed, missing and never-started workflows fail their jobs with plain reasons ([plan 01](superpowers/plans/2026-09-16-gap-closure-01-responsive-backend-jobs.md); `temporal_reconcile -- --ignored` against a local dev server). **Next action:** none on Linux; a packaged Temporal run needs a build that keeps `temporal-worker`. |
| VC-029 | P0 | `implemented` | Choice of AI agent | Use Claude as the AI agent on your own subscription, with no API key, and be told plainly when neither agent is available. | Original | `start_codex_conversation_edit_for_project` now reads the stored `agentBackend`, `claudeModel` and `claudeExecutablePath` and routes the turn: **Automatic prefers Claude** (2026-09-18) and takes Codex when Claude is not ready, an explicit choice is honoured and falls back to the other agent rather than refusing the turn, and a missing or unreadable preferences file behaves like Automatic ([docs](development/agents.md); routing tests in `src-tauri/tests/agent_claude/routing.rs`). Settings, the zero-token `agent.claude` readiness component, Advanced → Agent, the plain AI-tab reasons and the browser flow landed with [plan 02](superpowers/plans/2026-09-18-claude-agent-backend-02-settings-packaging-evidence.md). Chats map onto per-backend agent sessions, so switching backends inside one chat starts a new provider session and history records which agent produced each turn (`ab7e05a4`, `3c9312b8`). Nothing is bundled — Claude's terms are all-rights-reserved and Anthropic's own terms carve out running the user's unmodified binary under the user's own login ([research](research/2026-09-18-claude-agent-backend.md)). Evidence: the real ~70-variant action schema was accepted at full size by `claude` 2.1.270 on Haiku for $0.0139 (`src-tauri/tests/fixtures/claude_agent/real-turn-2026-09-18.jsonl`), and one real turn with the preference set to Claude ran through the app's own turn path for $0.0579 — Claude was chosen over a ready Codex, the sidecar reported `video-creater: connected`, the proposal validated into one `trimItems` action, nothing was denied and no id reached the summary (`src-tauri/tests/fixtures/claude_agent/real-command-turn-2026-09-18.json`). A **packaged native Claude turn through the Tauri command itself** ran on 2026-09-18 (`output/gap-closure-07/agent-flows-claude/`): the smoke stored `agentBackend: "claude"` and `claudeModel: "haiku"` through `update_app_preferences`, the packaged app read them from `AppPreferencesState`, the `agent.claude` row reported "Claude 2.1.270 is signed in as …" (`authMethod: claude.ai`), both turns ran the user's own `~/.local/bin/claude` with nothing bundled, and the history was written to the project's `context/agent-sessions.json` and `context/app-server-conversations.json` with `costUsd` 0.068835 and 0.0154177. That run also exposed and fixed a readiness bug: `claude auth status` pretty-prints one JSON object over twelve lines, which the line-oriented probe could not read, so a signed-in Claude was reported as `failed / agent.claude.malformedStatus` (`a7c9b0f5`). Claude is the preferred backend and neither agent is required: the Agent category is ready when *either* is, a missing bundled sidecar reads as `notConfigured` ("It is optional: turns can run on Claude instead."), and the Linux smoke gate needs the two support rows plus at least one ready backend. Subscription auth is explicit — `claude auth status`'s `loggedIn`/`authMethod`/`subscriptionType` distinguish "signed in with your Claude subscription" from an API-key session, the row never prints an email or organisation id (those fields are no longer deserialized), a signed-out user is pointed at `claude` → `/login`, and `ANTHROPIC_API_KEY` is withheld from a `claude.ai` session so the subscription cannot be silently shadowed while still being passed through when it is the only credential. Four auth states and both env behaviours are covered by stub executables (`src-tauri/tests/agent_claude/auth.rs`). **Next action:** whether a Tauri-spawned child inherits the macOS keychain login (no Mac on this host). |
| VC-030 | P0 | `implemented` | Secure remote web editor | Start Video Creater on a trusted host and edit, run AI work, monitor jobs, render, and download from another tailnet laptop or phone. | Original | The authenticated loopback Rust host, remote transport, scoped upload/media/artifact access, single-writer leases, browser capability UX, Settings controls, Linux package, systemd user unit, and real local-browser desktop/phone workflows are implemented. The 2026-09-25 redacted report is in [remote host local acceptance](reviews/evidence/remote-web-host-local-2026-09-25.md). It remains `implemented`, not `verified`: this VM's Tailscale daemon requires interactive host authentication, so no current verified MagicDNS URL or physical second-device run was retained. |

## Current P0 Acceptance Direction

### VC-001 — Proposal-focused agent workspace

The approved design and plan are:

- [Design specification](superpowers/specs/2026-07-25-codex-rail-proposal-workspace-design.md)
- [Implementation plan](superpowers/plans/2026-07-25-codex-rail-proposal-workspace.md)
- [Editor redesign specification](superpowers/specs/2026-09-13-editor-ui-ux-redesign-design.md),
  which implements this contract in the AI tab and supersedes the plan's React rail

As of 2026-09-15 the contract is implemented. Browser fixture flows
(`e2e/editor-ai.spec.ts` and `e2e/editor-acceptance.spec.ts`) are its acceptance
gate. Native Tauri checks are recorded evidence, not packaged release proof. On
2026-09-17 the packaged Linux run of AI flows 1 and 2 through the bundled Codex
app-server did not pass (model version, then usage limit), but the same two flows
passed on a packaged Linux app through the Claude backend on 2026-09-18 — applied
result frames, Show changes and Undo included — so this capability has native
evidence. The Codex-backed repeat is optional confirmation of the other backend. The
redesign also lets the current selection appear as one removable, human-named
composer chip; internal IDs and adaptive context stay hidden.

Acceptance requires:

- no hidden default prompt, preset, context strip, or internal media ID;
- adaptive project context remains internal;
- validated safe actions apply atomically;
- risky, external, expensive, or unknown actions stop for review;
- applied results show accurate facts, affected ranges, a real preview frame,
  timeline navigation, and Undo;
- failures never partially mutate the canonical project.

### VC-002 — Unified media understanding

The local foundations already include transcript timing, VAD, speaker
identities, silence ranges, denoise, sampled visual search, semantic
retrieval, metadata, generated-asset state, and media-analysis moments.

The product gap is a coherent, inspectable contract. Acceptance requires:

- one source view exposes available analysis and freshness;
- every result links to canonical media and a precise time range;
- stale or missing analysis is visible and recoverable;
- transcript and visual/audio findings can feed both story modes;
- local and remote enrichment are clearly distinguished.

### VC-003 and VC-004 — Make or find the story

Both modes converge on the same source-grounded story-plan contract:

- intended audience, format, duration, tone, and constraints;
- ordered beats with narrative purpose;
- selected canonical source ranges with reasons and score evidence;
- alternatives and omissions;
- a valid EDL before graphics or generated layers;
- acceptance checks for duration, repetition, continuity, captions, audio,
  overlays, artifacts, and logs.

**Make this story** begins with a user brief. **Find the story** begins with
media analysis and returns multiple meaningfully different candidates for user
selection. Neither mode may fabricate source evidence.

### VC-006 and VC-009 — Transcript editing and objective cleanup

Transcript-native editing is a view and action surface over canonical media
time, not a separate copy of the project.

The first complete creator slice is solo talking-head media. One **Clean up**
action may automatically apply only objective, high-confidence, local,
reversible operations:

- remove confirmed false starts or repeated takes;
- remove configured filler words;
- shorten pauses beyond an explicit policy;
- apply local speech leveling or denoise;
- repair caption timing, punctuation, and line breaking.

The result is one atomic revision with duration before/after, per-operation
counts, affected transcript passages, timeline highlights, playback from
changes, and one-step Undo. Uncertain detections remain suggestions. Narrative
reordering or rewriting always requires review.

## Locally Closed Foundations

These capabilities remain visible so future work does not reopen solved
foundations or mistake local proof for packaged release proof.

| Foundation | State | Evidence boundary |
| --- | --- | --- |
| Local transcription and word timing | `verified` | Local production model and retained fixtures; packaged model lifecycle is tracked separately. |
| VAD, speaker analysis, silence handling, and denoise | `verified` | Local model artifacts, deterministic projection, retained real and fixture evidence. |
| Local semantic visual search | `verified` | Pinned local model, durable index, text/image inference, timed results, and offline reuse. |
| Source-backed multi-source EDL construction | `verified` | Deterministic no-spend fixture with canonical media IDs, source ranges, reasons, and render checks. |
| Effects, keyframes, nested timelines, and canonical sampling | `verified` | Local preview/render fixtures and native output evidence. |
| MCP editing surface and durable named agent sessions | `verified` | Project-isolated session persistence and structured tool/proposal contracts. |

Detailed implementation and evidence references remain in
[the Palmier parity tracker](parity.md).

## Inspiration Map

### Palmier Pro

Retain the native professional timeline, direct manipulation, generation inside
the editor, MCP access, and the principle that a person and agent share the
same canonical project.

### Descript

Adopt transcript-first accessibility, scenes/story structure, objective
cleanup, and fast repurposing. Do not copy a workflow that hides transcript
alignment errors, silently changes media, or makes professional correction
hard.

References:

- <https://www.descript.com/tour>
- <https://help.descript.com/hc/en-us/articles/36803785502221-Underlord-beta-Your-AI-co-editor-in-Descript>

### Remotion

Adopt reusable parameterized compositions, code/agent authoring, interactive
preview, deterministic rendering, and automation-friendly inputs. Treat this
as a bounded composition layer rather than replacing the canonical editor with
arbitrary application code.

References:

- <https://www.remotion.dev/>
- <https://github.com/remotion-dev/remotion>

## Maintenance Rules

1. Keep backlog IDs stable.
2. Update `Last updated` whenever status, priority, or acceptance changes.
3. Move items between statuses; do not delete completed or deferred work.
4. Record the exact evidence boundary when moving an item to `verified`.
5. Link approved specs and plans from the backlog item.
6. Keep detailed implementation history in its source document; summarize
   rather than duplicate it here.
7. Add inspiration tags as provenance, not as product requirements.
8. Update this backlog after material product decisions, completed acceptance,
   newly discovered blockers, or explicit reprioritization.
