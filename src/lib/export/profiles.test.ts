import { describe, expect, it } from "vitest";
import type { ExportProfile, ExportProfileAvailability, ExportQuality } from "@/lib/project";
import {
  exportPolicyStatusLabel,
  exportProfileById,
  exportProfileDisabledReason,
  exportProfileRuntimeDetail,
  fallbackExportProfileAvailability,
  inProcessExportLabel,
  mediaExportOutputPath,
  defaultResolutionForQuality,
  dimensionsForResolution,
  draftExportDimensions,
  evenDimension,
  firstDraftVideoProfile,
  firstProfile,
  formatDurationTimecode,
  formatEstimatedSize,
  isCapabilityCheckedVideoProfile,
  profileQualityAvailability,
  resolutionOptions,
  type ExportDestination,
  type ExportResolution,
  type ExportSheetProfile,
  type InProcessExportProfile,
} from "@/lib/export/profiles";

const qualities: ExportQuality[] = ["draft", "final"];
const inProcessProfiles: InProcessExportProfile[] = ["webm", "mp4H264", "mp4H265", "proResMov"];
const resolutions: ExportResolution[] = ["match-timeline", "hd", "full-hd", "4k"];
const destinations: ExportDestination[] = ["video", "timeline", "palmier-project"];
const timelines = [
  { width: 1920, height: 1080 },
  { width: 1080, height: 1920 },
  { width: 1280, height: 720 },
  { width: 1001, height: 777 },
  { width: 0, height: 0 },
];
const seconds = [0, 0.04, 0.5, 1, 59.999, 60, 61.25, 3599.5, 3600, 7322.125, -1, Number.NaN];

const sheetProfiles: ExportSheetProfile[] = [
  { id: "mp4H264", destination: "video", label: "H.264", fileType: "MPEG-4 (.mp4)", available: false },
  {
    id: "webm",
    destination: "video",
    label: "WebM",
    fileType: "WebM (.webm)",
    available: true,
    qualityAvailability: { draft: true, final: false },
  },
  {
    id: "draftWebm",
    destination: "video",
    label: "Draft WebM",
    fileType: "WebM (.webm)",
    available: true,
    qualityAvailability: { draft: true, final: true },
  },
  {
    id: "proResMov",
    destination: "video",
    label: "ProRes",
    fileType: "QuickTime (.mov)",
    available: true,
    qualityAvailability: { draft: false, final: true },
  },
  { id: "premiereXmeml", destination: "timeline", label: "Premiere", fileType: "XML", available: false },
  { id: "davinciFcpxml", destination: "timeline", label: "DaVinci", fileType: "FCPXML", available: false },
];

