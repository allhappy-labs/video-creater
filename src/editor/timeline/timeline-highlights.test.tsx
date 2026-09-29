import "@testing-library/jest-dom/vitest";
import { act, fireEvent, screen, within } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import type { CodexProposalImpact, VideoProject } from "@/lib/project";
import { backendRequest } from "@/lib/runtime/backend-client";
import { BackendUnavailableError } from "@/lib/runtime/backend-transport";
import { agentProject, appliedMessage } from "@/test-utils/agent-fixtures";
import { renderWithEditorStore } from "@/test-utils/editor-render";
import { fixtureItem, fixtureProject } from "@/test-utils/editor-fixtures";
import { AppliedCard } from "../panels/ai/applied-card";
import { useEditorShortcuts } from "../shell/use-editor-shortcuts";
import type { EditorStore } from "../store/editor-store";
import { TimelinePanel } from "./timeline-panel";

vi.mock("@/lib/runtime/backend-client", () => ({ backendRequest: vi.fn(), backendListen: vi.fn(), backendMediaUrl: (p: string) => p }));

/** Default zoom: 80 px per second. */
const pixelsPerSecond = 80;
let sequence = 0;

function EditorShortcuts() {
  useEditorShortcuts("linux");
  return null;
}

function renderTimeline(project: VideoProject, options: { mobile?: boolean; withCard?: boolean; impact?: Partial<CodexProposalImpact> } = {}) {
  sequence += 1;
  const base = appliedMessage(project, "assistant-highlight", "user-highlight").card?.result?.impact;
  if (!base) throw new Error("expected an applied impact");
  const message = appliedMessage(project, "assistant-highlight", "user-highlight", { result: { historyEntryId: "highlight", impact: { ...base, ...options.impact } } });
  const card = message.card;
  if (!card?.result) throw new Error("expected an applied card");
  const rendered = renderWithEditorStore(
    <>
      <EditorShortcuts />
      {options.withCard && <AppliedCard message={message} card={{ ...card, result: card.result }} />}
      <TimelinePanel mobile={options.mobile ?? false} />
    </>,
    { project, projectDir: `/projects/highlights-${sequence.toString()}` },
  );
  act(() => rendered.store.setState({ zoomPercent: 100, mergeLoadedProject: vi.fn(async () => true) }));
  return rendered;
}

function lane(trackId: string): HTMLElement {
  const element = document.querySelector<HTMLElement>(`[data-track-id="${trackId}"]`);
  if (!element) throw new Error(`no lane for ${trackId}`);
  return element;
}

function bands(trackId: string): HTMLElement[] {
  return within(lane(trackId)).queryAllByTestId("timeline-highlight-range");
}

function clip(itemId: string): HTMLElement {
  const element = document.querySelector<HTMLElement>(`[role="option"][data-item-id="${itemId}"]`);
  if (!element) throw new Error(`no clip ${itemId}`);
  return element;
}

function highlightCount(store: EditorStore): number {
  const { highlightedItemIds, highlightedRanges } = store.getState();
  return highlightedItemIds.length + highlightedRanges.length + screen.queryAllByTestId("clip-highlight").length + screen.queryAllByTestId("timeline-highlight-range").length;
}

function escape() {
  act(() => void window.dispatchEvent(new KeyboardEvent("keydown", { key: "Escape" })));
}

