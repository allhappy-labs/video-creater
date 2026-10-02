import "@testing-library/jest-dom/vitest";
import { act, screen, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import type { VideoProject } from "@/lib/project";
import type { TimelineItem } from "@/lib/timeline";
import { BackendUnavailableError } from "@/lib/runtime/backend-transport";
import { renderWithEditorStore } from "@/test-utils/editor-render";
import { fixtureProject } from "@/test-utils/editor-fixtures";

const backendRequest = vi.fn();
vi.mock("@/lib/runtime/backend-client", () => ({
  backendRequest: (...args: unknown[]) => backendRequest(...args),
  backendListen: vi.fn(),
  backendMediaUrl: (path: string) => `media://${path}`,
}));

const { ClipFilmstrip } = await import("./clip-filmstrip");

let nextItem = 0;
/** Each test uses a fresh item id because filmstrip requests are cached per clip for the session. */
function videoItem(properties: Record<string, unknown> = {}): TimelineItem {
  nextItem += 1;
  return {
    id: `filmstrip-clip-${nextItem}`,
    kind: "video_clip",
    startSeconds: 0,
    durationSeconds: 4,
    source: { type: "media", mediaId: "media-1" },
    label: "Clip",
    properties: { sourceIn: 1, sourceOut: 5, ...properties },
  };
}

function splitProject(): VideoProject {
  return { ...fixtureProject(), schemaVersion: 2, contentRevision: 1 };
}

function renderFilmstrip(item: TimelineItem, projectDir = "/p", project = splitProject()) {
  return renderWithEditorStore(<ClipFilmstrip item={item} width={320} height={52} />, { project, projectDir });
}

function filmstrip(): HTMLElement {
  return screen.getByTestId("clip-filmstrip");
}

describe("ClipFilmstrip", () => {
  beforeEach(() => {
    backendRequest.mockReset();
    window.localStorage.clear();
  });

  it("renders cached frames through the backend media URL", async () => {
    backendRequest.mockResolvedValue({
      frames: [
        { timeSeconds: 1, relativePath: ".cache/filmstrip/a.jpg" },
        { timeSeconds: 3, relativePath: ".cache/filmstrip/b.jpg" },
      ],
    });
    const item = videoItem({ speed: 2 });
    const { container } = renderFilmstrip(item);

    await waitFor(() => expect(filmstrip()).toHaveAttribute("data-state", "frames"));
    expect(Array.from(container.querySelectorAll("img")).map((image) => image.getAttribute("src"))).toEqual([
      "media:///p/.cache/filmstrip/a.jpg",
      "media:///p/.cache/filmstrip/b.jpg",
    ]);
    expect(backendRequest).toHaveBeenCalledWith("cache_timeline_filmstrip_in_split_project_folder", {
      projectDir: "/p",
      mediaId: "media-1",
      sourceIn: 1,
      sourceOut: 5,
      speed: 2,
      zoomBucket: 100,
      heightBucket: 52,
      clipPixelWidth: 320,
    });
  });

  it("shows a reversed clip's frames last to first", async () => {
    backendRequest.mockResolvedValue({
      frames: [
        { timeSeconds: 1, relativePath: ".cache/filmstrip/a.jpg" },
        { timeSeconds: 3, relativePath: ".cache/filmstrip/b.jpg" },
      ],
    });
    const { container } = renderFilmstrip(videoItem({ reverse: true }));
    await waitFor(() => expect(filmstrip()).toHaveAttribute("data-state", "frames"));
    expect(Array.from(container.querySelectorAll("img")).map((image) => image.getAttribute("src"))).toEqual([
      "media:///p/.cache/filmstrip/b.jpg",
      "media:///p/.cache/filmstrip/a.jpg",
    ]);
  });

  it("falls back to a solid fill when the backend is unavailable, without retrying", async () => {
    backendRequest.mockRejectedValue(new BackendUnavailableError());
    const item = videoItem();
    const first = renderFilmstrip(item);
    await waitFor(() => expect(backendRequest).toHaveBeenCalledTimes(1));
    expect(filmstrip()).toHaveAttribute("data-state", "fallback");
    expect(first.container.querySelector("img")).not.toBeInTheDocument();
    first.unmount();

    renderFilmstrip(item);
    await act(async () => {});
    expect(backendRequest).toHaveBeenCalledTimes(1);
    expect(filmstrip()).toHaveAttribute("data-state", "fallback");
  });

  it("falls back when the request fails", async () => {
    backendRequest.mockRejectedValue(new Error("ffmpeg exploded"));
    renderFilmstrip(videoItem());
    await waitFor(() => expect(backendRequest).toHaveBeenCalledTimes(1));
    await act(async () => {});
    expect(filmstrip()).toHaveAttribute("data-state", "fallback");
  });

  it("never requests frames for the browser sample project", async () => {
    renderFilmstrip(videoItem(), "browser://bundled-sample-project");
    await act(async () => {});
    expect(backendRequest).not.toHaveBeenCalled();
    expect(filmstrip()).toHaveAttribute("data-state", "fallback");
  });

  it("never requests frames for single-file (schema 1) projects", async () => {
    renderFilmstrip(videoItem(), "/p", fixtureProject());
    await act(async () => {});
    expect(backendRequest).not.toHaveBeenCalled();
    expect(filmstrip()).toHaveAttribute("data-state", "fallback");
  });

  it("requests at most once per clip per zoom bucket", async () => {
    backendRequest.mockResolvedValue({ frames: [{ timeSeconds: 1, relativePath: "a.jpg" }] });
    const { store } = renderFilmstrip(videoItem());
    await waitFor(() => expect(backendRequest).toHaveBeenCalledTimes(1));

    act(() => store.getState().setZoomPercent(110));
    await act(async () => {});
    expect(backendRequest).toHaveBeenCalledTimes(1);

    act(() => store.getState().setZoomPercent(150));
    await waitFor(() => expect(backendRequest).toHaveBeenCalledTimes(2));
    expect(backendRequest).toHaveBeenLastCalledWith(
      "cache_timeline_filmstrip_in_split_project_folder",
      expect.objectContaining({ zoomBucket: 150 }),
    );
  });

  it("requests new sampling when width changes within the same zoom bucket", async () => {
    backendRequest.mockResolvedValue({ frames: [{ timeSeconds: 1, relativePath: "a.jpg" }] });
    const item = videoItem();
    const rendered = renderFilmstrip(item);
    await waitFor(() => expect(backendRequest).toHaveBeenCalledTimes(1));
    rendered.unmount();
    renderWithEditorStore(<ClipFilmstrip item={item} width={321} height={52} />, { project: splitProject(), projectDir: "/p" });
    await waitFor(() => expect(backendRequest).toHaveBeenCalledTimes(2));
    expect(backendRequest).toHaveBeenLastCalledWith("cache_timeline_filmstrip_in_split_project_folder", expect.objectContaining({ clipPixelWidth: 321, zoomBucket: 100 }));
  });
});
