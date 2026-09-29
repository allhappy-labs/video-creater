# Runtime Boundary and Build Cache Optimization Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make the desktop runtime the default development experience, render a deliberate disconnected shell in a regular browser, isolate Tauri behind a transport boundary that can later support remote browser clients, and keep comprehensive Cargo verification from growing without bounds.

**Architecture:** Bootstrap selects one immutable `RuntimeDescriptor`: connected desktop through `TauriTransport`, connected development-only visual QA through `FixtureTransport`, or disconnected browser. Typed domain modules use a shared backend client; React components never invoke transport operations by string. Native verification runs in a fixed `src-tauri/target/verify` directory with incremental compilation disabled, exact-path cleanup guards, a 24 GiB cache cap, a 20 GiB free-space reserve, and machine-readable metrics.

**Tech Stack:** React 19, TypeScript 5, Vite 6, Vitest, Tauri 2, Rust/Cargo, Node.js scripts and `node:test`, Playwright browser visual QA.

## Global Constraints

- Preserve all existing user changes and keep each commit limited to the task named in that checkpoint.
- Prefix every repository shell command with `rtk`.
- Use Conventional Commits.
- Do not add the provisional application name to new types, globals, runtime copy, transport protocols, or cache contracts.
- Existing Rust command names, binary names, bundle identifiers, storage keys, and native event names are compatibility surfaces; this refactor may route them through the new boundary without renaming them.
- Only files under `src/lib/runtime/adapters/` may import `@tauri-apps/api/core` or `@tauri-apps/api/event`.
- Fixture mode requires an explicit development-only marker. It must never be inferred from missing Tauri globals.
- A disconnected browser must not issue preference, project, provider, filesystem, media, or event operations.
- Do not implement remote discovery, pairing, authentication, HTTP, WebSocket, conflict resolution, or mobile editing in this change.
- Never automatically delete `src-tauri/target`, the repository root, the home directory, or any path other than the validated `src-tauri/target/verify`.
- Do not remove the exact default, MCP-only, or packaged custom-protocol Clippy profiles from complete verification.
- Do not claim a build speedup until cold, warm, and focused-edit measurements have been recorded.

---

## Task 1: Add the Runtime and Backend Primitives

**Files:**

- Create: `src/lib/runtime/backend-transport.ts`
- Create: `src/lib/runtime/backend-client.ts`
- Create: `src/lib/runtime/backend-client.test.ts`
- Create: `src/lib/runtime/runtime-descriptor.ts`
- Create: `src/lib/runtime/runtime-descriptor.test.ts`

- [ ] **Step 1: Write failing tests for connected and disconnected backend behavior**

Cover:

- a disconnected client rejects `request`, `listen`, and `mediaUrl` with `BackendUnavailableError`;
- a connected client delegates all three operations;
- transport failures are wrapped in `BackendOperationError` with the operation name and original cause;
- a connection can be installed once but cannot silently change;
- runtime mode accepts only `desktop`, `browser`, and `fixture`.

Use a local recording transport rather than mocking Tauri:

```ts
const transport: BackendTransport = {
  kind: "fixture",
  request: vi.fn(async () => ({ ok: true })),
  listen: vi.fn(async () => vi.fn()),
  mediaUrl: vi.fn((path) => `fixture-media:${path}`),
};
```

Run:

```bash
rtk pnpm exec vitest run src/lib/runtime/backend-client.test.ts src/lib/runtime/runtime-descriptor.test.ts
```

Expected: FAIL because the runtime modules do not exist.

- [ ] **Step 2: Implement the transport contract and typed errors**

`src/lib/runtime/backend-transport.ts` must export this contract:

```ts
export type BackendTransportKind = "tauri" | "fixture" | "remote";
export type BackendInput = Record<string, unknown>;
export type BackendUnlisten = () => void;

export interface BackendTransport {
  readonly kind: BackendTransportKind;
  request<Result>(operation: string, input?: BackendInput): Promise<Result>;
  listen<Payload>(
    event: string,
    handler: (payload: Payload) => void,
  ): Promise<BackendUnlisten>;
  mediaUrl(path: string): string;
}

export class BackendUnavailableError extends Error {
  readonly code = "backend_unavailable";
}

export class FixtureOperationUnsupportedError extends Error {
  readonly code = "fixture_operation_unsupported";
  constructor(readonly operation: string) {
    super(`Fixture operation is not implemented: ${operation}`);
  }
}

export class BackendOperationError extends Error {
  readonly code = "backend_operation_failed";
  constructor(
    readonly operation: string,
    readonly cause: unknown,
  ) {
    super(
      cause instanceof Error
        ? `${operation}: ${cause.message}`
        : `${operation}: backend operation failed`,
    );
  }
}
```

- [ ] **Step 3: Implement an install-once backend client**

`src/lib/runtime/backend-client.ts` must expose both an independently testable class and the application singleton:

