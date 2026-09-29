import "@testing-library/jest-dom/vitest";
import { act, fireEvent, screen, waitFor, within } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import type { VideoProject } from "@/lib/project";
import { backendRequest } from "@/lib/runtime/backend-client";
import type { TimelineItem, TimelineTrack } from "@/lib/timeline";
import { renderWithEditorStore } from "@/test-utils/editor-render";
import { fixtureProject } from "@/test-utils/editor-fixtures";
import { TimelinePanel } from "./timeline-panel";
import { crossfade, transitionsOf, transitionTestProject } from "./transition-test-project";

vi.mock("@/lib/runtime/backend-client", () => ({ backendRequest: vi.fn(), backendListen: vi.fn(), backendMediaUrl: (p: string) => p }));

function clip(id: string, startSeconds: number, durationSeconds: number, patch: Partial<TimelineItem> = {}): TimelineItem {
  return { id, kind: "video_clip", startSeconds, durationSeconds, source: { type: "media", mediaId: "media-1" }, label: id, properties: {}, ...patch };
}

function track(id: string, kind: TimelineTrack["kind"], items: TimelineItem[]): TimelineTrack {
  return { id, name: id, kind, locked: false, enabled: true, items };
}

/** 80 px/s: "a" 0–2 s and "b" 4–6 s on Video 1, "m" 0–3 s on Audio 1. */
function projectWith(extra: TimelineItem[] = []): VideoProject {
  const project = fixtureProject();
  project.timeline = {
    durationSeconds: 30,
    tracks: [
      track("v1", "video", [clip("a", 0, 2, { properties: { sourceIn: 0, sourceOut: 2 } }), clip("b", 4, 2), ...extra]),
      track("a1", "audio", [clip("m", 0, 3, { kind: "audio_clip", source: { type: "media", mediaId: "media-voiceover" } })]),
    ],
  };
  return project;
}

function option(itemId: string): HTMLElement {
  const element = screen.getAllByRole("option").find((candidate) => candidate.dataset.itemId === itemId);
  if (!element) throw new Error(`no option for ${itemId}`);
  return element;
}

async function openClipMenu(itemId: string) {
  fireEvent.contextMenu(option(itemId), { clientX: 200, clientY: 20 });
  return within(await screen.findByRole("menu", { name: "Clip actions" }));
}

async function select(menu: ReturnType<typeof within>, name: string) {
  await act(async () => {
    fireEvent.click(menu.getByRole("menuitem", { name: new RegExp(`^${name}`) }));
    await Promise.resolve();
  });
}

function itemNames(menu: ReturnType<typeof within>): string[] {
  return menu.getAllByRole("menuitem").map((item: HTMLElement) => item.querySelector("span")?.textContent ?? "");
}

