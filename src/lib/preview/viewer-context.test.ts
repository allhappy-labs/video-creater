import { describe, expect, it } from "vitest";
import type { VideoProject } from "@/lib/project";
import type { TimelineItem } from "@/lib/timeline";
import type { TimelinePreviewCanvasState } from "@/lib/preview/canvas-geometry";
import { fixtureProject } from "@/test-utils/editor-fixtures";
import {
  initialSelectedTimelineItem,
  resolveContextToolbarPlacement,
  viewerContextKindForItem,
  viewerContextToolbarPlacementForItem,
  type ViewerContextKind,
} from "@/lib/preview/viewer-context";

const extraItems: TimelineItem[] = [
  {
    id: "image-item",
    kind: "image_clip",
    startSeconds: 0,
    durationSeconds: 2,
    source: { type: "media", mediaId: "media-still" },
    label: "Still",
    properties: {},
  },
  {
    id: "lottie-item",
    kind: "lottie_clip",
    startSeconds: 0,
    durationSeconds: 2,
    source: { type: "media", mediaId: "sample-generated-output" },
    label: "Lottie",
    properties: {},
  },
  {
    id: "generated-clip-item",
    kind: "generated_clip",
    startSeconds: 0,
    durationSeconds: 2,
    source: { type: "generated", artifactId: "unknown-artifact" },
    label: "Generated clip",
    properties: {},
  },
  {
    id: "generated-output-video",
    kind: "video_clip",
    startSeconds: 0,
    durationSeconds: 2,
    source: { type: "media", mediaId: "sample-generated-output" },
    label: "Generated output by media id",
    properties: {},
  },
  {
    id: "nested-timeline-video",
    kind: "video_clip",
    startSeconds: 0,
    durationSeconds: 2,
    source: { type: "timeline", timelineId: "timeline-2" },
    label: "Nested timeline",
    properties: {},
  },
  {
    id: "text-overlay",
    kind: "overlay",
    startSeconds: 0,
    durationSeconds: 2,
    source: { type: "text", text: "Hello" },
    label: "Text overlay",
    properties: {},
  },
  {
    id: "template-overlay",
    kind: "overlay",
    startSeconds: 0,
    durationSeconds: 2,
    source: { type: "text", text: "Hook" },
    label: "Template overlay",
    properties: { templateId: "kinetic-lower-third-v1" },
  },
  {
    id: "media-overlay",
    kind: "overlay",
    startSeconds: 0,
    durationSeconds: 2,
    source: { type: "media", mediaId: "media-1" },
    label: "Media overlay",
    properties: {},
  },
  {
    id: "generated-template-overlay",
    kind: "overlay",
    startSeconds: 0,
    durationSeconds: 2,
    source: { type: "text", text: "Generated template" },
    label: "Generated template overlay",
    properties: { templateId: "kinetic-lower-third-v1", generatedAssetId: "sample-generated-shot" },
  },
  {
    id: "scene-item",
    kind: "hyperframe_scene",
    startSeconds: 0,
    durationSeconds: 2,
    source: { type: "text", text: "<scene />" },
    label: "Scene",
    properties: {},
  },
  {
    id: "upper-caption",
    kind: "caption",
    startSeconds: 0,
    durationSeconds: 2,
    source: { type: "text", text: "Upper caption" },
    label: "Upper caption",
    properties: { captionPlacement: "upper" },
  },
  {
    id: "generated-audio",
    kind: "audio_clip",
    startSeconds: 0,
    durationSeconds: 2,
    source: { type: "media", mediaId: "sample-generated-output" },
    label: "Generated audio",
    properties: { generatedAssetId: "sample-generated-shot" },
  },
];

function allItems(project: VideoProject) {
  return [...project.timeline.tracks.flatMap((track) => track.items), ...extraItems];
}

function projectWithItems(items: TimelineItem[], generatedAssets = fixtureProject().generatedAssets): VideoProject {
  const project = fixtureProject();
  project.generatedAssets = generatedAssets;
  project.timeline.tracks = project.timeline.tracks.map((track, index) => ({
    ...track,
    items: index === 0 ? items : [],
  }));
  return project;
}