```ts
export type BackendConnection =
  | { status: "disconnected" }
  | { status: "connected"; transport: BackendTransport };

export class BackendClient {
  private connection: BackendConnection | null = null;

  install(connection: BackendConnection): void;
  request<Result>(operation: string, input?: BackendInput): Promise<Result>;
  listen<Payload>(
    event: string,
    handler: (payload: Payload) => void,
  ): Promise<BackendUnlisten>;
  mediaUrl(path: string): string;
}

export const backendClient = new BackendClient();
export const backendRequest = backendClient.request.bind(backendClient);
export const backendListen = backendClient.listen.bind(backendClient);
export const backendMediaUrl = backendClient.mediaUrl.bind(backendClient);
```

`install` must throw on a second call. Tests that need a fresh client instantiate `BackendClient`; production modules use only the singleton.

- [ ] **Step 4: Implement the immutable runtime descriptor**

```ts
export type RuntimeMode = "desktop" | "browser" | "fixture";

export interface RuntimeDescriptor {
  readonly mode: RuntimeMode;
  readonly connection: BackendConnection;
}

export function createRuntimeDescriptor(
  mode: RuntimeMode,
  connection: BackendConnection,
): RuntimeDescriptor {
  if (mode !== "browser" && connection.status !== "connected") {
    throw new Error(`${mode} runtime requires a connected backend`);
  }
  return Object.freeze({ mode, connection });
}
```

- [ ] **Step 5: Run focused tests**

```bash
rtk pnpm exec vitest run src/lib/runtime/backend-client.test.ts src/lib/runtime/runtime-descriptor.test.ts
```

Expected: PASS.

- [ ] **Step 6: Commit**

```bash
rtk git add src/lib/runtime
rtk git commit -m "feat(runtime): add backend transport primitives"
```

---

## Task 2: Implement Tauri and Fixture Transport Adapters

**Files:**

- Create: `src/lib/runtime/adapters/tauri-transport.ts`
- Create: `src/lib/runtime/adapters/tauri-transport.test.ts`
- Create: `src/lib/runtime/adapters/fixture-transport.ts`
- Create: `src/lib/runtime/adapters/fixture-transport.test.ts`
- Create: `src/lib/runtime/adapters/tauri-settings-acceptance-bridge.ts`

- [ ] **Step 1: Write failing adapter tests**

The Tauri adapter tests mock `@tauri-apps/api/core` and `@tauri-apps/api/event` only at the adapter boundary. Verify:

- `request` delegates to Tauri `invoke`;
- `listen` unwraps Tauri's `{ payload }` event object;
- `mediaUrl` delegates to `convertFileSrc`;
- failures remain available to `BackendClient` for typed wrapping.

The fixture adapter tests verify:

- registered operation handlers receive cloned input and return cloned output;
- unsupported operations throw `FixtureOperationUnsupportedError`;
- listeners receive emitted fixture payloads;
- unlisten removes only the selected listener;
- fixture media URLs are deterministic and do not expose local filesystem URLs.

Run:

```bash
rtk pnpm exec vitest run src/lib/runtime/adapters/tauri-transport.test.ts src/lib/runtime/adapters/fixture-transport.test.ts
```

Expected: FAIL because the adapters do not exist.

- [ ] **Step 2: Implement `TauriTransport`**

This is the production adapter and the only runtime implementation that imports Tauri APIs:

```ts
import { convertFileSrc, invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";

export class TauriTransport implements BackendTransport {
  readonly kind = "tauri" as const;

  request<Result>(operation: string, input?: BackendInput): Promise<Result> {
    return invoke<Result>(operation, input);
  }

  listen<Payload>(
    event: string,
    handler: (payload: Payload) => void,
  ): Promise<BackendUnlisten> {
    return listen<Payload>(event, ({ payload }) => handler(payload));
  }

  mediaUrl(path: string): string {
    return convertFileSrc(path);
  }
}
```

- [ ] **Step 3: Implement `FixtureTransport`**

Use explicit operation handlers and an in-memory event map:

```ts
export type FixtureOperationHandler = (
  input: BackendInput,
) => unknown | Promise<unknown>;

export class FixtureTransport implements BackendTransport {
  readonly kind = "fixture" as const;
  constructor(
    private readonly operations: ReadonlyMap<string, FixtureOperationHandler>,
  ) {}

  request<Result>(operation: string, input: BackendInput = {}): Promise<Result>;
  listen<Payload>(
    event: string,
    handler: (payload: Payload) => void,
  ): Promise<BackendUnlisten>;
  mediaUrl(path: string): string;
  emitForVisualQa<Payload>(event: string, payload: Payload): void;
}
```

`emitForVisualQa` is intentionally fixture-only and is not added to `BackendTransport`.

- [ ] **Step 4: Isolate the packaged Settings acceptance bridge**

Move the current `tauriInvoke` and `tauriEmit` imports out of `settings-acceptance-runner.ts`. `tauri-settings-acceptance-bridge.ts` exports:

```ts
export function createTauriSettingsAcceptanceBridge(): SettingsAcceptanceBridge {
  return {
    invoke: (operation, input) => invoke(operation, input),
    emit: (event, payload) => emit(event, payload),
    document,
    sleep: (milliseconds) =>
      new Promise((resolve) => window.setTimeout(resolve, milliseconds)),
  };
}
```

The bootstrap task will dynamically load this adapter only in desktop mode. The acceptance runner keeps its explicit injected bridge and no longer has a default Tauri bridge.

