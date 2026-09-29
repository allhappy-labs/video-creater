# Runtime Boundary and Build Cache Optimization Design

## Summary

Create a small runtime-boundary refactor and a bounded Cargo verification
lifecycle.

The frontend currently assumes that Tauri globals exist. Opening the Vite URL
directly fails while loading application preferences because the Tauri invoke
bridge is absent. Browser visual QA works only because its Playwright harness
injects a partial bridge fixture before the page loads.

The native verification workflow also shares one unbounded Cargo target
directory across development, Clippy feature combinations, binaries, and native
test targets. The measured target directory reached 54 GiB:

- 26 GiB in `debug/deps`;
- 20 GiB in `debug/incremental`;
- 3 GiB in the architecture-specific debug target;
- the remainder in bundles, build scripts, release artifacts, and supporting
  output.

The approved design separates runtime mode from backend connectivity, keeps
Tauri as the complete desktop runtime, preserves fixture-backed browser QA, and
leaves a deliberate boundary for a future cross-platform browser client. It
also separates development and verification cache lifecycles so exhaustive
verification cannot silently consume the development disk budget.

## Implementation Status

The first refactoring stage is implemented:

- runtime selection and the install-once backend boundary live in
  `src/lib/runtime/`;
- desktop and deterministic fixture transports live in
  `src/lib/runtime/adapters/`;
- a regular browser renders a disconnected desktop-host connection shell;
- production Tauri core and event imports are confined to runtime adapters;
- desktop development is the default command and raw Vite is an internal entry
  point;
- frontend and native verification are separate;
- native verification uses a validated bounded target with atomic metrics.

The operational commands and evidence boundaries are documented in
[`docs/development/runtime-and-verification.md`](../../development/runtime-and-verification.md).
The remote browser work listed under Planned Follow-Up Designs remains planned;
this stage does not implement a remote transport or pairing.

## Naming Constraint

The current application name is provisional.

New interfaces, user-facing connection copy, protocol concepts, artifacts, and
documentation introduced by this work must use product-neutral terms:

- `desktop host`;
- `browser client`;
- `editor runtime`;
- `backend transport`;
- `project service`.

Existing repository, package, bundle, binary, and command names remain
unchanged unless a rename is independently requested. This refactor must not
spread the provisional product name into new contracts that would later require
migration.

## Goals

- Make runtime selection explicit before the React application renders.
- Remove direct Tauri core, event, and media dependencies from React
  components and typed domain modules.
- Preserve typed project, settings, provider, rendering, and transcription
  APIs as the frontend's public backend surface.
- Replace the raw-browser missing-global failure with a deliberate disconnected
  browser state.
- Preserve deterministic fixture-backed browser visual QA.
- Keep the runtime boundary compatible with a later authenticated remote
  transport hosted by a desktop application on macOS, Windows, or Linux.
- Make the complete desktop runtime the default development entry point.
- Keep frontend verification free of Rust compilation.
- Preserve exact packaged custom-protocol and MCP sidecar coverage in final
  release readiness.
- Bound comprehensive Cargo verification storage while retaining useful
  incremental development artifacts.
- Record build duration, cache growth, and free-space evidence before claiming
  an optimization.

## Non-Goals

- Implement an HTTP, WebSocket, or other remote transport.
- Serve the frontend from the desktop host.
- Implement local-network discovery, pairing, authentication, or TLS.
- Implement project synchronization, version conflicts, or reconnect behavior.
- Implement remote media delivery or responsive mobile editing.
- Make fixture mode a standalone product runtime.
- Remove the custom-protocol or MCP sidecar release profiles.
- Automatically delete the development Cargo cache.
- Rename the application, repository, package, binaries, or bundle identifier.
- Generalize all platform-specific native capabilities in this pass.

## Current Evidence

### Browser runtime

A headed browser opened against `http://127.0.0.1:1420/` rendered only:

> Settings could not be loaded
>
> Cannot read properties of undefined (reading `invoke`)

