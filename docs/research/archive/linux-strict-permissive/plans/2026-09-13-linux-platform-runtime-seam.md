# Linux Platform Runtime Seam Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add the minimal Packet 4A contract for explicit platform identity, a closed Linux feature graph, and immutable fixed-root runtime ownership that Packet 2C can inject into canonical export and Settings.

**Architecture:** A build-script-provided target triple maps to `PlatformTarget::{MacOsApp, LinuxPermissive}` without runtime discovery. The Linux production constructor reads only `/usr/lib/video-creater/<CARGO_PKG_VERSION>/runtime-manifest.json`, while a lower-level explicit-root helper exists only for pure contract tests and package staging. `AppServices` owns the one resolver; Packet 4A documents later `&AppServices` consumer signatures but does not change application setup, existing export/Settings functions, or routing behavior.

**Tech Stack:** Rust 2021, existing `platform_runtime::{RuntimeManifest, RuntimeResolver}`, Cargo features/build scripts, an empty-workspace Rust contract harness, Node built-in audit tests, pinned Rust 1.97.1/Clang 18/musl private build inputs.

**Spec:** [`docs/superpowers/specs/2026-09-12-linux-compatibility-design.md`](../specs/2026-09-12-linux-compatibility-design.md)

**Roadmap:** [`docs/superpowers/plans/2026-09-12-linux-full-compatibility.md`](./2026-09-12-linux-full-compatibility.md), minimal Packet 4A prerequisite accepted by `.superpowers/sdd/2026-09-12-linux-full-compatibility/packet-2c-preparation.md` R2-A.

**Accepted baseline:** Start from `31d76749ca3382cdf429320ca45be89846f9b963`. Preserve the active independent W1 review and the separate Desktop review branch. Treat all retained `output/` evidence as immutable; create a new `output/linux-platform-runtime-seam/` evidence root only during authorized execution.

## Global constraints

- No app-linked or app-loaded LGPL/GPL/MPL component is allowed, including a system client library. Independent installed kernel, display, audio, notification, portal, and credential services remain allowed only through separately reviewed protocols.
- Linux tools and assets resolve only through the existing `RuntimeResolver` beneath the fixed immutable production root. Never search environment variables, argv, `PATH`, `current_exe`, shell state, package-manager directories, developer checkouts, writable model stores, or global plugin paths.
- The production Linux root is exactly `concat!("/usr/lib/video-creater/", env!("CARGO_PKG_VERSION"))`; its manifest child is exactly `runtime-manifest.json`. No host temporary/development path may be compiled into production.
- The explicit-root lower-level helper is for isolated pure tests and package staging. Its success is not production fixed-root execution. GNU host tests validate injected musl contract data and must never be labeled musl execution.
- Keep the root default feature list byte-for-byte unchanged. `linux-permissive` rejects combination with `app-runtime`, `custom-protocol`, `coreml-inspect`, `ges-render`, `gpu-render`, `graphics-render`, and `temporal-worker`. Leave the current `custom-protocol = ["tauri/custom-protocol"]` Mac edge unchanged.
- `gpu-render` and `graphics-render` remain rejected until their separate reviewed gates. F1 may later remove only the graphics exclusion after qualifying root cosmic-text 0.19/fontdb 0.23 and an empty database with a resolver-owned font.
- Make `tauri-build` optional and activate it only from `app-runtime`. Removing that host-build edge does not qualify any other component or feature combination.
- Do not modify `src-tauri/src/main.rs`, `package.json`, Tauri state setup, canonical export/Settings implementations, workflows, desktop gateway code, or existing zero-argument/public wrappers in Packet 4A.
- Do not add a second resolver, global mutable override, alternate manifest path, Linux fallback route, capability taxonomy, render backend, font, asset, runtime payload, or dependency.
- Use only the pinned closed launcher and already retained Cargo source set. Do not invoke global `cargo`, `rustup`, `musl-gcc`, download/install commands, or a network-enabled resolver.
- Source/license closure precedes the first compilation. Reuse a prior approval only when package identity, source/archive/tree content, enabled features, dependency roles, selected license, notices, and distribution role match exactly. Stop for focused independent audit on every mismatch.
- All implementation and fix commits stage only the task's owned paths and use Conventional Commits. Each task receives independent spec and code review before the next task changes an overlapping file.

## Preflight and ownership

| Gate | Accepted state | Binding rule |
| --- | --- | --- |
| R2-A | Accepted | Packet 4A owns target identity, Linux feature isolation, trusted bootstrap, immutable resolver ownership, and documented injection signatures only. |
| Production layout | Accepted | `/usr/lib/video-creater/<CARGO_PKG_VERSION>/runtime-manifest.json`; non-relocatable production package by design. |
| macOS compatibility | Frozen | Default feature list, `custom-protocol` edge, package scripts, Tauri setup, legacy APIs, Settings IDs, and behavior remain unchanged. Ubuntu source checks are not Mac runtime evidence. |
| Runtime resolver | Accepted existing API | Reuse `RuntimeResolver::from_install_root(&Path, RuntimeManifest, &str)` and its fresh resolve checks. Do not weaken or duplicate it. |
| Linux graph | Must close before compile | Empty-workspace harness uses `video-creater` with `default-features = false, features = ["linux-permissive"]`; normal, host-build, and resolution-only roles are reported separately. |
| Source approval baseline | Reuse candidate | Committed Desktop source lock from `96b83f4d`, current SHA-256 `b28582409a2c4572c6c33847dcf653acec5f3245c0578ed3b96bee2713c26e83`. Exact matches may be imported; mismatches require focused audit. |
| Known first-party role mismatch | Review required | The accepted `cargo:video-creater@0.1.0` record contains `cargo:tauri-build@2.6.3` in `host-build`. Task 1 intentionally removes that role, so the app record cannot be auto-imported even though the change narrows the graph. Rebind the first-party-private-development decision to the exact new dependency roles before compilation. |
| Closed Cargo launcher | Reuse unchanged after Desktop review | `native/linux-desktop/scripts/private-cargo.sh`, SHA-256 `f64a7cc0c9cb3d8e480e42de08e3c0c09854fe482b0d22a5939f87995c2e30a4`; it pins Cargo/rustc 1.97.1, Clang 18, musl, linkers, private Cargo home, and sanitized environment. It unsets `CARGO_BUILD_TARGET`; its `.cargo/config.toml` has no `[build] target` and only replaces crates.io with `native/linux-desktop/vendor/registry`. Therefore host commands are GNU and every musl command names `--target x86_64-unknown-linux-musl`. It hardcodes `native/linux-desktop/target` and its link-map directory, so 4A gets an exclusive build slot after Desktop review. |
| Root dev Tauri | Test-only obstacle | It is not in the downstream normal target closure but contaminates direct root package tests. Never use root `cargo test` as Linux closure evidence. |
| `tauri-build` | Current host-build defect | It remains a host build edge under `default-features = false` until Task 1 makes it optional. |
| W1/Desktop | Concurrent and preserved | Do not touch worker, desktop source/evidence, or their frozen outputs. Recheck status before every commit. |

Packet 4A owns only:

- `src-tauri/Cargo.toml`
- `src-tauri/build.rs`
- `src-tauri/src/lib.rs`
- `src-tauri/src/platform_target.rs`
- `src-tauri/src/app_services/mod.rs`
- `src-tauri/src/app_services/state.rs`
- `native/linux-platform-runtime-contract/`

