import "@testing-library/jest-dom/vitest";
import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

import { defaultAppPreferences } from "@/lib/app-settings";
import { getMcpClientConfiguration } from "@/lib/settings/agent";
import { AdvancedSettings } from "./advanced-settings";

const { openMock } = vi.hoisted(() => ({ openMock: vi.fn() }));

vi.mock("@tauri-apps/plugin-dialog", () => ({ open: openMock }));
vi.mock("@/lib/settings/agent", () => ({
  getMcpClientConfiguration: vi.fn(),
}));

const mockGetMcpClientConfiguration = vi.mocked(getMcpClientConfiguration);

const model = {
  modelId: "whisper-small",
  displayName: "Whisper Small",
  isActive: false,
  installStatus: "missing" as const,
  localPath: "/models/whisper-small",
  approximateSizeBytes: 500,
  downloadedFiles: 0,
  totalFiles: 2,
};

describe("AdvancedSettings", () => {
  beforeEach(() => {
    openMock.mockReset();
    mockGetMcpClientConfiguration.mockReset().mockResolvedValue({
      actionLabel: "Open a project",
      configuration: null,
      executable: "/Applications/Video Creater.app/video-creater-mcp-server",
      projectDir: null,
    });
  });

  it("opens System Health without adding placeholder log actions", () => {
    const onOpenSystemHealth = vi.fn();
    render(
      <AdvancedSettings
        preferences={defaultAppPreferences}
        projectRoot={null}
        models={[model]}
        onChange={vi.fn()}
        onImportLocalModel={vi.fn()}
        onRetryModelStatus={vi.fn()}
        onOpenSystemHealth={onOpenSystemHealth}
      />,
    );

    expect(screen.getByLabelText("Generation execution backend")).toHaveValue(
      "inProcess",
    );
    expect(
      screen.getByText("Open a project to copy MCP configuration."),
    ).toBeVisible();
    fireEvent.click(screen.getByRole("button", { name: "Open System Health" }));
    expect(onOpenSystemHealth).toHaveBeenCalledTimes(1);
    expect(
      screen.queryByRole("button", { name: "Reveal logs" }),
    ).not.toBeInTheDocument();
    expect(screen.queryByText("GStreamer status")).not.toBeInTheDocument();
  });

  it("submits execution changes without optimistic control state", () => {
    const onChange = vi.fn();
    render(
      <AdvancedSettings
        preferences={defaultAppPreferences}
        projectRoot={null}
        models={[model]}
        onChange={onChange}
        onImportLocalModel={vi.fn()}
        onRetryModelStatus={vi.fn()}
        onOpenSystemHealth={vi.fn()}
      />,
    );

    const select = screen.getByLabelText("Generation execution backend");
    fireEvent.change(select, { target: { value: "temporal" } });
    expect(onChange).toHaveBeenCalledWith({
      generationExecutionBackend: "temporal",
    });
    expect(select).toHaveValue("inProcess");
  });

  it("resets all app preferences only after confirmation", () => {
    const onChange = vi.fn();
    render(
      <AdvancedSettings
        preferences={{
          ...defaultAppPreferences,
          requireProviderUploadConfirmation: false,
          generationExecutionBackend: "temporal",
        }}
        projectRoot={null}
        models={[model]}
        onChange={onChange}
        onImportLocalModel={vi.fn()}
        onRetryModelStatus={vi.fn()}
        onOpenSystemHealth={vi.fn()}
      />,
    );

    fireEvent.click(
      screen.getByRole("button", { name: "Reset app preferences" }),
    );
    expect(onChange).not.toHaveBeenCalled();
    fireEvent.click(
      screen.getByRole("button", { name: "Confirm reset app preferences" }),
    );
    expect(onChange).toHaveBeenCalledWith(
      expect.objectContaining({
        requireProviderUploadConfirmation: true,
        generationExecutionBackend: "inProcess",
        newProjectDefaults: defaultAppPreferences.newProjectDefaults,
      }),
    );
  });

  it("traps reset focus, closes on Escape, and restores the reset trigger", async () => {
    render(
      <AdvancedSettings
        preferences={defaultAppPreferences}
        projectRoot={null}
        models={[model]}
        onChange={vi.fn()}
        onImportLocalModel={vi.fn()}
        onRetryModelStatus={vi.fn()}
        onOpenSystemHealth={vi.fn()}
      />,
    );
    const trigger = screen.getByRole("button", { name: "Reset app preferences" });
    trigger.focus();
    fireEvent.click(trigger);

    const dialog = screen.getByRole("alertdialog", { name: "Reset app preferences?" });
    const cancel = screen.getByRole("button", { name: "Cancel" });
    const confirm = screen.getByRole("button", { name: "Confirm reset app preferences" });
    await waitFor(() => expect(cancel).toHaveFocus());

    fireEvent.keyDown(dialog, { key: "Tab" });
    expect(confirm).toHaveFocus();
    fireEvent.keyDown(dialog, { key: "Tab" });
    expect(cancel).toHaveFocus();
    fireEvent.keyDown(dialog, { key: "Tab", shiftKey: true });
    expect(confirm).toHaveFocus();
    fireEvent.keyDown(dialog, { key: "Escape" });

    await waitFor(() => expect(trigger).toHaveFocus());
    expect(screen.queryByRole("alertdialog")).not.toBeInTheDocument();
  });

  it("imports a selected model through the real folder-import action", async () => {
    const onImportLocalModel = vi.fn().mockResolvedValue(undefined);
    openMock.mockResolvedValue("/Downloads/whisper-small");
    render(
      <AdvancedSettings
        preferences={defaultAppPreferences}
        projectRoot={null}
        models={[model]}
        onChange={vi.fn()}
        onImportLocalModel={onImportLocalModel}
        onRetryModelStatus={vi.fn()}
        onOpenSystemHealth={vi.fn()}
      />,
    );

    fireEvent.click(
      screen.getByRole("button", {
        name: "Import Whisper Small from folder",
      }),
    );

    await waitFor(() =>
      expect(onImportLocalModel).toHaveBeenCalledWith(
        "whisper-small",
        "/Downloads/whisper-small",
      ),
    );
  });

  it("labels a native import failure without offering status-only retry", async () => {
    const onImportLocalModel = vi.fn().mockRejectedValue(new Error("model manifest invalid"));
    openMock.mockResolvedValue("/Downloads/whisper-small");
    render(
      <AdvancedSettings
        preferences={defaultAppPreferences}
        projectRoot={null}
        models={[model]}
        onChange={vi.fn()}
        onImportLocalModel={onImportLocalModel}
        onRetryModelStatus={vi.fn()}
        onOpenSystemHealth={vi.fn()}
      />,
    );

    fireEvent.click(
      screen.getByRole("button", { name: "Import Whisper Small from folder" }),
    );

    expect(await screen.findByRole("alert")).toHaveTextContent(
      "Model folder import failed: model manifest invalid",
    );
    expect(
      screen.queryByRole("button", { name: "Retry imported model status refresh" }),
    ).not.toBeInTheDocument();
  });

  it("offers status refresh retry after a successful import without repeating import", async () => {
    const onImportLocalModel = vi.fn().mockResolvedValue({
      status: "importedRefreshFailed",
      detail: "model inventory unavailable",
    });
    const onRetryModelStatus = vi.fn().mockResolvedValue(true);
    openMock.mockResolvedValue("/Downloads/whisper-small");
    render(
      <AdvancedSettings
        preferences={defaultAppPreferences}
        projectRoot={null}
        models={[model]}
        onChange={vi.fn()}
        onImportLocalModel={onImportLocalModel}
        onRetryModelStatus={onRetryModelStatus}
        onOpenSystemHealth={vi.fn()}
      />,
    );

    fireEvent.click(
      screen.getByRole("button", { name: "Import Whisper Small from folder" }),
    );

    expect(
      await screen.findByText(
        "Imported, status refresh failed: model inventory unavailable",
      ),
    ).toBeVisible();
    fireEvent.click(
      screen.getByRole("button", { name: "Retry imported model status refresh" }),
    );

    await waitFor(() => expect(onRetryModelStatus).toHaveBeenCalledTimes(1));
    await waitFor(() =>
      expect(
        screen.queryByText(/Imported, status refresh failed/),
      ).not.toBeInTheDocument(),
    );
    expect(onImportLocalModel).toHaveBeenCalledTimes(1);
  });
});
