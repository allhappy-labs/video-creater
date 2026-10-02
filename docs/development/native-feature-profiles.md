# Native feature profiles and a possible domain crate

Status on 2026-10-01: keep the current runtime features and defer crate extraction.
The measured warm root-library rebuild warrants a small domain-only experiment,
but no extracted crate or improved incremental cycle has been verified yet.

## Current supported boundaries

| Purpose | Current invocation or package | Practical boundary |
| --- | --- | --- |
| Desktop development | Default features | `app-runtime`, `coreml-inspect`, `ges-render`, `gpu-render`, `graphics-render`, `temporal-worker` |
| Existing host development | Defaults plus `web-host` | Includes the desktop/media/GPU/Temporal defaults; this is not a lightweight host profile |
| Focused project/service/host tests | `--no-default-features --features web-host --lib` | Disables the root optional renderer/worker features; still resolves desktop dependencies |
| MCP server | Root `mcp-server` feature | Selects the binary; enabling it alone does not define an independent pure domain crate |
| Native workers | Existing workspace worker/protocol packages | Separate process/protocol packages; retain their target, runtime and packaging checks |
| Domain-only tests | Proposed small crate | Not implemented or verified |

The manifest intentionally keeps its existing release defaults. Do not infer a
supported deployment profile merely because a feature combination compiles.

Fresh dependency inspection:

```sh
rtk proxy cargo tree --manifest-path src-tauri/Cargo.toml --no-default-features --features web-host -e normal -i tauri
rtk proxy cargo tree --manifest-path src-tauri/Cargo.toml --no-default-features --features web-host -e features -i tauri
```

The normal graph reaches `tauri` through unconditional `tauri-plugin-dialog` and
its `tauri-plugin-fs` dependency. The test graph additionally reaches Tauri's
`test`, default `wry`, WebKitGTK and X11 features through the root dev dependency.
Therefore `--no-default-features` currently does not remove desktop libraries.
Making the dialog plugin optional belongs to a deliberate desktop/host adapter
profile change, with checks for every binary that uses it.

## Measured warm root rebuild

On this Ubuntu x86_64 VM, Cargo/rustc 1.98.1, with the existing dependency cache
and a dirty workspace based on `167ea2df36930299b2a507f34034a8ff579e3080`:

```sh
rtk proxy cargo test --manifest-path src-tauri/Cargo.toml --no-default-features --features web-host --lib --timings -- --test-threads=4
```

The 2026-10-01 11:43:22 UTC Cargo timing report recorded 602 fresh units and one
root test-library compilation taking 30.29 seconds, within 30.58 seconds total
Cargo build time. No native dependency recompilation dominated that particular
warm run. The tests then took 60.54 seconds: 638 passed and eight were explicitly
ignored. This verifies the narrow library lane on Linux, not a cold build,
packaging, macOS or a speedup caused by a crate split.

Local raw timing artifact:
`src-tauri/target/cargo-timings/cargo-timing-20261001T114322074Z-b32f36994aa090cb.html`.
It is an ignored build artifact, not a portable committed benchmark. The root
manifest's test profile has incremental compilation disabled. Source changes
therefore still rebuild the large root test library in this lane.

An earlier broad run with unrestricted test concurrency passed 636 tests and
failed one admission test's two-second worker-finalization allowance under heavy
filesystem contention. The allowance was made consistent with other bounded
worker tests and the four-thread run above passed. That failed run is not passing
performance evidence. Tests that bind local HTTP sockets required the authorized
execution boundary; the freshly prepared reviewed Linux runtime also supplied
the previously missing `uridecodebin` factory.

## Smallest useful extraction experiment

Start with a module boundary, then a pilot crate containing serializable project
models, deterministic project action validation/application, and owned wire
contracts. Keep split-project persistence, mutation/artifact leases, Tauri
commands, render execution, provider credentials and Temporal workers outside it.
Preserve the current public imports through re-exports during the experiment.

The current code identifies real dependencies to resolve before that move:

- `project/model.rs` imports `RenderQuality` from `edit/render_plan.rs` and stores
  `JobExportSettings` from `project/export_options.rs`.
- Export settings depend on encode-tier, export-profile and destination DTOs.
- `project/action.rs` depends on audio edits, reverse/source-window and transition
  logic, effect descriptors, and CPU keyframe/curve types from `frame_compositor`.
- Render admission wire envelopes include `VideoProject` and render-report DTOs;
  Rust-owned contract checks must remain shared by desktop and host adapters.

Move only the required data and deterministic operations; do not copy renderer
implementations into a nominally pure crate or add behavior-free dependency traits.
The recently removed service dependency objects exposed only `name()` and had no
callers. Actual project and render services retain their behavioral boundaries.

The pilot is accepted only if exact model/action/contract tests build and run
without Tauri, GTK/WebKit, GStreamer, GPU or Temporal dependencies, and repeated
warm edits to a representative model/action source improve the measured cycle
on the same machine/cache/profile without concurrent Cargo work. Record the
source revision, dirty state, feature set, compilation and test spans separately.
A deliberately isolated cold comparison may supplement those runs; never clear
the user's development cache to manufacture one.

After acceptance, explicitly check desktop, host, MCP and worker composition plus
existing release/license/packaging gates. Until then, extraction and any build
speedup claim remain deferred. See [iteration guidance](iteration-and-performance.md)
and [the verification map](ci-gate-map.md).
