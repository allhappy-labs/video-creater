import "@testing-library/jest-dom/vitest";
import { act, fireEvent, screen, within } from "@testing-library/react";
import type { ReactElement } from "react";
import { afterEach, beforeAll, beforeEach, describe, expect, it, vi } from "vitest";
import type { MediaAsset, VideoProject } from "@/lib/project";
import { openMediaFiles } from "@/lib/runtime/adapters/tauri-dialog";
import { backendRequest } from "@/lib/runtime/backend-client";
import { BackendUnavailableError } from "@/lib/runtime/backend-transport";
import { fakeDataTransfer, installDragEventPolyfill } from "@/test-utils/data-transfer";
import { renderWithEditorStore } from "@/test-utils/editor-render";
import { fixtureProject } from "@/test-utils/editor-fixtures";
import { installMediaAndFrameStubs } from "../../preview/media-element-stubs";
import { EditorEnvironmentProvider } from "../../services/editor-environment";
import { assetDragMimeType } from "../../timeline/drag-data";
import { AudioPanel } from "./audio-panel";

vi.mock("@/lib/runtime/backend-client", () => ({ backendRequest: vi.fn(), backendListen: vi.fn(), backendMediaUrl: (p: string) => p }));
vi.mock("@/lib/runtime/adapters/tauri-dialog", () => ({ openMediaFiles: vi.fn() }));

const music: MediaAsset = { id: "media-music", name: "Phonograph ambience", relativePath: "media/ambience.wav", kind: "audio", durationSeconds: 120, width: null, height: null, fps: null };

function silenceProject(): VideoProject {
  return {
    ...fixtureProject(),
    mediaSilenceRanges: [
      { mediaId: "media-1", sourceIn: 0.5, sourceOut: 1.5, confidence: 0.9 },
      { mediaId: "media-1", sourceIn: 2, sourceOut: 3.5, confidence: 0.9 },
    ],
  };
}

async function settle() {
  await act(async () => {
    await Promise.resolve();
  });
}

interface RenderOptions {
  readonly projectDir?: string;
  readonly modelReady?: boolean;
  readonly speechModelsReady?: boolean;
  readonly onOpenModelSettings?: () => void;
}

async function renderPanel(project: VideoProject = fixtureProject(), options: RenderOptions = {}) {
  const ui: ReactElement = (
    <EditorEnvironmentProvider
      transcriptionModelReady={options.modelReady ?? true}
      speechModelsReady={options.speechModelsReady ?? true}
      onOpenModelSettings={options.onOpenModelSettings ?? (() => undefined)}
    >
      <AudioPanel />
    </EditorEnvironmentProvider>
  );
  const rendered = renderWithEditorStore(ui, { project, projectDir: options.projectDir ?? "" });
  await settle();
  return rendered;
}

function allItems(project: VideoProject) {
  return project.timeline.tracks.flatMap((track) => track.items);
}

