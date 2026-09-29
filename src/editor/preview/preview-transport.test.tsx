import "@testing-library/jest-dom/vitest";
import { act, fireEvent, screen, within } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { VideoProject } from "@/lib/project";
import { renderWithEditorStore } from "@/test-utils/editor-render";
import { fixtureProject } from "@/test-utils/editor-fixtures";

vi.mock("@/lib/runtime/backend-client", () => ({ backendRequest: vi.fn(), backendListen: vi.fn(), backendMediaUrl: (p: string) => p }));

const { PreviewPanel } = await import("./preview-panel");
const { installMediaAndFrameStubs } = await import("./media-element-stubs");

let stubs: ReturnType<typeof installMediaAndFrameStubs>;

function renderPreview(project: VideoProject = fixtureProject()) {
  return renderWithEditorStore(<PreviewPanel />, { project });
}

function transport() {
  return screen.getByRole("group", { name: "Preview transport" });
}

function scrubber() {
  return within(transport()).getByRole("slider", { name: "Preview scrubber" });
}

describe("preview transport", () => {
  beforeEach(() => {
    window.localStorage.clear();
    stubs = installMediaAndFrameStubs();
  });

  afterEach(() => {
    vi.unstubAllGlobals();
    vi.restoreAllMocks();
  });

  it("names every transport control and shows time and aspect ratio", () => {
    renderPreview();
    const group = transport();
    for (const name of ["Previous frame", "Play preview", "Next frame", "Enter fullscreen"]) {
      expect(within(group).getByRole("button", { name })).toBeEnabled();
    }
    expect(scrubber()).toHaveAttribute("aria-valuetext", "00:00:00");
    expect(group).toHaveTextContent("00:00:00 / 00:00:08");
    expect(group).toHaveTextContent("16:9");
  });

  it("plays and pauses from the play button", () => {
    const { store } = renderPreview();
    fireEvent.click(within(transport()).getByRole("button", { name: "Play preview" }));
    expect(store.getState().playing).toBe(true);
    stubs.runFrame(0);
    stubs.runFrame(1500);
    expect(transport()).toHaveTextContent("00:00:01.500 / 00:00:08");
    fireEvent.click(within(transport()).getByRole("button", { name: "Pause preview" }));
    expect(store.getState().playing).toBe(false);
  });

  it("steps one frame at 24 fps and snaps onto the frame grid", () => {
    const { store } = renderPreview();
    act(() => store.getState().seek(1));
    fireEvent.click(within(transport()).getByRole("button", { name: "Next frame" }));
    expect(store.getState().playheadSeconds).toBeCloseTo(1 + 1 / 24, 9);
    fireEvent.click(within(transport()).getByRole("button", { name: "Previous frame" }));
    fireEvent.click(within(transport()).getByRole("button", { name: "Previous frame" }));
    expect(store.getState().playheadSeconds).toBeCloseTo(1 - 1 / 24, 9);

    act(() => store.getState().seek(1.01));
    fireEvent.click(within(transport()).getByRole("button", { name: "Next frame" }));
    expect(store.getState().playheadSeconds).toBeCloseTo(25 / 24, 9);
  });

  it("seeks with the scrubber", () => {
    const { store } = renderPreview();
    fireEvent.keyDown(scrubber(), { key: "End" });
    expect(store.getState().playheadSeconds).toBe(8);
    expect(scrubber()).toHaveAttribute("aria-valuetext", "00:00:08");
    fireEvent.keyDown(scrubber(), { key: "ArrowLeft" });
    expect(store.getState().playheadSeconds).toBeCloseTo(8 - 1 / 24, 3);
    fireEvent.keyDown(scrubber(), { key: "Home" });
    expect(store.getState().playheadSeconds).toBe(0);
  });

  it("handles Space and the arrow keys inside the preview", () => {
    const { store } = renderPreview();
    const viewport = screen.getByRole("region", { name: "Preview viewport" });
    fireEvent.keyDown(viewport, { key: "ArrowRight" });
    expect(store.getState().playheadSeconds).toBeCloseTo(1 / 24, 9);
    fireEvent.keyDown(viewport, { key: " " });
    expect(store.getState().playing).toBe(true);
    // Space on a focused button presses the button instead.
    fireEvent.keyDown(within(transport()).getByRole("button", { name: "Next frame" }), { key: " " });
    expect(store.getState().playing).toBe(true);
  });

  it("disables playback and seeking for an empty timeline", () => {
    const project = fixtureProject();
    project.timeline.durationSeconds = 0;
    renderPreview(project);
    expect(within(transport()).getByRole("button", { name: "Play preview" })).toBeDisabled();
    expect(within(transport()).getByRole("button", { name: "Next frame" })).toBeDisabled();
    expect(scrubber()).toHaveAttribute("data-disabled");
  });
});

