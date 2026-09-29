import { describe, expect, it } from "vitest";
import {
  defaultTemplateStyle as workspaceDefaultTemplateStyle,
  templateFieldsForItem,
  templateMetadataForItem,
  templateStyleForItem,
} from "@/lib/templates/template-item";
import {
  defaultTemplateStyle,
  getMetadataValue,
  getStyleValue,
  getTemplateFields,
  type TemplateMetadataKey,
} from "@/lib/templates/template-item";
import {
  getTemplateFields as previewGetTemplateFields,
  gradientLoopPanelStyles as previewGradientLoopPanelStyles,
  splitPreviewLines as previewSplitPreviewLines,
} from "@/lib/templates/template-item";
import {
  backgroundCategories,
  categoryLabels,
  categoryOrder,
  formatCategory,
  formatPlacementTime,
  formatSourceKind,
  formatTrack,
  gradientLoopPanelStyles,
  splitPreviewLines,
  templateCategories,
} from "@/lib/templates/template-item";
import { formatCategory as textLibraryFormatCategory } from "@/lib/templates/template-item";
import {
  createTemplateOverlayItem,
  getMotionTemplate,
  motionTemplateCatalog,
  type MotionTemplateDefinition,
} from "@/lib/motion-templates";
import type { TimelineItem, TrackKind } from "@/lib/timeline";

const lowerThirdId = "kinetic-lower-third-v1";

function lowerThirdItem(): TimelineItem {
  return createTemplateOverlayItem({
    templateId: lowerThirdId,
    itemId: "template-item-1",
    startSeconds: 0,
    fields: { headline: "Olha API", subline: "Founder" },
  });
}

function templateItems(): Array<[string, TimelineItem]> {
  const base = lowerThirdItem();
  const { templateFields: _fields, ...withoutFieldProperties } = base.properties;
  return [
    ["with fields", base],
    ["without fields", { ...base, properties: withoutFieldProperties }],
    ["array fields", { ...base, properties: { ...base.properties, templateFields: ["headline"] } }],
    [
      "non-string fields",
      {
        ...base,
        properties: {
          ...base.properties,
          templateFields: { headline: 42, subline: null, flag: true, nested: { a: 1 } },
        },
      },
    ],
    [
      "style overrides",
      {
        ...base,
        properties: {
          ...base.properties,
          templateStyle: {
            accentColor: "  #ff0000  ",
            backgroundColor: "   ",
            textColor: 5,
            extraKey: "custom",
          },
        },
      },
    ],
    ["array style", { ...base, properties: { ...base.properties, templateStyle: ["#fff"] } }],
    [
      "metadata overrides",
      {
        ...base,
        properties: {
          ...base.properties,
          visualTreatment: "",
          motion: "  ",
          safeZone: " custom safe zone ",
          avoid: 42,
        },
      },
    ],
  ];
}

const styleKeys = ["accentColor", "backgroundColor", "textColor"] as const;
const metadataKeys: TemplateMetadataKey[] = ["visualTreatment", "motion", "safeZone", "avoid"];
const allCategories = ["text", "titles", "lower_thirds", "captions", "callouts", "transitions"];
const previewTexts = [
  "Love\nwins.",
  "  first \r\n\n  second  \n",
  "",
  "   ",
  undefined,
  "word ".repeat(200),
];