describe("timeline highlights", () => {
  beforeEach(() => {
    window.localStorage.clear();
    vi.mocked(backendRequest).mockReset();
    vi.mocked(backendRequest).mockRejectedValue(new BackendUnavailableError());
  });

  it.each([
    ["desktop", false],
    ["phone", true],
  ])("Show changes outlines the affected clips and bands the ranges on their tracks (%s)", async (_layout, mobile) => {
    const project = agentProject();
    const video = fixtureItem(project, "video");
    renderTimeline(project, { mobile, withCard: true });
    expect(screen.queryAllByTestId("timeline-highlight-range")).toHaveLength(0);

    fireEvent.click(screen.getByRole("button", { name: "Show changes" }));

    expect(within(clip(video.id)).getByTestId("clip-highlight")).toBeInTheDocument();
    expect(clip(video.id)).toHaveAccessibleDescription("Changed by the AI edit");
    expect(screen.getAllByTestId("clip-highlight")).toHaveLength(1);
    expect(clip(fixtureItem(project, "caption").id)).not.toHaveAccessibleDescription();

    const videoBands = bands("track-video");
    expect(videoBands.map((band) => [band.style.left, band.style.width])).toEqual([
      [`${pixelsPerSecond}px`, `${pixelsPerSecond}px`],
      [`${3 * pixelsPerSecond}px`, `${0.5 * pixelsPerSecond}px`],
    ]);
    expect(screen.getAllByTestId("timeline-highlight-range")).toHaveLength(2);
    // Decoration only: no pointer events, hidden from assistive technology, and out of the layout flow.
    for (const element of [...videoBands, ...screen.getAllByTestId("clip-highlight")]) {
      expect(element).toHaveClass("pointer-events-none", "absolute");
      expect(element).toHaveAttribute("aria-hidden", "true");
    }
    // Let filmstrip and result frame requests settle.
    await act(async () => {
      for (let index = 0; index < 20; index += 1) await Promise.resolve();
    });
  });

  it("clamps changed ranges to the current timeline end and drops the ones past it", () => {
    const project = agentProject();
    const end = project.timeline.durationSeconds;
    // Impact ranges can include where clips were before the edit, past the new end.
    const { store } = renderTimeline(project, {
      withCard: true,
      impact: {
        affectedRanges: [
          { startSeconds: -1, endSeconds: 1 },
          { startSeconds: end - 1, endSeconds: end + 2 },
          { startSeconds: end, endSeconds: end + 4 },
          { startSeconds: end + 1, endSeconds: end + 3 },
        ],
      },
    });

    fireEvent.click(screen.getByRole("button", { name: "Show changes" }));

    expect(store.getState().highlightedRanges.map((range) => [range.startSeconds, range.endSeconds])).toEqual([
      [0, 1],
      [end - 1, end],
    ]);
    expect(bands("track-video").map((band) => [band.style.left, band.style.width])).toEqual([
      ["0px", `${pixelsPerSecond}px`],
      [`${(end - 1) * pixelsPerSecond}px`, `${pixelsPerSecond}px`],
    ]);
  });

  it("draws the range band behind the clips so they stay legible", () => {
    const project = fixtureProject();
    const video = fixtureItem(project, "video");
    const { store } = renderTimeline(project);
    act(() => {
      store.getState().selectItems([video.id]);
      store.getState().setHighlights([video.id], [{ trackIds: ["track-video"], startSeconds: 0.5, endSeconds: 3 }]);
    });
    const [band] = bands("track-video");
    if (!band) throw new Error("expected a band");
    const listbox = within(lane("track-video")).getByRole("listbox");
    // Painted first and without a z-index, so clips, selection outlines, transition badges and trim
    // handles (all later in the lane) paint over it; the clip keeps full opacity.
    expect(band.compareDocumentPosition(listbox) & Node.DOCUMENT_POSITION_FOLLOWING).toBeTruthy();
    expect(band.className).not.toMatch(/(^|\s)z-/);
    expect(band).toHaveClass("pointer-events-none", "absolute");
    expect(clip(video.id).className).not.toMatch(/opacity/);
    expect(within(clip(video.id)).getByTestId("clip-body").className).not.toMatch(/opacity/);
  });

  it("spans every track for a range without track ids and keeps the outline inside a selection", () => {
    const project = fixtureProject();
    const video = fixtureItem(project, "video");
    const { store } = renderTimeline(project);
    act(() => {
      store.getState().selectItems([video.id]);
      store.getState().setHighlights([video.id], [{ trackIds: [], startSeconds: 0.5, endSeconds: 1 }]);
    });
    for (const track of project.timeline.tracks) expect(bands(track.id)).toHaveLength(1);
    expect(within(clip(video.id)).getByTestId("clip-highlight")).toHaveClass("inset-[2px]");
  });

  it("clears on the next undoable edit, but not on job bookkeeping", async () => {
    const project = fixtureProject();
    const video = fixtureItem(project, "video");
    const { store } = renderTimeline(project);
    act(() => store.getState().setHighlights([video.id], [{ trackIds: ["track-video"], startSeconds: 1, endSeconds: 2 }]));

    await act(async () => {
      await store.getState().applyActions([{ type: "recordJob", job: { id: "job-bookkeeping", kind: "render_draft", status: "queued", updatedAt: "2026-09-15T10:00:00Z" } }]);
    });
    expect(store.getState().project.jobs.some((job) => job.id === "job-bookkeeping")).toBe(true);
    expect(screen.getAllByTestId("timeline-highlight-range")).toHaveLength(1);

    await act(async () => {
      await store.getState().applyActions([{ type: "updateVisualClipOpacity", itemId: video.id, opacity: 0.4 }]);
    });
    expect(highlightCount(store)).toBe(0);
  });

  it("clears on undo", async () => {
    const project = fixtureProject();
    const video = fixtureItem(project, "video");
    const { store } = renderTimeline(project);
    await act(async () => {
      await store.getState().applyActions([{ type: "updateVisualClipOpacity", itemId: video.id, opacity: 0.4 }]);
    });
    act(() => store.getState().setHighlights([video.id], []));
    await act(async () => {
      await store.getState().undo();
    });
    expect(highlightCount(store)).toBe(0);
  });

  it("clears on Esc with the selection, after Esc closes an open sheet", () => {
    const project = fixtureProject();
    const video = fixtureItem(project, "video");
    const { store } = renderTimeline(project);
    act(() => {
      store.getState().selectItems([video.id]);
      store.getState().setHighlights([video.id], [{ trackIds: ["track-video"], startSeconds: 0, endSeconds: 4 }]);
      store.getState().openSheet("ai");
    });

    escape();
    expect(store.getState().openSheetId).toBeNull();
    expect(screen.getAllByTestId("timeline-highlight-range")).toHaveLength(1);

    escape();
    expect(store.getState().selectedItemIds).toEqual([]);
    expect(highlightCount(store)).toBe(0);
  });
});