- [ ] **Step 5: Run focused tests**

```bash
rtk pnpm exec vitest run src/lib/runtime/adapters/tauri-transport.test.ts src/lib/runtime/adapters/fixture-transport.test.ts
```

Expected: PASS.

- [ ] **Step 6: Commit**

```bash
rtk git add src/lib/runtime/adapters
rtk git commit -m "feat(runtime): add desktop and fixture adapters"
```

---

## Task 3: Bootstrap Runtime Mode and Render the Disconnected Browser Shell

**Files:**

- Create: `src/lib/runtime/bootstrap.ts`
- Create: `src/lib/runtime/bootstrap.test.ts`
- Create: `src/components/runtime/desktop-host-connection.tsx`
- Create: `src/components/runtime/desktop-host-connection.test.tsx`
- Modify: `src/main.tsx`
- Modify: `src/App.tsx`
- Modify: `src/App.test.tsx`

- [ ] **Step 1: Write failing bootstrap tests**

Inject capabilities into a pure selector so the test never depends on ambient globals:

```ts
export interface RuntimeBootstrapInput {
  fixtureTransport: FixtureTransport | null;
  tauriAvailable: boolean;
}

export function selectRuntime(
  input: RuntimeBootstrapInput,
): RuntimeDescriptor;
```

Verify priority and output:

- explicit fixture wins over Tauri;
- Tauri selects connected desktop;
- neither selects disconnected browser;
- a missing Tauri global never selects fixture mode.

- [ ] **Step 2: Write the disconnected-shell component test**

Assert the rendered shell:

- has `main` label `Desktop host connection`;
- displays `Connect to a desktop host`;
- says a running desktop host is required;
- does not mention the provisional application name, macOS, Windows, or Linux;
- does not offer a fake connect action before pairing exists.

Run:

```bash
rtk pnpm exec vitest run src/lib/runtime/bootstrap.test.ts src/components/runtime/desktop-host-connection.test.tsx
```

Expected: FAIL.

- [ ] **Step 3: Implement bootstrap detection**

Use a product-neutral development marker:

```ts
export interface EditorFixtureRuntimeMarker {
  readonly enabled: true;
  readonly settingsFixtureId?: string;
  readonly exportCapabilities?: readonly unknown[];
}

declare global {
  interface Window {
    __EDITOR_FIXTURE_RUNTIME__?: EditorFixtureRuntimeMarker;
    __TAURI_INTERNALS__?: unknown;
  }
}
```

`bootstrapRuntime()` must:

1. check `import.meta.env.DEV` and `__EDITOR_FIXTURE_RUNTIME__.enabled`;
2. dynamically import `fixture-bootstrap.ts` only in that branch;
3. otherwise detect `__TAURI_INTERNALS__` and create `TauriTransport`;
4. otherwise return disconnected browser;
5. install the descriptor's connection into `backendClient` exactly once.

- [ ] **Step 4: Split `App` into runtime shell and connected application**

Keep the current hooks in a connected child so hook order remains stable:

```tsx
export interface AppProps {
  runtime: RuntimeDescriptor;
}

export default function App({ runtime }: AppProps) {
  return runtime.connection.status === "disconnected" ? (
    <DesktopHostConnection />
  ) : (
    <ConnectedApp />
  );
}

function ConnectedApp() {
  // Existing App implementation moves here unchanged first.
}
```

Update `App.test.tsx` to supply a connected fixture descriptor. Add a test with a disconnected descriptor and prove `loadAppPreferences`, event listeners, and backend requests are not called.

- [ ] **Step 5: Wire `main.tsx`**

`main()` awaits `bootstrapRuntime()`, renders `<App runtime={runtime} />`, and only runs Settings packaged acceptance when `runtime.mode === "desktop"`. Dynamically import `createTauriSettingsAcceptanceBridge` in that desktop-only branch and pass it explicitly to `runSettingsAcceptanceIfEnabled`.

- [ ] **Step 6: Run focused tests**

```bash
rtk pnpm exec vitest run src/lib/runtime/bootstrap.test.ts src/components/runtime/desktop-host-connection.test.tsx src/App.test.tsx
```

Expected: PASS.

- [ ] **Step 7: Commit**

```bash
rtk git add src/main.tsx src/App.tsx src/App.test.tsx src/components/runtime src/lib/runtime
rtk git commit -m "feat(runtime): add disconnected browser bootstrap"
```

---

## Task 4: Route Domain Operations, Events, and Media Through the Boundary

**Files:**

- Modify: `src/App.tsx`
- Modify: `src/components/workspace/editor-workspace.tsx`
- Modify: `src/lib/app-settings.ts`
- Modify: `src/lib/native-menu.ts`
- Modify: `src/lib/project.ts`
- Modify: `src/lib/provider-account.ts`
- Modify: `src/lib/provider-credentials.ts`
- Modify: `src/lib/settings-acceptance-runner.ts`
- Modify: `src/lib/settings/agent.ts`
- Modify: `src/lib/settings/health.ts`
- Modify: `src/lib/settings/operations.ts`
- Modify: `src/lib/settings/providers.ts`
- Modify: `src/lib/settings/render-system.ts`
- Modify: `src/lib/settings/skills.ts`
- Modify: `src/lib/settings/storage.ts`
- Modify: `src/lib/settings/use-settings-operations.ts`
- Modify: `src/lib/shader-background-templates.ts`
- Modify: `src/lib/transcription-models.ts`
- Modify matching tests for every module above
- Create: `scripts/runtime-boundary-policy.test.ts`
- Modify: `package.json`

