import "@testing-library/jest-dom/vitest";
import { act, fireEvent, screen, within } from "@testing-library/react";
import { afterEach, beforeAll, beforeEach, describe, expect, it, vi } from "vitest";
import type { VideoProject } from "@/lib/project";
import { BackendUnavailableError } from "@/lib/runtime/backend-transport";
import { findTimelineItem } from "@/lib/preview/canvas-geometry";
import { installPointerEventPolyfill, renderWithEditorStore } from "@/test-utils/editor-render";
import { fixtureProject, fixtureTrack } from "@/test-utils/editor-fixtures";
import { requiredValue } from "@/test-utils/required";

const backendRequest = vi.fn();
vi.mock("@/lib/runtime/backend-client", () => ({
  backendRequest: (...args: unknown[]) => backendRequest(...args),
  backendListen: vi.fn(),
  backendMediaUrl: (p: string) => p,
}));

const { PreviewPanel } = await import("./preview-panel");
const { installMediaAndFrameStubs } = await import("./media-element-stubs");

/** Caption 1 without a transcript word link, plus a plain text overlay; both active at t=1. */
function textProject(): VideoProject {
  const project = fixtureProject();
  const caption = requiredValue(fixtureTrack(project, "caption").items[0], "caption-1");
  delete caption.properties.transcriptId;
  delete caption.properties.wordIndex;
  fixtureTrack(project, "overlay").items.push({
    id: "title",
    kind: "overlay",
    startSeconds: 0,
    durationSeconds: 4,
    source: { type: "text", text: "Title" },
    label: "Title",
    properties: {},
  });
  return project;
}

function renderText(project = textProject()) {
  const result = renderWithEditorStore(<PreviewPanel />, { project, projectDir: "/p" });
  act(() => result.store.getState().seek(1));
  return result;
}

function viewport() {
  return screen.getByRole("region", { name: "Preview viewport" });
}

function item(project: VideoProject, itemId: string) {
  return requiredValue(findTimelineItem(project.timeline, itemId) ?? undefined, itemId);
}

async function settle() {
  await act(async () => {
    await Promise.resolve();
    await Promise.resolve();
  });
}

describe("inline text editing on the preview canvas", () => {
  beforeAll(() => installPointerEventPolyfill());

  beforeEach(() => {
    window.localStorage.clear();
    backendRequest.mockReset();
    backendRequest.mockRejectedValue(new BackendUnavailableError());
    installMediaAndFrameStubs();
  });

  afterEach(() => {
    vi.unstubAllGlobals();
    vi.restoreAllMocks();
  });

  it("selects a caption on click and commits edited caption text on Enter", async () => {
    const { store } = renderText();
    const caption = within(viewport()).getByText("Original caption text");
    fireEvent.pointerDown(caption, { button: 0, pointerId: 1 });
    expect(store.getState().selectedItemIds).toEqual(["caption-1"]);

    fireEvent.doubleClick(caption);
    const editor = screen.getByRole("textbox", { name: "Edit caption Caption 1" });
    expect(editor).toHaveFocus();
    expect(editor).toHaveTextContent("Original caption text");
    expect(store.getState().editingTextItemId).toBe("caption-1");

    editor.textContent = "Fixed caption text";
    fireEvent.keyDown(editor, { key: " " });
    expect(store.getState().playing).toBe(false);
    fireEvent.keyDown(editor, { key: "Enter" });
    await settle();

    expect(screen.queryByRole("textbox", { name: "Edit caption Caption 1" })).not.toBeInTheDocument();
    expect(store.getState().editingTextItemId).toBeNull();
    expect(item(store.getState().project, "caption-1").source).toEqual({ type: "text", text: "Fixed caption text" });
    expect(store.getState().history.past).toHaveLength(1);
  });

  it("commits edited text for a multi-word sample caption linked to a transcript word", async () => {
    const { store } = renderText(fixtureProject());
    fireEvent.doubleClick(within(viewport()).getByText("Original caption text"));
    const editor = screen.getByRole("textbox", { name: "Edit caption Caption 1" });
    editor.textContent = "Restored voice";
    fireEvent.keyDown(editor, { key: "Enter" });
    await settle();

    const caption = item(store.getState().project, "caption-1");
    expect(caption.source).toEqual({ type: "text", text: "Restored voice" });
    expect(caption.properties).toMatchObject({ textEdited: true, transcriptId: "transcript-media-1", wordIndex: 0 });
    expect(store.getState().history.past).toHaveLength(1);
    expect(within(viewport()).getByText("Restored voice")).toBeInTheDocument();
  });

  it("leaves the project unchanged on Escape", async () => {
    const { store } = renderText();
    fireEvent.doubleClick(within(viewport()).getByText("Original caption text"));
    const editor = screen.getByRole("textbox", { name: "Edit caption Caption 1" });
    editor.textContent = "Discarded";
    fireEvent.keyDown(editor, { key: "Escape" });
    await settle();
    expect(screen.queryByRole("textbox")).not.toBeInTheDocument();
    expect(store.getState().history.past).toHaveLength(0);
    expect(within(viewport()).getByText("Original caption text")).toBeInTheDocument();
  });

  it("commits trimmed text overlay text on blur with the default guidance metadata", async () => {
    const { store } = renderText();
    fireEvent.doubleClick(within(viewport()).getByText("Title"));
    const editor = screen.getByRole("textbox", { name: "Edit text Title" });
    editor.textContent = "  New title  ";
    fireEvent.blur(editor);
    await settle();
    const title = item(store.getState().project, "title");
    expect(title.source).toEqual({ type: "text", text: "New title" });
    expect(title.properties).toMatchObject({
      visualTreatment: "clean editable text overlay with high-contrast type and transparent backing",
      motion: "quick fade in, hold, and soft fade out",
      safeZone: "keep text inside 10% title-safe margins",
      avoid: "opaque slabs, default-font template look, and covering faces or key action",
    });
    expect(store.getState().history.past).toHaveLength(1);
  });

  it("does not commit blank or unchanged text", async () => {
    const { store } = renderText();
    fireEvent.doubleClick(within(viewport()).getByText("Title"));
    const blank = screen.getByRole("textbox", { name: "Edit text Title" });
    blank.textContent = "   ";
    fireEvent.keyDown(blank, { key: "Enter" });
    await settle();
    fireEvent.doubleClick(within(viewport()).getByText("Title"));
    fireEvent.keyDown(screen.getByRole("textbox", { name: "Edit text Title" }), { key: "Enter" });
    await settle();
    expect(screen.queryByRole("textbox")).not.toBeInTheDocument();
    expect(store.getState().history.past).toHaveLength(0);
  });

  it("stops editing without a commit when the playhead leaves the layer", async () => {
    const { store } = renderText();
    fireEvent.doubleClick(within(viewport()).getByText("Original caption text"));
    expect(screen.getByRole("textbox", { name: "Edit caption Caption 1" })).toBeInTheDocument();
    act(() => store.getState().seek(3));
    await settle();
    expect(store.getState().editingTextItemId).toBeNull();
    act(() => store.getState().seek(1));
    expect(screen.queryByRole("textbox")).not.toBeInTheDocument();
    expect(store.getState().history.past).toHaveLength(0);
  });

  it("does not edit text on a locked track", () => {
    const project = textProject();
    fixtureTrack(project, "caption").locked = true;
    const { store } = renderText(project);
    fireEvent.doubleClick(within(viewport()).getByText("Original caption text"));
    expect(screen.queryByRole("textbox")).not.toBeInTheDocument();
    expect(store.getState().editingTextItemId).toBeNull();
  });
});
