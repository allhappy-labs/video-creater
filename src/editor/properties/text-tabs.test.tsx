import "@testing-library/jest-dom/vitest";
import { act, fireEvent, screen, within } from "@testing-library/react";
import { beforeAll, beforeEach, describe, expect, it, vi } from "vitest";
import { kineticLowerThirdTemplate } from "@/lib/motion-templates";
import type { TimelineItem } from "@/lib/timeline";
import { installPointerEventPolyfill } from "@/test-utils/editor-render";
import { enterValue, projectWithTracks, renderProperties, track } from "./properties-test-utils";

vi.mock("@/lib/runtime/backend-client", () => ({ backendRequest: vi.fn(), backendListen: vi.fn(), backendMediaUrl: (p: string) => p }));

function textItem(properties: Record<string, unknown> = {}): TimelineItem {
  return { id: "title", kind: "overlay", startSeconds: 0, durationSeconds: 4, source: { type: "text", text: "Hello" }, label: "Title", properties };
}

async function renderText(tab?: string, properties: Record<string, unknown> = {}) {
  return renderProperties(projectWithTracks([track("overlays", "overlay", [textItem(properties)])]), ["title"], tab);
}

async function typeAndBlur(label: string, value: string) {
  const field = screen.getByRole("textbox", { name: label });
  await act(async () => {
    fireEvent.change(field, { target: { value } });
    fireEvent.blur(field);
  });
}

describe("text property tabs", () => {
  beforeAll(() => {
    installPointerEventPolyfill();
    Element.prototype.hasPointerCapture = () => true;
    Element.prototype.setPointerCapture = () => undefined;
    Element.prototype.releasePointerCapture = () => undefined;
    Element.prototype.scrollIntoView = () => undefined;
  });

  beforeEach(() => window.localStorage.clear());

  it("commits the content on blur through editTextItem", async () => {
    const { applyActions } = await renderText();
    await typeAndBlur("Text", "  Thomas A. Edison.  ");
    expect(applyActions).toHaveBeenCalledTimes(1);
    expect(applyActions).toHaveBeenCalledWith([{ type: "editTextItem", itemId: "title", text: "Thomas A. Edison." }]);
  });

  it("commits multiline content on Cmd+Enter and rejects empty text inline", async () => {
    const { applyActions } = await renderText();
    const field = screen.getByRole("textbox", { name: "Text" });
    await act(async () => {
      fireEvent.change(field, { target: { value: "   " } });
      fireEvent.keyDown(field, { key: "Enter", metaKey: true });
    });
    expect(applyActions).not.toHaveBeenCalled();
    expect(screen.getByRole("alert")).toHaveTextContent("Text can't be empty.");
    expect(field).toHaveAttribute("aria-invalid", "true");

    await act(async () => {
      fireEvent.change(field, { target: { value: "Line one\nLine two" } });
      fireEvent.keyDown(field, { key: "Enter", ctrlKey: true });
    });
    expect(applyActions).toHaveBeenCalledWith([{ type: "editTextItem", itemId: "title", text: "Line one\nLine two" }]);
  });

  it("styles size, color, stroke and background with one property update each", async () => {
    const { applyActions } = await renderText(undefined, { borderColor: "#000000" });
    await enterValue("Size", "64");
    expect(applyActions).toHaveBeenLastCalledWith([
      { type: "updateItemProperties", updates: [{ itemId: "title", set: { fontSize: 64 }, remove: [] }] },
    ]);

    fireEvent.click(within(screen.getByRole("group", { name: "Color" })).getByRole("button", { name: "Yellow" }));
    expect(applyActions).toHaveBeenLastCalledWith([
      { type: "updateItemProperties", updates: [{ itemId: "title", set: { color: "#ffcf5a" }, remove: [] }] },
    ]);

    fireEvent.click(within(screen.getByRole("group", { name: "Stroke" })).getByRole("button", { name: "No stroke" }));
    expect(applyActions).toHaveBeenLastCalledWith([
      { type: "updateItemProperties", updates: [{ itemId: "title", set: {}, remove: ["borderColor"] }] },
    ]);

    fireEvent.click(within(screen.getByRole("group", { name: "Background" })).getByRole("button", { name: "Black" }));
    expect(applyActions).toHaveBeenLastCalledWith([
      { type: "updateItemProperties", updates: [{ itemId: "title", set: { backgroundColor: "#000000" }, remove: [] }] },
    ]);
    expect(applyActions).toHaveBeenCalledTimes(4);
  });

  it("applies a text treatment's guidance from the style preset grid", async () => {
    const { applyActions } = await renderText("Style");
    const grid = screen.getByRole("group", { name: "Text style preset" });
    expect(within(grid).queryByRole("button", { name: "Punchy Caption" })).not.toBeInTheDocument();
    fireEvent.click(within(grid).getByRole("button", { name: kineticLowerThirdTemplate.name }));
    expect(applyActions).toHaveBeenCalledWith([
      {
        type: "updateTextOverlayItems",
        updates: [
          {
            itemId: "title",
            startSeconds: 0,
            durationSeconds: 4,
            text: "Hello",
            visualTreatment: kineticLowerThirdTemplate.visualTreatment,
            motion: kineticLowerThirdTemplate.motion,
            safeZone: kineticLowerThirdTemplate.safeZone,
            avoid: kineticLowerThirdTemplate.avoid,
          },
        ],
      },
    ]);
  });

  it("sets alignment from the position tab", async () => {
    const { applyActions } = await renderText("Position");
    const group = screen.getByRole("radiogroup", { name: "Alignment" });
    expect(within(group).getByRole("radio", { name: "Align center" })).toHaveAttribute("aria-checked", "true");
    fireEvent.click(within(group).getByRole("radio", { name: "Align left" }));
    expect(applyActions).toHaveBeenCalledWith([
      { type: "updateItemProperties", updates: [{ itemId: "title", set: { alignment: "left" }, remove: [] }] },
    ]);
  });

  it("applies in and out animation presets as keyframe lanes", async () => {
    const { applyActions } = await renderText("Animation");
    expect(screen.queryByRole("group", { name: "Loop animation" })).not.toBeInTheDocument();
    fireEvent.click(within(screen.getByRole("group", { name: "In animation" })).getByRole("button", { name: "Slide Fade Up" }));
    expect(applyActions).toHaveBeenCalledTimes(1);
    expect(applyActions.mock.calls[0]?.[0]).toContainEqual({
      type: "updateItemProperties",
      updates: [{ itemId: "title", set: { animationInPresetId: "slide-fade-up-v1" }, remove: [] }],
    });
    expect(screen.getByRole("group", { name: "Out animation" })).toBeInTheDocument();
  });
});
