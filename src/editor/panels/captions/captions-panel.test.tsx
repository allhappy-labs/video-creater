import "@testing-library/jest-dom/vitest";
import { act, fireEvent, screen, within } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { ProjectAction, ProjectJobSummary, VideoProject } from "@/lib/project";
import { backendRequest } from "@/lib/runtime/backend-client";
import { BackendUnavailableError } from "@/lib/runtime/backend-transport";
import { renderWithEditorStore } from "@/test-utils/editor-render";
import { fixtureProject } from "@/test-utils/editor-fixtures";
import { EditorEnvironmentProvider } from "../../services/editor-environment";
import type { EditorStore } from "../../store/editor-store";
import { CaptionsPanel } from "./captions-panel";

vi.mock("@/lib/runtime/backend-client", () => ({ backendRequest: vi.fn(), backendListen: vi.fn(), backendMediaUrl: (p: string) => p }));

type Handler = (input: Record<string, unknown>) => unknown;

function mockBackend(handlers: Record<string, Handler>) {
  vi.mocked(backendRequest).mockImplementation(async (command: string, input?: Record<string, unknown>) => {
    const handler = handlers[command];
    if (!handler) throw new BackendUnavailableError();
    return handler(input ?? {});
  });
}

function mockTranscriptionStart() {
  mockBackend({
    build_temporal_job_summary: (input) => ({ id: input.jobId, kind: input.kind, status: "queued", updatedAt: input.updatedAt }),
    build_temporal_transcribe_media_start_request: (input) => ({
      workflowId: "wf",
      workflowType: "VideoCreaterTranscribeMediaWorkflow",
      taskQueue: "q",
      input,
      searchAttributes: {},
      activityTypes: [],
      idReusePolicy: "rejectDuplicate",
    }),
    start_temporal_workflow: () => ({ status: "started", workflowId: "wf", workflowType: "t", taskQueue: "q", runId: "run-1", message: "" }),
    build_temporal_start_result_action: (input) => ({
      type: "updateJobStatus",
      jobId: (input.job as ProjectJobSummary).id,
      status: "running",
      updatedAt: input.updatedAt,
      runId: input.runId,
    }),
  });
}

async function settle() {
  await act(async () => {
    await Promise.resolve();
  });
}

async function renderPanel(project: VideoProject = fixtureProject(), options: { modelReady?: boolean; onOpenModelSettings?: () => void } = {}) {
  const rendered = renderWithEditorStore(
    <EditorEnvironmentProvider transcriptionModelReady={options.modelReady ?? true} onOpenModelSettings={options.onOpenModelSettings ?? (() => undefined)}>
      <CaptionsPanel />
    </EditorEnvironmentProvider>,
    { project, projectDir: "/projects/demo" },
  );
  const batches: ProjectAction[][] = [];
  const applyActions = rendered.store.getState().applyActions;
  rendered.store.setState({
    applyActions: (actions, applyOptions) => {
      batches.push([...actions]);
      return applyActions(actions, applyOptions);
    },
  });
  await settle();
  return { ...rendered, batches };
}

function captions(store: EditorStore) {
  return store.getState().project.timeline.tracks.flatMap((track) => track.items).filter((item) => item.kind === "caption");
}

function captionText(store: EditorStore, itemId: string) {
  const item = captions(store).find((candidate) => candidate.id === itemId);
  return item?.source.type === "text" ? item.source.text : null;
}

function word(text: string) {
  return within(screen.getByRole("group", { name: "Transcript" })).getByRole("button", { name: text });
}

