# Linux Runtime Manifest and Resolver Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use `superpowers:subagent-driven-development` (recommended) or `superpowers:executing-plans` to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add the fail-closed Rust contract that validates a Linux runtime manifest and resolves exact app-owned artifacts only from a trusted immutable installation root.

**Architecture:** `platform_runtime` owns typed manifest parsing, exact license selection policy, graph and independent-service validation, and fresh artifact verification at every resolution. One isolated static-musl harness path-depends on the real library with default features disabled, so tests cannot pull the root package's unconditional Tauri dev-dependency or GTK.

**Tech Stack:** Rust 2021, serde/serde_json, sha2, Unix metadata permissions, `x86_64-unknown-linux-musl`, JSON Schema draft 2020-12.

**Spec:** [`docs/superpowers/specs/2026-09-12-linux-compatibility-design.md`](../specs/2026-09-12-linux-compatibility-design.md)

**Roadmap:** [`docs/superpowers/plans/2026-09-12-linux-full-compatibility.md`](./2026-09-12-linux-full-compatibility.md)

## Global Constraints

- No LGPL in the app, helpers, or any libraries they link or load.
- Resolution never searches `PATH`, environment overrides, package-manager roots, developer checkouts, writable model stores, or global plugin paths.
- Artifact paths are relative to one canonical install root and contain only normal path components.
- Every artifact records target, bytes, SHA-256, source identity, declared and selected license, dependency IDs, notices, and protocol identity where present.
- Independent OS services are separate declarations without artifact path, source license, linked dependency, or distribution-classification fields.
- Accepted selected terms are exactly `MIT`, `Apache-2.0`, `BSD-2-Clause`, `BSD-3-Clause`, `ISC`, `Zlib`, and `Apache-2.0 WITH LLVM-exception`. Selection must derive from the declared SPDX expression under the repository's bounded preflight semantics.
- This task does not discover a payload, build or launch a worker, route media, change readiness/default features/Tauri configuration, or claim any artifact qualified.
- Preserve existing macOS behavior and defaults.

## Root rulings for Task 1

- Every `resolve` and `resolve_protocol` call revalidates the canonical install
  root itself before walking child components: it must still exist as a real,
  nonsymlink directory with no group/world write bits. Tests change the root
  mode and replace the root with a symlink after resolver construction. This
  prevents a construction-time trust check from becoming stale, at the cost of
  one additional metadata read per resolution. The documented trusted-owner
  TOCTOU limitation remains unchanged.
- `Executable` and `NativeLibrary` artifacts must declare the exact expected
  target. The value `all` is reserved for architecture-independent payload kinds
  (`Model`, `Tokenizer`, `Font`, `Template`, `Skill`, `Asset`, and `Notice`). This
  prevents a wrong-architecture native binary from passing as target-neutral. If
  this ruling is too strict, it rejects a legitimate native alias until a later
  manifest-format revision represents that alias without weakening target checks.
- Artifact `relativePath` values must be unique across the manifest. Notice paths
  and runtime identities otherwise become ambiguous even when artifact IDs are
  unique. If this ruling is too strict, it rejects legitimate same-byte aliases
  until a later format revision gives aliases one canonical artifact identity.

## Exact public contract

Create these types in `platform_runtime::manifest`:

```rust
pub const RUNTIME_MANIFEST_SCHEMA_VERSION: u32 = 1;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RuntimeManifest {
    pub schema_version: u32,
    pub app_version: String,
    pub target: String,
    pub artifacts: Vec<RuntimeArtifact>,
    pub independent_services: Vec<IndependentService>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RuntimeArtifact {
    pub id: String,
    pub kind: RuntimeArtifactKind,
    pub target: String,
    pub relative_path: String,
    pub bytes: u64,
    pub sha256: String,
    pub source_url: String,
    pub source_revision: String,
    pub license: String,
    pub selected_license: String,
    pub notice_paths: Vec<String>,
    pub build_flags: Vec<String>,
    pub dependencies: Vec<String>,
    pub protocol: Option<RuntimeProtocol>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum RuntimeArtifactKind { Executable, NativeLibrary, Model, Tokenizer, Font, Template, Skill, Asset, Notice }

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RuntimeProtocol { pub name: String, pub version: u32 }
```

