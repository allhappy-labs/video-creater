import "@testing-library/jest-dom/vitest";
import { act, fireEvent, screen, waitFor, within } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { applyProjectActionsLocally } from "@/lib/agent/project-merge";
import { fallbackExportProfileAvailability } from "@/lib/export/profiles";
import type { ExportProfileAvailability, ProjectAction, VideoProject } from "@/lib/project";
import { backendRequest } from "@/lib/runtime/backend-client";
import { chooseExportDirectory } from "@/lib/runtime/adapters/tauri-dialog";
import { BackendUnavailableError } from "@/lib/runtime/backend-transport";
import { installRuntimeMode } from "@/lib/runtime/runtime-mode";
import { fixtureProject } from "@/test-utils/editor-fixtures";
import { renderWithEditorStore } from "@/test-utils/editor-render";
import { ExportPopover } from "./export-popover";

vi.mock("@/lib/runtime/backend-client", () => ({ backendRequest: vi.fn(), backendListen: vi.fn(), backendMediaUrl: (p: string) => p }));
vi.mock("@/lib/runtime/adapters/tauri-dialog", () => ({ chooseExportDirectory: vi.fn() }));

type Handler = (input: Record<string, unknown>) => unknown;

const projectDir = "/projects/edison";

function report(unavailable: Partial<Record<ExportProfileAvailability["profile"], string>> = {}): ExportProfileAvailability[] {
  return fallbackExportProfileAvailability.map((profile) => {
    const reason = unavailable[profile.profile];
    return { ...profile, available: !reason, unavailableReason: reason ?? null, qualityAvailability: { draft: !reason, final: !reason }, qualityUnavailableReasons: {} };
  });
}

function setup(handlers: Record<string, Handler> = {}, options: { readonly projectDir?: string; readonly project?: VideoProject } = {}) {
  const backend = { project: options.project ?? ({ ...fixtureProject(), schemaVersion: 2 } as VideoProject) };
  const all: Record<string, Handler> = {
    get_export_profile_availability_report: () => report(),
    apply_project_actions_to_split_project_folder: (input) => {
      backend.project = applyProjectActionsLocally(backend.project, input.actions as ProjectAction[]);
      return { project: backend.project };
    },
    load_split_project_from_folder: () => backend.project,
    ...handlers,
  };
  vi.mocked(backendRequest).mockImplementation(async (command: string, input?: Record<string, unknown>) => {
    const handler = all[command];
    if (!handler) throw new BackendUnavailableError();
    return handler(input ?? {});
  });
  const result = renderWithEditorStore(<ExportPopover />, { project: backend.project, projectDir: options.projectDir ?? projectDir });
  return { ...result, backend };
}

async function openPopover() {
  fireEvent.click(screen.getByRole("button", { name: "Export" }));
  const dialog = await screen.findByRole("dialog", { name: "Export" });
  return dialog;
}

function calls(command: string) {
  return vi.mocked(backendRequest).mock.calls.filter(([name]) => name === command);
}

