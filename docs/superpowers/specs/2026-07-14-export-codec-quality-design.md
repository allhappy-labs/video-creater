# Independent Export Codec and Quality Design

## Intent

Make video export decisions explicit and independent. Codec selects the delivery format; quality selects render effort. `Draft` must accelerate review renders without preventing a user from choosing a different resolution.

## User Experience

The Video destination presents these controls in order:

1. **Codec**: `H.264`, `H.265 (HEVC)`, `ProRes`, or `WebM`.
2. **Quality**: `Draft` or `Final` for every enabled codec.
3. **Resolution**: `Match Timeline`, `HD (1280x720)`, `Full HD (1920x1080)`, and the supported higher presets.

Selecting Draft applies the `HD (1280x720)` preset only when the timeline is larger than HD. It preserves aspect ratio and never upscales a smaller timeline. The resolution selector remains enabled. If the user changes it, the choice becomes an explicit override and survives Draft/Final changes and codec changes. Selecting Final defaults to `Match Timeline` only when no explicit resolution override exists.

The file-type row and footer identify the selected container and extension. The progress and render report identify both quality and the actual output dimensions.

## Runtime Availability

The export dialog has three capability states:

- **Checking**: show a compact neutral `Checking native export capabilities…` status. Do not show a GStreamer warning.
- **Ready**: enable only the codecs the native capability report approves.
- **Unavailable**: retain disabled codecs and show the report's actual reason. A missing desktop bridge is described as unavailable in that session, not as an unfinished GStreamer factory check.

Browser visual QA supplies a healthy native-capability fixture. Its screenshots therefore represent the desktop product instead of the browser-only fallback state.

## Render Contract

Export requests carry two independent fields:

```text
outputProfile: webm | mp4H264 | mp4H265 | proResMov
quality: draft | final
```

The Rust-owned render plan, Temporal workflow inputs, output artifact naming, render command metadata, and render report carry both values. Existing combined `draftWebm` and `finalWebm` values are read through a compatibility adapter so existing saved projects and queued-workflow payloads remain valid while all new exports use the independent fields.

Draft quality uses the requested resolution (HD by default) with a fast, lower-bitrate encoder configuration. Final uses the requested resolution (timeline resolution by default) with delivery-quality settings.

Per codec:

| Codec | Draft | Final |
| --- | --- | --- |
| H.264 | fast hardware encode and draft bitrate | delivery H.264 bitrate/settings |
| H.265 / HEVC | fast hardware encode and draft bitrate | delivery HEVC bitrate/settings |
| WebM | fast WebM encoder path and draft bitrate | delivery WebM encoder path and bitrate |
| ProRes | ProRes Proxy | ProRes 422 |

The native capability report must distinguish ProRes Proxy support from ProRes 422 support before enabling Draft ProRes. If Proxy is not supported, Draft ProRes remains unavailable with an explicit capability reason; no silent fallback to a different codec is allowed.

## Validation and Review

Rust validates the selected output profile, quality, dimensions, extension, codec/container, and availability before a render job begins. A successful render review records:

- requested codec and quality;
- requested and actual dimensions;
- output duration;
- required video and audio stream presence;
- codec/container validation;
- artifact paths and render logs.

Draft renders must still run the same timeline/EDL, caption alignment, overlay timing, and stream validation as final renders. Lower quality affects rendering settings only; it never bypasses project, media, or render validation.

## Testing

- Unit tests cover independent codec/quality selection, Draft HD defaults, user resolution overrides, and quality changes without unwanted resolution resets.
- Frontend integration tests prove each enabled codec can submit both Draft and Final selections and receives the correct request fields.
- Rust tests validate the compatibility adapter and profile-specific Draft/Final encoder settings.
- Fixture render tests validate Draft and Final outputs for H.264, HEVC, WebM, and ProRes where the local capability report approves them.
- Browser visual QA captures the ready export dialog with the healthy capability fixture; desktop/native verification remains the proof for real codec availability and output streams.

## Scope

This changes video export selection and execution only. Timeline XML and Palmier Project destinations remain unchanged.
