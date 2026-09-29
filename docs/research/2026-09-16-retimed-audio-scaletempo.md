# Retimed audio: scaletempo is not a GES time effect

Date: 2026-09-16. Design decision 12 (`docs/superpowers/specs/2026-09-16-editor-redesign-gap-closure-design.md`). Plan: `docs/superpowers/plans/2026-09-16-gap-closure-04-media-editing-features.md`, Task 4.

## Finding

Task 4 planned to render retimed audio clips in GES with a `scaletempo rate=<speed>` clip effect, the audio counterpart of the `videorate rate=<speed>` effect visual clips use. That does not work on the staged runtime `linux-3451661e032bbcf9d529a468ad23f69a` (GStreamer and GES 1.24.2):

- Creating `ges::Effect::new("scaletempo rate=2.0")` logs `g_object_new_is_valid_property: property 'rate' of object class 'GstScaletempo' is not writable`. `scaletempo` takes its rate from the playback segment, so the property is read-only.
- `ges_base_effect_is_time_effect` returns false for that effect after it is attached to an audio `UriClip`. GES therefore does not treat it as a time effect. The render kept the source span at 1x (3.0 s instead of 1.5 s for speed 2).
- Evidence test: `render_transitions_ges::audio_speed::scaletempo_rate_is_not_a_ges_time_effect`. It fails loudly if a future GES registers the effect.

## Decision

The plan's recorded fallback is used: a tempo-changed intermediate rendered by `scaletempo` in a plain pipeline.

- **Preparation stage.** `precompose/audio_retime.rs` runs in `prepare_project_for_render` after denoise.
  - Each enabled audio clip with `speed != 1` is rendered by `filesrc ! decodebin ! audioconvert ! audioresample ! scaletempo ! audioconvert ! audioresample ! S16LE 48 kHz ! appsink` (`precompose/audio_retime_pipeline.rs`).
  - The pipeline takes a flushing, accurate seek at `rate = speed` over the source range plus the clip's audio transition handles.
  - `scaletempo` turns the seek rate into a tempo change at the original pitch.
  - Every factory is checked with `require_allowed_factories`. `wavenc` is not in the Rust allowlist, so the WAV is written with `hound`.
- **Cache.** The WAV is cached under `cache/audio-retime/v1/sha256/<fingerprint>/`. The fingerprint covers the source SHA-256, the range, the speed and the algorithm.
- **Prepared clip.** The clip then plays the intermediate at speed 1, from `sourceIn = head` for the clip duration. It keeps its transitions and fades.
- **Exact length.** `scaletempo` holds back its last stride; a speed-0.5 render came out 2.910 s instead of 3.000 s. The seek therefore reads 0.25 output seconds past the range, and the WAV is cut to the planned frame count. At the very end of the media, a shortfall of up to 0.15 s is padded with silence.
- **Denoise.** Denoised audio no longer bakes varispeed (`CACHE_VERSION` 3). It keeps the clip speed, and the retime stage then applies it with pitch preserved.
- **Unprepared plans are rejected.** The GES backend rejects audio clips with `speed != 1` at `renderPlan.audioClips[i].properties.speed`, and so does the AVFoundation backend ("AVFoundation export does not retime audio clips."). Project export always prepares first, so neither rejection is reachable from export.
- **Why not `pitch`.** The `pitch` element from `soundtouch` is registered as a GES time effect and is present in the staged inventory (LGPL). It was not used: it adds a plugin and library to the reviewed runtime (design decision 12 names `scaletempo`), and the plan excluded it.

## Measurements

All observed with `cargo test --test render_transitions_ges audio_speed`, the staged runtime and the compatibility decoder:

| Test | Result |
| --- | --- |
| Speed 2, source 0.5–3.5 s of a 440 Hz tone | 1.5000 s output, 439.4 Hz |
| Speed 0.5, 3 s clip | 3.0000 s output, 440.0 Hz |
| Speed 2, 0.5 s fade-in, `sourceIn` 1 | rms relative to steady: 0.103 at 0.05 s, 0.503 at 0.25 s, 1.007 at 0.50 s (linear in timeline time) |
| Speed-2 clip crossfading 0.5 s into a speed-1 clip | midpoint −0.00 dB from steady; quietest 10 ms window 0.338 rms |
| Speed 2, `sourceIn` 1, linear 200→2000 Hz sweep | 865 Hz at 0.25 s (source 1.5 s: 875 Hz); 1545 Hz at 1.0 s (source 3.0 s: 1550 Hz) |
| Prepared clip with a 0.25 s tail handle | intermediate 1.5 s, `sourceIn` 0, `sourceOut` 1.25; the second preparation is a cache hit |

## Not verified here

- macOS: the same pipeline needs `scaletempo` from `audiofx`, which Task 1 added to the macOS required plugins. No macOS run was possible.
- Native WebKitGTK preview pitch: the preview still uses `playbackRate` with `preservesPitch`. Workstream 06 covers it.
