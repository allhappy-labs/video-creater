import { describe, expect, it } from "vitest";
import { motionTemplateCatalog, type MotionTemplateDefinition } from "@/lib/motion-templates";
import { textStyleTemplates, titleTemplateGroups } from "./text-catalog";

const ids = (templates: readonly MotionTemplateDefinition[]) => templates.map((template) => template.id);

describe("text panel catalog", () => {
  it("lists type-led templates as text styles", () => {
    expect(ids(textStyleTemplates(motionTemplateCatalog))).toEqual(["punchy-caption-v1", "gradient-background-loop-v1"]);
  });

  it("groups the remaining overlay templates by category in title order", () => {
    expect(titleTemplateGroups(motionTemplateCatalog).map((group) => [group.label, ids(group.templates)])).toEqual([
      ["Titles", ["chapter-card-v1", "holographic-logo-cutout-v1"]],
      ["Lower thirds", ["kinetic-lower-third-v1"]],
      ["Callouts", ["metric-callout-v1", "tracking-highlight-v1"]],
    ]);
  });

  it("skips templates that can't be placed as overlays and shows every placeable template once", () => {
    const [base] = motionTemplateCatalog;
    if (!base) throw new Error("empty catalog");
    const transition: MotionTemplateDefinition = { ...base, id: "wipe", category: "transitions", kind: "transition" };
    const catalog = [...motionTemplateCatalog, transition];
    const shown = [...ids(textStyleTemplates(catalog)), ...titleTemplateGroups(catalog).flatMap((group) => ids(group.templates))];
    expect(shown).not.toContain("wipe");
    expect([...shown].sort()).toEqual(ids(motionTemplateCatalog).sort());
  });
});