The first failing boundary is `get_app_preferences`. The visual-QA harness
renders successfully because `scripts/browser-visual-qa.mjs` installs a
synthetic Tauri invoke and event bridge before navigation.

The browser harness is therefore a useful test runtime but the raw Vite page is
not a supported application runtime today.

### Frontend coupling

Seventeen production files import `@tauri-apps/api/core` directly. Most calls
are already grouped inside domain modules, but `App.tsx` and
`editor-workspace.tsx` still issue native commands directly. Event handling and
local media URL conversion introduce additional Tauri-specific boundaries.

### Cargo accumulation

The debug target contained approximately:

- 236,003 files;
- 191,065 `*.rcgu.o` files in `debug/deps`;
- 316 incremental compilation directories;
- 298 executable or test artifacts;
- 314 project fingerprint records.

Of those fingerprints, 292 were created since July 25, and 266 represented the
large default native feature set. This demonstrates recent rapid regeneration,
not merely old cache residue.

The canonical Clippy command evaluates:

1. the default workspace across all targets;
2. the MCP sidecar with only `mcp-server`;
3. the packaged desktop binary with `custom-protocol` and production rendering
   features.

The configurations protect real release boundaries, but they need not run
during unrelated frontend iteration.

## Runtime Architecture

### Runtime mode

Runtime mode answers where and why the frontend is running:

```ts
export type RuntimeMode = "desktop" | "browser" | "fixture";
```

- `desktop`: the frontend is running inside a Tauri WebView.
- `browser`: the frontend is running in a regular browser without a connected
  backend.
- `fixture`: the explicit deterministic visual-QA runtime.

Runtime mode is selected once during bootstrap and cannot silently change after
React renders.

### Backend connection

Backend connection is separate from runtime mode:

```ts
export type BackendConnection =
  | { status: "disconnected" }
  | { status: "connected"; transport: BackendTransport };
```

A browser starts disconnected. It does not receive a placeholder or fake
remote transport. A future pairing flow will create a connected
`RemoteTransport` only after connection and authentication succeed.

Desktop and fixture bootstrap install connected transports immediately.

### Backend transport

The low-level boundary owns requests, events, and media URL resolution:

```ts
export interface BackendTransport {
  readonly kind: "tauri" | "fixture" | "remote";

  request<Result>(
    operation: string,
    input?: Record<string, unknown>,
  ): Promise<Result>;

  listen<Payload>(
    event: string,
    handler: (payload: Payload) => void,
  ): Promise<() => void>;

  mediaUrl(path: string): string;
}
```

Only transport adapters may import Tauri core, event, or media APIs. A future
remote adapter may use HTTP and WebSocket internally, but those mechanisms do
not appear in React components or domain modules.

### Domain gateway

Existing domain modules remain the public frontend API:

- project operations;
- application and project settings;
- providers and credentials;
- model and transcription operations;
- rendering and generated assets;
- system health and storage;
- native menu behavior.

Domain methods remain typed and call the configured backend transport.
Components must not receive a generic `request(operation)` escape hatch.

The direct native commands in `App.tsx` and `editor-workspace.tsx` move into
the appropriate typed domain modules. The settings acceptance runner keeps its
explicit injected bridge because it is an acceptance orchestrator, but its
default production bridge delegates to the configured transport.

### Bootstrap

Bootstrap performs these steps in order:

1. Detect the explicit fixture marker. If present, select `fixture` mode and
   install `FixtureTransport`.
2. Otherwise, detect the Tauri runtime. If present, select `desktop` mode and
   install `TauriTransport`.
3. Otherwise, select `browser` mode with a disconnected backend.
4. Render React with the immutable runtime descriptor.

Fixture mode must never be inferred from the absence of Tauri globals.
Production desktop builds must not contain fixture data or activate fixture
mode without its explicit development-only marker.

## Browser Experience

The raw browser page becomes a deliberate connection shell rather than a
crashed application.

The immediate disconnected state:

