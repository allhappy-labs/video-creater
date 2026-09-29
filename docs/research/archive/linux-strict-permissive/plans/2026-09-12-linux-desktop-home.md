# Linux Native Project Home Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Deliver the first production-shaped Linux native executable: a bundled-font, CPU-rendered Wayland Project Home that opens the canonical sample or one explicitly supplied split-project directory and displays its typed summary.

**Architecture:** Create an isolated `native/linux-desktop` Rust/musl workspace. A custom wayrs host owns the Wayland connection and `wl_shm` lifecycle, selected patched Iced 0.14 components own widget layout and CPU drawing, and `video-creater` with default features disabled remains the canonical project model/loader. Dependency qualification is a hard first gate; application work does not begin until the exact source, feature, license, notice, toolchain, and target closure passes review.

**Tech Stack:** Rust 2021, `x86_64-unknown-linux-musl`, `wayrs-client = 1.3.1`, `wayrs-protocols = 0.14.11+1.45` with `xdg-shell`, patched Iced component crates `0.14.0`, patched cosmic-text `0.15.0`, tiny-skia CPU rendering, bundled AOSP Roboto Regular candidate at commit `a781a172907fa1a48aead0911172988056f78e90`.

**Spec:** `docs/superpowers/specs/2026-09-12-linux-compatibility-design.md`

## Global Constraints

- Follow `docs/research/2026-09-12-linux-desktop-fonts.md`; every candidate in that report remains unapproved until Task 1 finishes.
- Keep all implementation changes under the new `native/linux-desktop/` tree. Do not edit `src-tauri/`, its `Cargo.toml` or `Cargo.lock`, the React UI, media code, runtime-worker code, or existing proofs without a new coordinated task.
- Make `native/linux-desktop/Cargo.toml` an isolated workspace with its own `Cargo.lock`. Consume `../../src-tauri` using `default-features = false`.
- Use no umbrella `iced`, `iced_renderer`, `iced_winit`, Iced window features, wgpu, GTK, WebKit, Electron, libwayland, libxkbcommon, fontconfig, softbuffer, dynamic loader, file dialog, or host font.
- Bundle all application assets. Never search `PATH`, system font directories, package-manager locations, or developer checkouts at runtime.
- The first backend is Wayland only. An X11 implementation is a separately qualified later task.
- The first slice supports `--project <absolute-dir>` and the bundled sample. It does not advertise project creation, recent-project persistence, arbitrary text input, file dialogs, IME, clipboard, accessibility service exposure, media preview, timeline editing, settings, agents, models, or release parity.
- The existing `load_split_project` may take a mutation lease and recover an interrupted transaction. Invoke it only after an explicit project selection. Do not call it an inspection-only API.
- Each task begins with the failing focused test described below and ends with its focused command. Do not add tests that merely duplicate source expressions.
- Run every Cargo command through `native/linux-desktop/scripts/private-cargo.sh`. The launcher must use the retained private Rust/Cargo 1.97.1 paths, never `rustup`, a `cargo` found on `PATH`, or an implicit toolchain install.
- Audit the enabled `x86_64-unknown-linux-musl` normal target closure separately from host build scripts and procedural macros. Keep inactive platform-only lockfile packages in the resolution record, but do not reject or approve them as if they were compiled target dependencies.
- Serialize Task 1's compilation with the runtime and media builds. Check current free space immediately before Step 10 and do not start while another large native build is active.
- Use Conventional Commits. Preserve unrelated changes and ignored proof output.

---

### Task 1: Qualify and lock the exact desktop source closure

This is the first independently implementable task and a hard stop gate. It may create a qualification workspace and tests, but no Project Home behavior.

**Files:**

- Create: `native/linux-desktop/Cargo.toml`
- Create: `native/linux-desktop/Cargo.lock`
- Create: `native/linux-desktop/.cargo/config.toml`
- Create: `native/linux-desktop/scripts/private-cargo.sh`
- Create: `native/linux-desktop/scripts/source-policy.mjs`
- Create: `native/linux-desktop/scripts/source-policy.test.mjs`
- Create: `native/linux-desktop/src/lib.rs`
- Create: `native/linux-desktop/source-lock.json`
- Create: `native/linux-desktop/linux-inventory.json`
- Create: `native/linux-desktop/audit/cargo-metadata-musl.json`
- Create: `native/linux-desktop/audit/cargo-tree-musl.txt`
- Create: `native/linux-desktop/audit/target-normal.json`
- Create: `native/linux-desktop/audit/host-build.json`
- Create: `native/linux-desktop/assets/fonts/Roboto-Regular.ttf`
- Create: `native/linux-desktop/notices/ROBOTO-NOTICE.txt`
- Create: `native/linux-desktop/notices/ICED-LICENSE.txt`
- Create: `native/linux-desktop/vendor/` with the complete exact locked registry source closure
- Modify vendored files listed in `docs/research/2026-09-12-linux-desktop-fonts.md`
- Create: `native/linux-desktop/tests/font_database.rs`
- Create: `native/linux-desktop/tests/vector_icons.rs`

