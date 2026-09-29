# Palmier Inspector Rail Header Design

## Intent

Palmier's right rail reads as a focused inspector: the active context is named at the top, with compact tabs such as `Details` / `AI Edit` beneath it. Video Creater currently uses a full-width `Source` / `Timeline` segmented switch above the inspector body, which feels like extra app chrome. Tighten this right rail so it has a compact context header and small view tabs while preserving source/timeline switching.

## Requirements

- When both source and timeline inspectors are available, the right rail shows a compact `Inspector context` header.
- The header labels the active view as `Source` or `Timeline`.
- `Source` and `Timeline` remain accessible tabs in the `Inspector rail views` tablist.
- The tablist no longer uses the full two-column segmented-control treatment.
- Switching to Timeline still renders the project timeline inspector.
- Switching back to Source still renders the source inspector.
- No inspector data, selected media, timeline action, or project behavior changes.

## Verification

- Update `EditorWorkspace` tests for the compact header and tablist class.
- Keep existing source/timeline switching assertions passing.
- Browser-smoke the editor and confirm the right rail has less segmented-control chrome while staying readable.