- uses product-neutral copy such as `Connect to a desktop host`;
- explains that editing requires a running desktop host;
- does not promise that pairing is already available;
- contains no hard-coded host operating system;
- does not issue project, preference, event, filesystem, or provider requests;
- provides no fixture-data fallback.

The existing visual-QA runtime bypasses the connection shell only through its
explicit marker and deterministic fixture transport.

The desktop runtime continues to open the complete project home and editor.

## Error Model

The boundary distinguishes runtime state from operation failure:

```ts
export class BackendUnavailableError extends Error {
  readonly code = "backend_unavailable";
}

export class FixtureOperationUnsupportedError extends Error {
  readonly code = "fixture_operation_unsupported";
}

export class BackendOperationError extends Error {
  readonly code = "backend_operation_failed";
  readonly operation: string;
  readonly cause: unknown;
}
```

- A disconnected browser does not call domain operations during bootstrap.
- Calling a backend-dependent operation while disconnected fails with
  `BackendUnavailableError`.
- Fixture operations not covered by the fixture contract fail with
  `FixtureOperationUnsupportedError` and name the operation.
- Tauri failures retain the operation name and native recovery detail inside
  `BackendOperationError`.
- Components use error codes and typed recovery information rather than
  matching the JavaScript error text produced by missing Tauri globals.

Future authentication, authorization, conflict, and reconnect errors will be
designed with the remote transport rather than guessed in this refactor.

## Cross-Platform Host Boundary

The desktop host may run on macOS, Windows, or Linux.

The frontend must not infer host capabilities from browser platform, local path
syntax, or user-agent information. A later host capability contract will
report:

- host platform and architecture;
- available import, analysis, rendering, and export capabilities;
- supported media URL and preview behavior;
- available sidecars and model runtimes;
- platform-specific limitations and recovery actions.

Current macOS-specific functionality can remain macOS-specific. This refactor
only ensures that the transport and domain interfaces do not encode macOS as a
universal assumption.

## Development Commands

Development entry points become explicit:

- `pnpm dev` launches the complete Tauri desktop runtime.
- `pnpm tauri:dev` remains the underlying native development command.
- `pnpm dev:web-runtime` starts the internal Vite server used by Tauri and
  browser tooling.
- Tauri's `beforeDevCommand` calls `pnpm dev:web-runtime`, preventing a command
  cycle when `pnpm dev` becomes the desktop entry point.
- Browser visual QA continues through `pnpm visual:qa:browser`.

Opening `dev:web-runtime` directly is supported for the disconnected-browser
shell and browser tooling. It is not described as a complete editor runtime.

## Verification Architecture

### Frontend verification

`verify:frontend` performs:

- source-quality checks that do not invoke Cargo;
- TypeScript linting;
- Vitest;
- the production frontend build;
- fixture-backed browser visual QA and baseline policy.

It must not execute Cargo, Tauri bundling, native sidecar builds, or native
tests.

### Fast native verification

`verify:native:fast` performs:

- Rust formatting;
- workspace `cargo check` with the default development feature set;
- focused Rust tests explicitly selected by the current change.

It does not run:

- the exact packaged custom-protocol Clippy profile;
- the MCP-only Clippy profile;
- the full all-target native suite;
- package or signing acceptance.

Focused test selection remains caller-provided because the affected Rust
surface depends on the change.

### Release verification

`verify:release` performs:

- the complete frontend gate;
- default workspace Clippy across all targets;
- exact MCP-only Clippy;
- exact packaged desktop custom-protocol Clippy;
- the full native suite and required native lanes;
- packaged-runtime source and policy checks;
- the existing browser baseline release gate.

The top-level `verify` command remains the complete final gate by delegating to
`verify:release`. Documentation for agentic work must say to use focused or
fast lanes during iteration and the complete gate before completion.

The custom-protocol and MCP profiles remain mandatory because they represent
different shipped executables and feature configurations.

## Cargo Profiles

Normal development retains incremental compilation:

