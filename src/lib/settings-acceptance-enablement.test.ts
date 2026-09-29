import { describe, expect, it, vi } from "vitest";

import { runPackagedSettingsAcceptanceIfEnabled } from "./settings-acceptance-enablement";

function bridge(context: unknown) {
  return {
    invoke: vi.fn().mockResolvedValue(context),
    emit: vi.fn().mockResolvedValue(undefined),
    document,
    sleep: vi.fn().mockResolvedValue(undefined),
  };
}

describe("runPackagedSettingsAcceptanceIfEnabled", () => {
  it("does not load the acceptance implementation for a normal launch", async () => {
    const loadRunner = vi.fn();

    await runPackagedSettingsAcceptanceIfEnabled(bridge(null), loadRunner);

    expect(loadRunner).not.toHaveBeenCalled();
  });

  it("loads the acceptance implementation after enablement is detected", async () => {
    const runtimeBridge = bridge({
      stage: "pre_restart",
      projectRoot: "/private/tmp/video-creater-settings-acceptance-test/projects/acceptance-project",
    });
    const runSettingsAcceptanceIfEnabled = vi.fn().mockResolvedValue(undefined);
    const loadRunner = vi.fn().mockResolvedValue({ runSettingsAcceptanceIfEnabled });

    await runPackagedSettingsAcceptanceIfEnabled(runtimeBridge, loadRunner);

    expect(runSettingsAcceptanceIfEnabled).toHaveBeenCalledWith(runtimeBridge);
  });
});
