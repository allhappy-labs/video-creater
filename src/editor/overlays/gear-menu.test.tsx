import "@testing-library/jest-dom/vitest";
import { act, fireEvent, render, screen, within } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { TooltipProvider } from "@/components/ui/tooltip";
import { fixtureProject } from "@/test-utils/editor-fixtures";
import { createEditorStore } from "../store/editor-store";
import { EditorStoreProvider } from "../store/editor-store-context";
import { GearMenu } from "./gear-menu";

vi.mock("@/lib/runtime/backend-client", () => ({ backendRequest: vi.fn(), backendListen: vi.fn(), backendMediaUrl: (p: string) => p }));

function renderGearMenu() {
  const store = createEditorStore({ projectDir: "/projects/edison", project: fixtureProject() });
  const props = { onOpenSettings: vi.fn(), onOpenProjectSettings: vi.fn() };
  render(
    <TooltipProvider>
      <EditorStoreProvider store={store}>
        <GearMenu {...props} />
      </EditorStoreProvider>
    </TooltipProvider>,
  );
  return { store, props };
}

async function openMenu() {
  const menuButton = screen.getByRole("button", { name: "Editor menu" });
  // jsdom lacks PointerEvent, so pointerDown loses `button`; open the Radix menu via keyboard instead.
  fireEvent.keyDown(menuButton, { key: "Enter" });
  return { menuButton, menu: await screen.findByRole("menu") };
}

describe("GearMenu", () => {
  it("lists the settings and help items in order with the shortcuts hint", async () => {
    renderGearMenu();
    const { menu } = await openMenu();
    expect(within(menu).getAllByRole("menuitem").map((item) => item.textContent)).toEqual([
      "Project settings",
      "App settings",
      "Keyboard shortcuts⌘/",
      "Connect external agents…",
      "Project skills…",
    ]);
    expect(within(menu).getByRole("separator")).toBeInTheDocument();
    expect(within(menu).getByRole("menuitem", { name: "Keyboard shortcuts" })).toHaveAttribute("aria-keyshortcuts", "Meta+/");
  });

  it("opens project and app settings with the menu button as origin", async () => {
    const { props } = renderGearMenu();
    const { menuButton } = await openMenu();
    fireEvent.click(screen.getByRole("menuitem", { name: "Project settings" }));
    expect(props.onOpenProjectSettings).toHaveBeenCalledWith(menuButton);

    await openMenu();
    fireEvent.click(screen.getByRole("menuitem", { name: "App settings" }));
    expect(props.onOpenSettings).toHaveBeenCalledWith(menuButton);
  });

  it.each([
    ["Keyboard shortcuts", "shortcuts", "Keyboard shortcuts"],
    ["Connect external agents…", "connectAgents", "Connect external agents"],
    ["Project skills…", "projectSkills", "Project skills"],
  ] as const)("%s opens the %s overlay", async (item, overlay, dialogName) => {
    const { store } = renderGearMenu();
    await openMenu();
    fireEvent.click(screen.getByRole("menuitem", { name: item }));
    expect(store.getState().overlay).toBe(overlay);
    expect(await screen.findByRole("dialog", { name: dialogName })).toBeInTheDocument();

    fireEvent.click(screen.getByRole("button", { name: `Close ${dialogName}` }));
    expect(store.getState().overlay).toBeNull();
  });

  it("opens overlays requested through the store, as the native menu does", async () => {
    const { store } = renderGearMenu();
    act(() => store.getState().openOverlay("connectAgents"));
    expect(await screen.findByRole("dialog", { name: "Connect external agents" })).toBeInTheDocument();
    expect(screen.getByText("/projects/edison")).toBeInTheDocument();
  });

  it("routes Project skills to Project settings, closing the dialog first", async () => {
    const { store, props } = renderGearMenu();
    act(() => store.getState().openOverlay("projectSkills"));
    fireEvent.click(await screen.findByRole("button", { name: "Verify in Project settings" }));
    expect(store.getState().overlay).toBeNull();
    expect(props.onOpenProjectSettings).toHaveBeenCalledWith(screen.getByRole("button", { name: "Editor menu" }));
  });
});