Create these types in `platform_runtime::services`:

```rust
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct IndependentService {
    pub id: String,
    pub protocol: IndependentServiceProtocol,
    pub purpose: String,
    pub endpoint_environment: Vec<ServiceEndpointEnvironment>,
    pub availability_probe: ServiceAvailabilityProbe,
    pub failure_behavior: ServiceFailureBehavior,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum IndependentServiceProtocol { Wayland, X11, PulseAudioNative, DbusPortal, DbusNotifications, DbusSecretService }

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Ord, PartialOrd)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ServiceEndpointEnvironment { WaylandDisplay, Display, XdgRuntimeDir, PulseServer, DbusSessionBusAddress }

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ServiceAvailabilityProbe { pub operation: String, pub timeout_ms: u32 }

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum ServiceFailureBehavior { TryAlternateDisplay, DegradeFeature, OfflineEditingContinues }
```

`endpoint_environment` is the manifest's policy allowlist of variables that a later service client may preserve. It is not a claim that the process inherits all listed variables. A declared value must belong to this per-protocol set:

```rust
let allowed = match protocol {
    IndependentServiceProtocol::Wayland => &[WaylandDisplay, XdgRuntimeDir],
    IndependentServiceProtocol::X11 => &[Display],
    IndependentServiceProtocol::PulseAudioNative => &[PulseServer, XdgRuntimeDir],
    IndependentServiceProtocol::DbusPortal
    | IndependentServiceProtocol::DbusNotifications
    | IndependentServiceProtocol::DbusSecretService => &[DbusSessionBusAddress],
};
```

Create this API in `platform_runtime::resolver`:

```rust
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedRuntimeArtifact {
    pub id: String,
    pub path: PathBuf,
    pub bytes: u64,
    pub sha256: String,
    pub protocol: Option<RuntimeProtocol>,
}

pub struct RuntimeResolver {
    install_root: PathBuf,
    expected_target: String,
    artifacts: BTreeMap<String, RuntimeArtifact>,
}

impl RuntimeManifest {
    pub fn from_json(bytes: &[u8], expected_target: &str) -> Result<Self, RuntimeManifestError>;
    pub fn validate(&self, expected_target: &str) -> Result<(), RuntimeManifestError>;
}

impl RuntimeResolver {
    pub fn from_install_root(install_root: &Path, manifest: RuntimeManifest, expected_target: &str) -> Result<Self, RuntimeResolverError>;
    pub fn resolve(&self, artifact_id: &str) -> Result<ResolvedRuntimeArtifact, RuntimeResolverError>;
    pub fn resolve_protocol(&self, artifact_id: &str, protocol_name: &str, protocol_version: u32) -> Result<ResolvedRuntimeArtifact, RuntimeResolverError>;
}
```

Errors are typed enums with these stable codes:

```text
runtime.manifest.jsonInvalid
runtime.manifest.schemaUnsupported
runtime.manifest.targetMismatch
runtime.manifest.invalidField
runtime.manifest.duplicateId
runtime.manifest.unknownDependency
runtime.manifest.invalidPath
runtime.manifest.licenseDenied
runtime.manifest.serviceInvalid
runtime.resolver.installRootInvalid
runtime.resolver.installRootWritable
runtime.resolver.artifactUnknown
runtime.resolver.artifactMissing
runtime.resolver.artifactTypeInvalid
runtime.resolver.artifactSymlinkDenied
runtime.resolver.artifactOutsideRoot
runtime.resolver.artifactPathWritable
runtime.resolver.artifactNotExecutable
runtime.resolver.artifactSizeMismatch
runtime.resolver.artifactHashMismatch
runtime.resolver.protocolMismatch
```