It does not own `src-tauri/Cargo.lock` unless the locked resolver proves the optional build-dependency edit changes it. Do not rewrite the lock speculatively. It owns no roadmap/spec, runtime payload, shared resource, Desktop, W1, Packet 2C, F1, Settings, export, workflow, `main.rs`, or package-script file.

The reused launcher is the only accepted compiler entry but its target/cache paths are Desktop-owned. Root must finish the independent Desktop review and grant 4A the sole compilation slot before Task 2 Step 2. If that cache cannot be reserved without touching live Desktop work, stop: copying or parameterizing the launcher changes its reviewed hash and expands the owned-file/source-policy boundary, so root must amend and review this plan first.

## Corrected defect and cost ledger

| Defect closed by this revision | Binding correction | Cost retained |
| --- | --- | --- |
| Copied Desktop lock is not a valid final harness lock | One pinned private-Cargo offline regeneration, then exact package-record diff and `--locked` forever | Regeneration may expose a new version/source and block for audit; it may not fetch or silently update. |
| Optional build dependency left a statically referenced symbol | Task 1 cfg-guards the existing call before optionalization | `build.rs` is shared across Tasks 1-2 and receives review twice. |
| Target-specific assertions ran on both targets | GNU and musl expectations use `cfg`; both full test binaries run | More test time; evidence remains accurately split. |
| Shared target cache can contain stale test artifacts | Parse exact executable from current Cargo JSON | One small first-party audit adapter and negative parser tests. |
| Metadata-before-open manifest validation raced or blocked on FIFO | Retained nofollow/nonblocking descriptors plus post-open `fstat` and identity rechecks | Linux/Unix-specific descriptor code and bounded race tests; existing resolver semantics remain unchanged. |
| Fresh approvals/content were implicit | Exact reviewed mismatch records plus cumulative first-party path/hash bindings are evaluator inputs | Task 1 waits for focused role review; Tasks 2-3 refresh content before each compile and receive normal post-test review. |
| Proposed source checker duplicated policy | Thin adapter calls authoritative Desktop `evaluateSourcePolicy` unchanged | Adapter must translate sibling harness inputs and is blocked if that engine cannot accept the exact candidate lock/current byte roots. |

## Binding public interfaces