describe("TimelineContextMenu", () => {
  beforeEach(() => {
    window.localStorage.clear();
    vi.mocked(backendRequest).mockReset();
  });

  it("lists the full clip menu for a video clip without the nested-only entry", async () => {
    const { store } = renderWithEditorStore(<TimelinePanel />, { project: projectWith() });
    const menu = await openClipMenu("b");
    expect(store.getState().selectedItemIds).toEqual(["b"]);
    expect(itemNames(menu)).toEqual([
      "Cut",
      "Copy",
      "Paste",
      "Paste insert",
      "Duplicate",
      "Delete",
      "Ripple delete",
      "Split at playhead",
      "Set duration…",
      "Speed…",
      "Reverse",
      "Link",
      "Detach audio",
      "Replace with media…",
      "Reveal in Media",
      "Save range as media",
      "Add transition",
      "Ask AI about this clip",
      "Lock track",
      "Hide track",
    ]);
    expect(menu.getByRole("menuitem", { name: /^Copy/ })).toHaveTextContent("⌘C");

    expect(menu.getByRole("menuitem", { name: "Save range as media" })).not.toHaveAttribute("aria-disabled");
    expect(menu.getByRole("menuitem", { name: /^Paste insert/ })).toHaveAttribute("aria-disabled", "true");
  });

  it("detaches a video clip's audio and disables Detach audio once it is linked", async () => {
    const { store } = renderWithEditorStore(<TimelinePanel />, { project: projectWith() });
    await select(await openClipMenu("b"), "Detach audio");
    await waitFor(() => expect(store.getState().selectedItemIds).toEqual(["b-audio"]));
    const audio = store.getState().project.timeline.tracks.flatMap((track) => track.items).find((entry) => entry.id === "b-audio");
    expect(audio).toMatchObject({ kind: "audio_clip", startSeconds: 4, durationSeconds: 2 });

    const menu = await openClipMenu("b");
    const detach = menu.getByRole("menuitem", { name: "Detach audio" });
    expect(detach).toHaveAttribute("aria-disabled", "true");
    fireEvent.focus(detach);
    expect(await screen.findByRole("tooltip")).toHaveTextContent("This clip's sound is already on a linked audio clip.");
    fireEvent.keyDown(screen.getByRole("menu"), { key: "Escape" });
    fireEvent.keyDown(screen.getByRole("menu"), { key: "Escape" });
    await waitFor(() => expect(screen.queryByRole("menu")).not.toBeInTheDocument());

    const audioMenu = await openClipMenu("m");
    expect(audioMenu.queryByRole("menuitem", { name: "Detach audio" })).not.toBeInTheDocument();
  });

  it("reverses a clip with its linked audio as one undo step and plays it forward again", async () => {
    const project = projectWith();
    for (const track of project.timeline.tracks) {
      for (const item of track.items) if (item.id === "b" || item.id === "m") item.properties.linkGroupId = "link-1";
    }
    const { store } = renderWithEditorStore(<TimelinePanel />, { project });
    const reversedIds = () =>
      store.getState().project.timeline.tracks.flatMap((track) => track.items).filter((item) => item.properties.reverse === true).map((item) => item.id);
    await select(await openClipMenu("b"), "Reverse");
    await waitFor(() => expect(reversedIds()).toEqual(["b", "m"]));
    expect(store.getState().history.past).toHaveLength(1);

    const menu = await openClipMenu("m");
    expect(menu.queryByRole("menuitem", { name: "Reverse" })).not.toBeInTheDocument();
    await select(menu, "Play forward");
    await waitFor(() => expect(reversedIds()).toEqual([]));
    expect(store.getState().history.past).toHaveLength(2);
  });

  it("shows Reverse only for video and audio clips, disabled with the reason on a locked track", async () => {
    const project = projectWith([clip("still", 8, 2, { kind: "image_clip" })]);
    const video = project.timeline.tracks[0];
    if (video) video.locked = true;
    renderWithEditorStore(<TimelinePanel />, { project });
    const imageMenu = await openClipMenu("still");
    expect(imageMenu.queryByRole("menuitem", { name: "Reverse" })).not.toBeInTheDocument();
    fireEvent.keyDown(screen.getByRole("menu"), { key: "Escape" });
    await waitFor(() => expect(screen.queryByRole("menu")).not.toBeInTheDocument());

    const reverse = (await openClipMenu("b")).getByRole("menuitem", { name: "Reverse" });
    expect(reverse).toHaveAttribute("aria-disabled", "true");
    fireEvent.focus(reverse);
    expect(await screen.findByRole("tooltip")).toHaveTextContent("Unlock the track to reverse this clip.");
  });

  it("saves the clicked clip's timeline span as media", async () => {
    vi.mocked(backendRequest).mockRejectedValue(new Error("Render stopped."));
    renderWithEditorStore(<TimelinePanel />, { project: { ...projectWith(), schemaVersion: 2 }, projectDir: "/projects/edison" });
    await select(await openClipMenu("b"), "Save range as media");
    await waitFor(() =>
      expect(backendRequest).toHaveBeenCalledWith(
        "render_media_to_split_project_folder",
        expect.objectContaining({ projectDir: "/projects/edison", jobId: expect.stringMatching(/^save-range-/), rangeStartSeconds: 4, rangeEndSeconds: 6 }),
      ),
    );
  });

  it("saves the I/O range from the lane menu, and explains when there is no range", async () => {
    vi.mocked(backendRequest).mockRejectedValue(new Error("Render stopped."));
    const { store } = renderWithEditorStore(<TimelinePanel />, { project: { ...projectWith(), schemaVersion: 2 }, projectDir: "/projects/edison" });
    const lane = screen.getByRole("listbox", { name: "Video 1" });
    fireEvent.contextMenu(lane, { clientX: 240, clientY: 20 });
    const menu = within(await screen.findByRole("menu", { name: "Track actions" }));
    const saveRange = menu.getByRole("menuitem", { name: "Save range as media" });
    expect(saveRange).toHaveAttribute("aria-disabled", "true");
    fireEvent.focus(saveRange);
    expect(await screen.findByRole("tooltip")).toHaveTextContent("Select a clip or mark in and out points");
    fireEvent.keyDown(screen.getByRole("menu"), { key: "Escape" });
    fireEvent.keyDown(screen.getByRole("menu"), { key: "Escape" });
    await waitFor(() => expect(screen.queryByRole("menu")).not.toBeInTheDocument());

    act(() => {
      store.getState().setRangeIn(2);
      store.getState().setRangeOut(5);
    });
    fireEvent.contextMenu(lane, { clientX: 240, clientY: 20 });
    await select(within(await screen.findByRole("menu", { name: "Track actions" })), "Save range as media");
    await waitFor(() =>
      expect(backendRequest).toHaveBeenCalledWith("render_media_to_split_project_folder", expect.objectContaining({ rangeStartSeconds: 2, rangeEndSeconds: 5 })),
    );
  });

  it("disables Add transition with a reason when the clip touches no other clip", async () => {
    renderWithEditorStore(<TimelinePanel />, { project: projectWith() });
    const menu = await openClipMenu("b");
    const addTransition = menu.getByRole("menuitem", { name: "Add transition" });
    expect(addTransition).toHaveAttribute("aria-disabled", "true");
    fireEvent.focus(addTransition);
    expect(await screen.findByRole("tooltip")).toHaveTextContent("Place two clips next to each other first");
  });

  it("adds one transition from the clip menu on the cut nearest the right-click", async () => {
    const { store } = renderWithEditorStore(<TimelinePanel />, { project: transitionTestProject() });
    // "b" spans 2–4 s (x 160–320 in the lane); right-clicking near its left edge picks the 2 s cut.
    fireEvent.contextMenu(option("b"), { clientX: 180, clientY: 20 });
    const menu = within(await screen.findByRole("menu", { name: "Clip actions" }));
    const trigger = menu.getByRole("menuitem", { name: "Add transition" });
    expect(trigger).toHaveAttribute("aria-haspopup", "menu");
    fireEvent.click(trigger);
    const submenu = within(await screen.findByRole("menu", { name: "Add transition" }));
    expect(submenu.getAllByRole("menuitem").map((item: HTMLElement) => item.textContent)).toEqual(["Crossfade", "Dip to black", "Dip to white", "Wipe"]);
    await select(submenu, "Dip to white");
    expect(transitionsOf(store.getState().project)).toEqual([
      { id: "transition-a-b", leftItemId: "a", rightItemId: "b", kind: "dipToWhite", durationSeconds: 0.5 },
    ]);
    expect(store.getState().history.past).toHaveLength(1);
    expect(store.getState().selectedTransitionId).toBe("transition-a-b");
  });

  it("changes or deletes a transition from its badge menu", async () => {
    const { store } = renderWithEditorStore(<TimelinePanel />, { project: transitionTestProject(undefined, [crossfade()]) });
    fireEvent.contextMenu(screen.getByRole("button", { name: "Crossfade transition, 0.5s" }), { clientX: 160, clientY: 20 });
    const menu = within(await screen.findByRole("menu", { name: "Transition actions" }));
    expect(store.getState().selectedTransitionId).toBe("fade");
    fireEvent.click(menu.getByRole("menuitem", { name: "Change transition" }));
    const submenu = within(await screen.findByRole("menu", { name: "Change transition" }));
    expect(submenu.getByRole("menuitem", { name: "Crossfade (current)" })).toBeInTheDocument();
    await select(submenu, "Wipe");
    expect(transitionsOf(store.getState().project)).toMatchObject([{ id: "fade", kind: "wipe" }]);

    fireEvent.contextMenu(screen.getByRole("button", { name: "Wipe transition, 0.5s" }), { clientX: 160, clientY: 20 });
    await select(within(await screen.findByRole("menu", { name: "Transition actions" })), "Delete transition");
    expect(transitionsOf(store.getState().project)).toEqual([]);
    expect(store.getState().history.past).toHaveLength(2);
  });

  it("offers decompose only for nested sequence clips", async () => {
    const nested = clip("n", 8, 2, { source: { type: "timeline", timelineId: "intro" }, label: "Intro" });
    renderWithEditorStore(<TimelinePanel />, { project: projectWith([nested]) });
    const menu = await openClipMenu("n");
    expect(menu.getByRole("menuitem", { name: "Decompose nested sequence" })).toBeInTheDocument();
    expect(menu.getByRole("menuitem", { name: "Reveal in Media" })).toHaveAttribute("aria-disabled", "true");
  });

  it("sets the replace target, revealed media and agent request and opens their tabs", async () => {
    const { store } = renderWithEditorStore(<TimelinePanel />, { project: projectWith() });
    await select(await openClipMenu("a"), "Replace with media…");
    expect(store.getState()).toMatchObject({ replaceTargetItemId: "a", activeTab: "media" });

    act(() => store.getState().setActiveTab("text"));
    await select(await openClipMenu("m"), "Reveal in Media");
    expect(store.getState()).toMatchObject({ revealMediaId: "media-voiceover", activeTab: "media" });

    act(() => store.getState().selectItems(["a", "b"]));
    await select(await openClipMenu("b"), "Ask AI about this clip");
    expect(store.getState()).toMatchObject({ pendingAgentRequest: { itemIds: ["a", "b"] }, activeTab: "ai" });
  });

  it("sets a duration through the dialog with inline validation", async () => {
    const { store } = renderWithEditorStore(<TimelinePanel />, { project: projectWith() });
    await select(await openClipMenu("b"), "Set duration…");
    const dialog = within(await screen.findByRole("dialog", { name: "Set duration" }));
    const input = dialog.getByRole("textbox", { name: "Duration" });
    expect(input).toHaveValue("00:00:02");

    fireEvent.change(input, { target: { value: "soon" } });
    fireEvent.click(dialog.getByRole("button", { name: "Set duration" }));
    expect(input).toHaveAttribute("aria-invalid", "true");
    expect(input).toHaveAccessibleDescription("Enter a duration like 2.5 or 00:00:02.500.");

    fireEvent.change(input, { target: { value: "0" } });
    fireEvent.click(dialog.getByRole("button", { name: "Set duration" }));
    expect(await dialog.findByText("Enter a duration longer than zero.")).toBeInTheDocument();

    fireEvent.change(input, { target: { value: "3.5" } });
    await act(async () => {
      fireEvent.click(dialog.getByRole("button", { name: "Set duration" }));
      await Promise.resolve();
    });
    await waitFor(() => expect(screen.queryByRole("dialog")).not.toBeInTheDocument());
    expect(store.getState().project.timeline.tracks[0]?.items.find((item) => item.id === "b")?.durationSeconds).toBe(3.5);
    expect(store.getState().history.past).toHaveLength(1);
  });

  it("enables speed for audio clips and sets it for video clips", async () => {
    const { store } = renderWithEditorStore(<TimelinePanel />, { project: projectWith() });
    const audioMenu = await openClipMenu("m");
    expect(audioMenu.getByRole("menuitem", { name: "Speed…" })).not.toHaveAttribute("aria-disabled", "true");
    fireEvent.keyDown(screen.getByRole("menu"), { key: "Escape" });
    await waitFor(() => expect(screen.queryByRole("menu")).not.toBeInTheDocument());

    await select(await openClipMenu("b"), "Speed…");
    const dialog = within(await screen.findByRole("dialog", { name: "Speed" }));
    const input = dialog.getByRole("textbox", { name: "Playback speed" });
    fireEvent.change(input, { target: { value: "12" } });
    fireEvent.click(dialog.getByRole("button", { name: "Set speed" }));
    expect(await dialog.findByText("Enter a speed between 0.1x and 8x.")).toBeInTheDocument();
    fireEvent.change(input, { target: { value: "2" } });
    await act(async () => {
      fireEvent.click(dialog.getByRole("button", { name: "Set speed" }));
      await Promise.resolve();
    });
    await waitFor(() => expect(screen.queryByRole("dialog")).not.toBeInTheDocument());
    const item = store.getState().project.timeline.tracks[0]?.items.find((entry) => entry.id === "b");
    expect(item).toMatchObject({ durationSeconds: 1, properties: { speed: 2 } });
  });

  it("locks the clip's track", async () => {
    const { store } = renderWithEditorStore(<TimelinePanel />, { project: projectWith() });
    await select(await openClipMenu("a"), "Lock track");
    expect(store.getState().project.timeline.tracks[0]?.locked).toBe(true);
  });

  it("pastes, deletes gaps and adds tracks from the lane menu", async () => {
    const { store } = renderWithEditorStore(<TimelinePanel />, { project: projectWith() });
    const lane = screen.getByRole("listbox", { name: "Video 1" });
    // The lane starts at client x 0 in jsdom, so 240 px is 3 s: inside the gap between "a" and "b".
    fireEvent.contextMenu(lane, { clientX: 240, clientY: 20 });
    let menu = within(await screen.findByRole("menu", { name: "Track actions" }));
    expect(itemNames(menu)).toEqual(["Paste", "Delete gap", "Save range as media", "Add track above", "Add track below"]);
    expect(menu.getByRole("menuitem", { name: /^Paste/ })).toHaveAttribute("aria-disabled", "true");
    await select(menu, "Delete gap");
    expect(store.getState().project.timeline.tracks[0]?.items.find((item) => item.id === "b")?.startSeconds).toBe(2);

    fireEvent.contextMenu(lane, { clientX: 700, clientY: 20 });
    menu = within(await screen.findByRole("menu", { name: "Track actions" }));
    await select(menu, "Add track below");
    await waitFor(() => expect(screen.getAllByRole("listbox").map((entry) => entry.getAttribute("aria-label"))).toEqual(["Video 1", "Video 2", "Audio 1"]));
  });
});
