import "@testing-library/jest-dom/vitest";
import { act, fireEvent, screen, within } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { renderWithEditorStore } from "@/test-utils/editor-render";
import { crossfade, sourceClip, transitionsOf, transitionTestProject } from "../../timeline/transition-test-project";
import { EffectsPanel } from "./effects-panel";

vi.mock("@/lib/runtime/backend-client", () => ({ backendRequest: vi.fn(), backendListen: vi.fn(), backendMediaUrl: (p: string) => p }));
vi.mock("../../properties/use-effect-catalog", () => ({ useEffectCatalog: () => ({ status: "ready", effects: [] }) }));

async function openTransitions(project = transitionTestProject()) {
  const rendered = renderWithEditorStore(<EffectsPanel />, { project });
  fireEvent.click(screen.getByRole("button", { name: "Transitions" }));
  await act(async () => undefined);
  return rendered;
}

async function clickAdd(name: string) {
  await act(async () => {
    fireEvent.click(screen.getByRole("button", { name: `Add ${name} transition` }));
    await Promise.resolve();
  });
}

describe("Effects tab transitions", () => {
  beforeEach(() => window.localStorage.clear());

  it("lists the four transitions with animated previews", async () => {
    await openTransitions();
    const tiles = within(screen.getByRole("list", { name: "Transitions" })).getAllByRole("listitem");
    expect(tiles.map((tile) => tile.querySelector("p")?.textContent)).toEqual(["Crossfade", "Dip to black", "Dip to white", "Wipe"]);
    const previews = screen.getAllByTestId("transition-preview");
    expect(previews).toHaveLength(4);
    // Motion only under motion-safe; the static layer shows the midpoint.
    expect(previews[0]?.innerHTML).toContain("motion-safe:animate-transition-reveal");
    expect(tiles[0]).toHaveAttribute("draggable", "true");
  });

  it("disables + with a reason when no clips touch", async () => {
    const { store } = await openTransitions(transitionTestProject([sourceClip("a", 0, 1, 0), sourceClip("b", 2, 1, 2)]));
    const add = screen.getByRole("button", { name: "Add Crossfade transition" });
    expect(add).toHaveAttribute("aria-disabled", "true");
    fireEvent.focus(add);
    expect(await screen.findByRole("tooltip")).toHaveTextContent("Place two clips next to each other first");
    await clickAdd("Crossfade");
    expect(store.getState().history.past).toHaveLength(0);
  });

  it("+ adds one half-second transition on the cut nearest the playhead and selects it", async () => {
    const { store } = await openTransitions();
    act(() => store.getState().seek(1.4));
    expect(screen.getByText(/\+ adds at the cut at 00:00:02 on Video 1/)).toBeInTheDocument();
    await clickAdd("Dip to black");
    expect(transitionsOf(store.getState().project)).toEqual([
      { id: "transition-a-b", leftItemId: "a", rightItemId: "b", kind: "dipToBlack", durationSeconds: 0.5 },
    ]);
    expect(store.getState().history.past).toHaveLength(1);
    expect(store.getState().selectedTransitionId).toBe("transition-a-b");
    expect(store.getState().toasts).toEqual([]);

    // With the transition selected, + changes its type on the same cut.
    await clickAdd("Wipe");
    expect(transitionsOf(store.getState().project)).toMatchObject([{ id: "transition-a-b", kind: "wipe" }]);
    expect(store.getState().history.past).toHaveLength(2);
  });

  it("shortens to the maximum with a toast when handles run out", async () => {
    // "a" uses 1.8–3.8 s of the 4 s source: 0.2 s of tail handle allows 0.4 s.
    const { store } = await openTransitions(transitionTestProject([sourceClip("a", 0, 2, 1.8), sourceClip("b", 2, 2, 2)]));
    await clickAdd("Crossfade");
    expect(transitionsOf(store.getState().project)).toMatchObject([{ durationSeconds: 0.4 }]);
    expect(store.getState().toasts.map((toast) => toast.title)).toEqual(["Shortened to 0.4s — not enough unused media"]);
  });

  it("refuses a cut without a frame of unused media with the validation message", async () => {
    const { store } = await openTransitions(transitionTestProject([sourceClip("a", 0, 2, 2), sourceClip("b", 2, 2, 2)]));
    await clickAdd("Crossfade");
    expect(transitionsOf(store.getState().project)).toEqual([]);
    expect(store.getState().lastError).toBe("Not enough unused media after a for a 0.5s transition. Maximum is 0.0s.");
    expect(store.getState().history.past).toHaveLength(0);
  });

  it("describes changing an existing transition", async () => {
    const { store } = await openTransitions(transitionTestProject(undefined, [crossfade()]));
    act(() => store.getState().selectTransition("fade"));
    expect(screen.getByText(/\+ changes the transition at 00:00:02/)).toBeInTheDocument();
  });
});
