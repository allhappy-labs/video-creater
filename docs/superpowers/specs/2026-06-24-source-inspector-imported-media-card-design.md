# Source Inspector Imported Media Card

## Problem

Palmier-style editing keeps manual timeline work and agent actions grounded in concrete files: selected media is visible as a source object, not just a filename. Video Creater already gives generated media provenance cards with thumbnails and fallback icons, but imported library media in the Source Inspector is currently text-heavy. When an editor selects an imported source or an imported timeline clip, the AI edit actions and trim controls do not share a compact visual identity card for the source file.

## Goals

- Show a compact imported source card in the Source Inspector whenever the selected source resolves to an imported media asset.
- Use real local preview URLs for image, video, and generated-compatible visual media when available.
- Fall back to the existing media-type icon treatment when a preview URL is missing or fails.
- Keep generated provenance cards unchanged.
- Keep the card dense enough for the existing right-rail inspector.

## Non-goals

- No new project file schema.
- No new media thumbnail generation backend.
- No changes to timeline trim semantics or AI generation queue payloads.

## UX Requirements

- The card must be labelled as imported source media for screen readers and tests.
- The card must show filename, media summary, and relative path.
- Visual sources should render an image or video preview from `mediaPreviewUrls`.
- Audio sources should keep an icon fallback and must not gain AI visual edit controls.
- If a preview fails to load, the same card should remain visible with a file-type icon.

## Verification

- Add component tests for imported library media preview rendering and preview-error fallback.
- Run the focused SourceClipInspector test file, the full test suite, lint, and whitespace diff checks.
- Run a visual smoke check of the editor after implementation.
