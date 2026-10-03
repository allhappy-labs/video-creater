import "@testing-library/jest-dom/vitest";
import { act, fireEvent, screen, within } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { PreparedProjectPreview, VideoProject } from "@/lib/project";
import { BackendUnavailableError } from "@/lib/runtime/backend-transport";
import { renderWithEditorStore } from "@/test-utils/editor-render";
import { fixtureItem, fixtureProject } from "@/test-utils/editor-fixtures";
import type { TransitionKind } from "@/lib/timeline";
import { crossfade, transitionTestProject } from "../timeline/transition-test-project";

const backendRequest = vi.fn();
vi.mock("@/lib/runtime/backend-client", () => ({
  backendRequest: (...args: unknown[]) => backendRequest(...args),
  backendListen: vi.fn(),
  backendMediaUrl: (p: string) => p,
}));

const { PreviewPanel } = await import("./preview-panel");
const { installMediaAndFrameStubs } = await import("./media-element-stubs");

let stubs: ReturnType<typeof installMediaAndFrameStubs>;

function renderPreview(project: VideoProject = fixtureProject(), projectDir = "/p") {
  return renderWithEditorStore(<PreviewPanel />, { project, projectDir });
}

function viewport() {
  return screen.getByRole("region", { name: "Preview viewport" });
}

/** The sample video clip with a richer blend mode, which needs canonical preparation. */
function canonicalProject(): VideoProject {
  const project = fixtureProject();
  fixtureItem(project, "video").properties.blendMode = "add";
  return project;
}

async function flushPromises() {
  await act(async () => {
    await Promise.resolve();
    await Promise.resolve();
  });
}

