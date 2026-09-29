# Reverse intermediate feasibility spike

Date: 2026-09-16. Design decision 14 (`docs/superpowers/specs/2026-09-16-editor-redesign-gap-closure-design.md`). Plan: `docs/superpowers/plans/2026-09-16-gap-closure-04-media-editing-features.md`, Task 12.

Reverse is added only if GES/GStreamer can render reversed clips reliably through a precomposed reversed intermediate on this runtime. This note records the go/no-go criteria (written before the harness ran), the measured numbers and the decision.

## Go/no-go criteria (written before running)

It is **go** only if all of these hold:

1. At least one strategy yields exactly reversed video and audio for media (i), (ii) and (iv): the segment colour order is exactly reversed, with zero dropped or duplicated frames, and the tone sweep falls instead of rising.
2. GES renders that intermediate with transitions (a 0.5 s crossfade on both sides), with a parity mismatch ratio ≤ 0.01 against the canonical frame sampler.
3. For media (iii), preparation takes ≤ 120 s wall time on this host, and the intermediate is ≤ 4 GB with PNG-MOV or ≤ 1 GB with ProRes.
4. The approach uses only reviewed LGPL/BSD factories on Linux, and the macOS path has a named equivalent (`vtenc_prores` or PNG-MOV), even though it is unverified here.

Otherwise it is **no-go**.

## Setup

- Runtime: staged Linux render runtime `linux-3451661e032bbcf9d529a468ad23f69a` (`VIDEO_CREATER_RENDER_RUNTIME_ROOT`), GStreamer 1.24.2, bundled LGPL FFmpeg 6.1.6 through gst-libav 1.24.2.
- Host: Linux 6.8 x86_64, 10 cores, 11 GB RAM. Three other Rust builds shared the host, so the load average was 13–23 during the timed runs. Wall times are therefore upper bounds.
- Build: the default `cargo test` profile (debug, `opt-level = 0`). GStreamer and FFmpeg are prebuilt C libraries, so only Rust-side work (for example `image`-crate PNG encoding) is slowed by the debug build.
- Harness: `src-tauri/tests/reverse_intermediate_spike.rs` and `src-tauri/tests/reverse_intermediate_spike/support.rs`. It reuses the `render_transitions_ges` media, fixture and parity modules through `#[path]`.
- Commands (from the repository root):
  ```
  TAURI_CONFIG='{"bundle":{"externalBin":[],"resources":[]}}' VIDEO_CREATER_RENDER_RUNTIME_ROOT="$HOME/.local/share/com.olhapi.video-creater/render-runtime/linux-3451661e032bbcf9d529a468ad23f69a" cargo test --manifest-path src-tauri/Cargo.toml --test reverse_intermediate_spike -- --test-threads=1 --nocapture
  TAURI_CONFIG='{"bundle":{"externalBin":[],"resources":[]}}' VIDEO_CREATER_RENDER_RUNTIME_ROOT="$HOME/.local/share/com.olhapi.video-creater/render-runtime/linux-3451661e032bbcf9d529a468ad23f69a" cargo test --manifest-path src-tauri/Cargo.toml --test reverse_intermediate_spike -- --test-threads=1 --nocapture --ignored spike_b_
  ```
  The two 30 s 1080p cost tests are `#[ignore]`d so a plain run of the target stays fast; the second command runs them.
- The OpenH264 encoder prints `CWelsH264SVCEncoder::EncodeFrame(), cmInitParaError` once per fixture encode. Every fixture still decoded with all its frames (98/98 and 720/720), so the message does not affect the measurements.

## Media

- (i) Segmented VP8 WebM: 14 solid-colour segments × 7 frames at 24 fps, 128×72.
- (ii) The same content as H.264 MP4 (`openh264enc ! h264parse ! mp4mux`).
- (iii) 30 s 1920×1080 24 fps H.264 MP4 with AAC audio (`openh264enc`, `avenc_aac`).
- (iv) Opus WebM tone sweep, 200 Hz rising to 2000 Hz over 4 s.