- [ ] **Step 1: Write the source-only policy test before Cargo resolution**

Create `source-policy.test.mjs` first. Fixture tests must cover an unpatched
Iced icon font, unconditional softbuffer, system-font initialization, duplicate
normalized source paths, a known prohibited license, an unknown/custom license,
an approved exact license record, a target-inactive package, and distinct
target-normal versus host-build classifications.

Run:

```bash
rtk node --test native/linux-desktop/scripts/source-policy.test.mjs
```

Expected: FAIL because `source-policy.mjs` does not exist.

Implement the source-only checker using Node built-ins only. It accepts a source
root, source-lock candidate, and optional resolved-graph receipts. Before a
Cargo graph exists it must inspect the exact candidate manifests/files directly
and report stable findings. Its status is exactly `blocked`, `review-required`,
or `source-policy-eligible`. Known prohibited license terms are `blocked`.
Unknown, custom, missing, or apparently permissive but unreviewed terms are
`review-required` with the exact source identity and evidence path; they are
neither silently approved nor universally rejected. Only an exact reviewed
source-lock decision can clear `review-required`. `source-policy-eligible`
means the checked source policy is satisfied; it is not legal approval or
release readiness. Collect and sort every finding before choosing the exit
status; never fail fast on an unrelated native, system, or license record and
hide the expected Iced/font finding.

Run the unit test again. Expected: PASS.

- [ ] **Step 2: Populate direct unmodified sources and prove the intended RED state**

Populate the five future `[patch.crates-io]` directories from the exact
unmodified archives named in the research report, plus the unmodified Roboto
candidate evidence. Do not create a Cargo lock or compile anything yet.

Run:

```bash
rtk node native/linux-desktop/scripts/source-policy.mjs \
  --source-root native/linux-desktop \
  --source-lock native/linux-desktop/source-lock.json
```

Expected: exit 1 with findings for the exact Iced icon font hash, default
system-font initialization, and unconditional tiny-skia softbuffer/window path.
This is the required RED result; native resolution or compilation cannot hide
it behind an earlier system dependency failure.

- [ ] **Step 3: Create the isolated manifest and exact private-Cargo launcher**

The root manifest must contain `[workspace]`, `publish = false`, a pinned
`rust-version`, and only exact dependency requirements. Patch every selected
Iced/cosmic package to the reviewed local source. Keep features explicit:

```toml
[package]
name = "linux-desktop"
version = "0.1.0"
edition = "2021"
rust-version = "1.88"
publish = false

[workspace]

[dependencies]
iced_core = { version = "=0.14.0", default-features = false }
iced_graphics = { version = "=0.14.0", default-features = false }
iced_runtime = { version = "=0.14.0", default-features = false }
iced_tiny_skia = { version = "=0.14.0", default-features = false }
iced_widget = { version = "=0.14.0", default-features = false }
tiny-skia = { version = "=0.11.4", default-features = false, features = ["simd", "std"] }
video-creater = { path = "../../src-tauri", default-features = false }
wayrs-client = { version = "=1.3.1", default-features = false }
wayrs-protocols = { version = "=0.14.11", default-features = false, features = ["xdg-shell"] }

[patch.crates-io]
cosmic-text = { path = "vendor/cosmic-text-0.15.0" }
iced_core = { path = "vendor/iced_core-0.14.0" }
iced_graphics = { path = "vendor/iced_graphics-0.14.0" }
iced_tiny_skia = { path = "vendor/iced_tiny_skia-0.14.0" }
iced_widget = { path = "vendor/iced_widget-0.14.0" }
```

`private-cargo.sh` resolves the repository root from its own checked-in path and
uses only these retained inputs:

```text
output/linux-combined-media-proof/inputs/rust/bin/cargo
output/linux-combined-media-proof/inputs/rust/bin/rustc
output/linux-combined-media-proof/inputs/cargo-home
```

It must reject a missing/nonexecuting path or a version other than Cargo/Rustc
1.97.1 at commit `8bab26f4f68e0e26f0bb7960be334d5b520ea452`, export the exact
`CARGO_HOME` and `RUSTC`, and `exec` the absolute Cargo path with caller
arguments. It must not run `rustup`, mutate the retained input tree, fall back
to `PATH`, or download a toolchain. Task 1 separately qualifies this exact
toolchain's Rust libraries, musl CRT/libc, compiler-builtins, unwind, linker,
and host Cargo inputs; prior proof use is orientation only.

Confirm that Cargo resolves the exact published package version
`0.14.11+1.45`, as the accepted proof did. Its exact manifest defines
`xdg-shell` as the only feature needed here; the client dependency is
unconditional and has no separate `client` feature.

