import "@testing-library/jest-dom/vitest";
import { act, fireEvent, screen, waitFor, within } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import type { VideoProject } from "@/lib/project";
import { renderWithEditorStore } from "@/test-utils/editor-render";
import { fixtureProject, fixtureTrack } from "@/test-utils/editor-fixtures";
import { TimelineSelector } from "./timeline-selector";

vi.mock("@/lib/runtime/backend-client", () => ({ backendRequest: vi.fn(), backendListen: vi.fn(), backendMediaUrl: (p: string) => p }));

function renderSelector(project: VideoProject = fixtureProject()) {
  return renderWithEditorStore(<TimelineSelector />, { project });
}

/** jsdom lacks PointerEvent, so Radix dropdowns open from the keyboard. */
async function openMenu(name: string) {
  fireEvent.keyDown(screen.getByRole("button", { name }), { key: "Enter" });
  return within(await screen.findByRole("menu"));
}

async function choose(menuItem: string, trigger: string) {
  const menu = await openMenu(trigger);
  await act(async () => {
    fireEvent.click(menu.queryByRole("menuitem", { name: menuItem }) ?? menu.getByRole("menuitemradio", { name: menuItem }));
    await Promise.resolve();
  });
}

describe("TimelineSelector", () => {
  beforeEach(() => window.localStorage.clear());

  it("creates, duplicates and switches timelines", async () => {
    const { store } = renderSelector();
    await choose("New timeline", "Timeline 1");
    await waitFor(() => expect(screen.getByRole("button", { name: "Timeline 2" })).toBeInTheDocument());
    expect(store.getState().project.activeTimelineId).toBe("timeline-2");

    await choose("Duplicate timeline", "Timeline 2");
    await waitFor(() => expect(screen.getByRole("button", { name: "Copy of Timeline 2" })).toBeInTheDocument());

    await choose("Timeline 1", "Copy of Timeline 2");
    await waitFor(() => expect(store.getState().project.activeTimelineId).toBe("main"));
  });

  it("checks the active timeline and explains why the only timeline cannot be deleted", async () => {
    renderSelector();
    const menu = await openMenu("Timeline 1");
    expect(menu.getByRole("menuitemradio", { name: "Timeline 1" })).toHaveAttribute("aria-checked", "true");
    const deleteItem = menu.getByRole("menuitem", { name: "Delete…" });
    expect(deleteItem).toHaveAttribute("aria-disabled", "true");
    fireEvent.focus(deleteItem);
    expect(await screen.findByRole("tooltip")).toHaveTextContent("A project needs at least one timeline.");
  });

  it("renames through a dialog with inline validation", async () => {
    const { store } = renderSelector();
    await choose("Rename…", "Timeline 1");
    const dialog = within(await screen.findByRole("dialog", { name: "Rename timeline" }));
    const input = dialog.getByRole("textbox", { name: "Name" });
    expect(input).toHaveValue("Timeline 1");

    fireEvent.change(input, { target: { value: "   " } });
    fireEvent.click(dialog.getByRole("button", { name: "Rename" }));
    expect(await dialog.findByText("Enter a timeline name.")).toBeInTheDocument();
    expect(input).toHaveAttribute("aria-invalid", "true");
    expect(input).toHaveAccessibleDescription("Enter a timeline name.");

    fireEvent.change(input, { target: { value: "Rough cut" } });
    expect(input).toHaveAttribute("aria-invalid", "false");
    await act(async () => {
      fireEvent.click(dialog.getByRole("button", { name: "Rename" }));
      await Promise.resolve();
    });
    await waitFor(() => expect(screen.queryByRole("dialog")).not.toBeInTheDocument());
    expect(store.getState().project.timelines?.find((entry) => entry.id === "main")?.name).toBe("Rough cut");
    expect(screen.getByRole("button", { name: "Rough cut" })).toBeInTheDocument();
  });

  it("confirms deletion in a dialog that names the timeline", async () => {
    const { store } = renderSelector();
    await choose("New timeline", "Timeline 1");
    await waitFor(() => expect(screen.getByRole("button", { name: "Timeline 2" })).toBeInTheDocument());

    await choose("Delete…", "Timeline 2");
    const dialog = within(await screen.findByRole("dialog", { name: "Delete Timeline 2?" }));
    expect(dialog.getByText("This cannot be undone.")).toBeInTheDocument();
    fireEvent.click(dialog.getByRole("button", { name: "Cancel" }));
    await waitFor(() => expect(screen.queryByRole("dialog")).not.toBeInTheDocument());
    expect(store.getState().project.timelines).toHaveLength(2);

    await choose("Delete…", "Timeline 2");
    await act(async () => {
      fireEvent.click(within(await screen.findByRole("dialog")).getByRole("button", { name: "Delete timeline" }));
      await Promise.resolve();
    });
    await waitFor(() => expect(store.getState().project.timelines).toHaveLength(1));
    expect(screen.getByRole("button", { name: "Timeline 1" })).toBeInTheDocument();
  });

  it("lists nested sequences and selects one", async () => {
    const project = fixtureProject();
    fixtureTrack(project, "video").items.push({
      id: "nested-1",
      kind: "video_clip",
      startSeconds: 8,
      durationSeconds: 2,
      source: { type: "timeline", timelineId: "intro" },
      label: "Intro sequence",
      properties: {},
    });
    project.timelines = [
      { id: "main", name: "Main", timeline: project.timeline },
      { id: "intro", name: "Intro", timeline: { durationSeconds: 2, tracks: [] } },
    ];
    project.activeTimelineId = "main";
    const { store } = renderSelector(project);
    const menu = await openMenu("Main");
    expect(menu.getByText("Nested sequences")).toBeInTheDocument();
    fireEvent.click(menu.getByRole("menuitem", { name: /Intro sequence/ }));
    expect(store.getState().selectedItemIds).toEqual(["nested-1"]);
  });
});