## Task 1: Implement and verify the complete resolver contract

**Status (2026-09-12): Complete and independently reviewed at `52a800d`.** The
review reported zero findings. Fresh evidence passed 14 host contract tests, 14
musl contract tests, 5 core tests, and 31 preflight tests; the release harness
also passed static ELF inspection.

**Files:**

- Create `src-tauri/src/platform_runtime/license.rs`
- Create `src-tauri/src/platform_runtime/services.rs`
- Create `src-tauri/src/platform_runtime/manifest.rs`
- Create `src-tauri/src/platform_runtime/resolver.rs`
- Create `src-tauri/src/platform_runtime/mod.rs`
- Modify `src-tauri/src/lib.rs`
- Create `src-tauri/resources/linux-runtime/runtime-manifest.schema.json`
- Create `native/linux-runtime-contract/Cargo.toml`
- Create `native/linux-runtime-contract/Cargo.lock`
- Create `native/linux-runtime-contract/src/main.rs`
- Create `native/linux-runtime-contract/tests/contract.rs`

**Produces:** The public API above for Packet 2's media client and every later app-owned worker, plus a standalone validator that resolves one protocol artifact without launching it.

- [x] **Step 1: Scaffold the isolated RED harness first**

Create `native/linux-runtime-contract/Cargo.toml`:

```toml
[package]
name = "video-creater-linux-runtime-contract"
version = "0.1.0"
edition = "2021"
license = "MIT OR Apache-2.0"

[workspace]

[dependencies]
video-creater = { path = "../../src-tauri", default-features = false }
serde_json = "1.0.134"
```

Generate its own lockfile. Do not use `src-tauri` tests: its unconditional Tauri dev-dependency can pull GTK even with `--no-default-features`. In `tests/contract.rs`, import `video_creater_lib::platform_runtime` and add one smoke case.

Run:

```bash
rtk cargo test --manifest-path native/linux-runtime-contract/Cargo.toml --locked
```

Expected: RED at compile time because `platform_runtime` does not exist. Record the nonzero result and missing-module error.

- [x] **Step 2: Add failing license-policy tests**

Port the bounded semantics from `scripts/linux-permissive-preflight.mjs`; do not invoke Node at runtime. Limit expressions to 512 characters, 64 tokens, and 64 enumerated choices. Recognize the current preflight IDs and only `LLVM-exception`. Require:

```rust
assert!(validate_license_selection("MIT OR Apache-2.0", "MIT").is_ok());
assert!(validate_license_selection("Apache-2.0 WITH LLVM-exception OR MIT", "Apache-2.0 WITH LLVM-exception").is_ok());
assert!(validate_license_selection("LGPL-2.1-or-later OR MIT", "MIT").is_ok());
assert_eq!(validate_license_selection("LGPL-2.1-or-later", "MIT").unwrap_err().stable_code(), "runtime.manifest.licenseDenied");
assert_eq!(validate_license_selection("MIT AND BSD-4-Clause", "MIT").unwrap_err().stable_code(), "runtime.manifest.licenseDenied");
```

Also reject unknown IDs/exceptions, malformed expressions, an underived selection, dropping an `AND` obligation, retaining a denied leaf, repeated `WITH`, and size/count-limit breaches.

- [x] **Step 3: Implement the bounded license parser**

Expose within the crate:

```rust
pub(crate) fn validate_license_selection(declared: &str, selected: &str) -> Result<(), RuntimeManifestError>;
```

An `OR` selection chooses one complete branch; `AND` retains every leaf; only the seven eligible exact terms survive selection. Recognition is not approval.

- [x] **Step 4: Add service-declaration tests and implementation**

Cover every allowed protocol/environment pair, empty declarations, duplicate environment values, foreign variables, timeout 0/30,001, and incompatible failure behavior. Require timeout `1..=30_000` and nonempty `id`, `purpose`, and `operation`.