- [ ] **Step 1: Add a failing import-boundary policy test**

`scripts/runtime-boundary-policy.test.ts` recursively scans production `.ts` and `.tsx` files under `src`. It must fail if a Tauri core/event import appears outside:

- `src/lib/runtime/adapters/tauri-transport.ts`
- `src/lib/runtime/adapters/tauri-settings-acceptance-bridge.ts`

It must also fail if `invoke(` or `convertFileSrc(` appears in `App.tsx` or `src/components/`.

Add the policy test to `test:source-quality`.

Run:

```bash
rtk node --test scripts/runtime-boundary-policy.test.ts
```

Expected: FAIL and list the current direct imports.

- [ ] **Step 2: Migrate typed domain wrappers mechanically**

Replace direct `invoke` calls with `backendRequest<Result>`. Preserve every public function signature and native operation string. Example:

```ts
export function listTranscriptionModels(): Promise<TranscriptionModelStatus[]> {
  return backendRequest("list_transcription_models");
}
```

Update each unit test to mock `@/lib/runtime/backend-client` rather than Tauri. Do not give components a generic request prop.

- [ ] **Step 3: Move component-native commands into typed domain functions**

Add to `src/lib/project.ts`:

```ts
export function materializeSampleProjectMedia(input: {
  projectDir: string;
}): Promise<void> {
  return backendRequest("materialize_sample_project_media", input);
}

export function analyzeProjectSpeech(input: {
  projectDir: string;
  mediaId: string;
}): Promise<unknown> {
  return backendRequest("analyze_project_speech", input);
}

export function assignProjectMediaSpeaker(input: {
  projectDir: string;
  mediaId: string;
  speakerId: string;
}): Promise<unknown> {
  return backendRequest("assign_project_media_speaker", input);
}
```

Before committing, copy the exact existing argument shapes and return types from the current calls; do not retain the illustrative `unknown` return if a concrete current type exists.

Replace the direct calls in `App.tsx` and `editor-workspace.tsx`.

- [ ] **Step 4: Route native events and media URLs**

`native-menu.ts` and `use-settings-operations.ts` use `backendListen`. Event parsers remain unchanged.

Replace `convertFileSrc(safePath)` in `editor-workspace.tsx` with `backendMediaUrl(safePath)`.

- [ ] **Step 5: Remove missing-global string matching**

Replace `isTauriBridgeUnavailableError` with error-code detection:

```ts
export function isBackendUnavailableError(
  error: unknown,
): error is BackendUnavailableError {
  return error instanceof BackendUnavailableError;
}
```

Callers handle `backend_unavailable`; they no longer match `__TAURI_INTERNALS__`, `invoke`, or `transformCallback` error strings.

- [ ] **Step 6: Keep the acceptance runner injected**

Remove its Tauri imports and default bridge. Require an explicit bridge:

```ts
export async function runSettingsAcceptanceIfEnabled(
  bridge: SettingsAcceptanceBridge,
): Promise<void>;
```

Existing acceptance unit tests continue to inject their deterministic bridge.

- [ ] **Step 7: Run the boundary and affected tests**

```bash
rtk node --test scripts/runtime-boundary-policy.test.ts
rtk pnpm exec vitest run src/App.test.tsx src/lib/app-settings.test.ts src/lib/native-menu.test.ts src/lib/project.test.ts src/lib/provider-credentials.test.ts src/lib/transcription-models.test.ts src/lib/settings
rtk pnpm lint
```

Expected: PASS, and `rtk rg '@tauri-apps/api/(core|event)' src` returns only the two adapter files and adapter tests.

- [ ] **Step 8: Commit**

```bash
rtk git add package.json scripts/runtime-boundary-policy.test.ts src
rtk git commit -m "refactor(runtime): route frontend operations through backend boundary"
```

---

## Task 5: Replace Fake Tauri Globals in Fixture-Backed Browser QA

**Files:**

- Create: `src/lib/runtime/fixture-bootstrap.ts`
- Create: `src/lib/runtime/fixture-bootstrap.test.ts`
- Modify: `src/lib/settings-visual-qa-fixtures.ts`
- Modify: `src/lib/settings-visual-qa-fixtures.test.ts`
- Modify: `src/browser-visual-qa-script.test.ts`
- Modify: `scripts/browser-visual-qa.mjs`

- [ ] **Step 1: Write failing fixture-bootstrap tests**

Verify that the fixture bootstrap:

- installs base visual-QA preferences and export capabilities;
- overlays a selected Settings fixture;
- maps each existing `fixtureCommandResponses` entry to an explicit fixture operation;
- exposes native-menu event emission through `FixtureTransport.emitForVisualQa`;
- rejects unknown operations with `FixtureOperationUnsupportedError`;
- exists only behind the development marker.

