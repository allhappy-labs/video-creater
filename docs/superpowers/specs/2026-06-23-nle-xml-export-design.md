# NLE XML Export Design

## Context

Palmier documents export support for MP4 codecs and NLE XML for Premiere Pro and DaVinci Resolve. Video Creater currently has WebM render profiles and a policy-gated GStreamer/GES backend. Existing specs intentionally keep MP4/H.264 disabled until encoder provenance, licensing, CI availability, and distribution policy are approved.

The export gap that can move forward safely is NLE XML. It does not require enabling an unapproved video encoder, and it gives editors and agents a text-file handoff path from the canonical project timeline into external NLEs.

## Goals

- Export the canonical project timeline as NLE XML strings from Rust.
- Support a Premiere-oriented XMEML shape and a DaVinci-oriented FCPXML shape.
- Preserve timeline start, duration, media path, and source in/out ranges.
- Validate missing media and invalid source ranges before generating XML.
- Keep the first slice independent from file dialogs, render jobs, or a running Temporal worker.

## Non-Goals

- No MP4, H.264, H.265, AAC, or ProRes encoding in this slice.
- No UI export picker yet.
- No file writing command yet.
- No full effect, caption, transition, or audio mix interchange semantics.
- No persistent export job records until the command/UI layer exists.

## First Slice Contract

Rust exposes:

```rust
export_project_timeline_to_nle_xml(project, format) -> Result<NleXmlExport, NleXmlExportError>
```

Formats:

- `PremiereXmeml`: returns `<xmeml version="5">` with one video track and `clipitem` entries.
- `DavinciFcpxml`: returns `<fcpxml version="1.10">` with resources, project, sequence, spine, and `asset-clip` entries.

The export includes:

- project name and render settings,
- sequence duration,
- clip labels,
- timeline start and end frames,
- source in/out frames,
- project-relative media paths.

Frame math uses the project render frame rate. Source ranges come from timeline item `sourceIn` and `sourceOut` properties when present, otherwise from the clip duration.

## Validation

NLE XML export fails when:

- project render frame rate is non-finite or non-positive,
- a video/audio timeline item references missing media,
- a video/audio timeline item uses a non-media source,
- timeline or source ranges are non-finite, negative, empty, or exceed media duration.

Later slices should reuse split-project validation before export commands write files.

## Follow-Up Plan

1. Add Tauri commands to write NLE XML files under `exports/`.
2. Add a Temporal-backed `ExportNleXmlWorkflow` job record with activities:
   - `BuildNleXml`
   - `ValidateNleXml`
   - `WriteExportArtifact`
   - `AttachExportReport`
3. Add an export popover beside the existing `Export` chrome button with:
   - WebM render actions,
   - Premiere XML,
   - DaVinci XML,
   - MP4 profiles shown as policy-gated/unavailable until approved.
4. Expand interchange coverage for audio tracks, captions, markers, generated media provenance, and transitions.
5. Create a separate MP4/H.264 policy addendum before enabling any MP4 encoder profile.

## Acceptance Criteria

- Focused Rust tests prove Premiere XMEML contains timeline/source ranges and media paths.
- Focused Rust tests prove DaVinci FCPXML contains project media assets and asset clips.
- Missing media and out-of-range source spans fail with typed errors.
- MP4 remains disabled by policy until a later approved design changes that.
