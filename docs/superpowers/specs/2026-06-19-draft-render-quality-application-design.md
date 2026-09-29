# Draft Render Quality Application Design

## Goal

Make the draft render path actually use the draft quality settings that are now reported in preview metadata: cap draft output to 960px wide, preserve aspect ratio, and cap draft fps at 24 while keeping graphics layers aligned with the render output.

## Current Gap

`effective_quality_settings` reports draft dimensions and fps, but `proposal_to_render_plan` still copies `project.render_settings.width`, `height`, and `fps` into `RenderPlan`. `GstreamerGesRenderBackend` consumes those `RenderPlan` fields directly for command metadata and GES encoding restrictions, so draft renders remain source-size even when the preview says they are downscaled.

Preview and actual graphics generation also still use project render settings. If only the video render plan is downscaled, generated captions and overlays can be authored at the wrong dimensions.

## Design

Apply draft quality settings at the render-plan boundary:

- `proposal_to_render_plan` computes `effective_quality_settings(RenderQualityProfile::DraftWebm, project.render_settings.width, project.render_settings.height, project.render_settings.fps)` before constructing `RenderPlan`.
- The resulting `RenderPlan.width`, `height`, and `fps` become the authoritative draft output settings.
- `build_render_proposal_preview` uses `plan.width`, `plan.height`, and `plan.fps` for CPU and GPU graphics layers.
- `run_render_proposal_with_runner` passes the render plan dimensions into graphics artifact generation so actual artifacts match the rendered output.
- Public graphics-only helpers keep their current API by applying the same draft settings internally.

## Data Flow

```text
Project render settings
  -> effective draft quality settings
  -> RenderPlan width/height/fps
  -> GStreamer command and encoding profile
  -> graphics layer dimensions/fps
  -> render report performance/cache metadata
```

## Non-Goals

- Do not add UI controls for custom draft quality.
- Do not change final WebM behavior.
- Do not change the Codex proposal schema.
- Do not introduce encoder bitrate controls in this slice; the backend currently selects profiles by quality name.

## Tests

- Update render-plan conversion tests to expect draft dimensions/fps.
- Add preview command assertions for `--size=960x540` and `--fps=24.000`.
- Add a graphics generation test proving layer manifests use draft dimensions instead of source dimensions.
- Run the focused render-pipeline tests, then the full render-pipeline suite.
