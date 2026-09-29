import { FixtureTransport } from "./adapters/fixture-transport";
import { TauriTransport } from "./adapters/tauri-transport";
import { RemoteTransport } from "./adapters/remote-transport";
import { backendClient } from "./backend-client";
import { loadHostPlatform, type HostPlatform } from "./platform";
import {
  createRuntimeDescriptor,
  type RuntimeDescriptor,
} from "./runtime-descriptor";
import { fixtureRuntimeMarkerEnabled, installRuntimeMode } from "./runtime-mode";
import { discoverRemoteSession } from "./remote-session";

export interface EditorFixtureRuntimeMarker {
  readonly enabled: true;
  readonly settingsFixtureId?: string;
  readonly preferences?: unknown;
  readonly exportCapabilities?: readonly unknown[];
  /** Adds the DEV-only handlers the editor panel flows need (effect catalog, sample silences). */
  readonly panelFixtures?: boolean;
  /**
   * Answers AI conversation turns by keyword ("tighten" a safe cut, "generate" a review bundle,
   * "fail" a validation failure, anything else a safe caption fix) and applies, undoes, captures
   * result frames and keeps chats against the committed, folder-backed sample.
   */
  readonly conversationFixture?: boolean;
  /** Seeds background tasks into the sample project and answers the task details commands. */
  readonly tasksFixture?: boolean;
  /**
   * Answers the export commands: a capability report, a render that completes over three folder
   * reloads, and XML and package exports. Includes the task handlers, seeded only with `tasksFixture`.
   */
  readonly exportFixture?: boolean;
  /**
   * Every handler the editor acceptance flows reach, over one folder-backed sample that starts
   * untranscribed: implies `panelFixtures`, `conversationFixture` and `exportFixture` (tasks seeded
   * only with `tasksFixture`), and adds media import through a fixture chooser, preview, filmstrip,
   * background and search commands, transcription with speakers, and media generation.
   */
  readonly acceptanceFixture?: boolean;
  /** Host platform presented by the fixture backend; defaults to macOS. */
  readonly platform?: HostPlatform;
}

declare global {
  interface Window {
    __EDITOR_FIXTURE_RUNTIME__?: EditorFixtureRuntimeMarker;
    __EDITOR_FIXTURE_DRIVER__?: {
      emit(event: string, payload: unknown): void;
    };
    /** Reveal requests the tasks (and export) fixture accepted, for e2e assertions on "Show in folder". */
    __EDITOR_FIXTURE_REVEALS__?: { projectDir: string; artifactPath: string }[];
    __TAURI_INTERNALS__?: unknown;
  }
}

export interface RuntimeBootstrapInput {
  readonly fixtureTransport: FixtureTransport | null;
  readonly tauriAvailable: boolean;
}

export function selectRuntime(
  input: RuntimeBootstrapInput,
): RuntimeDescriptor {
  if (input.fixtureTransport) {
    return createRuntimeDescriptor("fixture", {
      status: "connected",
      transport: input.fixtureTransport,
    });
  }
  if (input.tauriAvailable) {
    return createRuntimeDescriptor("desktop", {
      status: "connected",
      transport: new TauriTransport(),
    });
  }
  return createRuntimeDescriptor("browser", { status: "disconnected" });
}

async function explicitFixtureTransport(): Promise<FixtureTransport | null> {
  // Keep the DEV guard inline so production builds drop the fixture bootstrap chunk.
  if (!import.meta.env.DEV || !fixtureRuntimeMarkerEnabled()) return null;
  const marker = window.__EDITOR_FIXTURE_RUNTIME__;
  if (!marker) return null;
  const { createFixtureTransport } = await import("./fixture-bootstrap");
  return createFixtureTransport(marker);
}

export async function bootstrapRuntime(): Promise<RuntimeDescriptor> {
  let runtime = selectRuntime({
    fixtureTransport: await explicitFixtureTransport(),
    tauriAvailable:
      typeof window !== "undefined" && window.__TAURI_INTERNALS__ !== undefined,
  });
  if (runtime.mode === "browser") {
    const session = await discoverRemoteSession();
    runtime = session.kind === "connected"
      ? createRuntimeDescriptor("browser", {
          status: "connected",
          transport: new RemoteTransport({ csrfToken: session.csrfToken }),
        }, session)
      : createRuntimeDescriptor("browser", { status: "disconnected" }, session);
  }
  backendClient.install(runtime.connection);
  installRuntimeMode(runtime.mode);
  if (runtime.connection.status === "connected") {
    await loadHostPlatform();
  }
  return runtime;
}