describe("PreviewPanel composition", () => {
  beforeEach(() => {
    window.localStorage.clear();
    backendRequest.mockReset();
    stubs = installMediaAndFrameStubs();
  });

  afterEach(() => {
    vi.useRealTimers();
    vi.unstubAllGlobals();
    vi.restoreAllMocks();
  });

  it("keeps the preview landmarks and renders the fixture layers at t=0", () => {
    renderPreview();
    expect(screen.getByRole("region", { name: "Preview panel" })).toBeInTheDocument();
    expect(screen.getByRole("group", { name: "Preview transport" })).toBeInTheDocument();
    const video = within(viewport()).getByLabelText("Timeline video Opening clip");
    expect(video.tagName).toBe("VIDEO");
    expect(video).toHaveAttribute("src", "/p/media/input.mp4");
    expect((video as HTMLVideoElement).muted).toBe(true);
    const audio = within(viewport()).getByLabelText("Timeline audio Music bed");
    expect(audio.tagName).toBe("AUDIO");
    expect(audio).toHaveClass("hidden");
    expect(within(viewport()).queryByLabelText(/Timeline preview caption/)).not.toBeInTheDocument();
    expect(screen.queryByRole("alert")).not.toBeInTheDocument();
  });

  it("renders the caption active at t=3 and seeks paused media to the source time", () => {
    const { store } = renderPreview();
    act(() => store.getState().seek(3));
    expect(within(viewport()).getByLabelText("Timeline preview caption Caption 2")).toHaveTextContent("Second clean split");
    const video = within(viewport()).getByLabelText("Timeline video Opening clip") as HTMLVideoElement;
    expect(video.currentTime).toBe(3);
  });

  it("marks each caption layer with its style preset", () => {
    const project = fixtureProject();
    const { store } = renderPreview(project);
    act(() => store.getState().seek(3));
    expect(within(viewport()).getByLabelText("Timeline preview caption Caption 2")).toHaveAttribute("data-caption-style", "boldReadableLower");

    const restyled = structuredClone(project);
    for (const item of restyled.timeline.tracks.flatMap((track) => track.items)) {
      if (item.kind === "caption") item.properties.stylePreset = "kineticFocus";
    }
    act(() => store.getState().replaceProject(restyled));
    expect(within(viewport()).getByLabelText("Timeline preview caption Caption 2")).toHaveAttribute("data-caption-style", "kineticFocus");
  });

  it("scales a clip's output-pixel position to the preview canvas", () => {
    const project = fixtureProject();
    fixtureItem(project, "video").properties.positionX = 480;
    renderPreview(project);
    // 480 of the 1920 px render width is a quarter of the canvas width.
    expect(within(viewport()).getByLabelText("Timeline video Opening clip").style.transform).toBe(
      "translate(25cqw, 0cqw) scale(1) rotate(0deg)",
    );
  });

  it("applies a live property preview to the layer without changing the project", () => {
    const { store } = renderPreview();
    const project = store.getState().project;
    const video = () => within(viewport()).getByLabelText("Timeline video Opening clip");
    expect(video().style.opacity).toBe("1");
    act(() => store.getState().setPropertyPreview({ itemId: "item-1", patch: { opacity: 0.3 } }));
    expect(video().style.opacity).toBe("0.3");
    expect(store.getState().project).toBe(project);
    expect(fixtureItem(project, "video").properties.opacity).toBeUndefined();
    act(() => store.getState().clearPropertyPreview());
    expect(video().style.opacity).toBe("1");
  });

  it("shows the empty state after the last clip", () => {
    const project = fixtureProject();
    project.timeline.durationSeconds = 12;
    const { store } = renderPreview(project);
    act(() => store.getState().seek(10));
    expect(within(viewport()).getByText("No timeline media at playhead")).toBeInTheDocument();
  });

  it("advances the playhead from animation frames and plays media while playing", () => {
    const { store } = renderPreview();
    act(() => store.getState().togglePlaying());
    expect(stubs.play).toHaveBeenCalled();
    stubs.runFrame(1000);
    stubs.runFrame(1500);
    expect(store.getState().playheadSeconds).toBeCloseTo(0.5, 6);

    // Time spent hidden is not skipped over.
    act(() => document.dispatchEvent(new Event("visibilitychange")));
    stubs.runFrame(9000);
    stubs.runFrame(9250);
    expect(store.getState().playheadSeconds).toBeCloseTo(0.75, 6);

    act(() => store.getState().togglePlaying());
    expect(stubs.pause).toHaveBeenCalled();
    expect(stubs.pendingFrameCount()).toBe(0);
  });

  it("stops at the end of the timeline", () => {
    const project = fixtureProject();
    const { store } = renderPreview(project);
    act(() => store.getState().seek(project.timeline.durationSeconds - 0.1));
    act(() => store.getState().togglePlaying());
    stubs.runFrame(0);
    stubs.runFrame(500);
    expect(store.getState()).toMatchObject({ playing: false, playheadSeconds: project.timeline.durationSeconds });
    expect(stubs.pendingFrameCount()).toBe(0);
  });

  it("holds the playhead while remote video is buffering and resumes without skipping", () => {
    let ready = 0;
    vi.spyOn(HTMLMediaElement.prototype, "readyState", "get").mockImplementation(() => ready);
    const { store } = renderPreview();
    const media = Array.from(viewport().querySelectorAll<HTMLMediaElement>("video, audio"));
    act(() => store.getState().togglePlaying());
    stubs.runFrame(1000);
    stubs.runFrame(4000);
    expect(store.getState().playheadSeconds).toBe(0);
    expect(store.getState().playing).toBe(true);
    expect(media.map((element) => element.paused)).toEqual([true, true]);
    ready = 4;
    stubs.runFrame(5000);
    expect(media.map((element) => element.paused)).toEqual([false, false]);
    stubs.runFrame(5500);
    expect(store.getState().playheadSeconds).toBeCloseTo(0.5, 6);
    ready = 2; // A decoded current frame without future data is a mid-playback stall.
    stubs.runFrame(6000);
    stubs.runFrame(9000);
    expect(store.getState().playheadSeconds).toBeCloseTo(0.5, 6);
    expect(media.map((element) => element.paused)).toEqual([true, true]);
    ready = 4;
    stubs.runFrame(10000);
    expect(media.map((element) => element.paused)).toEqual([false, false]);
    stubs.runFrame(10500);
    expect(store.getState().playheadSeconds).toBeCloseTo(1, 6);
  });

  it("marks a failed layer and reloads it on Retry preview", () => {
    renderPreview();
    fireEvent.error(within(viewport()).getByLabelText("Timeline video Opening clip"));
    const alert = screen.getByRole("alert", { name: "Preview issue" });
    expect(alert).toHaveTextContent("Preview failed");
    expect(alert).toHaveTextContent("Timeline item item-1 failed to load.");
    expect(screen.getByText("Preview skipped 1 failed timeline layer.")).toBeInTheDocument();
    fireEvent.click(within(alert).getByRole("button", { name: "Retry preview" }));
    expect(within(viewport()).getByLabelText("Timeline video Opening clip")).toBeInTheDocument();
    expect(screen.queryByRole("alert")).not.toBeInTheDocument();
  });

  for (const interruption of ["pause", "another stall"] as const) {
    it(`does not restart media when a buffering resume settles after ${interruption}`, async () => {
      let ready = 4;
      vi.spyOn(HTMLMediaElement.prototype, "readyState", "get").mockImplementation(() => ready);
      const { store } = renderPreview();
      const media = Array.from(viewport().querySelectorAll<HTMLMediaElement>("video, audio"));
      act(() => store.getState().setPlaying(true));
      await flushPromises();
      ready = 2;
      stubs.runFrame(1000);
      const start = stubs.play.getMockImplementation()!;
      const settle: (() => void)[] = [];
      stubs.play.mockImplementation(function (this: HTMLMediaElement) {
        return new Promise<void>((resolve) => {
          settle.push(() => { void start.call(this); resolve(); });
        });
      });
      ready = 4;
      stubs.runFrame(2000);
      expect(settle).toHaveLength(2);
      if (interruption === "pause") act(() => store.getState().setPlaying(false));
      else { ready = 2; stubs.runFrame(2100); }
      await act(async () => { settle.forEach((complete) => complete()); });
      expect(media.map((element) => element.paused)).toEqual([true, true]);
      expect(store.getState().playing).toBe(interruption !== "pause");
    });
  }

  it("opens the failed layer's media in asset preview from Open source", () => {
    const { store } = renderPreview();
    fireEvent.error(within(viewport()).getByLabelText("Timeline video Opening clip"));
    fireEvent.click(screen.getByRole("button", { name: "Open source" }));
    expect(store.getState()).toMatchObject({ selectedItemIds: ["item-1"], previewSource: { kind: "asset", mediaId: "media-1" } });
  });
});

