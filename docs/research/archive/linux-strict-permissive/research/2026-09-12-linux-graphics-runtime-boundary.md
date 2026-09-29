# Linux graphics runtime boundary

Date: 2026-09-12

Status: source and runtime-dependency audit only. This investigation did not
install or build a browser engine and did not qualify a release artifact.

## Finding

Video Creater currently uses the name “HyperFrame” for six proposal and
timeline item kinds that are lowered to the app's bounded Rust graphics IR.
Those layers, captions, and overlays are rendered to transparent PNG sequences
by `cosmic-text`, `tiny-skia`, `image`, and `png`. Chrome, Electron, Node,
FFmpeg, GTK, and GStreamer do not participate in that PNG-generation call
path.

That current implementation is a useful compatibility subset, but it is not
the arbitrary HTML/CSS/JavaScript HyperFrames runtime described by the initial
architecture. The Node sidecar in that design was never added to this
repository. Upstream HyperFrames 0.8.36 renders a web page through Puppeteer
and encodes through FFmpeg. Its ordinary Chrome runtime is disqualified by the
selected Linux policy because Chromium ships an LGPL-2.1 FFmpeg component and
the inspected Linux browser binary is a glibc executable with a large dynamic
system-library closure.

The Rust subset has one known all-bundled violation before it can be called a
permissive static-musl path: `FontSystem::new()` scans system fonts. Loading
only the already reviewed bundled Roboto asset into an explicitly constructed
font database is a direct source-supported repair. A feature-minimal musl
graphics harness still has to prove the resulting binary and live runtime
closure.

There is no tested all-bundled permissive Linux implementation of the original
arbitrary HyperFrames web contract in the repository today. This is a precise
unclosed product gate, not evidence that no suitable engine can exist. Moli
1.1.5 is a concrete permissively licensed candidate for a small DOM/CSS/JS
capture experiment, but its published Linux artifacts target GNU, its native
V8/libcurl and transitive source closure are unaudited here, and its own
documentation excludes high-fidelity Canvas, WebGL, and media playback. It is
therefore not presently a full-parity answer.

## Policy and acceptance boundary

This audit applies the approved Linux boundary in
[the full compatibility design](../superpowers/specs/2026-09-12-linux-compatibility-design.md):

- every app-owned executable, helper, model, library, and loaded runtime
  dependency must be bundled and permissively licensed;
- LGPL is excluded even when it would only be dynamically linked;
- independent kernel, display, audio, and portal services may remain outside
  the bundle;
- an installed host browser is not an allowed fallback; and
- the graphics acceptance row preserves the *existing supported*
  caption/layer/precompose timing, alpha, transforms, and review metadata.

The corresponding
[implementation roadmap](../superpowers/plans/2026-09-12-linux-full-compatibility.md)
reuses the current CPU graphics manifests in Packet 7 and tests graphics in
Packet 9. It does not contain a packet that supplies or qualifies the original
HyperFrames web runtime. Packet 16's bundled Codex parity does not close that
render-engine gap.

## Current PNG artifact path

The implemented proposal path is:

```text
Codex JSON proposal
  captions[] / overlays[] / hyperframes[]
    -> caption_to_graphics_layer / overlay_to_graphics_layer /
       hyperframe_to_graphics_layer
    -> template_layer_to_graphics_ir for template-backed layers
    -> validate GraphicsLayer and registered local image assets
    -> render_graphics_preview_cancellable
       -> evaluate Rust keyframes at each frame time
       -> cosmic-text shapes/rasterizes text
       -> tiny-skia draws and composites CPU pixels
       -> image decodes registered imageRef assets
       -> PNG files plus manifest.json and preview.png
```

The key source boundaries are:

- [`render_pipeline/proposal.rs`](../../src-tauri/src/render_pipeline/proposal.rs)
  converts proposals. `hyperframe_to_graphics_layer` accepts exactly
  `template_overlay`, `title_card`, `lower_third`, `diagram`, `transition`,
  and `immersive_scene`, then selects a known template.
- [`graphics/templates.rs`](../../src-tauri/src/graphics/templates.rs) contains
  seven implemented template functions: kinetic lower third, punchy caption,
  metric callout, chapter card, tracking highlight, holographic logo cutout,
  and gradient background loop.
