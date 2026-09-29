import { afterEach, describe, expect, it, vi } from "vitest";
import { createElement, StrictMode, useEffect } from "react";
import { render, waitFor } from "@testing-library/react";

import {
  appSettingsStorageKey,
  generationExecutionModeForModel,
  loadAppPreferences,
  updateAppPreferences,
  updateAppPreferencesFromIntent,
} from "./app-settings";

const mockInvoke = vi.hoisted(() => vi.fn());

vi.mock("@/lib/runtime/backend-client", () => ({
  backendRequest: mockInvoke,
}));

function appPreferences(
  overrides: Partial<{
    schemaVersion: 2;
    projectLocation:
      | { mode: "ask" }
      | { mode: "suggestedParent"; parentPath: string };
    requireProviderUploadConfirmation: boolean;
    renderCompletionNotifications: boolean;
    newProjectDefaults: {
      width: number;
      height: number;
      fps: number;
      loudnessLufs: number;
      captions: "burn_in" | "mux" | "off";
    };
    enabledGenerationModelIds: string[];
    generationExecutionBackend: "inProcess" | "temporal";
  }> = {},
) {
  return {
    schemaVersion: 2 as const,
    projectLocation: { mode: "ask" as const },
    requireProviderUploadConfirmation: true,
    renderCompletionNotifications: false,
    newProjectDefaults: {
      width: 1920,
      height: 1080,
      fps: 30,
      loudnessLufs: -14,
      captions: "burn_in" as const,
    },
    enabledGenerationModelIds: [],
    generationExecutionBackend: "inProcess" as const,
    ...overrides,
  };
}

function deferred<T>() {
  let resolve!: (value: T | PromiseLike<T>) => void;
  let reject!: (reason?: unknown) => void;
  const promise = new Promise<T>((nextResolve, nextReject) => {
    resolve = nextResolve;
    reject = nextReject;
  });
  return { promise, resolve, reject };
}

function AppPreferencesBootstrapProbe() {
  useEffect(() => {
    void loadAppPreferences().catch(() => undefined);
  }, []);
  return null;
}

afterEach(() => {
  mockInvoke.mockReset();
  window.localStorage.clear();
});

