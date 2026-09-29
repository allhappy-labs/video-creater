import { describe, expect, it } from "vitest";
import type { ProjectRenderReport } from "@/lib/project";
import { buildSampleRenderReport, type RenderReport } from "@/lib/render";
import {
  artifactBackedFrameCount,
  deliveryQualityLabel,
  formatMismatchRatio,
  formatTimelineSeconds,
  normalizeArtifactPath,
  qaMetricEntries,
  renderCheckEntries,
  renderCheckLabel,
} from "@/lib/export/render-report";

const timelineSeconds = [0, 0.004, 0.005, 0.5, 1, 1.25, 1.2345, 10, 10.1, 59.999, 100, -1.5, Number.NaN];
const mismatchRatios = [0, 0.00001, 0.0123456, 0.5, 1, 1.5, -0.25, Number.NaN];

function mismatchedReport(): RenderReport {
  const report = buildSampleRenderReport("finalWebm");
  return {
    ...report,
    summary: { ...report.summary, status: "failed", quality: "final", container: " MP4 " },
    artifacts: [
      ...report.artifacts,
      "renders\\graphics\\shader-hook-bg\\frames\\frame-000012.png",
      "renders/preview/preview-comparison.json",
    ],
    previewComparison: {
      status: "failed",
      comparedFrames: [
        {
          timelineSeconds: 0,
          previewFrame: "preview/frame-0.png",
          renderedFrame: "rendered/frame-0.png",
          diffFrame: null,
          mismatchRatio: 0,
          passed: true,
        },
        {
          timelineSeconds: 1.5,
          previewFrame: "preview/frame-36.png",
          renderedFrame: "rendered/frame-36.png",
          diffFrame: "diff/frame-36.png",
          mismatchRatio: 0.083333,
          passed: false,
        },
      ],
    },
  };
}

function projectRenderReport(checks: ProjectRenderReport["checks"]): ProjectRenderReport {
  return {
    schemaVersion: 1,
    id: "report-1",
    status: "completed",
    outputPath: "renders/report-1/output.webm",
    durationSeconds: 4,
    streams: { video: true, audio: true },
    checks,
    artifacts: [],
    logPath: "renders/report-1/render.log",
    createdAt: "2026-09-13T00:00:00Z",
  };
}