Run:

```bash
rtk pnpm exec vitest run src/lib/runtime/fixture-bootstrap.test.ts src/lib/settings-visual-qa-fixtures.test.ts
```

Expected: FAIL.

- [ ] **Step 2: Convert Settings fixture installation to operation registration**

Replace `installSettingsVisualQaFixtureBridge` with:

```ts
export function settingsVisualQaOperations(
  fixture: SettingsVisualQaFixture,
): ReadonlyMap<string, FixtureOperationHandler>;
```

Delete `TauriInternals`, callback IDs, plugin event emulation, and writes to `__TAURI_INTERNALS__` or `__TAURI_EVENT_PLUGIN_INTERNALS__`.

- [ ] **Step 3: Build the development-only fixture transport**

`fixture-bootstrap.ts` reads `__EDITOR_FIXTURE_RUNTIME__`, builds one operation map, and returns the `FixtureTransport` plus a product-neutral driver:

```ts
declare global {
  interface Window {
    __EDITOR_FIXTURE_DRIVER__?: {
      emit(event: string, payload: unknown): void;
    };
  }
}
```

Install the driver only in development fixture mode. It must call `emitForVisualQa`; it must not emulate Tauri callbacks.

- [ ] **Step 4: Update the browser harness**

In `scripts/browser-visual-qa.mjs`:

- replace the fake `__TAURI_INTERNALS__` init script with `__EDITOR_FIXTURE_RUNTIME__`;
- pass preferences, export capabilities, and optional Settings fixture ID through that marker;
- replace `plugin:event|emit` calls with `__EDITOR_FIXTURE_DRIVER__.emit(...)`;
- remove the legacy browser-QA, Settings-QA, and visual-export globals whose
  names contain the provisional product name; the neutral runtime marker owns
  all three inputs;
- keep existing visual scenarios, selectors, screenshots, thresholds, and baseline paths unchanged.

Update `src/browser-visual-qa-script.test.ts` to require the two
product-neutral fixture globals and explicitly reject `__TAURI_INTERNALS__`,
`__TAURI_EVENT_PLUGIN_INTERNALS__`, and the three removed provisional-name
globals.

- [ ] **Step 5: Run fixture and script tests**

```bash
rtk pnpm exec vitest run src/lib/runtime/fixture-bootstrap.test.ts src/lib/settings-visual-qa-fixtures.test.ts src/browser-visual-qa-script.test.ts
rtk node --check scripts/browser-visual-qa.mjs
```

Expected: PASS.

- [ ] **Step 6: Run one browser smoke scenario**

Start the internal Vite runtime in one terminal:

```bash
rtk pnpm dev:web-runtime
```

In another terminal:

```bash
rtk pnpm visual:qa:browser -- --only home
```

Expected: the fixture-backed project home renders without page or console errors and writes its screenshot under `output/playwright/browser-visual-qa`.

- [ ] **Step 7: Commit**

```bash
rtk git add scripts/browser-visual-qa.mjs src/browser-visual-qa-script.test.ts src/lib/runtime src/lib/settings-visual-qa-fixtures.ts src/lib/settings-visual-qa-fixtures.test.ts
rtk git commit -m "refactor(qa): replace fake Tauri globals with fixture transport"
```

---

## Task 6: Make Desktop Development the Default Entry Point

**Files:**

- Modify: `package.json`
- Modify: `src-tauri/tauri.conf.json`
- Modify: `scripts/gstreamer-release-policy.test.ts`
- Create: `scripts/development-entrypoint-policy.test.ts`

- [ ] **Step 1: Write failing command-policy tests**

Require:

```json
{
  "dev": "pnpm tauri:dev",
  "dev:web-runtime": "vite --host 127.0.0.1",
  "tauri:dev": "pnpm prepare:tauri:dev && tauri dev"
}
```

and:

```json
{
  "build": {
    "beforeDevCommand": "pnpm dev:web-runtime"
  }
}
```

Also assert there is no command cycle among `dev`, `tauri:dev`, and `beforeDevCommand`.

Run:

```bash
rtk node --test scripts/development-entrypoint-policy.test.ts scripts/gstreamer-release-policy.test.ts
```

Expected: FAIL against the current scripts.

- [ ] **Step 2: Update scripts and Tauri configuration**

Make only the changes asserted above. Keep runtime preparation in `tauri:dev`, so Tauri begins polling `devUrl` only after helper preparation completes.

- [ ] **Step 3: Run policy tests**

```bash
rtk node --test scripts/development-entrypoint-policy.test.ts scripts/gstreamer-release-policy.test.ts
```

Expected: PASS.

- [ ] **Step 4: Verify the raw browser shell**

Run:

```bash
rtk pnpm dev:web-runtime
```

Open `http://127.0.0.1:1420` with the Playwright workflow and verify:

- `Connect to a desktop host` is visible;
- there are no page errors or console errors;
- the network/runtime trace contains no backend operation attempt;
- no fixture marker is present.

Save evidence to:

```text
output/playwright/runtime-boundary/disconnected-browser.png
output/playwright/runtime-boundary/disconnected-browser.json
```

- [ ] **Step 5: Commit**