- [ ] **Step 4: Resolve, fetch, hash, and retain every exact candidate source**

Vendor the complete locked source graph, including build and procedural-macro
packages. Verify the direct archive hashes in the research report before
extracting. Record the original archive hash and a canonical vendored-tree hash
for each patched component. Store patch identity and upstream-base hash in
`source-lock.json`; do not overwrite upstream origin with the local path.
Place registry snapshots under `native/linux-desktop/vendor/registry/` so the
generated source replacement cannot overwrite the five patched directories.

Run resolution/vendor commands through the private launcher; do not build:

```bash
rtk bash native/linux-desktop/scripts/private-cargo.sh generate-lockfile --manifest-path native/linux-desktop/Cargo.toml
rtk bash native/linux-desktop/scripts/private-cargo.sh vendor --manifest-path native/linux-desktop/Cargo.toml --locked native/linux-desktop/vendor/registry > native/linux-desktop/.cargo/config.toml
```

Copy the exact AOSP Roboto files from commit
`a781a172907fa1a48aead0911172988056f78e90`. Verify:

```text
Roboto-Regular.ttf  f10a4d95fb922a38aeeae842005dfdc4c7ef1e7a66308f6379d749c44133b876
NOTICE              cfc7749b96f63bd31c3c42b5c471bf756814053e847c10f3eb003417bc523d30
README.android      3bfd4dd553fc9873470da664a5da2040d6a5cf3d5e6e575cb560c2d138caba95
```

Retain `README.android` as source evidence in the lock or notices tree. Do not
replace the complete Apache notice with a short attribution.

- [ ] **Step 5: Apply the bundled-only cosmic-text/fontdb patch**

Patch every vendored Iced Cargo dependency edge to cosmic-text with
`default-features = false`. In the vendored cosmic-text manifest, keep the
features needed by the chosen Iced renderer while removing `fontconfig` and
`fontdb/memmap`; patch the file-backed sharing call because the database can
contain only binary sources.

In `vendor/iced_graphics-0.14.0/src/text.rs`, construct an empty database with
the explicit locale and load no font until the application registers Roboto:

```rust
let database = cosmic_text::fontdb::Database::new();
let raw = cosmic_text::FontSystem::new_with_locale_and_db(
    "en-US".to_owned(),
    database,
);
```

Add a small public test-only query in the fork if needed so
`tests/font_database.rs` can assert `faces().count() == 0` before registration
and one `Source::Binary` face after loading the exact embedded Roboto bytes.
The test must also shape and rasterize `Welcome to Video Creater` so it catches
a database that is pure but unusable.

Do not compile yet; Step 10 runs this test only after the entire selected source
and host-build graph has passed review.

- [ ] **Step 6: Remove the unresolved Iced icon font and replace default glyphs**

Delete `vendor/iced_graphics-0.14.0/fonts/Iced-Icons.ttf`. Remove its renderer
constants from the exact core, null, and tiny-skia sites in the research report.
Add `vendor/iced_widget-0.14.0/src/vector_icon.rs` and draw the default checkbox,
pick-list arrow, and autoscroll arrows with `Renderer::fill_quad`. Retain
caller-supplied custom font icons, which remain the caller's provenance
responsibility. Remove the font-backed Iced-logo helper; do not enable SVG just
to preserve a helper unused by the product.

Use scale-relative axis-aligned segments, for example:

```rust
pub(crate) fn down_chevron<R: iced_core::Renderer>(
    renderer: &mut R,
    bounds: iced_core::Rectangle,
    color: iced_core::Color,
) {
    let unit = (bounds.width.min(bounds.height) / 8.0).max(1.0);
    for step in 0..3 {
        let y = bounds.center_y() - unit + step as f32 * unit;
        for x in [bounds.center_x() - (2 - step) as f32 * unit,
                  bounds.center_x() + (2 - step) as f32 * unit] {
            renderer.fill_quad(
                iced_core::renderer::Quad {
                    bounds: iced_core::Rectangle::new(
                        iced_core::Point::new(x, y),
                        iced_core::Size::new(unit, unit),
                    ),
                    ..Default::default()
                },
                color,
            );
        }
    }
}
```

Test actual 32×32 and 64×64 rendered buffers: the center/background and at least
four mark pixels must differ in the expected symmetric regions. Also assert the
old font bytes, hash, Fontello string, include path, and default glyph constants
are absent.

Do not compile yet.

- [ ] **Step 7: Remove the unused softbuffer window graph**

In the vendored tiny-skia manifest, make `softbuffer` optional behind a new
`window` feature. Gate `pub mod window` and the window compositor/default
implementation, including the `renderer::Headless` implementation that calls
the window screenshot helper. Keep `Renderer::draw` available without that
feature. Do not replace softbuffer with another window crate.

Run:

