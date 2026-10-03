import "@testing-library/jest-dom/vitest";
import { act, fireEvent, screen } from "@testing-library/react";
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import { fixtureProject } from "@/test-utils/editor-fixtures";
import { renderWithEditorStore } from "@/test-utils/editor-render";
import { installMediaAndFrameStubs } from "./media-element-stubs";
import { AssetPreview } from "./asset-preview";

let url: string | null = "/media/first";
const readiness = vi.hoisted(() => ({ status: "ready" }));
vi.mock("@/lib/media/use-media-readiness", () => ({ useMediaReadiness: () => readiness }));
const backend = vi.hoisted(() => ({ request: vi.fn() }));
let stubs: ReturnType<typeof installMediaAndFrameStubs>;
vi.mock("@/lib/runtime/backend-client", () => ({
  backendRequest: backend.request, backendListen: vi.fn(), backendMediaUrl: () => url,
}));
beforeEach(() => { url = "/media/first"; readiness.status = "ready"; stubs = installMediaAndFrameStubs(); backend.request.mockReset(); backend.request.mockReturnValue(new Promise(() => {})); });
afterEach(() => { vi.restoreAllMocks(); vi.unstubAllGlobals(); });

it("restores source position and resumes playback after a remote URL renews", async () => {
  const { store } = renderWithEditorStore(<AssetPreview mediaId="media-1" fullscreen={false} onToggleFullscreen={() => {}} />, { project: fixtureProject() });
  const video = screen.getByLabelText("Video preview input.mp4") as HTMLVideoElement;
  video.currentTime = 2.5;
  fireEvent.timeUpdate(video);
  act(() => store.getState().setPlaying(true));
  await act(async () => {});
  url = "/media/renewed";
  act(() => store.setState({ project: { ...store.getState().project } }));
  // Reloading src resets the native clock and dispatches pause before loadedmetadata.
  video.currentTime = 0;
  video.pause();
  fireEvent.pause(video);
  fireEvent.loadedMetadata(video);
  await act(async () => {});
  expect(video.currentTime).toBe(2.5);
  expect(video.paused).toBe(false);
  expect(store.getState().playing).toBe(true);
});

it("ignores a rejected play promise from a previous source URL", async () => {
  let reject!: (error: Error) => void;
  vi.spyOn(HTMLMediaElement.prototype, "play").mockImplementationOnce(() => new Promise<void>((_resolve, no) => { reject = no; }));
  const { store } = renderWithEditorStore(<AssetPreview mediaId="media-1" fullscreen={false} onToggleFullscreen={() => {}} />, { project: fixtureProject() });
  act(() => store.getState().setPlaying(true));
  url = "/media/renewed";
  act(() => store.setState({ project: { ...store.getState().project } }));
  await act(async () => { reject(new Error("old request aborted")); });
  expect(store.getState().playing).toBe(true);
});

it("keeps playback intent while an expired remote source ticket is being renewed", async () => {
  const { store } = renderWithEditorStore(<AssetPreview mediaId="media-1" fullscreen={false} onToggleFullscreen={() => {}} />, { project: fixtureProject() });
  const video = screen.getByLabelText("Video preview input.mp4") as HTMLVideoElement;
  video.currentTime = 2.5;
  fireEvent.timeUpdate(video);
  act(() => store.getState().setPlaying(true));
  await act(async () => {});
  readiness.status = "loading";
  url = null;
  act(() => store.setState({ project: { ...store.getState().project } }));
  expect(store.getState().playing).toBe(true);
  readiness.status = "ready";
  url = "/media/renewed";
  act(() => store.setState({ project: { ...store.getState().project } }));
  const renewed = screen.getByLabelText("Video preview input.mp4") as HTMLVideoElement;
  fireEvent.loadedMetadata(renewed);
  await act(async () => {});
  expect(renewed.currentTime).toBe(2.5);
  expect(renewed.paused).toBe(false);
});