Display uses `TryAlternateDisplay`; Secret Service uses `OfflineEditingContinues`; PulseAudio/portal/notifications use `DegradeFeature`. A nonempty subset of the per-protocol environment allowlist is valid.

- [x] **Step 5: Add manifest tests using exact fixture bytes**

The fixture target is `x86_64-unknown-linux-musl`. Worker and notice files contain bytes `[1, 2, 3, 4]`; their SHA-256 is:

```text
9f64a747e1b97f131fabb6b447296c9b6f0201e79fb3c5356e6c77e89b6a806a
```

The worker artifact is:

```json
{
  "id":"permissive-media-worker",
  "kind":"executable",
  "target":"x86_64-unknown-linux-musl",
  "relativePath":"bin/video-creater-permissive-media-worker",
  "bytes":4,
  "sha256":"9f64a747e1b97f131fabb6b447296c9b6f0201e79fb3c5356e6c77e89b6a806a",
  "sourceUrl":"https://example.invalid/video-creater",
  "sourceRevision":"test-revision",
  "license":"MIT OR Apache-2.0",
  "selectedLicense":"MIT",
  "noticePaths":["notices/video-creater-MIT.txt"],
  "buildFlags":["--target=x86_64-unknown-linux-musl"],
  "dependencies":["video-creater-notice"],
  "protocol":{"name":"video-creater.permissive-media","version":1}
}
```

The notice artifact has ID `video-creater-notice`, kind `notice`, target `all`, relative path `notices/video-creater-MIT.txt`, the same byte metadata, source identity, MIT declared/selected, and empty notice/build/dependency lists with no protocol. Wrap both in schema version 1, app version 0.1.0, expected target, and an empty service array.

Mutate one property per case and assert stable code: invalid JSON, unknown field, schema 0/2, wrong target, `all` on an executable or native library, empty required string, non-HTTPS source, malformed/uppercase hash, zero bytes, absolute/traversal/empty path component, duplicate artifact/service ID, duplicate artifact relative path, unknown/duplicate dependency, missing notice path target, notice resolving to non-notice, denied/underived license, protocol on non-executable, zero protocol version, and invalid service.

Executable protocol is optional because the desktop host has no worker protocol. When present, protocol is allowed only for executables and both fields validate.

- [x] **Step 6: Implement manifest parsing and validation**

Apply `serde(deny_unknown_fields)` to every object and enforce:

```text
schemaVersion == 1
manifest target == expected target
executable/native-library target == expected target
architecture-independent payload target == expected target or "all"
required strings nonempty after trim
sourceUrl begins with https:// and has a nonempty host token
sha256 exactly 64 lowercase hexadecimal characters
bytes > 0
relativePath and noticePaths contain only Component::Normal values
IDs unique across artifacts and services; artifact relativePath values unique
dependencies resolve and have no duplicate edge; graph cycles allowed
noticePaths resolve by relativePath to kind notice
protocol only on executable; name nonempty and version > 0
license declaration/selection passes Step 3
```

- [x] **Step 7: Add resolver tests including trust-boundary failures**

Create a private fixture: install root/bin/notices mode `0o755`, worker `0o755`, notice `0o644`, exact bytes. Resolve:

```rust
let manifest = RuntimeManifest::from_json(json, "x86_64-unknown-linux-musl")?;
let resolver = RuntimeResolver::from_install_root(install_root.path(), manifest, "x86_64-unknown-linux-musl")?;
let worker = resolver.resolve_protocol("permissive-media-worker", "video-creater.permissive-media", 1)?;
assert_eq!(worker.bytes, 4);
assert_eq!(worker.sha256, EXPECTED_SHA256);
```

Add one case per resolver code. Put a decoy executable on temporary `PATH` and prove it is never selected. Mutate the real artifact after resolver creation and catch size/hash mismatch. Mutate its notice/dependency and require resolving the worker to fail. Cover symlinked root/intermediate/artifact; directory replacing file; missing execute bits; group/world-writable root/intermediate/artifact; missing artifact; protocol mismatch.

