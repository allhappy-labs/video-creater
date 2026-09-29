# Source Viewer Generated Reference Previews Design

## Context

Palmier treats generated source inspection as a timeline-native workflow: double-clicking a generated clip opens its source, and the viewer can inspect the prompt, first frame, last frame, and references used for that generation. Video Creater already opens generated timeline clip sources in viewer tabs and shows generated provenance as compact text chips, but the reference evidence is not visual in the central source viewer.

## Requirements

- When a generated output is opened in the source viewer, the selected source details should show compact visual chips for first frame, last frame, and reference media.
- Reference chips should use safe local preview URLs when available.
- If a preview URL is missing or fails, each chip should fall back to a compact text/media-id treatment instead of disappearing.
- Existing generated provenance text chips must remain for asset id, model, prompt, and reference labels.
- The change must preserve viewer tabs, source playback controls, source range display, and right-rail source inspector behavior.

## Design

- Extend `PreviewSource` with optional structured `generatedReferencePreviews` entries containing label, media id, title, media kind, and preview URL.
- Keep the existing `generatedReferences` string array for compact provenance text and compatibility with current tests.
- Build structured reference previews in `selectedPreviewSource` from generated asset references and `previewUrlForMedia(projectDir, relativePath)`.
- Render a `Generated source reference previews` group inside `GeneratedSourceProvenance` when structured previews exist.
- Render video/generated references with muted `video` thumbnails, image references with `img`, and missing/failed previews as compact fallback cards.
- Track failed generated reference preview ids locally in `PreviewPanel` so one failed preview does not hide other references.

## Verification

- Add a preview-panel test for generated reference preview chips and per-chip fallback after a preview error.
- Add an editor-workspace test proving generated viewer source tabs receive safe local preview URLs for first-frame/reference chips.
- Run the focused preview-panel and editor-workspace tests.
- Run `pnpm lint`.
- Browser-smoke a generated timeline clip double-click and confirm the central source details show reference preview chips without crowding the viewer.