describe("native app preferences", () => {
  it("shares one in-flight legacy handoff across a StrictMode bootstrap", async () => {
    const request = deferred<ReturnType<typeof appPreferences>>();
    mockInvoke.mockReturnValue(request.promise);

    render(
      createElement(
        StrictMode,
        null,
        createElement(AppPreferencesBootstrapProbe),
      ),
    );

    await waitFor(() => expect(mockInvoke).toHaveBeenCalledTimes(1));
    request.resolve(appPreferences());
    await request.promise;
  });

  it("allows one fresh StrictMode bootstrap after a rejected handoff", async () => {
    mockInvoke.mockRejectedValue(new Error("preferences unavailable"));
    const first = render(
      createElement(
        StrictMode,
        null,
        createElement(AppPreferencesBootstrapProbe),
      ),
    );
    await waitFor(() => expect(mockInvoke).toHaveBeenCalledTimes(1));
    first.unmount();

    mockInvoke.mockReset();
    mockInvoke.mockResolvedValue(appPreferences());
    render(
      createElement(
        StrictMode,
        null,
        createElement(AppPreferencesBootstrapProbe),
      ),
    );

    await waitFor(() => expect(mockInvoke).toHaveBeenCalledTimes(1));
  });

  it("does not expose a settings-level credential environment contract", async () => {
    const module = await import("./app-settings");
    const obsoleteCredentialKey = ["providerCredential", "EnvVar"].join("");

    expect(module.defaultAppPreferences).not.toHaveProperty(
      obsoleteCredentialKey,
    );
    expect(module).not.toHaveProperty(
      `${obsoleteCredentialKey}ForGenerationModel`,
    );
  });

  it("sends legacy v1 once without secrets and accepts the Rust v2 authority", async () => {
    const obsoleteCredentialKey = ["providerCredential", "EnvVar"].join("");
    window.localStorage.setItem(
      appSettingsStorageKey,
      JSON.stringify({
        [obsoleteCredentialKey]: "FAL_KEY",
        disabledGenerationModelIds: ["openai:gpt-image-2"],
        renderCompletionNotifications: true,
      }),
    );
    mockInvoke.mockResolvedValueOnce(
      appPreferences({ renderCompletionNotifications: true }),
    );

    await expect(loadAppPreferences()).resolves.toMatchObject({
      schemaVersion: 2,
      renderCompletionNotifications: true,
      enabledGenerationModelIds: [],
    });
    expect(mockInvoke).toHaveBeenCalledWith("get_app_preferences", {
      legacy: expect.not.objectContaining({
        [obsoleteCredentialKey]: expect.anything(),
        disabledGenerationModelIds: expect.anything(),
      }),
    });
    expect(window.localStorage.getItem(appSettingsStorageKey)).toBeNull();
  });

  it("hands off only valid non-secret legacy preferences", async () => {
    window.localStorage.setItem(
      appSettingsStorageKey,
      JSON.stringify({
        defaultProjectStorageLocation: "/Volumes/Studio/Cuts",
        requireProviderUploadConfirmation: false,
        renderCompletionNotifications: true,
        generationExecutionBackend: "temporal",
        updatePolicy: "notify",
      }),
    );
    const accepted = appPreferences({
      projectLocation: {
        mode: "suggestedParent",
        parentPath: "/Volumes/Studio/Cuts",
      },
      requireProviderUploadConfirmation: false,
      renderCompletionNotifications: true,
      generationExecutionBackend: "temporal",
    });
    mockInvoke.mockResolvedValueOnce(accepted);

    await expect(loadAppPreferences()).resolves.toEqual(accepted);
    expect(mockInvoke).toHaveBeenCalledWith("get_app_preferences", {
      legacy: {
        projectLocation: {
          mode: "suggestedParent",
          parentPath: "/Volumes/Studio/Cuts",
        },
        requireProviderUploadConfirmation: false,
        renderCompletionNotifications: true,
        generationExecutionBackend: "temporal",
      },
    });
  });

  it("keeps legacy data for a retry when the native authority rejects loading", async () => {
    window.localStorage.setItem(
      appSettingsStorageKey,
      JSON.stringify({ renderCompletionNotifications: true }),
    );
    mockInvoke.mockRejectedValueOnce(new Error("preferences unavailable"));

    await expect(loadAppPreferences()).rejects.toThrow("preferences unavailable");

    expect(window.localStorage.getItem(appSettingsStorageKey)).not.toBeNull();
  });

  it("updates through Rust and returns the accepted preference record", async () => {
    const accepted = appPreferences({ renderCompletionNotifications: true });
    mockInvoke.mockResolvedValueOnce(accepted);

    await expect(
      updateAppPreferences({ renderCompletionNotifications: true }),
    ).resolves.toEqual(accepted);
    expect(mockInvoke).toHaveBeenCalledWith("update_app_preferences", {
      patch: { renderCompletionNotifications: true },
    });
    expect(window.localStorage.getItem(appSettingsStorageKey)).toBeNull();
  });

  it("serializes field intents and rebases nested Rust patches on acceptance", async () => {
    mockInvoke.mockResolvedValueOnce(appPreferences());
    await loadAppPreferences();
    mockInvoke.mockReset();
    const first = deferred<ReturnType<typeof appPreferences>>();
    const second = deferred<ReturnType<typeof appPreferences>>();
    mockInvoke
      .mockReturnValueOnce(first.promise)
      .mockReturnValueOnce(second.promise);

    const fpsUpdate = updateAppPreferencesFromIntent({
      newProjectDefaults: { fps: 24 },
    });
    const captionsUpdate = updateAppPreferencesFromIntent({
      newProjectDefaults: { captions: "mux" },
    });

    await waitFor(() => expect(mockInvoke).toHaveBeenCalledTimes(1));
    first.resolve(
      appPreferences({
        newProjectDefaults: {
          ...appPreferences().newProjectDefaults,
          fps: 24,
        },
      }),
    );
    await waitFor(() => expect(mockInvoke).toHaveBeenCalledTimes(2));
    expect(mockInvoke).toHaveBeenNthCalledWith(2, "update_app_preferences", {
      patch: {
        newProjectDefaults: {
          ...appPreferences().newProjectDefaults,
          fps: 24,
          captions: "mux",
        },
      },
    });
    second.resolve(
      appPreferences({
        newProjectDefaults: {
          ...appPreferences().newProjectDefaults,
          fps: 24,
          captions: "mux",
        },
      }),
    );
    await expect(fpsUpdate).resolves.toMatchObject({
      newProjectDefaults: { fps: 24 },
    });
    await expect(captionsUpdate).resolves.toMatchObject({
      newProjectDefaults: { fps: 24, captions: "mux" },
    });
  });

  it("does not resubmit a rejected nested field with the next intent", async () => {
    mockInvoke.mockResolvedValueOnce(appPreferences());
    await loadAppPreferences();
    mockInvoke.mockReset();
    const first = deferred<ReturnType<typeof appPreferences>>();
    mockInvoke
      .mockReturnValueOnce(first.promise)
      .mockResolvedValueOnce(
        appPreferences({
          newProjectDefaults: {
            ...appPreferences().newProjectDefaults,
            captions: "mux",
          },
        }),
      );

    const rejected = updateAppPreferencesFromIntent({
      newProjectDefaults: { fps: 24 },
    });
    const accepted = updateAppPreferencesFromIntent({
      newProjectDefaults: { captions: "mux" },
    });
    const rejection = expect(rejected).rejects.toThrow("frame rate rejected");
    await waitFor(() => expect(mockInvoke).toHaveBeenCalledTimes(1));
    first.reject(new Error("frame rate rejected"));

    await rejection;
    await expect(accepted).resolves.toMatchObject({
      newProjectDefaults: { fps: 30, captions: "mux" },
    });
    expect(mockInvoke).toHaveBeenNthCalledWith(2, "update_app_preferences", {
      patch: {
        newProjectDefaults: {
          ...appPreferences().newProjectDefaults,
          captions: "mux",
        },
      },
    });
  });
});

describe("generationExecutionModeForModel", () => {
  it("keeps the built-in mock provider on deterministic local execution", () => {
    expect(
      generationExecutionModeForModel({ provider: " mock ", id: "mock-video-v1" }),
    ).toBe("mock");
  });

  it("uses live execution for configured BYOK providers", () => {
    expect(
      generationExecutionModeForModel({ provider: "replicate", id: "flux-schnell" }),
    ).toBe("live");
  });
});
