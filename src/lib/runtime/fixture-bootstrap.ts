import { settingsVisualQaOperations, settingsVisualQaFixtures } from "../settings-visual-qa-fixtures";
import { FixtureTransport, type FixtureOperationHandler } from "./adapters/fixture-transport";
import { fixtureExportDirectoryChooserOperation, fixtureMediaChooserOperation } from "./adapters/tauri-dialog";
import { BackendUnavailableError } from "./backend-transport";
import type { EditorFixtureRuntimeMarker } from "./bootstrap";
import { fixtureDomainOperations } from "./fixtures";
import { defaultHostPlatform } from "./platform";

/** How long a fixture AI turn works before answering, so the progress row is visible. */
const conversationTurnDelayMs = 600;

/**
 * Where the browser fixture keeps preference edits. Session storage, so a reload
 * sees them exactly as the desktop store would and a fresh context does not.
 */
const fixturePreferencesKey = "video-creater.fixture.appPreferences.v1";

const recognizedUnavailableOperations = [
  "get_provider_account_status",
  "list_shader_background_templates",
  "list_visual_effect_catalog",
  "materialize_sample_project_media",
  "save_split_project_to_folder",
  "load_split_project_from_folder",
  "import_media_to_project",
  "prepare_project_preview",
  "validate_split_project_folder",
  "search_project_media",
  "rebuild_project_search_index",
  "get_temporal_worker_environment_report",
  "start_temporal_workflow",
  "run_generate_media_in_process",
  "start_codex_video_edit_for_project",
  // The AI tab shows its missing-agent state, as in a browser without the desktop backend.
  "start_codex_conversation_edit_for_project",
  // Startup model readiness; a settings fixture answers these with its model statuses.
  "list_transcription_models",
  "get_active_transcription_model",
  "get_transcription_runtime_status",
  "get_production_speech_model_status",
  // The fixture transcribes through its Temporal worker stand-in, the speech service's fallback.
  "run_transcribe_media_in_process",
  // Folder-backed fixture samples (tasks, export) have no saved chats to list.
  "load_agent_sessions_from_split_project_folder",
  "load_app_server_conversation_history_from_split_project_folder",
  // Media → Import reads as "needs the desktop app" unless the acceptance fixture chooses a file.
  fixtureMediaChooserOperation,
  // Export → Choose export folder reads as "needs the desktop app" unless the export fixture chooses one.
  fixtureExportDirectoryChooserOperation,
] as const;

function unavailable(): never {
  throw new BackendUnavailableError();
}

/** The seeded preferences plus whatever this browser session has since accepted. */
function fixturePreferences(seed: unknown) {
  const storage = typeof window === "undefined" ? null : window.sessionStorage;
  const base = isRecord(seed) ? seed : {};
  let accepted: Record<string, unknown> = { ...base, ...readSavedPreferences(storage) };
  return {
    read: () => accepted,
    write(patch: Record<string, unknown>) {
      accepted = { ...accepted, ...patch };
      try {
        storage?.setItem(fixturePreferencesKey, JSON.stringify(accepted));
      } catch {
        // A storage-less browser still gets the in-memory value for this page.
      }
      return accepted;
    },
  };
}

function readSavedPreferences(storage: Storage | null): Record<string, unknown> {
  try {
    const raw = storage?.getItem(fixturePreferencesKey);
    const parsed: unknown = raw ? JSON.parse(raw) : null;
    return isRecord(parsed) ? parsed : {};
  } catch {
    return {};
  }
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}

export async function createFixtureTransport(
  marker: EditorFixtureRuntimeMarker,
): Promise<FixtureTransport> {
  const operations = new Map<string, FixtureOperationHandler>(
    recognizedUnavailableOperations.map((operation) => [operation, unavailable]),
  );
  operations.set(
    "get_export_profile_availability_report",
    () => marker.exportCapabilities ?? [],
  );
  operations.set("sync_native_menu_state", () => undefined);
  operations.set("get_platform_info", () => ({
    platform: marker.platform ?? defaultHostPlatform,
  }));

  const fixture = marker.settingsFixtureId
    ? settingsVisualQaFixtures[
        marker.settingsFixtureId as keyof typeof settingsVisualQaFixtures
      ]
    : undefined;
  if (fixture) {
    for (const [operation, handler] of settingsVisualQaOperations(fixture)) {
      operations.set(operation, handler);
    }
  }

  // After the settings fixture, whose preferences are the seed when it has them.
  const preferences = fixturePreferences(fixture?.appPreferences ?? marker.preferences);
  operations.set("get_app_preferences", () => preferences.read());
  operations.set("update_app_preferences", (input) =>
    preferences.write((input as { patch?: Record<string, unknown> } | undefined)?.patch ?? {}),
  );

  for (const [operation, handler] of fixtureDomainOperations(marker, { turnDelayMs: conversationTurnDelayMs })) {
    operations.set(operation, handler);
  }

  const transport = new FixtureTransport(operations);
  window.__EDITOR_FIXTURE_DRIVER__ = {
    emit(event, payload) {
      transport.emitForVisualQa(event, payload);
    },
  };
  return transport;
}