- [`graphics/ir.rs`](../../src-tauri/src/graphics/ir.rs) defines the executable
  vocabulary: `Text`, `RoundedRect`, `Rect`, `Polygon`, `Line`, `ImageRef`, and
  `HolographicLogo`. Its keyframes cover position, scale, opacity, rotation,
  blur, shadow/glow opacity, clipping/path progress, repetition/yoyo, and text
  reveal/emphasis.
- [`graphics/renderer.rs`](../../src-tauri/src/graphics/renderer.rs) loops over
  frame times, renders into CPU pixmaps, and writes
  `frames/frame-%06d.png`, `preview.png`, and an `rgbaFrameSequence` manifest.
- [`render_pipeline/project_export.rs`](../../src-tauri/src/render_pipeline/project_export.rs)
  lowers canonical timeline caption, overlay, and HyperFrame items through the
  same IR and renderer for project export.

`preparedFrames` is downstream of this work. The compositor reads an already
prepared RGBA frame sequence and places it at the requested timeline time.
That contract neither parses HTML nor executes CSS or JavaScript and cannot by
itself generate an arbitrary HyperFrames composition.

The current *final video* path is a separate boundary. Project export and
[`graphics/webm_export.rs`](../../src-tauri/src/graphics/webm_export.rs) still
use GStreamer/GES, and the
[`video-creater-hyperframe-evidence` binary](../../src-tauri/src/bin/video-creater-hyperframe-evidence.rs)
requires the `ges-render` feature. Those paths remain excluded until the
permissive media worker can consume the neutral PNG manifests. Their use does
not make the PNG generator itself dependent on GStreamer.

## Implemented coverage versus the original contract

| Capability | Current Rust path | Original HyperFrames web contract |
| --- | --- | --- |
| Typed proposal/timeline timing and visual review metadata | Implemented | Required |
| Transparent per-frame PNG artifact and manifest | Implemented | Supported by upstream as PNG sequence |
| Captions, word timing, word emphasis, safe-zone validation | Implemented | Expressible in web compositions |
| Six HyperFrame item kinds through seven fixed templates | Implemented | A small subset of possible compositions |
| Seven typed 2D node kinds and bounded keyframes | Implemented | Much broader browser layout and scripting |
| Registered local raster image references | Implemented | Browser media and resource loading |
| Arbitrary HTML and CSS layout | Absent | Core capability |
| Arbitrary JavaScript and DOM execution | Absent | Core capability |
| CSS/WAAPI/GSAP/Anime/Lottie seek adapters | Absent | Documented upstream composition paths |
| Canvas, Three.js, WebGL/WebGPU/TypeGPU | Absent | Documented upstream/custom-adapter paths |
| Video/audio elements inside a composition | Absent | Browser/media path |

The 2026-06-11
[editor architecture](../superpowers/specs/2026-06-11-tauri-agent-video-editor-design.md)
assigned full-frame scenes and overlay assets to a local Node HyperFrames
sidecar and explicitly made porting HyperFrames to Rust a non-goal. The later
[Rust graphics design](../superpowers/specs/2026-06-17-rust-native-graphics-frame-generation-design.md)
explicitly excluded browser layout, CSS, HTML canvas, and HyperFrames. Repository
search found no HyperFrames package dependency, Node worker implementation,
sidecar manifest, or bundled sidecar executable. The current Rust naming should
therefore not be used as evidence that the earlier web-engine contract exists.

## Exact current renderer dependency observations

The selected dependency versions are from `src-tauri/Cargo.lock`, not loose
manifest ranges:

| Package | Resolved version | Role | Package-declared license observed in Cargo metadata |
| --- | ---: | --- | --- |
| `cosmic-text` | 0.19.0 | shaping, layout, glyph rasterization | MIT OR Apache-2.0 |
| `fontdb` | 0.23.0 | font database | MIT |
| `fontconfig-parser` | 0.5.8 | fontconfig file parser | MIT |
| `harfrust` | 0.5.2 | shaping | MIT OR Apache-2.0 |
| `swash` | 0.2.9 | glyph scaling/rasterization | MIT OR Apache-2.0 |
| `tiny-skia` / `tiny-skia-path` | 0.12.0 | CPU drawing | BSD-3-Clause |
| `image` | 0.25.10 | registered raster assets | MIT OR Apache-2.0 |
| direct `png` | 0.18.1 | RGBA output | MIT OR Apache-2.0 |
| `moxcms` | 0.8.1 | image color management | BSD-3-Clause OR Apache-2.0 |

