# Render Controls And Report Review UI Design

## Summary

Add a focused frontend slice that lets users choose draft versus final WebM render quality and inspect render reports, GPU visual artifacts, and visual QA state inside the editor workspace.

This slice is UI and typed-model first. It does not start real render jobs or mutate canonical project files. It establishes the frontend contract for render quality and render report review so the next backend slice can wire real Tauri render commands into the same UI.

## Goals

- Expose render quality as an explicit editor choice.
- Use the exact render quality wire values `draftWebm` and `finalWebm`.
- Keep `draftWebm` as the default for draft iteration.
- Use `finalWebm` when the user chooses to render a finished video.
- Show a compact render report review surface in the editor workspace.
- Surface GPU visual renderer mode, quality profile, visual QA status, and artifact paths.
- Keep the implementation testable with sample/demo state before real job orchestration exists.

## Non-Goals

- Starting real render jobs from the frontend.
- Implementing Tauri job queues, cancellation, progress events, or log streaming.
- Changing encoder bitrate or VP8/VP9 behavior.
- Adding MP4/H.264.
- Building a full artifact browser.
- Reworking the editor layout beyond the render controls and review surface.

## Current Context

The Rust backend now has:

- `RenderQualityProfile::DraftWebm` and `RenderQualityProfile::FinalWebm`.
- GStreamer/GES command metadata values `draftWebm` and `finalWebm`.
- Render reports with `graphics` rows containing `layerId`, `renderer`, `qualityProfile`, and `visualQaStatus`.
- GPU visual artifacts that include preview images, manifests, frame sequences, renderer mode, and visual QA status.

The React workspace currently has:

- `EditorWorkspace` as the main editor shell.
- `AgentPanel` for edit generation controls.
- `PreviewPanel` for preview playback placeholder and selected motion template preview.
- TypeScript project types in `src/lib/project.ts`.
- No frontend render quality selector.
- No frontend render report or GPU QA artifact review surface.

## Chosen Approach

Use a typed demo-state review panel as the first slice.

The frontend will define render quality and report types that mirror the Rust JSON contract. The editor will own a local render quality selection and a sample render report object. UI controls update that local state and render a compact report review panel. Later backend work can replace the sample state with real Tauri command results without redesigning the UI.

## Data Model

Add frontend render types, likely in `src/lib/render.ts`:

```ts
export type RenderQualityProfile = "draftWebm" | "finalWebm";

export interface RenderGraphicsReport {
  layerId: string;
  renderer: "gpu" | "software" | string;
  qualityProfile: string | null;
  visualQaStatus: string | null;
}

export interface RenderReportSummary {
  status: string;
  durationSeconds: number | null;
  outputPath: string | null;
}

export interface RenderReport {
  jobId: string;
  summary: RenderReportSummary;
  command: {
    program: string;
    args: string[];
  };
  artifacts: string[];
  graphics: RenderGraphicsReport[];
}
```

The values must stay aligned with Rust:

- `draftWebm` maps to `RenderQualityProfile::DraftWebm`.
- `finalWebm` maps to `RenderQualityProfile::FinalWebm`.

Do not use alternate values like `draft`, `final`, `FinalWebm`, or `final_webm` in the frontend. Tests should lock the exact wire values.

## Render Quality Control

Add a compact control to the editor workspace, preferably near the preview or Codex render/action area.

Behavior:

- Default selected quality is `draftWebm`.
- Draft label: `Draft WebM`.
- Final label: `Final WebM`.
- The control uses button/segmented-control behavior with accessible pressed or selected state.
- Selecting `Final WebM` updates state to `finalWebm`.
- When the user explicitly chooses to render a finished video, the UI must use `finalWebm`.
- Draft generation and iteration keep using `draftWebm`.

This first slice can expose a local action such as `Render draft` or `Render final` that creates or updates sample report state. It must not claim that a real render job has run.

## Render Review Panel

Add a compact render report review panel, likely in or below `PreviewPanel`.

Empty state:

- Show that no render report is available.
- Keep the preview surface usable.

Report state:

- Show report status.
- Show output path when present.
- Show selected quality as `Draft WebM` or `Final WebM`.
- Show command quality if the command args include `--quality=draftWebm` or `--quality=finalWebm`.
- Show graphics rows with:
  - layer id.
  - renderer mode: `gpu` or `software`.
  - quality profile such as `hq-neon-wireframe-shader-v1`.
  - QA status such as `passed`.
- Show important artifact paths:
  - output file.
  - GPU preview image path.
  - manifest path.
  - sampled QA frames when available in later reports.

Long paths should be visually constrained with truncation and `title` attributes so they are inspectable without breaking the layout.

## Component Boundaries

Add small focused components rather than growing `EditorWorkspace` too much:

- `RenderQualityControl`: selected quality and change callback.
- `RenderReportPanel`: render report, selected quality, empty/report state.
- `src/lib/render.ts`: frontend render/report types and helpers.

`EditorWorkspace` should only own state and pass props.

## Error And Edge States

The report panel should handle:

- no report.
- report with no output path.
- report with no graphics rows.
- failed report with errors once error fields are modeled later.
- unknown renderer or QA string without crashing.

This first slice can omit detailed error rendering if the sample type does not include Rust `PipelineError` yet, but the layout should not assume success-only reports.

## Testing Strategy

Use frontend tests first.

Add tests that verify:

- The quality selector defaults to `draftWebm`.
- Selecting `Final WebM` updates the selected quality to `finalWebm`.
- A finished render action uses `finalWebm` in the sample report command args.
- The report panel renders the empty state.
- The report panel renders output path, renderer mode, quality profile, and QA status.
- Long artifact paths are present with a constrained visible label or `title`.

Add type/helper tests if render quality helpers are nontrivial.

Run:

```bash
rtk pnpm test -- --run src/components/workspace/editor-workspace.test.tsx
rtk pnpm test -- --run src/lib/render.test.ts
rtk pnpm lint
```

## Follow-Up Specs

This spec intentionally prepares later implementation without doing it.

Follow-up 2: Final WebM encoder behavior and composite QA.

- Wire real render commands through Tauri.
- Apply actual final encoder settings.
- Run final composite QA.
- Persist and load real render reports.

Follow-up 3: GPU graphics expansion.

- Add more Rust-owned GPU visual profiles.
- Add bounded profile hinting.
- Move wireframe primitives toward a native GPU line pass.
- Add render performance benchmarks.

## Acceptance Criteria

- The editor exposes a clear draft/final WebM quality choice.
- The frontend uses exactly `draftWebm` and `finalWebm`.
- Finished-video render intent selects `finalWebm`.
- Draft iteration keeps `draftWebm`.
- The editor shows a render report review state with GPU renderer mode, profile, QA status, and artifact paths.
- Empty and unknown-value states do not break the editor.
- Tests cover the exact quality values and render review behavior.

## Spec Self-Review

- No placeholders or unresolved items.
- Scope is limited to frontend and typed model work.
- Real render execution is explicitly deferred to the next spec.
- The `draftWebm` and `finalWebm` wire values are called out in data model, behavior, tests, and acceptance criteria.
