import "@testing-library/jest-dom/vitest";
import { act, screen } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { renderWithEditorStore } from "@/test-utils/editor-render";
import { fixtureProject } from "@/test-utils/editor-fixtures";
import { Playhead } from "./playhead";

vi.mock("@/lib/runtime/backend-client", () => ({ backendRequest: vi.fn(), backendListen: vi.fn(), backendMediaUrl: (p: string) => p }));

describe("Playhead", () => {
  beforeEach(() => window.localStorage.clear());

  it("sits at playheadSeconds * pps - scrollLeft past the header column", () => {
    const { store } = renderWithEditorStore(
      <Playhead pixelsPerSecond={80} scrollLeft={40} offsetLeft={118} viewportWidth={720} />,
      { project: fixtureProject() },
    );
    act(() => store.getState().seek(2));
    expect(screen.getByTestId("playhead")).toHaveStyle({ left: `${118 + 2 * 80 - 40}px` });
    act(() => store.getState().seek(3.5));
    expect(screen.getByTestId("playhead")).toHaveStyle({ left: `${118 + 3.5 * 80 - 40}px` });
  });

  it("stays at the viewport centre when centered, whatever the time", () => {
    const { store } = renderWithEditorStore(
      <Playhead pixelsPerSecond={80} scrollLeft={400} offsetLeft={0} viewportWidth={402} centered />,
      { project: fixtureProject() },
    );
    act(() => store.getState().seek(1));
    expect(screen.getByTestId("playhead")).toHaveStyle({ left: "201px" });
    expect(screen.getByTestId("playhead")).toHaveAttribute("data-seconds", "1.000");
  });

  it("hides while scrolled out of the lanes viewport", () => {
    const { store } = renderWithEditorStore(
      <Playhead pixelsPerSecond={80} scrollLeft={400} offsetLeft={118} viewportWidth={720} />,
      { project: fixtureProject() },
    );
    act(() => store.getState().seek(1));
    expect(screen.queryByTestId("playhead")).not.toBeInTheDocument();
  });
});