describe("preview fullscreen", () => {
  beforeEach(() => {
    window.localStorage.clear();
    stubs = installMediaAndFrameStubs();
  });

  afterEach(() => {
    vi.unstubAllGlobals();
    vi.restoreAllMocks();
    Reflect.deleteProperty(HTMLElement.prototype, "requestFullscreen");
    Reflect.deleteProperty(document, "exitFullscreen");
    Reflect.deleteProperty(document, "fullscreenElement");
  });

  it("requests fullscreen on the preview panel and follows fullscreenchange", async () => {
    let fullscreenElement: Element | null = null;
    Object.defineProperty(document, "fullscreenElement", { configurable: true, get: () => fullscreenElement });
    const exitFullscreen = vi.fn(() => {
      fullscreenElement = null;
      document.dispatchEvent(new Event("fullscreenchange"));
      return Promise.resolve();
    });
    Object.defineProperty(document, "exitFullscreen", { configurable: true, value: exitFullscreen });
    Object.defineProperty(HTMLElement.prototype, "requestFullscreen", {
      configurable: true,
      value(this: HTMLElement) {
        fullscreenElement = this;
        document.dispatchEvent(new Event("fullscreenchange"));
        return Promise.resolve();
      },
    });
    const { store } = renderPreview();
    await act(async () => fireEvent.click(within(transport()).getByRole("button", { name: "Enter fullscreen" })));
    const panel = screen.getByRole("region", { name: "Preview panel" });
    expect(fullscreenElement).toBe(panel);
    expect(store.getState().fullscreen).toBe(true);
    expect(panel).toHaveClass("fixed");

    await act(async () => fireEvent.click(within(transport()).getByRole("button", { name: "Exit fullscreen" })));
    expect(exitFullscreen).toHaveBeenCalled();
    expect(store.getState().fullscreen).toBe(false);

    // Leaving fullscreen with the browser's own Esc handling updates the flag too.
    await act(async () => fireEvent.click(within(transport()).getByRole("button", { name: "Enter fullscreen" })));
    act(() => {
      fullscreenElement = null;
      document.dispatchEvent(new Event("fullscreenchange"));
    });
    expect(store.getState().fullscreen).toBe(false);
  });

  it("fills the window when element fullscreen is unavailable", async () => {
    const { store } = renderPreview();
    await act(async () => fireEvent.click(within(transport()).getByRole("button", { name: "Enter fullscreen" })));
    expect(store.getState().fullscreen).toBe(true);
    expect(screen.getByRole("region", { name: "Preview panel" })).toHaveClass("fixed", "inset-0");
  });
});

describe("asset preview", () => {
  beforeEach(() => {
    window.localStorage.clear();
    stubs = installMediaAndFrameStubs();
  });

  afterEach(() => {
    vi.unstubAllGlobals();
    vi.restoreAllMocks();
  });

  it("shows the asset alone with the Previewing chip and returns to the timeline on Back", () => {
    const { store } = renderPreview();
    act(() => store.getState().previewAsset("media-1"));
    const viewport = screen.getByRole("region", { name: "Preview viewport" });
    expect(within(viewport).getByText(/Previewing:/)).toHaveTextContent("Previewing: input.mp4");
    const video = within(viewport).getByLabelText("Video preview input.mp4") as HTMLVideoElement;
    expect(video.tagName).toBe("VIDEO");
    expect(video.muted).toBe(true);
    expect(within(viewport).queryByLabelText("Timeline video Opening clip")).not.toBeInTheDocument();
    expect(transport()).toHaveTextContent("00:00:00 / 00:00:04");

    fireEvent.click(within(viewport).getByRole("button", { name: "Back to timeline" }));
    expect(store.getState().previewSource).toEqual({ kind: "timeline" });
    expect(within(screen.getByRole("region", { name: "Preview viewport" })).getByLabelText("Timeline video Opening clip")).toBeInTheDocument();
  });

  it("plays the asset through the store and steps by its frame rate", () => {
    const { store } = renderPreview();
    act(() => store.getState().previewAsset("media-1"));
    const video = screen.getByLabelText("Video preview input.mp4") as HTMLVideoElement;
    fireEvent.click(within(transport()).getByRole("button", { name: "Play preview" }));
    expect(stubs.play).toHaveBeenCalled();
    expect(store.getState().playing).toBe(true);
    // The timeline clock does not run in asset mode.
    expect(stubs.pendingFrameCount()).toBe(0);

    fireEvent.ended(video);
    expect(store.getState().playing).toBe(false);

    video.currentTime = 1;
    fireEvent.click(within(transport()).getByRole("button", { name: "Next frame" }));
    expect(video.currentTime).toBeCloseTo(1 + 1 / 24, 9);
    expect(transport()).toHaveTextContent(/^00:00:01\.042 \/ 00:00:04/);
  });

  it("offers Retry after the asset fails to load", () => {
    const { store } = renderPreview();
    act(() => store.getState().previewAsset("media-1"));
    fireEvent.error(screen.getByLabelText("Video preview input.mp4"));
    const alert = screen.getByRole("alert");
    expect(alert).toHaveTextContent("Preview failed");
    expect(within(transport()).getByRole("button", { name: "Play preview" })).toBeDisabled();
    fireEvent.click(within(alert).getByRole("button", { name: "Retry preview" }));
    expect(screen.getByLabelText("Video preview input.mp4")).toBeInTheDocument();
  });

  it("falls back to the timeline when the previewed asset is gone", () => {
    const { store } = renderPreview();
    act(() => store.getState().previewAsset("missing-media"));
    expect(screen.queryByText(/Previewing:/)).not.toBeInTheDocument();
    expect(screen.getByLabelText("Timeline video Opening clip")).toBeInTheDocument();
  });
});
