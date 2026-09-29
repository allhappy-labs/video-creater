import { describe, expect, it } from "vitest";
import type { GeneratedAsset, GeneratedAssetSettings, MediaAsset, ProjectJobSummary } from "@/lib/project";
import type { TimelineItem } from "@/lib/timeline";
import { fixtureGeneratedAsset, fixtureItem, fixtureProject } from "@/test-utils/editor-fixtures";
import {
  findGeneratedAssetForItem,
  findGeneratedAssetForMedia,
  findGeneratedAssetForOutput,
  findGeneratedOutputForItem,
  findGeneratedOutputForMedia,
  findMediaAsset,
  formatGeneratedDuration,
  formatGeneratedResolution,
  generatedAssetCreatedAtMs,
  generatedAssetForTimelineItem,
  generatedAssetHasMedia,
  generatedAssetNeedsHistoryCard,
  generatedAssetTitleOrId,
  generatedAssetTitleOrPrompt,
  generatedAssetTitleWithPrompt,
  generatedModelLabel,
  generatedOutputFileLabel,
  generatedOutputHasProviderSourceUrl,
  generatedOutputMeta,
  generatedPendingOutputLabel,
  generatedReferenceCount,
  generatedReferenceMediaIds,
  generatedWorkflowLabel,
  humanizeWorkflowToken,
  isActiveGeneratedAsset,
  promptExcerpt,
  recentGeneratedAssets,
  selectedGeneratedClipSettingsLabels,
  sortGeneratedAssetsByCreatedAtDesc,
  uniqueStringValues,
} from "@/lib/generation/assets";

function generatedAssetVariant(
  base: GeneratedAsset,
  id: string,
  overrides: Partial<GeneratedAsset>,
): GeneratedAsset {
  return { ...structuredClone(base), id, ...overrides };
}

function assetStates() {
  const project = fixtureProject();
  const base = fixtureGeneratedAsset(project);
  const pending = generatedAssetVariant(base, "asset-pending", {
    status: "queued",
    name: "  ",
    prompt: "\n  First pending line  \nSecond line",
    outputs: [],
    createdAt: "2026-07-13T00:00:00Z",
  });
  const running = generatedAssetVariant(base, "asset-running", {
    status: "running",
    name: null,
    prompt: "",
    outputs: [],
    createdAt: "2026-07-12T00:00:00Z",
  });
  const completed = base;
  const failed = generatedAssetVariant(base, "asset-failed", {
    status: "failed",
    name: undefined as unknown as null,
    prompt: "   \n\t",
    outputs: [],
    createdAt: "not a date",
  });
  const cancelled = generatedAssetVariant(base, "asset-cancelled", {
    status: "cancelled",
    outputs: [],
    createdAt: "",
  });
  const replacedOutput = generatedAssetVariant(base, "asset-replaced-output", {
    name: "Replaced output",
    placementIntent: "replace:item-1",
    outputs: [
      {
        mediaId: "media-replaced",
        relativePath: "generated/replaced/output-v2.mov",
        sourceUrl: "https://provider.example/output-v2.mov",
        width: 1920,
        height: 1080,
        durationSeconds: 2.5,
        fps: 29.97,
      },
      {
        mediaId: "sample-generated-output",
        relativePath: "sample/generated/product-reveal.mp4",
        sourceUrl: "   ",
        width: 640,
        height: 360,
        durationSeconds: 4,
        fps: 24,
      },
    ],
    createdAt: "2026-07-12T00:00:00Z",
  });
  const withReferences = generatedAssetVariant(base, "asset-with-references", {
    references: {
      mediaIds: ["media-1", "media-voiceover", "media-1"],
      firstFrameMediaId: "media-1",
      lastFrameMediaId: "media-last",
      sourceVideoMediaRef: "media-1",
      referenceImageMediaRefs: ["media-image", "media-1"],
      referenceVideoMediaRefs: ["media-video", ""],
      referenceAudioMediaRefs: ["media-voiceover", "media-audio"],
    },
    outputs: [],
    createdAt: "2026-07-11T23:59:59.999Z",
  });
  return {
    project,
    states: { pending, running, completed, failed, cancelled, replacedOutput, withReferences },
  };
}

