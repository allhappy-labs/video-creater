import type { FixtureOperationHandler } from "../adapters/fixture-transport";
import { conversationFixtureOperations } from "./conversation-fixtures";
import { exportFixtureOperations } from "./export-fixtures";
import { createFixtureProjectStore } from "./fixture-project-store";
import { generationFixtureOperations } from "./generation-fixtures";
import { panelFixtureOperations, withSampleSilences } from "./panel-fixtures";
import { projectFixtureOperations } from "./project-fixtures";
import { speechFixtureOperations, untranscribedSample } from "./speech-fixtures";
import { taskFixtureOperations } from "./task-fixtures";

/**
 * Registers the DEV-only fixture domains the window marker asks for (see `EditorFixtureRuntimeMarker`).
 * Every stateful domain reads and writes one `FixtureProjectStore`:
 *
 * - `panelFixtures`: the effect catalog and the sample's detected silences.
 * - `conversationFixture`, `tasksFixture`, `exportFixture`: their domain handlers plus the folder core
 *   (save, reload, action writes), so the sample opens folder backed. Tasks are seeded only with
 *   `tasksFixture`.
 * - `acceptanceFixture`: everything the editor acceptance flows reach — panels, conversation, export
 *   and tasks (unseeded), every project folder command (import, preview, filmstrip, backgrounds,
 *   search), speech and generation — over the sample as it was before transcription.
 */

export interface FixtureDomainFlags {
  readonly panelFixtures?: boolean;
  readonly conversationFixture?: boolean;
  readonly tasksFixture?: boolean;
  readonly exportFixture?: boolean;
  readonly acceptanceFixture?: boolean;
}

interface FixtureDomainOptions {
  /** How long a fixture AI turn works before answering. */
  readonly turnDelayMs: number;
}

/** The project commands a folder-backed sample needs even without the acceptance fixture. */
const folderCoreOperations: ReadonlySet<string> = new Set([
  "save_split_project_to_folder",
  "load_split_project_from_folder",
  "apply_project_actions_to_split_project_folder",
  "apply_project_action_to_split_project_folder",
]);

export function fixtureDomainOperations(flags: FixtureDomainFlags, options: FixtureDomainOptions): ReadonlyMap<string, FixtureOperationHandler> {
  const acceptance = flags.acceptanceFixture === true;
  const panels = acceptance || flags.panelFixtures === true;
  const conversation = acceptance || flags.conversationFixture === true;
  const exports = acceptance || flags.exportFixture === true;
  const tasks = flags.tasksFixture === true;
  const operations = new Map<string, FixtureOperationHandler>();
  const register = (handlers: ReadonlyMap<string, FixtureOperationHandler>, include: (operation: string) => boolean = () => true) => {
    for (const [operation, handler] of handlers) if (include(operation)) operations.set(operation, handler);
  };

  if (!conversation && !exports && !tasks) {
    if (panels) register(panelFixtureOperations());
    return operations;
  }

  const store = createFixtureProjectStore();
  if (panels) {
    store.addSeed(withSampleSilences);
    register(panelFixtureOperations(), (operation) => operation !== "save_split_project_to_folder");
  }
  register(projectFixtureOperations(store), (operation) => acceptance || folderCoreOperations.has(operation));
  register(exports ? exportFixtureOperations(store, { seedTasks: tasks }) : taskFixtureOperations(store, { seed: tasks }));
  if (conversation) register(conversationFixtureOperations({ store, turnDelayMs: options.turnDelayMs }));
  if (acceptance) {
    store.addSeed(untranscribedSample);
    register(speechFixtureOperations(store));
    register(generationFixtureOperations(store));
  }
  return operations;
}