## Strategy A: negative-rate seeks

Pipeline: an explicit demuxer and decoder into `videoconvert ! appsink` (or `audioconvert ! audioresample ! appsink`), prerolled in PAUSED, then `seek(rate = -1.0, FLUSH | ACCURATE, start 0, stop = media end)` and PLAYING. A frame k is correct when its centre colour matches source frame 97 − k (channel difference ≤ 16). Audio buffers arrive in descending order with forward content inside each buffer (appsink does not reverse inside buffers), so the harness reverses each buffer before measuring.

| Media | Decoder | Delivered | Strictly descending PTS | Duplicate PTS | Exactly reversed | EOS | Wall |
| --- | --- | --- | --- | --- | --- | --- | --- |
| (i) VP8 WebM | `vp8dec` | 98/98 frames | yes | 0 | 98/98 frames | yes | 0.008 s |
| (ii) H.264 MP4 | `avdec_h264` | 98/98 frames | yes | 0 | 98/98 frames | yes | 0.044 s |
| (ii) H.264 MP4 | `openh264dec` | 98/98 frames | yes | 0 | 98/98 frames | yes | 0.007 s |
| (iv) Opus sweep | `opusdec` | 200 buffers, 3.994 of 4.000 s | yes | 0 | head 1918 Hz, tail 278 Hz (falls) | yes | 0.010 s |

`avdec_aac` was not measured with negative-rate seeks. Its AAC audio was reversed through Strategy B in the (iii) cost run.

## Strategy B: forward decode and reversed reassembly

**Video.** `decode_video_frames_rgba` (the reviewed precompose decoder) decodes forward over the whole source. Each RGBA frame is written as a PNG named with its reversed index (`frame-{97 - k}.png`), and `package_png_frames_as_mov` packages them. The MOV is decoded again with `qtdemux ! pngdec`, and every frame is checked against the expected source segment.

**Audio.** A forward decode to 48 kHz mono F32, then the samples are reversed and written as a 16-bit WAV with `hound`. The result is decoded again, and the zero-crossing frequency is measured over 0.05–0.30 s and over the last 0.30–0.05 s.

| Media | Result | Dropped / duplicated / mismatched frames | Output | Wall |
| --- | --- | --- | --- | --- |
| (i) VP8 WebM | exactly reversed, 98/98 frames | 0 / 0 / 0 | 44,819-byte PNG-MOV | 0.134 s |
| (ii) H.264 MP4 | exactly reversed, 98/98 frames | 0 / 0 / 0 | 45,158-byte PNG-MOV | 0.144 s |
| (iv) Opus sweep | 4.000 s; head 1922 Hz, tail 278 Hz (forward: head 278 Hz, tail 1922 Hz) | n/a | WAV | 0.054 s |

**GES render with transitions.** The project places cool (0–2 s), the reversed warm PNG-MOV (2–4 s, `sourceIn` 1, `sourceOut` 3) and cool (4–6 s) on Video 1, with a 0.5 s crossfade on both sides of the reversed clip. `assert_render_parity("reverse-spike", …)` compared canonical frames with the GES render at 1.8, 2.0, 2.2, 3.0, 3.8, 4.0 and 4.2 s:
- status `passed`;
- mismatch ratio 0 at every sampled time (max channel difference 3–7);
- evidence in `output/transition-preview-parity/reverse-spike/`.

A second GES render placed the reversed sweep WAV as an audio clip at 1–5 s. The output measured 1898 Hz at 1.10–1.35 s and 300 Hz at 4.65–4.90 s, so the sweep falls through GES.

**Cost for (iii)** (30 s, 1920×1080, 24 fps H.264 at 8 Mbit/s plus AAC; the fixture is `videotestsrc pattern=smpte horizontal-speed=8`, 30,642,263 bytes):

