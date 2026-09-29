# Rust Render Pipeline Node Removal Design

## Summary

Remove Node.js from Video Creater's graphics, render, and video e2e pipeline while keeping the React/Vite frontend toolchain. Rust will own reusable render orchestration, graphics artifact generation, `ffmpeg` command execution, `ffprobe` validation, reports, and concise actionable errors.

This is a pipeline migration, not a frontend rewrite. `pnpm`, Vite, Vitest, React, and TypeScript remain for app UI development and UI tests.

## Goals

- Replace the Node render/e2e harnesses with Rust library code and small Rust CLI binaries.
- Keep `ffmpeg` and `ffprobe` as the v1 video backend.
- Use the existing Rust graphics IR, validation, `imageRef` support, rasterizer, manifests, and ffmpeg overlay command hooks.
- Make render and e2e entry points easy for AI agents to run and repair from.
- Return compact actionable errors for graphics, render-plan, ffmpeg, ffprobe, IO, and report-writing failures.
- Preserve artifact/report workflows so humans and agents can inspect generated media.

## Non-Goals

- Removing Node from the frontend build, dev server, or frontend tests.
- Replacing `ffmpeg` in this implementation.
- Rewriting the Tauri app UI.
- Adding arbitrary executable graphics code from agents.
- Porting browser layout, CSS, HTML canvas, or HyperFrames itself.

## Current State

The Rust graphics slice already exists in `src-tauri/src/graphics/`:

- graphics IR and validation
- actionable graphics errors
- approved `imageRef` asset resolution
- tiny-skia/cosmic-text PNG preview rendering
- manifest writing
- template-to-IR expansion
- ffmpeg overlay command construction from graphics manifests

The remaining Node pipeline code lives in script harnesses:

- `scripts/render-codex-funny-draft.mjs` builds SVG graphics states, rasterizes them with `rsvg-convert`, creates an overlay movie with `ffmpeg`, composites it over selected clips, validates the MP4, and writes a JSON report.
- `scripts/e2e-combined-video.mjs` generates fixture media, builds a canonical project and render plan, synthesizes a HyperFrames-like scene through ffmpeg filters, composites overlays/captions, validates the final MP4, and writes JSON/Markdown reports.
- `scripts/e2e-codex-app-server-funny.mjs` is a Node Codex app-server e2e harness. It produces proposals consumed by the render harness and should move to Rust after the render-proposal path exists.

## Chosen Approach

Create a Rust-owned render pipeline library plus thin Rust CLI binaries.

Library code goes under `src-tauri/src/render_pipeline/`. It should contain reusable units for:

- loading proposal/project inputs
- constructing render jobs from validated project/proposal data
- rendering graphics manifests through `src-tauri/src/graphics/`
- converting manifests into `GraphicsOverlayInput`
- building `ffmpeg` commands through existing render-plan code
- executing external tools with bounded timeouts and captured logs
- validating output media with `ffprobe`
- writing JSON and Markdown reports

CLI binaries go under `src-tauri/src/bin/`. Each binary should parse arguments or environment variables, call library code, print compact progress and errors, write artifacts/reports, and exit nonzero on failure.

The Tauri app binary stays focused on the desktop app. Pipeline tooling should not be hidden behind app startup or Tauri runtime behavior.

## Proposed CLI Entry Points

### `video-creater-render-proposal`

Rust replacement for `scripts/render-codex-funny-draft.mjs`.

Responsibilities:

- Read a Codex proposal report JSON.
- Validate clips, captions, overlays, dimensions, timing, and language-sensitive visible text inputs where structured data is available.
- Convert supported proposal captions/overlays into graphics IR.
- Render graphics artifacts with Rust.
- Build and execute the ffmpeg trim/concat/overlay command.
- Validate the final MP4.
- Write a render report with artifact paths, validation checks, and concise structured errors.

### `video-creater-e2e-combined`

Rust replacement for `scripts/e2e-combined-video.mjs`.

Responsibilities:

- Generate tiny deterministic fixture media through ffmpeg.
- Build the canonical project and render plan in Rust structs.
- Generate deterministic graphics overlays through the Rust graphics path.
- Compose the final MP4 through the ffmpeg backend.
- Validate duration, stream presence, codec, dimensions, and size.
- Write JSON and Markdown reports.
- Keep the e2e small enough for local and CI use.

### `video-creater-codex-e2e`

Rust replacement for `scripts/e2e-codex-app-server-funny.mjs`.

Responsibilities:

- Reuse the existing Rust `codex::app_server` transport.
- Load the project skill bundle through Rust.
- Request a structured video edit proposal from Codex.
- Validate proposal shape before writing the report.
- Emit the same proposal report shape consumed by `video-creater-render-proposal`.
- Write compact actionable errors for Codex process, transport, proposal validation, and report failures.

## Data Flow