```bash
rtk git add package.json src-tauri/tauri.conf.json scripts/development-entrypoint-policy.test.ts scripts/gstreamer-release-policy.test.ts
rtk git commit -m "build(dev): make desktop runtime the default"
```

---

## Task 7: Add Cargo Profiles and Safe Cache Policy Primitives

**Files:**

- Modify: `src-tauri/Cargo.toml`
- Create: `scripts/cargo-cache-policy.mjs`
- Create: `scripts/cargo-cache-policy.test.ts`
- Modify: `scripts/clean-caches.mjs`
- Modify: `package.json`

- [ ] **Step 1: Write failing Cargo-profile and cache-policy tests**

Test constants:

```ts
export const VERIFY_CACHE_LIMIT_BYTES = 24 * 1024 ** 3;
export const REQUIRED_FREE_BYTES = 20 * 1024 ** 3;
```

Test policy decisions:

- cache at or below 24 GiB with at least 20 GiB free is retained;
- cache above 24 GiB selects cleanup;
- free space below 20 GiB selects cleanup;
- cleanup still fails closed if the reserve remains unavailable;
- missing verification target is valid and reports zero bytes.

Test path safety with temporary directories:

- canonical exact `src-tauri/target/verify` is accepted;
- a symlink at `verify` is rejected;
- parent traversal, repository root, workspace root, home, filesystem root, empty strings, unresolved `$VARIABLE`, and any sibling under `target` are rejected.

Test the manifest contains:

```toml
[profile.dev]
debug = 1
incremental = true

[profile.test]
debug = 1
incremental = false
```

Run:

```bash
rtk node --test scripts/cargo-cache-policy.test.ts
```

Expected: FAIL.

- [ ] **Step 2: Add Cargo profiles**

Append the exact development and test profiles above. Do not change release profile, LTO, codegen units, or panic behavior.

- [ ] **Step 3: Implement pure policy and filesystem measurement**

Export pure functions:

```ts
export function decideVerificationCacheAction(input: {
  verificationBytes: number;
  availableBytes: number;
}): "retain" | "clean";

export function validateCargoTargetPath(input: {
  repoRoot: string;
  targetPath: string;
  kind: "verification" | "development";
}): Promise<string>;

export function directorySizeBytes(path: string): Promise<number>;
export function filesystemBytes(path: string): Promise<{
  capacityBytes: number;
  availableBytes: number;
}>;
```

Use `node:fs/promises.statfs` for cross-platform capacity measurement. Walk directories without following symbolic links. Validation must canonicalize the repository and existing parents, inspect the final entry with `lstat`, and compare against the exact expected path before any removal.

- [ ] **Step 4: Add explicit development cleanup without automatic deletion**

Expose:

```bash
rtk pnpm clean:cargo:dev
```

This command validates and reports the exact `src-tauri/target` path and size before running `cargo clean --manifest-path src-tauri/Cargo.toml`. It executes only because the user explicitly invoked the command. Keep `clean:caches` separate and preserve its native-sidecar and Git cleanup behavior.

- [ ] **Step 5: Run policy tests**

```bash
rtk node --test scripts/cargo-cache-policy.test.ts
```

Expected: PASS.

- [ ] **Step 6: Commit**

```bash
rtk git add src-tauri/Cargo.toml scripts/cargo-cache-policy.mjs scripts/cargo-cache-policy.test.ts scripts/clean-caches.mjs package.json
rtk git commit -m "build(cargo): bound verification cache policy"
```

---

## Task 8: Add Native Verification Wrapper, Metrics, and Verification Tiers

**Files:**

- Create: `scripts/run-native-verification.mjs`
- Create: `scripts/native-verification-policy.test.ts`
- Modify: `scripts/run-native-rust-tests.mjs`
- Modify: `scripts/native-rust-test-policy.test.ts`
- Modify: `scripts/zero-debt-gate.test.ts`
- Modify: `package.json`

- [ ] **Step 1: Write failing wrapper-policy tests**

Test parsing and command plans without spawning Cargo:

- `--lane fast` requires at least one positional Rust test filter;
- fast runs fmt, default workspace check, then one `cargo test --lib` command per supplied exact test filter;
- fast excludes all-target, MCP-only, custom-protocol, and native AppKit lanes;
- release runs fmt, the three exact existing Clippy profiles, and `run-native-rust-tests.mjs`;
- every Cargo child receives:

```text
CARGO_TARGET_DIR=/canonical/repository/path/src-tauri/target/verify
CARGO_INCREMENTAL=0
```

- SIGINT and SIGTERM are forwarded to the active child and produce `cancelled` metrics;
- success, failure, and cancellation all execute final measurement and post-run cap enforcement.

Run:

```bash
rtk node --test scripts/native-verification-policy.test.ts scripts/native-rust-test-policy.test.ts scripts/zero-debt-gate.test.ts
```

Expected: FAIL.

- [ ] **Step 2: Make native Rust lanes inherit the wrapper environment**

Keep `run-native-rust-tests.mjs` command coverage unchanged. Add its effective `CARGO_TARGET_DIR` and `CARGO_INCREMENTAL` values to the environment section of the existing report, and assert them in `native-rust-test-policy.test.ts`.