describe("PreviewPanel transitions", () => {
  // "a" (0-2 s, source 0-2 s) and "b" (2-4 s, source 2-4 s) of the 4 s input.mp4, with a 1 s
  // transition spanning [1.5, 2.5).
  function renderTransition(kind: TransitionKind) {
    const project = transitionTestProject(undefined, [{ ...crossfade(1), kind }]);
    return renderPreview(project);
  }

  const video = (label: string) => within(viewport()).getByLabelText(`Timeline video ${label}`) as HTMLVideoElement;
  const layerFrame = (label: string) => video(label).closest<HTMLElement>("[data-testid='preview-layer']");

  beforeEach(() => {
    window.localStorage.clear();
    backendRequest.mockReset();
    stubs = installMediaAndFrameStubs();
  });

  afterEach(() => {
    vi.unstubAllGlobals();
    vi.restoreAllMocks();
  });

  it("renders both clips during a crossfade with the incoming clip on top at opacity p", () => {
    const { store } = renderTransition("crossfade");
    act(() => store.getState().seek(1.4));
    expect(within(viewport()).queryByLabelText("Timeline video b")).not.toBeInTheDocument();

    act(() => store.getState().seek(2));
    const frames = within(viewport()).getAllByTestId("preview-layer");
    expect(frames.map((frame) => [frame.dataset.itemId, frame.dataset.transitionRole, frame.dataset.transitionProgress])).toEqual([
      ["a", "outgoing", "0.5"],
      ["b", "incoming", "0.5"],
    ]);
    expect(video("a").style.opacity).toBe("1");
    expect(video("b").style.opacity).toBe("0.5");
    expect(screen.queryByTestId("preview-transition-solid")).not.toBeInTheDocument();

    act(() => store.getState().seek(2.5));
    expect(within(viewport()).queryByLabelText("Timeline video a")).not.toBeInTheDocument();
    expect(layerFrame("b")).not.toHaveAttribute("data-transition-role");
    expect(video("b").style.opacity).toBe("1");
  });

  it("seeks and plays the incoming video from its head handle before its canonical start", () => {
    const { store } = renderTransition("crossfade");
    act(() => store.getState().seek(1.75));
    expect(video("b").currentTime).toBe(1.75);
    expect(video("a").currentTime).toBe(1.75);
    act(() => store.getState().togglePlaying());
    expect(video("b").paused).toBe(false);
    expect(video("a").paused).toBe(false);
    act(() => store.getState().togglePlaying());
    act(() => store.getState().seek(2.25));
    // The outgoing clip continues past sourceOut into its tail handle.
    expect(video("a").currentTime).toBe(2.25);
  });

  it("draws the dip solid beneath both clips, which are transparent at the cut", () => {
    const { store } = renderTransition("dipToWhite");
    act(() => store.getState().seek(2));
    const solid = screen.getByTestId("preview-transition-solid");
    expect(solid).toHaveAttribute("data-color", "white");
    expect(solid.nextElementSibling).toBe(layerFrame("a"));
    expect(video("a").style.opacity).toBe("0");
    expect(video("b").style.opacity).toBe("0");
    act(() => store.getState().seek(2.25));
    expect(video("b").style.opacity).toBe("0.5");
    act(() => store.getState().seek(2.5));
    expect(screen.queryByTestId("preview-transition-solid")).not.toBeInTheDocument();
  });

  it("wipes the incoming clip in from the left in canvas space", () => {
    const { store } = renderTransition("wipe");
    act(() => store.getState().seek(2));
    expect(layerFrame("b")?.style.clipPath).toBe("inset(0 50% 0 0)");
    expect(layerFrame("a")?.style.clipPath).toBe("");
    expect(video("b").style.opacity).toBe("1");
    const incoming = video("b");
    act(() => store.getState().seek(2.5));
    expect(layerFrame("b")?.style.clipPath).toBe("");
    // Leaving the window keeps the same video element.
    expect(video("b")).toBe(incoming);
  });
});