```toml
[profile.dev]
debug = 1
incremental = true

[profile.test]
debug = 1
incremental = false
```

`debug = 1` preserves line-table information for useful stack traces while
avoiding full debug information.

Comprehensive verification also sets `CARGO_INCREMENTAL=0`, including Clippy
and check lanes executed inside that verification process. Release builds
continue to use Cargo's release-profile behavior.

The implementation must record cold and warm measurements before deciding
whether further codegen, LTO, or split-debug settings are warranted. Those
additional compiler settings are outside this design.

## Cargo Target Separation

The existing `src-tauri/target` remains the persistent development target.

Comprehensive verification uses:

```text
src-tauri/target/verify
```

through an explicit `CARGO_TARGET_DIR`.

All commands inside one comprehensive verification use the same verification
target so compatible dependency artifacts are reused. Normal Tauri
development never writes into the verification target.

The verification path is fixed relative to the canonical repository root.
Cleanup code must:

- resolve the canonical repository root;
- reject symlinked verification targets;
- reject targets outside `src-tauri/target/verify`;
- refuse empty, root, home, workspace-root, and unresolved-variable paths;
- report the exact path and measured size before deletion.

## Cache Policy

Constants:

- verification cache soft cap: 24 GiB;
- required free-space reserve before native verification: 20 GiB.

The verification wrapper measures the target and filesystem before starting.

If the verification cache is larger than 24 GiB or available space is below
20 GiB:

1. validate the exact verification target;
2. remove only `src-tauri/target/verify`;
3. remeasure available space;
4. fail closed if the 20 GiB reserve is still unavailable.

After success, failure, or cancellation, the wrapper remeasures the target. An
oversized verification target is removed after exact-path validation. A cache
within the cap is retained to accelerate the next verification.

The development target is never deleted automatically. Its size is reported,
and a separate explicit development-cache cleanup command is available.

The existing broader cleanup command remains separate because native sidecar
build directories and Git garbage collection have different ownership and
risk from Cargo verification output.

## Build Metrics

Every native verification writes a machine-readable report under:

```text
output/build-metrics/native-verification-latest.json
```

The report includes:

- schema version;
- start and completion timestamps;
- command and verification lane;
- success, failure, or cancellation status;
- cold or warm cache classification;
- filesystem capacity and available bytes before and after;
- development and verification target bytes before and after;
- whether policy cleanup ran and the exact validated target;
- elapsed duration;
- Cargo feature profiles executed.

The report contains no environment secrets, credentials, project contents, or
home-directory inventory.

Optimization acceptance compares:

1. a cold complete verification;
2. an unchanged warm complete verification;
3. a representative small Rust edit followed by fast native verification.

The implementation report must state measured results rather than promise a
specific speedup in advance.

## Testing Strategy

### Runtime unit tests

- Runtime selection chooses fixture only with its explicit marker.
- Runtime selection chooses desktop only with a real Tauri bridge.
- A regular browser starts disconnected.
- Runtime selection is immutable after bootstrap.
- Disconnected requests produce `BackendUnavailableError`.
- Unsupported fixture operations produce
  `FixtureOperationUnsupportedError`.
- Tauri operation failures retain the operation name and cause.

### Boundary policy tests

- Production components do not import `@tauri-apps/api/core` or
  `@tauri-apps/api/event`.
- Domain modules use the configured backend transport.
- Tauri imports are limited to the Tauri adapter.
- The fixture adapter is development-only.
- New interfaces and connection copy contain no provisional product name.

### Browser checks

- A raw browser URL shows the disconnected connection shell.
- Raw browser startup produces zero unexpected console errors.
- Raw browser startup makes no backend calls.
- Fixture-backed browser visual scenarios still render the editor.
- Existing desktop and narrow visual baselines remain valid unless the approved
  connection shell adds a new dedicated baseline.

### Desktop checks

