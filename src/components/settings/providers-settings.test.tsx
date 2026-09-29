import "@testing-library/jest-dom/vitest";
import { StrictMode } from "react";
import { act, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

import {
  deleteProviderCredential,
  listProviderCredentialStatuses,
  setProviderCredential,
} from "@/lib/provider-credentials";
import {
  getProviderHealth,
  refreshProviderHealth,
  type ProviderHealth,
} from "@/lib/settings/providers";
import type { SettingsOperation } from "@/lib/settings/operations";
import { requiredAt } from "@/test-utils/required";
import { defaultHostPlatform, installHostPlatform } from "@/lib/runtime/platform";
import { ProvidersSettings } from "./providers-settings";

vi.mock("@/lib/provider-credentials", () => ({
  deleteProviderCredential: vi.fn(),
  listProviderCredentialStatuses: vi.fn(),
  setProviderCredential: vi.fn(),
}));

vi.mock("@/lib/settings/providers", () => ({
  getProviderHealth: vi.fn(),
  providerIntegrationGroup: (
    { configured }: { configured: boolean },
    credentialStatus?: { configured: boolean } | null,
  ) => (credentialStatus?.configured ?? configured) ? "configured" : "available",
  refreshProviderHealth: vi.fn(),
}));

const mockGetProviderHealth = vi.mocked(getProviderHealth);
const mockRefreshProviderHealth = vi.mocked(refreshProviderHealth);
const mockListProviderCredentialStatuses = vi.mocked(listProviderCredentialStatuses);
const mockSetProviderCredential = vi.mocked(setProviderCredential);
const mockDeleteProviderCredential = vi.mocked(deleteProviderCredential);

const health: ProviderHealth[] = [
  {
    provider: "fal.ai",
    displayName: "fal.ai",
    credentialSource: "keychain",
    configured: true,
    validationState: "available",
    accountLabel: "Studio account",
    balanceLabel: "$24.50",
    dependentModelIds: ["fal.ai:flux-pro"],
    lastCheckedAt: "2026-07-17T09:00:00Z",
    diagnosticCode: null,
  },
  {
    provider: "openai",
    displayName: "OpenAI",
    credentialSource: "missing",
    configured: false,
    validationState: "missing",
    accountLabel: null,
    balanceLabel: null,
    dependentModelIds: ["openai:gpt-image-1"],
    lastCheckedAt: "2026-07-17T09:00:00Z",
    diagnosticCode: "providers.credentialMissing",
  },
  {
    provider: "replicate",
    displayName: "Replicate",
    credentialSource: "missing",
    configured: false,
    validationState: "missing",
    accountLabel: null,
    balanceLabel: null,
    dependentModelIds: [],
    lastCheckedAt: null,
    diagnosticCode: null,
  },
  {
    provider: "google",
    displayName: "Google",
    credentialSource: "keychain",
    configured: true,
    validationState: "rejected",
    accountLabel: null,
    balanceLabel: null,
    dependentModelIds: ["google:veo-3"],
    lastCheckedAt: "2026-07-17T09:00:00Z",
    diagnosticCode: "providers.credentialRejected",
  },
  {
    provider: "xai",
    displayName: "xAI",
    credentialSource: "keychain",
    configured: true,
    validationState: "unavailable",
    accountLabel: null,
    balanceLabel: null,
    dependentModelIds: [],
    lastCheckedAt: "2026-07-17T09:00:00Z",
    diagnosticCode: "providers.validationUnavailable",
  },
];

const falHealth = requiredAt(health, 0, "fal.ai health fixture");
const openAiHealth = requiredAt(health, 1, "OpenAI health fixture");
const replicateHealth = requiredAt(health, 2, "Replicate health fixture");
const googleHealth = requiredAt(health, 3, "Google health fixture");

function operation(
  id: string,
  targetId: string,
  state: SettingsOperation["state"],
): SettingsOperation {
  return {
    id,
    kind: "providerRefresh",
    targetId,
    phase: state,
    state,
    completedUnits: state === "succeeded" ? 1 : 0,
    totalUnits: 1,
    unit: "providers",
    cancellable: false,
    message: `Provider refresh ${state}.`,
    error: null,
    startedAt: "2026-07-17T09:00:00Z",
    updatedAt: `2026-07-17T09:00:0${state === "succeeded" ? "2" : "1"}Z`,
  };
}

function deferred<T>() {
  let resolve!: (value: T | PromiseLike<T>) => void;
  let reject!: (reason?: unknown) => void;
  const promise = new Promise<T>((resolvePromise, rejectPromise) => {
    resolve = resolvePromise;
    reject = rejectPromise;
  });
  return { promise, reject, resolve };
}

const generationModels = [
  { provider: "fal.ai", id: "flux-pro", displayName: "Flux Pro", kind: "image" as const },
  { provider: "openai", id: "gpt-image-1", displayName: "GPT Image 1", kind: "image" as const },
  { provider: "google", id: "veo-3", displayName: "Veo 3", kind: "video" as const },
];

function renderProviders(
  overrides: Partial<React.ComponentProps<typeof ProvidersSettings>> = {},
) {
  const props: React.ComponentProps<typeof ProvidersSettings> = {
    disabledGenerationModelIds: [],
    generationModels,
    operations: [],
    ...overrides,
  };
  return { props, ...render(<ProvidersSettings {...props} />) };
}

describe("ProvidersSettings", () => {
  beforeEach(() => {
    mockGetProviderHealth.mockReset();
    mockGetProviderHealth.mockResolvedValue(health);
    mockRefreshProviderHealth.mockReset();
    mockRefreshProviderHealth.mockResolvedValue(operation("refresh-1", "providers", "queued"));
    mockListProviderCredentialStatuses.mockReset();
    mockListProviderCredentialStatuses.mockResolvedValue([
      { provider: "fal.ai", displayName: "fal.ai", configured: true, source: "keychain" },
      { provider: "openai", displayName: "OpenAI", configured: false, source: "missing" },
      { provider: "replicate", displayName: "Replicate", configured: false, source: "missing" },
      { provider: "google", displayName: "Google", configured: true, source: "keychain" },
      { provider: "xai", displayName: "xAI", configured: true, source: "keychain" },
    ]);
    mockSetProviderCredential.mockReset();
    mockSetProviderCredential.mockResolvedValue({
      provider: "openai",
      displayName: "OpenAI",
      configured: true,
      source: "keychain",
    });
    mockDeleteProviderCredential.mockReset();
    mockDeleteProviderCredential.mockResolvedValue({
      provider: "fal.ai",
      displayName: "fal.ai",
      configured: false,
      source: "missing",
    });
  });

  it("does not duplicate generation model selection in Integrations", async () => {
    renderProviders();

    expect(
      await screen.findByRole("button", { name: "Refresh all providers" }),
    ).toBeInTheDocument();
    expect(
      screen.queryByRole("region", { name: "Generation model dependencies" }),
    ).not.toBeInTheDocument();
    expect(
      screen.queryByLabelText("Enable generation model GPT Image 1"),
    ).not.toBeInTheDocument();
    expect(
      screen.queryByLabelText("Generation execution backend"),
    ).not.toBeInTheDocument();
  });

  it("treats an untouched provider as available rather than failed", async () => {
    mockGetProviderHealth.mockResolvedValueOnce([
      {
        ...openAiHealth,
        dependentModelIds: [],
        diagnosticCode: "providers.credentialMissing",
      },
    ]);

    renderProviders();

    expect(
      await screen.findByRole("heading", { name: "Available integrations" }),
    ).toBeInTheDocument();
    expect(
      screen.getByRole("button", { name: "Connect OpenAI" }),
    ).toBeInTheDocument();
    expect(
      screen.queryByText(/Needs attention|environment|OPENAI_API_KEY/i),
    ).not.toBeInTheDocument();
  });

  it("warns before disconnecting a provider used by enabled models", async () => {
    mockGetProviderHealth.mockResolvedValueOnce([
      {
        ...openAiHealth,
        credentialSource: "keychain",
        configured: true,
        validationState: "available",
        dependentModelIds: ["openai:gpt-image-2"],
        diagnosticCode: null,
      },
    ]);
    mockListProviderCredentialStatuses.mockResolvedValueOnce([
      {
        provider: "openai",
        displayName: "OpenAI",
        configured: true,
        source: "keychain",
      },
    ]);

    renderProviders({
      generationModels: [
        {
          provider: "openai",
          id: "gpt-image-2",
          displayName: "GPT-image-2",
          kind: "image",
        },
      ],
    });

    const trigger = await screen.findByRole("button", {
      name: "Disconnect OpenAI",
    });
    trigger.focus();
    fireEvent.click(trigger);

    const dialog = screen.getByRole("dialog", { name: "Disconnect OpenAI" });
    const cancel = within(dialog).getByRole("button", { name: "Cancel" });
    const confirm = within(dialog).getByRole("button", {
      name: "Disconnect OpenAI",
    });
    expect(dialog).toHaveTextContent("GPT-image-2 will become unavailable");
    expect(cancel).toHaveFocus();
    fireEvent.keyDown(dialog, { key: "Tab" });
    expect(confirm).toHaveFocus();
    fireEvent.keyDown(dialog, { key: "Tab", shiftKey: true });
    expect(cancel).toHaveFocus();
    fireEvent.keyDown(dialog, { key: "Escape" });
    await waitFor(() => expect(trigger).toHaveFocus());
    expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
  });

  it("describes the Linux system keyring instead of macOS Keychain", async () => {
    installHostPlatform("linux");
    try {
      renderProviders();

      const configured = await screen.findByRole("region", { name: "Configured integrations" });
      expect(within(configured).getByRole("form", { name: "fal.ai credential" })).toHaveTextContent(
        "Saved in system keyring",
      );
      expect(screen.getByText("Keys are write-only and stay in the system keyring.")).toBeInTheDocument();
      expect(document.body).not.toHaveTextContent(/macOS|Keychain|this Mac/);
      const fal = within(configured).getByRole("form", { name: "fal.ai credential" });
      fireEvent.click(within(fal).getByRole("button", { name: "Disconnect fal.ai" }));
      expect(screen.getByRole("dialog", { name: "Disconnect fal.ai" })).toHaveTextContent(
        "The credential will be removed from the system keyring on this computer.",
      );
      expect(
        screen.getByTestId("providers-settings-page"),
      ).toHaveAttribute("data-settings-credential-boundary", "keychain-only");
    } finally {
      act(() => installHostPlatform(defaultHostPlatform));
    }
  });

  it("groups configured integrations first and keeps validation failures inline", async () => {
    renderProviders();

    const configured = await screen.findByRole("region", { name: "Configured integrations" });
    const available = screen.getByRole("region", { name: "Available integrations" });

    expect(within(configured).getByRole("form", { name: "fal.ai credential" })).toHaveTextContent(
      "Saved in macOS Keychain",
    );
    expect(configured).toHaveTextContent("Flux Pro");
    expect(configured).toHaveTextContent("Studio account");
    expect(configured).toHaveTextContent("$24.50");
    expect(within(configured).getByRole("form", { name: "Google credential" })).toHaveTextContent(
      "Credential rejected",
    );
    expect(within(configured).getByRole("form", { name: "xAI credential" })).toHaveTextContent(
      "Provider validation unavailable",
    );
    expect(within(available).getByRole("form", { name: "OpenAI credential" }))
      .not.toHaveTextContent(/required|failed|attention/i);
    expect(within(available).getByRole("form", { name: "Replicate credential" }))
      .toHaveTextContent("Not configured");
  });

  it("keeps integration groups and actions aligned when Keychain status diverges from cached health", async () => {
    mockGetProviderHealth.mockResolvedValueOnce([falHealth, openAiHealth]);
    mockListProviderCredentialStatuses.mockResolvedValueOnce([
      {
        provider: "fal.ai",
        displayName: "fal.ai",
        configured: false,
        source: "missing",
      },
      {
        provider: "openai",
        displayName: "OpenAI",
        configured: true,
        source: "keychain",
      },
    ]);

    renderProviders();

    const configured = await screen.findByRole("region", {
      name: "Configured integrations",
    });
    const available = screen.getByRole("region", {
      name: "Available integrations",
    });
    await waitFor(() => {
      const openAi = within(configured).getByRole("form", {
        name: "OpenAI credential",
      });
      expect(within(openAi).getByRole("button", { name: "Update OpenAI key" }))
        .toBeInTheDocument();
      expect(within(openAi).getByRole("button", { name: "Disconnect OpenAI" }))
        .toBeInTheDocument();

      const fal = within(available).getByRole("form", {
        name: "fal.ai credential",
      });
      expect(within(fal).getByRole("button", { name: "Connect fal.ai" }))
        .toBeInTheDocument();
      expect(within(fal).queryByRole("button", { name: "Disconnect fal.ai" }))
        .not.toBeInTheDocument();
    });
  });

  it("uses bounded responsive provider rows for desktop and narrow layouts", async () => {
    renderProviders();
    const page = await screen.findByTestId("providers-settings-page");
    const fal = screen.getByRole("form", { name: "fal.ai credential" });

    expect(page).toHaveClass("grid", "gap-5");
    expect(fal).toHaveClass("grid", "min-w-0", "lg:grid-cols-[minmax(11rem,0.8fr)_minmax(15rem,1.35fr)_auto]");
    expect(page.getAttribute("style") ?? "").not.toMatch(/min-width|width/);
  });

  it("never advertises an environment credential fallback", async () => {
    renderProviders();

    const google = await screen.findByRole("form", { name: "Google credential" });
    expect(google).not.toHaveTextContent(/environment fallback/i);
    expect(within(google).queryByLabelText("Environment fallback for Google")).not.toBeInTheDocument();
  });

  it("retains a rejected credential only in the password draft and never echoes it in errors", async () => {
    const rejectedDraft = "sk-rejected-private-value";
    mockSetProviderCredential.mockRejectedValue(
      new Error(`Credential ${rejectedDraft} was rejected`),
    );
    renderProviders();
    const openAi = await screen.findByRole("form", { name: "OpenAI credential" });
    const input = within(openAi).getByLabelText("New OpenAI key");

    fireEvent.change(input, { target: { value: rejectedDraft } });
    fireEvent.submit(openAi);

    await waitFor(() => expect(mockSetProviderCredential).toHaveBeenCalledWith("openai", rejectedDraft));
    expect(input).toHaveValue(rejectedDraft);
    const alert = await within(openAi).findByRole("alert");
    expect(alert).toHaveTextContent("Credential could not be saved");
    expect(alert).not.toHaveTextContent(rejectedDraft);
    expect(mockRefreshProviderHealth).not.toHaveBeenCalled();
  });

  it("guards credential mutation before React can commit the disabled state", async () => {
    mockSetProviderCredential.mockImplementation(() => new Promise(() => {}));
    renderProviders();
    const openAi = await screen.findByRole("form", { name: "OpenAI credential" });
    fireEvent.change(within(openAi).getByLabelText("New OpenAI key"), {
      target: { value: "one-private-value" },
    });

    fireEvent.submit(openAi);
    fireEvent.submit(openAi);

    expect(mockSetProviderCredential).toHaveBeenCalledTimes(1);
  });

  it("saves and removes with exact Keychain semantics then refreshes only the affected provider", async () => {
    const { rerender, props } = renderProviders();
    const openAi = await screen.findByRole("form", { name: "OpenAI credential" });
    const openAiInput = within(openAi).getByLabelText("New OpenAI key");
    fireEvent.change(openAiInput, { target: { value: "  exact-secret  " } });
    openAiInput.focus();
    fireEvent.submit(openAi);

    await waitFor(() =>
      expect(mockSetProviderCredential).toHaveBeenCalledWith("openai", "exact-secret"),
    );
    await waitFor(() =>
      expect(mockRefreshProviderHealth).toHaveBeenCalledWith("openai", []),
    );
    expect(screen.getByLabelText("New OpenAI key")).toHaveValue("");
    expect(document.activeElement).toHaveAccessibleName("New OpenAI key");

    mockGetProviderHealth.mockResolvedValue([
      ...health.filter(({ provider }) => provider !== "openai"),
      { ...openAiHealth, configured: true, credentialSource: "keychain", validationState: "available" },
    ]);
    rerender(
      <ProvidersSettings
        {...props}
        operations={[operation("refresh-1", "openai", "succeeded")]}
      />,
    );
    await waitFor(() => expect(mockGetProviderHealth).toHaveBeenCalledTimes(2));
    await waitFor(() => expect(screen.getByLabelText("New OpenAI key")).toHaveValue(""));
    await waitFor(() => expect(document.activeElement).toHaveAccessibleName("New OpenAI key"));

    const fal = screen.getByRole("form", { name: "fal.ai credential" });
    fireEvent.click(within(fal).getByRole("button", { name: "Disconnect fal.ai" }));
    const disconnect = screen.getByRole("dialog", { name: "Disconnect fal.ai" });
    fireEvent.click(within(disconnect).getByRole("button", { name: "Disconnect fal.ai" }));
    await waitFor(() => expect(mockDeleteProviderCredential).toHaveBeenCalledWith("fal.ai"));
    await waitFor(() => expect(mockRefreshProviderHealth).toHaveBeenCalledWith("fal.ai", []));
  });

  it("retries one provider, refreshes all, and handles an operation that is terminal before start resolves", async () => {
    const alreadyDone = operation("early", "google", "succeeded");
    mockRefreshProviderHealth.mockResolvedValueOnce(alreadyDone);
    const { rerender, props } = renderProviders({ operations: [alreadyDone] });
    const google = await screen.findByRole("form", { name: "Google credential" });

    fireEvent.click(within(google).getByRole("button", { name: "Validate Google" }));
    await waitFor(() => expect(mockRefreshProviderHealth).toHaveBeenCalledWith("google", []));
    await waitFor(() => expect(mockGetProviderHealth).toHaveBeenCalledTimes(2));

    mockRefreshProviderHealth.mockResolvedValueOnce(operation("all", "providers", "queued"));
    fireEvent.click(screen.getByRole("button", { name: "Refresh all providers" }));
    await waitFor(() => expect(mockRefreshProviderHealth).toHaveBeenLastCalledWith(null, []));
    rerender(
      <ProvidersSettings
        {...props}
        operations={[alreadyDone, operation("all", "providers", "succeeded")]}
      />,
    );
    await waitFor(() => expect(mockGetProviderHealth).toHaveBeenCalledTimes(3));
  });

  it("refreshes dependency grouping with the current disabled model set", async () => {
    const { rerender, props } = renderProviders({
      disabledGenerationModelIds: ["openai:gpt-image-1"],
    });
    fireEvent.click(
      await screen.findByRole("button", { name: "Refresh all providers" }),
    );
    await waitFor(() =>
      expect(mockRefreshProviderHealth).toHaveBeenCalledWith(null, ["openai:gpt-image-1"]),
    );

    mockGetProviderHealth.mockResolvedValue([
      falHealth,
      { ...openAiHealth, dependentModelIds: [] },
      replicateHealth,
      googleHealth,
    ]);
    rerender(
      <ProvidersSettings
        {...props}
        disabledGenerationModelIds={["openai:gpt-image-1"]}
        operations={[operation("refresh-1", "providers", "succeeded")]}
      />,
    );
    await waitFor(() => expect(screen.getByRole("region", { name: "Available integrations" })).toHaveTextContent("OpenAI"));
  });

  it("starts an all-provider refresh when the health cache has not been populated", async () => {
    mockGetProviderHealth.mockResolvedValueOnce([]).mockResolvedValueOnce(health);
    const { rerender, props } = renderProviders();

    await waitFor(() => expect(mockRefreshProviderHealth).toHaveBeenCalledWith(null, []));
    rerender(
      <ProvidersSettings
        {...props}
        operations={[operation("refresh-1", "providers", "succeeded")]}
      />,
    );

    expect(await screen.findByRole("form", { name: "fal.ai credential" }))
      .toBeInTheDocument();
  });

  it("ignores a delayed initial health failure after a refresh becomes authoritative", async () => {
    const initialHealth = deferred<ProviderHealth[]>();
    mockGetProviderHealth
      .mockReturnValueOnce(initialHealth.promise)
      .mockResolvedValueOnce(health);
    mockRefreshProviderHealth.mockResolvedValueOnce(
      operation("authoritative-refresh", "providers", "queued"),
    );
    const { rerender, props } = renderProviders();

    fireEvent.click(screen.getByRole("button", { name: "Refresh all providers" }));
    await waitFor(() => expect(mockRefreshProviderHealth).toHaveBeenCalledTimes(1));
    await act(async () => {
      initialHealth.reject(new Error("stale initial failure"));
      await initialHealth.promise.catch(() => undefined);
    });
    expect(screen.queryByText("Provider health could not be loaded. Retry the health check."))
      .not.toBeInTheDocument();

    rerender(
      <ProvidersSettings
        {...props}
        operations={[operation("authoritative-refresh", "providers", "succeeded")]}
      />,
    );
    expect(await screen.findByRole("form", { name: "OpenAI credential" }))
      .toBeInTheDocument();
  });

  it("ignores delayed initial health success after a refresh becomes authoritative", async () => {
    const initialHealth = deferred<ProviderHealth[]>();
    const staleProvider: ProviderHealth = {
      ...falHealth,
      provider: "stale-initial",
      displayName: "Stale initial provider",
    };
    mockGetProviderHealth
      .mockReturnValueOnce(initialHealth.promise)
      .mockResolvedValueOnce(health);
    mockRefreshProviderHealth.mockResolvedValueOnce(
      operation("authoritative-refresh", "providers", "queued"),
    );
    const { rerender, props } = renderProviders();

    fireEvent.click(screen.getByRole("button", { name: "Refresh all providers" }));
    await waitFor(() => expect(mockRefreshProviderHealth).toHaveBeenCalledTimes(1));
    await act(async () => {
      initialHealth.resolve([staleProvider]);
      await initialHealth.promise;
    });
    expect(screen.queryByRole("form", { name: "Stale initial provider credential" }))
      .not.toBeInTheDocument();

    rerender(
      <ProvidersSettings
        {...props}
        operations={[operation("authoritative-refresh", "providers", "succeeded")]}
      />,
    );
    expect(await screen.findByRole("form", { name: "OpenAI credential" }))
      .toBeInTheDocument();
  });

  it("adopts an active refresh after remount and reloads health when it becomes terminal", async () => {
    const queued = operation("remount-refresh", "providers", "queued");
    const first = renderProviders({ operations: [queued] });
    await screen.findByRole("region", { name: "Integration status" });
    first.unmount();

    mockGetProviderHealth.mockClear();
    const second = renderProviders({ operations: [queued] });
    await waitFor(() => expect(mockGetProviderHealth).toHaveBeenCalledTimes(1));
    second.rerender(
      <ProvidersSettings
        {...second.props}
        operations={[operation("remount-refresh", "providers", "succeeded")]}
      />,
    );

    await waitFor(() => expect(mockRefreshProviderHealth).toHaveBeenCalledWith(null, []));
  });

  it("re-adopts an active refresh after StrictMode effect replay", async () => {
    const queued = operation("strict-refresh", "providers", "queued");
    const props: React.ComponentProps<typeof ProvidersSettings> = {
      disabledGenerationModelIds: [],
      generationModels,
      operations: [queued],
    };
    const view = render(
      <StrictMode>
        <ProvidersSettings {...props} />
      </StrictMode>,
    );
    await screen.findByRole("region", { name: "Integration status" });

    view.rerender(
      <StrictMode>
        <ProvidersSettings
          {...props}
          operations={[operation("strict-refresh", "providers", "succeeded")]}
        />
      </StrictMode>,
    );

    await waitFor(() => expect(mockRefreshProviderHealth).toHaveBeenCalledWith(null, []));
  });

  it("correlates an operation event that arrives before the local refresh invoke resolves", async () => {
    const invoke = deferred<SettingsOperation>();
    const queued = operation("local-google", "google", "queued");
    mockRefreshProviderHealth.mockReturnValueOnce(invoke.promise);
    const { rerender, props } = renderProviders();
    const google = await screen.findByRole("form", { name: "Google credential" });

    fireEvent.click(within(google).getByRole("button", { name: "Validate Google" }));
    await waitFor(() => expect(mockRefreshProviderHealth).toHaveBeenCalledTimes(1));
    rerender(<ProvidersSettings {...props} operations={[queued]} />);
    invoke.resolve(queued);
    await waitFor(() => expect(screen.getByLabelText("Google refresh status")).toHaveTextContent("queued"));

    rerender(
      <ProvidersSettings
        {...props}
        operations={[operation("local-google", "google", "succeeded")]}
      />,
    );
    await waitFor(() => expect(mockGetProviderHealth).toHaveBeenCalledTimes(2));
    expect(mockRefreshProviderHealth).toHaveBeenCalledTimes(1);
  });

  it("adopts a matching active operation when the local refresh invoke rejects after backend start", async () => {
    const invoke = deferred<SettingsOperation>();
    const active = operation("transport-lost", "google", "running");
    mockRefreshProviderHealth
      .mockReturnValueOnce(invoke.promise)
      .mockResolvedValueOnce(operation("recovery-current", "google", "queued"));
    const { rerender, props } = renderProviders();
    const google = await screen.findByRole("form", { name: "Google credential" });

    fireEvent.click(within(google).getByRole("button", { name: "Validate Google" }));
    await waitFor(() => expect(mockRefreshProviderHealth).toHaveBeenCalledTimes(1));
    rerender(<ProvidersSettings {...props} operations={[active]} />);
    await act(async () => {
      invoke.reject(new Error("transport closed after backend accepted request"));
      await invoke.promise.catch(() => undefined);
    });
    expect(mockRefreshProviderHealth).toHaveBeenCalledTimes(1);

    rerender(
      <ProvidersSettings
        {...props}
        operations={[operation("transport-lost", "google", "succeeded")]}
      />,
    );
    await waitFor(() => expect(mockRefreshProviderHealth).toHaveBeenCalledTimes(2));
    rerender(
      <ProvidersSettings
        {...props}
        operations={[operation("recovery-current", "google", "succeeded")]}
      />,
    );
    await waitFor(() => expect(mockGetProviderHealth).toHaveBeenCalledTimes(2));
    expect(mockRefreshProviderHealth).toHaveBeenCalledTimes(2);
  });

  it("recovers when the backend operation becomes terminal before the local invoke rejects", async () => {
    const invoke = deferred<SettingsOperation>();
    const queued = operation("transport-terminal", "google", "queued");
    mockRefreshProviderHealth
      .mockReturnValueOnce(invoke.promise)
      .mockResolvedValueOnce(operation("terminal-recovery", "google", "queued"));
    const { rerender, props } = renderProviders();
    const google = await screen.findByRole("form", { name: "Google credential" });

    fireEvent.click(within(google).getByRole("button", { name: "Validate Google" }));
    await waitFor(() => expect(mockRefreshProviderHealth).toHaveBeenCalledTimes(1));
    rerender(<ProvidersSettings {...props} operations={[queued]} />);
    rerender(
      <ProvidersSettings
        {...props}
        operations={[operation("transport-terminal", "google", "succeeded")]}
      />,
    );
    await act(async () => {
      invoke.reject(new Error("transport closed after terminal backend event"));
      await invoke.promise.catch(() => undefined);
    });

    await waitFor(() => expect(mockRefreshProviderHealth).toHaveBeenCalledTimes(2));
    rerender(
      <ProvidersSettings
        {...props}
        operations={[operation("terminal-recovery", "google", "succeeded")]}
      />,
    );
    await waitFor(() => expect(mockGetProviderHealth).toHaveBeenCalledTimes(2));
    expect(mockRefreshProviderHealth).toHaveBeenCalledTimes(2);
  });

  it.each(["missing", "rejected", "unavailable"] as const)(
    "clears a written key while keeping authoritative %s validation inline",
    async (validationState) => {
      const draft = `private-${validationState}-value`;
      mockRefreshProviderHealth.mockResolvedValueOnce(
        operation("validate-openai", "openai", "queued"),
      );
      const { rerender, props } = renderProviders();
      const input = await screen.findByLabelText("New OpenAI key");
      fireEvent.change(input, { target: { value: draft } });
      fireEvent.submit(screen.getByRole("form", { name: "OpenAI credential" }));
      await waitFor(() => expect(mockRefreshProviderHealth).toHaveBeenCalledWith("openai", []));
      expect(screen.getByLabelText("New OpenAI key")).toHaveValue("");

      mockGetProviderHealth.mockResolvedValue([
        ...health.filter(({ provider }) => provider !== "openai"),
        {
          ...openAiHealth,
          credentialSource: "keychain",
          configured: true,
          validationState,
        },
      ]);
      rerender(
        <ProvidersSettings
          {...props}
          operations={[operation("validate-openai", "openai", "succeeded")]}
        />,
      );

      const regroupedInput = await screen.findByLabelText("New OpenAI key");
      expect(regroupedInput).toHaveValue("");
      const alert = await within(
        screen.getByRole("form", { name: "OpenAI credential" }),
      ).findByRole("alert");
      expect(alert).toHaveTextContent("Credential was saved, but provider validation did not accept it");
      expect(alert).not.toHaveTextContent(draft);
    },
  );

  it("clears only the unchanged draft generation after accepted validation", async () => {
    mockRefreshProviderHealth.mockResolvedValueOnce(
      operation("validate-openai", "openai", "queued"),
    );
    const { rerender, props } = renderProviders();
    const input = await screen.findByLabelText("New OpenAI key");
    fireEvent.change(input, { target: { value: "saved-private-value" } });
    fireEvent.submit(screen.getByRole("form", { name: "OpenAI credential" }));
    await waitFor(() => expect(mockSetProviderCredential).toHaveBeenCalledOnce());

    mockGetProviderHealth.mockResolvedValue([
      ...health.filter(({ provider }) => provider !== "openai"),
      {
        ...openAiHealth,
        credentialSource: "keychain",
        configured: true,
        validationState: "available",
      },
    ]);
    rerender(
      <ProvidersSettings
        {...props}
        operations={[operation("validate-openai", "openai", "succeeded")]}
      />,
    );
    await waitFor(() => expect(screen.getByLabelText("New OpenAI key")).toHaveValue(""));
  });

  it("keeps edits made while credential validation is in flight", async () => {
    mockRefreshProviderHealth.mockResolvedValueOnce(
      operation("validate-openai", "openai", "queued"),
    );
    const { rerender, props } = renderProviders();
    const input = await screen.findByLabelText("New OpenAI key");
    fireEvent.change(input, { target: { value: "submitted-private-value" } });
    fireEvent.submit(screen.getByRole("form", { name: "OpenAI credential" }));
    await waitFor(() => expect(mockSetProviderCredential).toHaveBeenCalledOnce());
    fireEvent.change(screen.getByLabelText("New OpenAI key"), {
      target: { value: "newer-unsaved-private-value" },
    });

    mockGetProviderHealth.mockResolvedValue([
      ...health.filter(({ provider }) => provider !== "openai"),
      {
        ...openAiHealth,
        credentialSource: "keychain",
        configured: true,
        validationState: "available",
      },
    ]);
    rerender(
      <ProvidersSettings
        {...props}
        operations={[operation("validate-openai", "openai", "succeeded")]}
      />,
    );
    await waitFor(() =>
      expect(screen.getByLabelText("New OpenAI key"))
        .toHaveValue("newer-unsaved-private-value"),
    );
  });

  it("keeps a validation operation failure inline without retaining the saved key", async () => {
    const draft = "private-operation-failure";
    mockRefreshProviderHealth.mockResolvedValueOnce(
      operation("validate-openai", "openai", "queued"),
    );
    const { rerender, props } = renderProviders();
    const input = await screen.findByLabelText("New OpenAI key");
    fireEvent.change(input, { target: { value: draft } });
    fireEvent.submit(screen.getByRole("form", { name: "OpenAI credential" }));
    await waitFor(() => expect(mockRefreshProviderHealth).toHaveBeenCalledWith("openai", []));

    rerender(
      <ProvidersSettings
        {...props}
        operations={[operation("validate-openai", "openai", "failed")]}
      />,
    );

    expect(screen.getByLabelText("New OpenAI key")).toHaveValue("");
    const alert = await within(
      screen.getByRole("form", { name: "OpenAI credential" }),
    ).findByRole("alert");
    expect(alert).toHaveTextContent("Credential was saved, but provider validation could not be completed");
    expect(alert).not.toHaveTextContent(draft);
  });

  it("clears a rejected saved draft after a later retry validates it", async () => {
    const rejectedHealth = health.map((provider) =>
      provider.provider === "openai"
        ? { ...provider, configured: true, credentialSource: "keychain" as const, validationState: "rejected" as const }
        : provider,
    );
    const acceptedHealth = rejectedHealth.map((provider) =>
      provider.provider === "openai"
        ? { ...provider, validationState: "available" as const }
        : provider,
    );
    mockGetProviderHealth
      .mockResolvedValueOnce(health)
      .mockResolvedValueOnce(rejectedHealth)
      .mockResolvedValueOnce(acceptedHealth);
    mockRefreshProviderHealth
      .mockResolvedValueOnce(operation("rejected-save", "openai", "queued"))
      .mockResolvedValueOnce(operation("accepted-retry", "openai", "queued"));
    const { rerender, props } = renderProviders();
    const input = await screen.findByLabelText("New OpenAI key");
    fireEvent.change(input, { target: { value: "retryable-private-value" } });
    fireEvent.submit(screen.getByRole("form", { name: "OpenAI credential" }));
    await waitFor(() => expect(mockRefreshProviderHealth).toHaveBeenCalledTimes(1));
    rerender(
      <ProvidersSettings
        {...props}
        operations={[operation("rejected-save", "openai", "succeeded")]}
      />,
    );
    expect(await within(screen.getByRole("form", { name: "OpenAI credential" })).findByRole("alert"))
      .toHaveTextContent("provider validation did not accept it");

    fireEvent.click(screen.getByRole("button", { name: "Validate OpenAI" }));
    await waitFor(() => expect(mockRefreshProviderHealth).toHaveBeenCalledTimes(2));
    rerender(
      <ProvidersSettings
        {...props}
        operations={[
          operation("rejected-save", "openai", "succeeded"),
          operation("accepted-retry", "openai", "succeeded"),
        ]}
      />,
    );
    await waitFor(() => expect(screen.getByLabelText("New OpenAI key")).toHaveValue(""));
    expect(within(screen.getByRole("form", { name: "OpenAI credential" })).queryByRole("alert"))
      .not.toBeInTheDocument();
  });

  it("clears a failed saved draft after a later retry validates it", async () => {
    const acceptedHealth = health.map((provider) =>
      provider.provider === "openai"
        ? { ...provider, configured: true, credentialSource: "keychain" as const, validationState: "available" as const }
        : provider,
    );
    mockGetProviderHealth
      .mockResolvedValueOnce(health)
      .mockResolvedValueOnce(acceptedHealth);
    mockRefreshProviderHealth
      .mockResolvedValueOnce(operation("failed-save", "openai", "queued"))
      .mockResolvedValueOnce(operation("accepted-retry", "openai", "queued"));
    const { rerender, props } = renderProviders();
    const input = await screen.findByLabelText("New OpenAI key");
    fireEvent.change(input, { target: { value: "failed-retry-private-value" } });
    fireEvent.submit(screen.getByRole("form", { name: "OpenAI credential" }));
    await waitFor(() => expect(mockRefreshProviderHealth).toHaveBeenCalledTimes(1));
    rerender(
      <ProvidersSettings
        {...props}
        operations={[operation("failed-save", "openai", "failed")]}
      />,
    );
    expect(await within(screen.getByRole("form", { name: "OpenAI credential" })).findByRole("alert"))
      .toHaveTextContent("provider validation could not be completed");

    fireEvent.click(screen.getByRole("button", { name: "Validate OpenAI" }));
    await waitFor(() => expect(mockRefreshProviderHealth).toHaveBeenCalledTimes(2));
    rerender(
      <ProvidersSettings
        {...props}
        operations={[
          operation("failed-save", "openai", "failed"),
          operation("accepted-retry", "openai", "succeeded"),
        ]}
      />,
    );
    await waitFor(() => expect(screen.getByLabelText("New OpenAI key")).toHaveValue(""));
    expect(within(screen.getByRole("form", { name: "OpenAI credential" })).queryByRole("alert"))
      .not.toBeInTheDocument();
  });

  it("clears a saved draft after retry recovers a validation start failure", async () => {
    const acceptedHealth = health.map((provider) =>
      provider.provider === "openai"
        ? { ...provider, configured: true, credentialSource: "keychain" as const, validationState: "available" as const }
        : provider,
    );
    mockGetProviderHealth
      .mockResolvedValueOnce(health)
      .mockResolvedValueOnce(acceptedHealth);
    mockRefreshProviderHealth
      .mockRejectedValueOnce(new Error("validation transport unavailable"))
      .mockResolvedValueOnce(operation("accepted-retry", "openai", "queued"));
    const { rerender, props } = renderProviders();
    const input = await screen.findByLabelText("New OpenAI key");
    fireEvent.change(input, { target: { value: "start-retry-private-value" } });
    fireEvent.submit(screen.getByRole("form", { name: "OpenAI credential" }));
    expect(await screen.findByText(
      "Credential was saved, but provider validation could not be started.",
    ))
      .toHaveTextContent("provider validation could not be started");

    fireEvent.click(screen.getByRole("button", { name: "Validate OpenAI" }));
    await waitFor(() => expect(mockRefreshProviderHealth).toHaveBeenCalledTimes(2));
    rerender(
      <ProvidersSettings
        {...props}
        operations={[operation("accepted-retry", "openai", "succeeded")]}
      />,
    );
    await waitFor(() => expect(screen.getByLabelText("New OpenAI key")).toHaveValue(""));
    expect(within(screen.getByRole("form", { name: "OpenAI credential" })).queryByRole("alert"))
      .not.toBeInTheDocument();
  });

  it("keeps an edited draft when retry accepts an older saved generation", async () => {
    const rejectedHealth = health.map((provider) =>
      provider.provider === "openai"
        ? { ...provider, configured: true, credentialSource: "keychain" as const, validationState: "rejected" as const }
        : provider,
    );
    const acceptedHealth = rejectedHealth.map((provider) =>
      provider.provider === "openai"
        ? { ...provider, validationState: "available" as const }
        : provider,
    );
    mockGetProviderHealth
      .mockResolvedValueOnce(health)
      .mockResolvedValueOnce(rejectedHealth)
      .mockResolvedValueOnce(acceptedHealth);
    mockRefreshProviderHealth
      .mockResolvedValueOnce(operation("rejected-save", "openai", "queued"))
      .mockResolvedValueOnce(operation("accepted-retry", "openai", "queued"));
    const { rerender, props } = renderProviders();
    const input = await screen.findByLabelText("New OpenAI key");
    fireEvent.change(input, { target: { value: "older-saved-private-value" } });
    fireEvent.submit(screen.getByRole("form", { name: "OpenAI credential" }));
    await waitFor(() => expect(mockRefreshProviderHealth).toHaveBeenCalledTimes(1));
    rerender(
      <ProvidersSettings
        {...props}
        operations={[operation("rejected-save", "openai", "succeeded")]}
      />,
    );
    await within(screen.getByRole("form", { name: "OpenAI credential" })).findByRole("alert");
    fireEvent.change(screen.getByLabelText("New OpenAI key"), {
      target: { value: "newer-unsaved-private-value" },
    });

    fireEvent.click(screen.getByRole("button", { name: "Validate OpenAI" }));
    await waitFor(() => expect(mockRefreshProviderHealth).toHaveBeenCalledTimes(2));
    rerender(
      <ProvidersSettings
        {...props}
        operations={[
          operation("rejected-save", "openai", "succeeded"),
          operation("accepted-retry", "openai", "succeeded"),
        ]}
      />,
    );
    await waitFor(() => expect(screen.getByLabelText("New OpenAI key"))
      .toHaveValue("newer-unsaved-private-value"));
    expect(within(screen.getByRole("form", { name: "OpenAI credential" })).queryByRole("alert"))
      .not.toBeInTheDocument();
  });

  it("lets a newer all-provider refresh validate a saved draft after an older scoped result", async () => {
    const scoped = operation("scoped-openai", "openai", "queued");
    const all = operation("all-newer", "providers", "queued");
    const rejectedHealth = health.map((provider) =>
      provider.provider === "openai"
        ? { ...provider, configured: true, credentialSource: "keychain" as const, validationState: "rejected" as const }
        : provider,
    );
    const acceptedHealth = health.map((provider) =>
      provider.provider === "openai"
        ? { ...provider, configured: true, credentialSource: "keychain" as const, validationState: "available" as const }
        : provider,
    );
    mockGetProviderHealth
      .mockResolvedValueOnce(health)
      .mockResolvedValueOnce(rejectedHealth)
      .mockResolvedValueOnce(acceptedHealth);
    mockRefreshProviderHealth
      .mockResolvedValueOnce(scoped)
      .mockResolvedValueOnce(all);
    const { rerender, props } = renderProviders();
    const input = await screen.findByLabelText("New OpenAI key");
    fireEvent.change(input, { target: { value: "cross-target-private-value" } });
    fireEvent.submit(screen.getByRole("form", { name: "OpenAI credential" }));
    await waitFor(() => expect(mockRefreshProviderHealth).toHaveBeenCalledTimes(1));
    fireEvent.click(screen.getByRole("button", { name: "Refresh all providers" }));
    await waitFor(() => expect(mockRefreshProviderHealth).toHaveBeenCalledTimes(2));

    rerender(
      <ProvidersSettings
        {...props}
        operations={[operation("scoped-openai", "openai", "succeeded"), all]}
      />,
    );
    await waitFor(() => expect(mockGetProviderHealth).toHaveBeenCalledTimes(2));
    expect(screen.getByLabelText("New OpenAI key")).toHaveValue("");
    expect(within(screen.getByRole("form", { name: "OpenAI credential" })).queryByRole("alert"))
      .not.toBeInTheDocument();

    rerender(
      <ProvidersSettings
        {...props}
        operations={[
          operation("scoped-openai", "openai", "succeeded"),
          operation("all-newer", "providers", "succeeded"),
        ]}
      />,
    );
    await waitFor(() => expect(mockGetProviderHealth).toHaveBeenCalledTimes(3));
    await waitFor(() => expect(screen.getByLabelText("New OpenAI key")).toHaveValue(""));
  });

  it("ignores an older scoped validation that finishes after a newer accepted all-provider refresh", async () => {
    const scoped = operation("scoped-openai", "openai", "queued");
    const all = operation("all-newer", "providers", "queued");
    const acceptedHealth = health.map((provider) =>
      provider.provider === "openai"
        ? { ...provider, configured: true, credentialSource: "keychain" as const, validationState: "available" as const }
        : provider,
    );
    const rejectedHealth = health.map((provider) =>
      provider.provider === "openai"
        ? { ...provider, configured: true, credentialSource: "keychain" as const, validationState: "rejected" as const }
        : provider,
    );
    mockGetProviderHealth
      .mockResolvedValueOnce(health)
      .mockResolvedValueOnce(acceptedHealth)
      .mockResolvedValueOnce(rejectedHealth);
    mockRefreshProviderHealth
      .mockResolvedValueOnce(scoped)
      .mockResolvedValueOnce(all);
    const { rerender, props } = renderProviders();
    const input = await screen.findByLabelText("New OpenAI key");
    fireEvent.change(input, { target: { value: "inverse-private-value" } });
    fireEvent.submit(screen.getByRole("form", { name: "OpenAI credential" }));
    await waitFor(() => expect(mockRefreshProviderHealth).toHaveBeenCalledTimes(1));
    fireEvent.click(screen.getByRole("button", { name: "Refresh all providers" }));
    await waitFor(() => expect(mockRefreshProviderHealth).toHaveBeenCalledTimes(2));

    rerender(
      <ProvidersSettings
        {...props}
        operations={[scoped, operation("all-newer", "providers", "succeeded")]}
      />,
    );
    await waitFor(() => expect(screen.getByLabelText("New OpenAI key")).toHaveValue(""));
    rerender(
      <ProvidersSettings
        {...props}
        operations={[
          operation("scoped-openai", "openai", "failed"),
          operation("all-newer", "providers", "succeeded"),
        ]}
      />,
    );
    expect(within(screen.getByRole("form", { name: "OpenAI credential" })).queryByRole("alert"))
      .not.toBeInTheDocument();
    expect(screen.queryByText("Provider refresh did not complete. Retry the health check."))
      .not.toBeInTheDocument();
  });

  it("uses a failed newer all-provider refresh as the saved draft authority", async () => {
    const scoped = operation("scoped-openai", "openai", "queued");
    const all = operation("all-newer", "providers", "queued");
    mockRefreshProviderHealth
      .mockResolvedValueOnce(scoped)
      .mockResolvedValueOnce(all);
    const { rerender, props } = renderProviders();
    const input = await screen.findByLabelText("New OpenAI key");
    fireEvent.change(input, { target: { value: "failed-all-private-value" } });
    fireEvent.submit(screen.getByRole("form", { name: "OpenAI credential" }));
    await waitFor(() => expect(mockRefreshProviderHealth).toHaveBeenCalledTimes(1));
    fireEvent.click(screen.getByRole("button", { name: "Refresh all providers" }));
    await waitFor(() => expect(mockRefreshProviderHealth).toHaveBeenCalledTimes(2));

    rerender(
      <ProvidersSettings
        {...props}
        operations={[scoped, operation("all-newer", "providers", "failed")]}
      />,
    );

    expect(screen.getByLabelText("New OpenAI key")).toHaveValue("");
    expect(await within(screen.getByRole("form", { name: "OpenAI credential" })).findByRole("alert"))
      .toHaveTextContent("Credential was saved, but provider validation could not be completed");

    mockGetProviderHealth.mockResolvedValueOnce(health);
    rerender(
      <ProvidersSettings
        {...props}
        operations={[
          operation("scoped-openai", "openai", "succeeded"),
          operation("all-newer", "providers", "failed"),
        ]}
      />,
    );
    await waitFor(() => expect(mockGetProviderHealth).toHaveBeenCalledTimes(2));
    expect(screen.getByText("Provider refresh did not complete. Retry the health check."))
      .toBeInTheDocument();
  });

  it("lets only the latest all-provider health load claim a newly discovered provider", async () => {
    const firstHealth = deferred<ProviderHealth[]>();
    const secondHealth = deferred<ProviderHealth[]>();
    const first = operation("all-first", "providers", "queued");
    const second = operation("all-second", "providers", "queued");
    const discovered = (accountLabel: string): ProviderHealth => ({
      provider: "discovered",
      displayName: "Discovered provider",
      credentialSource: "keychain",
      configured: true,
      validationState: "available",
      accountLabel,
      balanceLabel: null,
      dependentModelIds: [],
      lastCheckedAt: "2026-07-17T09:00:00Z",
      diagnosticCode: null,
    });
    mockGetProviderHealth
      .mockResolvedValueOnce(health)
      .mockReturnValueOnce(firstHealth.promise)
      .mockReturnValueOnce(secondHealth.promise);
    mockRefreshProviderHealth
      .mockResolvedValueOnce(first)
      .mockResolvedValueOnce(second);
    const { rerender, props } = renderProviders();
    await screen.findByRole("form", { name: "OpenAI credential" });

    fireEvent.click(screen.getByRole("button", { name: "Refresh all providers" }));
    await waitFor(() => expect(mockRefreshProviderHealth).toHaveBeenCalledTimes(1));
    rerender(
      <ProvidersSettings
        {...props}
        operations={[operation("all-first", "providers", "succeeded")]}
      />,
    );
    await waitFor(() => expect(mockGetProviderHealth).toHaveBeenCalledTimes(2));
    fireEvent.click(screen.getByRole("button", { name: "Refresh all providers" }));
    await waitFor(() => expect(mockRefreshProviderHealth).toHaveBeenCalledTimes(2));
    rerender(
      <ProvidersSettings
        {...props}
        operations={[
          operation("all-first", "providers", "succeeded"),
          operation("all-second", "providers", "succeeded"),
        ]}
      />,
    );
    await waitFor(() => expect(mockGetProviderHealth).toHaveBeenCalledTimes(3));

    await act(async () => {
      firstHealth.resolve([...health, discovered("Stale account")]);
      await firstHealth.promise;
    });
    expect(screen.queryByRole("form", { name: "Discovered provider credential" }))
      .not.toBeInTheDocument();
    await act(async () => {
      secondHealth.resolve([...health, discovered("Current account")]);
      await secondHealth.promise;
    });
    expect(await screen.findByRole("form", { name: "Discovered provider credential" }))
      .toHaveTextContent("Current account");
    expect(screen.getByRole("form", { name: "Discovered provider credential" }))
      .not.toHaveTextContent("Stale account");
  });

  it("coalesces a save behind an active scoped retry into a later validation", async () => {
    const activeRetry = operation("retry-active", "openai", "running");
    mockRefreshProviderHealth.mockResolvedValueOnce(
      operation("save-validation", "openai", "queued"),
    );
    const { rerender, props } = renderProviders();
    const input = await screen.findByLabelText("New OpenAI key");
    rerender(<ProvidersSettings {...props} operations={[activeRetry]} />);
    fireEvent.change(input, { target: { value: "coalesced-private-value" } });
    fireEvent.submit(screen.getByRole("form", { name: "OpenAI credential" }));
    await waitFor(() => expect(mockSetProviderCredential).toHaveBeenCalledOnce());
    expect(mockRefreshProviderHealth).not.toHaveBeenCalled();

    rerender(
      <ProvidersSettings
        {...props}
        operations={[operation("retry-active", "openai", "succeeded")]}
      />,
    );
    await waitFor(() =>
      expect(mockRefreshProviderHealth).toHaveBeenCalledWith("openai", []),
    );
    expect(mockRefreshProviderHealth).toHaveBeenCalledTimes(1);
  });

  it("uses command-result credential source and actions even when refresh start fails", async () => {
    mockRefreshProviderHealth.mockRejectedValue(new Error("validation unavailable"));
    mockSetProviderCredential.mockResolvedValueOnce({
      provider: "google",
      displayName: "Google",
      configured: true,
      source: "keychain",
    });
    renderProviders();
    const googleInput = await screen.findByLabelText("New Google key");
    fireEvent.change(googleInput, { target: { value: "override-private-value" } });
    fireEvent.submit(screen.getByRole("form", { name: "Google credential" }));

    const google = await screen.findByRole("form", { name: "Google credential" });
    await waitFor(() => expect(google).toHaveTextContent("Saved in macOS Keychain"));
    expect(within(google).getByRole("button", { name: "Disconnect Google" }))
      .toBeEnabled();
    expect(within(google).getByRole("button", { name: "Update Google key" }))
      .toBeDisabled();
    expect(screen.getByLabelText("New Google key")).toHaveValue("");
    expect(await within(google).findByRole("alert"))
      .toHaveTextContent("Credential was saved, but provider validation could not be started");
  });

  it("publishes authoritative Keychain save and delete command results", async () => {
    const onCredentialStatusChange = vi.fn();
    const overrides = { onCredentialStatusChange };
    mockRefreshProviderHealth.mockRejectedValue(new Error("validation unavailable"));
    mockDeleteProviderCredential.mockResolvedValueOnce({
      provider: "openai",
      displayName: "OpenAI",
      configured: false,
      source: "missing",
    });
    renderProviders(overrides);
    const openAi = await screen.findByRole("form", { name: "OpenAI credential" });
    fireEvent.change(within(openAi).getByLabelText("New OpenAI key"), {
      target: { value: "private-value" },
    });
    fireEvent.submit(openAi);

    await waitFor(() =>
      expect(onCredentialStatusChange).toHaveBeenCalledWith({
        provider: "openai",
        displayName: "OpenAI",
        configured: true,
        source: "keychain",
      }),
    );
    const configuredOpenAi = await screen.findByRole("form", {
      name: "OpenAI credential",
    });
    fireEvent.click(
      within(configuredOpenAi).getByRole("button", { name: "Disconnect OpenAI" }),
    );
    fireEvent.click(
      within(screen.getByRole("dialog", { name: "Disconnect OpenAI" }))
        .getByRole("button", { name: "Disconnect OpenAI" }),
    );
    await waitFor(() =>
      expect(onCredentialStatusChange).toHaveBeenLastCalledWith({
        provider: "openai",
        displayName: "OpenAI",
        configured: false,
        source: "missing",
      }),
    );
  });

  it("does not let a delayed credential list overwrite a newer command result", async () => {
    let resolveStatuses!: (
      statuses: Awaited<ReturnType<typeof listProviderCredentialStatuses>>,
    ) => void;
    mockListProviderCredentialStatuses.mockImplementationOnce(
      () => new Promise((resolve) => { resolveStatuses = resolve; }),
    );
    mockRefreshProviderHealth.mockRejectedValue(new Error("validation unavailable"));
    renderProviders();
    const openAi = await screen.findByRole("form", { name: "OpenAI credential" });
    fireEvent.change(within(openAi).getByLabelText("New OpenAI key"), {
      target: { value: "newer-private-value" },
    });
    fireEvent.submit(openAi);
    await waitFor(() =>
      expect(screen.getByRole("form", { name: "OpenAI credential" }))
        .toHaveTextContent("Saved in macOS Keychain"),
    );

    resolveStatuses([
      {
        provider: "openai",
        displayName: "OpenAI",
        configured: false,
        source: "missing",
      },
      {
        provider: "google",
        displayName: "Google",
        configured: true,
        source: "keychain",
      },
      {
        provider: "replicate",
        displayName: "Replicate",
        configured: false,
        source: "missing",
      },
    ]);

    await waitFor(() => expect(mockListProviderCredentialStatuses).toHaveBeenCalledOnce());
    const currentOpenAi = screen.getByRole("form", { name: "OpenAI credential" });
    expect(currentOpenAi).toHaveTextContent("Saved in macOS Keychain");
    expect(within(currentOpenAi).getByRole("button", { name: "Disconnect OpenAI" }))
      .toBeEnabled();
    expect(screen.getByRole("form", { name: "Google credential" }))
      .toHaveTextContent("Saved in macOS Keychain");
    expect(screen.getByRole("form", { name: "Replicate credential" }))
      .toHaveTextContent("Not configured");
  });
});
