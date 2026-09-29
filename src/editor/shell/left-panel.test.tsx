import "@testing-library/jest-dom/vitest";
import { fireEvent, render, screen } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { TooltipProvider } from "@/components/ui/tooltip";
import { fixtureProject } from "@/test-utils/editor-fixtures";
import { createEditorStore } from "../store/editor-store";
import { EditorStoreProvider } from "../store/editor-store-context";
import { LeftPanel } from "./left-panel";

vi.mock("@/lib/runtime/backend-client", () => ({ backendRequest: vi.fn(), backendListen: vi.fn(), backendMediaUrl: (p: string) => p }));

describe("LeftPanel", () => {
  beforeEach(() => window.localStorage.clear());

  it("renders six tabs in order with AI active by default and switches tabs", () => {
    const store = createEditorStore({ projectDir: "/p", project: fixtureProject() });
    render(
      <TooltipProvider>
        <EditorStoreProvider store={store}>
          <LeftPanel />
        </EditorStoreProvider>
      </TooltipProvider>,
    );
    const tabs = screen.getAllByRole("tab");
    expect(tabs.map((tab) => tab.textContent)).toEqual(["AI", "Media", "Audio", "Text", "Captions", "Effects"]);
    expect(screen.getByRole("tab", { name: "AI" })).toHaveAttribute("data-state", "active");
    fireEvent.mouseDown(screen.getByRole("tab", { name: "Captions" }));
    expect(store.getState().activeTab).toBe("captions");
    expect(screen.getByRole("tabpanel", { name: "Captions" })).toBeInTheDocument();
  });
});