describe("AudioPanel", () => {
  beforeAll(() => installDragEventPolyfill());

  beforeEach(() => {
    window.localStorage.clear();
    vi.mocked(backendRequest).mockReset();
    vi.mocked(backendRequest).mockRejectedValue(new BackendUnavailableError());
    vi.mocked(openMediaFiles).mockReset();
    installMediaAndFrameStubs();
  });

  afterEach(() => {
    vi.unstubAllGlobals();
    vi.restoreAllMocks();
  });

  it("shows Import, and Generate opens the audio Generate view", async () => {
    const { store } = await renderPanel();
    expect(screen.getByRole("button", { name: "Import audio" })).toBeEnabled();
    fireEvent.click(screen.getByRole("button", { name: "Generate audio" }));
    await settle();
    expect(store.getState().generateView).toEqual({ open: true, mode: "audio" });
    const types = within(screen.getByRole("radiogroup", { name: "Generation type" })).getAllByRole("radio");
    expect(types.map((type) => type.textContent)).toEqual(["Music", "SFX", "Voice"]);
    fireEvent.click(screen.getByRole("button", { name: "Back to audio" }));
    expect(store.getState().generateView).toBeNull();
    expect(screen.getByRole("button", { name: "Generate audio" })).toBeInTheDocument();
  });

  describe("silence review", () => {
    it("summarizes pauses and applies only the checked ranges in one undo step", async () => {
      const { store } = await renderPanel(silenceProject());
      const card = screen.getByRole("listitem", { name: "Remove silences" });
      expect(within(card).getByText("2 pauses · saves 2s")).toBeInTheDocument();
      const durationBefore = store.getState().project.timeline.durationSeconds;

      fireEvent.click(within(card).getByRole("button", { name: "Review pauses" }));
      expect(store.getState().history.past).toHaveLength(0);
      const dialog = await screen.findByRole("dialog", { name: "Review pauses" });
      const rows = within(dialog).getAllByRole("checkbox");
      expect(rows).toHaveLength(2);
      expect(rows.every((row) => (row as HTMLInputElement).checked)).toBe(true);

      fireEvent.click(within(dialog).getByRole("button", { name: /^Preview pause 2/ }));
      expect(store.getState().playheadSeconds).toBe(2.12);

      fireEvent.click(within(dialog).getByRole("checkbox", { name: "Include pause 1" }));
      const apply = within(dialog).getByRole("button", { name: "Apply 1 cut" });
      await act(async () => {
        fireEvent.click(apply);
      });

      expect(store.getState().history.past).toHaveLength(1);
      expect(store.getState().project.timeline.durationSeconds).toBeCloseTo(durationBefore - 1.26, 3);
      expect(screen.queryByRole("dialog", { name: "Review pauses" })).not.toBeInTheDocument();
    });

    it("focuses the Remove silences review when Properties routes here, then clears the request", async () => {
      const { store } = await renderPanel(silenceProject());
      act(() => store.getState().setPendingCleanupFocus("removeSilences"));
      expect(screen.getByRole("button", { name: "Review pauses" })).toHaveFocus();
      expect(screen.getByRole("listitem", { name: "Remove silences" })).toHaveAttribute("data-highlighted", "true");
      expect(store.getState().pendingCleanupFocus).toBeNull();
    });

    it("focuses the Remove silences card itself when there is nothing to review", async () => {
      const { store } = await renderPanel();
      act(() => store.getState().setPendingCleanupFocus("removeSilences"));
      expect(screen.getByRole("listitem", { name: "Remove silences" })).toHaveFocus();
      expect(store.getState().pendingCleanupFocus).toBeNull();
    });

    it("disables Apply when every range is unchecked", async () => {
      await renderPanel(silenceProject());
      fireEvent.click(screen.getByRole("button", { name: "Review pauses" }));
      const dialog = await screen.findByRole("dialog", { name: "Review pauses" });
      for (const checkbox of within(dialog).getAllByRole("checkbox")) fireEvent.click(checkbox);
      expect(within(dialog).getByRole("button", { name: "Apply 0 cuts" })).toBeDisabled();
    });
  });

  it("gates speaker detection on the on-device speech models, not the transcription model", async () => {
    vi.mocked(backendRequest).mockImplementation(async (command: string) => {
      if (command === "get_project_speaker_registry") return { speakers: [] };
      throw new BackendUnavailableError();
    });
    await renderPanel({ ...fixtureProject(), schemaVersion: 2 }, { projectDir: "/projects/demo", modelReady: false, speechModelsReady: true });
    await settle();
    expect(screen.queryByText("Install speech models")).not.toBeInTheDocument();
    expect(screen.queryByText("Install a transcription model")).not.toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Analyze speech" })).toBeEnabled();
  });

  it("shows the missing speech models state with the settings button", async () => {
    const onOpenModelSettings = vi.fn();
    await renderPanel(fixtureProject(), { speechModelsReady: false, onOpenModelSettings });
    expect(screen.getByText("Install speech models")).toBeInTheDocument();
    // Short copy keeps the notice and the card description on one line in the narrow panel.
    expect(screen.getByText("Needed to detect speakers")).toBeInTheDocument();
    expect(within(screen.getByRole("listitem", { name: "Detect speakers" })).getByText("Save the project first")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Analyze speech" })).toBeDisabled();
    fireEvent.click(screen.getByRole("button", { name: "Open model settings" }));
    expect(onOpenModelSettings).toHaveBeenCalledTimes(1);
  });

  it("applies noise reduction to the target's audio clips", async () => {
    const { store } = await renderPanel();
    act(() => store.getState().selectItems(["music-bed"]));
    await act(async () => {
      fireEvent.click(screen.getByRole("button", { name: "Apply noise reduction" }));
    });
    expect(store.getState().history.past).toHaveLength(1);
    const audio = allItems(store.getState().project).find((item) => item.id === "music-bed");
    expect(audio?.properties.audioDenoisePreparation).toMatchObject({ status: "queued" });
    expect(screen.getByRole("button", { name: "Turn off noise reduction" })).toBeInTheDocument();
  });

  it("analyzes speakers, shows the count and renames in the speakers dialog", async () => {
    let speakers = [] as { id: string; name: string; color: string }[];
    vi.mocked(backendRequest).mockImplementation(async (command: string, input?: Record<string, unknown>) => {
      if (command === "get_project_speaker_registry") return { speakers };
      if (command === "analyze_project_speech") {
        speakers = [
          { id: "s1", name: "Speaker 1", color: "#ff5a5a" },
          { id: "s2", name: "Speaker 2", color: "#5a8bff" },
        ];
        return undefined;
      }
      if (command === "rename_project_speaker") {
        speakers = speakers.map((speaker) => (speaker.id === input?.speakerId ? { ...speaker, name: String(input.name) } : speaker));
        return { speakers };
      }
      throw new BackendUnavailableError();
    });
    const { store } = await renderPanel({ ...fixtureProject(), schemaVersion: 2 }, { projectDir: "/projects/demo" });
    const card = screen.getByRole("listitem", { name: "Detect speakers" });
    expect(within(card).getByRole("button", { name: "Rename speakers" })).toBeDisabled();

    await act(async () => {
      fireEvent.click(within(card).getByRole("button", { name: "Analyze speech" }));
    });
    await settle();
    expect(backendRequest).toHaveBeenCalledWith("analyze_project_speech", expect.objectContaining({ mediaId: "media-1" }));
    expect(within(card).getByText("2 speakers found")).toBeInTheDocument();
    expect(store.getState().lastError).toBeNull();

    fireEvent.click(within(card).getByRole("button", { name: "Rename speakers" }));
    const dialog = await screen.findByRole("dialog", { name: "Speakers" });
    const input = within(dialog).getByRole("textbox", { name: "Name for Speaker 1" });
    fireEvent.change(input, { target: { value: "Host" } });
    await act(async () => {
      fireEvent.blur(input);
    });
    expect(backendRequest).toHaveBeenCalledWith("rename_project_speaker", { projectDir: "/projects/demo", speakerId: "s1", name: "Host" });
    expect(await within(dialog).findByRole("textbox", { name: "Name for Host" })).toBeInTheDocument();
  });

  describe("audio list", () => {
    function listProject() {
      const project = fixtureProject();
      project.media.push(music);
      return project;
    }

    it("filters rows by chip and shows duration and kind", async () => {
      await renderPanel(listProject());
      const list = screen.getByRole("list", { name: "Project audio" });
      expect(within(list).getAllByRole("listitem")).toHaveLength(2);
      expect(within(list).getByText("2:00 · Audio")).toBeInTheDocument();
      fireEvent.click(screen.getByRole("button", { name: "Voice" }));
      expect(screen.getByText("No voice audio")).toBeInTheDocument();
    });

    it("plays one row at a time through a single audio element", async () => {
      const { container } = await renderPanel(listProject(), { projectDir: "/p" });
      expect(container.querySelectorAll("audio")).toHaveLength(1);
      const audio = container.querySelector("audio") as HTMLAudioElement;

      await act(async () => {
        fireEvent.click(screen.getByRole("button", { name: "Play voiceover.m4a" }));
      });
      expect(audio.getAttribute("src")).toBe("/p/media/voiceover.m4a");
      expect(screen.getByRole("button", { name: "Pause voiceover.m4a" })).toBeInTheDocument();

      await act(async () => {
        fireEvent.click(screen.getByRole("button", { name: "Play Phonograph ambience" }));
      });
      expect(audio.getAttribute("src")).toBe("/p/media/ambience.wav");
      expect(screen.getByRole("button", { name: "Play voiceover.m4a" })).toBeInTheDocument();
      expect(screen.getByRole("button", { name: "Pause Phonograph ambience" })).toBeInTheDocument();

      fireEvent.click(screen.getByRole("button", { name: "Pause Phonograph ambience" }));
      expect(audio.paused).toBe(true);
      expect(screen.getByRole("button", { name: "Play Phonograph ambience" })).toBeInTheDocument();
    });

    it("inserts on an audio track at the playhead, creating one when needed", async () => {
      const project = listProject();
      project.timeline.tracks = project.timeline.tracks.filter((track) => track.kind !== "audio");
      const { store } = await renderPanel(project);
      act(() => store.getState().seek(1));

      await act(async () => {
        fireEvent.click(screen.getByRole("button", { name: "Add Phonograph ambience to the timeline" }));
      });

      const audioTracks = store.getState().project.timeline.tracks.filter((track) => track.kind === "audio");
      expect(audioTracks).toHaveLength(1);
      expect(audioTracks[0]?.items).toEqual([
        expect.objectContaining({ kind: "audio_clip", startSeconds: 1, source: { type: "media", mediaId: "media-music" } }),
      ]);
    });

    it("drags rows as media assets", async () => {
      await renderPanel(listProject());
      const row = screen.getByRole("button", { name: "Play Phonograph ambience" }).closest("li");
      const dataTransfer = fakeDataTransfer();
      fireEvent.dragStart(row as HTMLElement, { dataTransfer });
      expect(JSON.parse(dataTransfer.getData(assetDragMimeType))).toEqual({ kind: "media", id: "media-music" });
    });
  });

  describe("import", () => {
    it("opens the audio file chooser and adds the imported media", async () => {
      vi.mocked(openMediaFiles).mockResolvedValue(["/tmp/ambience.wav"]);
      vi.mocked(backendRequest).mockImplementation(async (command: string, input?: Record<string, unknown>) => {
        if (command !== "import_media_to_project") throw new BackendUnavailableError();
        const project = input?.project as VideoProject;
        return { project: { ...project, media: [...project.media, music] }, imported: [music], skipped: [{ sourcePath: "/tmp/notes.txt", reason: "unsupported" }] };
      });
      const { store } = await renderPanel(fixtureProject(), { projectDir: "/projects/demo" });

      await act(async () => {
        fireEvent.click(screen.getByRole("button", { name: "Import audio" }));
      });

      expect(openMediaFiles).toHaveBeenCalledWith({ title: "Import audio", filters: [{ name: "Audio", extensions: expect.arrayContaining(["wav", "mp3", "m4a"]) }] });
      expect(backendRequest).toHaveBeenCalledWith("import_media_to_project", expect.objectContaining({ projectDir: "/projects/demo", sourcePaths: ["/tmp/ambience.wav"] }));
      expect(store.getState().project.media.map((media) => media.id)).toContain("media-music");
      expect(screen.getByRole("status")).toHaveTextContent("notes.txt: unsupported");
    });

    it("explains that import needs the desktop app", async () => {
      vi.mocked(openMediaFiles).mockRejectedValue(new BackendUnavailableError());
      const { store } = await renderPanel();
      await act(async () => {
        fireEvent.click(screen.getByRole("button", { name: "Import audio" }));
      });
      expect(store.getState().lastError).toBe("Import needs the desktop app");
    });
  });
});
