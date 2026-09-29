import "@testing-library/jest-dom/vitest";
import { act, fireEvent, screen, within } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { temporalCancelReason } from "@/lib/jobs/task-records";
import type { ProjectJobSummary, VideoProject } from "@/lib/project";
import { backendRequest } from "@/lib/runtime/backend-client";
import { BackendUnavailableError } from "@/lib/runtime/backend-transport";
import { installRuntimeMode } from "@/lib/runtime/runtime-mode";
import { fixtureProject } from "@/test-utils/editor-fixtures";
import { renderWithEditorStore } from "@/test-utils/editor-render";
import { TasksIndicator } from "./tasks-indicator";

vi.mock("@/lib/runtime/backend-client", () => ({ backendRequest: vi.fn(), backendListen: vi.fn(), backendMediaUrl: (p: string) => p }));
const revealExportArtifact = vi.hoisted(() => vi.fn(async (_path: string) => true));
vi.mock("../services/reveal-service", () => ({ useRevealService: () => ({ revealExportArtifact }) }));

const minutesAgo = (minutes: number) => new Date(Date.now() - minutes * 60_000).toISOString();

function job(id: string, kind: string, status: ProjectJobSummary["status"], options: { readonly runId?: string; readonly updatedAt?: string } = {}): ProjectJobSummary {
  return {
    id,
    kind,
    status,
    updatedAt: options.updatedAt ?? minutesAgo(1),
    ...(options.runId
      ? { workflow: { workflowId: `video-creater/demo/${id}`, workflowType: "TranscribeMedia", taskQueue: "video-creater-workflows", runId: options.runId, activityTypes: [] } }
      : {}),
  };
}

function project(jobs: readonly ProjectJobSummary[], renderReports: VideoProject["renderReports"] = []): VideoProject {
  return { ...fixtureProject(), schemaVersion: 2, jobs: [...jobs], generatedAssets: [], renderReports, exportArtifacts: [] };
}

function setup(jobs: readonly ProjectJobSummary[], options: { readonly compact?: boolean; readonly renderReports?: VideoProject["renderReports"] } = {}) {
  return renderWithEditorStore(<TasksIndicator compact={options.compact ?? false} />, { project: project(jobs, options.renderReports), projectDir: "/projects/demo" });
}

async function openTasks() {
  fireEvent.click(screen.getByRole("button", { name: "Background tasks" }));
  return screen.findByRole("dialog", { name: "Background tasks" });
}

