import "@testing-library/jest-dom/vitest";
import { act, fireEvent, screen, within } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { createTemplateOverlayItem, kineticLowerThirdTemplate } from "@/lib/motion-templates";
import { defaultTemplateStyle } from "@/lib/templates/template-item";
import { projectWithTracks, renderProperties, track } from "./properties-test-utils";

vi.mock("@/lib/runtime/backend-client", () => ({ backendRequest: vi.fn(), backendListen: vi.fn(), backendMediaUrl: (p: string) => p }));
vi.mock("./use-effect-catalog", () => ({ useEffectCatalog: () => ({ status: "ready", effects: [] }) }));

const blur = { effectInstanceId: "blur-1", effectType: "blur.gaussian", enabled: true, params: { radius: 4 } };

function templateProject(properties: Record<string, unknown> = {}) {
  const item = createTemplateOverlayItem({ templateId: kineticLowerThirdTemplate.id, itemId: "lower", startSeconds: 1 });
  return projectWithTracks([track("overlays", "overlay", [{ ...item, properties: { ...item.properties, ...properties } }])]);
}

async function typeAndBlur(label: string, value: string) {
  const field = screen.getByRole("textbox", { name: label });
  await act(async () => {
    fireEvent.change(field, { target: { value } });
    fireEvent.blur(field);
  });
  return field;
}

const guidance = {
  visualTreatment: kineticLowerThirdTemplate.visualTreatment,
  motion: kineticLowerThirdTemplate.motion,
  safeZone: kineticLowerThirdTemplate.safeZone,
  avoid: kineticLowerThirdTemplate.avoid,
};

describe("template property tabs", () => {
  beforeEach(() => window.localStorage.clear());

  it("commits a field edit through updateTemplateItems with timing unchanged", async () => {
    const { applyActions } = await renderProperties(templateProject(), ["lower"]);
    expect(screen.getByRole("textbox", { name: "Headline" })).toHaveValue("Name / Role");
    await typeAndBlur("Headline", "  Thomas Edison ");
    expect(applyActions).toHaveBeenCalledTimes(1);
    expect(applyActions).toHaveBeenCalledWith([
      {
        type: "updateTemplateItems",
        updates: [
          { itemId: "lower", startSeconds: 1, durationSeconds: 2.4, templateFields: { headline: "Thomas Edison", subline: "Context label" } },
        ],
      },
    ]);
  });

  it("rejects an empty required field inline without committing", async () => {
    const { applyActions } = await renderProperties(templateProject(), ["lower"]);
    const field = await typeAndBlur("Subline", "  ");
    expect(applyActions).not.toHaveBeenCalled();
    expect(screen.getByRole("alert")).toHaveTextContent("Subline can't be empty.");
    expect(field).toHaveAttribute("aria-invalid", "true");
    expect(field).toBeRequired();
  });

  it("overrides a style color and guidance through updateTemplateOverride", async () => {
    const { applyActions } = await renderProperties(templateProject(), ["lower"], "Style");
    await typeAndBlur("Accent color", "#ff5c7a");
    const override = {
      templateId: kineticLowerThirdTemplate.id,
      name: kineticLowerThirdTemplate.name,
      fields: { headline: "Name / Role", subline: "Context label" },
      style: { ...defaultTemplateStyle, accentColor: "#ff5c7a" },
      ...guidance,
    };
    expect(applyActions).toHaveBeenLastCalledWith([{ type: "updateTemplateOverride", override }]);

    await typeAndBlur("Motion", "quick pop in");
    expect(applyActions).toHaveBeenLastCalledWith([
      { type: "updateTemplateOverride", override: { ...override, style: defaultTemplateStyle, motion: "quick pop in" } },
    ]);

    await typeAndBlur("Text color", "");
    expect(screen.getByRole("alert")).toHaveTextContent("Text color can't be empty.");
    expect(applyActions).toHaveBeenCalledTimes(2);
  });

  it("sets the motion preset from the animation tab", async () => {
    const { applyActions } = await renderProperties(templateProject(), ["lower"], "Animation");
    const grid = screen.getByRole("group", { name: "Template motion" });
    expect(within(grid).getByRole("button", { name: "Slide Fade Up" })).toHaveAttribute("aria-pressed", "true");
    fireEvent.click(within(grid).getByRole("button", { name: "Snap Pop" }));
    expect(applyActions).toHaveBeenCalledWith([
      { type: "updateItemProperties", updates: [{ itemId: "lower", set: { motionPresetId: "snap-pop-v1" }, remove: [] }] },
    ]);
  });

  it("lists and removes applied effects with the shared effects section", async () => {
    const { applyActions } = await renderProperties(templateProject({ effects: [blur] }), ["lower"], "Effects");
    const effects = screen.getByRole("list", { name: "Applied effects" });
    fireEvent.click(within(effects).getByRole("button", { name: "Remove blur.gaussian" }));
    expect(applyActions).toHaveBeenCalledWith([{ type: "updateItemEffects", itemIds: ["lower"], effects: [] }]);
  });
});