| Path | Frames | Wall | Intermediate size |
| --- | --- | --- | --- |
| Forward decode `avdec_h264 ! videoconvert ! pngenc ! multifilesink`, rename in reverse, `package_png_frames_as_mov` | 720/720 | 33.9 s decode + PNG, 36.1 s total | 219,979,174 bytes PNG-MOV |
| ProRes: the reversed PNGs through `pngdec ! videoconvert ! avenc_prores_ks ! qtmux` | 720/720 | 218.0 s encode, on top of the 33.9 s above | 304,241,650 bytes |
| Reviewed `decode_video_frames_rgba` at 1080p, decode only | 720/720 | 1.4 s (11.9 s total, of which 10.5 s was `image`-crate PNG encoding of 24 frames) | n/a |
| `image`-crate PNG encoding at 1080p, debug build | 24 frames | 436 ms per frame (about 314 s for 720 frames) | not measured |
| AAC audio: forward decode with `avdec_aac`, reverse, WAV | 1,441,792 mono samples | 1.5 s | WAV |
| Worst-case size: 48 frames of `videotestsrc pattern=snow` through `pngenc` | 48 | n/a | 3,119,364 bytes per frame, 2,245,942,080 bytes projected for 720 frames |

Factory policy, as observed through `evaluate_gstreamer_factory`:
- `avenc_prores_ks`: present and allowed.
- `pngenc`: present, but "not in the reviewed allowlist". It comes from the same `png` plugin (gst-plugins-good, LGPL) as the reviewed `pngdec`.

## Criteria results

| # | Criterion | Result | Evidence |
| --- | --- | --- | --- |
| 1 | Exactly reversed video and audio for (i), (ii), (iv) | **pass** | Strategy B: 98/98 frames with 0 mismatches for (i) and (ii), and the sweep falls from 1922 Hz to 278 Hz. Strategy A also delivered exact reversals on every measured decoder. |
| 2 | GES renders the intermediate through transitions with mismatch ≤ 0.01 | **pass** | `reverse-spike` parity `passed` with mismatch ratio 0 at all 7 samples; the reversed sweep falls through the GES audio render (1898 Hz to 300 Hz). |
| 3 | (iii) prepared in ≤ 120 s, with PNG-MOV ≤ 4 GB or ProRes ≤ 1 GB | **pass with PNG-MOV** | 36.1 s and 220 MB with the `pngenc` path; worst-case noise projects to 2.25 GB. ProRes fails the time bound (218 s encode). The `image`-crate PNG path fails it in a debug build (about 314 s); a release-build number was not measured. |
| 4 | Only reviewed LGPL/BSD factories on Linux, and a named macOS equivalent | **pass, with one review addition** | Decode (`decodebin`/`avdec_h264`/`vp8dec`/`opusdec`/`avdec_aac`) and packaging (`appsrc`, `qtmux`) are reviewed. The fast PNG path needs `pngenc`, which is LGPL and comes from the already-reviewed `png` plugin, but must get an `AllowedFactoryPolicy` entry. macOS: the same `pngenc` from the `png` plugin, which is already in the macOS `requiredPlugins`, or `vtenc_prores`. Neither is verified on this host. |

## Decision

**Go**, using Strategy B with PNG-MOV intermediates, under two conditions:

1. **Encoder.** Task R2 produces reversed video intermediates by decoding forward and encoding PNGs with `pngenc`, and adds a reviewed `pngenc` policy entry (plugin `png`, GStreamer Good packages, LGPL). If `pngenc` stays unreviewed, R2 must instead show that release-build `image`-crate encoding meets the 120 s bound for (iii).
2. **Audio.** Reversed audio intermediates are forward-decoded and sample-reversed WAVs, keeping the source channel layout. The spike measured mono only.

Strategy A (negative-rate seeks) also reversed exactly on every measured decoder with small media. It is not chosen, because production still needs an encoded intermediate for GES and the canonical sampler, and forward decode reuses the reviewed `decode_video_frames_rgba` path.
