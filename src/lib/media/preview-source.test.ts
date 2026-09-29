import { describe, expect, it, vi } from "vitest";
import type { GeneratedAsset, VideoProject } from "@/lib/project";
import { sampleProjectBrowserDir, sampleProjectDir } from "@/lib/sample-project";
import type { TimelineItem } from "@/lib/timeline";
import { fixtureGeneratedAsset, fixtureItem, fixtureProject } from "@/test-utils/editor-fixtures";
import { formatSecondsShort as formatViewerSeconds } from "@/lib/format";
import {
  formatPreviewCurrentTime,
  formatViewerDuration,
  mediaElementDurationSeconds,
  parsePreviewDurationLabel,
  previewTransportData,
  previewUrlForMedia,
  previewUrlsForMedia,
  previewUrlsForTimelineSources,
  safeProjectMediaPath,
  selectedPreviewSource,
  sourcePreviewStepSeconds,
  sourceRangeLabel,
  sourceRangeLabelForItem,
  sourceViewerTabId,
  type PreviewSource,
} from "@/lib/media/preview-source";

vi.mock("@/lib/runtime/backend-client", () => ({
  backendMediaUrl: (path: string) => {
    if (path.includes("backend-throws")) {
      throw new Error("media transport unavailable");
    }
    return `media://${path}`;
  },
}));

const unsafePaths = [
  "media/a.mp4",
  "/etc/passwd",
  "../x.mp4",
  "file:///x",
  "https://x",
  "media/../x.mp4",
  "C:\\x.mp4",
  "\\\\server\\share.mp4",
  "./media//nested/./b.mov",
  "media\\windows\\c.wav",
  "  media/padded.mp4  ",
  ".",
  "",
];

const otherProjectDir = "/home/user/Projects/Demo Project/";

function projectWithExtraGeneratedOutput(): VideoProject {
  const project = fixtureProject();
  const extraAsset: GeneratedAsset = {
    ...fixtureGeneratedAsset(project),
    id: "extra-generated",
    outputs: [
      {
        mediaId: "sample-generated-output",
        relativePath: "should/not/override.mp4",
        width: 640,
        height: 360,
        durationSeconds: 4,
        fps: 24,
      },
      {
        mediaId: "generated-only-output",
        relativePath: "generated/only.mp4",
        width: 640,
        height: 360,
        durationSeconds: 4,
        fps: 24,
      },
      {
        mediaId: "unsafe-generated-output",
        relativePath: "../escape.mp4",
        width: 640,
        height: 360,
        durationSeconds: 4,
        fps: 24,
      },
    ],
  };
  project.generatedAssets.push(extraAsset);
  return project;
}

const sources: PreviewSource[] = [
  {
    id: "media-1",
    label: "input.mp4",
    kind: "video",
    durationLabel: "00:04",
    resolutionLabel: "640x360",
    aspectRatioLabel: "16:9",
    fpsLabel: "24 fps",
    qualityLabel: "SD",
    pathLabel: "media/input.mp4",
    sourceRangeLabel: "Source 0.00s-4.00s",
    previewUrl: "media://x",
  },
  {
    id: "media-voiceover",
    label: "voiceover.m4a",
    kind: "audio",
    durationLabel: "01:02:03",
    resolutionLabel: null,
    fpsLabel: null,
    pathLabel: "media/voiceover.m4a",
  },
  {
    id: "generated",
    label: "generated.mp4",
    kind: "generated",
    durationLabel: null,
    resolutionLabel: null,
    fpsLabel: "29.97 fps",
    pathLabel: "generated.mp4",
  },
  {
    id: "bad-fps",
    label: "bad.mp4",
    kind: "video",
    durationLabel: "bogus",
    resolutionLabel: null,
    fpsLabel: "0 fps",
    pathLabel: "bad.mp4",
  },
  {
    id: "image",
    label: "still.png",
    kind: "image",
    durationLabel: "00:05",
    resolutionLabel: "10x10",
    fpsLabel: null,
    pathLabel: "still.png",
  },
];