describe("generated asset characterization", () => {
  it("dedupes string values and reference media ids", () => {
    const { states } = assetStates();
    expect({
      unique: uniqueStringValues(["a", null, "b", "", undefined, "a", "c"]),
      references: Object.fromEntries(
        Object.entries(states).map(([key, asset]) => [key, generatedReferenceMediaIds(asset.references)]),
      ),
    }).toMatchInlineSnapshot(`
      {
        "references": {
          "cancelled": [
            "media-1",
          ],
          "completed": [
            "media-1",
          ],
          "failed": [
            "media-1",
          ],
          "pending": [
            "media-1",
          ],
          "replacedOutput": [
            "media-1",
          ],
          "running": [
            "media-1",
          ],
          "withReferences": [
            "media-1",
            "media-voiceover",
            "media-image",
            "media-video",
            "media-audio",
          ],
        },
        "unique": [
          "a",
          "b",
          "c",
        ],
      }
    `);
  });

  it("labels and titles generated assets in every state", () => {
    const { states } = assetStates();
    const mediaIds = new Set(["sample-generated-output", "media-1"]);
    expect(
      Object.fromEntries(
        Object.entries(states).map(([key, asset]) => [
          key,
          {
            titleWithPrompt: generatedAssetTitleWithPrompt(asset),
            titleOrPrompt: generatedAssetTitleOrPrompt(asset),
            titleOrId: generatedAssetTitleOrId(asset),
            pendingOutput: generatedPendingOutputLabel(asset),
            active: isActiveGeneratedAsset(asset),
            needsHistoryCard: generatedAssetNeedsHistoryCard(asset, mediaIds),
            needsHistoryCardWithoutMedia: generatedAssetNeedsHistoryCard(asset, new Set()),
            createdAtMs: generatedAssetCreatedAtMs(asset),
            referenceCount: generatedReferenceCount(asset),
            modelLabel: generatedModelLabel(asset),
            outputs: asset.outputs.map((output) => ({
              hasProviderSourceUrl: generatedOutputHasProviderSourceUrl(output),
              fileLabel: generatedOutputFileLabel(output),
              metaWithoutMedia: generatedOutputMeta(output, null),
            })),
          },
        ]),
      ),
    ).toMatchInlineSnapshot(`
      {
        "cancelled": {
          "active": false,
          "createdAtMs": null,
          "modelLabel": "local/bundled-edison-restoration",
          "needsHistoryCard": true,
          "needsHistoryCardWithoutMedia": true,
          "outputs": [],
          "pendingOutput": "no outputs yet",
          "referenceCount": 2,
          "titleOrId": "Bundled Edison restoration",
          "titleOrPrompt": "Bundled Edison restoration",
          "titleWithPrompt": "Bundled Edison restoration",
        },
        "completed": {
          "active": false,
          "createdAtMs": 1783814400000,
          "modelLabel": "local/bundled-edison-restoration",
          "needsHistoryCard": false,
          "needsHistoryCardWithoutMedia": true,
          "outputs": [
            {
              "fileLabel": "product-reveal.mp4",
              "hasProviderSourceUrl": false,
              "metaWithoutMedia": "640 x 360 - 4s - 24 fps",
            },
          ],
          "pendingOutput": null,
          "referenceCount": 2,
          "titleOrId": "Bundled Edison restoration",
          "titleOrPrompt": "Bundled Edison restoration",
          "titleWithPrompt": "Bundled Edison restoration",
        },
        "failed": {
          "active": false,
          "createdAtMs": null,
          "modelLabel": "local/bundled-edison-restoration",
          "needsHistoryCard": true,
          "needsHistoryCardWithoutMedia": true,
          "outputs": [],
          "pendingOutput": "no outputs yet",
          "referenceCount": 2,
          "titleOrId": "asset-failed",
          "titleOrPrompt": "   
      	",
          "titleWithPrompt": "asset-failed",
        },
        "pending": {
          "active": true,
          "createdAtMs": 1783900800000,
          "modelLabel": "local/bundled-edison-restoration",
          "needsHistoryCard": true,
          "needsHistoryCardWithoutMedia": true,
          "outputs": [],
          "pendingOutput": "waiting for generated output",
          "referenceCount": 2,
          "titleOrId": "asset-pending",
          "titleOrPrompt": "
        First pending line  
      Second line",
          "titleWithPrompt": "First pending line",
        },
        "replacedOutput": {
          "active": false,
          "createdAtMs": 1783814400000,
          "modelLabel": "local/bundled-edison-restoration",
          "needsHistoryCard": true,
          "needsHistoryCardWithoutMedia": true,
          "outputs": [
            {
              "fileLabel": "output-v2.mov",
              "hasProviderSourceUrl": true,
              "metaWithoutMedia": "1920 x 1080 - 2.5s - 30 fps",
            },
            {
              "fileLabel": "product-reveal.mp4",
              "hasProviderSourceUrl": false,
              "metaWithoutMedia": "640 x 360 - 4s - 24 fps",
            },
          ],
          "pendingOutput": null,
          "referenceCount": 2,
          "titleOrId": "Replaced output",
          "titleOrPrompt": "Replaced output",
          "titleWithPrompt": "Replaced output",
        },
        "running": {
          "active": true,
          "createdAtMs": 1783814400000,
          "modelLabel": "local/bundled-edison-restoration",
          "needsHistoryCard": true,
          "needsHistoryCardWithoutMedia": true,
          "outputs": [],
          "pendingOutput": "waiting for generated output",
          "referenceCount": 2,
          "titleOrId": "asset-running",
          "titleOrPrompt": "",
          "titleWithPrompt": "asset-running",
        },
        "withReferences": {
          "active": false,
          "createdAtMs": 1783814399999,
          "modelLabel": "local/bundled-edison-restoration",
          "needsHistoryCard": true,
          "needsHistoryCardWithoutMedia": true,
          "outputs": [],
          "pendingOutput": "no outputs yet",
          "referenceCount": 5,
          "titleOrId": "Bundled Edison restoration",
          "titleOrPrompt": "Bundled Edison restoration",
          "titleWithPrompt": "Bundled Edison restoration",
        },
      }
    `);
  });

  it("formats generated output metadata", () => {
    const { project, states } = assetStates();
    const output = states.replacedOutput.outputs[0]!;
    const media: MediaAsset = {
      id: "media-replaced",
      relativePath: "generated/replaced/output-v2.mov",
      kind: "video",
      durationSeconds: 3,
      width: 3840,
      height: 2160,
      fps: 59.94,
      folderId: null,
    };
    expect({
      resolutions: [
        formatGeneratedResolution(1920, 1080),
        formatGeneratedResolution(null, 1080),
        formatGeneratedResolution(0, 0),
      ],
      durations: [null, 0, -1, 4, 2.25, 0.04].map((seconds) => formatGeneratedDuration(seconds)),
      metaWithMedia: generatedOutputMeta(output, media),
      metaWithUnsizedMedia: generatedOutputMeta(output, {
        ...media,
        width: null,
        height: null,
        fps: null,
      }),
      metaWithSampleMedia: generatedOutputMeta(
        states.completed.outputs[0]!,
        project.media.find((asset) => asset.id === "sample-generated-output") ?? null,
      ),
    }).toMatchInlineSnapshot(`
      {
        "durations": [
          "Auto",
          "Auto",
          "Auto",
          "4s",
          "2.3s",
          "0.0s",
        ],
        "metaWithMedia": "3840 x 2160 - 3s - 60 fps",
        "metaWithSampleMedia": "640 x 360 - 4s - 24 fps",
        "metaWithUnsizedMedia": "1920 x 1080 - 3s - 30 fps",
        "resolutions": [
          "1920 x 1080",
          "Auto",
          "Auto",
        ],
      }
    `);
  });

  it("finds generated assets, outputs and media", () => {
    const { project, states } = assetStates();
    const assets = Object.values(states);
    const clip = fixtureItem(project, "video");
    const generatedClip = project.timeline.tracks
      .flatMap((track) => track.items)
      .find((item) => item.id === "sample-generated-clip")!;
    const items: Record<string, TimelineItem> = {
      plainClip: clip,
      generatedClip,
      explicitMissingAsset: {
        ...generatedClip,
        properties: { ...generatedClip.properties, generatedAssetId: "missing-asset" },
      },
      outputPropertyClip: {
        ...generatedClip,
        properties: {
          generatedAssetId: "asset-replaced-output",
          generatedOutputMediaId: "sample-generated-output",
        },
      },
      assetIdSourceClip: {
        ...clip,
        id: "asset-id-source",
        source: { type: "media", mediaId: "asset-failed" },
        properties: {},
      },
      textClip: {
        id: "text-clip",
        kind: "overlay",
        startSeconds: 0,
        durationSeconds: 1,
        source: { type: "text", text: "Title" },
        label: "Title",
        properties: {},
      },
    };
    const projectWithAssets = { ...project, generatedAssets: assets };
    expect({
      byItem: Object.fromEntries(
        Object.entries(items).map(([key, item]) => {
          const inspectorAsset = findGeneratedAssetForItem(item, assets);
          const workspaceAsset = generatedAssetForTimelineItem(projectWithAssets, item);
          return [
            key,
            {
              inspectorAsset: inspectorAsset?.id ?? null,
              workspaceAsset: workspaceAsset?.id ?? null,
              variantsAgree: inspectorAsset === workspaceAsset,
              output: findGeneratedOutputForItem(item, inspectorAsset)?.mediaId ?? null,
              outputWithoutAsset: findGeneratedOutputForItem(item, null),
            },
          ];
        }),
      ),
      byMedia: [null, "sample-generated-output", "media-replaced", "asset-running", "media-1"].map(
        (mediaId) => {
          const asset = findGeneratedAssetForMedia(mediaId, assets);
          return {
            mediaId,
            asset: asset?.id ?? null,
            forOutput: findGeneratedAssetForOutput(assets, mediaId)?.id ?? null,
            output: findGeneratedOutputForMedia(mediaId, asset)?.mediaId ?? null,
            outputFromReplaced:
              findGeneratedOutputForMedia(mediaId, states.replacedOutput)?.mediaId ?? null,
            media: findMediaAsset(project.media, mediaId)?.id ?? null,
          };
        },
      ),
      hasMedia: [
        generatedAssetHasMedia(states.replacedOutput, "media-replaced"),
        generatedAssetHasMedia(states.replacedOutput, "asset-replaced-output"),
        generatedAssetHasMedia(states.replacedOutput, "media-1"),
      ],
      findMediaUndefined: findMediaAsset(project.media, undefined),
      findMediaEmpty: findMediaAsset(project.media, ""),
      findOutputWithoutAsset: findGeneratedOutputForMedia("media-1", null),
      assetForOutputWithoutMedia: findGeneratedAssetForOutput(assets, null),
    }).toMatchInlineSnapshot(`
      {
        "assetForOutputWithoutMedia": null,
        "byItem": {
          "assetIdSourceClip": {
            "inspectorAsset": "asset-failed",
            "output": null,
            "outputWithoutAsset": null,
            "variantsAgree": true,
            "workspaceAsset": "asset-failed",
          },
          "explicitMissingAsset": {
            "inspectorAsset": "sample-generated-shot",
            "output": "sample-generated-output",
            "outputWithoutAsset": null,
            "variantsAgree": true,
            "workspaceAsset": "sample-generated-shot",
          },
          "generatedClip": {
            "inspectorAsset": "sample-generated-shot",
            "output": "sample-generated-output",
            "outputWithoutAsset": null,
            "variantsAgree": true,
            "workspaceAsset": "sample-generated-shot",
          },
          "outputPropertyClip": {
            "inspectorAsset": "asset-replaced-output",
            "output": "sample-generated-output",
            "outputWithoutAsset": null,
            "variantsAgree": true,
            "workspaceAsset": "asset-replaced-output",
          },
          "plainClip": {
            "inspectorAsset": null,
            "output": null,
            "outputWithoutAsset": null,
            "variantsAgree": true,
            "workspaceAsset": null,
          },
          "textClip": {
            "inspectorAsset": null,
            "output": null,
            "outputWithoutAsset": null,
            "variantsAgree": true,
            "workspaceAsset": null,
          },
        },
        "byMedia": [
          {
            "asset": null,
            "forOutput": null,
            "media": null,
            "mediaId": null,
            "output": null,
            "outputFromReplaced": "media-replaced",
          },
          {
            "asset": "sample-generated-shot",
            "forOutput": "sample-generated-shot",
            "media": "sample-generated-output",
            "mediaId": "sample-generated-output",
            "output": "sample-generated-output",
            "outputFromReplaced": "sample-generated-output",
          },
          {
            "asset": "asset-replaced-output",
            "forOutput": "asset-replaced-output",
            "media": null,
            "mediaId": "media-replaced",
            "output": "media-replaced",
            "outputFromReplaced": "media-replaced",
          },
          {
            "asset": "asset-running",
            "forOutput": null,
            "media": null,
            "mediaId": "asset-running",
            "output": null,
            "outputFromReplaced": "media-replaced",
          },
          {
            "asset": null,
            "forOutput": null,
            "media": "media-1",
            "mediaId": "media-1",
            "output": null,
            "outputFromReplaced": "media-replaced",
          },
        ],
        "findMediaEmpty": null,
        "findMediaUndefined": null,
        "findOutputWithoutAsset": null,
        "hasMedia": [
          true,
          true,
          false,
        ],
      }
    `);
  });

  it("sorts generated assets by creation time", () => {
    const { states } = assetStates();
    const equalA = generatedAssetVariant(states.completed, "equal-a", {
      createdAt: "2026-07-12T00:00:00Z",
    });
    const equalB = generatedAssetVariant(states.completed, "equal-b", {
      createdAt: "2026-07-12T00:00:00.000Z",
    });
    const invalidA = generatedAssetVariant(states.completed, "invalid-a", { createdAt: "later" });
    const invalidB = generatedAssetVariant(states.completed, "invalid-b", { createdAt: "" });
    const newest = generatedAssetVariant(states.completed, "newest", {
      createdAt: "2026-09-13T00:00:00+02:00",
    });
    const assets = [invalidA, equalA, invalidB, equalB, newest, ...Object.values(states)];
    const projectForRecent = { ...fixtureProject(), generatedAssets: assets };
    expect({
      sorted: sortGeneratedAssetsByCreatedAtDesc(assets).map((asset) => asset.id),
      recent: recentGeneratedAssets(projectForRecent).map((asset) => asset.id),
      recentLeavesInputOrder: projectForRecent.generatedAssets.map((asset) => asset.id),
      empty: sortGeneratedAssetsByCreatedAtDesc([]),
    }).toMatchInlineSnapshot(`
      {
        "empty": [],
        "recent": [
          "asset-failed",
          "invalid-a",
          "newest",
        ],
        "recentLeavesInputOrder": [
          "invalid-a",
          "equal-a",
          "invalid-b",
          "equal-b",
          "newest",
          "asset-pending",
          "asset-running",
          "sample-generated-shot",
          "asset-failed",
          "asset-cancelled",
          "asset-replaced-output",
          "asset-with-references",
        ],
        "sorted": [
          "newest",
          "asset-pending",
          "equal-a",
          "equal-b",
          "asset-running",
          "sample-generated-shot",
          "asset-replaced-output",
          "asset-with-references",
          "invalid-a",
          "invalid-b",
          "asset-failed",
          "asset-cancelled",
        ],
      }
    `);
  });

  it("formats workflow labels and prompt excerpts", () => {
    const jobs: ProjectJobSummary[] = [
      { id: "job-1", kind: "generate_media", status: "running", updatedAt: "" },
      { id: "job-2", kind: "export-project__bundle", status: "progress", updatedAt: "" },
      { id: "job-3", kind: "_", status: "completed", updatedAt: "" },
    ];
    expect({
      tokens: ["generate_media", "a-b_c", "--", "  spaced_value  "].map(humanizeWorkflowToken),
      workflows: jobs.map(generatedWorkflowLabel),
      excerpts: [
        "",
        "  short\n\tprompt  ",
        "x".repeat(130),
        "y".repeat(131),
        `${"word ".repeat(40)}end`,
      ].map(promptExcerpt),
    }).toMatchInlineSnapshot(`
      {
        "excerpts": [
          "",
          "short prompt",
          "xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx",
          "yyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyy...",
          "word word word word word word word word word word word word word word word word word word word word word word word word word wo...",
        ],
        "tokens": [
          "generate media",
          "a b c",
          "",
          "spaced value",
        ],
        "workflows": [
          "Workflow running - generate media",
          "Workflow progress - export project bundle",
          "Workflow completed - ",
        ],
      }
    `);
  });

  it("labels selected generated clip settings", () => {
    const settings: Array<GeneratedAssetSettings | null | undefined> = [
      null,
      undefined,
      fixtureGeneratedAsset(fixtureProject()).settings,
      {
        width: 1920.6,
        height: 1079.4,
        durationSeconds: 2.25,
        fps: 29.97,
        aspectRatio: "  9:16  ",
      },
      {
        width: 0,
        height: 1080,
        durationSeconds: 0,
        fps: Number.NaN,
        aspectRatio: "   ",
      },
      {
        width: Number.POSITIVE_INFINITY,
        height: 720,
        durationSeconds: 10,
        fps: 24,
        aspectRatio: null,
      },
    ];
    expect(settings.map(selectedGeneratedClipSettingsLabels)).toMatchInlineSnapshot(`
      [
        [],
        [],
        [
          "640x360",
          "4s",
          "24 fps",
          "16:9",
        ],
        [
          "1921x1079",
          "2.3s",
          "29.97 fps",
          "9:16",
        ],
        [],
        [
          "10s",
          "24 fps",
        ],
      ]
    `);
  });
});
