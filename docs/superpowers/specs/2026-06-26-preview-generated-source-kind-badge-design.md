# Preview Generated Source Kind Badge Design

## Intent

Palmier marks generated media as AI-native editor objects throughout the media library, viewer tabs, and inspector. Video Creater already shows an `AI` chip inside generated source viewer tabs, but the preview header still shows the raw media kind `generated`. Make the preview header use the same compact source-kind label so generated sources read consistently as AI media while imported video, image, and audio keep their normal kind labels.

## Requirements

- In source viewer mode, the preview header source-kind badge shows `AI` for generated sources.
- Imported source header badges keep readable title-case labels such as `Video`.
- The viewer source tab kind chips and source provenance remain unchanged.
- No project schema, timeline, generation, Temporal, or render behavior changes.

## Verification

- Add a `PreviewPanel` test proving generated source mode renders an `AI` header badge.
- Add or preserve coverage that imported video source mode renders `Video`.
- Existing preview panel source viewer, generated provenance, and viewer tab tests continue to pass.
