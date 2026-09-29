import "@testing-library/jest-dom/vitest";
import { act, screen } from "@testing-library/react";
import { beforeAll, beforeEach, describe, expect, it, vi } from "vitest";
import type { VideoProject } from "@/lib/project";
import { installPointerEventPolyfill, renderWithEditorStore } from "@/test-utils/editor-render";
import { trimHandleHitBox, trimHandleHitWidth } from "./clip-trim-handle";
import { keyframeDiamondHitSize } from "./keyframe-lane";
import { TimelinePanel } from "./timeline-panel";
import { transitionHandleHitWidth } from "./transition-badge";
import { crossfade, sourceClip, transitionTestProject } from "./transition-test-project";

vi.mock("@/lib/runtime/backend-client", () => ({ backendRequest: vi.fn(), backendListen: vi.fn(), backendMediaUrl: (p: string) => p }));

/** The minimum touch target on phones and tablets. */
const minimumTouchPixels = 24;

/** "a" 0–2 s with opacity keyframes and "b" 2–4 s on Video 1, joined by a crossfade. */
function touchProject(): VideoProject {
  return transitionTestProject(
    [
      { ...sourceClip("a", 0, 2, 0), properties: { sourceIn: 0, sourceOut: 2, keyframes: { opacity: [{ atSeconds: 0.5, value: 1 }, { atSeconds: 1.5, value: 0.4 }] } } },
      sourceClip("b", 2, 2, 2),
    ],
    [crossfade()],
  );
}

function pixels(value: string): number {
  return Number.parseFloat(value);
}

function renderMobileTimeline() {
  return renderWithEditorStore(<TimelinePanel mobile />, { project: touchProject() });
}

describe("mobile timeline touch targets", () => {
  beforeAll(() => installPointerEventPolyfill());
  beforeEach(() => window.localStorage.clear());

  it("gives clip trim handles a 24 px hit box that reaches past the clip edge", () => {
    expect(trimHandleHitWidth.touch).toBeGreaterThanOrEqual(minimumTouchPixels);
    expect(trimHandleHitBox("left", 100, true)).toEqual({ left: 92, width: 24 });
    expect(trimHandleHitBox("right", 100, true)).toEqual({ left: 84, width: 24 });
    expect(trimHandleHitBox("right", 100, false)).toEqual({ left: 93, width: 7 });

    const { store } = renderMobileTimeline();
    act(() => store.getState().selectItems(["b"]));
    const handles = screen.getAllByRole("slider", { name: /Resize b/ });
    expect(handles).toHaveLength(2);
    for (const handle of handles) {
      expect(pixels(handle.style.width)).toBeGreaterThanOrEqual(minimumTouchPixels);
      expect(pixels(handle.style.height)).toBeGreaterThanOrEqual(minimumTouchPixels);
    }
  });

  it("wraps keyframe diamonds in an invisible 24 px hit box", () => {
    expect(keyframeDiamondHitSize).toBeGreaterThanOrEqual(minimumTouchPixels);
    const { store } = renderMobileTimeline();
    act(() => {
      store.getState().setKeyframesVisible(true);
      store.getState().selectItems(["a"]);
    });
    const diamonds = screen.getAllByRole("button", { name: /Opacity keyframe at/ });
    expect(diamonds).toHaveLength(2);
    for (const diamond of diamonds) {
      expect(diamond).toHaveStyle({ width: "24px", height: "24px" });
      // The hit box is centred on the point and has no fill of its own.
      expect(diamond).toHaveClass("-translate-x-1/2", "-translate-y-1/2");
      expect(diamond.className).not.toMatch(/\bbg-/);
      expect(diamond.querySelector('[data-testid="keyframe-diamond"]')).toHaveClass("h-2.5", "w-2.5", "bg-keyframe");
    }
  });

  it("gives a selected transition's edge handles 24 px hit boxes", () => {
    expect(transitionHandleHitWidth.touch).toBeGreaterThanOrEqual(minimumTouchPixels);
    const { store } = renderMobileTimeline();
    act(() => store.getState().selectTransition("fade"));
    const badge = screen.getByRole("button", { name: "Crossfade transition, 0.5s" });
    expect(pixels(badge.style.width)).toBeGreaterThanOrEqual(minimumTouchPixels);
    expect(pixels(badge.style.height)).toBeGreaterThanOrEqual(minimumTouchPixels);
    const edges = document.querySelectorAll<HTMLElement>("[data-transition-handle]");
    expect(edges).toHaveLength(2);
    for (const edge of edges) {
      expect(pixels(edge.style.width)).toBeGreaterThanOrEqual(minimumTouchPixels);
      expect(pixels(edge.style.height)).toBeGreaterThanOrEqual(minimumTouchPixels);
    }
  });
});