- [x] **Step 8: Implement construction and fresh resolution**

`from_install_root` uses `symlink_metadata`, rejects missing/non-directory/symlink root and group/world write bits (`mode & 0o022 != 0`), canonicalizes once, validates the manifest, and builds an ID map without environment state.

Every `resolve` first rechecks the canonical install root with
`symlink_metadata`: it must still exist as a real nonsymlink directory without
group/world write bits. It then walks every relative component, rejects
symlinks and group/world-writable directories/files, requires a regular final
file, canonicalizes it, and requires `starts_with(install_root)`. Every call
re-reads length and streams SHA-256. It recursively verifies the artifact's
dependency and notice closure with cycle-safe ID tracking before returning.
Executables require an execute bit. `resolve_protocol` then requires exact
name/version.

Document the trust assumption: package installation supplies an owner-controlled root not writable by untrusted users. These checks and fresh hashes detect ordinary corruption/path substitution; they are not code signing, execution attestation, or complete protection from a privileged/same-owner TOCTOU race.

- [x] **Step 9: Add and agree the JSON Schema**

Create a draft 2020-12 schema with `$defs`, explicit required arrays, `additionalProperties:false` on every object, exact enums, schema version 1, positive minima, and hash pattern `^[0-9a-f]{64}$`. Artifact/service definitions stay distinct. Harness tests read it and assert constants, fields, enums, and restrictions agree with Rust; add no schema-validator dependency.

- [x] **Step 10: Implement the harness CLI and integration checks**

CLI:

```text
video-creater-linux-runtime-contract <install-root> <manifest-json> <expected-target> <artifact-id> <protocol-name> <protocol-version>
```

It validates/resolves without spawning, emits one typed JSON success line, and emits stable code/detail plus nonzero status on failure. Integration tests compare success JSON, mutate a file and assert rejection, then inspect `cargo tree --edges normal --no-dev` to require the `video-creater` dependency feature set empty and absence of Tauri, GTK/WebKitGTK, GStreamer, Core ML, Temporal, and wgpu packages.

- [x] **Step 11: Run the focused gate**

```bash
rtk cargo test --manifest-path native/linux-runtime-contract/Cargo.toml --locked --target x86_64-unknown-linux-musl
rtk cargo build --manifest-path native/linux-runtime-contract/Cargo.toml --release --locked --target x86_64-unknown-linux-musl
rtk cargo fmt --manifest-path src-tauri/Cargo.toml --all -- --check
rtk cargo test --manifest-path native/linux-core-probe/Cargo.toml --locked
rtk node --test scripts/linux-permissive-preflight.test.ts
rtk readelf -lW native/linux-runtime-contract/target/x86_64-unknown-linux-musl/release/video-creater-linux-runtime-contract
rtk readelf -dW native/linux-runtime-contract/target/x86_64-unknown-linux-musl/release/video-creater-linux-runtime-contract
```

Expected: all tests/build/format checks pass; ELF has no `INTERP`, `DT_NEEDED`, `RPATH`, or `RUNPATH`. If the linker environment cannot create static musl, retain the exact blocked output; do not call it passed.

- [x] **Step 12: Review scope and commit**

Confirm no changes to the default feature list, shared/macOS commands, Tauri config, render routing/readiness, or accepted proof/spec/report files. Commit:

```bash
rtk git add src-tauri/src/platform_runtime src-tauri/src/lib.rs src-tauri/resources/linux-runtime/runtime-manifest.schema.json native/linux-runtime-contract
rtk git commit -m "feat(linux): add trusted runtime resolver"
```

## Completion boundary

Passing proves typed manifest/license/service validation and fresh absolute artifact resolution through the real platform-neutral library under musl. It does not prove declarations true, qualify a payload, verify static contributor closure, launch a worker, validate a package, or establish Linux release readiness. Packet 2 begins after this API passes task and whole-diff review; manifest generation and payload audit remain later review boundaries.