- `pnpm dev` reaches the Tauri development startup path.
- The internal Vite command does not recursively invoke the desktop command.
- Desktop bootstrap loads preferences through `TauriTransport`.
- The bundled sample project still opens in the desktop runtime.
- Native menu events and media URL conversion still pass through the adapter.

### Cache policy tests

All cleanup tests use disposable fixture directories.

- Cache below both boundaries is retained.
- Cache above 24 GiB selects only the verification target for cleanup.
- Free space below 20 GiB triggers verification-cache cleanup.
- Insufficient free space after cleanup fails closed before Cargo starts.
- Symlinked, missing, root, home, workspace-root, and outside-repository targets
  are rejected.
- Failure and cancellation apply the same post-run size policy.
- The development target cannot be selected by automatic cleanup.
- Metrics accurately record cleanup, sizes, status, and elapsed duration.

### Release checks

- Default all-target Clippy remains present.
- MCP-only Clippy remains present.
- Packaged custom-protocol Clippy remains present.
- The full native suite and required native lanes remain present.
- The top-level final verification delegates to the release gate.

## Acceptance Criteria

- Raw browser startup no longer exposes a missing Tauri global error.
- A raw browser renders the disconnected connection shell without backend
  requests.
- Fixture browser mode remains explicit and deterministic.
- Desktop behavior remains functionally unchanged.
- React components contain no direct Tauri requests.
- Tauri imports are isolated to the adapter.
- `pnpm dev` launches the complete desktop runtime.
- Frontend verification runs without Cargo.
- Iterative native verification excludes release-only feature profiles.
- Final verification still covers default, custom-protocol, and MCP feature
  configurations.
- Comprehensive verification writes only to its dedicated Cargo target.
- Verification incremental output is disabled.
- Test/debug output uses line-table rather than full debug information.
- The verification cache is removed only when it exceeds 24 GiB or the
  filesystem violates the 20 GiB reserve.
- Automatic cleanup cannot delete the development target.
- Cold, warm, and representative-edit metrics are captured and reported.
- New contracts and user-facing connection copy do not encode the provisional
  product name or a single desktop operating system.

## Planned Follow-Up Designs

The following are intentionally separate designs. This refactor must preserve
their path without claiming to implement them.

### 1. Cross-platform desktop-host capability contract

Define versioned capability discovery for macOS, Windows, and Linux hosts.
Separate universal project operations from platform-specific import, model,
render, export, sidecar, and notification capabilities.

### 2. Local-network discovery and pairing

Design host discovery, explicit user approval, device identity, scoped
authentication, certificate handling, revocation, and local-network threat
boundaries.

### 3. Remote command and event protocol

Design versioned edit commands, progress events, project revisions,
idempotency, reconnect behavior, optimistic concurrency, and conflict
resolution. The desktop host remains authoritative for project mutation.

### 4. Remote media delivery

Design authorized thumbnail, waveform, proxy-preview, and render-artifact
delivery without exposing arbitrary host filesystem paths.

### 5. Responsive browser editing

Design phone, tablet, and desktop-browser workflows around the same canonical
project model. Mobile layouts remain editor-first and expose only interactions
that can be made reliable at the target viewport.

### 6. Access beyond the local network

Treat internet access as a separate security and deployment problem. Evaluate
relay, VPN, tunnel, and hosted-control-plane options only after the local
network protocol is secure and versioned.

## Rollout Order

1. Add runtime mode, connection, transport, and typed error primitives.
2. Add Tauri and fixture adapters with explicit bootstrap.
3. Move direct component commands into domain modules and migrate production
   Tauri imports to the transport boundary.
4. Add the disconnected browser shell and browser runtime checks.
5. Separate desktop, internal Vite, and verification commands.
6. Add Cargo profile and target separation.
7. Add cache preflight, safe cleanup, metrics, and policy tests.
8. Run focused tests, browser checks, desktop smoke, and release policy checks.
9. Clean the currently oversized rebuildable Cargo target only with explicit
   user authorization, then capture cold, warm, and representative-edit
   measurements.
