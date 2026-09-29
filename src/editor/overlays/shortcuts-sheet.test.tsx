import "@testing-library/jest-dom/vitest";
import { fireEvent, render, screen, within } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { ShortcutsSheet } from "./shortcuts-sheet";

function renderSheet(platform: "macos" | "linux") {
  render(<ShortcutsSheet open platform={platform} onOpenChange={vi.fn()} />);
  return screen.getByRole("dialog", { name: "Keyboard shortcuts" });
}

function row(sheet: HTMLElement, label: string) {
  const match = within(sheet).getAllByRole("listitem").find((item) => item.firstElementChild?.textContent === label);
  if (!match) throw new Error(`No shortcut row for ${label}`);
  return match;
}

describe("ShortcutsSheet", () => {
  it("groups the keymap registry and formats bindings for macOS", () => {
    const sheet = renderSheet("macos");
    expect(within(sheet).getAllByRole("heading", { level: 3 }).map((heading) => heading.textContent)).toEqual([
      "Editing",
      "Timeline",
      "Playback",
      "Navigation",
      "Panels",
    ]);
    expect(within(row(sheet, "Undo")).getByText("⌘Z")).toBeInTheDocument();
    expect(within(row(sheet, "Redo")).getByText("⇧⌘Z")).toBeInTheDocument();
    const split = row(sheet, "Split at playhead");
    expect(within(split).getByText("S")).toBeInTheDocument();
    expect(within(split).getByText("⌘K")).toBeInTheDocument();
  });

  it("formats bindings with Ctrl on Linux", () => {
    const sheet = renderSheet("linux");
    expect(within(row(sheet, "Undo")).getByText("Ctrl+Z")).toBeInTheDocument();
    expect(within(row(sheet, "Redo")).getByText("Ctrl+Shift+Z")).toBeInTheDocument();
  });

  it("lists a shortcut shared by timeline and preview once", () => {
    const sheet = renderSheet("macos");
    expect(within(sheet).getAllByText("Play / pause")).toHaveLength(1);
  });

  it("filters by label, group, or binding and says when nothing matches", () => {
    const sheet = renderSheet("linux");
    const search = within(sheet).getByRole("searchbox", { name: "Search shortcuts" });

    fireEvent.change(search, { target: { value: "ripple" } });
    expect(within(sheet).getAllByRole("listitem").map((item) => item.firstElementChild?.textContent)).toEqual(["Ripple delete"]);

    fireEvent.change(search, { target: { value: "ctrl+shift+z" } });
    expect(within(sheet).getAllByRole("listitem").map((item) => item.firstElementChild?.textContent)).toEqual(["Redo"]);

    fireEvent.change(search, { target: { value: "panels" } });
    expect(within(sheet).getAllByRole("heading", { level: 3 }).map((heading) => heading.textContent)).toEqual(["Panels"]);

    fireEvent.change(search, { target: { value: "zzz" } });
    expect(within(sheet).queryAllByRole("listitem")).toHaveLength(0);
    expect(within(sheet).getByText("No shortcuts match “zzz”.")).toBeInTheDocument();
  });
});
