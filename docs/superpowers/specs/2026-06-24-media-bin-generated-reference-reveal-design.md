# Media Bin Generated Reference Reveal Design

## Context

Palmier shows generated-source references as visual thumbnails that can be inspected directly from the editor. Video Creater's Source Inspector already has reveal actions for generated references, but the Media Bin selected generated source panel still renders first frame, last frame, and reference media as text-only cards. That makes the Media Bin weaker as a generated-source browser even though it already has the selected generated prompt, model, settings, and rerun action.

## Goal

Make generated-source references in the Media Bin details panel visual and actionable.

## Behavior

- In the selected generated source `Details` tab, each first frame, last frame, and reference media entry renders as a compact media card with a thumbnail treatment, relationship label, filename, and media id.
- When `onSelectMedia` is available, each reference card is a button labeled `Reveal <relationship> <media id>`.
- Pressing a reference card calls `onSelectMedia(mediaId)`, reusing the existing source selection path to open that media in the editor.
- When `onSelectMedia` is not available, the cards remain static and readable.
- Missing reference media ids still render as text fallback cards so broken provenance stays inspectable.

## Visual Treatment

- Use the existing Media Bin thumbnail treatments for video, image, audio, and generated media.
- Keep cards compact enough for the left rail: thumbnail on top, relationship/filename/id below.
- Avoid nested cards or modal reveal flows.

## Verification

- Media Bin tests prove reference cards are rendered as reveal buttons when `onSelectMedia` exists.
- Media Bin tests prove clicking first-frame and reference cards calls `onSelectMedia` with the referenced media id.
- Existing details, rerun, and AI Edit variation tests keep passing.
- Browser QA checks the selected generated source panel on desktop and narrow widths for overflow or unreadable controls.