describe("viewer context characterization", () => {
  it("classifies every item kind", () => {
    const project = fixtureProject();
    expect([
      ["null", viewerContextKindForItem(project, null)],
      ...allItems(project).map((item) => [item.id, viewerContextKindForItem(project, item)]),
    ]).toMatchInlineSnapshot(`
      [
        [
          "null",
          null,
        ],
        [
          "item-1",
          "visual",
        ],
        [
          "sample-generated-clip",
          "generated",
        ],
        [
          "caption-1",
          "caption",
        ],
        [
          "caption-2",
          "caption",
        ],
        [
          "music-bed",
          null,
        ],
        [
          "image-item",
          "visual",
        ],
        [
          "lottie-item",
          "lottie",
        ],
        [
          "generated-clip-item",
          "visual",
        ],
        [
          "generated-output-video",
          "generated",
        ],
        [
          "nested-timeline-video",
          "visual",
        ],
        [
          "text-overlay",
          "text",
        ],
        [
          "template-overlay",
          "template",
        ],
        [
          "media-overlay",
          "visual",
        ],
        [
          "generated-template-overlay",
          "generated",
        ],
        [
          "scene-item",
          "visual",
        ],
        [
          "upper-caption",
          "caption",
        ],
        [
          "generated-audio",
          null,
        ],
      ]
    `);
  });

  it("places the viewer context toolbar", () => {
    const project = fixtureProject();
    const kinds: Array<ViewerContextKind | null> = [
      null,
      "visual",
      "caption",
      "text",
      "template",
      "lottie",
      "generated",
    ];
    expect(
      [null, ...allItems(project)].map((item) => [
        item?.id ?? "null",
        viewerContextToolbarPlacementForItem(item, item ? viewerContextKindForItem(project, item) : null),
        kinds.map((kind) => viewerContextToolbarPlacementForItem(item, kind)).join(","),
      ]),
    ).toMatchInlineSnapshot(`
      [
        [
          "null",
          "bottom",
          "bottom,bottom,top,bottom,bottom,bottom,bottom",
        ],
        [
          "item-1",
          "bottom",
          "bottom,bottom,top,bottom,bottom,bottom,bottom",
        ],
        [
          "sample-generated-clip",
          "bottom",
          "bottom,bottom,top,bottom,bottom,bottom,bottom",
        ],
        [
          "caption-1",
          "top",
          "bottom,bottom,top,bottom,bottom,bottom,bottom",
        ],
        [
          "caption-2",
          "top",
          "bottom,bottom,top,bottom,bottom,bottom,bottom",
        ],
        [
          "music-bed",
          "bottom",
          "bottom,bottom,top,bottom,bottom,bottom,bottom",
        ],
        [
          "image-item",
          "bottom",
          "bottom,bottom,top,bottom,bottom,bottom,bottom",
        ],
        [
          "lottie-item",
          "bottom",
          "bottom,bottom,top,bottom,bottom,bottom,bottom",
        ],
        [
          "generated-clip-item",
          "bottom",
          "bottom,bottom,top,bottom,bottom,bottom,bottom",
        ],
        [
          "generated-output-video",
          "bottom",
          "bottom,bottom,top,bottom,bottom,bottom,bottom",
        ],
        [
          "nested-timeline-video",
          "bottom",
          "bottom,bottom,top,bottom,bottom,bottom,bottom",
        ],
        [
          "text-overlay",
          "bottom",
          "bottom,bottom,top,bottom,bottom,bottom,bottom",
        ],
        [
          "template-overlay",
          "bottom",
          "bottom,bottom,top,bottom,bottom,bottom,bottom",
        ],
        [
          "media-overlay",
          "bottom",
          "bottom,bottom,top,bottom,bottom,bottom,bottom",
        ],
        [
          "generated-template-overlay",
          "bottom",
          "bottom,bottom,top,bottom,bottom,bottom,bottom",
        ],
        [
          "scene-item",
          "bottom",
          "bottom,bottom,top,bottom,bottom,bottom,bottom",
        ],
        [
          "upper-caption",
          "bottom",
          "bottom,bottom,bottom,bottom,bottom,bottom,bottom",
        ],
        [
          "generated-audio",
          "bottom",
          "bottom,bottom,top,bottom,bottom,bottom,bottom",
        ],
      ]
    `);
  });

  it("chooses the initial selected timeline item", () => {
    const project = fixtureProject();
    const [imageItem, , , , , textOverlay, , , , , upperCaption] = extraItems;
    const videoItem = project.timeline.tracks[0]?.items[0];
    expect(
      [
        project,
        projectWithItems(project.timeline.tracks.flatMap((track) => track.items), []),
        projectWithItems([textOverlay, upperCaption].filter((item): item is TimelineItem => Boolean(item)), []),
        projectWithItems([imageItem, videoItem].filter((item): item is TimelineItem => Boolean(item)), []),
        projectWithItems([], []),
      ].map((candidate) => initialSelectedTimelineItem(candidate)?.id ?? null),
    ).toMatchInlineSnapshot(`
      [
        "sample-generated-clip",
        "item-1",
        "text-overlay",
        "item-1",
        null,
      ]
    `);
  });

  it("resolves context toolbar placement against canvas occupancy", () => {
    const issueStates: TimelinePreviewCanvasState["issueState"][] = ["clear", "notice", "retry"];
    const results: string[] = [];
    for (const preferred of ["top", "bottom"] as const) {
      for (const issueState of issueStates) {
        for (const topOccupied of [false, true]) {
          for (const bottomOccupied of [false, true]) {
            const placement = resolveContextToolbarPlacement(preferred, {
              issueState,
              topOccupied,
              bottomOccupied,
            });
            results.push(
              `${preferred} ${issueState} top=${topOccupied} bottom=${bottomOccupied} -> ${placement}`,
            );
          }
        }
      }
    }
    expect(results).toMatchInlineSnapshot(`
      [
        "top clear top=false bottom=false -> top",
        "top clear top=false bottom=true -> top",
        "top clear top=true bottom=false -> bottom",
        "top clear top=true bottom=true -> null",
        "top notice top=false bottom=false -> top",
        "top notice top=false bottom=true -> top",
        "top notice top=true bottom=false -> bottom",
        "top notice top=true bottom=true -> null",
        "top retry top=false bottom=false -> null",
        "top retry top=false bottom=true -> null",
        "top retry top=true bottom=false -> null",
        "top retry top=true bottom=true -> null",
        "bottom clear top=false bottom=false -> bottom",
        "bottom clear top=false bottom=true -> top",
        "bottom clear top=true bottom=false -> bottom",
        "bottom clear top=true bottom=true -> null",
        "bottom notice top=false bottom=false -> bottom",
        "bottom notice top=false bottom=true -> top",
        "bottom notice top=true bottom=false -> bottom",
        "bottom notice top=true bottom=true -> null",
        "bottom retry top=false bottom=false -> null",
        "bottom retry top=false bottom=true -> null",
        "bottom retry top=true bottom=false -> null",
        "bottom retry top=true bottom=true -> null",
      ]
    `);
  });
});
