import "@testing-library/jest-dom/vitest";
import { act, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { TooltipProvider } from "@/components/ui/tooltip";
import { installPointerEventPolyfill } from "@/test-utils/editor-render";
import { fixtureItem, fixtureProject } from "@/test-utils/editor-fixtures";
import { createEditorStore } from "../store/editor-store";
import { EditorStoreProvider } from "../store/editor-store-context";
import { MobileLayout } from "./mobile-layout";

vi.mock("@/lib/runtime/backend-client", () => ({ backendRequest: vi.fn(), backendListen: vi.fn(), backendMediaUrl: (p: string) => p }));

function renderMobile() {
  const project = fixtureProject();
  const store = createEditorStore({ projectDir: "/p", project });
  render(
    <TooltipProvider>
      <EditorStoreProvider store={store}>
        <MobileLayout topBar={<div>top</div>} />
      </EditorStoreProvider>
    </TooltipProvider>,
  );
  return { store, project };
}

describe("MobileLayout", () => {
  beforeEach(() => window.localStorage.clear());

  it("stacks preview and timeline above a six-tab bottom tool bar", () => {
    renderMobile();
    expect(screen.getByRole("region", { name: "Preview viewport" })).toBeInTheDocument();
    expect(screen.getByRole("region", { name: "Timeline canvas" })).toBeInTheDocument();
    const toolbar = screen.getByRole("toolbar", { name: "Editor tools" });
    expect(Array.from(toolbar.querySelectorAll("button")).map((button) => button.textContent)).toEqual([
      "AI", "Media", "Audio", "Text", "Captions", "Effects",
    ]);
  });

  it("opens a tab as a labelled bottom sheet and closes it", () => {
    const { store } = renderMobile();
    fireEvent.click(screen.getByRole("button", { name: "Media" }));
    expect(store.getState().openSheetId).toBe("media");
    expect(screen.getByRole("dialog", { name: "Media" })).toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: "Close Media" }));
    expect(screen.queryByRole("dialog", { name: "Media" })).not.toBeInTheDocument();
  });

  it("keeps one sheet open at a time: opening another replaces it", () => {
    const { store, project } = renderMobile();
    fireEvent.click(screen.getByRole("button", { name: "Media" }));
    act(() => store.getState().openSheet("captions"));
    expect(screen.getAllByRole("dialog")).toHaveLength(1);
    expect(screen.getByRole("dialog", { name: "Captions" })).toBeInTheDocument();

    // A clip tool's property sheet replaces a tab sheet too.
    act(() => {
      store.getState().selectItems([fixtureItem(project, "video").id]);
      store.getState().openSheet("property:adjust");
    });
    expect(screen.getAllByRole("dialog")).toHaveLength(1);
    expect(screen.getByRole("dialog", { name: "Adjust" })).toBeInTheDocument();
  });

  it("closes a tab sheet by dragging its grab handle down", () => {
    installPointerEventPolyfill();
    const { store } = renderMobile();
    fireEvent.click(screen.getByRole("button", { name: "Media" }));
    const handle = screen.getByRole("button", { name: "Dismiss Media sheet" });
    fireEvent.pointerDown(handle, { pointerId: 1, pointerType: "touch", clientY: 300 });
    fireEvent.pointerMove(handle, { pointerId: 1, pointerType: "touch", clientY: 420 });
    fireEvent.pointerUp(handle, { pointerId: 1, pointerType: "touch", clientY: 420 });
    expect(store.getState().openSheetId).toBeNull();
    expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
  });

  it("pads the bottom tool bar above the home indicator", () => {
    renderMobile();
    const bar = screen.getByRole("navigation", { name: "Bottom tool bar" });
    expect(bar).toHaveClass("pb-[env(safe-area-inset-bottom)]");
    expect(bar.style.height).toBe("calc(78px + env(safe-area-inset-bottom))");
  });

  it("opens the AI sheet for a handoff with no sheet open, filling the chip and focusing the composer", async () => {
    const { store, project } = renderMobile();
    const video = fixtureItem(project, "video");
    // The timeline context menu's Ask AI runs with the clip selected and no sheet open.
    act(() => {
      store.getState().selectItems([video.id]);
      store.getState().requestAgent({ itemIds: [video.id] });
    });
    const sheet = screen.getByRole("dialog", { name: "AI" });
    const composer = within(sheet).getByRole("textbox", { name: "Describe an edit" });
    await waitFor(() => expect(composer).toHaveFocus());
    expect(within(sheet).getByRole("button", { name: `Remove ${video.label} from the request` })).toBeInTheDocument();
    expect(store.getState().pendingAgentRequest).toBeNull();
  });

  it("swaps the Media sheet for the AI sheet with the Organize with AI confirmation", async () => {
    const { store } = renderMobile();
    fireEvent.click(screen.getByRole("button", { name: "Media" }));
    act(() => store.getState().requestAgent({ itemIds: [], prompt: "Organize the current project media", confirmSend: true }));
    expect(screen.queryByRole("dialog", { name: "Media" })).not.toBeInTheDocument();
    const sheet = screen.getByRole("dialog", { name: "AI" });
    const confirmation = await within(sheet).findByRole("group", { name: "Send this request?" });
    expect(within(confirmation).getByRole("button", { name: "Send" })).toHaveFocus();
    expect(within(sheet).getByRole("textbox", { name: "Describe an edit" })).toHaveValue("Organize the current project media");
  });

  it("switches to clip tools while an item is selected and back", () => {
    const { store, project } = renderMobile();
    act(() => store.getState().selectItems([fixtureItem(project, "video").id]));
    expect(screen.getByRole("toolbar", { name: "Clip tools" })).toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: "Back to editor tools" }));
    expect(store.getState().selectedItemIds).toEqual([]);
    expect(screen.getByRole("toolbar", { name: "Editor tools" })).toBeInTheDocument();
  });

  it("lists the clip tools for each selection kind in bar order", () => {
    const { store, project } = renderMobile();
    const video = fixtureItem(project, "video");
    const caption = fixtureItem(project, "caption");
    const audio = fixtureItem(project, "audio");
    const toolNames = () =>
      within(screen.getByRole("toolbar", { name: "Clip tools" }))
        .getAllByRole("button")
        .map((button) => button.getAttribute("aria-label") ?? button.textContent);
    act(() => store.getState().selectItems([video.id]));
    expect(toolNames()).toEqual(["Back to editor tools", "Split", "Speed", "Volume", "Animation", "Effects", "Adjust", "Detach audio", "Delete", "AI"]);
    act(() => store.getState().selectItems([caption.id]));
    expect(toolNames()).toEqual(["Back to editor tools", "Split", "Text", "Style", "Animation", "Delete"]);
    act(() => store.getState().selectItems([audio.id]));
    expect(toolNames()).toEqual(["Back to editor tools", "Split", "Speed", "Volume", "Delete"]);
    act(() => store.getState().selectItems([video.id, audio.id]));
    expect(toolNames()).toEqual(["Back to editor tools", "Split", "Adjust", "Delete"]);
  });

  it("detaches the selected video clip's audio from the clip tools", async () => {
    const { store, project } = renderMobile();
    const video = fixtureItem(project, "video");
    act(() => store.getState().selectItems([video.id]));
    const before = store.getState().project.timeline.tracks.flatMap((track) => track.items).length;
    await act(async () => {
      fireEvent.click(within(screen.getByRole("toolbar", { name: "Clip tools" })).getByRole("button", { name: "Detach audio" }));
      await Promise.resolve();
    });
    const items = () => store.getState().project.timeline.tracks.flatMap((track) => track.items);
    await waitFor(() => expect(items()).toHaveLength(before + 1));
    expect(items().some((item) => item.id === `${video.id}-audio` && item.kind === "audio_clip")).toBe(true);
  });

  it("splits the selected clip at the playhead from the Split tool as one undo step", async () => {
    const { store, project } = renderMobile();
    const item = fixtureItem(project, "video");
    act(() => {
      store.getState().selectItems([item.id]);
      store.getState().seek(item.startSeconds + item.durationSeconds / 2);
    });
    const count = () => store.getState().project.timeline.tracks.flatMap((track) => track.items).length;
    const before = count();
    await act(async () => {
      fireEvent.click(screen.getByRole("button", { name: "Split" }));
      await Promise.resolve();
    });
    expect(count()).toBe(before + 1);
    expect(store.getState().history.past).toHaveLength(1);
  });

  it("reports a blocked Split tap in the timeline feedback", () => {
    const { store, project } = renderMobile();
    const item = fixtureItem(project, "video");
    act(() => {
      store.getState().selectItems([item.id]);
      store.getState().seek(item.startSeconds + item.durationSeconds + 1);
    });
    const split = screen.getByRole("button", { name: "Split" });
    expect(split).toHaveAttribute("aria-disabled", "true");
    fireEvent.click(split);
    expect(store.getState().lastError).toMatch(/playhead/i);
    expect(store.getState().history.past).toHaveLength(0);
  });

  it("deletes the selection", async () => {
    const { store, project } = renderMobile();
    const item = fixtureItem(project, "video");
    act(() => store.getState().selectItems([item.id]));
    await act(async () => {
      fireEvent.click(screen.getByRole("button", { name: "Delete" }));
      await Promise.resolve();
    });
    expect(store.getState().project.timeline.tracks.flatMap((track) => track.items).some((entry) => entry.id === item.id)).toBe(false);
  });

  it("opens Adjust as a compact sheet with the Transform section and commits scale as one undo step", async () => {
    const { store, project } = renderMobile();
    const item = fixtureItem(project, "video");
    act(() => store.getState().selectItems([item.id]));
    fireEvent.click(screen.getByRole("button", { name: "Adjust" }));
    expect(store.getState().openSheetId).toBe("property:adjust");
    const sheet = screen.getByRole("dialog", { name: "Adjust" });
    expect(sheet).toHaveClass("h-[40dvh]");
    // Property sheets sit above the clip tools, which stay tappable.
    expect(sheet.style.bottom).toBe("calc(78px + env(safe-area-inset-bottom))");
    expect(within(sheet).getByRole("heading", { name: "Transform" })).toBeInTheDocument();
    for (const name of ["Transform", "Blend", "Crop", "Fade"]) expect(within(sheet).getByRole("tab", { name })).toBeInTheDocument();

    await act(async () => {
      const scale = within(sheet).getByRole("textbox", { name: "Scale" });
      fireEvent.change(scale, { target: { value: "150" } });
      fireEvent.keyDown(scale, { key: "Enter" });
    });
    expect(store.getState().history.past).toHaveLength(1);

    fireEvent.click(within(sheet).getByRole("button", { name: "Close Adjust" }));
    expect(screen.queryByRole("dialog", { name: "Adjust" })).not.toBeInTheDocument();
    expect(store.getState().openSheetId).toBeNull();
  });

  it("opens Speed, Volume, Animation and Effects sheets from the matching Properties tabs", () => {
    const { store, project } = renderMobile();
    act(() => store.getState().selectItems([fixtureItem(project, "video").id]));
    const open = (name: string) => {
      fireEvent.click(screen.getByRole("button", { name }));
      return screen.getByRole("dialog", { name });
    };
    const speed = open("Speed");
    expect(within(speed).getByRole("group", { name: "Speed presets" })).toBeInTheDocument();
    expect(within(speed).getByRole("switch", { name: "Reverse" })).not.toBeChecked();
    // The sample clip's sound has no linked audio clip, so the Audio tab offers Detach audio instead.
    expect(within(open("Volume")).getByText(/isn't on its own audio clip yet/)).toBeInTheDocument();
    expect(within(open("Animation")).getByRole("heading", { name: "Keyframes" })).toBeInTheDocument();
    const effects = open("Effects");
    expect(within(effects).getByRole("heading", { level: 3, name: "Effects" })).toBeInTheDocument();
    expect(within(effects).queryByRole("heading", { name: "Transform" })).not.toBeInTheDocument();
    expect(screen.getAllByRole("dialog")).toHaveLength(1);
  });

  it("opens the audio clip Basic tab from Volume and caption tabs from Text and Style", () => {
    const { store, project } = renderMobile();
    act(() => store.getState().selectItems([fixtureItem(project, "audio").id]));
    fireEvent.click(screen.getByRole("button", { name: "Volume" }));
    expect(within(screen.getByRole("dialog", { name: "Volume" })).getByRole("heading", { name: "Fade" })).toBeInTheDocument();

    act(() => store.getState().selectItems([fixtureItem(project, "caption").id]));
    // Volume does not exist for captions, so its sheet closes with the selection change.
    expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: "Style" }));
    expect(within(screen.getByRole("dialog", { name: "Style" })).getByRole("radiogroup", { name: "Applies to" })).toBeInTheDocument();
  });

  it("closes a property sheet when the selection clears or crop mode starts", () => {
    const { store, project } = renderMobile();
    const item = fixtureItem(project, "video");
    act(() => store.getState().selectItems([item.id]));
    const cropModes: (string | null)[] = [];
    store.subscribe((state) => cropModes.push(state.cropModeItemId));
    fireEvent.click(screen.getByRole("button", { name: "Adjust" }));
    fireEvent.mouseDown(screen.getByRole("tab", { name: "Crop" }), { button: 0 });
    fireEvent.click(screen.getByRole("button", { name: "Edit on canvas" }));
    expect(cropModes).toContain(item.id);
    expect(screen.queryByRole("dialog", { name: "Adjust" })).not.toBeInTheDocument();

    fireEvent.click(screen.getByRole("button", { name: "Volume" }));
    expect(screen.getByRole("dialog", { name: "Volume" })).toBeInTheDocument();
    act(() => store.getState().clearSelection());
    expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
    expect(store.getState().openSheetId).toBeNull();
  });
});