describe("TasksIndicator", () => {
  beforeEach(() => {
    installRuntimeMode("desktop");
    window.localStorage.clear();
    vi.mocked(backendRequest).mockReset();
    vi.mocked(backendRequest).mockRejectedValue(new BackendUnavailableError());
    revealExportArtifact.mockClear();
  });

  it("is hidden without tasks and after the last completion is more than 10 minutes old", () => {
    const { unmount } = setup([]);
    expect(screen.queryByRole("button", { name: "Background tasks" })).not.toBeInTheDocument();
    unmount();
    setup([job("render-1", "render_draft", "completed", { updatedAt: minutesAgo(11) })]);
    expect(screen.queryByRole("button", { name: "Background tasks" })).not.toBeInTheDocument();
  });

  it("shows a spinner and the running task's label", () => {
    setup([job("transcribe-1", "transcribe_media", "running", { runId: "run-1" })]);
    const button = screen.getByRole("button", { name: "Background tasks" });
    expect(button).toHaveTextContent("Transcribing…");
    expect(button).toHaveAccessibleDescription("Transcribing…");
    expect(within(button).getByTestId("tasks-spinner")).toBeInTheDocument();
  });

  it("shows a failed dot and text when the newest task failed", () => {
    setup([job("render-1", "render_draft", "failed"), job("render-0", "render_draft", "completed", { updatedAt: minutesAgo(5) })]);
    const button = screen.getByRole("button", { name: "Background tasks" });
    expect(button).toHaveTextContent("Render failed");
    expect(within(button).getByTestId("tasks-failed-dot")).toBeInTheDocument();
    expect(within(button).queryByTestId("tasks-spinner")).not.toBeInTheDocument();
  });

  it("compact mode shows a spinner and percent and opens the list as a bottom sheet", async () => {
    const { store } = setup([job("transcribe-1", "transcribe_media", "running", { runId: "run-1" })], { compact: true });
    // Record the derived inputs first, so the progress override isn't re-derived away. No backend emits progress yet.
    store.getState().syncJobs();
    act(() => store.setState({ tasks: store.getState().tasks.map((task) => ({ ...task, progress: 0.62 })) }));
    const button = screen.getByRole("button", { name: "Background tasks" });
    expect(button).toHaveTextContent("62%");
    expect(button).toHaveAccessibleDescription("Transcribing · 62%");
    const sheet = await openTasks();
    expect(within(sheet).getByRole("progressbar", { name: "Transcribe media progress" })).toHaveAttribute("aria-valuenow", "62");
  });

  it("lists every task with plain failure copy and Retry calling the jobs slice", async () => {
    const { store } = setup([job("render-1", "render_draft", "failed"), job("transcribe-1", "transcribe_media", "running", { runId: "run-1" })]);
    const retryTask = vi.fn(async () => true);
    store.setState({ retryTask });
    const popover = await openTasks();
    const rows = within(popover).getAllByRole("listitem");
    expect(rows).toHaveLength(2);
    const failed = within(popover).getByRole("listitem", { name: "Timeline render" });
    expect(within(failed).getByText("The render stopped before it finished.")).toBeInTheDocument();
    fireEvent.click(within(failed).getByRole("button", { name: "Retry" }));
    expect(retryTask).toHaveBeenCalledWith("render-1");
    expect(within(failed).queryByRole("button", { name: "Cancel" })).not.toBeInTheDocument();
  });

  it("shows a disabled Cancel with the workflow worker reason on Temporal rows", async () => {
    const { store } = setup([job("transcribe-1", "transcribe_media", "running", { runId: "run-1" })]);
    const cancelTask = vi.fn(async () => true);
    store.setState({ cancelTask });
    const popover = await openTasks();
    const cancel = within(popover).getByRole("button", { name: "Cancel" });
    expect(cancel).toHaveAttribute("aria-disabled", "true");
    expect(cancel).toHaveAccessibleDescription(temporalCancelReason);
    fireEvent.click(cancel);
    expect(cancelTask).not.toHaveBeenCalled();
    expect(within(popover).queryByRole("button", { name: "Retry" })).not.toBeInTheDocument();
  });

  it("Cancel calls the jobs slice when the record can be cancelled", async () => {
    const { store } = setup([]);
    const cancelTask = vi.fn(async () => true);
    act(() => {
      store.getState().setActiveRender({ jobId: "render-live", attemptId: "render-attempt/1", startedAt: new Date().toISOString() });
      store.setState({ cancelTask });
    });
    const popover = await openTasks();
    const cancel = within(popover).getByRole("button", { name: "Cancel" });
    expect(cancel).not.toHaveAttribute("aria-disabled");
    fireEvent.click(cancel);
    expect(cancelTask).toHaveBeenCalledWith("render-live");
  });

  it("Show and Log reveal the recorded render output and log through the reveal service", async () => {
    const report = { id: "render-1", outputPath: "renders/render-1/output.webm", logPath: "renders/render-1/render.log" } as VideoProject["renderReports"][number];
    setup([job("render-1", "render_draft", "completed")], { renderReports: [report] });
    const row = within(await openTasks()).getByRole("listitem", { name: "Timeline render" });
    fireEvent.click(within(row).getByRole("button", { name: "Show" }));
    expect(revealExportArtifact).toHaveBeenCalledWith("renders/render-1/output.webm");
    fireEvent.click(within(row).getByRole("button", { name: "Log" }));
    expect(revealExportArtifact).toHaveBeenLastCalledWith("renders/render-1/render.log");
  });

  it("downloads completed artifacts and omits host-only log reveal in a browser", async () => {
    installRuntimeMode("browser");
    const report = { id: "render-1", outputPath: "renders/render-1/output.webm", logPath: "renders/render-1/render.log" } as VideoProject["renderReports"][number];
    setup([job("render-1", "render_draft", "completed")], { renderReports: [report] });
    const row = within(await openTasks()).getByRole("listitem", { name: "Timeline render" });
    fireEvent.click(within(row).getByRole("button", { name: "Download" }));
    expect(revealExportArtifact).toHaveBeenCalledWith("renders/render-1/output.webm");
    expect(within(row).queryByRole("button", { name: "Log" })).not.toBeInTheDocument();
  });

  it("offers Log on a failed render with a report but Show only for a completed one, and neither without paths", async () => {
    const report = { id: "render-1", outputPath: "renders/render-1/output.webm", logPath: "renders/render-1/render.log" } as VideoProject["renderReports"][number];
    setup([job("render-1", "render_draft", "failed"), job("transcribe-1", "transcribe_media", "completed")], { renderReports: [report] });
    const popover = await openTasks();
    const failed = within(popover).getByRole("listitem", { name: "Timeline render" });
    expect(within(failed).queryByRole("button", { name: "Show" })).not.toBeInTheDocument();
    expect(within(failed).getByRole("button", { name: "Log" })).toBeInTheDocument();
    const transcription = within(popover).getByRole("listitem", { name: "Transcribe media" });
    expect(within(transcription).queryByRole("button", { name: "Show" })).not.toBeInTheDocument();
    expect(within(transcription).queryByRole("button", { name: "Log" })).not.toBeInTheDocument();
  });

  it("Details closes the list and opens the task details", async () => {
    const { store } = setup([job("render-1", "render_draft", "failed")]);
    const popover = await openTasks();
    fireEvent.click(within(popover).getByRole("button", { name: "Details" }));
    expect(store.getState().taskDetailsId).toBe("render-1");
    expect(await screen.findByRole("dialog", { name: "Task details" })).toBeInTheDocument();
    expect(screen.queryByRole("dialog", { name: "Background tasks" })).not.toBeInTheDocument();
  });
});