```bash
rtk node native/linux-desktop/scripts/source-policy.mjs --source-root native/linux-desktop --source-lock native/linux-desktop/source-lock.json
```

Expected: the three intended patch findings are gone. Unreviewed dependency
licenses remain explicit `review-required` findings and keep exit status 1; the
checker must not call the source set approved before Step 9.

- [ ] **Step 8: Reconcile target-normal and host-build graphs separately**

Generate exact locked metadata plus one raw tree containing normal and build
edges for the musl selection. `source-policy.mjs` must derive two separately
classified receipts from metadata target kinds and dependency reachability:

- `target-normal.json`: code linked into the musl executable, excluding build
  scripts, procedural macros, and dependencies used only by those host tools;
- `host-build.json`: every build script, procedural macro, and its complete
  host-executed dependency closure.

Reconcile both receipts against `source-lock.json` and `linux-inventory.json`,
along with native archives, Rust sysroot inputs, fonts, notices, and patches.
Record inactive platform-only lockfile packages as `resolution-only`; do not
fail merely because their names or licenses appear in `Cargo.lock`, and do not
count them in the target executable closure. A package used in both roles must
have both classifications rather than being dropped from one receipt. Every
compiled or distributed record needs exact source, version/revision, hash,
feature set, selected license branch, notices, role, and dependencies. Duplicate
normalized artifact paths, a selected package absent from the source lock, or
an unreviewed selected license fails the task.

Run:

```bash
rtk bash native/linux-desktop/scripts/private-cargo.sh metadata --manifest-path native/linux-desktop/Cargo.toml --locked --filter-platform x86_64-unknown-linux-musl --format-version 1 > native/linux-desktop/audit/cargo-metadata-musl.json
rtk bash native/linux-desktop/scripts/private-cargo.sh tree --manifest-path native/linux-desktop/Cargo.toml --locked --target x86_64-unknown-linux-musl -e normal,build --prefix depth --format "{p} {f} {lib}" > native/linux-desktop/audit/cargo-tree-musl.txt
rtk node native/linux-desktop/scripts/source-policy.mjs --source-root native/linux-desktop --source-lock native/linux-desktop/source-lock.json --metadata native/linux-desktop/audit/cargo-metadata-musl.json --cargo-tree native/linux-desktop/audit/cargo-tree-musl.txt --write-receipts native/linux-desktop/audit
rtk node scripts/linux-permissive-preflight.mjs native/linux-desktop/linux-inventory.json
```

Expected: the target-normal and host-build reports are separately classified,
the source policy names every remaining review-required selected component, the
evaluator may print `"status": "inventory-eligible"`, and reconciliation reports
zero missing or extra selected components. Preflight is declaration eligibility
only and does not override the source policy's review-required status.

- [ ] **Step 9: Perform the mandatory human source/license review**

Review all vendored source headers, manifests, license files, notice obligations,
toolchain inputs, enabled features, generated code, and the exact target closure.
Record the reviewer and decision in the source lock. A known prohibited term is
blocked. An unknown/custom term or an unreviewed permissive-looking term remains
`review-required` until its exact source and obligations are decided. If a
selected component is blocked or review-required, ambiguously generated, or
cannot be tied to its binary/source hash, stop here and report that concrete
component; do not generalize the result to unrelated versions or libraries. Do
not begin Task 2 on a conditional or partial result.

- [ ] **Step 10: Compile the reviewed font and widget tests**

Only after Step 9 records an exact reviewed decision for every selected
target-normal and host-build component, run:

```bash
rtk node native/linux-desktop/scripts/source-policy.mjs --source-root native/linux-desktop --source-lock native/linux-desktop/source-lock.json --metadata native/linux-desktop/audit/cargo-metadata-musl.json --cargo-tree native/linux-desktop/audit/cargo-tree-musl.txt --write-receipts native/linux-desktop/audit
rtk bash native/linux-desktop/scripts/private-cargo.sh test --manifest-path native/linux-desktop/Cargo.toml --locked --test font_database --test vector_icons --target x86_64-unknown-linux-musl
rtk bash native/linux-desktop/scripts/private-cargo.sh tree --manifest-path native/linux-desktop/Cargo.toml --locked --target x86_64-unknown-linux-musl -e features
```

Expected: source policy prints `source-policy-eligible`; both tests pass with zero
initial faces, one binary Roboto face, nonempty shaped pixels, and working vector
marks. The selected feature tree contains no `softbuffer`,
`raw-window-handle`, `winit`, `wgpu`, Iced X11/Wayland, fontconfig,
`fontconfig-parser`, `memmap2`, GTK, WebKit, or dynamic-loader branch.

- [ ] **Step 11: Commit the qualified source gate**

```bash
rtk git add native/linux-desktop
rtk git commit -m "feat(linux): qualify native desktop rendering sources"
```

---

### Task 2: Add the canonical project-open boundary

**Files:**

