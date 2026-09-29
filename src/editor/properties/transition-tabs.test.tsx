import "@testing-library/jest-dom/vitest";
import { act, fireEvent, screen } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import type { ProjectAction, VideoProject } from "@/lib/project";
import { renderWithEditorStore } from "@/test-utils/editor-render";
import { crossfade, transitionsOf, transitionTestProject } from "../timeline/transition-test-project";
import { enterValue } from "./properties-test-utils";
import { PropertiesPanel } from "./properties-panel";

vi.mock("@/lib/runtime/backend-client", () => ({ backendRequest: vi.fn(), backendListen: vi.fn(), backendMediaUrl: (p: string) => p }));
vi.mock("./use-effect-catalog", () => ({ useEffectCatalog: () => ({ status: "ready", effects: [] }) }));

function renderTransition(project: VideoProject = transitionTestProject(undefined, [crossfade()])) {
  const rendered = renderWithEditorStore(<PropertiesPanel />, { project });
  const applyActions = vi.fn(rendered.store.getState().applyActions);
  act(() => {
    rendered.store.setState({ applyActions: applyActions as (actions: readonly ProjectAction[]) => Promise<VideoProject | null> });
    rendered.store.getState().selectTransition("fade");
  });
  return { ...rendered, applyActions };
}

describe("Properties Transition tab", () => {
  beforeEach(() => window.localStorage.clear());

  it("shows the type, the pair and the duration with its maximum", () => {
    renderTransition();
    expect(screen.getByRole("heading", { level: 2, name: "Crossfade transition" })).toBeInTheDocument();
    expect(screen.getByRole("radio", { name: "Crossfade" })).toHaveAttribute("aria-checked", "true");
    expect(screen.getByText("Between a and b")).toBeInTheDocument();
    expect(screen.getByRole("slider", { name: "Duration" })).toHaveAttribute("aria-valuemax", "2");
    expect(screen.getByRole("textbox", { name: "Duration" })).toHaveValue("0.5s");
    expect(screen.getByText("Max 2.0s")).toBeInTheDocument();
  });

  it("commits one updateTransition per type change and duration entry", async () => {
    const { store, applyActions } = renderTransition();
    await act(async () => {
      fireEvent.click(screen.getByRole("radio", { name: "Dip to white" }));
      await Promise.resolve();
    });
    expect(applyActions).toHaveBeenLastCalledWith([{ type: "updateTransition", trackId: "v1", transitionId: "fade", kind: "dipToWhite" }]);
    expect(screen.getByRole("heading", { level: 2, name: "Dip to white transition" })).toBeInTheDocument();

    await enterValue("Duration", "1");
    expect(applyActions).toHaveBeenLastCalledWith([{ type: "updateTransition", trackId: "v1", transitionId: "fade", durationSeconds: 1 }]);
    expect(applyActions).toHaveBeenCalledTimes(2);
    expect(transitionsOf(store.getState().project)).toMatchObject([{ kind: "dipToWhite", durationSeconds: 1 }]);
    expect(store.getState().history.past).toHaveLength(2);

    await enterValue("Duration", "3");
    expect(screen.getByText("Enter a value from 0.05s to 2.0s.")).toBeInTheDocument();
    expect(applyActions).toHaveBeenCalledTimes(2);
  });

  it("deletes the transition and closes the tab", async () => {
    const { store } = renderTransition();
    await act(async () => {
      fireEvent.click(screen.getByRole("button", { name: "Delete transition" }));
      await Promise.resolve();
    });
    expect(transitionsOf(store.getState().project)).toEqual([]);
    expect(store.getState().selectedTransitionId).toBeNull();
    expect(screen.queryByRole("tab", { name: "Transition" })).not.toBeInTheDocument();
  });
});