- [ ] **Step 3: Implement the wrapper**

Required lifecycle:

1. locate the canonical repository root;
2. measure development target, verification target, and filesystem;
3. classify cache as `cold` when verification target is absent/empty, otherwise `warm`;
4. apply pre-run cleanup policy only to the validated verification target;
5. fail closed before Cargo if 20 GiB remains unavailable;
6. run the selected plan sequentially with the fixed environment;
7. forward cancellation to the active child;
8. remeasure in `finally`;
9. remove an oversized verification target after exact validation;
10. write metrics atomically via a temporary file and rename.

The metrics schema is:

```ts
export interface NativeVerificationMetrics {
  schemaVersion: 1;
  lane: "fast" | "release";
  command: string[];
  status: "passed" | "failed" | "cancelled" | "blocked";
  startedAt: string;
  completedAt: string;
  elapsedMilliseconds: number;
  cache: {
    classification: "cold" | "warm";
    cleanupRan: boolean;
    cleanupTarget: string | null;
    cleanupReasons: Array<"over_limit" | "low_free_space" | "post_run_over_limit">;
  };
  filesystem: {
    capacityBytesBefore: number;
    availableBytesBefore: number;
    capacityBytesAfter: number;
    availableBytesAfter: number;
  };
  targets: {
    developmentBytesBefore: number;
    developmentBytesAfter: number;
    verificationBytesBefore: number;
    verificationBytesAfter: number;
  };
  cargo: {
    targetDir: string;
    incremental: "0";
    featureProfiles: string[];
  };
  steps: Array<{
    command: string[];
    status: "passed" | "failed" | "cancelled";
    elapsedMilliseconds: number;
    exitCode: number | null;
  }>;
}
```

Write the latest report to `output/build-metrics/native-verification-latest.json`.

- [ ] **Step 4: Define package verification tiers**

Use these responsibilities:

```json
{
  "verify:frontend": "pnpm check:source-quality && pnpm check:tooling-source && pnpm test:source-quality && pnpm lint && pnpm test && pnpm build && pnpm visual:qa:browser-release",
  "verify:native:fast": "node scripts/run-native-verification.mjs --lane fast",
  "verify:native:release": "node scripts/run-native-verification.mjs --lane release",
  "test:verification-policy": "node --test scripts/cargo-cache-policy.test.ts scripts/native-verification-policy.test.ts scripts/native-rust-test-policy.test.ts scripts/zero-debt-gate.test.ts",
  "verify:release": "pnpm verify:frontend && pnpm test:gstreamer-release-policy && pnpm test:release-runtime-policy && pnpm check:release-runtime-policy && pnpm test:verification-policy && pnpm verify:native:release",
  "verify:zero-debt": "pnpm verify:release",
  "verify": "pnpm verify:release"
}
```

Invocation for focused Rust work:

```bash
rtk pnpm verify:native:fast -- frame_compositor::gpu::tests::gpu_matches_canonical_cpu_blends_with_documented_tolerance
```

Keep `rust:clippy` and `rust:test:native` as directly invocable compatibility scripts, but the release wrapper owns their target directory and incremental settings during complete verification.

- [ ] **Step 5: Assert frontend verification contains no native work**

`zero-debt-gate.test.ts` must parse the `verify:frontend` composition and reject the tokens `cargo`, `tauri`, `rust:`, native helper builds, and native test scripts. It must separately assert that release verification delegates to the native release wrapper.

Add `runtime-boundary-policy.test.ts` and
`development-entrypoint-policy.test.ts` to `test:source-quality`, so the
frontend gate enforces those source-only contracts without running Cargo.

- [ ] **Step 6: Run policy tests**

```bash
rtk node --test scripts/cargo-cache-policy.test.ts scripts/native-verification-policy.test.ts scripts/native-rust-test-policy.test.ts scripts/zero-debt-gate.test.ts
```

Expected: PASS.

- [ ] **Step 7: Commit**

```bash
rtk git add package.json scripts/run-native-verification.mjs scripts/native-verification-policy.test.ts scripts/run-native-rust-tests.mjs scripts/native-rust-test-policy.test.ts scripts/zero-debt-gate.test.ts
rtk git commit -m "build(verify): separate frontend and bounded native gates"
```

---

## Task 9: Document the Operational Contract and Planned Remote Work

**Files:**

- Modify: `README.md`
- Modify: `docs/superpowers/specs/2026-07-27-runtime-boundary-build-cache-optimization-design.md`
- Create: `docs/development/runtime-and-verification.md`

- [ ] **Step 1: Document development commands**

State:

- `rtk pnpm dev` is the complete desktop development runtime;
- `rtk pnpm dev:web-runtime` is the internal Vite/disconnected-browser entry point;
- `rtk pnpm visual:qa:browser` is fixture-backed browser QA;
- a raw browser is intentionally disconnected until future pairing exists.

- [ ] **Step 2: Document verification selection**

Include:

```bash
rtk pnpm verify:frontend
rtk pnpm verify:native:fast -- frame_compositor::gpu::tests::gpu_matches_canonical_cpu_blends_with_documented_tolerance
rtk pnpm verify
rtk pnpm clean:cargo:dev
```