- Modify: `native/linux-desktop/src/lib.rs`
- Create: `native/linux-desktop/src/project_session.rs`
- Create: `native/linux-desktop/tests/project_session.rs`

- [ ] **Step 1: Write failing summary and explicit-open tests**

Define the product-facing value without copying persisted project structs:

```rust
#[derive(Debug, Clone, PartialEq)]
pub struct ProjectSummary {
    pub root: Option<PathBuf>,
    pub id: String,
    pub name: String,
    pub updated_at: String,
    pub duration_seconds: f64,
    pub media_count: usize,
    pub track_count: usize,
}

pub trait ProjectGateway {
    fn open_sample(&self) -> ProjectSummary;
    fn open_selected(&self, root: &Path) -> Result<ProjectSummary, ProjectOpenError>;
}
```

Test `open_sample()` against `project::fixtures::sample_project()`. For the path
case, create an isolated temporary directory using `std`, save the real sample
with `project::split::save_split_project`, reopen it through
`load_split_project`, and compare all summary fields. Test a missing manifest
and malformed split project as distinct user-safe errors. Remove the exact
temporary directory in a guard even after assertion failure.

Run:

```bash
rtk bash native/linux-desktop/scripts/private-cargo.sh test --manifest-path native/linux-desktop/Cargo.toml --test project_session --target x86_64-unknown-linux-musl
```

Expected: FAIL because `ProjectGateway` does not exist.

- [ ] **Step 2: Implement the adapter using the canonical Rust types**

`CanonicalProjectGateway::open_sample` calls `sample_project()`.
`open_selected` rejects nonabsolute input before calling `load_split_project`.
Map the returned `VideoProject` into `ProjectSummary`; count
`project.timeline.tracks.len()` and `project.media.len()`. Preserve the selected
root, but never expose internal filesystem errors with credentials or unrelated
paths.

Run the focused test again. Expected: PASS.

- [ ] **Step 3: Commit**

```bash
rtk git add native/linux-desktop/src native/linux-desktop/tests/project_session.rs
rtk git commit -m "feat(linux): add canonical project open boundary"
```

---

### Task 3: Implement the Project Home reducer and semantic contract

**Files:**

- Create: `native/linux-desktop/src/app.rs`
- Create: `native/linux-desktop/src/semantics.rs`
- Modify: `native/linux-desktop/src/lib.rs`
- Create: `native/linux-desktop/tests/project_home_state.rs`

- [ ] **Step 1: Write failing state-transition tests**

Use a reducer with injected `ProjectGateway`:

```rust
pub enum HomeMessage {
    OpenSample,
    OpenSelected,
    FocusNext,
    FocusPrevious,
    ActivateFocused,
    CloseRequested,
}

pub enum HomeStatus {
    Ready,
    Opening,
    Opened(ProjectSummary),
    Error { title: String, detail: String },
    Closing,
}

pub struct ProjectHomeState {
    pub selected_root: Option<PathBuf>,
    pub focused_action: HomeAction,
    pub status: HomeStatus,
}
```

Test sample success, selected project success, selected action absent, load
error, reactivation after error, forward/backward focus wrap, and close. Use a
fake gateway that records call count so double activation while `Opening` is
proved inert.

Add a semantic snapshot with stable IDs, role, name, state, bounds, and child
order. Assert the ready tree exposes `Project home`, `Open Sample`, and only
exposes `Open Selected Project` when a path is supplied; opened and error states
must be announced. This is an internal contract, not AT-SPI evidence.

Run:

```bash
rtk bash native/linux-desktop/scripts/private-cargo.sh test --manifest-path native/linux-desktop/Cargo.toml --test project_home_state --target x86_64-unknown-linux-musl
```

Expected: FAIL before the reducer exists.

- [ ] **Step 2: Implement pure state and semantic projection**

Keep filesystem calls behind the gateway. `update` returns whether a redraw is
needed and never touches Wayland. Derive the semantic tree from the same state
used by the visual tree so enabled/focused/error state cannot diverge.

Run the focused test again. Expected: PASS.

- [ ] **Step 3: Commit**

```bash
rtk git add native/linux-desktop/src/app.rs native/linux-desktop/src/semantics.rs native/linux-desktop/src/lib.rs native/linux-desktop/tests/project_home_state.rs
rtk git commit -m "feat(linux): add Project Home state contract"
```

---

### Task 4: Render the native Project Home into deterministic CPU pixels

**Files:**

- Create: `native/linux-desktop/src/ui/mod.rs`
- Create: `native/linux-desktop/src/ui/fonts.rs`
- Create: `native/linux-desktop/src/ui/theme.rs`
- Create: `native/linux-desktop/src/ui/project_home.rs`
- Create: `native/linux-desktop/src/software_frame.rs`
- Modify: `native/linux-desktop/src/lib.rs`
- Create: `native/linux-desktop/tests/project_home_render.rs`
- Create: `native/linux-desktop/tests/software_frame.rs`

