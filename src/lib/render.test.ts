import { describe, expect, it } from "vitest";
import {
  buildSampleRenderReport,
  commandQualityProfile,
  renderQualityOptions,
  renderQualityProfileLabel,
  type RenderQualityProfile,
  type RenderReport,
} from "./render";

describe("render model", () => {
  it("uses exact Rust render quality wire values", () => {
    const values = renderQualityOptions.map((option) => option.value);

    expect(values).toEqual<RenderQualityProfile[]>(["draftWebm", "finalWebm"]);
  });

  it("labels render quality profiles for editor controls", () => {
    expect(renderQualityProfileLabel("draftWebm")).toBe("Draft WebM");
    expect(renderQualityProfileLabel("finalWebm")).toBe("Final WebM");
  });

  it("extracts command quality metadata from render args", () => {
    expect(
      commandQualityProfile({
        program: "gstreamer-ges",
        args: ["--input=source.mp4", "--quality=finalWebm"],
      }),
    ).toBe("finalWebm");
  });

  it("builds a sample report with the requested final quality command", () => {
    const report = buildSampleRenderReport("finalWebm");

    expect(report.command.args).toContain("--quality=finalWebm");
    expect(report.summary.outputPath).toBe("renders/final.webm");
    expect(report.stdout).toBe("");
    expect(report.stderr).toBe("");
    expect(report.errors).toEqual([]);
    expect(report.graphics[0]?.renderer).toBe("gpu");
    expect(report.graphics[0]?.templateId).toBeNull();
    expect(report.graphics[0]?.motionPresetId).toBeNull();
    expect(report.graphics[0]?.visualQaStatus).toBe("passed");
    expect(report.graphics[0]?.sampledFrames).toHaveLength(3);
    expect(report.graphics[0]?.qaMetrics.temporalDelta).toBe("0.006500");
  });

  it("accepts optional render performance metadata", () => {
    const report: RenderReport = {
      jobId: "render-proposal",
      summary: {
        status: "succeeded",
        durationSeconds: 5.6,
        outputPath: "renders/draft.webm",
      },
      command: {
        program: "gstreamer-ges",
        args: ["--quality=draftWebm"],
      },
      stdout: "",
      stderr: "",
      errors: [],
      artifacts: [],
      graphics: [
        {
          layerId: "proposal-caption-1",
          renderer: "rust",
          qualityProfile: null,
          templateId: null,
          motionPresetId: null,
          visualQaStatus: "passed",
          cacheStatus: "hit",
          sampledFrames: ["frames/frame-000000.png"],
          qaMetrics: {
            visibleAlphaRatio: "0.041000",
            temporalDelta: "0.002000",
          },
        },
      ],
      performance: {
        totalDurationMs: 125,
        stages: [
          {
            name: "graphics",
            status: "succeeded",
            durationMs: 40,
            details: { cache: "hit" },
          },
        ],
      },
    };

    expect(report.performance?.stages[0]?.details.cache).toBe("hit");
    expect(report.graphics[0]?.cacheStatus).toBe("hit");
    expect(report.graphics[0]?.qaMetrics.visibleAlphaRatio).toBe("0.041000");
  });
});
