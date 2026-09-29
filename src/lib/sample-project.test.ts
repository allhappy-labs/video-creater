import { describe, expect, it } from "vitest";
import {
  createSampleProject,
  sampleProjectBrowserPreviewUrl,
  sampleProjectBrowserDir,
} from "./sample-project";

describe("bundled sample project", () => {
  it("describes real bundled media with truthful local generation provenance", () => {
    const project = createSampleProject();

    expect(project.id).toBe("project-sample");
    expect(project.media.map((asset) => asset.relativePath)).toEqual([
      "media/input.mp4",
      "media/voiceover.m4a",
      "sample/generated/product-reveal.mp4",
    ]);
    expect(project.media.every((asset) => asset.durationSeconds === 4)).toBe(true);
    expect(project.generatedAssets).toEqual([
      expect.objectContaining({
        status: "completed",
        model: { provider: "local", id: "bundled-edison-restoration" },
      }),
    ]);
  });

  it("maps only safe bundled sample media paths to browser assets", () => {
    expect(sampleProjectBrowserPreviewUrl(sampleProjectBrowserDir, "media/input.mp4")).toBe(
      "/media/input.mp4",
    );
    expect(sampleProjectBrowserPreviewUrl(sampleProjectBrowserDir, "../private.mov")).toBeNull();
    expect(sampleProjectBrowserPreviewUrl("/projects/real", "media/input.mp4")).toBeNull();
  });
});