- [ ] **Step 1: Write failing frame-conversion tests**

Define a checked conversion from tiny-skia premultiplied RGBA bytes to Wayland
little-endian XRGB8888 bytes. Reject zero dimensions, integer overflow, short
source/destination slices, and stride smaller than `width * 4`. Test opaque
black, white, cyan, and a padded destination stride. Assert the exact B,G,R,0
byte order for XRGB on a little-endian target.

Run:

```bash
rtk bash native/linux-desktop/scripts/private-cargo.sh test --manifest-path native/linux-desktop/Cargo.toml --test software_frame --target x86_64-unknown-linux-musl
```

Expected: FAIL before `software_frame` exists.

- [ ] **Step 2: Implement the checked pixel conversion**

Keep raster scratch storage independent from Wayland buffers. Conversion must
write row by row using the destination stride and set the unused X byte
deterministically. Run the focused test. Expected: PASS.

- [ ] **Step 3: Write failing layout and pixel-region tests**

Render at logical 1024×640 with scale factors 1 and 2. Assert stable layout
regions rather than whole-image equality:

- the left rail is 220 logical pixels and differs from the main background;
- the welcome heading bounds do not intersect the rail;
- each enabled action has at least a 36-pixel logical hit target;
- focused action pixels contain the theme focus color;
- a loaded project shows the exact project name and numeric summary;
- an error shows the title/detail in the semantic tree and a visible error
  region;
- rendering the same state twice produces the same raw RGBA SHA-256.

Use `src/components/workspace/project-home.tsx` and
`docs/visual-qa/browser-visual-baseline-linux-x64/home-desktop.png` only as design
references. Do not assert exact browser/native pixel equality.

Run:

```bash
rtk bash native/linux-desktop/scripts/private-cargo.sh test --manifest-path native/linux-desktop/Cargo.toml --test project_home_render --target x86_64-unknown-linux-musl
```

Expected: FAIL before the view exists.

- [ ] **Step 4: Implement the first view**

Register only the bundled Roboto bytes. Build the Iced element tree with the
dark `#121314` canvas, `#1d2021` 220-pixel action rail, welcome heading, sample
card, optional selected-project action, project summary, error treatment, and
visible focus. Show only actions that work in this slice; do not put dependency
or implementation status in the user flow.

Run the two focused tests again. Expected: PASS.

- [ ] **Step 5: Commit**

```bash
rtk git add native/linux-desktop/src/ui native/linux-desktop/src/software_frame.rs native/linux-desktop/src/lib.rs native/linux-desktop/tests/project_home_render.rs native/linux-desktop/tests/software_frame.rs
rtk git commit -m "feat(linux): render native Project Home pixels"
```

---

### Task 5: Define the protocol-neutral desktop host

**Files:**

- Create: `native/linux-desktop/src/host.rs`
- Create: `native/linux-desktop/src/ui_driver.rs`
- Modify: `native/linux-desktop/src/lib.rs`
- Create: `native/linux-desktop/tests/host_event_translation.rs`
- Create: `native/linux-desktop/tests/ui_driver.rs`

- [ ] **Step 1: Write failing event and frame-lifecycle tests**

Use protocol-neutral types:

```rust
pub enum HostEvent {
    Configured { logical_size: LogicalSize, scale: u32 },
    FocusChanged(bool),
    PointerEntered { x: f64, y: f64 },
    PointerMoved { x: f64, y: f64 },
    PointerLeft,
    PointerButton { button: u32, state: ButtonState },
    PointerAxis { horizontal: f64, vertical: f64 },
    PhysicalKey { code: u32, state: KeyState, repeat: bool },
    FrameReady,
    CloseRequested,
    Disconnected,
}

pub trait PixelFrame {
    fn size(&self) -> PhysicalSize;
    fn stride(&self) -> usize;
    fn bytes_mut(&mut self) -> &mut [u8];
    fn present(self, damage: &[DamageRect]) -> Result<FrameToken, HostError>
    where
        Self: Sized;
}

pub trait DesktopBackend {
    type Frame<'a>: PixelFrame where Self: 'a;
    fn next_event(&mut self) -> Result<HostEvent, HostError>;
    fn acquire_frame(&mut self) -> Result<Self::Frame<'_>, HostError>;
    fn request_frame(&mut self) -> Result<(), HostError>;
}
```

Test configure-before-draw, scale change, focus, pointer coordinates, button
activation, raw keyboard control keys, one redraw for a burst of invalidations,
frame backpressure, close, and disconnect. A fake backend must reject acquiring
a busy frame and reject presenting the same token twice.

Run:

```bash
rtk bash native/linux-desktop/scripts/private-cargo.sh test --manifest-path native/linux-desktop/Cargo.toml --test host_event_translation --test ui_driver --target x86_64-unknown-linux-musl
```