describe("CaptionsPanel", () => {
  beforeEach(() => {
    window.localStorage.clear();
    vi.mocked(backendRequest).mockReset();
    vi.mocked(backendRequest).mockRejectedValue(new BackendUnavailableError());
  });

  afterEach(() => {
    vi.restoreAllMocks();
  });

  describe("generate captions", () => {
    it("transcribes first, then builds the captions when the transcript loads", async () => {
      mockTranscriptionStart();
      const sample = fixtureProject();
      const { store, batches } = await renderPanel({ ...sample, transcripts: [] });

      expect(screen.getByRole("heading", { name: "Generate captions" })).toBeInTheDocument();
      await act(async () => {
        fireEvent.click(screen.getByRole("button", { name: "Generate captions" }));
      });

      expect(backendRequest).toHaveBeenCalledWith("build_temporal_transcribe_media_start_request", expect.objectContaining({ mediaId: "media-1", languageMode: "auto" }));
      expect(screen.getByRole("button", { name: "Transcribing…" })).toBeDisabled();
      const captionsBefore = captions(store).length;
      const batchesBefore = batches.length;

      // The Temporal job completes and the project reload brings the transcript.
      const reloaded = store.getState().project;
      await act(async () => {
        store.getState().replaceProject({
          ...reloaded,
          transcripts: sample.transcripts,
          jobs: reloaded.jobs.map((job) => (job.kind === "transcribe_media" ? { ...job, status: "completed" } : job)),
        });
        await Promise.resolve();
      });
      await settle();

      expect(batches.slice(batchesBefore)).toHaveLength(1);
      const built = captions(store).filter((item) => store.getState().selectedItemIds.includes(item.id));
      expect(captions(store)).toHaveLength(captionsBefore + 1);
      expect(built.map((item) => item.source)).toEqual([{ type: "text", text: "Original caption Second split" }]);
      expect(screen.getByRole("group", { name: "Transcript" })).toBeInTheDocument();
    });

    it("builds directly without transcribing when the transcript already exists", async () => {
      const project = fixtureProject();
      project.timeline.tracks = project.timeline.tracks.map((track) => (track.kind === "caption" ? { ...track, items: [] } : track));
      const { store, batches } = await renderPanel(project);

      expect(screen.getByText("0 captions")).toBeInTheDocument();
      await act(async () => {
        fireEvent.click(screen.getByRole("button", { name: "Generate captions" }));
      });

      expect(batches).toHaveLength(1);
      expect(backendRequest).not.toHaveBeenCalledWith("build_temporal_transcribe_media_start_request", expect.anything());
      expect(captions(store)).toHaveLength(1);
      expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
    });

    it("shows the install notice and blocks generation without a transcription model", async () => {
      const onOpenModelSettings = vi.fn();
      await renderPanel({ ...fixtureProject(), transcripts: [] }, { modelReady: false, onOpenModelSettings });

      expect(screen.getByText("Install a transcription model")).toBeInTheDocument();
      expect(screen.getByRole("button", { name: "Generate captions" })).toBeDisabled();
      fireEvent.click(screen.getByRole("button", { name: "Open model settings" }));
      expect(onOpenModelSettings).toHaveBeenCalledTimes(1);
    });

    it("reports a failed transcription job", async () => {
      const project = { ...fixtureProject(), transcripts: [] };
      project.jobs = [...project.jobs, { id: "transcribe-media-1-x", kind: "transcribe_media", status: "failed", updatedAt: "2026-09-14T00:00:00Z" }];
      await renderPanel(project);
      expect(screen.getByRole("alert")).toHaveTextContent("Transcription failed. Try again.");
      expect(screen.getByRole("button", { name: "Generate captions" })).toBeEnabled();
    });
  });

  describe("transcript", () => {
    it("renders word buttons with indices, highlights the current word, and seeks on click", async () => {
      const { store } = await renderPanel();

      expect(word("Second")).toHaveAttribute("data-word-index", "2");
      act(() => store.getState().seek(2.3));
      expect(word("Second")).toHaveAttribute("aria-current", "true");
      expect(word("Original")).not.toHaveAttribute("aria-current");

      fireEvent.click(word("caption"));
      expect(store.getState().playheadSeconds).toBe(1.1);
      expect(store.getState().transcriptRange).toBeNull();
    });

    it("marks low-confidence words, shows pauses, and counts words to check", async () => {
      const project = fixtureProject();
      const [transcript] = project.transcripts;
      if (!transcript) throw new Error("fixture transcript");
      transcript.words = transcript.words.map((entry, index) => (index === 3 ? { ...entry, confidence: 0.3, startSeconds: 3.4, endSeconds: 3.9 } : entry));
      await renderPanel(project);

      expect(word("split")).toHaveAccessibleDescription("Low confidence — double-click to fix");
      expect(word("split").className).toContain("decoration-wavy");
      expect(screen.getByRole("img", { name: "Pause 0.7s" })).toBeInTheDocument();
      expect(screen.getByText("2 captions · 1 word to check")).toBeInTheDocument();
    });

    it("fixes a word in one batch that updates the transcript and the caption text", async () => {
      const { store, batches } = await renderPanel();

      fireEvent.doubleClick(word("Original"));
      const input = screen.getByRole("textbox", { name: "Fix word “Original”" });
      fireEvent.change(input, { target: { value: "Restored" } });
      await act(async () => {
        fireEvent.keyDown(input, { key: "Enter" });
      });

      expect(batches).toHaveLength(1);
      expect(batches[0]?.map((action) => action.type)).toEqual(["editTranscriptWords", "editCaptionText"]);
      expect(batches[0]?.[0]).toMatchObject({ edits: [{ transcriptId: "transcript-media-1", wordIndex: 0, text: "Restored" }] });
      expect(store.getState().history.past).toHaveLength(1);
      expect(captionText(store, "caption-1")).toBe("Restored caption text");
      expect(screen.queryByRole("textbox")).not.toBeInTheDocument();
      expect(document.activeElement).toHaveAttribute("data-word-index", "0");
    });

    it("moves between words with arrow keys, opens the editor with F2 and cancels with Escape", async () => {
      const { batches } = await renderPanel();

      const first = word("Original");
      expect(first).toHaveAttribute("tabindex", "0");
      expect(word("caption")).toHaveAttribute("tabindex", "-1");
      act(() => first.focus());
      fireEvent.keyDown(first, { key: "ArrowRight" });
      expect(document.activeElement).toBe(word("caption"));

      fireEvent.keyDown(word("caption"), { key: "F2" });
      const input = screen.getByRole("textbox", { name: "Fix word “caption”" });
      expect(document.activeElement).toBe(input);
      fireEvent.keyDown(input, { key: "Escape" });

      expect(screen.queryByRole("textbox")).not.toBeInTheDocument();
      expect(document.activeElement).toBe(word("caption"));
      expect(batches).toHaveLength(0);
    });

    it("asks before regenerating existing captions and replaces them in one batch", async () => {
      const { store, batches } = await renderPanel();

      fireEvent.click(screen.getByRole("button", { name: "Regenerate" }));
      const dialog = await screen.findByRole("dialog", { name: "Regenerate 2 captions?" });
      expect(batches).toHaveLength(0);

      await act(async () => {
        fireEvent.click(within(dialog).getByRole("button", { name: "Regenerate" }));
      });

      expect(batches).toHaveLength(1);
      expect(batches[0]?.[0]).toEqual({ type: "removeItems", itemIds: ["caption-1", "caption-2"] });
      expect(store.getState().history.past).toHaveLength(1);
      expect(captions(store).map((item) => item.source)).toEqual([{ type: "text", text: "Original caption Second split" }]);
      expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
    });
  });

  it("applies a style preset to every caption in one batch", async () => {
    const { store, batches } = await renderPanel();

    fireEvent.click(screen.getByRole("radio", { name: "Styles" }));
    expect(screen.getByText("Applies to all 2 captions.")).toBeInTheDocument();
    await act(async () => {
      fireEvent.click(screen.getByRole("button", { name: "Kinetic focus" }));
    });

    expect(batches).toHaveLength(1);
    expect(store.getState().history.past).toHaveLength(1);
    expect(captions(store).map((item) => item.properties.stylePreset)).toEqual(["kineticFocus", "kineticFocus"]);
    expect(screen.getByRole("button", { name: "Kinetic focus" })).toHaveAttribute("aria-pressed", "true");
  });
});