```text
project/proposal JSON
  -> Rust input loader
  -> typed validation
  -> EDL/render plan
  -> graphics IR/template expansion
  -> graphics artifacts and manifests
  -> ffmpeg backend command
  -> MP4 output
  -> ffprobe validation
  -> JSON/Markdown report
```

The graphics engine remains backend-neutral. It produces RGBA frame sequences and manifests. The render pipeline adapts those manifests to the current ffmpeg backend.

## Error Contract

All recoverable pipeline failures should return compact actionable errors. The stable shape is:

```json
{
  "code": "RENDER_FRAME_SEQUENCE_EMPTY",
  "path": "graphics[0]",
  "message": "No frames were rendered.",
  "fix": "Render at least one RGBA frame before composing."
}
```

Errors should be concise enough for agent repair loops. CLI stderr may print one-line summaries, while JSON reports should preserve structured errors and relevant log paths.

Initial error categories:

- `GRAPHICS_*` from the existing graphics module
- `RENDER_PLAN_*` for invalid clips, timeline ranges, overlays, dimensions, or unsupported tracks
- `RENDER_BACKEND_*` for missing tools, nonzero ffmpeg/ffprobe exits, timeout, or malformed probe output
- `RENDER_ARTIFACT_*` for missing, empty, unreadable, or unauthorized artifact paths
- `PIPELINE_INPUT_*` for invalid proposal/project JSON and missing required fields
- `PIPELINE_REPORT_*` for report write failures

## ffmpeg Boundary

Introduce a narrow render backend boundary in `render_pipeline`:

- input: typed render job plus graphics manifests
- output: rendered media path plus logs/probe data
- v1 implementation: `FfmpegRenderBackend`

The backend should own process execution, timeout, stdout/stderr capture, and nonzero exit conversion into actionable errors. Render-plan construction should remain testable without spawning ffmpeg.

This boundary keeps later GStreamer, OpenH264, or pure-Rust experiments from changing graphics IR or proposal validation.

## Graphics Conversion

The first implementation should support the visual assets used by the current Node render harness:

- captions as designed overlay graphics
- title cards
- lower thirds
- callouts
- simple persistent labels

Each generated graphic must include:

- `visualTreatment`
- `motion`
- `safeZone`
- `avoid`
- dimensions, fps, duration, timeline start, alpha, and source beat

The migration should avoid recreating the old SVG implementation literally. Rust should generate normal graphics IR and render through the existing graphics renderer.

## Script Removal

After Rust equivalents are passing:

- remove `scripts/render-codex-funny-draft.mjs`
- remove `scripts/e2e-combined-video.mjs`
- remove `scripts/e2e-codex-app-server-funny.mjs`
- update `package.json` pipeline/e2e scripts so they call Rust binaries through `cargo run --manifest-path src-tauri/Cargo.toml --bin ...`

Keep Node only for frontend tooling. After this migration, no non-frontend `.mjs` pipeline harness should remain.

## Testing

Add focused Rust tests before implementation code for each migrated unit:

- proposal JSON validation rejects invalid clips and missing proposal data
- proposal graphics conversion produces valid graphics IR
- graphics manifests convert into `GraphicsOverlayInput`
- ffmpeg backend command construction does not require spawning a process
- process wrapper converts missing binary, timeout, and nonzero exit into actionable errors
- ffprobe parser validates duration, streams, codec, dimensions, and file size
- Codex e2e proposal generation uses the Rust app-server transport and writes a render-proposal-compatible report
- combined e2e fixture render produces a playable MP4 with video and audio streams
- package scripts no longer reference Node for graphics/render/e2e pipeline entry points

The full verification target remains `rtk pnpm verify`, with Rust tests covering the pipeline migration. The combined media e2e can stay opt-in if runtime cost is too high for every verify run, but it must be runnable from a documented package script.

## Rollout

1. Add `render_pipeline` error, process, ffprobe, report, and backend primitives.
2. Add `video-creater-render-proposal` tests and implementation using existing graphics/ffmpeg hooks.
3. Add `video-creater-e2e-combined` tests and implementation.
4. Add `video-creater-codex-e2e` tests and implementation using the Rust app-server transport.
5. Update `package.json` scripts to use Rust binaries for pipeline e2e.
6. Remove replaced Node scripts.
7. Run targeted Rust tests, pipeline e2e if feasible, and `rtk pnpm verify`.

## Acceptance Criteria

- Graphics/frame generation for the render harness no longer uses Node, SVG generation, or `rsvg-convert`.
- The combined video e2e path no longer uses Node.
- The Codex app-server video proposal e2e path no longer uses Node.
- No non-frontend `.mjs` pipeline harness remains.
- `package.json` has no Node-backed graphics/render/e2e pipeline script.
- Rust emits artifacts, logs, JSON reports, and Markdown reports equivalent in purpose to the existing scripts.
- Every recoverable failure exposed to agents has `code`, `path`, `message`, and `fix`.
- A tiny generated media e2e validates output duration, streams, codec, dimensions, and file size.
- Frontend Node tooling remains intact and explicitly out of scope.