it("provides playback for a Lottie source instead of an inert placeholder", () => {
  const project = fixtureProject();
  project.media[0] = { ...project.media[0]!, kind: "lottie", durationSeconds: 2 };
  renderWithEditorStore(<AssetPreview mediaId="media-1" fullscreen={false} onToggleFullscreen={() => {}} />, { project, projectDir: "/p" });
  expect(screen.queryByLabelText("Lottie preview input.mp4")).not.toBeInTheDocument();
  expect(screen.getByRole("status")).toHaveTextContent(/Preparing.*source/i);
});

it("advances prepared source frames with recovered legacy metadata, seeks, and restarts at the end", async () => {
  const project = fixtureProject();
  const prepared = fixtureProject();
  project.media[0] = { ...project.media[0]!, kind: "lottie", durationSeconds: 0, fps: null };
  prepared.timeline.durationSeconds = 2;
  prepared.renderSettings.fps = 2;
  backend.request.mockResolvedValue({
    project: prepared, reports: [], frameSequences: [{
      itemId: "item-1", preparedMediaId: "media-1", startSeconds: 0, durationSeconds: 2, fps: 2,
      framePaths: ["cache/0.png", "cache/1.png", "cache/2.png", "cache/3.png"],
    }],
  });
  // Use distinct frame URLs so a stalled animation cannot pass.
  url = "";
  const client = await import("@/lib/runtime/backend-client");
  vi.spyOn(client, "backendMediaUrl").mockImplementation((path) => path);
  const { store } = renderWithEditorStore(<AssetPreview mediaId="media-1" fullscreen={false} onToggleFullscreen={() => {}} />, { project });
  await act(async () => {});
  const frame = screen.getByTestId("canonical-prepared-preview-frame");
  expect(frame).toHaveAttribute("src", "/p/cache/0.png");
  let decoded = false;
  vi.spyOn(HTMLImageElement.prototype, "complete", "get").mockImplementation(() => decoded);
  fireEvent.click(screen.getByRole("button", { name: "Play preview" }));
  stubs.runFrame(-2000);
  stubs.runFrame(-1000);
  expect(frame).toHaveAttribute("src", "/p/cache/0.png");
  expect(store.getState().playing).toBe(true);
  decoded = true;
  stubs.runFrame(100);
  stubs.runFrame(600);
  expect(frame).toHaveAttribute("src", "/p/cache/1.png");
  stubs.runFrame(1100);
  expect(frame).toHaveAttribute("src", "/p/cache/2.png");
  stubs.runFrame(2100);
  expect(store.getState().playing).toBe(false);
  fireEvent.click(screen.getByRole("button", { name: "Play preview" }));
  expect(frame).toHaveAttribute("src", "/p/cache/0.png");
});

it("stops a late source play promise after the user pauses", async () => {
  let resume!: () => void;
  let paused = true;
  const pending = new Promise<void>((resolve) => { resume = resolve; });
  vi.spyOn(HTMLMediaElement.prototype, "paused", "get").mockImplementation(() => paused);
  vi.spyOn(HTMLMediaElement.prototype, "pause").mockImplementation(() => { paused = true; });
  vi.spyOn(HTMLMediaElement.prototype, "play").mockImplementation(() => pending.then(() => { paused = false; }));
  const { store } = renderWithEditorStore(<AssetPreview mediaId="media-1" fullscreen={false} onToggleFullscreen={() => {}} />, { project: fixtureProject() });
  act(() => store.getState().setPlaying(true));
  act(() => store.getState().setPlaying(false));
  await act(async () => { resume(); });
  expect((screen.getByLabelText("Video preview input.mp4") as HTMLVideoElement).paused).toBe(true);
});

it("plays the source video's sound", () => {
  renderWithEditorStore(<AssetPreview mediaId="media-1" fullscreen={false} onToggleFullscreen={() => {}} />, { project: fixtureProject() });
  expect((screen.getByLabelText("Video preview input.mp4") as HTMLVideoElement).muted).toBe(false);
});