describe("ExportPopover", () => {
  beforeEach(() => {
    window.localStorage.clear();
    vi.mocked(backendRequest).mockReset();
    vi.mocked(chooseExportDirectory).mockReset();
    installRuntimeMode("desktop");
  });

  it("opens from the Export button with the project name and MP4 1080p High", async () => {
    setup();
    const dialog = await openPopover();
    expect(within(dialog).getByRole("textbox", { name: "Name" })).toHaveValue("Edison Restoration Demo");
    await waitFor(() => expect(within(dialog).getByRole("radio", { name: "MP4" })).toHaveAttribute("aria-checked", "true"));
    expect(within(dialog).getByRole("radio", { name: "1080p" })).toHaveAttribute("aria-checked", "true");
    expect(within(dialog).getByRole("radio", { name: "High" })).toHaveAttribute("aria-checked", "true");
    expect(within(dialog).getByText(`${projectDir}/exports`)).toBeInTheDocument();
    expect(within(dialog).getByRole("button", { name: "Choose export folder" })).not.toHaveAttribute("aria-disabled");
    expect(within(dialog).getByRole("button", { name: "Export video" })).not.toHaveAttribute("aria-disabled");
  });

  it("renders unavailable options, ProRes included, disabled with their reasons", async () => {
    setup({ get_export_profile_availability_report: () => report({ proResMov: "ProRes needs macOS.", webm: "WebM encoder missing." }) });
    const dialog = await openPopover();
    const webm = await within(dialog).findByRole("radio", { name: "WebM" });
    await waitFor(() => expect(webm).toHaveAttribute("aria-disabled", "true"));
    expect(webm).toHaveAccessibleDescription("WebM encoder missing.");
    expect(within(dialog).getByRole("radio", { name: "ProRes" })).toHaveAccessibleDescription("ProRes needs macOS.");
    expect(within(dialog).queryByRole("radio", { name: /Legacy/ })).not.toBeInTheDocument();
    expect(within(dialog).getByRole("radio", { name: "Master" })).not.toHaveAttribute("aria-disabled");

    fireEvent.click(webm);
    expect(within(dialog).getByRole("radio", { name: "MP4" })).toHaveAttribute("aria-checked", "true");
  });

  it("shows the split project reason and disables every export outside a project folder", async () => {
    setup({}, { projectDir: "", project: fixtureProject() });
    const dialog = await openPopover();
    const exportVideo = within(dialog).getByRole("button", { name: "Export video" });
    expect(exportVideo).toHaveAttribute("aria-disabled", "true");
    expect(exportVideo).toHaveAccessibleDescription("Save as a schema-v2 split project to export media.");
    expect(within(dialog).getByRole("button", { name: "Premiere XML" })).toHaveAttribute("aria-disabled", "true");
    fireEvent.click(within(dialog).getByRole("button", { name: "Premiere XML" }));
    fireEvent.click(exportVideo);
    expect(screen.getByRole("dialog", { name: "Export" })).toBeInTheDocument();
    expect(calls("render_media_to_split_project_folder")).toHaveLength(0);
  });

  it("updates the summary line as resolution, quality and codec change", async () => {
    const project = { ...fixtureProject(), schemaVersion: 2 } as VideoProject;
    setup({}, { project: { ...project, renderSettings: { ...project.renderSettings, fps: 30 } } });
    const dialog = await openPopover();
    await waitFor(() => expect(within(dialog).getByText(/^H\.264 · 30 fps · ≈ /)).toBeInTheDocument());
    const summary = () => within(dialog).getByText(/ fps · ≈ /).textContent;
    const hd = summary();

    fireEvent.click(within(dialog).getByRole("radio", { name: "4K" }));
    expect(summary()).not.toBe(hd);
    fireEvent.click(within(dialog).getByRole("radio", { name: "Draft" }));
    expect(summary()).toMatch(/^H\.264 · 24 fps · ≈ /);

    fireEvent.click(within(dialog).getByRole("button", { name: "Advanced" }));
    fireEvent.click(within(dialog).getByRole("radio", { name: "H.265" }));
    expect(summary()).toMatch(/^H\.265 · 24 fps · ≈ /);
  });

  it("starts one export task and closes the popover", async () => {
    const { store } = setup({ render_media_to_split_project_folder: () => new Promise(() => undefined) });
    const dialog = await openPopover();
    await waitFor(() => expect(within(dialog).getByRole("radio", { name: "MP4" })).toHaveAttribute("aria-checked", "true"));
    fireEvent.change(within(dialog).getByRole("textbox", { name: "Name" }), { target: { value: "Edison intro" } });

    await act(async () => {
      fireEvent.click(within(dialog).getByRole("button", { name: "Export video" }));
    });
    await waitFor(() => expect(screen.queryByRole("dialog", { name: "Export" })).not.toBeInTheDocument());
    expect(store.getState().exportPopover).toBeNull();
    await waitFor(() => expect(calls("render_media_to_split_project_folder")).toHaveLength(1));
    const exports = store.getState().tasks.filter((task) => task.kind === "export");
    expect(exports).toHaveLength(1);
    expect(exports[0]).toMatchObject({ status: "running" });
    expect([...store.getState().exportPlans.values()]).toEqual([expect.objectContaining({ name: "Edison intro", profile: "mp4H264" })]);
  });

  it("starts the NLE export from a footer link", async () => {
    const { store } = setup({ export_nle_xml_to_split_project_folder: (input) => ({ project: store.getState().project, exportPath: "exports/timeline.fcpxml", job: { id: input.jobId } }) });
    const dialog = await openPopover();
    await act(async () => {
      fireEvent.click(within(dialog).getByRole("button", { name: "DaVinci XML" }));
    });
    await waitFor(() => expect(calls("export_nle_xml_to_split_project_folder")).toHaveLength(1));
    expect(calls("export_nle_xml_to_split_project_folder")[0]?.[1]).toMatchObject({ projectDir, format: "davinciFcpxml" });
    expect(screen.queryByRole("dialog", { name: "Export" })).not.toBeInTheDocument();
    await waitFor(() => expect(store.getState().toasts.map((toast) => toast.title)).toEqual(["Exported DaVinci XML"]));
  });

  it("saves into a chosen folder, and keeps the folder when the chooser is cancelled", async () => {
    const { store } = setup({ render_media_to_split_project_folder: () => new Promise(() => undefined) });
    vi.mocked(chooseExportDirectory).mockResolvedValueOnce("/home/me/Movies").mockResolvedValueOnce(null);
    const dialog = await openPopover();
    await waitFor(() => expect(within(dialog).getByRole("radio", { name: "MP4" })).toHaveAttribute("aria-checked", "true"));

    await act(async () => {
      fireEvent.click(within(dialog).getByRole("button", { name: "Choose export folder" }));
    });
    expect(chooseExportDirectory).toHaveBeenCalledWith(`${projectDir}/exports`);
    expect(within(dialog).getByText("/home/me/Movies")).toBeInTheDocument();
    await act(async () => {
      fireEvent.click(within(dialog).getByRole("button", { name: "Choose export folder" }));
    });
    expect(within(dialog).getByText("/home/me/Movies")).toBeInTheDocument();

    await act(async () => {
      fireEvent.click(within(dialog).getByRole("button", { name: "Export video" }));
    });
    await waitFor(() => expect(calls("render_media_to_split_project_folder")).toHaveLength(1));
    expect(calls("render_media_to_split_project_folder")[0]?.[1]).toMatchObject({ output: { fileName: "Edison Restoration Demo", directory: "/home/me/Movies" } });
    expect(store.getState().exportPopover).toBeNull();
  });

  it("explains that choosing a folder needs the desktop app in the browser", async () => {
    installRuntimeMode("browser");
    setup();
    const dialog = await openPopover();
    const chooser = within(dialog).getByRole("button", { name: "Choose export folder" });
    expect(chooser).toHaveAttribute("aria-disabled", "true");
    expect(chooser).toHaveAccessibleDescription("Choosing a folder needs the desktop app.");
    fireEvent.click(chooser);
    expect(chooseExportDirectory).not.toHaveBeenCalled();
  });

  it("shows the saved file name, and blocks a name the backend would refuse", async () => {
    setup();
    const dialog = await openPopover();
    await waitFor(() => expect(within(dialog).getByRole("radio", { name: "MP4" })).toHaveAttribute("aria-checked", "true"));
    const name = within(dialog).getByRole("textbox", { name: "Name" });
    fireEvent.change(name, { target: { value: "Edison intro" } });
    expect(within(dialog).getByText("Saves as Edison intro.mp4")).toBeInTheDocument();

    fireEvent.change(name, { target: { value: "a/b" } });
    const exportVideo = within(dialog).getByRole("button", { name: "Export video" });
    expect(exportVideo).toHaveAttribute("aria-disabled", "true");
    expect(exportVideo).toHaveAccessibleDescription("Export names can't contain slashes.");
    expect(within(dialog).getAllByText("Export names can't contain slashes.").length).toBeGreaterThan(0);
  });

  it("renders at a chosen frame rate", async () => {
    const project = { ...fixtureProject(), schemaVersion: 2 } as VideoProject;
    setup({ render_media_to_split_project_folder: () => new Promise(() => undefined) }, { project: { ...project, renderSettings: { ...project.renderSettings, fps: 30 } } });
    const dialog = await openPopover();
    await waitFor(() => expect(within(dialog).getByText(/^H\.264 · 30 fps · ≈ /)).toBeInTheDocument());
    fireEvent.click(within(dialog).getByRole("button", { name: "Advanced" }));
    const frameRate = within(dialog).getByRole("combobox", { name: "Frame rate" });
    expect(frameRate).toHaveTextContent("Timeline (30 fps)");
    fireEvent.keyDown(frameRate, { key: "Enter" });
    fireEvent.click(screen.getByRole("option", { name: "25 fps" }));
    expect(within(dialog).getByText(/^H\.264 · 25 fps · ≈ /)).toBeInTheDocument();

    await act(async () => {
      fireEvent.click(within(dialog).getByRole("button", { name: "Export video" }));
    });
    await waitFor(() => expect(calls("render_media_to_split_project_folder")).toHaveLength(1));
    expect(calls("render_media_to_split_project_folder")[0]?.[1]).toMatchObject({ fps: 25 });
  });

  it("exports Master for MP4 and explains why ProRes has no Master", async () => {
    setup({ render_media_to_split_project_folder: () => new Promise(() => undefined) });
    const dialog = await openPopover();
    await waitFor(() => expect(within(dialog).getByRole("radio", { name: "MP4" })).toHaveAttribute("aria-checked", "true"));
    const summary = () => within(dialog).getByText(/ fps · ≈ /).textContent;
    const high = summary();
    const master = within(dialog).getByRole("radio", { name: "Master" });
    expect(master).not.toHaveAttribute("aria-disabled");
    fireEvent.click(master);
    expect(master).toHaveAttribute("aria-checked", "true");
    expect(summary()).not.toBe(high);

    fireEvent.click(within(dialog).getByRole("radio", { name: "ProRes" }));
    expect(within(dialog).getByRole("radio", { name: "Master" })).toHaveAccessibleDescription(
      "Master quality is for MP4 and WebM exports; ProRes already exports at mastering quality.",
    );
    fireEvent.click(within(dialog).getByRole("radio", { name: "MP4" }));

    await act(async () => {
      fireEvent.click(within(dialog).getByRole("button", { name: "Export video" }));
    });
    await waitFor(() => expect(calls("render_media_to_split_project_folder")).toHaveLength(1));
    expect(calls("render_media_to_split_project_folder")[0]?.[1]).toMatchObject({ quality: "final", encodeTier: "master" });
  });

  it("restores the folder, frame rate, Master and name from a retried job's settings", async () => {
    const { store } = setup();
    const settings = { profile: "mp4H264" as const, quality: "final" as const, width: 1920, height: 1080, fps: 25, encodeTier: "master" as const, output: { fileName: "Edison master", directory: "/home/me/Movies" } };
    act(() => store.getState().openExportPopover({ jobId: "export-mp4H264-9", profile: "mp4H264", quality: "final", nleFormat: null, settings }));
    const dialog = await screen.findByRole("dialog", { name: "Export" });
    await waitFor(() => expect(within(dialog).getByRole("radio", { name: "Master" })).toHaveAttribute("aria-checked", "true"));
    expect(within(dialog).getByRole("textbox", { name: "Name" })).toHaveValue("Edison master");
    expect(within(dialog).getByText("/home/me/Movies")).toBeInTheDocument();
    expect(within(dialog).getByText(/^H\.264 · 25 fps · ≈ /)).toBeInTheDocument();
  });

  it("presets its choices from a retried job", async () => {
    const { store } = setup();
    act(() => store.getState().openExportPopover({ jobId: "export-webm-1", profile: "webm", quality: "draft", nleFormat: null, settings: null }));
    const dialog = await screen.findByRole("dialog", { name: "Export" });
    await waitFor(() => expect(within(dialog).getByRole("radio", { name: "WebM" })).toHaveAttribute("aria-checked", "true"));
    expect(within(dialog).getByRole("radio", { name: "Draft" })).toHaveAttribute("aria-checked", "true");
  });
});