Expected: FAIL before the host contract exists.

- [ ] **Step 2: Implement event translation and the UI driver**

The driver owns `ProjectHomeState`, Iced runtime state, the tiny-skia renderer,
damage tracking, and RGBA scratch pixels. Map physical Linux input codes only
for Escape, Tab, Enter, and Space in this slice. Do not infer typed characters
or modifiers without a qualified XKB/text-input implementation. Drop redraws
while no released frame exists and request one when a buffer becomes available.

Run the focused tests again. Expected: PASS.

- [ ] **Step 3: Commit**

```bash
rtk git add native/linux-desktop/src/host.rs native/linux-desktop/src/ui_driver.rs native/linux-desktop/src/lib.rs native/linux-desktop/tests/host_event_translation.rs native/linux-desktop/tests/ui_driver.rs
rtk git commit -m "feat(linux): add native desktop host contract"
```

---

### Task 6: Implement the Wayland backend and executable

**Files:**

- Create: `native/linux-desktop/src/platform/mod.rs`
- Create: `native/linux-desktop/src/platform/wayland.rs`
- Create: `native/linux-desktop/src/cli.rs`
- Create: `native/linux-desktop/src/main.rs`
- Modify: `native/linux-desktop/src/lib.rs`
- Create: `native/linux-desktop/tests/cli.rs`
- Create: `native/linux-desktop/tests/wayland_protocol.rs`

- [x] **Step 1: Write failing CLI tests**

Parse arguments without adding a CLI framework. Accept no project, `--project
<absolute-dir>`, `--evidence-dir <absolute-dir>`, and `--dump-frame
<absolute-file>`. Reject missing values, relative paths, duplicates, unknown
flags, and non-UTF-8 only with a stable error and exit code 2. `--dump-frame`
writes the app-owned pixel buffer for evidence; label it as a client frame, not
a compositor capture.

Run:

```bash
rtk bash native/linux-desktop/scripts/private-cargo.sh test --manifest-path native/linux-desktop/Cargo.toml --test cli --target x86_64-unknown-linux-musl
```

Expected: FAIL before CLI parsing exists.

- [x] **Step 2: Write failing Wayland state-machine tests**

Test protocol callbacks without a live compositor: required global discovery,
version caps, wm-base ping/pong, configure/ack order, zero-size configure fallback,
XRGB8888 advertisement, seat capability addition/removal, pointer translation,
keyboard keymap-fd closure, focus, raw key/repeat events, two-buffer release/reuse,
resize replacement after releases, frame callback, close, and disconnect. Verify
dimension/stride calculations reject overflow before file creation.

Run:

```bash
rtk bash native/linux-desktop/scripts/private-cargo.sh test --manifest-path native/linux-desktop/Cargo.toml --test wayland_protocol --target x86_64-unknown-linux-musl
```

Expected: FAIL before the backend exists.

- [x] **Step 3: Implement only the required Wayland protocols**

Use wayrs core protocol plus xdg-shell. Create each `wl_shm` pool from an
app-owned file under the selected runtime directory, size it exactly, transfer
the descriptor, remove the directory entry after creation, and write converted
XRGB bytes through the retained descriptor. Never mmap the file. Keep two
buffers and do not write or destroy one until its release event. A compositor
disconnect becomes a stable nonzero exit, not a retry loop.

Handle the exact protocol surface listed in the research report. Receive and
close the XKB keymap descriptor but do not parse it in this slice. Preserve raw
key codes for the four navigation/activation controls. Do not add cursor,
clipboard, portal, IME, fractional-scale, decoration, or accessibility protocols
inside this task.

Implementation ruling: use optional `wl_output` versions 1 through 3 for the
largest integer scale among entered outputs, and emit no synthetic key repeat
for the bounded controls. A resize replaces the two-buffer generation only
after every busy stale buffer is released. The draw scheduler requires a
pending request, a configured surface, a released buffer, and frame-callback
permission; requests remain pending while a gate is closed and do not create an
idle commit loop. Evidence stays typed and content-free. Evidence and client
frame files are created without clobbering or following symlinks, kept open for
updates, and retained as persistent app-owned outputs. Only the temporary shm
directory entries are removed after their descriptors are opened.

Run the CLI and protocol tests again. Expected: PASS.

- [x] **Step 4: Wire the executable**

Construct `CanonicalProjectGateway`, the reducer, UI driver, and
`WaylandBackend`. Load a selected project only after activation. Log stable
machine-readable evidence events when `--evidence-dir` is present: globals,
configure, focus, input, buffer commit/release, frame callback, selected action,
project result, close, and clean exit. Logs contain no project contents or
environment dump.

Run all target tests:

```bash
rtk bash native/linux-desktop/scripts/private-cargo.sh test --manifest-path native/linux-desktop/Cargo.toml --locked --target x86_64-unknown-linux-musl
```