describe("export profile characterization", () => {
  it("labels in-process export profiles", () => {
    expect(
      inProcessProfiles.flatMap((profile) => qualities.map((quality) => inProcessExportLabel(profile, quality))),
    ).toMatchInlineSnapshot(`
      [
        "Draft WebM",
        "Final WebM",
        "H.264 Draft",
        "H.264 Final",
        "H.265 (HEVC) Draft",
        "H.265 (HEVC) Final",
        "ProRes Draft",
        "ProRes Final",
      ]
    `);
  });

  it("labels Master exports by their encode tier", () => {
    expect(inProcessExportLabel("mp4H264", "final", "master")).toBe("H.264 Master");
    expect(inProcessExportLabel("mp4H265", "final", "master")).toBe("H.265 (HEVC) Master");
    expect(inProcessExportLabel("webm", "final", "master")).toBe("Master WebM");
    expect(inProcessExportLabel("mp4H264", "final", "standard")).toBe("H.264 Final");
  });

  it("defines fallback export profile availability", () => {
    expect(fallbackExportProfileAvailability).toMatchInlineSnapshot(`
      [
        {
          "audioCodec": "opus",
          "available": false,
          "container": "webm",
          "extension": "webm",
          "label": "Legacy WebM",
          "mimeType": "video/webm",
          "policyStatus": "missingRuntime",
          "profile": "webm",
          "qualityAvailability": {
            "draft": false,
            "final": false,
          },
          "qualityUnavailableReasons": {
            "draft": "Native export capabilities are being checked.",
            "final": "Native export capabilities are being checked.",
          },
          "requiredRuntime": [
            "gstreamer:webmmux",
            "gstreamer:vp8enc",
            "gstreamer:opusenc",
          ],
          "unavailableReason": "Native export capabilities are being checked.",
          "videoCodec": "vp8",
        },
        {
          "audioCodec": "aac",
          "available": false,
          "container": "mp4",
          "extension": "mp4",
          "label": "MP4 / H.264",
          "mimeType": "video/mp4",
          "policyStatus": "missingRuntime",
          "profile": "mp4H264",
          "qualityAvailability": {
            "draft": false,
            "final": false,
          },
          "qualityUnavailableReasons": {
            "draft": "Native export capabilities are being checked.",
            "final": "Native export capabilities are being checked.",
          },
          "requiredRuntime": [
            "gstreamer:mp4mux",
            "gstreamer:vtenc_h264",
            "gstreamer:atenc",
            "gstreamer:aacparse",
          ],
          "unavailableReason": "Native export capabilities are being checked.",
          "videoCodec": "h264",
        },
        {
          "audioCodec": "aac",
          "available": false,
          "container": "mp4",
          "extension": "mp4",
          "label": "MP4 / H.265",
          "mimeType": "video/mp4",
          "policyStatus": "missingRuntime",
          "profile": "mp4H265",
          "qualityAvailability": {
            "draft": false,
            "final": false,
          },
          "qualityUnavailableReasons": {
            "draft": "Native export capabilities are being checked.",
            "final": "Native export capabilities are being checked.",
          },
          "requiredRuntime": [
            "gstreamer:mp4mux",
            "gstreamer:vtenc_h265",
            "gstreamer:atenc",
            "gstreamer:aacparse",
          ],
          "unavailableReason": "Native export capabilities are being checked.",
          "videoCodec": "hevc",
        },
        {
          "audioCodec": "pcm",
          "available": false,
          "container": "mov",
          "extension": "mov",
          "label": "ProRes MOV",
          "mimeType": "video/quicktime",
          "policyStatus": "missingRuntime",
          "profile": "proResMov",
          "qualityAvailability": {
            "draft": false,
            "final": false,
          },
          "qualityUnavailableReasons": {
            "draft": "Native export capabilities are being checked.",
            "final": "Native export capabilities are being checked.",
          },
          "requiredRuntime": [
            "gstreamer:qtmux",
            "gstreamer:vtenc_prores",
          ],
          "unavailableReason": "Native export capabilities are being checked.",
          "videoCodec": "prores",
        },
      ]
    `);
  });

  it("describes disabled reasons for every fallback profile", () => {
    const variants = fallbackExportProfileAvailability.flatMap((profile): Array<[string, ExportProfileAvailability]> => [
      [`${profile.profile} fallback`, profile],
      [`${profile.profile} available`, { ...profile, available: true }],
      [`${profile.profile} blank reason`, { ...profile, unavailableReason: "   " }],
      [`${profile.profile} null reason`, { ...profile, unavailableReason: null }],
    ]);
    expect(
      variants.flatMap(([label, profile]) =>
        [true, false].flatMap((canExportMediaProfiles) =>
          [true, false].map((temporalExecution) => [
            label,
            canExportMediaProfiles,
            temporalExecution,
            exportProfileDisabledReason(profile, canExportMediaProfiles, temporalExecution),
          ]),
        ),
      ),
    ).toMatchInlineSnapshot(`
      [
        [
          "webm fallback",
          true,
          true,
          "Native export capabilities are being checked.",
        ],
        [
          "webm fallback",
          true,
          false,
          "Native export capabilities are being checked.",
        ],
        [
          "webm fallback",
          false,
          true,
          "Native export capabilities are being checked.",
        ],
        [
          "webm fallback",
          false,
          false,
          "Native export capabilities are being checked.",
        ],
        [
          "webm available",
          true,
          true,
          "Start Temporal export workflow",
        ],
        [
          "webm available",
          true,
          false,
          "Render in the desktop app",
        ],
        [
          "webm available",
          false,
          true,
          "Save as a schema-v2 split project to start Temporal export workflows.",
        ],
        [
          "webm available",
          false,
          false,
          "Save as a schema-v2 split project to export media.",
        ],
        [
          "webm blank reason",
          true,
          true,
          "Unavailable in this build",
        ],
        [
          "webm blank reason",
          true,
          false,
          "Unavailable in this build",
        ],
        [
          "webm blank reason",
          false,
          true,
          "Unavailable in this build",
        ],
        [
          "webm blank reason",
          false,
          false,
          "Unavailable in this build",
        ],
        [
          "webm null reason",
          true,
          true,
          "Unavailable in this build",
        ],
        [
          "webm null reason",
          true,
          false,
          "Unavailable in this build",
        ],
        [
          "webm null reason",
          false,
          true,
          "Unavailable in this build",
        ],
        [
          "webm null reason",
          false,
          false,
          "Unavailable in this build",
        ],
        [
          "mp4H264 fallback",
          true,
          true,
          "Native export capabilities are being checked.",
        ],
        [
          "mp4H264 fallback",
          true,
          false,
          "Native export capabilities are being checked.",
        ],
        [
          "mp4H264 fallback",
          false,
          true,
          "Native export capabilities are being checked.",
        ],
        [
          "mp4H264 fallback",
          false,
          false,
          "Native export capabilities are being checked.",
        ],
        [
          "mp4H264 available",
          true,
          true,
          "Start Temporal export workflow",
        ],
        [
          "mp4H264 available",
          true,
          false,
          "Render in the desktop app",
        ],
        [
          "mp4H264 available",
          false,
          true,
          "Save as a schema-v2 split project to start Temporal export workflows.",
        ],
        [
          "mp4H264 available",
          false,
          false,
          "Save as a schema-v2 split project to export media.",
        ],
        [
          "mp4H264 blank reason",
          true,
          true,
          "Unavailable in this build",
        ],
        [
          "mp4H264 blank reason",
          true,
          false,
          "Unavailable in this build",
        ],
        [
          "mp4H264 blank reason",
          false,
          true,
          "Unavailable in this build",
        ],
        [
          "mp4H264 blank reason",
          false,
          false,
          "Unavailable in this build",
        ],
        [
          "mp4H264 null reason",
          true,
          true,
          "Unavailable in this build",
        ],
        [
          "mp4H264 null reason",
          true,
          false,
          "Unavailable in this build",
        ],
        [
          "mp4H264 null reason",
          false,
          true,
          "Unavailable in this build",
        ],
        [
          "mp4H264 null reason",
          false,
          false,
          "Unavailable in this build",
        ],
        [
          "mp4H265 fallback",
          true,
          true,
          "Native export capabilities are being checked.",
        ],
        [
          "mp4H265 fallback",
          true,
          false,
          "Native export capabilities are being checked.",
        ],
        [
          "mp4H265 fallback",
          false,
          true,
          "Native export capabilities are being checked.",
        ],
        [
          "mp4H265 fallback",
          false,
          false,
          "Native export capabilities are being checked.",
        ],
        [
          "mp4H265 available",
          true,
          true,
          "Start Temporal export workflow",
        ],
        [
          "mp4H265 available",
          true,
          false,
          "Render in the desktop app",
        ],
        [
          "mp4H265 available",
          false,
          true,
          "Save as a schema-v2 split project to start Temporal export workflows.",
        ],
        [
          "mp4H265 available",
          false,
          false,
          "Save as a schema-v2 split project to export media.",
        ],
        [
          "mp4H265 blank reason",
          true,
          true,
          "Unavailable in this build",
        ],
        [
          "mp4H265 blank reason",
          true,
          false,
          "Unavailable in this build",
        ],
        [
          "mp4H265 blank reason",
          false,
          true,
          "Unavailable in this build",
        ],
        [
          "mp4H265 blank reason",
          false,
          false,
          "Unavailable in this build",
        ],
        [
          "mp4H265 null reason",
          true,
          true,
          "Unavailable in this build",
        ],
        [
          "mp4H265 null reason",
          true,
          false,
          "Unavailable in this build",
        ],
        [
          "mp4H265 null reason",
          false,
          true,
          "Unavailable in this build",
        ],
        [
          "mp4H265 null reason",
          false,
          false,
          "Unavailable in this build",
        ],
        [
          "proResMov fallback",
          true,
          true,
          "Native export capabilities are being checked.",
        ],
        [
          "proResMov fallback",
          true,
          false,
          "Native export capabilities are being checked.",
        ],
        [
          "proResMov fallback",
          false,
          true,
          "Native export capabilities are being checked.",
        ],
        [
          "proResMov fallback",
          false,
          false,
          "Native export capabilities are being checked.",
        ],
        [
          "proResMov available",
          true,
          true,
          "Start Temporal export workflow",
        ],
        [
          "proResMov available",
          true,
          false,
          "Render in the desktop app",
        ],
        [
          "proResMov available",
          false,
          true,
          "Save as a schema-v2 split project to start Temporal export workflows.",
        ],
        [
          "proResMov available",
          false,
          false,
          "Save as a schema-v2 split project to export media.",
        ],
        [
          "proResMov blank reason",
          true,
          true,
          "Unavailable in this build",
        ],
        [
          "proResMov blank reason",
          true,
          false,
          "Unavailable in this build",
        ],
        [
          "proResMov blank reason",
          false,
          true,
          "Unavailable in this build",
        ],
        [
          "proResMov blank reason",
          false,
          false,
          "Unavailable in this build",
        ],
        [
          "proResMov null reason",
          true,
          true,
          "Unavailable in this build",
        ],
        [
          "proResMov null reason",
          true,
          false,
          "Unavailable in this build",
        ],
        [
          "proResMov null reason",
          false,
          true,
          "Unavailable in this build",
        ],
        [
          "proResMov null reason",
          false,
          false,
          "Unavailable in this build",
        ],
      ]
    `);
  });

  it("labels policy statuses and runtime details", () => {
    const policyStatuses: ExportProfileAvailability["policyStatus"][] = [
      "approved",
      "missingRuntime",
      "policyGated",
      "unsupportedBuild",
    ];
    expect(policyStatuses.map((status) => exportPolicyStatusLabel(status))).toMatchInlineSnapshot(`
      [
        "Approved",
        "Missing runtime",
        "Policy gated",
        "Unsupported build",
      ]
    `);
    expect(
      fallbackExportProfileAvailability.flatMap((profile) => [
        exportProfileRuntimeDetail(profile),
        exportProfileRuntimeDetail({ ...profile, available: true }),
        exportProfileRuntimeDetail({ ...profile, requiredRuntime: [] }),
        exportProfileRuntimeDetail({ ...profile, policyStatus: "policyGated" }),
      ]),
    ).toMatchInlineSnapshot(`
      [
        "Missing runtime · Requires gstreamer:webmmux, gstreamer:vp8enc, gstreamer:opusenc",
        null,
        null,
        "Policy gated · Requires gstreamer:webmmux, gstreamer:vp8enc, gstreamer:opusenc",
        "Missing runtime · Requires gstreamer:mp4mux, gstreamer:vtenc_h264, gstreamer:atenc, gstreamer:aacparse",
        null,
        null,
        "Policy gated · Requires gstreamer:mp4mux, gstreamer:vtenc_h264, gstreamer:atenc, gstreamer:aacparse",
        "Missing runtime · Requires gstreamer:mp4mux, gstreamer:vtenc_h265, gstreamer:atenc, gstreamer:aacparse",
        null,
        null,
        "Policy gated · Requires gstreamer:mp4mux, gstreamer:vtenc_h265, gstreamer:atenc, gstreamer:aacparse",
        "Missing runtime · Requires gstreamer:qtmux, gstreamer:vtenc_prores",
        null,
        null,
        "Policy gated · Requires gstreamer:qtmux, gstreamer:vtenc_prores",
      ]
    `);
  });

  it("resolves export profiles by id with fallbacks", () => {
    const custom = fallbackExportProfileAvailability.map((profile) => ({
      ...profile,
      label: `Custom ${profile.label}`,
      available: true,
    }));
    const profiles: ExportProfile[] = ["webm", "mp4H264", "mp4H265", "proResMov", "palmierProject"];
    expect(
      profiles.map((profile) => [
        profile,
        exportProfileById([], profile).label,
        exportProfileById(custom, profile).label,
        exportProfileById(custom.slice(2), profile).label,
      ]),
    ).toMatchInlineSnapshot(`
      [
        [
          "webm",
          "Legacy WebM",
          "Custom Legacy WebM",
          "Legacy WebM",
        ],
        [
          "mp4H264",
          "MP4 / H.264",
          "Custom MP4 / H.264",
          "MP4 / H.264",
        ],
        [
          "mp4H265",
          "MP4 / H.265",
          "Custom MP4 / H.265",
          "Custom MP4 / H.265",
        ],
        [
          "proResMov",
          "ProRes MOV",
          "Custom ProRes MOV",
          "Custom ProRes MOV",
        ],
        [
          "palmierProject",
          "Legacy WebM",
          "Legacy WebM",
          "Legacy WebM",
        ],
      ]
    `);
    expect(exportProfileById([], "webm")).toBe(fallbackExportProfileAvailability[0]);
  });

  it("builds media export output paths", () => {
    expect([
      mediaExportOutputPath("project-sample", "mp4H264", "mp4", "export-mp4H264-abc"),
      mediaExportOutputPath("", "webm", "", ""),
      mediaExportOutputPath("Project Sample", "proResMov", "mov", "job/1"),
    ]).toMatchInlineSnapshot(`
      [
        "exports/project-sample-mp4H264-export-mp4H264-abc.mp4",
        "exports/-webm-.",
        "exports/Project Sample-proResMov-job/1.mov",
      ]
    `);
  });
});

