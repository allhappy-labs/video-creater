# pnpm Build Policy and Temporal Defaults Design

## Problem

`rtk pnpm check:temporal-worker` fails before running the package script because pnpm blocks dependency lifecycle builds without an explicit repository policy. The blocked package is `esbuild`, which is used by the Vite/Vitest toolchain. Separately, the Rust Temporal worker is only enabled when callers pass `--features temporal-worker`, but Temporal workflows should be part of the default Rust build path.

## Goals

- Allow the reviewed `esbuild` lifecycle build at the pnpm workspace policy layer.
- Make `temporal-worker` part of the Rust default feature set.
- Update the npm script so `check:temporal-worker` proves the default feature set includes the worker.
- Keep an explicit `temporal-worker` feature for consumers that need to reason about or disable default features.

## Non-Goals

- Do not replace pnpm, Vite, Vitest, or esbuild.
- Do not remove all Temporal feature gates from the codebase in this change.
- Do not change render workflow behavior beyond the default feature selection.

## Design

Add an explicit `allowBuilds` policy to `pnpm-workspace.yaml`:

```yaml
allowBuilds:
  esbuild: true
```

This lets pnpm complete its pre-script dependency policy check without relying on generated placeholders or local machine state.

Update `src-tauri/Cargo.toml` so default features include both `ges-render` and `temporal-worker`. Leave the `temporal-worker` feature definition in place so `--no-default-features` remains available for deliberately smaller builds.

Update `package.json` so `check:temporal-worker` no longer passes `--features temporal-worker`. If that script succeeds, it proves the worker binary compiles through default Rust features.

## Verification

Run:

```bash
rtk pnpm check:temporal-worker
rtk cargo check --manifest-path src-tauri/Cargo.toml --bin video-creater-temporal-worker
rtk cargo test --manifest-path src-tauri/Cargo.toml --test temporal_workflows -- --test-threads=1
```

The pnpm command specifically verifies the previous failure path. The direct Cargo command verifies the same build outside pnpm. The Temporal workflow test verifies the now-default worker feature still compiles and exercises its workflow contracts.
