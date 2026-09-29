import { describe, expect, it } from "vitest";
import type { VideoProject, VisualEffectCatalog } from "@/lib/project";
import { createSampleProject } from "@/lib/sample-project";
import { timelineSilenceRippleRanges } from "@/lib/timeline-ops/silence";
import { panelFixtureOperations } from "./panel-fixtures";

async function request<Result>(operation: string, input: Record<string, unknown> = {}): Promise<Result> {
  const handler = panelFixtureOperations().get(operation);
  if (!handler) throw new Error(`no panel fixture handler for ${operation}`);
  return (await handler(input)) as Result;
}

describe("panel fixture operations", () => {
  it("lists a deterministic catalog of non-color effects", async () => {
    const first = await request<VisualEffectCatalog>("list_visual_effect_catalog");
    const second = await request<VisualEffectCatalog>("list_visual_effect_catalog");
    expect(second).toEqual(first);
    expect(first.effects.map((effect) => effect.displayName)).toEqual(["Gaussian Blur", "Film Grain", "Vignette"]);
    expect(first.effectCount).toBe(3);
    expect(first.canonicalOrder).toEqual(first.effects.map((effect) => effect.id));
    expect(first.effects.every((effect) => !effect.colorEffect && effect.params.length > 0)).toBe(true);
  });

  it("commits the sample project with its detected silences as whole-length ripple ranges", async () => {
    const sample = createSampleProject();
    const result = await request<{ project: VideoProject }>("save_split_project_to_folder", { projectDir: "/tmp/sample", project: sample, expectedRevision: 0 });
    expect(result.project.timeline).toEqual(sample.timeline);
    expect(timelineSilenceRippleRanges(result.project)).toEqual([
      expect.objectContaining({ startSeconds: 1, endSeconds: 2 }),
      expect.objectContaining({ startSeconds: 2.5, endSeconds: 3.3 }),
    ]);
  });

  it("commits other projects unchanged and keeps silences the sample already has", async () => {
    const other = { ...createSampleProject(), id: "project-other" };
    expect((await request<{ project: VideoProject }>("save_split_project_to_folder", { project: other })).project).toEqual(other);

    const analyzed = { ...createSampleProject(), mediaSilenceRanges: [{ mediaId: "media-1", sourceIn: 0, sourceOut: 1, confidence: 1 }] };
    expect((await request<{ project: VideoProject }>("save_split_project_to_folder", { project: analyzed })).project).toEqual(analyzed);
  });
});