describe("render report characterization", () => {
  it("labels delivery quality", () => {
    const summaries: Array<{ quality?: "draft" | "final" | null; container?: string | null }> = [
      {},
      { quality: "draft" },
      { container: "mp4" },
      { quality: "draft", container: "mp4" },
      { quality: "final", container: " MoV " },
      { quality: "final", container: "WEBM" },
      { quality: "final", container: "   " },
      { quality: null, container: "mp4" },
    ];
    expect(summaries.map((summary) => deliveryQualityLabel(summary))).toMatchInlineSnapshot(`
      [
        null,
        null,
        null,
        "Draft MP4",
        "Final MOV",
        null,
        null,
        null,
      ]
    `);
  });

  it("sorts QA metrics and render checks", () => {
    expect(qaMetricEntries({ zeta: "1", alpha: "2", Beta: "3", "10": "4", "": "5" })).toMatchInlineSnapshot(`
      [
        [
          "",
          "5",
        ],
        [
          "10",
          "4",
        ],
        [
          "alpha",
          "2",
        ],
        [
          "Beta",
          "3",
        ],
        [
          "zeta",
          "1",
        ],
      ]
    `);
    const report = projectRenderReport({
      streamsPresent: "passed",
      playable: "failed",
      caption_alignment: "passed",
      "overlay-timing": "skipped",
      HDRCheck: "passed",
    });
    expect(renderCheckEntries(report)).toMatchInlineSnapshot(`
      [
        [
          "caption_alignment",
          "passed",
        ],
        [
          "HDRCheck",
          "passed",
        ],
        [
          "overlay-timing",
          "skipped",
        ],
        [
          "playable",
          "failed",
        ],
        [
          "streamsPresent",
          "passed",
        ],
      ]
    `);
    expect(
      ["streamsPresent", "caption_alignment", "overlay-timing", "HDRCheck", "  spaced__Name-- ", "video2Audio", ""].map(
        (check) => renderCheckLabel(check),
      ),
    ).toMatchInlineSnapshot(`
      [
        "streams present",
        "caption alignment",
        "overlay timing",
        "hdrcheck",
        "spaced name",
        "video2 audio",
        "",
      ]
    `);
  });

  it("normalizes artifact paths and counts artifact-backed frames", () => {
    expect(
      ["renders/a.png", "renders\\graphics\\a.png", "C:\\\\renders\\a.png", ""].map((path) => normalizeArtifactPath(path)),
    ).toMatchInlineSnapshot(`
      [
        "renders/a.png",
        "renders/graphics/a.png",
        "C://renders/a.png",
        "",
      ]
    `);
    expect([
      artifactBackedFrameCount([], ["frames/a.png"]),
      artifactBackedFrameCount(["frames/a.png"], []),
      artifactBackedFrameCount(["frames/a.png", "frames/b.png"], ["renders/frames/a.png", "frames/b.png"]),
      artifactBackedFrameCount(["frames\\a.png"], ["renders\\frames\\a.png"]),
      artifactBackedFrameCount(["a.png"], ["renders/frames/xa.png"]),
      artifactBackedFrameCount(["frames/a.png", "frames/a.png"], ["frames/a.png"]),
    ]).toMatchInlineSnapshot(`
      [
        0,
        0,
        2,
        1,
        0,
        2,
      ]
    `);
  });

  it("formats timeline seconds and mismatch ratios", () => {
    expect(timelineSeconds.map((value) => [value, formatTimelineSeconds(value)])).toMatchInlineSnapshot(`
      [
        [
          0,
          "0s",
        ],
        [
          0.004,
          "0s",
        ],
        [
          0.005,
          "0.01s",
        ],
        [
          0.5,
          "0.5s",
        ],
        [
          1,
          "1s",
        ],
        [
          1.25,
          "1.25s",
        ],
        [
          1.2345,
          "1.23s",
        ],
        [
          10,
          "10s",
        ],
        [
          10.1,
          "10.1s",
        ],
        [
          59.999,
          "60s",
        ],
        [
          100,
          "100s",
        ],
        [
          -1.5,
          "-1.5s",
        ],
        [
          NaN,
          "NaNs",
        ],
      ]
    `);
    expect(mismatchRatios.map((value) => [value, formatMismatchRatio(value)])).toMatchInlineSnapshot(`
      [
        [
          0,
          "0.00% diff",
        ],
        [
          0.00001,
          "0.00% diff",
        ],
        [
          0.0123456,
          "1.23% diff",
        ],
        [
          0.5,
          "50.00% diff",
        ],
        [
          1,
          "100.00% diff",
        ],
        [
          1.5,
          "150.00% diff",
        ],
        [
          -0.25,
          "-25.00% diff",
        ],
        [
          NaN,
          "NaN% diff",
        ],
      ]
    `);
  });

  it("summarizes no report, a passing report, and a report with frame mismatches", () => {
    const payloads: Array<[string, RenderReport | null]> = [
      ["no report", null],
      ["passing", buildSampleRenderReport("draftWebm")],
      ["mismatched", mismatchedReport()],
    ];
    expect(
      payloads.map(([label, report]) => ({
        label,
        deliveryQuality: report ? deliveryQualityLabel(report.summary) : null,
        previewEvidence: report
          ? report.artifacts.find((artifact) => normalizeArtifactPath(artifact).endsWith("/preview-comparison.json")) ??
            null
          : null,
        graphics: report
          ? report.graphics.map((graphic) => ({
              layerId: graphic.layerId,
              backedFrames: artifactBackedFrameCount(graphic.sampledFrames, report.artifacts),
              sampledFrames: graphic.sampledFrames.length,
              metrics: qaMetricEntries(graphic.qaMetrics),
            }))
          : [],
        frames: (report?.previewComparison?.comparedFrames ?? []).map((frame) => [
          formatTimelineSeconds(frame.timelineSeconds),
          formatMismatchRatio(frame.mismatchRatio),
          frame.passed,
        ]),
      })),
    ).toMatchInlineSnapshot(`
      [
        {
          "deliveryQuality": null,
          "frames": [],
          "graphics": [],
          "label": "no report",
          "previewEvidence": null,
        },
        {
          "deliveryQuality": null,
          "frames": [],
          "graphics": [
            {
              "backedFrames": 1,
              "layerId": "shader-hook-bg",
              "metrics": [
                [
                  "edgeRatio",
                  "0.004000",
                ],
                [
                  "opaqueRatio",
                  "0.320000",
                ],
                [
                  "saturatedRatio",
                  "0.120000",
                ],
                [
                  "temporalDelta",
                  "0.006500",
                ],
              ],
              "sampledFrames": 3,
            },
          ],
          "label": "passing",
          "previewEvidence": null,
        },
        {
          "deliveryQuality": "Final MP4",
          "frames": [
            [
              "0s",
              "0.00% diff",
              true,
            ],
            [
              "1.5s",
              "8.33% diff",
              false,
            ],
          ],
          "graphics": [
            {
              "backedFrames": 2,
              "layerId": "shader-hook-bg",
              "metrics": [
                [
                  "edgeRatio",
                  "0.004000",
                ],
                [
                  "opaqueRatio",
                  "0.320000",
                ],
                [
                  "saturatedRatio",
                  "0.120000",
                ],
                [
                  "temporalDelta",
                  "0.006500",
                ],
              ],
              "sampledFrames": 3,
            },
          ],
          "label": "mismatched",
          "previewEvidence": "renders/preview/preview-comparison.json",
        },
      ]
    `);
  });
});