Explain that complete verification retains all three shipped Rust feature profiles, uses `src-tauri/target/verify`, enforces the 24 GiB/20 GiB policy, and writes the latest metrics report.

- [ ] **Step 3: Mark implemented versus planned scope in the approved spec**

Add an implementation-status section that links to the runtime modules and operational guide. Keep the six planned follow-up designs explicitly future work:

1. cross-platform desktop-host capability contract;
2. LAN discovery, pairing, authentication, and TLS;
3. remote command/event protocol, revisions, conflicts, and reconnect;
4. remote thumbnails, proxies, and artifact delivery;
5. responsive phone/tablet editing;
6. beyond-LAN security and deployment.

Do not name the provisional application in new prose or future protocol examples.

- [ ] **Step 4: Run documentation and source policy checks**

```bash
rtk pnpm check:source-quality
rtk node --test scripts/runtime-boundary-policy.test.ts scripts/development-entrypoint-policy.test.ts scripts/zero-debt-gate.test.ts
```

Expected: checks PASS. Then run the repository's placeholder scan across the
two changed documents and confirm that it returns no matches.

- [ ] **Step 5: Commit**

```bash
rtk git add README.md docs/development/runtime-and-verification.md docs/superpowers/specs/2026-07-27-runtime-boundary-build-cache-optimization-design.md
rtk git commit -m "docs(development): explain runtime and verification lanes"
```

---

## Task 10: Integrated Verification and Build Measurements

**Files:**

- Modify only if evidence finds a defect in the implementation above.
- Generate ignored evidence under:
  - `output/playwright/runtime-boundary/`
  - `output/build-metrics/`

- [ ] **Step 1: Verify repository scope**

```bash
rtk git status --short
rtk git diff --check
rtk git log --oneline -10
```

Expected: no unrelated edits and no whitespace errors.

- [ ] **Step 2: Run the frontend gate**

```bash
rtk pnpm verify:frontend
```

Expected: source policies, TypeScript, Vitest, frontend build, and fixture-backed browser baseline all PASS without invoking Cargo.

- [ ] **Step 3: Verify the disconnected browser separately**

With `rtk pnpm dev:web-runtime` running, use Playwright to open the raw URL and record:

- connection shell visible;
- no page/console errors;
- no fake Tauri globals;
- no backend calls;
- screenshot and JSON evidence paths.

- [ ] **Step 4: Verify the desktop runtime separately**

Run:

```bash
rtk pnpm dev
```

Confirm the Tauri window reaches project home, preferences load, the sample project opens, native-menu events still work, and media URLs resolve. This is desktop evidence; do not present the raw-browser or fixture checks as a substitute.

- [ ] **Step 5: Run focused native verification**

```bash
rtk pnpm verify:native:fast -- frame_compositor::gpu::tests::gpu_matches_canonical_cpu_blends_with_documented_tolerance
```

Expected: default workspace check and the exact focused test PASS, the metrics report names lane `fast`, `CARGO_INCREMENTAL` is `0`, and only `src-tauri/target/verify` grows.

- [ ] **Step 6: Run cold complete verification**

If `src-tauri/target/verify` is already non-empty, invoke only the explicit verification-cache cleanup path after its exact target is printed and validated. Do not clean the development target.

Run:

```bash
rtk pnpm verify
```

Copy the resulting latest metrics report to:

```text
output/build-metrics/native-verification-cold.json
```

Expected: complete frontend and native release gates PASS.

- [ ] **Step 7: Run unchanged warm complete verification**

Run the same command without source edits:

```bash
rtk pnpm verify
```

Copy the report to:

```text
output/build-metrics/native-verification-warm.json
```

Compare elapsed time, free space, development target size, verification target size, and cache classification. Report measured results without promising a threshold.

- [ ] **Step 8: Confirm cache enforcement**

Verify from reports and filesystem inspection:

- the development target was not automatically removed;
- verification incremental compilation stayed disabled;
- the verification target remained at or below 24 GiB or was automatically removed;
- at least 20 GiB free was required before native work;
- cleanup target, reason, and before/after bytes are recorded.

- [ ] **Step 9: Request code review**

Apply `superpowers:requesting-code-review` and review:

- approved-spec coverage;
- transport-boundary leaks;
- fixture isolation;
- browser and desktop evidence separation;
- cross-platform path and process handling;
- destructive cleanup safety;
- verification profile completeness.

Address findings with focused tests and a Conventional Commit.

- [ ] **Step 10: Run final verification after review fixes**

```bash
rtk git diff --check
rtk pnpm verify:frontend
rtk node --test scripts/cargo-cache-policy.test.ts scripts/native-verification-policy.test.ts scripts/native-rust-test-policy.test.ts scripts/zero-debt-gate.test.ts
rtk pnpm verify
rtk git status --short
```

Expected: every gate passes and the worktree is clean after the final commit.

- [ ] **Step 11: Commit any evidence-driven fixes**

List `rtk git status --short`, stage each reviewed source or documentation path
explicitly with `rtk git add`, and then run:

```bash
rtk git commit -m "fix(runtime): address boundary verification findings"
```

Do not commit generated output unless repository policy explicitly tracks that evidence.