describe("export sheet profile characterization", () => {
  it("lists resolution options", () => {
    expect(resolutionOptions).toMatchInlineSnapshot(`
      [
        {
          "id": "match-timeline",
          "label": "Match Timeline",
        },
        {
          "height": 720,
          "id": "hd",
          "label": "HD (up to 1280×720)",
          "width": 1280,
        },
        {
          "height": 1080,
          "id": "full-hd",
          "label": "Full HD (1920×1080)",
          "width": 1920,
        },
        {
          "height": 2160,
          "id": "4k",
          "label": "4K (3840×2160)",
          "width": 3840,
        },
      ]
    `);
  });

  it("chooses default resolutions per quality", () => {
    expect(
      qualities.flatMap((quality) =>
        timelines.flatMap((timeline) =>
          [false, true].map((overridden) => [
            quality,
            `${timeline.width}x${timeline.height}`,
            overridden,
            defaultResolutionForQuality(quality, timeline, overridden, "4k"),
          ]),
        ),
      ),
    ).toMatchInlineSnapshot(`
      [
        [
          "draft",
          "1920x1080",
          false,
          "hd",
        ],
        [
          "draft",
          "1920x1080",
          true,
          "4k",
        ],
        [
          "draft",
          "1080x1920",
          false,
          "hd",
        ],
        [
          "draft",
          "1080x1920",
          true,
          "4k",
        ],
        [
          "draft",
          "1280x720",
          false,
          "match-timeline",
        ],
        [
          "draft",
          "1280x720",
          true,
          "4k",
        ],
        [
          "draft",
          "1001x777",
          false,
          "hd",
        ],
        [
          "draft",
          "1001x777",
          true,
          "4k",
        ],
        [
          "draft",
          "0x0",
          false,
          "match-timeline",
        ],
        [
          "draft",
          "0x0",
          true,
          "4k",
        ],
        [
          "final",
          "1920x1080",
          false,
          "match-timeline",
        ],
        [
          "final",
          "1920x1080",
          true,
          "4k",
        ],
        [
          "final",
          "1080x1920",
          false,
          "match-timeline",
        ],
        [
          "final",
          "1080x1920",
          true,
          "4k",
        ],
        [
          "final",
          "1280x720",
          false,
          "match-timeline",
        ],
        [
          "final",
          "1280x720",
          true,
          "4k",
        ],
        [
          "final",
          "1001x777",
          false,
          "match-timeline",
        ],
        [
          "final",
          "1001x777",
          true,
          "4k",
        ],
        [
          "final",
          "0x0",
          false,
          "match-timeline",
        ],
        [
          "final",
          "0x0",
          true,
          "4k",
        ],
      ]
    `);
  });

  it("derives export dimensions", () => {
    expect(
      [
        [1920, 1080],
        [1080, 1920],
        [1001, 777],
        [0, 0],
      ].map(([width, height]) => draftExportDimensions(width!, height!)),
    ).toMatchInlineSnapshot(`
      [
        {
          "height": 720,
          "width": 1280,
        },
        {
          "height": 720,
          "width": 404,
        },
        {
          "height": 720,
          "width": 928,
        },
        {
          "height": 0,
          "width": 0,
        },
      ]
    `);
    expect(
      resolutions.map((resolution) => [
        resolution,
        timelines.map((timeline) => dimensionsForResolution(resolution, timeline)),
      ]),
    ).toMatchInlineSnapshot(`
      [
        [
          "match-timeline",
          [
            {
              "height": 1080,
              "width": 1920,
            },
            {
              "height": 1920,
              "width": 1080,
            },
            {
              "height": 720,
              "width": 1280,
            },
            {
              "height": 776,
              "width": 1000,
            },
            {
              "height": 0,
              "width": 0,
            },
          ],
        ],
        [
          "hd",
          [
            {
              "height": 720,
              "width": 1280,
            },
            {
              "height": 720,
              "width": 404,
            },
            {
              "height": 720,
              "width": 1280,
            },
            {
              "height": 720,
              "width": 928,
            },
            {
              "height": 0,
              "width": 0,
            },
          ],
        ],
        [
          "full-hd",
          [
            {
              "height": 1080,
              "width": 1920,
            },
            {
              "height": 1080,
              "width": 1920,
            },
            {
              "height": 1080,
              "width": 1920,
            },
            {
              "height": 1080,
              "width": 1920,
            },
            {
              "height": 1080,
              "width": 1920,
            },
          ],
        ],
        [
          "4k",
          [
            {
              "height": 2160,
              "width": 3840,
            },
            {
              "height": 2160,
              "width": 3840,
            },
            {
              "height": 2160,
              "width": 3840,
            },
            {
              "height": 2160,
              "width": 3840,
            },
            {
              "height": 2160,
              "width": 3840,
            },
          ],
        ],
      ]
    `);
    expect([0, 1, 2, 3.9, 1001, -3, Number.NaN].map((value) => evenDimension(value))).toMatchInlineSnapshot(`
      [
        0,
        0,
        2,
        2,
        1000,
        0,
        NaN,
      ]
    `);
  });

  it("reads profile quality availability and capability checks", () => {
    const candidates: Array<ExportSheetProfile | null> = [null, ...sheetProfiles];
    expect(
      candidates.map((profile) => [
        profile?.id ?? null,
        profileQualityAvailability(profile),
        isCapabilityCheckedVideoProfile(profile),
      ]),
    ).toMatchInlineSnapshot(`
      [
        [
          null,
          {
            "draft": false,
            "final": false,
          },
          false,
        ],
        [
          "mp4H264",
          {
            "draft": false,
            "final": false,
          },
          true,
        ],
        [
          "webm",
          {
            "draft": true,
            "final": false,
          },
          true,
        ],
        [
          "draftWebm",
          {
            "draft": true,
            "final": true,
          },
          true,
        ],
        [
          "proResMov",
          {
            "draft": false,
            "final": true,
          },
          true,
        ],
        [
          "premiereXmeml",
          {
            "draft": false,
            "final": false,
          },
          false,
        ],
        [
          "davinciFcpxml",
          {
            "draft": false,
            "final": false,
          },
          false,
        ],
      ]
    `);
  });

  it("selects first profiles per destination and first draft video profile", () => {
    expect(destinations.map((destination) => firstProfile(sheetProfiles, destination)?.id ?? null)).toMatchInlineSnapshot(`
      [
        "webm",
        "premiereXmeml",
        null,
      ]
    `);
    expect([
      firstDraftVideoProfile(sheetProfiles)?.id ?? null,
      firstDraftVideoProfile(sheetProfiles.filter((profile) => profile.id !== "draftWebm"))?.id ?? null,
      firstDraftVideoProfile(sheetProfiles.filter((profile) => profile.id !== "draftWebm" && profile.id !== "webm"))
        ?.id ?? null,
      firstDraftVideoProfile([
        { id: "mp4H265", destination: "video", label: "H.265", fileType: "MPEG-4 (.mp4)", available: true },
      ])?.id ?? null,
      firstDraftVideoProfile([])?.id ?? null,
    ]).toMatchInlineSnapshot(`
      [
        "draftWebm",
        "webm",
        null,
        "mp4H265",
        null,
      ]
    `);
  });

  it("formats duration timecodes and estimated sizes", () => {
    expect(
      seconds.map((value) => [value, formatDurationTimecode(value, 24), formatDurationTimecode(value, 29.97), formatDurationTimecode(value, 0)]),
    ).toMatchInlineSnapshot(`
      [
        [
          0,
          "00:00:00:00",
          "00:00:00:00",
          "00:00:00:00",
        ],
        [
          0.04,
          "00:00:00:00",
          "00:00:00:01",
          "00:00:00:00",
        ],
        [
          0.5,
          "00:00:00:12",
          "00:00:00:15",
          "00:00:00:00",
        ],
        [
          1,
          "00:00:01:00",
          "00:00:01:00",
          "00:00:01:00",
        ],
        [
          59.999,
          "00:00:59:23",
          "00:00:59:29",
          "00:00:59:00",
        ],
        [
          60,
          "00:01:00:00",
          "00:01:00:00",
          "00:01:00:00",
        ],
        [
          61.25,
          "00:01:01:06",
          "00:01:01:07",
          "00:01:01:00",
        ],
        [
          3599.5,
          "00:59:59:12",
          "00:59:59:15",
          "00:59:59:00",
        ],
        [
          3600,
          "01:00:00:00",
          "01:00:00:00",
          "01:00:00:00",
        ],
        [
          7322.125,
          "02:02:02:03",
          "02:02:02:03",
          "02:02:02:00",
        ],
        [
          -1,
          "00:00:00:00",
          "00:00:00:00",
          "00:00:00:00",
        ],
        [
          NaN,
          "NaN:NaN:NaN:NaN",
          "NaN:NaN:NaN:NaN",
          "NaN:NaN:NaN:NaN",
        ],
      ]
    `);
    expect(
      [0, 999, 1000, 1023, 1024 * 1024, 1.5 * 1024 * 1024 * 1024, Number.NaN, -1, null, undefined, Number.POSITIVE_INFINITY].map(
        (bytes) => [bytes, formatEstimatedSize(bytes)],
      ),
    ).toMatchInlineSnapshot(`
      [
        [
          0,
          "0 B",
        ],
        [
          999,
          "999 B",
        ],
        [
          1000,
          "1.0 KB",
        ],
        [
          1023,
          "1.0 KB",
        ],
        [
          1048576,
          "1.0 MB",
        ],
        [
          1610612736,
          "1.6 GB",
        ],
        [
          NaN,
          "—",
        ],
        [
          -1,
          "—",
        ],
        [
          null,
          "—",
        ],
        [
          undefined,
          "—",
        ],
        [
          Infinity,
          "—",
        ],
      ]
    `);
  });
});
