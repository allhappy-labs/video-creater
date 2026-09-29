import { describe, expect, it } from "vitest";
import {
  createTemplateOverlayItem,
  gradientBackgroundLoopTemplate,
  getMotionTemplate,
  holographicLogoTemplate,
  kineticLowerThirdTemplate,
  motionTemplateCatalog,
} from "./motion-templates";
import { motionPresetCatalog } from "./motion-presets";

describe("motion template catalog", () => {
  it("ships a compact built-in manifest catalog with visual metadata", () => {
    expect(motionTemplateCatalog).toHaveLength(7);
    expect(kineticLowerThirdTemplate).toMatchObject({
      id: "kinetic-lower-third-v1",
      name: "Kinetic Lower Third",
      category: "lower_thirds",
      kind: "overlay",
      durationSeconds: 2.4,
      version: 1,
      fieldDefinitions: [
        { name: "headline", label: "Headline", multiline: false, required: true },
        { name: "subline", label: "Subline", multiline: false, required: true },
      ],
      placement: { trackKind: "overlay", defaultStartSeconds: 0 },
      renderContract: { dimensions: "project", fps: "project", alpha: true },
      preview: {
        thumbnailKind: "css",
        cssVariant: "lower-third",
      },
    });
    expect(kineticLowerThirdTemplate.visualTreatment).toContain("lower-third");
    expect(kineticLowerThirdTemplate.motion).toContain("slide");
    expect(kineticLowerThirdTemplate.safeZone).toContain("10% margins");
    expect(kineticLowerThirdTemplate.avoid).toContain("opaque black slabs");
    expect(motionTemplateCatalog.map((template) => template.id)).toEqual([
      "kinetic-lower-third-v1",
      "punchy-caption-v1",
      "metric-callout-v1",
      "chapter-card-v1",
      "tracking-highlight-v1",
      "holographic-logo-cutout-v1",
      "gradient-background-loop-v1",
    ]);
  });

  it("looks up templates by id", () => {
    expect(getMotionTemplate("kinetic-lower-third-v1")?.name).toBe("Kinetic Lower Third");
    expect(getMotionTemplate("missing-template")).toBeNull();
  });

  it("assigns deterministic motion presets to every built-in template", () => {
    expect(motionPresetCatalog.map((preset) => preset.id)).toEqual([
      "slide-fade-up-v1",
      "snap-pop-v1",
      "underline-wipe-v1",
      "metric-count-pop-v1",
      "vertical-reveal-v1",
      "tracking-draw-v1",
      "spring-pop-v2",
      "slide-rotate-settle-v2",
      "mask-wipe-v2",
      "line-draw-v2",
      "word-pop-stagger-v2",
      "soft-depth-card-v2",
      "pulse-emphasis-v2",
      "exit-snap-v2",
    ]);
    expect(
      motionTemplateCatalog.map((template) => [template.id, template.motionPresetId]),
    ).toEqual([
      ["kinetic-lower-third-v1", "slide-fade-up-v1"],
      ["punchy-caption-v1", "snap-pop-v1"],
      ["metric-callout-v1", "metric-count-pop-v1"],
      ["chapter-card-v1", "vertical-reveal-v1"],
      ["tracking-highlight-v1", "tracking-draw-v1"],
      ["holographic-logo-cutout-v1", "pulse-emphasis-v2"],
      ["gradient-background-loop-v1", "soft-depth-card-v2"],
    ]);
  });

  it("ships a holographic logo cutout template with a built-in logo asset contract", () => {
    expect(holographicLogoTemplate).toMatchObject({
      id: "holographic-logo-cutout-v1",
      name: "Holographic Logo Cutout",
      category: "titles",
      kind: "overlay",
      durationSeconds: 3.2,
      version: 1,
      fieldDefinitions: [
        { name: "logoAssetId", label: "Logo asset", multiline: false, required: true },
      ],
      defaultTextFields: {
        logoAssetId: "builtin:v-photo-light",
      },
      placement: { trackKind: "overlay", defaultStartSeconds: 0 },
      renderContract: { dimensions: "project", fps: "project", alpha: true },
      preview: {
        thumbnailKind: "css",
        cssVariant: "holographic-logo",
      },
    });
    expect(holographicLogoTemplate.visualTreatment).toContain("holographic");
    expect(holographicLogoTemplate.motion).toContain("shader");
    expect(holographicLogoTemplate.safeZone).toContain("central");
    expect(holographicLogoTemplate.avoid).toContain("plain boxes");
  });

  it("ships a gradient background loop text template matching the reference concept", () => {
    expect(gradientBackgroundLoopTemplate).toMatchObject({
      id: "gradient-background-loop-v1",
      name: "Gradient Background Loop",
      category: "text",
      kind: "overlay",
      durationSeconds: 4,
      version: 1,
      fieldDefinitions: [
        { name: "headline", label: "Headline", multiline: true, required: true },
      ],
      defaultTextFields: {
        headline: "Love\nwins.",
      },
      placement: { trackKind: "overlay", defaultStartSeconds: 0 },
      renderContract: { dimensions: "project", fps: "project", alpha: true },
      preview: {
        thumbnailKind: "css",
        cssVariant: "gradient-background-loop",
      },
    });
    expect(gradientBackgroundLoopTemplate.visualTreatment).toContain("vertical blue gradient panels");
    expect(gradientBackgroundLoopTemplate.motion).toContain("loop");
    expect(gradientBackgroundLoopTemplate.safeZone).toContain("10% margins");
    expect(gradientBackgroundLoopTemplate.avoid).toContain("plain boxes");
  });

  it("creates a canonical overlay item from a template and field overrides", () => {
    const item = createTemplateOverlayItem({
      templateId: "kinetic-lower-third-v1",
      itemId: "template-item-1",
      startSeconds: 1.25,
      fields: {
        headline: "Olha API",
        subline: "Founder",
      },
    });

    expect(item).toMatchObject({
      id: "template-item-1",
      kind: "overlay",
      startSeconds: 1.25,
      durationSeconds: 2.4,
      source: {
        type: "generated",
        artifactId: "template:kinetic-lower-third-v1:template-item-1",
      },
      label: "Kinetic Lower Third",
      properties: {
        templateId: "kinetic-lower-third-v1",
        templateFields: {
          headline: "Olha API",
          subline: "Founder",
        },
        motionPresetId: kineticLowerThirdTemplate.motionPresetId,
        visualTreatment: kineticLowerThirdTemplate.visualTreatment,
        motion: kineticLowerThirdTemplate.motion,
        safeZone: kineticLowerThirdTemplate.safeZone,
        avoid: kineticLowerThirdTemplate.avoid,
      },
    });
  });

  it("rejects unknown template ids and empty required fields", () => {
    expect(() =>
      createTemplateOverlayItem({
        templateId: "missing-template",
        itemId: "template-item-1",
        startSeconds: 0,
        fields: { headline: "Name", subline: "Role" },
      }),
    ).toThrow("Unknown motion template: missing-template");

    expect(() =>
      createTemplateOverlayItem({
        templateId: "kinetic-lower-third-v1",
        itemId: "template-item-1",
        startSeconds: 0,
        fields: { headline: "   ", subline: "Role" },
      }),
    ).toThrow("Template field headline cannot be empty");
  });

  it("creates canonical overlay items for every built-in template", () => {
    for (const template of motionTemplateCatalog) {
      const item = createTemplateOverlayItem({
        templateId: template.id,
        itemId: `item-${template.id}`,
        startSeconds: template.placement.defaultStartSeconds,
      });

      expect(item.kind).toBe("overlay");
      expect(item.label).toBe(template.name);
      expect(item.durationSeconds).toBe(template.durationSeconds);
      expect(item.properties.templateId).toBe(template.id);
      expect(item.properties.templateFields).toEqual(template.defaultTextFields);
      expect(item.properties.previewVariant).toBe(template.preview.cssVariant);
      expect(item.properties.motionPresetId).toBe(template.motionPresetId);
    }
  });
});
