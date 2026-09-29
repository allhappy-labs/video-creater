# Linux native desktop and bundled-font qualification

Date: 2026-09-12. Scope: source qualification and architecture planning for
the first native Linux Project Home. No desktop crate, vendored source, font,
binary, package, or lockfile was created by this research.

## Decision

**Conditional candidate only.** The most direct production path is a new,
isolated Rust/musl desktop executable using the existing canonical Rust project
library, a custom Wayland protocol host, and selected Iced 0.14 components for
widgets, layout, input routing, text, and CPU rasterization. No qualifying web
engine has been established, so the existing React presentation cannot yet be
reused on Linux. Standard Tauri/GTK/WebKitGTK and Electron do not escape the
application dependency policy.

Iced is not approved for adoption. Its normal features pull window and graphics
paths that are outside the proposed host, its text initialization scans system
fonts, its software renderer retains an unconditional `softbuffer` window
dependency, and its embedded icon font lacks adequate redistributable-source
provenance. A small reviewed fork can remove those behaviors without replacing
the whole UI toolkit. The complete locked build graph, patched source, build
toolchain, asset notices, and final executable still require the repository's
full source/license review before application UI work starts.

A fully custom widget layer over cosmic-text and tiny-skia is not the preferred
first implementation. It would remove the Iced patches, but it would also make
this project responsible for layout, focus, keyboard routing, pointer capture,
scrolling, editing, IME integration, and accessibility semantics. That is a
larger and riskier product surface than the bounded fork. It remains a fallback
if the Iced closure fails review.

## Intended component boundary

