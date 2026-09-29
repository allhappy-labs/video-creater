import "@testing-library/jest-dom/vitest";
import { act, render, screen } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { TooltipProvider } from "@/components/ui/tooltip";
import { fixtureItem, fixtureProject } from "@/test-utils/editor-fixtures";
import { createEditorStore } from "../store/editor-store";
import { EditorStoreProvider } from "../store/editor-store-context";
import { DesktopLayout } from "./desktop-layout";

vi.mock("@/lib/runtime/backend-client", () => ({ backendRequest: vi.fn(), backendListen: vi.fn(), backendMediaUrl: (p: string) => p }));

function renderLayout(mode: "desktop-docked" | "desktop-overlay") {
  const project = fixtureProject();
  const store = createEditorStore({ projectDir: "/p", project });
  render(
    <TooltipProvider>
      <EditorStoreProvider store={store}>
        <DesktopLayout mode={mode} topBar={<div>top</div>} />
      </EditorStoreProvider>
    </TooltipProvider>,
  );
  return { store, project };
}

describe("DesktopLayout", () => {
  beforeEach(() => window.localStorage.clear());

  it("hides properties without a selection and docks them with one", () => {
    const { store, project } = renderLayout("desktop-docked");
    expect(screen.queryByRole("complementary", { name: "Properties" })).not.toBeInTheDocument();
    act(() => store.getState().selectItems([fixtureItem(project, "video").id]));
    const properties = screen.getByRole("complementary", { name: "Properties" });
    expect(properties).not.toHaveClass("absolute");
    expect(screen.getByRole("region", { name: "Preview panel" }).style.paddingRight).toBe("");
  });

  it("overlays properties in the narrow desktop mode", () => {
    const { store, project } = renderLayout("desktop-overlay");
    act(() => store.getState().selectItems([fixtureItem(project, "video").id]));
    expect(screen.getByRole("complementary", { name: "Properties" })).toHaveClass("absolute");
    // The preview content keeps clear of the overlay, so the canvas and transport stay usable.
    const preview = screen.getByRole("region", { name: "Preview panel" });
    expect(preview.style.paddingRight).toBe("330px");
    // Canvas handles stack inside the panel, below the overlay.
    expect(preview).toHaveClass("isolate");
    act(() => store.getState().clearSelection());
    expect(preview.style.paddingRight).toBe("");
  });

  it("renders all fixed regions and both resizers", () => {
    renderLayout("desktop-docked");
    expect(screen.getByRole("tablist", { name: "Editor tools" })).toBeInTheDocument();
    expect(screen.getByRole("region", { name: "Preview viewport" })).toBeInTheDocument();
    expect(screen.getByRole("region", { name: "Timeline canvas" })).toBeInTheDocument();
    expect(screen.getByRole("separator", { name: "Resize left panel" })).toBeInTheDocument();
    expect(screen.getByRole("separator", { name: "Resize timeline" })).toBeInTheDocument();
  });
});