Expected: PASS.

- [x] **Step 5: Commit**

```bash
rtk git add native/linux-desktop/src native/linux-desktop/tests
rtk git commit -m "feat(linux): run Project Home on Wayland"
```

---

### Task 7: Capture bounded integration and binary evidence

**Files:**

- Create: `native/linux-desktop/tests/wayland_smoke.sh`
- Create: `native/linux-desktop/tests/verify_evidence.py`
- Create: `native/linux-desktop/README.md`
- Create: `native/linux-desktop/evidence/.gitignore`

- [ ] **Step 1: Write the failing evidence verifier**

The verifier requires one current run with exact binary identity, source-lock
hash, Cargo.lock hash, target, compositor/service identity, event log, client
frame hash, ELF report, process maps, file-open trace, and cleanup report.
Reject stale hashes, a missing release event, no focus/input/action, missing
frame callback, project-open failure, duplicate paths, a client frame mislabeled
as compositor capture, any `.so` map, any system font/fontconfig open, or any
`INTERP`, `NEEDED`, `RPATH`, or `RUNPATH` entry.

Run against an empty evidence directory. Expected: FAIL with the complete
missing-artifact list.

- [ ] **Step 2: Build and run on an isolated nested Wayland service**

Build the locked release target, start the already qualified independent nested
Wayland service without installing packages or opening a network listener, and
run the executable once for the sample and once for an explicit real split
project fixture. Deliver configure, focus, pointer activation, Tab/Enter keyboard
activation, buffer release, and close. Capture the client frame and, only if the
service supplies an independently understood capture path, a separately labeled
compositor screenshot.

```bash
rtk bash native/linux-desktop/scripts/private-cargo.sh build --manifest-path native/linux-desktop/Cargo.toml --locked --release --target x86_64-unknown-linux-musl
rtk bash native/linux-desktop/tests/wayland_smoke.sh
rtk python3 native/linux-desktop/tests/verify_evidence.py native/linux-desktop/evidence/current
```

Expected: PASS for the bounded nested-compositor contract.

- [ ] **Step 3: Inspect dependency and runtime closure**

```bash
rtk bash native/linux-desktop/scripts/private-cargo.sh tree --manifest-path native/linux-desktop/Cargo.toml --locked --target x86_64-unknown-linux-musl -e features
rtk readelf -lW native/linux-desktop/target/x86_64-unknown-linux-musl/release/linux-desktop
rtk readelf -dW native/linux-desktop/target/x86_64-unknown-linux-musl/release/linux-desktop
rtk node scripts/linux-permissive-preflight.mjs native/linux-desktop/linux-inventory.json
```

Expected: the reviewed feature tree is unchanged; no ELF interpreter, needed
library, rpath, or runpath; preflight remains `inventory-eligible`. Reconcile
the linker map and shipped-file inventory to source-lock identities before
describing the binary as source-qualified.

- [ ] **Step 4: Perform visual and interaction review**

Compare the native real-compositor capture side by side with
`docs/visual-qa/browser-visual-baseline-linux-x64/home-desktop.png`. Record
viewport, scale, font hash, action hit targets, focus visibility, text clipping,
summary/error legibility, and observed differences. Exact pixels may differ;
missing hierarchy, broken spacing, invisible focus, or fallback glyph boxes fail.

- [ ] **Step 5: Document honest scope and external blockers**

`README.md` must state that this is a Wayland Project Home vertical slice. List
unimplemented product surfaces and keep the following external verification
open:

- physical Wayland desktops across the claimed compositors, scales, keyboards,
  pointing devices, suspend/resume, and disconnect behavior;
- a qualified keymap/text-input/IME path and multilingual bundled fonts;
- AT-SPI and screen-reader testing, keyboard-only navigation, high contrast,
  and reduced-motion behavior;
- X11, portals, clipboard, drag/drop, notifications, credentials, reveal, and
  packaging;
- editor/media/audio/model/agent integration and offline full-product workflow;
- macOS build, Xcode, signing, packaged application, and regression checks on
  real Mac infrastructure.

The Ubuntu VM cannot close the physical-desktop or any Mac gate.

- [ ] **Step 6: Commit**

```bash
rtk git add native/linux-desktop/tests/wayland_smoke.sh native/linux-desktop/tests/verify_evidence.py native/linux-desktop/README.md native/linux-desktop/evidence/.gitignore
rtk git commit -m "test(linux): verify native Project Home slice"
```

---

## Completion boundary

Completion of this plan establishes a reviewed, bundled-font, CPU-rendered
Wayland Project Home that exercises the canonical project boundary. It does not
establish full Linux compatibility or release readiness. Begin editor-shell,
media, audio, agent, model, desktop-service, X11, accessibility, packaging, and
Mac parity work only through their separately reviewed plans and gates.
