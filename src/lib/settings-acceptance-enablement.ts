import type { SettingsAcceptanceBridge } from "./settings-acceptance-runner";

type AcceptanceRunnerModule = Pick<
  typeof import("./settings-acceptance-runner"),
  "runSettingsAcceptanceIfEnabled"
>;

type AcceptanceRunnerLoader = () => Promise<AcceptanceRunnerModule>;

function hasPackagedAcceptanceContext(value: unknown): boolean {
  if (typeof value !== "object" || value === null || Array.isArray(value)) {
    return false;
  }
  const context = value as { stage?: unknown; projectRoot?: unknown };
  return (context.stage === "pre_restart" || context.stage === "post_restart") &&
    typeof context.projectRoot === "string";
}

export async function runPackagedSettingsAcceptanceIfEnabled(
  bridge: SettingsAcceptanceBridge,
  loadRunner: AcceptanceRunnerLoader = () => import("./settings-acceptance-runner"),
): Promise<void> {
  const context = await bridge.invoke("get_settings_acceptance_context");
  if (!hasPackagedAcceptanceContext(context)) return;

  const { runSettingsAcceptanceIfEnabled } = await loadRunner();
  await runSettingsAcceptanceIfEnabled(bridge);
}