describe("template item characterization", () => {
  it("shares the default template style", () => {
    expect(workspaceDefaultTemplateStyle).toEqual(defaultTemplateStyle);
    expect(defaultTemplateStyle).toMatchInlineSnapshot(`
      {
        "accentColor": "#22d3ee",
        "backgroundColor": "rgba(2, 6, 23, 0.72)",
        "textColor": "#ffffff",
      }
    `);
  });

  it("reads template fields identically in the inspector and preview", () => {
    const items = templateItems();
    const fields = items.map(([label, item]) => [label, getTemplateFields(item)]);
    expect(items.map(([label, item]) => [label, previewGetTemplateFields(item)])).toEqual(fields);
    expect(getTemplateFields(null)).toEqual({});
    expect(fields).toMatchInlineSnapshot(`
      [
        [
          "with fields",
          {
            "headline": "Olha API",
            "subline": "Founder",
          },
        ],
        [
          "without fields",
          {},
        ],
        [
          "array fields",
          {},
        ],
        [
          "non-string fields",
          {
            "flag": "true",
            "headline": "42",
            "nested": "[object Object]",
            "subline": "",
          },
        ],
        [
          "style overrides",
          {
            "headline": "Olha API",
            "subline": "Founder",
          },
        ],
        [
          "array style",
          {
            "headline": "Olha API",
            "subline": "Founder",
          },
        ],
        [
          "metadata overrides",
          {
            "headline": "Olha API",
            "subline": "Founder",
          },
        ],
      ]
    `);
  });

  it("merges workspace template fields with template defaults", () => {
    expect(
      templateItems().map(([label, item]) => [
        label,
        templateFieldsForItem(item, lowerThirdId),
        templateFieldsForItem(item, "missing-template"),
      ]),
    ).toMatchInlineSnapshot(`
      [
        [
          "with fields",
          {
            "headline": "Olha API",
            "subline": "Founder",
          },
          {
            "headline": "Olha API",
            "subline": "Founder",
          },
        ],
        [
          "without fields",
          {
            "headline": "Name / Role",
            "subline": "Context label",
          },
          {},
        ],
        [
          "array fields",
          {
            "headline": "Name / Role",
            "subline": "Context label",
          },
          {},
        ],
        [
          "non-string fields",
          {
            "flag": "true",
            "headline": "42",
            "nested": "[object Object]",
            "subline": "",
          },
          {
            "flag": "true",
            "headline": "42",
            "nested": "[object Object]",
            "subline": "",
          },
        ],
        [
          "style overrides",
          {
            "headline": "Olha API",
            "subline": "Founder",
          },
          {
            "headline": "Olha API",
            "subline": "Founder",
          },
        ],
        [
          "array style",
          {
            "headline": "Olha API",
            "subline": "Founder",
          },
          {
            "headline": "Olha API",
            "subline": "Founder",
          },
        ],
        [
          "metadata overrides",
          {
            "headline": "Olha API",
            "subline": "Founder",
          },
          {
            "headline": "Olha API",
            "subline": "Founder",
          },
        ],
      ]
    `);
  });

  it("resolves template style values in the inspector and workspace variants", () => {
    expect(
      [...templateItems(), ["null item", null] as const].map(([label, item]) => [
        label,
        Object.fromEntries(styleKeys.map((key) => [key, getStyleValue(item, key)])),
        item ? templateStyleForItem(item) : null,
      ]),
    ).toMatchInlineSnapshot(`
      [
        [
          "with fields",
          {
            "accentColor": "#22d3ee",
            "backgroundColor": "rgba(2, 6, 23, 0.72)",
            "textColor": "#ffffff",
          },
          {
            "accentColor": "#22d3ee",
            "backgroundColor": "rgba(2, 6, 23, 0.72)",
            "textColor": "#ffffff",
          },
        ],
        [
          "without fields",
          {
            "accentColor": "#22d3ee",
            "backgroundColor": "rgba(2, 6, 23, 0.72)",
            "textColor": "#ffffff",
          },
          {
            "accentColor": "#22d3ee",
            "backgroundColor": "rgba(2, 6, 23, 0.72)",
            "textColor": "#ffffff",
          },
        ],
        [
          "array fields",
          {
            "accentColor": "#22d3ee",
            "backgroundColor": "rgba(2, 6, 23, 0.72)",
            "textColor": "#ffffff",
          },
          {
            "accentColor": "#22d3ee",
            "backgroundColor": "rgba(2, 6, 23, 0.72)",
            "textColor": "#ffffff",
          },
        ],
        [
          "non-string fields",
          {
            "accentColor": "#22d3ee",
            "backgroundColor": "rgba(2, 6, 23, 0.72)",
            "textColor": "#ffffff",
          },
          {
            "accentColor": "#22d3ee",
            "backgroundColor": "rgba(2, 6, 23, 0.72)",
            "textColor": "#ffffff",
          },
        ],
        [
          "style overrides",
          {
            "accentColor": "  #ff0000  ",
            "backgroundColor": "rgba(2, 6, 23, 0.72)",
            "textColor": "#ffffff",
          },
          {
            "accentColor": "#ff0000",
            "backgroundColor": "rgba(2, 6, 23, 0.72)",
            "extraKey": "custom",
            "textColor": "#ffffff",
          },
        ],
        [
          "array style",
          {
            "accentColor": "#22d3ee",
            "backgroundColor": "rgba(2, 6, 23, 0.72)",
            "textColor": "#ffffff",
          },
          {
            "accentColor": "#22d3ee",
            "backgroundColor": "rgba(2, 6, 23, 0.72)",
            "textColor": "#ffffff",
          },
        ],
        [
          "metadata overrides",
          {
            "accentColor": "#22d3ee",
            "backgroundColor": "rgba(2, 6, 23, 0.72)",
            "textColor": "#ffffff",
          },
          {
            "accentColor": "#22d3ee",
            "backgroundColor": "rgba(2, 6, 23, 0.72)",
            "textColor": "#ffffff",
          },
        ],
        [
          "null item",
          {
            "accentColor": "#22d3ee",
            "backgroundColor": "rgba(2, 6, 23, 0.72)",
            "textColor": "#ffffff",
          },
          null,
        ],
      ]
    `);
  });

  it("resolves template metadata in the inspector and workspace variants", () => {
    const template = requiredTemplate(lowerThirdId);
    expect(
      [...templateItems(), ["null item", null] as const].map(([label, item]) => [
        label,
        Object.fromEntries(metadataKeys.map((key) => [key, getMetadataValue(item, template, key)])),
        Object.fromEntries(metadataKeys.map((key) => [key, getMetadataValue(item, null, key)])),
        item ? templateMetadataForItem(item, template) : null,
      ]),
    ).toMatchInlineSnapshot(`
      [
        [
          "with fields",
          {
            "avoid": "full-width opaque black slabs, centered title-card layout, default-font template look, and long unmoving holds",
            "motion": "slide-and-fade in over 8 frames, hold, then soft fade out",
            "safeZone": "keep essential text inside 10% margins and below face/action priority areas",
            "visualTreatment": "compact lower-third block with translucent backing, accent rule, and strong hierarchy",
          },
          {
            "avoid": "full-width opaque black slabs, centered title-card layout, default-font template look, and long unmoving holds",
            "motion": "slide-and-fade in over 8 frames, hold, then soft fade out",
            "safeZone": "keep essential text inside 10% margins and below face/action priority areas",
            "visualTreatment": "compact lower-third block with translucent backing, accent rule, and strong hierarchy",
          },
          {
            "avoid": "full-width opaque black slabs, centered title-card layout, default-font template look, and long unmoving holds",
            "motion": "slide-and-fade in over 8 frames, hold, then soft fade out",
            "safeZone": "keep essential text inside 10% margins and below face/action priority areas",
            "visualTreatment": "compact lower-third block with translucent backing, accent rule, and strong hierarchy",
          },
        ],
        [
          "without fields",
          {
            "avoid": "full-width opaque black slabs, centered title-card layout, default-font template look, and long unmoving holds",
            "motion": "slide-and-fade in over 8 frames, hold, then soft fade out",
            "safeZone": "keep essential text inside 10% margins and below face/action priority areas",
            "visualTreatment": "compact lower-third block with translucent backing, accent rule, and strong hierarchy",
          },
          {
            "avoid": "full-width opaque black slabs, centered title-card layout, default-font template look, and long unmoving holds",
            "motion": "slide-and-fade in over 8 frames, hold, then soft fade out",
            "safeZone": "keep essential text inside 10% margins and below face/action priority areas",
            "visualTreatment": "compact lower-third block with translucent backing, accent rule, and strong hierarchy",
          },
          {
            "avoid": "full-width opaque black slabs, centered title-card layout, default-font template look, and long unmoving holds",
            "motion": "slide-and-fade in over 8 frames, hold, then soft fade out",
            "safeZone": "keep essential text inside 10% margins and below face/action priority areas",
            "visualTreatment": "compact lower-third block with translucent backing, accent rule, and strong hierarchy",
          },
        ],
        [
          "array fields",
          {
            "avoid": "full-width opaque black slabs, centered title-card layout, default-font template look, and long unmoving holds",
            "motion": "slide-and-fade in over 8 frames, hold, then soft fade out",
            "safeZone": "keep essential text inside 10% margins and below face/action priority areas",
            "visualTreatment": "compact lower-third block with translucent backing, accent rule, and strong hierarchy",
          },
          {
            "avoid": "full-width opaque black slabs, centered title-card layout, default-font template look, and long unmoving holds",
            "motion": "slide-and-fade in over 8 frames, hold, then soft fade out",
            "safeZone": "keep essential text inside 10% margins and below face/action priority areas",
            "visualTreatment": "compact lower-third block with translucent backing, accent rule, and strong hierarchy",
          },
          {
            "avoid": "full-width opaque black slabs, centered title-card layout, default-font template look, and long unmoving holds",
            "motion": "slide-and-fade in over 8 frames, hold, then soft fade out",
            "safeZone": "keep essential text inside 10% margins and below face/action priority areas",
            "visualTreatment": "compact lower-third block with translucent backing, accent rule, and strong hierarchy",
          },
        ],
        [
          "non-string fields",
          {
            "avoid": "full-width opaque black slabs, centered title-card layout, default-font template look, and long unmoving holds",
            "motion": "slide-and-fade in over 8 frames, hold, then soft fade out",
            "safeZone": "keep essential text inside 10% margins and below face/action priority areas",
            "visualTreatment": "compact lower-third block with translucent backing, accent rule, and strong hierarchy",
          },
          {
            "avoid": "full-width opaque black slabs, centered title-card layout, default-font template look, and long unmoving holds",
            "motion": "slide-and-fade in over 8 frames, hold, then soft fade out",
            "safeZone": "keep essential text inside 10% margins and below face/action priority areas",
            "visualTreatment": "compact lower-third block with translucent backing, accent rule, and strong hierarchy",
          },
          {
            "avoid": "full-width opaque black slabs, centered title-card layout, default-font template look, and long unmoving holds",
            "motion": "slide-and-fade in over 8 frames, hold, then soft fade out",
            "safeZone": "keep essential text inside 10% margins and below face/action priority areas",
            "visualTreatment": "compact lower-third block with translucent backing, accent rule, and strong hierarchy",
          },
        ],
        [
          "style overrides",
          {
            "avoid": "full-width opaque black slabs, centered title-card layout, default-font template look, and long unmoving holds",
            "motion": "slide-and-fade in over 8 frames, hold, then soft fade out",
            "safeZone": "keep essential text inside 10% margins and below face/action priority areas",
            "visualTreatment": "compact lower-third block with translucent backing, accent rule, and strong hierarchy",
          },
          {
            "avoid": "full-width opaque black slabs, centered title-card layout, default-font template look, and long unmoving holds",
            "motion": "slide-and-fade in over 8 frames, hold, then soft fade out",
            "safeZone": "keep essential text inside 10% margins and below face/action priority areas",
            "visualTreatment": "compact lower-third block with translucent backing, accent rule, and strong hierarchy",
          },
          {
            "avoid": "full-width opaque black slabs, centered title-card layout, default-font template look, and long unmoving holds",
            "motion": "slide-and-fade in over 8 frames, hold, then soft fade out",
            "safeZone": "keep essential text inside 10% margins and below face/action priority areas",
            "visualTreatment": "compact lower-third block with translucent backing, accent rule, and strong hierarchy",
          },
        ],
        [
          "array style",
          {
            "avoid": "full-width opaque black slabs, centered title-card layout, default-font template look, and long unmoving holds",
            "motion": "slide-and-fade in over 8 frames, hold, then soft fade out",
            "safeZone": "keep essential text inside 10% margins and below face/action priority areas",
            "visualTreatment": "compact lower-third block with translucent backing, accent rule, and strong hierarchy",
          },
          {
            "avoid": "full-width opaque black slabs, centered title-card layout, default-font template look, and long unmoving holds",
            "motion": "slide-and-fade in over 8 frames, hold, then soft fade out",
            "safeZone": "keep essential text inside 10% margins and below face/action priority areas",
            "visualTreatment": "compact lower-third block with translucent backing, accent rule, and strong hierarchy",
          },
          {
            "avoid": "full-width opaque black slabs, centered title-card layout, default-font template look, and long unmoving holds",
            "motion": "slide-and-fade in over 8 frames, hold, then soft fade out",
            "safeZone": "keep essential text inside 10% margins and below face/action priority areas",
            "visualTreatment": "compact lower-third block with translucent backing, accent rule, and strong hierarchy",
          },
        ],
        [
          "metadata overrides",
          {
            "avoid": "full-width opaque black slabs, centered title-card layout, default-font template look, and long unmoving holds",
            "motion": "slide-and-fade in over 8 frames, hold, then soft fade out",
            "safeZone": " custom safe zone ",
            "visualTreatment": "compact lower-third block with translucent backing, accent rule, and strong hierarchy",
          },
          {
            "avoid": "",
            "motion": "",
            "safeZone": " custom safe zone ",
            "visualTreatment": "",
          },
          {
            "avoid": "full-width opaque black slabs, centered title-card layout, default-font template look, and long unmoving holds",
            "motion": "  ",
            "safeZone": " custom safe zone ",
            "visualTreatment": "",
          },
        ],
        [
          "null item",
          {
            "avoid": "full-width opaque black slabs, centered title-card layout, default-font template look, and long unmoving holds",
            "motion": "slide-and-fade in over 8 frames, hold, then soft fade out",
            "safeZone": "keep essential text inside 10% margins and below face/action priority areas",
            "visualTreatment": "compact lower-third block with translucent backing, accent rule, and strong hierarchy",
          },
          {
            "avoid": "",
            "motion": "",
            "safeZone": "",
            "visualTreatment": "",
          },
          null,
        ],
      ]
    `);
  });

  it("formats template categories identically in the library and text panel", () => {
    const inputs = [...allCategories, "", "a_b_c", "already Title"];
    const labels = inputs.map((category) => formatCategory(category));
    expect(inputs.map((category) => textLibraryFormatCategory(category))).toEqual(labels);
    expect(labels).toMatchInlineSnapshot(`
      [
        "Text",
        "Titles",
        "Lower thirds",
        "Captions",
        "Callouts",
        "Transitions",
        "",
        "A b c",
        "Already Title",
      ]
    `);
  });

  it("formats source kinds, tracks and placement times", () => {
    const trackKinds: TrackKind[] = ["video", "hyperframe_scene", "overlay", "caption", "audio"];
    expect({
      sourceKinds: [formatSourceKind("built_in"), formatSourceKind("user")],
      tracks: trackKinds.map((trackKind) => formatTrack(trackKind)),
      placementTimes: [-1, 0, 1.5, 59.9999, 61.25, 3600].map((seconds) => formatPlacementTime(seconds)),
    }).toMatchInlineSnapshot(`
      {
        "placementTimes": [
          "00:00.000",
          "00:00.000",
          "00:01.500",
          "00:60.000",
          "01:01.250",
          "60:00.000",
        ],
        "sourceKinds": [
          "Built-in",
          "User",
        ],
        "tracks": [
          "video",
          "HyperFrames",
          "Overlays",
          "Captions",
          "audio",
        ],
      }
    `);
  });

  it("classifies template library categories", () => {
    const base = requiredTemplate(lowerThirdId);
    const synthetic: Array<[string, MotionTemplateDefinition]> = [
      ["transition kind", { ...base, kind: "transition" }],
      ["transitions category", { ...base, category: "transitions" }],
      ["hyperframe kind", { ...base, kind: "hyperframe_scene" }],
      ["hyperframe track", { ...base, placement: { ...base.placement, trackKind: "hyperframe_scene" } }],
      ["caption kind", { ...base, kind: "caption" }],
      ["captions category", { ...base, category: "captions" }],
      ["caption track", { ...base, placement: { ...base.placement, trackKind: "caption" } }],
      ["transition and caption", { ...base, kind: "caption", category: "transitions" }],
    ];
    expect({
      categoryLabels,
      categoryOrder,
      backgroundCategories: backgroundCategories(),
      catalog: motionTemplateCatalog.map((template) => [
        template.id,
        template.category,
        formatCategory(template.category),
        templateCategories(template),
      ]),
      synthetic: synthetic.map(([label, template]) => [label, templateCategories(template)]),
    }).toMatchInlineSnapshot(`
      {
        "backgroundCategories": [
          "hyperframes",
          "shader-backgrounds",
        ],
        "catalog": [
          [
            "kinetic-lower-third-v1",
            "lower_thirds",
            "Lower thirds",
            [
              "overlays",
            ],
          ],
          [
            "punchy-caption-v1",
            "captions",
            "Captions",
            [
              "captions",
            ],
          ],
          [
            "metric-callout-v1",
            "callouts",
            "Callouts",
            [
              "overlays",
            ],
          ],
          [
            "chapter-card-v1",
            "titles",
            "Titles",
            [
              "overlays",
            ],
          ],
          [
            "tracking-highlight-v1",
            "callouts",
            "Callouts",
            [
              "overlays",
            ],
          ],
          [
            "holographic-logo-cutout-v1",
            "titles",
            "Titles",
            [
              "overlays",
            ],
          ],
          [
            "gradient-background-loop-v1",
            "text",
            "Text",
            [
              "overlays",
            ],
          ],
        ],
        "categoryLabels": {
          "captions": "Captions",
          "hyperframes": "HyperFrames",
          "overlays": "Overlays",
          "shader-backgrounds": "Shader backgrounds",
          "transitions": "Transitions",
        },
        "categoryOrder": [
          "overlays",
          "captions",
          "hyperframes",
          "transitions",
          "shader-backgrounds",
        ],
        "synthetic": [
          [
            "transition kind",
            [
              "transitions",
            ],
          ],
          [
            "transitions category",
            [
              "transitions",
            ],
          ],
          [
            "hyperframe kind",
            [
              "hyperframes",
            ],
          ],
          [
            "hyperframe track",
            [
              "hyperframes",
            ],
          ],
          [
            "caption kind",
            [
              "captions",
            ],
          ],
          [
            "captions category",
            [
              "captions",
            ],
          ],
          [
            "caption track",
            [
              "captions",
            ],
          ],
          [
            "transition and caption",
            [
              "transitions",
            ],
          ],
        ],
      }
    `);
  });

  it("shares gradient loop panel styles and preview line splitting", () => {
    expect(previewGradientLoopPanelStyles).toEqual(gradientLoopPanelStyles);
    const lines = previewTexts.map((text) => splitPreviewLines(text));
    expect(previewTexts.map((text) => previewSplitPreviewLines(text))).toEqual(lines);
    expect(gradientLoopPanelStyles).toMatchInlineSnapshot(`
      [
        "linear-gradient(180deg,#146492 0%,#0b3b56 25%,#d4e1e1 31%,#d6e3e0 50%,#2191d6 62%,#1c87d1 100%)",
        "linear-gradient(180deg,#0d4363 0%,#031417 22%,#0a405f 46%,#bddfe9 58%,#75b9d6 78%,#1f8cd0 100%)",
        "linear-gradient(180deg,#40aaf0 0%,#3da3dc 5%,#020f12 6%,#031f2b 36%,#104d73 56%,#031416 70%,#1f92d8 100%)",
        "linear-gradient(180deg,#1f8bd0 0%,#166b9e 22%,#0f496b 54%,#1e8bd1 68%,#2094dc 100%)",
        "linear-gradient(180deg,#aad5e3 0%,#6bb3d6 16%,#2393d6 48%,#2aa1e4 74%,#d5e2df 80%,#d6e0de 100%)",
      ]
    `);
    expect(
      lines.map((textLines) => textLines.map((line) => (line.length > 40 ? `${line.length} chars` : line))),
    ).toMatchInlineSnapshot(`
      [
        [
          "Love",
          "wins.",
        ],
        [
          "first",
          "second",
        ],
        [
          "",
        ],
        [
          "",
        ],
        [
          "",
        ],
        [
          "999 chars",
        ],
      ]
    `);
  });
});

function requiredTemplate(templateId: string): MotionTemplateDefinition {
  const template = getMotionTemplate(templateId);
  if (!template) throw new Error(`Expected motion template ${templateId}.`);
  return template;
}