Task 2 produces:

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlatformTarget {
    MacOsApp,
    LinuxPermissive,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CompiledPlatformTarget {
    pub platform: PlatformTarget,
    pub target_triple: &'static str,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PlatformTargetError {
    UnsupportedTarget(String),
    FeatureTargetMismatch { feature: &'static str, target: String },
}

impl PlatformTarget {
    pub fn from_target_triple(target: &str) -> Result<Self, PlatformTargetError>;
}

pub const COMPILED_TARGET_TRIPLE: &str = env!("VIDEO_CREATER_TARGET_TRIPLE");
pub fn compiled_platform_target() -> Result<CompiledPlatformTarget, PlatformTargetError>;
```

Exact mappings are `aarch64-apple-darwin | x86_64-apple-darwin -> MacOsApp` and `x86_64-unknown-linux-musl -> LinuxPermissive`. GNU/Linux and every unlisted target return `UnsupportedTarget`. `linux-permissive` may compile on the GNU Linux host for pure tests, but `compiled_platform_target()` must truthfully reject the GNU compiled triple.

Task 3 produces:

```rust
pub const RUNTIME_MANIFEST_RELATIVE_PATH: &str = "runtime-manifest.json";
pub const LINUX_PRODUCTION_INSTALL_ROOT: &str =
    concat!("/usr/lib/video-creater/", env!("CARGO_PKG_VERSION"));
pub const MAX_RUNTIME_MANIFEST_BYTES: u64 = 1024 * 1024;

#[derive(Debug)]
pub struct AppServices {
    platform: CompiledPlatformTarget,
    runtime_resolver: Option<std::sync::Arc<RuntimeResolver>>,
}

#[derive(Debug)]
pub enum AppServicesBootstrapError {
    Platform(PlatformTargetError),
    InstallRootInvalid(String),
    ManifestMissing(String),
    ManifestTypeInvalid(String),
    ManifestWritable(String),
    ManifestTooLarge { bytes: u64, limit: u64 },
    ManifestRead(String),
    ManifestIdentityChanged(String),
    InstallRootIdentityChanged(String),
    Manifest(RuntimeManifestError),
    AppVersionMismatch { expected: String, actual: String },
    Resolver(RuntimeResolverError),
    RuntimeResolverUnavailable,
}

impl AppServices {
    pub fn bootstrap_linux_permissive() -> Result<Self, AppServicesBootstrapError>;
    pub const fn platform(&self) -> CompiledPlatformTarget;
    pub fn runtime_resolver(&self) -> Option<&RuntimeResolver>;
    pub fn require_runtime_resolver(
        &self,
    ) -> Result<&RuntimeResolver, AppServicesBootstrapError>;
}

pub fn bootstrap_linux_app_services_for_root(
    install_root: &std::path::Path,
    expected_target: &'static str,
    expected_app_version: &str,
) -> Result<AppServices, AppServicesBootstrapError>;
```

The explicit-root helper requires an absolute nonsymlink root and `PlatformTarget::from_target_triple(expected_target) == LinuxPermissive`. It opens the root as a retained nofollow directory descriptor, opens the fixed child relative to that descriptor with nofollow and nonblocking flags, and validates the post-open descriptor before a bounded read. It validates target and exact app version, proves the root path still names the retained root descriptor, calls the existing resolver, rechecks root identity, and returns staging-owned `AppServices`. Production calls that helper with compiled constants. There is no production path parameter or mutable replacement.

Packet 4A documents these exact future Packet 2C consumers in `app_services/mod.rs` module docs without implementing them:

```rust
pub fn render_media_to_split_project_folder_for_timeline_with_services(
    services: &AppServices,
    project_dir: &Path,
    project_id: &str,
    options: ExportRenderOptions,
    job: JobSummary,
    updated_at: &str,
    run_id: Option<String>,
    range_seconds: Option<(f64, f64)>,
    timeline_id: Option<&str>,
) -> PipelineResult<ProjectMediaRenderResult>;

pub fn get_render_system_health_with_services(
    services: &AppServices,
) -> RenderSystemHealth;
```

Packet 2C will add these functions, carry `services` into the canonical internal request, and migrate real Linux composition/workflow callers. Packet 4A must not add stubs, temporary unavailable behavior, cfg-remove legacy APIs, or construct Mac state.

---

### Task 1: Close the Linux feature and source graph before compilation

**Files:**

- Modify: `src-tauri/Cargo.toml:27-50,85-86`
- Modify: `src-tauri/build.rs:1-5`
- Create: `native/linux-platform-runtime-contract/Cargo.toml`
- Create: `native/linux-platform-runtime-contract/Cargo.lock`
- Create: `native/linux-platform-runtime-contract/src/lib.rs`
- Create: `native/linux-platform-runtime-contract/audit/lock-diff.mjs`
- Create: `native/linux-platform-runtime-contract/audit/lock-diff.test.mjs`
- Create: `native/linux-platform-runtime-contract/audit/first-party-content.mjs`
- Create: `native/linux-platform-runtime-contract/audit/first-party-content.test.mjs`
- Create: `native/linux-platform-runtime-contract/audit/source-reuse-adapter.mjs`
- Create: `native/linux-platform-runtime-contract/audit/source-reuse-adapter.test.mjs`
- Create: `native/linux-platform-runtime-contract/audit/reviewed-mismatches.json`
- Create: `native/linux-platform-runtime-contract/audit/first-party-content.json`
- Create: `native/linux-platform-runtime-contract/source-reuse-lock.json`

**Interfaces:**

- Consumes: accepted Desktop source decisions at commit `96b83f4d`; unchanged closed private Cargo launcher.
- Produces: empty `linux-permissive` feature; a compile-time guard around the existing Tauri build call; optional `tauri-build` activated only by `app-runtime`; a regenerated offline locked harness; explicit mismatch authorization and first-party content bindings; independently accepted exact source/feature/role closure required by Tasks 2-3.

- [ ] **Step 1: Write the source-reuse policy tests**

Create thin-adapter tests around the existing authoritative `native/linux-desktop/scripts/source-policy.mjs` engine. Require rejection for: unknown package/version/source, changed enabled features or target-normal/host-build role without an exact mismatch authorization, stale first-party content hashes, paths outside authorized task scope, unexpected `tauri-build`, stale baseline hash, and tampered current vendor source, archive, license/notice, Cargo metadata, tree, or lock bytes. Require acceptance only when the adapter constructs a candidate lock from exact imported approvals plus independently reviewed mismatch records and the unchanged authoritative evaluator returns `source-policy-eligible` with zero findings. Resolution-only packages remain separately reported.

Add `lock-diff` tests proving an offline regenerated lock may remove packages but may not add/change a registry or git name/version/source/checksum relative to the retained Desktop lock. Add `first-party-content` tests proving it hashes only explicit repository-relative regular files, rejects symlinks/duplicates/out-of-scope paths, records scope/authorization/distribution, and detects any post-binding edit.

- [ ] **Step 2: Run the pure policy tests and observe RED**

Run:

```bash
rtk node --test native/linux-platform-runtime-contract/audit/lock-diff.test.mjs native/linux-platform-runtime-contract/audit/first-party-content.test.mjs native/linux-platform-runtime-contract/audit/source-reuse-adapter.test.mjs
```

Expected: FAIL because the three policy implementations do not exist.

- [ ] **Step 3: Implement the lock/content tools and thin authoritative-policy adapter**

Use only Node built-ins in the owned adapter/binders. Do not implement a second license/source evaluator. Import `evaluateSourcePolicy` unchanged from `native/linux-desktop/scripts/source-policy.mjs`. The concrete mismatch requiring the adapter is that the existing CLI reads `Cargo.lock` only from `sourceRoot`, while this harness lock is a sibling; the accepted Desktop lock is also bound to Desktop manifest/metadata/tree hashes and the app's old feature/host-build roles.

The adapter reads the current harness metadata/tree/lock bytes, verifies the exact retained Desktop lock hash, and constructs a candidate lock by importing prior component records only when identity, roles, features, distribution, and review binding match. It applies only exact records from `reviewed-mismatches.json`, updates graph-input hashes/manifests to the harness inputs, and calls `evaluateSourcePolicy({ sourceRoot: desktopRoot, sourceLock: candidate, metadata, cargoTreeText, cargoLockText: harnessLock, mode: "resolved" })`.

The adapter translates every absolute metadata manifest into the exact path form accepted by the existing engine and proves the round trip before evaluation: the sibling harness is `../../native/linux-platform-runtime-contract/Cargo.toml`; repository app/protocol manifests retain `src-tauri/...`; registry manifests retain Desktop-root-relative `vendor/registry/...`. It binds the complete metadata manifest set, including the new harness first-party component, rather than copying the old Desktop manifest list. For every first-party component, it calls the existing exported `discoverCompileInputPaths`, hashes every current discovered input from repository bytes, and builds the candidate `compileInputs`; scoped content records authorize only the exact differences from the accepted binding. Unchanged entries are still rehashed. The Task 1 focused decision covers the new harness component and the app's changed Cargo/build bytes, features, dependencies, roles, complete compile-input binding, and exact private-development distribution. Task 2/3 authorizations may cover only their predeclared path sets with unchanged roles/features/dependencies/distribution.

The authoritative engine then independently recomputes and verifies current vendored source trees, archive/compile inputs, licenses, notices, patches, graph roles/features, metadata, tree, and lock bindings. The adapter must never copy an alleged current archive/tree/notice/compile-input hash from the prior lock and treat that self-comparison as current evidence. If any metadata manifest cannot round-trip through `resolveRecordPath`, or the existing engine reports a finding, the adapter fails closed; changing `source-policy.mjs` or `source-policy-lib.mjs` is outside 4A and requires a plan amendment.

`reviewed-mismatches.json` records exact `componentId`, `priorBindingSha256`, `candidateBindingSha256`, sorted `changedFields`, `allowedPaths`, `distribution: "compiled-target-source"`, `scope: "first-party-private-development"`, `decision: "approved"`, reviewer identity, and review timestamp. Only root-recorded independent focused decisions may populate it. `source-reuse-lock.json` copies the accepted allowed Task 2/3 path sets and role/distribution scope into `authorizedFirstPartyScopes`. `first-party-content.json` records per-task `scopeId`, `authorizationBindingSha256`, exact sorted path/SHA-256 pairs, and the same private development distribution. Tasks 2-3 may refresh content hashes only within those approved roles/files/distribution; any role, feature, path-scope, third-party, license, or notice change still blocks for focused independent audit before compilation.

The binder replaces only the named scope record and preserves/revalidates every other scope record. Thus Task 2 or Task 3 refresh cannot discard Task 1 bindings. A stale hash in any retained scope blocks the evaluator.

The CLI requires `--metadata`, `--cargo-tree`, `--cargo-lock`, `--prior-lock`, `--reuse-lock`, `--reviewed-mismatches`, and `--first-party-content`; verifies every raw input hash before parsing; validates authorization/content bindings; delegates source/license evaluation unchanged; prints canonical JSON; and returns nonzero on a finding or malformed argument. It makes no public MIT, redistribution, release, or patent claim.

- [ ] **Step 4: Guard the existing build call and make Cargo graph edits without compiling**

First make the existing build call compile-time conditional so optionalizing its crate cannot produce an unrelated RED:

```rust
fn main() {
    #[cfg(feature = "app-runtime")]
    tauri_build::build();
}
```

Do not emit the target yet; Task 2 owns that behavior. Keep the existing default array unchanged. Add `linux-permissive = []`. Add `"dep:tauri-build"` to `app-runtime`. Change only the build dependency declaration to:

```toml
tauri-build = { version = "2.6.2", features = [], optional = true }
```

Leave `custom-protocol = ["tauri/custom-protocol"]` unchanged. Create the harness manifest with an empty `[workspace]`, no build script, and this exact shape:

```toml
[package]
name = "linux-platform-runtime-contract"
version = "0.1.0"
edition = "2021"
rust-version = "1.87"
publish = false

[workspace]

[dependencies]
video-creater = { path = "../../src-tauri", default-features = false, features = ["linux-permissive"] }
```

Create `src/lib.rs` containing only `//! Isolated Linux platform/runtime contract harness.` so metadata has a real target without introducing test behavior before source closure.

Preserve the retained Desktop lock as the comparison input, seed a candidate only to constrain the closed offline resolver, then explicitly regenerate the harness lock with the pinned launcher. Do not use `--locked` for this one regeneration command because the copied lock belongs to a different workspace root:

```bash
rtk mkdir -p output/linux-platform-runtime-seam/task1-lock
rtk cp native/linux-desktop/Cargo.lock output/linux-platform-runtime-seam/task1-lock/desktop-before.lock
rtk cp native/linux-desktop/Cargo.lock native/linux-platform-runtime-contract/Cargo.lock
rtk native/linux-desktop/scripts/private-cargo.sh generate-lockfile --manifest-path native/linux-platform-runtime-contract/Cargo.toml --offline
rtk node native/linux-platform-runtime-contract/audit/lock-diff.mjs --before output/linux-platform-runtime-seam/task1-lock/desktop-before.lock --after native/linux-platform-runtime-contract/Cargo.lock --output output/linux-platform-runtime-seam/task1-lock/lock-diff.json
```

Expected: the pinned offline Cargo rewrites the lock for the harness root; the comparator reports only removal/root-workspace differences and zero new or changed registry/git name/version/source/checksum records. Inspect the canonical before/after report. Any new version/source/checksum blocks metadata and requires source audit; never rerun online or loosen a requirement. Every later Cargo command returns to `--locked`.

- [ ] **Step 5: Generate final metadata/tree with the closed launcher, without compilation**

Create a fresh nonshared evidence directory. Use the existing private launcher, never global Cargo:

```bash
rtk mkdir -p output/linux-platform-runtime-seam/task1-source
rtk native/linux-desktop/scripts/private-cargo.sh metadata --manifest-path native/linux-platform-runtime-contract/Cargo.toml --locked --offline --filter-platform x86_64-unknown-linux-musl --format-version 1 > output/linux-platform-runtime-seam/task1-source/cargo-metadata-musl.json
rtk native/linux-desktop/scripts/private-cargo.sh tree --manifest-path native/linux-platform-runtime-contract/Cargo.toml --locked --offline --target x86_64-unknown-linux-musl --edges normal,build > output/linux-platform-runtime-seam/task1-source/cargo-tree-musl.txt
```

Expected: metadata/tree generation succeeds offline. `tauri-build`, Tauri, GTK, GStreamer, temporal, wgpu, cosmic-text, and macOS-only dependencies are absent from normal and host-build reachability. No Rust compilation occurs.

- [ ] **Step 6: Bind exact source reuse and stop on every mismatch**

Write `source-reuse-lock.json` with schema version 1, target, baseline commit/path/hash, regenerated-lock and metadata/tree hashes, exact imported component records, separate resolution-only records, and zero findings. Bind Task 1's exact first-party owned files into `first-party-content.json`. Run the evaluator against the generated metadata, committed Desktop lock, mismatch authorizations, and content binding. The known first-party app role mismatch and any other feature/role/content/license/notice mismatch block Step 7. Root assigns a focused reviewer to inspect the exact changed role/content record; Sol records the accepted decision in `reviewed-mismatches.json` before rerunning the gate. Automatic import is allowed only for exact matches.

Run:

```bash
rtk node --test native/linux-platform-runtime-contract/audit/lock-diff.test.mjs native/linux-platform-runtime-contract/audit/first-party-content.test.mjs native/linux-platform-runtime-contract/audit/source-reuse-adapter.test.mjs
rtk node native/linux-platform-runtime-contract/audit/first-party-content.mjs --scope task1 --authorization native/linux-platform-runtime-contract/audit/reviewed-mismatches.json --output native/linux-platform-runtime-contract/audit/first-party-content.json --paths src-tauri/Cargo.toml,src-tauri/build.rs,native/linux-platform-runtime-contract/Cargo.toml,native/linux-platform-runtime-contract/Cargo.lock,native/linux-platform-runtime-contract/src/lib.rs,native/linux-platform-runtime-contract/audit/lock-diff.mjs,native/linux-platform-runtime-contract/audit/lock-diff.test.mjs,native/linux-platform-runtime-contract/audit/first-party-content.mjs,native/linux-platform-runtime-contract/audit/first-party-content.test.mjs,native/linux-platform-runtime-contract/audit/source-reuse-adapter.mjs,native/linux-platform-runtime-contract/audit/source-reuse-adapter.test.mjs
rtk node native/linux-platform-runtime-contract/audit/source-reuse-adapter.mjs --metadata output/linux-platform-runtime-seam/task1-source/cargo-metadata-musl.json --cargo-tree output/linux-platform-runtime-seam/task1-source/cargo-tree-musl.txt --cargo-lock native/linux-platform-runtime-contract/Cargo.lock --prior-lock native/linux-desktop/source-lock.json --reuse-lock native/linux-platform-runtime-contract/source-reuse-lock.json --reviewed-mismatches native/linux-platform-runtime-contract/audit/reviewed-mismatches.json --first-party-content native/linux-platform-runtime-contract/audit/first-party-content.json
```

Expected after the focused decision is recorded: tests PASS and evaluator reports `eligible`, with zero findings. Root dispatches an independent source/license review of the exact metadata, tree, reuse lock, known app-role change, and imported approvals before authorizing any compilation.

- [ ] **Step 7: Commit only the closed graph paths after source review passes**

```bash
rtk git add src-tauri/Cargo.toml src-tauri/build.rs native/linux-platform-runtime-contract
rtk git commit --only -m "build(linux): isolate platform runtime feature graph" -- src-tauri/Cargo.toml src-tauri/build.rs native/linux-platform-runtime-contract/Cargo.toml native/linux-platform-runtime-contract/Cargo.lock native/linux-platform-runtime-contract/src/lib.rs native/linux-platform-runtime-contract/audit/lock-diff.mjs native/linux-platform-runtime-contract/audit/lock-diff.test.mjs native/linux-platform-runtime-contract/audit/first-party-content.mjs native/linux-platform-runtime-contract/audit/first-party-content.test.mjs native/linux-platform-runtime-contract/audit/source-reuse-adapter.mjs native/linux-platform-runtime-contract/audit/source-reuse-adapter.test.mjs native/linux-platform-runtime-contract/audit/reviewed-mismatches.json native/linux-platform-runtime-contract/audit/first-party-content.json native/linux-platform-runtime-contract/source-reuse-lock.json
```

Expected: one owned-path commit. Root obtains independent task review before Task 2.

---

### Task 2: Add explicit compiled target identity and feature guards

**Files:**

- Modify: `src-tauri/build.rs:1-5`
- Modify: `src-tauri/src/lib.rs:1-23`
- Create: `src-tauri/src/platform_target.rs`
- Create: `native/linux-platform-runtime-contract/tests/contract.rs`
- Create: `native/linux-platform-runtime-contract/audit/feature-policy.mjs`
- Create: `native/linux-platform-runtime-contract/audit/feature-policy.test.mjs`

**Interfaces:**

- Consumes: Task 1's accepted `linux-permissive` graph and exact source closure.
- Produces: the binding `PlatformTarget`, `CompiledPlatformTarget`, error, constant, and parser above; compile-time rejection of forbidden combinations; build-script target emission with Mac Tauri build behavior preserved.

- [ ] **Step 1: Write target parser and feature-policy RED tests**

In `contract.rs`, assert exact Apple/musl mappings, GNU/unknown rejection, and the compiled GNU host returning `UnsupportedTarget`. Add a target-only test named `compiled_musl_contract` that, when built for `x86_64-unknown-linux-musl`, requires `COMPILED_TARGET_TRIPLE` and `compiled_platform_target()` to report musl/Linux.

Use these named test seams:

```rust
#[test]
fn parser_contract() {
    assert_eq!(PlatformTarget::from_target_triple("x86_64-unknown-linux-musl"), Ok(PlatformTarget::LinuxPermissive));
    assert_eq!(PlatformTarget::from_target_triple("aarch64-apple-darwin"), Ok(PlatformTarget::MacOsApp));
    assert!(matches!(PlatformTarget::from_target_triple("x86_64-unknown-linux-gnu"), Err(PlatformTargetError::UnsupportedTarget(_))));
}

#[cfg(target_env = "gnu")]
#[test]
fn host_compiled_target_is_rejected() {
    assert!(matches!(compiled_platform_target(), Err(PlatformTargetError::UnsupportedTarget(_))));
}

#[cfg(target_env = "musl")]
#[test]
fn compiled_musl_contract() {
    assert_eq!(COMPILED_TARGET_TRIPLE, "x86_64-unknown-linux-musl");
    assert_eq!(compiled_platform_target().unwrap().platform, PlatformTarget::LinuxPermissive);
}
```

The Node feature-policy test reads `Cargo.toml`, `build.rs`, and `lib.rs`. Require the exact unchanged default array; optional `tauri-build` reachable from `app-runtime`; unchanged `custom-protocol` edge; target emission; and a guard set containing all seven forbidden combinations. It rejects missing/renamed guards and any Linux feature dependency.

Refresh Task 2's exact first-party content binding after writing the RED tests and before compiling them:

```bash
rtk mkdir -p output/linux-platform-runtime-seam/task2-red-source
rtk native/linux-desktop/scripts/private-cargo.sh metadata --manifest-path native/linux-platform-runtime-contract/Cargo.toml --locked --offline --filter-platform x86_64-unknown-linux-musl --format-version 1 > output/linux-platform-runtime-seam/task2-red-source/cargo-metadata-musl.json
rtk native/linux-desktop/scripts/private-cargo.sh tree --manifest-path native/linux-platform-runtime-contract/Cargo.toml --locked --offline --target x86_64-unknown-linux-musl --edges normal,build > output/linux-platform-runtime-seam/task2-red-source/cargo-tree-musl.txt
rtk node native/linux-platform-runtime-contract/audit/first-party-content.mjs --scope task2-red --authorization native/linux-platform-runtime-contract/source-reuse-lock.json --output native/linux-platform-runtime-contract/audit/first-party-content.json --paths native/linux-platform-runtime-contract/tests/contract.rs,native/linux-platform-runtime-contract/audit/feature-policy.test.mjs
rtk node native/linux-platform-runtime-contract/audit/source-reuse-adapter.mjs --metadata output/linux-platform-runtime-seam/task2-red-source/cargo-metadata-musl.json --cargo-tree output/linux-platform-runtime-seam/task2-red-source/cargo-tree-musl.txt --cargo-lock native/linux-platform-runtime-contract/Cargo.lock --prior-lock native/linux-desktop/source-lock.json --reuse-lock native/linux-platform-runtime-contract/source-reuse-lock.json --reviewed-mismatches native/linux-platform-runtime-contract/audit/reviewed-mismatches.json --first-party-content native/linux-platform-runtime-contract/audit/first-party-content.json
```

Expected: content binding and reuse gate PASS within the already accepted Task 2 paths/roles/private-development distribution. Any path/role/feature/third-party/license change stops before compilation for focused audit.

- [ ] **Step 2: Run the first authorized compilation and observe RED**

This step occurs only after Task 1's independent source review passes.

```bash
rtk native/linux-desktop/scripts/private-cargo.sh test --manifest-path native/linux-platform-runtime-contract/Cargo.toml --locked --no-run --test contract
rtk node --test native/linux-platform-runtime-contract/audit/feature-policy.test.mjs
```

Expected: Rust compilation fails because `platform_target` is absent; Node fails because the feature-policy implementation/guards are absent. The GNU result is a host compile check, never musl execution.

- [ ] **Step 3: Implement build target emission and platform mapping**

In `build.rs`, read Cargo's required `TARGET`, emit `cargo:rustc-env=VIDEO_CREATER_TARGET_TRIPLE=<value>`, and call `tauri_build::build()` only inside `#[cfg(feature = "app-runtime")]`. Do not use a runtime environment feature check to guard the crate reference.

Export `platform_target` from `lib.rs`. Implement the binding interfaces and exact mapping table. Add one compile-time guard for non-Linux `linux-permissive` and guards covering its combination with each of `app-runtime`, `custom-protocol`, `coreml-inspect`, `ges-render`, `gpu-render`, `graphics-render`, and `temporal-worker`. GNU Linux may compile pure tests, but `compiled_platform_target()` returns `UnsupportedTarget` for its explicit triple.

Before any GREEN compilation, refresh the binding over the full accepted Task 2 path set and rerun the thin authoritative-policy adapter:

```bash
rtk mkdir -p output/linux-platform-runtime-seam/task2-green-source
rtk native/linux-desktop/scripts/private-cargo.sh metadata --manifest-path native/linux-platform-runtime-contract/Cargo.toml --locked --offline --filter-platform x86_64-unknown-linux-musl --format-version 1 > output/linux-platform-runtime-seam/task2-green-source/cargo-metadata-musl.json
rtk native/linux-desktop/scripts/private-cargo.sh tree --manifest-path native/linux-platform-runtime-contract/Cargo.toml --locked --offline --target x86_64-unknown-linux-musl --edges normal,build > output/linux-platform-runtime-seam/task2-green-source/cargo-tree-musl.txt
rtk node native/linux-platform-runtime-contract/audit/first-party-content.mjs --scope task2-green --authorization native/linux-platform-runtime-contract/source-reuse-lock.json --output native/linux-platform-runtime-contract/audit/first-party-content.json --paths src-tauri/build.rs,src-tauri/src/lib.rs,src-tauri/src/platform_target.rs,native/linux-platform-runtime-contract/tests/contract.rs,native/linux-platform-runtime-contract/audit/feature-policy.mjs,native/linux-platform-runtime-contract/audit/feature-policy.test.mjs
rtk node native/linux-platform-runtime-contract/audit/source-reuse-adapter.mjs --metadata output/linux-platform-runtime-seam/task2-green-source/cargo-metadata-musl.json --cargo-tree output/linux-platform-runtime-seam/task2-green-source/cargo-tree-musl.txt --cargo-lock native/linux-platform-runtime-contract/Cargo.lock --prior-lock native/linux-desktop/source-lock.json --reuse-lock native/linux-platform-runtime-contract/source-reuse-lock.json --reviewed-mismatches native/linux-platform-runtime-contract/audit/reviewed-mismatches.json --first-party-content native/linux-platform-runtime-contract/audit/first-party-content.json
```

Expected: PASS with changed content only; a changed role/path/dependency blocks.

- [ ] **Step 4: Implement and pass the static feature policy**

Use Node built-ins only. Parse the relevant TOML/source lines conservatively and fail closed on ambiguity. Do not invoke Cargo with a forbidden feature combination: Cargo may build transitive forbidden packages before reaching this crate's `compile_error!`.

Run:

```bash
rtk node --test native/linux-platform-runtime-contract/audit/feature-policy.test.mjs
rtk node native/linux-platform-runtime-contract/audit/feature-policy.mjs
rtk native/linux-desktop/scripts/private-cargo.sh test --manifest-path native/linux-platform-runtime-contract/Cargo.toml --locked --test contract
rtk native/linux-desktop/scripts/private-cargo.sh test --manifest-path native/linux-platform-runtime-contract/Cargo.toml --locked --target x86_64-unknown-linux-musl --test contract
```

Expected: policy plus every applicable named contract test PASS on GNU and musl. GNU-only and musl-only compiled-target assertions are cfg-separated; neither suite contains an unconditional expectation for the other target. The musl command executes the static x86_64 musl test directly on this x86_64 Linux host; if execution is blocked, record that exact status and do not relabel compilation as execution.

- [ ] **Step 5: Recheck final graph/content binding and commit owned paths**

Regenerate Task 1 metadata/tree into a new `task2` evidence child, refresh the exact Task 2 content record, and rerun the thin authoritative-policy adapter. Expected: zero new/mismatched components, no Tauri build edge, and final content hashes match the files about to be committed.

```bash
rtk mkdir -p output/linux-platform-runtime-seam/task2-source
rtk native/linux-desktop/scripts/private-cargo.sh metadata --manifest-path native/linux-platform-runtime-contract/Cargo.toml --locked --offline --filter-platform x86_64-unknown-linux-musl --format-version 1 > output/linux-platform-runtime-seam/task2-source/cargo-metadata-musl.json
rtk native/linux-desktop/scripts/private-cargo.sh tree --manifest-path native/linux-platform-runtime-contract/Cargo.toml --locked --offline --target x86_64-unknown-linux-musl --edges normal,build > output/linux-platform-runtime-seam/task2-source/cargo-tree-musl.txt
rtk node native/linux-platform-runtime-contract/audit/first-party-content.mjs --scope task2-green --authorization native/linux-platform-runtime-contract/source-reuse-lock.json --output native/linux-platform-runtime-contract/audit/first-party-content.json --paths src-tauri/build.rs,src-tauri/src/lib.rs,src-tauri/src/platform_target.rs,native/linux-platform-runtime-contract/tests/contract.rs,native/linux-platform-runtime-contract/audit/feature-policy.mjs,native/linux-platform-runtime-contract/audit/feature-policy.test.mjs
rtk node native/linux-platform-runtime-contract/audit/source-reuse-adapter.mjs --metadata output/linux-platform-runtime-seam/task2-source/cargo-metadata-musl.json --cargo-tree output/linux-platform-runtime-seam/task2-source/cargo-tree-musl.txt --cargo-lock native/linux-platform-runtime-contract/Cargo.lock --prior-lock native/linux-desktop/source-lock.json --reuse-lock native/linux-platform-runtime-contract/source-reuse-lock.json --reviewed-mismatches native/linux-platform-runtime-contract/audit/reviewed-mismatches.json --first-party-content native/linux-platform-runtime-contract/audit/first-party-content.json
```

```bash
rtk git add src-tauri/build.rs src-tauri/src/lib.rs src-tauri/src/platform_target.rs native/linux-platform-runtime-contract/tests/contract.rs native/linux-platform-runtime-contract/audit/feature-policy.mjs native/linux-platform-runtime-contract/audit/feature-policy.test.mjs native/linux-platform-runtime-contract/audit/first-party-content.json
rtk git commit --only -m "feat(linux): add explicit platform target contract" -- src-tauri/build.rs src-tauri/src/lib.rs src-tauri/src/platform_target.rs native/linux-platform-runtime-contract/tests/contract.rs native/linux-platform-runtime-contract/audit/feature-policy.mjs native/linux-platform-runtime-contract/audit/feature-policy.test.mjs native/linux-platform-runtime-contract/audit/first-party-content.json
```

Expected: one owned-path commit. Root obtains independent spec and code review before Task 3; Sol alone applies any requested fixes in a separate owned-path Conventional Commit.

---

### Task 3: Bootstrap and own the fixed-root runtime resolver

**Files:**

- Modify: `src-tauri/src/lib.rs`
- Create: `src-tauri/src/app_services/mod.rs`
- Create: `src-tauri/src/app_services/state.rs`
- Modify: `native/linux-platform-runtime-contract/tests/contract.rs`
- Create: `native/linux-platform-runtime-contract/audit/verify-cargo-elf.mjs`
- Create: `native/linux-platform-runtime-contract/audit/verify-cargo-elf.test.mjs`

**Interfaces:**

- Consumes: Task 2's compiled target identity and the existing accepted manifest/resolver API.
- Produces: the binding constants, `AppServices`, bootstrap error, fixed-root production constructor, explicit-root pure helper, and resolver accessors above; documented Packet 2C `&AppServices` consumer signatures with no consumer implementation.

- [ ] **Step 1: Write bootstrap RED tests using only standard-library fixtures**

Extend `contract.rs` with a unique temp root made from `std::env::temp_dir`, process ID, and an atomic counter; set root/directories to `0o755`, files to `0o644`, and always remove only the owned fixture. Build exact manifest JSON bytes and SHA-256 values already used by `native/linux-runtime-contract`; add no test dependency.

Assert: fixed constants; explicit helper success for injected `x86_64-unknown-linux-musl` and exact app version; GNU/Apple/unknown injected target rejection; relative root; symlink root/manifest; manifest FIFO/directory; missing/writable/oversized/malformed/wrong-target/wrong-app-version manifest; missing/corrupt/protocol-old artifact; root replacement before and during bootstrap; manifest replacement during open/read; `runtime_resolver()` and `require_runtime_resolver()` identity; and decoys in argv/environment/`PATH` never selected. A FIFO test has a short external watchdog and must return a typed nonregular error without waiting for a writer.

Keep the aggregate happy-path seam explicit:

```rust
#[test]
fn bootstrap_contract() {
    let fixture = RuntimeFixture::new("x86_64-unknown-linux-musl", env!("CARGO_PKG_VERSION"));
    let services = bootstrap_linux_app_services_for_root(
        fixture.root(),
        "x86_64-unknown-linux-musl",
        env!("CARGO_PKG_VERSION"),
    ).expect("explicit musl staging contract");
    assert_eq!(services.platform().platform, PlatformTarget::LinuxPermissive);
    assert_eq!(services.require_runtime_resolver().unwrap().expected_target(), "x86_64-unknown-linux-musl");
    assert_eq!(LINUX_PRODUCTION_INSTALL_ROOT, concat!("/usr/lib/video-creater/", env!("CARGO_PKG_VERSION")));
}

#[cfg(target_env = "gnu")]
#[test]
fn production_bootstrap_rejects_gnu_target() {
    assert!(matches!(AppServices::bootstrap_linux_permissive(), Err(AppServicesBootstrapError::Platform(_))));
}

#[cfg(target_env = "musl")]
#[test]
fn production_bootstrap_uses_compiled_musl_identity() {
    assert_eq!(compiled_platform_target().unwrap().platform, PlatformTarget::LinuxPermissive);
    assert_eq!(LINUX_PRODUCTION_INSTALL_ROOT, concat!("/usr/lib/video-creater/", env!("CARGO_PKG_VERSION")));
}
```

Add one named test per failure class above so the aggregate cannot hide a skipped corruption case. `RuntimeFixture` exposes only its owned root and removes only that root in `Drop`.

For manifest/root replacement races, use a start barrier and a bounded mutation thread that atomically renames a valid and an invalid inode/root while the helper runs. A result may be the retained valid descriptor or a typed identity/manifest error; it must never accept bytes from the invalid replacement, follow a symlink, block, or resolve an artifact outside the retained root identity. Repeat a fixed bounded count and join the mutator under the test deadline.

Write `verify-cargo-elf.test.mjs` with synthetic Cargo JSON lines: accept exactly one `compiler-artifact` whose target name is `contract` and whose `executable` is present; reject zero, duplicates, stale filenames not present in the current JSON, non-musl target paths, and malformed JSON. This captures the artifact from the current build invocation instead of scanning the shared cache.

Refresh the exact Task 3 RED-test content before compilation:

```bash
rtk mkdir -p output/linux-platform-runtime-seam/task3-red-source
rtk native/linux-desktop/scripts/private-cargo.sh metadata --manifest-path native/linux-platform-runtime-contract/Cargo.toml --locked --offline --filter-platform x86_64-unknown-linux-musl --format-version 1 > output/linux-platform-runtime-seam/task3-red-source/cargo-metadata-musl.json
rtk native/linux-desktop/scripts/private-cargo.sh tree --manifest-path native/linux-platform-runtime-contract/Cargo.toml --locked --offline --target x86_64-unknown-linux-musl --edges normal,build > output/linux-platform-runtime-seam/task3-red-source/cargo-tree-musl.txt
rtk node native/linux-platform-runtime-contract/audit/first-party-content.mjs --scope task3-red --authorization native/linux-platform-runtime-contract/source-reuse-lock.json --output native/linux-platform-runtime-contract/audit/first-party-content.json --paths native/linux-platform-runtime-contract/tests/contract.rs,native/linux-platform-runtime-contract/audit/verify-cargo-elf.test.mjs
rtk node native/linux-platform-runtime-contract/audit/source-reuse-adapter.mjs --metadata output/linux-platform-runtime-seam/task3-red-source/cargo-metadata-musl.json --cargo-tree output/linux-platform-runtime-seam/task3-red-source/cargo-tree-musl.txt --cargo-lock native/linux-platform-runtime-contract/Cargo.lock --prior-lock native/linux-desktop/source-lock.json --reuse-lock native/linux-platform-runtime-contract/source-reuse-lock.json --reviewed-mismatches native/linux-platform-runtime-contract/audit/reviewed-mismatches.json --first-party-content native/linux-platform-runtime-contract/audit/first-party-content.json
```

Expected: PASS within the accepted Task 3 path/role/private-development scope. Any other change blocks before RED compilation.

- [ ] **Step 2: Run RED after confirming Task 2 closure is still accepted**

```bash
rtk native/linux-desktop/scripts/private-cargo.sh test --manifest-path native/linux-platform-runtime-contract/Cargo.toml --locked --no-run --test contract
rtk node --test native/linux-platform-runtime-contract/audit/verify-cargo-elf.test.mjs
```

Expected: Rust FAIL because `app_services` and bootstrap interfaces do not exist; Node FAIL because the Cargo-JSON/ELF verifier does not exist.

- [ ] **Step 3: Implement bounded explicit-root bootstrap**

Export `app_services` from `lib.rs`. In `state.rs`, require an absolute install root and open it with `O_RDONLY | O_DIRECTORY | O_NOFOLLOW | O_CLOEXEC`. Retain that `OwnedFd` for the entire bootstrap. Open only `runtime-manifest.json` relative to the root descriptor with `openat` and `O_RDONLY | O_NOFOLLOW | O_NONBLOCK | O_CLOEXEC`; never call path-based `File::open` after a metadata check. Immediately `fstat` the returned descriptor and reject nonregular files, group/world write bits, zero length, or length above `MAX_RUNTIME_MANIFEST_BYTES`. Read at most `MAX + 1` bytes from that same descriptor, then `fstat` again and require unchanged device, inode, mode, and size. This prevents FIFO blocking and path replacement from switching the bytes after validation.

Before and after `RuntimeResolver::from_install_root`, compare the retained root descriptor's `fstat` device/inode/type/mode to fresh nofollow path metadata and reject replacement or writable mutation. Parse the retained bytes with `RuntimeManifest::from_json(bytes, expected_target)`, compare `manifest.app_version` exactly to `expected_app_version`, and pass the same target to the existing resolver. Do not alter resolver semantics: its existing fresh hash/path checks remain intact, and the accepted trusted-owner TOCTOU limitation still applies after bootstrap.

- [ ] **Step 4: Implement immutable production ownership**

`AppServices::bootstrap_linux_permissive()` calls `compiled_platform_target()`, requires `LinuxPermissive`, and calls the lower-level helper only with `Path::new(LINUX_PRODUCTION_INSTALL_ROOT)`, `COMPILED_TARGET_TRIPLE`, and `env!("CARGO_PKG_VERSION")`. The lower helper stores the resolver in one `Arc`; expose only shared borrows. Do not add a setter, test override, Mac constructor, Tauri state, or application call site.

In `app_services/mod.rs` module documentation, include the exact future Packet 2C signatures from this plan and the caller-migration list: canonical `project_export` internal request, production workflows, Linux desktop composition gateway, both Settings snapshot builders, and render-health command adapters. State that existing zero-argument/public APIs remain compiled and behaviorally unchanged until Packet 2C performs the migration.

Implement `verify-cargo-elf.mjs` with Node built-ins. It parses only the captured JSON lines from one Cargo invocation, selects the exact `contract` executable, verifies the path is beneath the expected current target directory, invokes `/usr/bin/readelf -lW` and `-dW` on that exact path without a shell, and writes canonical JSON containing artifact path/SHA-256 and interpreter/dynamic findings. It fails on any ambiguity or forbidden ELF entry.

Before GREEN compilation, refresh and validate the full Task 3 content scope:

```bash
rtk mkdir -p output/linux-platform-runtime-seam/task3-green-source
rtk native/linux-desktop/scripts/private-cargo.sh metadata --manifest-path native/linux-platform-runtime-contract/Cargo.toml --locked --offline --filter-platform x86_64-unknown-linux-musl --format-version 1 > output/linux-platform-runtime-seam/task3-green-source/cargo-metadata-musl.json
rtk native/linux-desktop/scripts/private-cargo.sh tree --manifest-path native/linux-platform-runtime-contract/Cargo.toml --locked --offline --target x86_64-unknown-linux-musl --edges normal,build > output/linux-platform-runtime-seam/task3-green-source/cargo-tree-musl.txt
rtk node native/linux-platform-runtime-contract/audit/first-party-content.mjs --scope task3-green --authorization native/linux-platform-runtime-contract/source-reuse-lock.json --output native/linux-platform-runtime-contract/audit/first-party-content.json --paths src-tauri/src/lib.rs,src-tauri/src/app_services/mod.rs,src-tauri/src/app_services/state.rs,native/linux-platform-runtime-contract/tests/contract.rs,native/linux-platform-runtime-contract/audit/verify-cargo-elf.mjs,native/linux-platform-runtime-contract/audit/verify-cargo-elf.test.mjs
rtk node native/linux-platform-runtime-contract/audit/source-reuse-adapter.mjs --metadata output/linux-platform-runtime-seam/task3-green-source/cargo-metadata-musl.json --cargo-tree output/linux-platform-runtime-seam/task3-green-source/cargo-tree-musl.txt --cargo-lock native/linux-platform-runtime-contract/Cargo.lock --prior-lock native/linux-desktop/source-lock.json --reuse-lock native/linux-platform-runtime-contract/source-reuse-lock.json --reviewed-mismatches native/linux-platform-runtime-contract/audit/reviewed-mismatches.json --first-party-content native/linux-platform-runtime-contract/audit/first-party-content.json
```

Expected: PASS for refreshed content only; a new path/role/dependency/third-party/license input requires focused audit before compile.

- [ ] **Step 5: Run focused host and musl contracts**

```bash
rtk native/linux-desktop/scripts/private-cargo.sh test --manifest-path native/linux-platform-runtime-contract/Cargo.toml --locked --test contract
rtk native/linux-desktop/scripts/private-cargo.sh test --manifest-path native/linux-platform-runtime-contract/Cargo.toml --locked --target x86_64-unknown-linux-musl --test contract
rtk node --test native/linux-platform-runtime-contract/audit/lock-diff.test.mjs native/linux-platform-runtime-contract/audit/first-party-content.test.mjs native/linux-platform-runtime-contract/audit/source-reuse-adapter.test.mjs native/linux-platform-runtime-contract/audit/feature-policy.test.mjs native/linux-platform-runtime-contract/audit/verify-cargo-elf.test.mjs
```

Expected: every applicable named contract and policy regression PASS on both GNU host and musl. Cfg-separated compiled-target expectations prevent GNU assertions from running on musl or musl assertions from running on GNU. No test writes `/usr/lib`, and no successful staging helper call is called production bootstrap evidence.

- [ ] **Step 6: Run final isolation and binary checks in a fresh evidence child**

Use the private launcher for metadata, trees, test build, and release harness build. Copy only newly produced evidence into `output/linux-platform-runtime-seam/final-attempt1`; never alter earlier roots. Run the thin authoritative-policy adapter before compilation and again over final metadata.

```bash
rtk mkdir -p output/linux-platform-runtime-seam/final-attempt1
rtk native/linux-desktop/scripts/private-cargo.sh metadata --manifest-path native/linux-platform-runtime-contract/Cargo.toml --locked --offline --filter-platform x86_64-unknown-linux-musl --format-version 1 > output/linux-platform-runtime-seam/final-attempt1/cargo-metadata-musl.json
rtk native/linux-desktop/scripts/private-cargo.sh tree --manifest-path native/linux-platform-runtime-contract/Cargo.toml --locked --target x86_64-unknown-linux-musl --edges normal,build > output/linux-platform-runtime-seam/final-attempt1/cargo-tree-musl.txt
rtk node native/linux-platform-runtime-contract/audit/source-reuse-adapter.mjs --metadata output/linux-platform-runtime-seam/final-attempt1/cargo-metadata-musl.json --cargo-tree output/linux-platform-runtime-seam/final-attempt1/cargo-tree-musl.txt --cargo-lock native/linux-platform-runtime-contract/Cargo.lock --prior-lock native/linux-desktop/source-lock.json --reuse-lock native/linux-platform-runtime-contract/source-reuse-lock.json --reviewed-mismatches native/linux-platform-runtime-contract/audit/reviewed-mismatches.json --first-party-content native/linux-platform-runtime-contract/audit/first-party-content.json
rtk native/linux-desktop/scripts/private-cargo.sh build --manifest-path native/linux-platform-runtime-contract/Cargo.toml --locked --release --target x86_64-unknown-linux-musl --test contract --message-format=json > output/linux-platform-runtime-seam/final-attempt1/cargo-build.jsonl
rtk node native/linux-platform-runtime-contract/audit/verify-cargo-elf.mjs --cargo-json output/linux-platform-runtime-seam/final-attempt1/cargo-build.jsonl --expected-target-root native/linux-desktop/target/x86_64-unknown-linux-musl --output output/linux-platform-runtime-seam/final-attempt1/elf-report.json
```

Expected: normal/build trees contain only independently accepted components; `tauri-build`, Tauri/GTK, GStreamer, temporal, wgpu, cosmic-text/fontdb, and macOS frameworks are absent. The exact artifact named by current Cargo JSON has no ELF interpreter or `DT_NEEDED`, `RPATH`, or `RUNPATH`. Stale cache artifacts cannot affect selection. This qualifies only the 4A contract harness, not a packaged application or future render graph.

- [ ] **Step 7: Commit only AppServices-owned paths**

```bash
rtk git add src-tauri/src/lib.rs src-tauri/src/app_services/mod.rs src-tauri/src/app_services/state.rs native/linux-platform-runtime-contract/tests/contract.rs native/linux-platform-runtime-contract/audit/verify-cargo-elf.mjs native/linux-platform-runtime-contract/audit/verify-cargo-elf.test.mjs native/linux-platform-runtime-contract/audit/first-party-content.json
rtk git commit --only -m "feat(linux): own fixed-root runtime services" -- src-tauri/src/lib.rs src-tauri/src/app_services/mod.rs src-tauri/src/app_services/state.rs native/linux-platform-runtime-contract/tests/contract.rs native/linux-platform-runtime-contract/audit/verify-cargo-elf.mjs native/linux-platform-runtime-contract/audit/verify-cargo-elf.test.mjs native/linux-platform-runtime-contract/audit/first-party-content.json
```

Expected: one owned-path commit. Root dispatches independent Task 3 review, then a separate whole-packet review over Task 1 base through Task 3 head. Sol alone makes any implementation/fix commits; reviewers do not mutate source.

## Final review and handoff gates

Before Packet 4A acceptance, root verifies:

- every task review passed and every fix received a fresh review;
- tracked status contains no unrelated change and all commits used exact owned paths;
- source reuse is bound to final metadata/tree/lock hashes with zero mismatches and no compilation preceded the Task 1 source gate;
- Mac defaults and `custom-protocol` text are unchanged, `tauri-build` remains active under the Mac default through `app-runtime`, and no Ubuntu result is called Mac runtime verification;
- GNU host, musl compile, musl execution, staging helper, static harness, and production fixed-root claims are distinct;
- no `main.rs`, package script, export, Settings, workflow, desktop, worker, font, runtime resource, or legacy wrapper changed;
- the only Packet 2C coupling is the documented exact `&AppServices` interface; there is no unused route stub or new behavior;
- all prior evidence roots and concurrent W1/Desktop work remain untouched.

After acceptance, F1 rebases on the 4A resolver ownership and may seek approval to relax only `graphics-render`. Packet 2C then implements the documented service-aware export/Settings entries, migrates actual Linux consumers, and proves target-safe routing. The full Packet 4 application-service extraction and Mac state adoption remain later work.
