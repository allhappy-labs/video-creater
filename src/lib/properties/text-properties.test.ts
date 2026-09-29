import { describe, expect, it } from "vitest";
import {
  editTextAction,
  textContent,
  textOverlayMetadata,
  textOverlayUpdateAction,
  textStyle,
  textStyleAction,
} from "@/lib/properties/text-properties";
import type { TimelineItem } from "@/lib/timeline";
import type { CommandResult } from "@/lib/timeline-ops/clip-commands";

function overlay(properties: Record<string, unknown> = {}, text = "Hello"): TimelineItem {
  return {
    id: "title",
    kind: "overlay",
    startSeconds: 1,
    durationSeconds: 3,
    source: { type: "text", text },
    label: text,
    properties,
  };
}

function actionsOf(result: CommandResult) {
  if ("blocked" in result) throw new Error(`Unexpectedly blocked: ${result.blocked}`);
  return result.actions;
}

function blockedOf(result: CommandResult) {
  if (!("blocked" in result)) throw new Error("Expected the edit to be blocked");
  return result.blocked;
}

describe("text readers", () => {
  it("return defaults for missing or invalid style values", () => {
    expect(textStyle(overlay({ fontName: " ", fontSize: -4, color: 3, alignment: "justify", borderColor: "" }))).toEqual({
      fontName: "Helvetica",
      fontSize: 48,
      color: "#ffffff",
      alignment: "center",
      strokeColor: null,
      backgroundColor: null,
    });
  });

  it("read stored style values and content", () => {
    const item = overlay({
      fontName: "Inter-Bold",
      fontSize: 64,
      color: "#ffcf5a",
      alignment: "left",
      borderColor: "#000000",
      backgroundColor: "rgba(0,0,0,0.5)",
    });
    expect(textStyle(item)).toEqual({
      fontName: "Inter-Bold",
      fontSize: 64,
      color: "#ffcf5a",
      alignment: "left",
      strokeColor: "#000000",
      backgroundColor: "rgba(0,0,0,0.5)",
    });
    expect(textContent(item)).toBe("Hello");
  });

  it("fills missing overlay metadata with the legacy inline defaults", () => {
    expect(textOverlayMetadata(overlay({ motion: "slide in" }))).toEqual({
      visualTreatment: "clean editable text overlay with high-contrast type and transparent backing",
      motion: "slide in",
      safeZone: "keep text inside 10% title-safe margins",
      avoid: "opaque slabs, default-font template look, and covering faces or key action",
    });
  });
});

describe("editTextAction", () => {
  it("emits editTextItem with trimmed text", () => {
    expect(actionsOf(editTextAction(overlay(), "  New title  "))).toEqual([
      { type: "editTextItem", itemId: "title", text: "New title" },
    ]);
  });

  it("rejects empty text, templates and non-text items", () => {
    expect(blockedOf(editTextAction(overlay(), "   "))).toMatch(/empty/i);
    expect(blockedOf(editTextAction(overlay({ templateId: "kinetic-lower-third-v1" }), "x"))).toMatch(/text overlay/i);
    expect(blockedOf(editTextAction({ ...overlay(), kind: "caption" }, "x"))).toMatch(/text overlay/i);
  });
});

describe("textOverlayUpdateAction", () => {
  it("emits updateTextOverlayItems keeping timing and filling metadata defaults", () => {
    expect(actionsOf(textOverlayUpdateAction(overlay({ safeZone: "center safe" }), { motion: "pop" }))).toEqual([
      {
        type: "updateTextOverlayItems",
        updates: [
          {
            itemId: "title",
            startSeconds: 1,
            durationSeconds: 3,
            text: "Hello",
            visualTreatment: "clean editable text overlay with high-contrast type and transparent backing",
            motion: "pop",
            safeZone: "center safe",
            avoid: "opaque slabs, default-font template look, and covering faces or key action",
          },
        ],
      },
    ]);
  });

  it("rejects blank fields", () => {
    expect(blockedOf(textOverlayUpdateAction(overlay(), { avoid: " " }))).toMatch(/avoid/i);
  });
});

describe("textStyleAction", () => {
  it("sets style properties on every item in one updateItemProperties action", () => {
    expect(actionsOf(textStyleAction(["title", "other"], { fontSize: 72, strokeColor: "#000000", alignment: "right" }))).toEqual([
      {
        type: "updateItemProperties",
        updates: [
          { itemId: "title", set: { fontSize: 72, borderColor: "#000000", alignment: "right" }, remove: [] },
          { itemId: "other", set: { fontSize: 72, borderColor: "#000000", alignment: "right" }, remove: [] },
        ],
      },
    ]);
  });

  it("removes properties set to null and trims strings", () => {
    expect(actionsOf(textStyleAction(["title"], { backgroundColor: null, fontName: " Inter " }))).toEqual([
      { type: "updateItemProperties", updates: [{ itemId: "title", set: { fontName: "Inter" }, remove: ["backgroundColor"] }] },
    ]);
  });

  it("rejects invalid values and empty edits", () => {
    expect(blockedOf(textStyleAction(["title"], { fontSize: 0 }))).toMatch(/size/i);
    expect(blockedOf(textStyleAction(["title"], { color: "  " }))).toMatch(/color/i);
    expect(blockedOf(textStyleAction(["title"], {}))).toMatch(/style/i);
    expect(blockedOf(textStyleAction([], { fontSize: 20 }))).toMatch(/select/i);
  });
});
