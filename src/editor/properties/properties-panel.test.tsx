import "@testing-library/jest-dom/vitest";
import { act, fireEvent, screen, within } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import type { VideoProject } from "@/lib/project";
import type { TimelineItem, TimelineItemKind, TimelineTrack, TrackKind } from "@/lib/timeline";
import { renderWithEditorStore } from "@/test-utils/editor-render";
import { fixtureProject } from "@/test-utils/editor-fixtures";
import { requiredValue } from "@/test-utils/required";
import { PropertiesPanel } from "./properties-panel";
import { propertyTabsForKind } from "./property-tabs";

vi.mock("@/lib/runtime/backend-client", () => ({ backendRequest: vi.fn(), backendListen: vi.fn(), backendMediaUrl: (p: string) => p }));
// Routing tests don't exercise the Effects section's asynchronous catalog load.
vi.mock("./use-effect-catalog", () => ({ useEffectCatalog: () => ({ status: "ready", effects: [] }) }));

function item(
  id: string,
  kind: TimelineItemKind,
  options: { label?: string; properties?: Record<string, unknown>; source?: TimelineItem["source"] } = {},
): TimelineItem {
  return {
    id,
    kind,
    startSeconds: 0,
    durationSeconds: 2,
    source: options.source ?? { type: "media", mediaId: "media-1" },
    label: options.label ?? id,
    properties: options.properties ?? {},
  };
}

function track(id: string, kind: TrackKind, items: TimelineItem[]): TimelineTrack {
  return { id, name: id, kind, locked: false, enabled: true, items };
}

function project(): VideoProject {
  const tracks = [
    track("video", "video", [
      item("video", "video_clip", { label: "Archive broll" }),
      item("unlabeled", "video_clip", { label: "  " }),
      item("image", "image_clip", { label: "Portrait" }),
    ]),
    track("overlays", "overlay", [
      item("text", "overlay", { label: "Title", source: { type: "text", text: "Hello" } }),
      item("template", "overlay", { label: "Lower third", properties: { templateId: "kinetic-lower-third-v1" }, source: { type: "text", text: "" } }),
    ]),
    track("captions", "caption", [item("caption", "caption", { label: "The first", source: { type: "text", text: "The first" } })]),
    track("audio", "audio", [item("audio", "audio_clip", { label: "Ambience", source: { type: "media", mediaId: "media-voiceover" } })]),
  ];
  return { ...fixtureProject(), timeline: { durationSeconds: 2, tracks } };
}

function renderPanel(selected: readonly string[] = []) {
  const rendered = renderWithEditorStore(<PropertiesPanel />, { project: project() });
  act(() => rendered.store.getState().selectItems(selected));
  return rendered;
}

function tabNames(): string[] {
  return within(screen.getByRole("tablist", { name: "Property tabs" }))
    .getAllByRole("tab")
    .map((tab) => tab.textContent ?? "");
}

describe("PropertiesPanel", () => {
  beforeEach(() => window.localStorage.clear());

  it("renders nothing without a selection", () => {
    const { container } = renderPanel();
    expect(container).toBeEmptyDOMElement();
  });

  it.each([
    ["video", "Archive broll", ["Video", "Audio", "Speed", "Animation", "AI"]],
    ["image", "Portrait", ["Video", "Speed", "Animation", "AI"]],
    ["audio", "Ambience", ["Basic", "Voice", "Speed"]],
    ["text", "Title", ["Text", "Style", "Position", "Animation"]],
    ["caption", "The first", ["Text", "Style", "Position", "Animation"]],
    ["template", "Lower third", ["Content", "Style", "Animation", "Effects"]],
  ])("routes a %s selection to its tabs", (itemId, name, tabs) => {
    renderPanel([itemId]);
    expect(screen.getByRole("heading", { level: 2, name })).toBeInTheDocument();
    expect(tabNames()).toEqual(tabs);
    expect(screen.getByRole("tab", { name: requiredValue(tabs[0], "first tab") })).toHaveAttribute("aria-selected", "true");
  });

  it("names a multiple selection by its count and shows the common tab", () => {
    renderPanel(["video", "image", "missing"]);
    expect(screen.getByRole("heading", { level: 2, name: "2 items" })).toBeInTheDocument();
    expect(tabNames()).toEqual(["Common"]);
  });

  it("falls back to the media display name for an unlabeled clip", () => {
    renderPanel(["unlabeled"]);
    expect(screen.getByRole("heading", { level: 2, name: "input.mp4" })).toBeInTheDocument();
  });

  it("gives transitions a single Transition tab", () => {
    expect(propertyTabsForKind("transition", false)).toEqual([{ id: "transition", label: "Transition" }]);
    expect(propertyTabsForKind("none", false)).toEqual([]);
  });

  it("remembers the active tab per selection kind and scrolls the body", () => {
    const { store } = renderPanel(["video"]);
    fireEvent.mouseDown(screen.getByRole("tab", { name: "Speed" }), { button: 0 });
    expect(screen.getByRole("tab", { name: "Speed" })).toHaveAttribute("aria-selected", "true");
    expect(store.getState().propertiesTabByKind).toEqual({ visual: "speed" });
    expect(screen.getByRole("tabpanel", { name: "Speed" })).toHaveClass("overflow-y-auto");

    act(() => store.getState().selectItems(["text"]));
    expect(screen.getByRole("tab", { name: "Text" })).toHaveAttribute("aria-selected", "true");

    act(() => store.getState().selectItems(["image"]));
    expect(screen.getByRole("tab", { name: "Speed" })).toHaveAttribute("aria-selected", "true");
  });

  it("falls back to the first tab when the remembered tab is not available", () => {
    const { store } = renderPanel(["video"]);
    fireEvent.mouseDown(screen.getByRole("tab", { name: "Audio" }), { button: 0 });
    act(() => store.getState().selectItems(["image"]));
    expect(screen.getByRole("tab", { name: "Video" })).toHaveAttribute("aria-selected", "true");
    expect(store.getState().propertiesTabByKind).toEqual({ visual: "audio" });
  });
});
