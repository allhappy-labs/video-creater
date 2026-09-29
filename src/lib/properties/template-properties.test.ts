import { describe, expect, it } from "vitest";
import { createTemplateOverlayItem, kineticLowerThirdTemplate } from "@/lib/motion-templates";
import {
  templateContent,
  templateFieldAction,
  templateMetadataAction,
  templateMotionPreset,
  templateStyleAction,
} from "@/lib/properties/template-properties";
import type { TimelineItem } from "@/lib/timeline";
import type { CommandResult } from "@/lib/timeline-ops/clip-commands";

function lowerThird(properties: Record<string, unknown> = {}): TimelineItem {
  const item = createTemplateOverlayItem({ templateId: "kinetic-lower-third-v1", itemId: "lt", startSeconds: 1.5 });
  return { ...item, properties: { ...item.properties, ...properties } };
}

function plainOverlay(): TimelineItem {
  return {
    id: "plain",
    kind: "overlay",
    startSeconds: 0,
    durationSeconds: 2,
    source: { type: "text", text: "Hi" },
    label: "Hi",
    properties: {},
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

describe("template readers", () => {
  it("wrap the template fields, style and metadata helpers", () => {
    const content = templateContent(
      lowerThird({ templateFields: { headline: "Ada" }, templateStyle: { accentColor: " #ff0000 " }, motion: "" }),
    );
    expect(content?.template.id).toBe("kinetic-lower-third-v1");
    expect(content?.fields).toEqual({ headline: "Ada", subline: "Context label" });
    expect(content?.style).toEqual({
      accentColor: "#ff0000",
      backgroundColor: "rgba(2, 6, 23, 0.72)",
      textColor: "#ffffff",
    });
    expect(content?.metadata.visualTreatment).toBe(kineticLowerThirdTemplate.visualTreatment);
  });

  it("return null for items that are not catalog templates", () => {
    expect(templateContent(plainOverlay())).toBeNull();
    expect(templateContent(lowerThird({ templateId: "unknown-template" }))).toBeNull();
  });

  it("read the motion preset, falling back to the template default", () => {
    expect(templateMotionPreset(lowerThird({ motionPresetId: "spring-pop-v2" }))).toBe("spring-pop-v2");
    expect(templateMotionPreset(lowerThird({ motionPresetId: "bogus" }))).toBe("slide-fade-up-v1");
    expect(templateMotionPreset(plainOverlay())).toBeNull();
  });
});

describe("templateFieldAction", () => {
  it("emits updateTemplateItems with merged, trimmed fields and unchanged timing", () => {
    expect(actionsOf(templateFieldAction(lowerThird(), "headline", "  Grace Hopper "))).toEqual([
      {
        type: "updateTemplateItems",
        updates: [
          {
            itemId: "lt",
            startSeconds: 1.5,
            durationSeconds: 2.4,
            templateFields: { headline: "Grace Hopper", subline: "Context label" },
          },
        ],
      },
    ]);
  });

  it("rejects unknown fields, blank required fields and non-templates", () => {
    expect(blockedOf(templateFieldAction(lowerThird(), "footer", "x"))).toMatch(/field/i);
    expect(blockedOf(templateFieldAction(lowerThird(), "headline", "  "))).toMatch(/headline/i);
    expect(blockedOf(templateFieldAction(plainOverlay(), "headline", "x"))).toMatch(/template/i);
  });
});

describe("template overrides", () => {
  const override = {
    templateId: "kinetic-lower-third-v1",
    name: "Kinetic Lower Third",
    fields: { headline: "Name / Role", subline: "Context label" },
    style: { accentColor: "#22d3ee", backgroundColor: "rgba(2, 6, 23, 0.72)", textColor: "#ffffff" },
    visualTreatment: kineticLowerThirdTemplate.visualTreatment,
    motion: kineticLowerThirdTemplate.motion,
    safeZone: kineticLowerThirdTemplate.safeZone,
    avoid: kineticLowerThirdTemplate.avoid,
  };

  it("emits updateTemplateOverride with one trimmed style key changed", () => {
    expect(actionsOf(templateStyleAction(lowerThird(), "accentColor", " #ff00aa "))).toEqual([
      { type: "updateTemplateOverride", override: { ...override, style: { ...override.style, accentColor: "#ff00aa" } } },
    ]);
  });

  it("emits updateTemplateOverride with one trimmed metadata key changed", () => {
    expect(actionsOf(templateMetadataAction(lowerThird(), "motion", " quick snap "))).toEqual([
      { type: "updateTemplateOverride", override: { ...override, motion: "quick snap" } },
    ]);
  });

  it("rejects blank values and non-templates", () => {
    expect(blockedOf(templateStyleAction(lowerThird(), "textColor", " "))).toMatch(/color/i);
    expect(blockedOf(templateMetadataAction(lowerThird(), "avoid", ""))).toMatch(/avoid/i);
    expect(blockedOf(templateStyleAction(plainOverlay(), "textColor", "#fff"))).toMatch(/template/i);
  });
});
