import { invoke } from "@tauri-apps/api/core";
import { emit } from "@tauri-apps/api/event";

import type { SettingsAcceptanceBridge } from "../../settings-acceptance-runner";

export function createTauriSettingsAcceptanceBridge(): SettingsAcceptanceBridge {
  return {
    invoke: (operation, input) => invoke(operation, input),
    emit: (event, payload) => emit(event, payload),
    document,
    sleep: (milliseconds) =>
      new Promise((resolve) => window.setTimeout(resolve, milliseconds)),
  };
}