These declarations are evidence for a feasible permissive source graph, not a
completed source or binary audit. The exact renderer feature closure still
needs the repository's source-ledger, generated-code, native-artifact, notice,
and binary-runtime checks. In particular, the root crate's default features
also enable Tauri, GStreamer/GES, wgpu, and other systems. A release proof must
use a deliberately feature-minimal graphics target; calling only the CPU
function inside a default-feature binary is insufficient evidence that the
binary does not link or load excluded components.

### System-font leak

`TextRenderer::new` in the current renderer calls `FontSystem::new()`. In
[`cosmic-text` 0.19.0 source](https://github.com/pop-os/cosmic-text/blob/0.19.0/src/font/system.rs),
the `std` path calls `fontdb::Database::load_system_fonts()`. That makes glyph
selection, file access, and license closure depend on the host. The same source
offers `FontSystem::new_with_locale_and_db`, which permits an explicitly built
database. The smallest repair is to load only the reviewed bundled
`native/linux-desktop/assets/fonts/Roboto-Regular.ttf`, set a stable locale,
and fail closed if that asset is missing or its digest is wrong. The existing
[desktop font audit](2026-09-12-linux-desktop-fonts.md) records its pinned AOSP
source, Apache-2.0 terms, and digest; the graphics target must still prove that
it embeds or opens that exact bundled file and opens no host fonts.

### Browser and GPU observations

`@playwright/test` 1.63.0 and `scripts/playwright-cli.mjs` are development UI
QA only; no production graphics call edge to Playwright was found. The locally
cached Playwright Chromium 1228 binary is a 278,568,152-byte GNU ELF with
`/lib64/ld-linux-x86-64.so.2` as interpreter. `ldd` resolves glibc plus GLib,
GObject, GIO, ATK, NSS, D-Bus, CUPS, X11, Cairo, Pango, ALSA, GBM, and many
other host libraries. It cannot be bundled or used as a host fallback under
the selected policy. No Electron graphics runtime was found.

The optional wgpu renderer is also outside the proven baseline because it can
load a client graphics-driver stack. Its software fallback implements one
specific neon-wireframe profile, not the typed CPU renderer or an arbitrary
web composition. The baseline proof should disable `gpu-render`; later GPU use
needs its own independent-service/client-library boundary and runtime maps.

## Why upstream HyperFrames is not the bundled answer

At tag `v0.8.36` (commit
`f86aae655ae5aae7a9a2c124fa016f3bc30ebe52`), the official
[`@hyperframes/engine` manifest](https://github.com/heygen-com/hyperframes/blob/v0.8.36/packages/engine/package.json)
describes itself as a Puppeteer + FFmpeg engine, requires Node 22 or newer, and
depends on Puppeteer/Puppeteer Core 25.8. Its
[`render` documentation](https://github.com/heygen-com/hyperframes/blob/v0.8.36/docs/guides/rendering.mdx)
says validation opens the project in a browser, offers PNG sequences, and says
the Docker mode controls Chrome, FFmpeg, and fonts. A container would still be
an app-owned dependency closure, so it does not change this audit result.

Chrome is independently disqualified for this policy: Chromium's official
[`third_party/ffmpeg/README.chromium`](https://chromium.googlesource.com/chromium/third_party/ffmpeg/+/d2d06b12c22d27af58114e779270521074ff1f85/README.chromium)
records that its FFmpeg copy is shipped and licensed LGPL-2.1. Removing final
encoding from HyperFrames and requesting PNG does not prove that this library
is absent from or unloaded by the Chrome executable. The inspected GNU/glibc
binary and host-library closure add separate failures. The top-level
HyperFrames Apache-2.0 license cannot override those runtime component terms.

## Bounded candidate: Moli 1.1.5

Moli tag `v1.1.5` (commit
`3a9acf2e19988387ec118a3671d5d19fcdeb9e75`) is worth one small proof because
its official
[`README`](https://github.com/lexmount/moli/blob/v1.1.5/README.md) describes a
standalone Rust browser kernel with V8 JavaScript, DOM, CSS/Stylo, Taffy/Parley
layout, and on-demand CPU screenshots. Its repository default is MIT OR
Apache-2.0, subject to separately licensed components and fixtures.

It does not yet satisfy the product or policy boundary:

- the README names native `rusty_v8`/V8 and `libcurl`; their complete selected
  source and binary closures were not audited here;
- its published
  [`release workflow`](https://github.com/lexmount/moli/blob/v1.1.5/.github/workflows/release.yml)
  produces `x86_64-unknown-linux-gnu` and `aarch64-unknown-linux-gnu`, with no
  musl artifact or static-musl claim;
- its dependency manifests include source dependencies requiring an exact,
  immutable, reproducible pin before adoption; and
- its README explicitly says it does not provide Chrome pixel parity or
  high-fidelity Canvas, WebGL, or media playback, and has no GPU compositor or
  retained multi-frame paint architecture.

Those published gaps exclude full Three.js/WebGL/media parity even if a simple
DOM/CSS animation capture succeeds. No source evidence was found that Moli
implements the Chrome `HeadlessExperimental.beginFrame` behavior used by the
upstream HyperFrames engine. An adapter might be possible, but it is unproven
and must not be described as upstream compatibility yet.

## Next smallest proofs

Two separate gates avoid conflating the existing contract with the desired
web contract.

### Gate A: qualify the implemented Rust contract

Build a tiny, feature-minimal graphics worker from the existing IR and CPU
renderer. Replace system discovery with a fixed-locale database containing only
the exact reviewed bundled Roboto bytes. Render a fixture containing captions,
word emphasis, overlays, every one of the seven templates, and a registered
local `imageRef` at representative first/middle/last times. Require:

1. transparent PNG sequences, expected dimensions/frame counts/timing, and
   deterministic manifest/checksum results;
2. the current visual-review metrics and safe-zone failures;
3. successful consumption by the permissive compositor/media-worker boundary;
4. a static `x86_64-unknown-linux-musl` ELF with no `PT_INTERP` and no
   `DT_NEEDED` entries;
5. selected-target source/license/notices approval for the complete closure;
6. live `/proc/<pid>/maps` and file-open evidence showing no dynamic libraries,
   system fonts, host browser, or network resource access.

Passing Gate A preserves the graphics row currently promised by the Linux spec.
It does not close arbitrary HyperFrames compatibility.

### Gate B: decide whether Moli can cover a minimum web slice

Before downloading or building V8, perform a source-only selected-target audit
at the exact Moli commit. Enumerate every source/native artifact, replace or
pin mutable source dependencies, determine whether V8 and libcurl can produce
an auditable static musl binary, and reject immediately on any forbidden or
unknown license or unavoidable dynamic library.

Only if that audit passes, build an isolated offline spike that loads one local
fixture with bundled font, HTML layout, CSS styling, JavaScript-controlled time,
and alpha. Set two exact seek times through a narrow adapter and capture
transparent screenshots. Verify pixels differ as expected, repeat hashes are
stable, network is disabled, no system fonts are opened, and the binary/runtime
checks from Gate A pass.

Passing that spike would establish only the minimum DOM/CSS/JS slice. The
documented Canvas, WebGL, media, Lottie, Three.js, TypeGPU, and exact upstream
seek-adapter gaps would remain explicit full-parity gates. Failing the
source/musl audit or deterministic seek/alpha checks ends the candidate without
a large integration effort.

## Verification performed

- Inspected the current proposal, project-export, graphics IR/template,
  renderer, WebM, GStreamer evidence, compositor, manifest, lockfile, browser
  QA, Linux spec, roadmap, and original graphics/HyperFrames design sources at
  repository commit `ef00bb53d9993eb56b8eb75b7f156eee416b1543`.
- Inspected the cached Playwright Chromium executable with `file` and `ldd`.
- Read pinned upstream primary sources for HyperFrames 0.8.36, Chromium's
  shipped FFmpeg license record, cosmic-text 0.19.0, and Moli 1.1.5.
- Did not run repository tests, compile a musl target, inspect a release
  process's live maps, or build Moli. Those checks remain unverified rather
  than passed.