function mediaElementWithDuration(duration: number) {
  const element = document.createElement("video");
  Object.defineProperty(element, "duration", { configurable: true, value: duration });
  return element;
}

describe("preview source characterization", () => {
  it("resolves safe project media paths", () => {
    expect(unsafePaths.map((path) => [path, safeProjectMediaPath("/projects/demo", path)])).toMatchInlineSnapshot(`
      [
        [
          "media/a.mp4",
          "/projects/demo/media/a.mp4",
        ],
        [
          "/etc/passwd",
          null,
        ],
        [
          "../x.mp4",
          null,
        ],
        [
          "file:///x",
          null,
        ],
        [
          "https://x",
          null,
        ],
        [
          "media/../x.mp4",
          null,
        ],
        [
          "C:\\x.mp4",
          null,
        ],
        [
          "\\\\server\\share.mp4",
          null,
        ],
        [
          "./media//nested/./b.mov",
          "/projects/demo/media/nested/b.mov",
        ],
        [
          "media\\windows\\c.wav",
          "/projects/demo/media/windows/c.wav",
        ],
        [
          "  media/padded.mp4  ",
          "/projects/demo/media/padded.mp4",
        ],
        [
          ".",
          null,
        ],
        [
          "",
          null,
        ],
      ]
    `);
    expect(
      ["/projects/demo/", "C:\\Projects\\Demo\\", "  /padded/dir//  ", "   ", ""].map((projectDir) =>
        safeProjectMediaPath(projectDir, "media/a.mp4"),
      ),
    ).toMatchInlineSnapshot(`
      [
        "/projects/demo/media/a.mp4",
        "C:/Projects/Demo/media/a.mp4",
        "/padded/dir/media/a.mp4",
        null,
        null,
      ]
    `);
  });

  it("builds preview urls for bundled, sample, and other project directories", () => {
    const paths = [
      "media/input.mp4",
      "sample/generated/product-reveal.mp4",
      "media/a b#c.mp4",
      "media/../x.mp4",
      "https://x",
      "media/backend-throws.mp4",
      "",
    ];
    expect(
      [sampleProjectBrowserDir, sampleProjectDir, otherProjectDir, ""].map((projectDir) => [
        projectDir,
        paths.map((path) => previewUrlForMedia(projectDir, path)),
      ]),
    ).toMatchInlineSnapshot(`
      [
        [
          "browser://bundled-sample-project",
          [
            "/media/input.mp4",
            "/sample/generated/product-reveal.mp4",
            "/media/a%20b%23c.mp4",
            null,
            "/https%3A/x",
            "/media/backend-throws.mp4",
            null,
          ],
        ],
        [
          "/tmp/video-creater-editor-project",
          [
            "media:///tmp/video-creater-editor-project/media/input.mp4",
            "media:///tmp/video-creater-editor-project/sample/generated/product-reveal.mp4",
            "media:///tmp/video-creater-editor-project/media/a b#c.mp4",
            null,
            null,
            null,
            null,
          ],
        ],
        [
          "/home/user/Projects/Demo Project/",
          [
            "media:///home/user/Projects/Demo Project/media/input.mp4",
            "media:///home/user/Projects/Demo Project/sample/generated/product-reveal.mp4",
            "media:///home/user/Projects/Demo Project/media/a b#c.mp4",
            null,
            null,
            null,
            null,
          ],
        ],
        [
          "",
          [
            null,
            null,
            null,
            null,
            null,
            null,
            null,
          ],
        ],
      ]
    `);
  });

  it("maps preview urls for media and timeline sources", () => {
    const project = projectWithExtraGeneratedOutput();
    expect(previewUrlsForMedia(sampleProjectDir, project.media)).toMatchInlineSnapshot(`
      {
        "media-1": "media:///tmp/video-creater-editor-project/media/input.mp4",
        "media-voiceover": "media:///tmp/video-creater-editor-project/media/voiceover.m4a",
        "sample-generated-output": "media:///tmp/video-creater-editor-project/sample/generated/product-reveal.mp4",
      }
    `);
    expect(previewUrlsForMedia(sampleProjectBrowserDir, [])).toMatchInlineSnapshot(`{}`);
    expect(
      previewUrlsForTimelineSources(sampleProjectBrowserDir, project.media, project.generatedAssets),
    ).toMatchInlineSnapshot(`
      {
        "generated-only-output": "/generated/only.mp4",
        "media-1": "/media/input.mp4",
        "media-voiceover": "/media/voiceover.m4a",
        "sample-generated-output": "/sample/generated/product-reveal.mp4",
        "unsafe-generated-output": null,
      }
    `);
    expect(previewUrlsForTimelineSources(otherProjectDir, [], project.generatedAssets)).toMatchInlineSnapshot(`
      {
        "generated-only-output": "media:///home/user/Projects/Demo Project/generated/only.mp4",
        "sample-generated-output": "media:///home/user/Projects/Demo Project/sample/generated/product-reveal.mp4",
        "unsafe-generated-output": null,
      }
    `);
  });

  it("selects preview sources for media", () => {
    const project = fixtureProject();
    project.media.push(
      {
        id: "media-odd",
        name: "  Odd clip  ",
        relativePath: "media/odd.mov",
        kind: "video",
        durationSeconds: 3725.4,
        width: 3840,
        height: 2160,
        fps: 29.97,
        folderId: null,
      },
      {
        id: "media-still",
        relativePath: "media/still.png",
        kind: "image",
        durationSeconds: 0,
        width: 0,
        height: 1080,
        fps: 0,
      },
    );
    expect(
      [null, "", "missing", ...project.media.map((media) => media.id)].map((mediaId) =>
        selectedPreviewSource(project, mediaId),
      ),
    ).toMatchInlineSnapshot(`
      [
        null,
        null,
        null,
        {
          "aspectRatioLabel": "16:9",
          "durationLabel": "00:04",
          "fpsLabel": "24 fps",
          "id": "media-1",
          "kind": "video",
          "label": "input.mp4",
          "pathLabel": "media/input.mp4",
          "previewUrl": null,
          "qualityLabel": "SD",
          "resolutionLabel": "640x360",
          "sourceRangeLabel": null,
        },
        {
          "aspectRatioLabel": null,
          "durationLabel": "00:04",
          "fpsLabel": null,
          "id": "media-voiceover",
          "kind": "audio",
          "label": "voiceover.m4a",
          "pathLabel": "media/voiceover.m4a",
          "previewUrl": null,
          "qualityLabel": null,
          "resolutionLabel": null,
          "sourceRangeLabel": null,
        },
        {
          "aspectRatioLabel": "16:9",
          "durationLabel": "00:04",
          "fpsLabel": "24 fps",
          "id": "sample-generated-output",
          "kind": "generated",
          "label": "product-reveal.mp4",
          "pathLabel": "sample/generated/product-reveal.mp4",
          "previewUrl": null,
          "qualityLabel": "SD",
          "resolutionLabel": "640x360",
          "sourceRangeLabel": null,
        },
        {
          "aspectRatioLabel": "16:9",
          "durationLabel": "62:05",
          "fpsLabel": "30 fps",
          "id": "media-odd",
          "kind": "video",
          "label": "Odd clip",
          "pathLabel": "media/odd.mov",
          "previewUrl": null,
          "qualityLabel": "4K",
          "resolutionLabel": "3840x2160",
          "sourceRangeLabel": null,
        },
        {
          "aspectRatioLabel": null,
          "durationLabel": null,
          "fpsLabel": null,
          "id": "media-still",
          "kind": "image",
          "label": "still.png",
          "pathLabel": "media/still.png",
          "previewUrl": null,
          "qualityLabel": null,
          "resolutionLabel": null,
          "sourceRangeLabel": null,
        },
      ]
    `);
    expect(
      selectedPreviewSource(project, "media-1", "Source 0.50s-3.00s", sampleProjectBrowserDir),
    ).toMatchInlineSnapshot(`
      {
        "aspectRatioLabel": "16:9",
        "durationLabel": "00:04",
        "fpsLabel": "24 fps",
        "id": "media-1",
        "kind": "video",
        "label": "input.mp4",
        "pathLabel": "media/input.mp4",
        "previewUrl": "/media/input.mp4",
        "qualityLabel": "SD",
        "resolutionLabel": "640x360",
        "sourceRangeLabel": "Source 0.50s-3.00s",
      }
    `);
    expect(selectedPreviewSource(project, "media-odd", null, otherProjectDir)).toMatchInlineSnapshot(`
      {
        "aspectRatioLabel": "16:9",
        "durationLabel": "62:05",
        "fpsLabel": "30 fps",
        "id": "media-odd",
        "kind": "video",
        "label": "Odd clip",
        "pathLabel": "media/odd.mov",
        "previewUrl": "media:///home/user/Projects/Demo Project/media/odd.mov",
        "qualityLabel": "4K",
        "resolutionLabel": "3840x2160",
        "sourceRangeLabel": null,
      }
    `);
  });

  it("formats viewer durations and seconds", () => {
    expect(
      [0, -1, Number.NaN, Number.POSITIVE_INFINITY, 0.4, 0.5, 4, 59.5, 61, 3600.2, 3725.4].map((seconds) =>
        formatViewerDuration(seconds),
      ),
    ).toMatchInlineSnapshot(`
      [
        null,
        null,
        null,
        null,
        "00:00",
        "00:01",
        "00:04",
        "01:00",
        "01:01",
        "60:00",
        "62:05",
      ]
    `);
    const seconds = [0, 1.005, 2, -1.5, 12.3456, Number.NaN];
    const labels = seconds.map((value) => formatViewerSeconds(value));
    expect(labels).toMatchInlineSnapshot(`
      [
        "0.00s",
        "1.00s",
        "2.00s",
        "-1.50s",
        "12.35s",
        "NaNs",
      ]
    `);
  });

  it("labels item source ranges", () => {
    const project = fixtureProject();
    const items: TimelineItem[] = [
      fixtureItem(project, "video"),
      fixtureItem(project, "caption"),
      fixtureItem(project, "audio"),
      { ...fixtureItem(project, "video"), properties: {} },
      { ...fixtureItem(project, "video"), properties: { sourceIn: 1.25, sourceOut: "5" } },
      { ...fixtureItem(project, "video"), properties: { sourceIn: 1.005, sourceOut: 12.3456 } },
    ];
    expect(items.map((item) => [sourceRangeLabelForItem(item), sourceRangeLabel(item)])).toMatchInlineSnapshot(`
      [
        [
          "Source 0.00s-4.00s",
          "0.00s-4.00s",
        ],
        [
          "Source 0.65s-2.00s",
          "0.65s-2.00s",
        ],
        [
          "Source 0.00s-4.00s",
          "0.00s-4.00s",
        ],
        [
          null,
          null,
        ],
        [
          null,
          null,
        ],
        [
          "Source 1.00s-12.35s",
          "1.00s-12.35s",
        ],
      ]
    `);
  });

  it("parses preview duration labels and formats current time", () => {
    expect(
      [null, undefined, "", "00:04", "01:02:03", "1:2.5", "4", "a:b", "-1:00", "1:2:3:4", "00:04.250", " 1 : 05 "].map(
        (label) => parsePreviewDurationLabel(label),
      ),
    ).toMatchInlineSnapshot(`
      [
        0,
        0,
        0,
        4,
        3723,
        62.5,
        0,
        0,
        0,
        0,
        4.25,
        65,
      ]
    `);
    expect(
      [0, -1, Number.NaN, 1, 4.25, 59.9996, 0.0004, 3661.001, 7200, Number.POSITIVE_INFINITY].map((seconds) =>
        formatPreviewCurrentTime(seconds),
      ),
    ).toMatchInlineSnapshot(`
      [
        "00:00:00",
        "00:00:00",
        "00:00:00",
        "00:00:01",
        "00:00:04.250",
        "00:01:00",
        "00:00:00",
        "01:01:01.001",
        "02:00:00",
        "00:00:00",
      ]
    `);
  });

  it("builds preview transport data for timeline and source modes", () => {
    const project = fixtureProject();
    const selectedItem = fixtureItem(project, "video");
    const timeline = project.timeline;
    expect([
      previewTransportData({ activeViewerMode: "timeline", timeline, selectedItem, selectedSource: sources[0] ?? null }),
      previewTransportData({ activeViewerMode: "timeline", selectedItem, selectedSource: null }),
      previewTransportData({ activeViewerMode: "timeline", selectedItem: null, selectedSource: null }),
      previewTransportData({ activeViewerMode: "timeline", selectedItem: undefined, selectedSource: null }),
      previewTransportData({ activeViewerMode: "source", timeline, selectedItem, selectedSource: sources[0] ?? null }),
      previewTransportData({ activeViewerMode: "source", selectedItem: null, selectedSource: sources[1] ?? null }),
      previewTransportData({ activeViewerMode: "source", selectedItem: null, selectedSource: sources[2] ?? null }),
      previewTransportData({ activeViewerMode: "source", timeline, selectedItem, selectedSource: null }),
      previewTransportData({ activeViewerMode: "source", selectedItem, selectedSource: null }),
    ]).toMatchInlineSnapshot(`
      [
        {
          "badges": [
            "Fit",
          ],
          "durationLabel": "00:08",
          "primaryLabel": null,
        },
        {
          "badges": [
            "Fit",
          ],
          "durationLabel": "00:04",
          "primaryLabel": null,
        },
        {
          "badges": [
            "Fit",
          ],
          "durationLabel": "00:00",
          "primaryLabel": null,
        },
        {
          "badges": [
            "Fit",
          ],
          "durationLabel": "00:00",
          "primaryLabel": null,
        },
        {
          "badges": [
            "640x360",
            "16:9",
            "24 fps",
            "SD",
            "Fit",
          ],
          "durationLabel": "00:04",
          "primaryLabel": "Source 0.00s-4.00s",
        },
        {
          "badges": [
            "Fit",
          ],
          "durationLabel": "01:02:03",
          "primaryLabel": null,
        },
        {
          "badges": [
            "29.97 fps",
            "Fit",
          ],
          "durationLabel": "00:00",
          "primaryLabel": null,
        },
        {
          "badges": [
            "Fit",
          ],
          "durationLabel": "00:08",
          "primaryLabel": null,
        },
        {
          "badges": [
            "Fit",
          ],
          "durationLabel": "00:04",
          "primaryLabel": null,
        },
      ]
    `);
  });

  it("derives source step seconds, media element durations, and tab ids", () => {
    expect([null, ...sources].map((source) => sourcePreviewStepSeconds(source))).toMatchInlineSnapshot(`
      [
        0,
        0.041666666666666664,
        1,
        0.033366700033366704,
        0.041666666666666664,
        0,
      ]
    `);
    const elements = [
      null,
      mediaElementWithDuration(12.5),
      mediaElementWithDuration(Number.NaN),
      mediaElementWithDuration(Number.POSITIVE_INFINITY),
      mediaElementWithDuration(0),
    ];
    expect(
      elements.map((element) => [
        mediaElementDurationSeconds(element, null),
        mediaElementDurationSeconds(element, sources[1] ?? null),
        mediaElementDurationSeconds(element, sources[3] ?? null),
      ]),
    ).toMatchInlineSnapshot(`
      [
        [
          0,
          3723,
          0,
        ],
        [
          12.5,
          12.5,
          12.5,
        ],
        [
          0,
          3723,
          0,
        ],
        [
          0,
          3723,
          0,
        ],
        [
          0,
          3723,
          0,
        ],
      ]
    `);
    expect(["media-1", "a b/c?d&e", "", "émoji"].map((id) => sourceViewerTabId(id))).toMatchInlineSnapshot(`
      [
        "preview-viewer-tab-source-media-1",
        "preview-viewer-tab-source-a%20b%2Fc%3Fd%26e",
        "preview-viewer-tab-source-",
        "preview-viewer-tab-source-%C3%A9moji",
      ]
    `);
  });
});
