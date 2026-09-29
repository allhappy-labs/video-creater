import "@testing-library/jest-dom/vitest";
import { act, fireEvent, screen, within } from "@testing-library/react";
import { afterEach, beforeAll, beforeEach, describe, expect, it, vi } from "vitest";
import type { ProjectAction, VideoProject } from "@/lib/project";
import type { TimelineItem } from "@/lib/timeline";
import { fakeDataTransfer } from "@/test-utils/data-transfer";
import { installPointerEventPolyfill, renderWithEditorStore } from "@/test-utils/editor-render";
import { fixtureProject } from "@/test-utils/editor-fixtures";
import { assetDragMimeType } from "../../timeline/drag-data";
import { TextPanel } from "./text-panel";

const { appliedBatches } = vi.hoisted(() => ({ appliedBatches: [] as ProjectAction[][] }));

vi.mock("@/lib/runtime/backend-client", () => ({ backendRequest: vi.fn(), backendListen: vi.fn(), backendMediaUrl: (p: string) => p }));
vi.mock("@/lib/agent/project-merge", async (importOriginal) => {
  const actual = await importOriginal<typeof import("@/lib/agent/project-merge")>();
  return {
    ...actual,
    applyProjectActionsLocally: (project: VideoProject, actions: ProjectAction[]) => {
      appliedBatches.push(actions);
      return actual.applyProjectActionsLocally(project, actions);
    },
  };
});

const animations: { keyframes: Keyframe[]; cancel: ReturnType<typeof vi.fn> }[] = [];

/** Stubs `matchMedia` so only `matching` queries match. */
function stubMediaQueries(...matching: string[]) {
  window.matchMedia = ((query: string) => ({
    matches: matching.includes(query),
    media: query,
    addEventListener: () => undefined,
    removeEventListener: () => undefined,
  })) as unknown as typeof window.matchMedia;
}

function renderPanel(playheadSeconds = 2) {
  const rendered = renderWithEditorStore(<TextPanel />, { project: fixtureProject() });
  act(() => rendered.store.setState({ playheadSeconds }));
  return rendered;
}

async function settle() {
  await act(async () => {
    await Promise.resolve();
  });
}

function timelineItem(project: VideoProject, itemId: string | undefined): TimelineItem | undefined {
  return project.timeline.tracks.flatMap((track) => track.items).find((item) => item.id === itemId);
}

function tile(name: string): HTMLElement {
  const preview = screen.getByRole("button", { name: `Preview ${name}` });
  const element = preview.closest("li");
  if (!element) throw new Error(`no tile for ${name}`);
  return element;
}

describe("TextPanel", () => {
  beforeAll(() => installPointerEventPolyfill());

  beforeEach(() => {
    window.localStorage.clear();
    appliedBatches.length = 0;
    animations.length = 0;
    stubMediaQueries();
    HTMLElement.prototype.animate = function animate(keyframes: Keyframe[] | PropertyIndexedKeyframes | null) {
      const animation = { keyframes: keyframes as Keyframe[], cancel: vi.fn() };
      animations.push(animation);
      return animation as unknown as Animation;
    };
  });

  afterEach(() => {
    Reflect.deleteProperty(window, "matchMedia");
    Reflect.deleteProperty(HTMLElement.prototype, "animate");
  });

  it("adds a default text overlay at the playhead in one batch and selects it", async () => {
    const { store } = renderPanel(2);
    fireEvent.click(screen.getByRole("button", { name: "Add text" }));
    await settle();

    expect(appliedBatches).toHaveLength(1);
    expect(appliedBatches[0]?.at(-1)).toMatchObject({ type: "addItems", items: [{ kind: "overlay", startSeconds: 2 }] });
    const { project, selectedItemIds, history } = store.getState();
    expect(selectedItemIds).toHaveLength(1);
    expect(timelineItem(project, selectedItemIds[0])).toMatchObject({
      kind: "overlay",
      label: "Text",
      durationSeconds: 3,
      source: { type: "text", text: "Your text" },
    });
    expect(history.past).toHaveLength(1);
  });

  it("shows text styles and the titles grouped by category", () => {
    renderPanel();
    const styles = screen.getByRole("list", { name: "Text styles" });
    expect(within(styles).getAllByRole("button", { name: /^Add / }).map((button) => button.getAttribute("aria-label"))).toEqual([
      "Add Punchy Caption",
      "Add Gradient Background Loop",
    ]);
    expect(screen.getByRole("heading", { name: "Titles & lower thirds" })).toBeInTheDocument();
    expect(screen.getAllByRole("heading", { level: 3 }).map((heading) => heading.textContent)).toEqual(["Titles", "Lower thirds", "Callouts"]);
    expect(within(screen.getByRole("list", { name: "Lower thirds" })).getByRole("button", { name: "Add Kinetic Lower Third" })).toBeInTheDocument();
  });

  it("inserts the chosen template at the playhead and selects it", async () => {
    const { store } = renderPanel(1.5);
    fireEvent.click(screen.getByRole("button", { name: "Add Kinetic Lower Third" }));
    await settle();

    expect(appliedBatches).toHaveLength(1);
    const { project, selectedItemIds } = store.getState();
    expect(timelineItem(project, selectedItemIds[0])).toMatchObject({
      kind: "overlay",
      startSeconds: 1.5,
      properties: { templateId: "kinetic-lower-third-v1" },
    });
  });

  it("drags a tile as a template asset", () => {
    renderPanel();
    const dataTransfer = fakeDataTransfer();
    const element = tile("Metric Callout");
    expect(element).toHaveAttribute("draggable", "true");
    fireEvent.dragStart(element, { dataTransfer });
    expect(JSON.parse(dataTransfer.getData(assetDragMimeType))).toEqual({ kind: "template", id: "metric-callout-v1" });
  });

  it("animates a tile preview on hover and stops on leave", () => {
    renderPanel();
    const element = tile("Chapter Card");
    expect(within(element).getByRole("button", { name: "Add Chapter Card" })).toHaveClass("opacity-0");
    fireEvent.pointerEnter(element, { pointerType: "mouse" });
    expect(animations).toHaveLength(1);
    fireEvent.pointerLeave(element, { pointerType: "mouse" });
    expect(animations[0]?.cancel).toHaveBeenCalledTimes(1);
  });

  it("does not animate with reduced motion", () => {
    stubMediaQueries("(prefers-reduced-motion: reduce)");
    renderPanel();
    const element = tile("Chapter Card");
    fireEvent.pointerEnter(element, { pointerType: "mouse" });
    fireEvent.click(within(element).getByRole("button", { name: "Preview Chapter Card" }));
    expect(animations).toHaveLength(0);
  });

  it("previews on tap on touch devices, one tile at a time, with the add button visible", () => {
    stubMediaQueries("(hover: none)");
    renderPanel();
    const chapter = tile("Chapter Card");
    expect(within(chapter).getByRole("button", { name: "Add Chapter Card" })).not.toHaveClass("opacity-0");

    fireEvent.pointerEnter(chapter, { pointerType: "touch" });
    expect(animations).toHaveLength(0);

    const chapterPreview = within(chapter).getByRole("button", { name: "Preview Chapter Card" });
    fireEvent.click(chapterPreview);
    expect(chapterPreview).toHaveAttribute("aria-pressed", "true");
    expect(animations).toHaveLength(1);

    fireEvent.click(screen.getByRole("button", { name: "Preview Metric Callout" }));
    expect(chapterPreview).toHaveAttribute("aria-pressed", "false");
    expect(animations[0]?.cancel).toHaveBeenCalledTimes(1);
    expect(animations).toHaveLength(2);
  });
});
