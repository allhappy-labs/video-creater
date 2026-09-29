import "@testing-library/jest-dom/vitest";
import { act, fireEvent, render, screen, within } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { TooltipProvider } from "@/components/ui/tooltip";
import { MobileLayout } from "../shell/mobile-layout";
import { createEditorStore } from "../store/editor-store";
import { EditorStoreProvider } from "../store/editor-store-context";
import { crossfade, transitionsOf, transitionTestProject } from "./transition-test-project";

vi.mock("@/lib/runtime/backend-client", () => ({ backendRequest: vi.fn(), backendListen: vi.fn(), backendMediaUrl: (p: string) => p }));
vi.mock("../properties/use-effect-catalog", () => ({ useEffectCatalog: () => ({ status: "ready", effects: [] }) }));

function renderMobile() {
  const store = createEditorStore({ projectDir: "/p", project: transitionTestProject(undefined, [crossfade()]) });
  render(
    <TooltipProvider>
      <EditorStoreProvider store={store}>
        <MobileLayout topBar={<div>top</div>} />
      </EditorStoreProvider>
    </TooltipProvider>,
  );
  return store;
}

function toolNames(): (string | null)[] {
  return within(screen.getByRole("toolbar", { name: "Clip tools" }))
    .getAllByRole("button")
    .map((button) => button.getAttribute("aria-label") ?? button.textContent);
}

describe("mobile transition tools", () => {
  beforeEach(() => window.localStorage.clear());

  it("selects a transition from its badge and shows Type, Duration and Delete", () => {
    const store = renderMobile();
    const badge = screen.getByRole("button", { name: "Crossfade transition, 0.5s" });
    // Touch badges are at least 26 px tall; their edge handles are 24 px wide.
    expect(badge).toHaveStyle({ width: "30px", height: "26px" });
    fireEvent.click(badge);
    expect(store.getState().selectedTransitionId).toBe("fade");
    expect(toolNames()).toEqual(["Back to editor tools", "Type", "Duration", "Delete"]);
    expect(screen.getByText("Tools")).toBeInTheDocument();
    expect(document.querySelector('[data-transition-handle="right"]')).toHaveStyle({ width: "24px" });

    fireEvent.click(screen.getByRole("button", { name: "Back to editor tools" }));
    expect(store.getState().selectedTransitionId).toBeNull();
    expect(screen.getByRole("toolbar", { name: "Editor tools" })).toBeInTheDocument();
  });

  it("opens the Type and Duration sheets and commits from them", async () => {
    const store = renderMobile();
    act(() => store.getState().selectTransition("fade"));
    fireEvent.click(within(screen.getByRole("toolbar", { name: "Clip tools" })).getByRole("button", { name: "Type" }));
    const typeSheet = screen.getByRole("dialog", { name: "Type" });
    expect(within(typeSheet).queryByRole("slider", { name: "Duration" })).not.toBeInTheDocument();
    await act(async () => {
      fireEvent.click(within(typeSheet).getByRole("radio", { name: "Wipe" }));
      await Promise.resolve();
    });
    expect(transitionsOf(store.getState().project)).toMatchObject([{ kind: "wipe" }]);

    fireEvent.click(within(screen.getByRole("toolbar", { name: "Clip tools" })).getByRole("button", { name: "Duration" }));
    const durationSheet = screen.getByRole("dialog", { name: "Duration" });
    expect(within(durationSheet).getByRole("slider", { name: "Duration" })).toBeInTheDocument();
    expect(within(durationSheet).getByText("Max 2.0s")).toBeInTheDocument();
  });

  it("deletes the selected transition from the tools", async () => {
    const store = renderMobile();
    act(() => store.getState().selectTransition("fade"));
    await act(async () => {
      fireEvent.click(within(screen.getByRole("toolbar", { name: "Clip tools" })).getByRole("button", { name: "Delete" }));
      await Promise.resolve();
    });
    expect(transitionsOf(store.getState().project)).toEqual([]);
    expect(screen.getByRole("toolbar", { name: "Editor tools" })).toBeInTheDocument();
  });
});