Pin Iced `0.14.0`, whose tag resolves to commit
[`3997291f318a8bc06fa522f5579836fb3feb94df`](https://github.com/iced-rs/iced/commit/3997291f318a8bc06fa522f5579836fb3feb94df).
Use component crates with default features disabled:

```toml
iced_core = { version = "=0.14.0", default-features = false }
iced_graphics = { version = "=0.14.0", default-features = false }
iced_runtime = { version = "=0.14.0", default-features = false }
iced_widget = { version = "=0.14.0", default-features = false }
iced_tiny_skia = { version = "=0.14.0", default-features = false }
```

Do not depend on the umbrella `iced` crate, `iced_renderer`, `iced_winit`,
Iced's X11 or Wayland features, `wgpu`, `rfd`, or a webview. The Iced
[`0.14.0` manifest](https://github.com/iced-rs/iced/blob/0.14.0/Cargo.toml)
shows why the umbrella defaults are unsuitable: they enable `wgpu`, tiny-skia,
Linux theme discovery, X11, and Wayland. The app-owned shell instead speaks the
Wayland protocol through the exact reviewed wayrs path. Iced owns view layout,
widget state, event dispatch, and renderer commands, but never the display
connection or frame lifecycle.

[`iced_tiny_skia::Renderer::draw`](https://github.com/iced-rs/iced/blob/0.14.0/tiny_skia/src/lib.rs#L62-L140)
can rasterize into a caller-owned `tiny_skia::PixmapMut`. That is the essential
integration seam. The renderer writes an app-owned premultiplied RGBA scratch
pixmap; a tested conversion copies it into the compositor-advertised
little-endian XRGB8888 `wl_shm` buffer. The host alone owns buffer allocation,
release, and presentation. However, the crate manifest declares
`softbuffer` unconditionally and
[`pub mod window`](https://github.com/iced-rs/iced/blob/0.14.0/tiny_skia/src/lib.rs#L3)
unconditionally. The fork must add a disabled-by-default `window` feature,
make `softbuffer` optional behind it, gate `pub mod window`, gate the
`compositor::Default` implementation, and gate the window-backed
`renderer::Headless` implementation. The application must use `Renderer::draw`
directly.

The earlier Wayland proof established only `wl_compositor`, `wl_shm`, and
`xdg_wm_base`. The first Project Home needs these client-side protocol paths:

- `wl_registry` discovery with required-global version caps and actionable
  errors when `wl_compositor`, `wl_shm`, `xdg_wm_base`, or `wl_seat` is absent;
- `xdg_wm_base` ping/pong, `xdg_surface.configure` acknowledgement,
  `xdg_toplevel` title/configure/close, and zero-size configure handling;
- `wl_shm` XRGB8888 negotiation, at least two app-owned buffers, release before
  reuse, stride/overflow checks, damage, commit, and frame callbacks;
- `wl_seat` capability changes plus `wl_pointer` enter/leave/motion/button/axis
  and `wl_keyboard` keymap/enter/leave/key/modifiers/repeat events;
- focus loss, scale changes, redraw coalescing, compositor disconnect, and
  orderly destruction.

For the bounded slice, project selection comes from `--project <absolute-dir>`
or the bundled sample action. It does not yet promise arbitrary text entry,
file dialogs, clipboard, drag and drop, or IME. A production text field needs a
separate qualified XKB keymap path and Wayland text-input protocol work; raw
`wl_keyboard` keycodes alone are not sufficient for layout-correct Unicode
input. Accessibility also remains a release gate. A semantic tree snapshot is
useful now, but it is not AT-SPI verification.

## Bundled-only text initialization

Iced 0.14 currently violates the bundled-only font rule even if the application
never asks for a system font. Its
[`graphics/src/text.rs`](https://github.com/iced-rs/iced/blob/0.14.0/graphics/src/text.rs#L111-L133)
calls `cosmic_text::FontSystem::new_with_fonts`. cosmic-text `0.15.0`, tag
[`c82ee1c5b5b8032e91eaff1cb34294b538727a7d`](https://github.com/pop-os/cosmic-text/commit/c82ee1c5b5b8032e91eaff1cb34294b538727a7d),
implements that constructor by creating a font database and calling
[`load_system_fonts()`](https://github.com/pop-os/cosmic-text/blob/0.15.0/src/font/system.rs#L365-L374).
On Linux, fontdb scans configured and conventional system font directories.

Patch `vendor/iced_graphics-0.14.0/src/text.rs` so the singleton starts from an
empty database and a fixed locale:

```rust
let database = cosmic_text::fontdb::Database::new();
let raw = cosmic_text::FontSystem::new_with_locale_and_db(
    "en-US".to_owned(),
    database,
);
```

Then load the one approved application face through the existing
`FontSystem::load_font(Cow::Borrowed(...))` path. The constructor must not call
`FontSystem::new`, `new_with_fonts`, `fontdb::Database::load_system_fonts`, or
any file/directory font loader.

Feature selection matters independently of that call-site patch. The
[`cosmic-text 0.15.0 manifest`](https://github.com/pop-os/cosmic-text/blob/0.15.0/Cargo.toml)
enables `fontconfig` by default, and its `std` feature enables fontdb memory
mapping. The
[`fontdb 0.23.0 manifest`](https://github.com/RazrFalcon/fontdb/blob/v0.23.0/Cargo.toml)
shows that `memmap` activates filesystem support and `memmap2`. A fully
memory-only fork should therefore:

1. Patch every Iced manifest edge to cosmic-text to use
   `default-features = false` with only the reviewed shaping/raster features.
2. Vendor cosmic-text `0.15.0`; change its `std` feature from
   `fontdb/memmap` to `fontdb/std`, and remove the `make_shared_face_data` call
   that exists only to convert file-backed faces. Application faces are already
   shared `Source::Binary` values.
3. Keep `fontconfig`, `fontdb/fs`, and `fontdb/memmap` absent from the resolved
   feature tree. Confirm that `fontconfig-parser`, `memmap2`, and target Linux
   native font libraries are absent from the target graph.
4. Audit the remaining `sys-locale` target branch even though the explicit
   `en-US` constructor does not invoke it. Its
   [Unix implementation](https://github.com/1Password/sys-locale/blob/v0.3.1/src/unix.rs)
   reads locale environment variables through Rust `std`; its `libc` dependency
   is Android-only in the
   [manifest](https://github.com/1Password/sys-locale/blob/v0.3.1/Cargo.toml).

This design removes system-font reads and the font stack's Linux fontconfig or
memory-map dependency branches. It does not remove the statically linked musl
runtime used by the executable, and it is not a complete dependency audit.

Meaningful invariants for the fork are:

- immediately after initialization, the database contains no faces;
- after app font registration, it contains exactly the expected Roboto face;
- every face has `fontdb::Source::Binary`; filesystem-backed variants are
  impossible because `fontdb/fs` is absent;
- an `strace -f -e trace=file` launch contains no reads under common system font
  locations or fontconfig configuration paths;
- `cargo tree -e features` contains no `fontconfig`, `fontdb/fs`,
  `fontdb/memmap`, `memmap2`, `softbuffer`, `winit`, `wgpu`, GTK, or WebKit
  branch.

## Roboto Regular candidate

Use one exact Apache-2.0 candidate for the first ASCII/Latin Project Home,
pending asset review. Android's immutable `external/roboto-fonts` commit is
`a781a172907fa1a48aead0911172988056f78e90`. Its
[`README.android`](https://android.googlesource.com/platform/external/roboto-fonts.git/+/a781a172907fa1a48aead0911172988056f78e90/README.android)
records upstream URL `https://github.com/google/roboto/archive/v2.136.zip`,
local package version `2.138`, license `Apache 2.0`, license file `NOTICE`, and
the Android build modifications. Preserve that distinction rather than calling
the binary an unmodified upstream 2.136 asset.

| Candidate file | SHA-256 |
| --- | --- |
| `README.android` | `3bfd4dd553fc9873470da664a5da2040d6a5cf3d5e6e575cb560c2d138caba95` |
| `MODULE_LICENSE_APACHE2` | `e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855` |
| [`NOTICE`](https://android.googlesource.com/platform/external/roboto-fonts.git/+/a781a172907fa1a48aead0911172988056f78e90/NOTICE) | `cfc7749b96f63bd31c3c42b5c471bf756814053e847c10f3eb003417bc523d30` |
| [`Roboto-Regular.ttf`](https://android.googlesource.com/platform/external/roboto-fonts.git/+/a781a172907fa1a48aead0911172988056f78e90/Roboto-Regular.ttf) | `f10a4d95fb922a38aeeae842005dfdc4c7ef1e7a66308f6379d749c44133b876` |

The future source lock must retain all four files or their exact text, record
the commit and hashes, install `Roboto-Regular.ttf` as app-owned data, and copy
the complete `NOTICE` into the packaged notices. The empty
`MODULE_LICENSE_APACHE2` marker is evidence of AOSP classification, not a
substitute for the notice. This candidate is not approved merely because the
repository says Apache 2.0.

Roboto Regular does not cover every script, emoji, weight, or visual use in the
current editor. The first fixture must stay within its verified glyph coverage.
Before any multilingual or icon claim, add separately reviewed bundled faces,
define deterministic fallback order, and test missing-glyph behavior. The app
must never silently fall back to a host font.

## Iced icon font disposition

Do not distribute Iced's embedded icon font. The Iced `0.14.0` file is 5,700
bytes with SHA-256
`2289860dbd2201295bf451deb155561abf4b1fb06899d6f1f81eb294bfebb7ec`.
Its metadata says `Copyright (C) 2020 by original authors @ fontello.com` and
that svg2ttf generated it from a Fontello project, but the tag contains no
Fontello configuration, source glyph SVG set, font-specific license, or
attribution identifying the original authors. The repository's
[`MIT` license](https://github.com/iced-rs/iced/blob/0.14.0/LICENSE) does not
resolve that separately asserted third-party provenance.

The history confirms continued regeneration without adding the missing source
record: the font began in
[`2c8ba65`](https://github.com/iced-rs/iced/commit/2c8ba652a7929ac6c2af28ac60a8bd4b8e8e2f10),
gained the dropdown glyph in
[`e29feef`](https://github.com/iced-rs/iced/commit/e29feef8ba4f95f286039fcc1ca2e53bfe5019c5),
was renamed/regenerated in
[`b29de28`](https://github.com/iced-rs/iced/commit/b29de28d1f0f608f8029c93d154cfd1b0f8b8cbb),
moved in
[`ed34543`](https://github.com/iced-rs/iced/commit/ed3454301e663a7cb7d73cd56b57b188f4d14a2f),
and later gained the Iced logo and autoscroll arrows in
[`1305d3c`](https://github.com/iced-rs/iced/commit/1305d3c00fabdf04af179d16135e4b5b1ff22740)
and
[`99748b8`](https://github.com/iced-rs/iced/commit/99748b89dea3dc24db24a73d79d5fa3e410b73a4).

The fork must delete `graphics/fonts/Iced-Icons.ttf` and patch every default
consumer, not merely avoid those widgets in the first screen:

| Patch file | Required change |
| --- | --- |
| `vendor/iced_graphics-0.14.0/src/text.rs` | Remove the `include_bytes!` and default icon-font registration while installing the empty bundled-only database. |
| `vendor/iced_core-0.14.0/src/text.rs` | Remove built-in icon constants from the renderer contract. |
| `vendor/iced_core-0.14.0/src/renderer/null.rs` | Remove the corresponding null-renderer constants. |
| `vendor/iced_tiny_skia-0.14.0/src/lib.rs` | Remove tiny-skia icon constants. The unselected wgpu implementation must not be vendored or built. |
| `vendor/iced_widget-0.14.0/src/checkbox.rs` | Default to a vector check while retaining the explicit caller-supplied font icon API. |
| `vendor/iced_widget-0.14.0/src/pick_list.rs` | Draw `Handle::Arrow` as a vector; retain caller-supplied static/dynamic handles. |
| `vendor/iced_widget-0.14.0/src/scrollable.rs` | Replace four autoscroll glyph draws with directional vectors. |
| `vendor/iced_widget-0.14.0/src/helpers.rs` | Remove the font-backed Iced-logo helper; restore the feature-gated SVG form only if the separately reviewed SVG feature is enabled, otherwise omit the helper. |

Use a private `widget/src/vector_icon.rs` helper built from axis-aligned
`core::Renderer::fill_quad` segments. The generic renderer already exposes this
operation, so checkmarks and stepped chevrons need no canvas/geometry/SVG
feature. A source-policy test must reject `Iced-Icons.ttf`, its hash, the
Fontello metadata string, `include_bytes!` references to it, and old default
glyph constants. Rendering tests must assert the vector marks occupy expected
regions at scale factors 1 and 2; a source grep alone does not establish visual
behavior.

## Proposed source-lock layout

The implementation plan may create these files only after the qualification
task starts:

```text
native/linux-desktop/
  Cargo.toml
  Cargo.lock
  source-lock.json
  linux-inventory.json
  assets/fonts/Roboto-Regular.ttf
  notices/ROBOTO-NOTICE.txt
  notices/ICED-LICENSE.txt
  vendor/cosmic-text-0.15.0/
  vendor/iced_core-0.14.0/
  vendor/iced_graphics-0.14.0/
  vendor/iced_tiny_skia-0.14.0/
  vendor/iced_widget-0.14.0/
```

`source-lock.json` should use one record per exact distributed or compiled
component. Each record needs `name`, `version`, `role`, source URL, immutable
revision when available, archive SHA-256, vendored-tree SHA-256, enabled Cargo
features, target, declared SPDX expression, selected allowed branch, license
and notice paths, patch identifiers, transitive component names, distribution
classification, and review status. Patch files require their own SHA-256 and
an upstream-base hash. Paths must be repository-relative, unique after path
normalization, and may not use `..`.

The direct crates.io archive hashes measured for the proposed bases are:

| Package | SHA-256 |
| --- | --- |
| `iced_core 0.14.0` | `91ab1937d699403e7e69252ae743a902bcee9f4ab2052cc4c9a46fcf34729d85` |
| `iced_runtime 0.14.0` | `d1889b819ce4c06674183242e336c8d49465665441396914dc07cc86f44fa8d4` |
| `iced_widget 0.14.0` | `b698f173f370d959cac589c2000268b44862543d6995cf8f2934cf07afad2e54` |
| `iced_graphics 0.14.0` | `234ca1c2cec4155055f68fa5fad1b5242c496ac8238d80a259bca382fb44a102` |
| `iced_tiny_skia 0.14.0` | `fe0acf8b75a3bc914aff5f2329fdffc1b36eeaea29dda0e4bd232f1c62e9cc3d` |
| `cosmic-text 0.15.0` | `173852283a9a57a3cbe365d86e74dc428a09c50421477d5ad6fe9d9509e37737` |

These hashes qualify candidate inputs only. Generate the exact target lockfile,
run `cargo metadata` and `cargo tree -e features`, and inventory every runtime,
build, procedural-macro, toolchain, static archive, font, and notice before
review. A source-lock verifier and permissive preflight result may report only
`inventory-eligible`; neither result is legal approval or release readiness.

## Existing-code boundary and first product slice

The Linux executable can reuse `video-creater` with
`default-features = false`, as the canonical core probe demonstrated. The first
screen should reuse `project::fixtures::sample_project()` and
`project::split::load_split_project(&Path)`, then show the typed project's name,
updated time, duration, media count, and track count. The existing public loader
acquires a mutation lease and may recover an interrupted transaction, so call it
only after an explicit `--project` selection and describe the action as opening,
not read-only inspection. Do not duplicate the project JSON schema in the
desktop crate.

The present React screen in
`src/components/workspace/project-home.tsx` and the browser baseline
`docs/visual-qa/browser-visual-baseline-linux-x64/home-desktop.png` are visual
references. The first native screen should preserve the dark 220-pixel action
rail, welcome hierarchy, sample card, project summary, focus visibility, and
clear load errors. Browser pixel equality is not expected because the text and
raster engines differ. Require stable layout-region assertions plus side-by-side
human review on a real compositor.

This slice deliberately excludes recent-project persistence (currently browser
local storage under `video-creater.recentProjects`), project creation, native
file dialogs, media preview, editor timeline, settings, model services, and
agent services. Those are later product slices. Extracting the full Tauri open
application service also requires coordinated ownership because the current
wrapper performs lease handling, recovery, active-session recording, render
recovery, generation recovery, and speech-analysis initialization.

## Verification and remaining gates

The first executable needs automated evidence for reducer behavior, canonical
project loading, font database contents, vector icons, host event translation,
software-frame stride/damage, and deterministic layout regions. A real nested
Wayland check must create the window, deliver configure/pointer/keyboard/focus
events, present a frame, exercise sample and explicit-project actions, capture
the compositor result, and close cleanly. Inspect the release ELF for no
`INTERP`, `NEEDED`, `RPATH`, or `RUNPATH`; inspect live `/proc/<pid>/maps` for no
shared object; and trace file opens to prove there is no host-font access.

These results remain external blockers or later gates:

- real physical Wayland desktops across the claimed compositor set, scaling,
  keyboards, pointer devices, disconnect/reconnect, and IME;
- X11 parity through the separately qualified x11rb protocol path;
- AT-SPI exposure and screen-reader/keyboard-only verification;
- portal-backed file selection, clipboard, drag and drop, notifications,
  credentials, and reveal behavior through separately audited clients;
- full native editor, media preview, audio, models, agent services, packaging,
  SBOM/notices, and offline clean-machine behavior;
- macOS build, packaged-app, signing, and regression verification on real Mac
  infrastructure. This Ubuntu VM cannot supply any of that evidence.

The concrete implementation sequence is in
[`docs/superpowers/plans/2026-09-12-linux-desktop-home.md`](../superpowers/plans/2026-09-12-linux-desktop-home.md).