describe("PreviewPanel canonical preparation", () => {
  it("holds a shader-only timeline while its first frames are being prepared", () => {
    const project = fixtureProject();
    const track = project.timeline.tracks.find((entry) => entry.kind === "video")!;
    project.timeline.tracks = [{ ...track, kind: "hyperframe_scene", items: [{ ...track.items[0]!, kind: "hyperframe_scene", source: { type: "generated", artifactId: "shader-preview" }, properties: { shaderBackgroundTemplateId: "octagrams" } }] }];
    backendRequest.mockReturnValue(new Promise(() => {}));
    const { store } = renderPreview(project);
    act(() => store.getState().setPlaying(true));
    stubs.runFrame(1000);
    stubs.runFrame(4000);
    expect(store.getState().playheadSeconds).toBe(0);
    expect(store.getState().playing).toBe(true);
  });
  it("keeps an upper source image above a lower prepared frame", async () => {
    const project = canonicalProject();
    const videoTrack = project.timeline.tracks.find((track) => track.kind === "video");
    if (!videoTrack?.items[0]) throw new Error("fixture video track");
    project.media.push({ id: "upper-image", name: "Upper image", kind: "image", relativePath: "media/upper.png", durationSeconds: 4, width: 1920, height: 1080, fps: null });
    project.timeline.tracks.splice(1, 0, { ...videoTrack, id: "upper", name: "Upper", items: [{ ...videoTrack.items[0], id: "upper-item", kind: "image_clip", label: "Upper image", source: { type: "media", mediaId: "upper-image" }, properties: {} }] });
    backendRequest.mockResolvedValue({ project: structuredClone(project), reports: [], frameSequences: [{ itemId: "item-1", preparedMediaId: "media-1", startSeconds: 0, durationSeconds: 4, fps: 2, framePaths: ["f/0.png"] }] } satisfies PreparedProjectPreview);
    renderPreview(project);
    act(() => vi.advanceTimersByTime(400));
    await flushPromises();
    const prepared = screen.getByTestId("canonical-prepared-preview-frame").closest<HTMLElement>("[data-testid='preview-layer']")!;
    const upper = screen.getByAltText("Timeline image Upper image").closest<HTMLElement>("[data-testid='preview-layer']")!;
    expect(Number(upper.style.zIndex)).toBeGreaterThan(Number(prepared.style.zIndex));
    expect(screen.getByTestId("preview-canvas")).toHaveClass("isolate");
  });
  beforeEach(() => {
    window.localStorage.clear();
    backendRequest.mockReset();
    stubs = installMediaAndFrameStubs();
    vi.useFakeTimers({ toFake: ["setTimeout", "clearTimeout"] });
  });

  afterEach(() => {
    vi.useRealTimers();
    vi.unstubAllGlobals();
    vi.restoreAllMocks();
  });

  it("debounces preparation and shows Retry after a failure", async () => {
    backendRequest.mockRejectedValue(new Error("Precompose sidecar crashed."));
    renderPreview(canonicalProject());
    expect(within(viewport()).queryByLabelText("Timeline video Opening clip")).not.toBeInTheDocument();
    expect(screen.getByRole("alert", { name: "Preview issue" })).toHaveTextContent("Preparing preview");
    act(() => vi.advanceTimersByTime(399));
    expect(backendRequest).not.toHaveBeenCalled();
    act(() => vi.advanceTimersByTime(1));
    expect(backendRequest).toHaveBeenCalledWith("prepare_project_preview", expect.objectContaining({ projectDir: "/p" }));
    await flushPromises();

    const alert = screen.getByRole("alert", { name: "Preview issue" });
    expect(alert).toHaveTextContent("Precompose sidecar crashed.");
    fireEvent.click(within(alert).getByRole("button", { name: "Retry preview" }));
    act(() => vi.advanceTimersByTime(400));
    expect(backendRequest).toHaveBeenCalledTimes(2);
  });

  it("stays DOM-only without an error when there is no backend", async () => {
    backendRequest.mockRejectedValue(new BackendUnavailableError());
    renderPreview(canonicalProject());
    act(() => vi.advanceTimersByTime(400));
    await flushPromises();
    expect(within(viewport()).getByLabelText("Timeline video Opening clip")).toBeInTheDocument();
    expect(screen.queryByRole("alert")).not.toBeInTheDocument();
  });

  it("explains that an unsaved project cannot be prepared, without Retry", () => {
    renderPreview(canonicalProject(), "");
    const alert = screen.getByRole("alert", { name: "Preview issue" });
    expect(alert).toHaveTextContent("Canonical preview preparation requires a saved project folder.");
    expect(within(alert).queryByRole("button", { name: "Retry preview" })).not.toBeInTheDocument();
    expect(backendRequest).not.toHaveBeenCalled();
  });

  it("overlays prepared frames selected by the playhead", async () => {
    const project = canonicalProject();
    const preparedProject = structuredClone(project);
    const result: PreparedProjectPreview = {
      project: preparedProject,
      reports: [],
      frameSequences: [
        { itemId: "item-1", preparedMediaId: "media-1", startSeconds: 0, durationSeconds: 4, fps: 2, framePaths: ["f/0.png", "f/1.png", "f/2.png", "f/3.png", "f/4.png", "f/5.png", "f/6.png", "f/7.png"] },
      ],
    };
    backendRequest.mockResolvedValue(result);
    const { store } = renderPreview(project);
    act(() => vi.advanceTimersByTime(400));
    await flushPromises();
    act(() => store.getState().seek(1.6));
    const frame = screen.getByTestId("canonical-prepared-preview-frame");
    expect(frame).toHaveAttribute("src", "/p/f/3.png");
    expect(frame).toHaveAttribute("data-prepared-frame-url", "/p/f/3.png");
    expect(screen.getByText("Canonical ready")).toBeInTheDocument();
    expect(screen.queryByRole("alert")).not.toBeInTheDocument();

    fireEvent.error(frame);
    const alert = screen.getByRole("alert", { name: "Preview issue" });
    expect(alert).toHaveTextContent("Canonical preview frame failed to load.");
    fireEvent.click(within(alert).getByRole("button", { name: "Retry preview" }));
    act(() => vi.advanceTimersByTime(400));
    expect(backendRequest).toHaveBeenCalledTimes(2);
  });
});
