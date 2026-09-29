# Generation History Thumbnail Cards Design

Palmier's media panel keeps recent AI generations as visual project assets: each card carries a thumbnail, AI/status cues, duration, and enough model/prompt context to reuse the generation quickly. Video Creater already renders thumbnail-rich generated outputs in the `AI generations` section, but the media-generation composer history still shows recent generated assets as text rows. That makes history harder to scan while composing a new image, video, or audio generation.

## Goal

Render the composer `Generation history` drawer as compact thumbnail cards when a generated asset has a resolved output media item.

## Requirements

- Keep the existing newest-first, four-item history limit.
- Keep the existing `Use prompt` action and accessible label for every history item.
- When a generated asset has at least one output whose `mediaId` exists in the media library, show the same thumbnail treatment used by project media tiles.
- Generated visual outputs show an `AI` badge, duration badge, and generation status strip on the thumbnail.
- Each card still shows generated asset title, status, model label, and prompt excerpt.
- If no output media exists, keep a compact text fallback with the same metadata and `Use prompt` action.

## Non-Goals

- No generated-asset schema changes.
- No new media preview URL loading path.
- No Temporal, fal.ai, mock generation, or project-action changes.
- No change to history reuse behavior.

## Testing

- Media Bin tests prove a history item with output media renders a generated thumbnail, `AI` badge, duration, model/prompt metadata, and the existing `Use prompt` action.
- Existing history ordering and draft-reuse tests continue to cover recency and form population.
