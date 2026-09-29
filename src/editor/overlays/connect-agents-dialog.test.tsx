import "@testing-library/jest-dom/vitest";
import { act, fireEvent, render, screen, within } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { agentSetupSnippet } from "@/lib/agent/selection-context";
import { ConnectAgentsDialog } from "./connect-agents-dialog";

function renderDialog(projectDir = "/projects/edison") {
  render(<ConnectAgentsDialog open projectDir={projectDir} platform="macos" onOpenChange={vi.fn()} />);
  return screen.getByRole("dialog", { name: "Connect external agents" });
}

function stubClipboard(writeText: ((text: string) => Promise<void>) | undefined) {
  Object.defineProperty(navigator, "clipboard", { configurable: true, value: writeText ? { writeText } : undefined });
}

// jsdom's pointerDown lacks `button`, so Radix tabs are switched with the keyboard.
function selectTab(dialog: HTMLElement, name: string) {
  const tab = within(dialog).getByRole("tab", { name });
  fireEvent.mouseDown(tab);
  fireEvent.focus(tab);
  fireEvent.keyDown(tab, { key: "Enter" });
  return tab;
}

describe("ConnectAgentsDialog", () => {
  afterEach(() => {
    stubClipboard(undefined);
    window.getSelection()?.removeAllRanges();
  });

  it("shows a tab per agent client and the project folder", () => {
    const dialog = renderDialog();
    expect(within(dialog).getAllByRole("tab").map((tab) => tab.textContent)).toEqual(["Codex", "Claude Code", "Claude Desktop", "Cursor"]);
    expect(within(dialog).getByText("/projects/edison")).toBeInTheDocument();
    expect(within(dialog).getByLabelText("Codex setup")).toHaveTextContent("Codex command: codex app-server --stdio");
  });

  it("copies the selected client's snippet", async () => {
    const writeText = vi.fn(() => Promise.resolve());
    stubClipboard(writeText);
    const dialog = renderDialog();

    selectTab(dialog, "Cursor");
    await act(async () => {
      fireEvent.click(within(dialog).getByRole("button", { name: "Copy" }));
    });

    expect(writeText).toHaveBeenCalledWith(agentSetupSnippet("cursor", "/projects/edison"));
    expect(within(dialog).getByRole("status")).toHaveTextContent("Copied Cursor setup");
  });

  it("selects the snippet for manual copy when the Clipboard API is unavailable", async () => {
    stubClipboard(undefined);
    const dialog = renderDialog();

    await act(async () => {
      fireEvent.click(within(dialog).getByRole("button", { name: "Copy" }));
    });

    expect(window.getSelection()?.toString()).toBe(agentSetupSnippet("codex", "/projects/edison"));
    expect(within(dialog).getByRole("status")).toHaveTextContent("Press ⌘C to copy the selected setup");
  });

  it("falls back to selection when the clipboard write is rejected", async () => {
    stubClipboard(() => Promise.reject(new Error("denied")));
    const dialog = renderDialog();

    await act(async () => {
      fireEvent.click(within(dialog).getByRole("button", { name: "Copy" }));
    });

    expect(window.getSelection()?.toString()).toBe(agentSetupSnippet("codex", "/projects/edison"));
  });
});
