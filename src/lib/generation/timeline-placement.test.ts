import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { MediaGenerationRequest } from "@/lib/generation/types";
import type {
  GeneratedAsset,
  GeneratedAssetSettings,
  GenerationPlacementIntent,
  MediaAsset,
  VideoProject,
} from "@/lib/project";
import {
  fixtureGeneratedAsset,
  fixtureItem,
  fixtureMedia,
  fixtureProject,
  fixtureTrack,
} from "@/test-utils/editor-fixtures";
import {
  generatedAssetPlacementContextLabel,
  generatedAssetPlacementIntent,
  generatedComposerPlacementForItem,
  generatedOutputMediaType,
  generatedOutputTimelineActions,
  generatedOutputTimelineItemId,
  generatedPlacementLabel,
  generatedReplacementPlacementIntent,
  generatedTimelineOutputItem,
  generatedTimelineOutputLabel,
  generatedTimelinePlaceholderAction,
  generatedTimelinePlaceholderForAsset,
  generatedTimelinePlaceholderItemId,
  generatedTimelineStartSecondsFromSettings,
  mediaGenerationTargetKind,
  mediaTimelineAction,
  timelineGenerationSourceRange,
  timelineGenerationTargetForKind,
  timelineGenerationTargets,
  timelineGenerationTrackForKind,
  timelineHasVisualSourceInRange,
  timelineItemAcceptsGeneratedOutputMedia,
  timelineVisualSourceEndSeconds,
} from "@/lib/generation/timeline-placement";

const placementIntents: Array<GenerationPlacementIntent | null | undefined> = [
  "library",
  "timeline",
  "replace:item-1",
  "replace:",
  null,
  undefined,
];

function mediaAsset(overrides: Partial<MediaAsset> & Pick<MediaAsset, "id">): MediaAsset {
  return {
    relativePath: `media/${overrides.id}.mp4`,
    kind: "video",
    durationSeconds: 4,
    width: 1280,
    height: 720,
    fps: 30,
    folderId: null,
    ...overrides,
  };
}

function request(overrides: Partial<MediaGenerationRequest> = {}): MediaGenerationRequest {
  return {
    kind: "video",
    name: "  Hero shot  ",
    targetFolderId: null,
    placementIntent: "timeline",
    prompt: "A hero shot",
    model: { provider: "fal.ai", id: "fal-ai/wan-25-preview/text-to-video" },
    references: { mediaIds: [], firstFrameMediaId: null, lastFrameMediaId: null },
    settings: {
      width: 1280,
      height: 720,
      durationSeconds: 5.00049,
      fps: 24,
      aspectRatio: "16:9",
    },
    ...overrides,
  };
}

function projectWithGeneratedOutput(settings: Partial<GeneratedAssetSettings> = {}) {
  const project = fixtureProject();
  const base = fixtureGeneratedAsset(project);
  const asset: GeneratedAsset = {
    ...structuredClone(base),
    id: "asset-output",
    name: "   ",
    settings: { ...base.settings, ...settings },
    outputs: [
      {
        mediaId: "Generated Output #1",
        relativePath: "generated/output-1.mp4",
        width: 1280,
        height: 720,
        durationSeconds: 3.3333,
        fps: 24,
      },
      {
        mediaId: "generated-still",
        relativePath: "generated/still.webp",
        width: 1024,
        height: 1024,
        durationSeconds: 2,
        fps: 0,
      },
      {
        mediaId: "generated-zero",
        relativePath: "generated/zero.mp4",
        width: 1280,
        height: 720,
        durationSeconds: 0,
        fps: 24,
      },
      {
        mediaId: "generated-voice",
        relativePath: "generated/voice.wav",
        width: 0,
        height: 0,
        durationSeconds: 1.5,
        fps: 0,
      },
    ],
  };
  project.generatedAssets.push(asset);
  project.media.push(
    mediaAsset({ id: "Generated Output #1", kind: "generated", relativePath: "generated/output-1.mp4" }),
    mediaAsset({ id: "generated-still", kind: "generated", relativePath: "generated/still.webp?v=2" }),
    mediaAsset({ id: "generated-zero", kind: "video", relativePath: "generated/zero.mp4" }),
    mediaAsset({ id: "generated-voice", kind: "generated", relativePath: "generated/voice.wav" }),
  );
  return { project, asset };
}

function addPlaceholder(project: VideoProject, assetId: string, trackKind: "video" | "audio") {
  const placeholderAction = generatedTimelinePlaceholderAction(
    project,
    assetId,
    request({ kind: trackKind === "audio" ? "audio" : "video", settings: {
      width: null,
      height: null,
      durationSeconds: 2,
      fps: null,
      aspectRatio: null,
      timelineStartSeconds: 6.5,
    } }),
  );
  if (placeholderAction?.type !== "addItems") {
    throw new Error("Expected placeholder addItems action.");
  }
  const track = project.timeline.tracks.find((candidate) => candidate.id === placeholderAction.targetTrackId);
  track?.items.push(...placeholderAction.items);
}

describe("generation timeline placement characterization", () => {
  beforeEach(() => {
    vi.useFakeTimers();
    vi.setSystemTime(new Date("2026-09-13T00:00:00Z"));
  });

  afterEach(() => {
    vi.useRealTimers();
  });

  it("labels placement intents and compares placement variants", () => {
    const project = fixtureProject();
    const asset = fixtureGeneratedAsset(project);
    const item = fixtureItem(project, "video");
    expect({
      intents: placementIntents.map((placementIntent) => {
        const variant = { ...asset, placementIntent } as GeneratedAsset;
        return {
          placementIntent,
          assetIntent: generatedAssetPlacementIntent(variant),
          contextLabel: generatedAssetPlacementContextLabel(placementIntent),
          inspectorLabel: generatedPlacementLabel(variant),
          labelsAgree:
            generatedAssetPlacementContextLabel(placementIntent) === generatedPlacementLabel(variant),
        };
      }),
      nullAssetIntent: generatedAssetPlacementIntent(null),
      replacement: {
        workspace: generatedReplacementPlacementIntent(item.id),
        inspector: generatedComposerPlacementForItem(item),
        inspectorWithoutItem: generatedComposerPlacementForItem(null),
        workspaceEmptyId: generatedReplacementPlacementIntent(""),
      },
    }).toMatchInlineSnapshot(`
      {
        "intents": [
          {
            "assetIntent": "library",
            "contextLabel": "Library",
            "inspectorLabel": "Library",
            "labelsAgree": true,
            "placementIntent": "library",
          },
          {
            "assetIntent": "timeline",
            "contextLabel": "Timeline target",
            "inspectorLabel": "Timeline target",
            "labelsAgree": true,
            "placementIntent": "timeline",
          },
          {
            "assetIntent": "replace:item-1",
            "contextLabel": "Replacement target",
            "inspectorLabel": "Replacement target",
            "labelsAgree": true,
            "placementIntent": "replace:item-1",
          },
          {
            "assetIntent": "replace:",
            "contextLabel": "Replacement target",
            "inspectorLabel": "Replacement target",
            "labelsAgree": true,
            "placementIntent": "replace:",
          },
          {
            "assetIntent": "library",
            "contextLabel": null,
            "inspectorLabel": "Library",
            "labelsAgree": false,
            "placementIntent": null,
          },
          {
            "assetIntent": "library",
            "contextLabel": null,
            "inspectorLabel": "Library",
            "labelsAgree": false,
            "placementIntent": undefined,
          },
        ],
        "nullAssetIntent": "library",
        "replacement": {
          "inspector": "replace:item-1",
          "inspectorWithoutItem": "library",
          "workspace": "replace:item-1",
          "workspaceEmptyId": "replace:",
        },
      }
    `);
  });

  it("resolves generation tracks and targets", () => {
    const project = fixtureProject();
    const lockedProject = fixtureProject();
    for (const track of lockedProject.timeline.tracks) {
      track.locked = true;
    }
    const staggeredProject = fixtureProject();
    fixtureTrack(staggeredProject, "audio").items.push({
      id: "late-audio",
      kind: "audio_clip",
      startSeconds: 7.1234,
      durationSeconds: 1.00001,
      source: { type: "media", mediaId: "media-voiceover" },
      label: "Late audio",
      properties: {},
    });
    expect({
      track: {
        video: timelineGenerationTrackForKind(project, "video")?.id ?? null,
        audio: timelineGenerationTrackForKind(project, "audio")?.id ?? null,
        lockedVideo: timelineGenerationTrackForKind(lockedProject, "video"),
      },
      target: {
        video: timelineGenerationTargetForKind(project, "video"),
        audio: timelineGenerationTargetForKind(project, "audio"),
        staggeredAudio: timelineGenerationTargetForKind(staggeredProject, "audio"),
        locked: timelineGenerationTargetForKind(lockedProject, "audio"),
      },
      targets: timelineGenerationTargets(project),
      staggeredTargets: timelineGenerationTargets(staggeredProject),
      lockedTargets: timelineGenerationTargets(lockedProject),
      targetKinds: (["image", "video", "audio", "generated"] as const).map(mediaGenerationTargetKind),
    }).toMatchInlineSnapshot(`
      {
        "lockedTargets": {},
        "staggeredTargets": {
          "audio": {
            "startSeconds": 8.123,
            "trackName": "Audio",
          },
          "image": {
            "startSeconds": 8,
            "trackName": "Video",
          },
          "video": {
            "startSeconds": 8,
            "trackName": "Video",
          },
        },
        "target": {
          "audio": {
            "startSeconds": 4,
            "trackName": "Audio",
          },
          "locked": null,
          "staggeredAudio": {
            "startSeconds": 8.123,
            "trackName": "Audio",
          },
          "video": {
            "startSeconds": 8,
            "trackName": "Video",
          },
        },
        "targetKinds": [
          "video",
          "video",
          "audio",
          "video",
        ],
        "targets": {
          "audio": {
            "startSeconds": 4,
            "trackName": "Audio",
          },
          "image": {
            "startSeconds": 8,
            "trackName": "Video",
          },
          "video": {
            "startSeconds": 8,
            "trackName": "Video",
          },
        },
        "track": {
          "audio": "track-audio",
          "lockedVideo": null,
          "video": "track-video",
        },
      }
    `);
  });

  it("reads generated start seconds and output media types", () => {
    const base: GeneratedAssetSettings = {
      width: null,
      height: null,
      durationSeconds: null,
      fps: null,
      aspectRatio: null,
    };
    const media = [
      mediaAsset({ id: "audio", kind: "audio" }),
      mediaAsset({ id: "image", kind: "image" }),
      mediaAsset({ id: "video", kind: "video", relativePath: "media/video.mp3" }),
      mediaAsset({ id: "generated-mp3", kind: "generated", relativePath: "generated/a.MP3" }),
      mediaAsset({ id: "generated-query", kind: "generated", relativePath: "generated/a.opus?token=x.png" }),
      mediaAsset({ id: "generated-hash", kind: "generated", relativePath: "generated/a.jpeg#frame.wav" }),
      mediaAsset({ id: "generated-avif", kind: "generated", relativePath: "generated/a.avif" }),
      mediaAsset({ id: "generated-none", kind: "generated", relativePath: "generated/no-extension" }),
      mediaAsset({ id: "generated-mov", kind: "generated", relativePath: "generated/a.mov" }),
    ];
    const audioItem = fixtureItem(fixtureProject(), "audio");
    const videoItem = fixtureItem(fixtureProject(), "video");
    expect({
      startSeconds: [
        generatedTimelineStartSecondsFromSettings(base, 9),
        generatedTimelineStartSecondsFromSettings({ ...base, timelineStartSeconds: 1.23456 }, 9),
        generatedTimelineStartSecondsFromSettings({ ...base, videoSourceStartSeconds: 2.5 }, 9),
        generatedTimelineStartSecondsFromSettings(
          { ...base, timelineStartSeconds: null, videoSourceStartSeconds: 3 },
          9,
        ),
        generatedTimelineStartSecondsFromSettings(
          { ...base, timelineStartSeconds: 0, videoSourceStartSeconds: 3 },
          9,
        ),
        generatedTimelineStartSecondsFromSettings({ ...base, timelineStartSeconds: -1 }, 9),
        generatedTimelineStartSecondsFromSettings(
          { ...base, timelineStartSeconds: Number.POSITIVE_INFINITY },
          9,
        ),
      ],
      media: media.map((asset) => ({
        id: asset.id,
        type: generatedOutputMediaType(asset),
        acceptedByAudioItem: timelineItemAcceptsGeneratedOutputMedia(audioItem, asset),
        acceptedByVideoItem: timelineItemAcceptsGeneratedOutputMedia(videoItem, asset),
      })),
      missingItem: timelineItemAcceptsGeneratedOutputMedia(null, media[0] ?? null),
      missingMedia: timelineItemAcceptsGeneratedOutputMedia(videoItem, null),
    }).toMatchInlineSnapshot(`
      {
        "media": [
          {
            "acceptedByAudioItem": true,
            "acceptedByVideoItem": false,
            "id": "audio",
            "type": "audio",
          },
          {
            "acceptedByAudioItem": false,
            "acceptedByVideoItem": true,
            "id": "image",
            "type": "image",
          },
          {
            "acceptedByAudioItem": false,
            "acceptedByVideoItem": true,
            "id": "video",
            "type": "video",
          },
          {
            "acceptedByAudioItem": true,
            "acceptedByVideoItem": false,
            "id": "generated-mp3",
            "type": "audio",
          },
          {
            "acceptedByAudioItem": true,
            "acceptedByVideoItem": false,
            "id": "generated-query",
            "type": "audio",
          },
          {
            "acceptedByAudioItem": false,
            "acceptedByVideoItem": true,
            "id": "generated-hash",
            "type": "image",
          },
          {
            "acceptedByAudioItem": false,
            "acceptedByVideoItem": true,
            "id": "generated-avif",
            "type": "image",
          },
          {
            "acceptedByAudioItem": false,
            "acceptedByVideoItem": true,
            "id": "generated-none",
            "type": "video",
          },
          {
            "acceptedByAudioItem": false,
            "acceptedByVideoItem": true,
            "id": "generated-mov",
            "type": "video",
          },
        ],
        "missingItem": false,
        "missingMedia": false,
        "startSeconds": [
          9,
          1.235,
          2.5,
          3,
          0,
          9,
          9,
        ],
      }
    `);
  });

  it("builds generated timeline placeholders", () => {
    const project = fixtureProject();
    const lockedAudioProject = fixtureProject();
    fixtureTrack(lockedAudioProject, "audio").locked = true;
    expect({
      ids: ["Asset 1", "  --  ", "asset_ÄB"].map(generatedTimelinePlaceholderItemId),
      videoRequest: generatedTimelinePlaceholderAction(project, "Asset Video", request()),
      audioRequest: generatedTimelinePlaceholderAction(
        project,
        "asset-audio",
        request({
          kind: "audio",
          name: "   ",
          settings: {
            width: null,
            height: null,
            durationSeconds: 3,
            fps: null,
            aspectRatio: null,
            timelineStartSeconds: 1.5,
          },
        }),
      ),
      unnamedVideoRequest: generatedTimelinePlaceholderAction(
        project,
        "asset-unnamed",
        request({ name: null, kind: "image", settings: { ...request().settings, videoSourceStartSeconds: 2 } }),
      ),
      replaceRequest: generatedTimelinePlaceholderAction(
        project,
        "asset-replace",
        request({ placementIntent: "replace:item-1" }),
      ),
      libraryRequest: generatedTimelinePlaceholderAction(
        project,
        "asset-library",
        request({ placementIntent: "library" }),
      ),
      nullDuration: generatedTimelinePlaceholderAction(
        project,
        "asset-null-duration",
        request({ settings: { ...request().settings, durationSeconds: null } }),
      ),
      zeroDuration: generatedTimelinePlaceholderAction(
        project,
        "asset-zero-duration",
        request({ settings: { ...request().settings, durationSeconds: 0.0004 } }),
      ),
      lockedAudioTrack: generatedTimelinePlaceholderAction(
        lockedAudioProject,
        "asset-locked",
        request({ kind: "audio" }),
      ),
    }).toMatchInlineSnapshot(`
      {
        "audioRequest": {
          "items": [
            {
              "durationSeconds": 3,
              "id": "generated-placeholder-asset-audio",
              "kind": "audio_clip",
              "label": "Queued audio",
              "properties": {
                "generatedAssetId": "asset-audio",
                "generatedTimelinePlaceholder": true,
                "sourceIn": 0,
                "sourceOut": 3,
              },
              "source": {
                "artifactId": "asset-audio",
                "type": "generated",
              },
              "startSeconds": 1.5,
            },
          ],
          "targetTrackId": "track-audio",
          "type": "addItems",
        },
        "ids": [
          "generated-placeholder-asset-1",
          "generated-placeholder-asset",
          "generated-placeholder-asset-b",
        ],
        "libraryRequest": null,
        "lockedAudioTrack": null,
        "nullDuration": null,
        "replaceRequest": null,
        "unnamedVideoRequest": {
          "items": [
            {
              "durationSeconds": 5,
              "id": "generated-placeholder-asset-unnamed",
              "kind": "video_clip",
              "label": "Queued generation",
              "properties": {
                "generatedAssetId": "asset-unnamed",
                "generatedTimelinePlaceholder": true,
                "sourceIn": 0,
                "sourceOut": 5,
              },
              "source": {
                "artifactId": "asset-unnamed",
                "type": "generated",
              },
              "startSeconds": 2,
            },
          ],
          "targetTrackId": "track-video",
          "type": "addItems",
        },
        "videoRequest": {
          "items": [
            {
              "durationSeconds": 5,
              "id": "generated-placeholder-asset-video",
              "kind": "video_clip",
              "label": "Hero shot",
              "properties": {
                "generatedAssetId": "Asset Video",
                "generatedTimelinePlaceholder": true,
                "sourceIn": 0,
                "sourceOut": 5,
              },
              "source": {
                "artifactId": "Asset Video",
                "type": "generated",
              },
              "startSeconds": 8,
            },
          ],
          "targetTrackId": "track-video",
          "type": "addItems",
        },
        "zeroDuration": null,
      }
    `);
  });

  it("finds generated placeholders for assets", () => {
    const project = fixtureProject();
    addPlaceholder(project, "asset-placeholder", "audio");
    fixtureTrack(project, "video").items.push({
      id: "not-a-placeholder",
      kind: "video_clip",
      startSeconds: 9,
      durationSeconds: 1,
      source: { type: "generated", artifactId: "asset-flag-string" },
      label: "Flag string",
      properties: { generatedAssetId: "asset-flag-string", generatedTimelinePlaceholder: "true" },
    });
    const placeholder = generatedTimelinePlaceholderForAsset(project, "asset-placeholder");
    expect({
      placeholder: placeholder ? { track: placeholder.track.id, item: placeholder.item } : null,
      flagString: generatedTimelinePlaceholderForAsset(project, "asset-flag-string"),
      missing: generatedTimelinePlaceholderForAsset(project, "missing"),
    }).toMatchInlineSnapshot(`
      {
        "flagString": null,
        "missing": null,
        "placeholder": {
          "item": {
            "durationSeconds": 2,
            "id": "generated-placeholder-asset-placeholder",
            "kind": "audio_clip",
            "label": "Hero shot",
            "properties": {
              "generatedAssetId": "asset-placeholder",
              "generatedTimelinePlaceholder": true,
              "sourceIn": 0,
              "sourceOut": 2,
            },
            "source": {
              "artifactId": "asset-placeholder",
              "type": "generated",
            },
            "startSeconds": 6.5,
          },
          "track": "track-audio",
        },
      }
    `);
  });

  it("builds generated timeline output items and labels", () => {
    const { project, asset } = projectWithGeneratedOutput();
    const media = project.media.find((candidate) => candidate.id === "Generated Output #1");
    if (!media) {
      throw new Error("Expected generated output media.");
    }
    const namedAsset = { ...asset, name: "  Named output  " };
    expect({
      ids: [
        generatedOutputTimelineItemId("Generated Output #1"),
        generatedOutputTimelineItemId("Generated Output #1", "audio"),
        generatedOutputTimelineItemId("  ###  ", "primary"),
      ],
      labels: [generatedTimelineOutputLabel(asset, media), generatedTimelineOutputLabel(namedAsset, media)],
      videoItem: generatedTimelineOutputItem(asset, media, "item-video", "video_clip", 1, 2),
      imageItem: generatedTimelineOutputItem(namedAsset, media, "item-image", "image_clip", 0, 3, "link-1"),
      audioItem: generatedTimelineOutputItem(asset, media, "item-audio", "audio_clip", 2, 1.5, ""),
    }).toMatchInlineSnapshot(`
      {
        "audioItem": {
          "durationSeconds": 1.5,
          "id": "item-audio",
          "kind": "audio_clip",
          "label": "Generated Output #1",
          "properties": {
            "generatedAssetId": "asset-output",
            "generatedOutputMediaId": "Generated Output #1",
            "sourceIn": 0,
            "sourceOut": 1.5,
          },
          "source": {
            "mediaId": "Generated Output #1",
            "type": "media",
          },
          "startSeconds": 2,
        },
        "ids": [
          "generated-output-generated-output-1-mtz1s000",
          "generated-output-generated-output-1-audio-mtz1s000",
          "generated-output-media-mtz1s000",
        ],
        "imageItem": {
          "durationSeconds": 3,
          "id": "item-image",
          "kind": "image_clip",
          "label": "Named output",
          "properties": {
            "generatedAssetId": "asset-output",
            "generatedOutputMediaId": "Generated Output #1",
            "linkGroupId": "link-1",
          },
          "source": {
            "mediaId": "Generated Output #1",
            "type": "media",
          },
          "startSeconds": 0,
        },
        "labels": [
          "Generated Output #1",
          "Named output",
        ],
        "videoItem": {
          "durationSeconds": 2,
          "id": "item-video",
          "kind": "video_clip",
          "label": "Generated Output #1",
          "properties": {
            "generatedAssetId": "asset-output",
            "generatedOutputMediaId": "Generated Output #1",
            "sourceIn": 0,
            "sourceOut": 2,
          },
          "source": {
            "mediaId": "Generated Output #1",
            "type": "media",
          },
          "startSeconds": 1,
        },
      }
    `);
  });

  it("builds generated output timeline actions without placeholders", () => {
    const { project } = projectWithGeneratedOutput();
    const lockedProject = projectWithGeneratedOutput().project;
    fixtureTrack(lockedProject, "video").locked = true;
    expect({
      video: generatedOutputTimelineActions(project, "Generated Output #1"),
      image: generatedOutputTimelineActions(project, "generated-still"),
      audio: generatedOutputTimelineActions(project, "generated-voice"),
      zeroDuration: generatedOutputTimelineActions(project, "generated-zero"),
      sampleOutput: generatedOutputTimelineActions(project, "sample-generated-output"),
      notGeneratedMedia: generatedOutputTimelineActions(project, "media-1"),
      missingMedia: generatedOutputTimelineActions(project, "missing"),
      lockedVideoTrack: generatedOutputTimelineActions(lockedProject, "Generated Output #1"),
    }).toMatchInlineSnapshot(`
      {
        "audio": {
          "actions": [
            {
              "items": [
                {
                  "durationSeconds": 1.5,
                  "id": "generated-output-generated-voice-mtz1s000",
                  "kind": "audio_clip",
                  "label": "generated-voice",
                  "properties": {
                    "generatedAssetId": "asset-output",
                    "generatedOutputMediaId": "generated-voice",
                    "sourceIn": 0,
                    "sourceOut": 1.5,
                  },
                  "source": {
                    "mediaId": "generated-voice",
                    "type": "media",
                  },
                  "startSeconds": 4,
                },
              ],
              "targetTrackId": "track-audio",
              "type": "addItems",
            },
          ],
          "itemId": "generated-output-generated-voice-mtz1s000",
        },
        "image": {
          "actions": [
            {
              "items": [
                {
                  "durationSeconds": 2,
                  "id": "generated-output-generated-still-mtz1s000",
                  "kind": "image_clip",
                  "label": "generated-still",
                  "properties": {
                    "generatedAssetId": "asset-output",
                    "generatedOutputMediaId": "generated-still",
                  },
                  "source": {
                    "mediaId": "generated-still",
                    "type": "media",
                  },
                  "startSeconds": 8,
                },
              ],
              "targetTrackId": "track-video",
              "type": "addItems",
            },
          ],
          "itemId": "generated-output-generated-still-mtz1s000",
        },
        "lockedVideoTrack": null,
        "missingMedia": null,
        "notGeneratedMedia": null,
        "sampleOutput": {
          "actions": [
            {
              "items": [
                {
                  "durationSeconds": 4,
                  "id": "generated-output-sample-generated-output-mtz1s000",
                  "kind": "video_clip",
                  "label": "Bundled Edison restoration",
                  "properties": {
                    "generatedAssetId": "sample-generated-shot",
                    "generatedOutputMediaId": "sample-generated-output",
                    "sourceIn": 0,
                    "sourceOut": 4,
                  },
                  "source": {
                    "mediaId": "sample-generated-output",
                    "type": "media",
                  },
                  "startSeconds": 8,
                },
              ],
              "targetTrackId": "track-video",
              "type": "addItems",
            },
          ],
          "itemId": "generated-output-sample-generated-output-mtz1s000",
        },
        "video": {
          "actions": [
            {
              "items": [
                {
                  "durationSeconds": 3.333,
                  "id": "generated-output-generated-output-1-mtz1s000",
                  "kind": "video_clip",
                  "label": "Generated Output #1",
                  "properties": {
                    "generatedAssetId": "asset-output",
                    "generatedOutputMediaId": "Generated Output #1",
                    "sourceIn": 0,
                    "sourceOut": 3.333,
                  },
                  "source": {
                    "mediaId": "Generated Output #1",
                    "type": "media",
                  },
                  "startSeconds": 8,
                },
              ],
              "targetTrackId": "track-video",
              "type": "addItems",
            },
          ],
          "itemId": "generated-output-generated-output-1-mtz1s000",
        },
        "zeroDuration": null,
      }
    `);
  });

  it("builds generated output timeline actions with settings, placeholders and audio", () => {
    const withStart = projectWithGeneratedOutput({ timelineStartSeconds: 2.25 }).project;
    const withAudio = projectWithGeneratedOutput({ generateAudio: true }).project;
    const withAudioLocked = projectWithGeneratedOutput({ generateAudio: true }).project;
    fixtureTrack(withAudioLocked, "audio").locked = true;
    const withPlaceholder = projectWithGeneratedOutput({ generateAudio: true }).project;
    addPlaceholder(withPlaceholder, "asset-output", "video");
    expect({
      withStart: generatedOutputTimelineActions(withStart, "Generated Output #1"),
      videoWithAudio: generatedOutputTimelineActions(withAudio, "Generated Output #1"),
      imageWithAudioSetting: generatedOutputTimelineActions(withAudio, "generated-still"),
      videoWithAudioLockedAudioTrack: generatedOutputTimelineActions(withAudioLocked, "Generated Output #1"),
      placeholder: generatedOutputTimelineActions(withPlaceholder, "Generated Output #1"),
    }).toMatchInlineSnapshot(`
      {
        "imageWithAudioSetting": {
          "actions": [
            {
              "items": [
                {
                  "durationSeconds": 2,
                  "id": "generated-output-generated-still-mtz1s000",
                  "kind": "image_clip",
                  "label": "generated-still",
                  "properties": {
                    "generatedAssetId": "asset-output",
                    "generatedOutputMediaId": "generated-still",
                  },
                  "source": {
                    "mediaId": "generated-still",
                    "type": "media",
                  },
                  "startSeconds": 8,
                },
              ],
              "targetTrackId": "track-video",
              "type": "addItems",
            },
          ],
          "itemId": "generated-output-generated-still-mtz1s000",
        },
        "placeholder": {
          "actions": [
            {
              "itemIds": [
                "generated-placeholder-asset-output",
              ],
              "type": "removeItems",
            },
            {
              "items": [
                {
                  "durationSeconds": 2,
                  "id": "generated-output-generated-output-1-mtz1s000",
                  "kind": "video_clip",
                  "label": "Generated Output #1",
                  "properties": {
                    "generatedAssetId": "asset-output",
                    "generatedOutputMediaId": "Generated Output #1",
                    "linkGroupId": "link-generated-output-generated-output-1-mtz1s000",
                    "sourceIn": 0,
                    "sourceOut": 2,
                  },
                  "source": {
                    "mediaId": "Generated Output #1",
                    "type": "media",
                  },
                  "startSeconds": 6.5,
                },
              ],
              "targetTrackId": "track-video",
              "type": "addItems",
            },
            {
              "items": [
                {
                  "durationSeconds": 2,
                  "id": "generated-output-generated-output-1-audio-mtz1s000",
                  "kind": "audio_clip",
                  "label": "Generated Output #1",
                  "properties": {
                    "generatedAssetId": "asset-output",
                    "generatedOutputMediaId": "Generated Output #1",
                    "linkGroupId": "link-generated-output-generated-output-1-mtz1s000",
                    "sourceIn": 0,
                    "sourceOut": 2,
                  },
                  "source": {
                    "mediaId": "Generated Output #1",
                    "type": "media",
                  },
                  "startSeconds": 6.5,
                },
              ],
              "targetTrackId": "track-audio",
              "type": "addItems",
            },
          ],
          "itemId": "generated-output-generated-output-1-mtz1s000",
        },
        "videoWithAudio": {
          "actions": [
            {
              "items": [
                {
                  "durationSeconds": 3.333,
                  "id": "generated-output-generated-output-1-mtz1s000",
                  "kind": "video_clip",
                  "label": "Generated Output #1",
                  "properties": {
                    "generatedAssetId": "asset-output",
                    "generatedOutputMediaId": "Generated Output #1",
                    "linkGroupId": "link-generated-output-generated-output-1-mtz1s000",
                    "sourceIn": 0,
                    "sourceOut": 3.333,
                  },
                  "source": {
                    "mediaId": "Generated Output #1",
                    "type": "media",
                  },
                  "startSeconds": 8,
                },
              ],
              "targetTrackId": "track-video",
              "type": "addItems",
            },
            {
              "items": [
                {
                  "durationSeconds": 3.333,
                  "id": "generated-output-generated-output-1-audio-mtz1s000",
                  "kind": "audio_clip",
                  "label": "Generated Output #1",
                  "properties": {
                    "generatedAssetId": "asset-output",
                    "generatedOutputMediaId": "Generated Output #1",
                    "linkGroupId": "link-generated-output-generated-output-1-mtz1s000",
                    "sourceIn": 0,
                    "sourceOut": 3.333,
                  },
                  "source": {
                    "mediaId": "Generated Output #1",
                    "type": "media",
                  },
                  "startSeconds": 8,
                },
              ],
              "targetTrackId": "track-audio",
              "type": "addItems",
            },
          ],
          "itemId": "generated-output-generated-output-1-mtz1s000",
        },
        "videoWithAudioLockedAudioTrack": {
          "actions": [
            {
              "items": [
                {
                  "durationSeconds": 3.333,
                  "id": "generated-output-generated-output-1-mtz1s000",
                  "kind": "video_clip",
                  "label": "Generated Output #1",
                  "properties": {
                    "generatedAssetId": "asset-output",
                    "generatedOutputMediaId": "Generated Output #1",
                    "sourceIn": 0,
                    "sourceOut": 3.333,
                  },
                  "source": {
                    "mediaId": "Generated Output #1",
                    "type": "media",
                  },
                  "startSeconds": 8,
                },
              ],
              "targetTrackId": "track-video",
              "type": "addItems",
            },
          ],
          "itemId": "generated-output-generated-output-1-mtz1s000",
        },
        "withStart": {
          "actions": [
            {
              "items": [
                {
                  "durationSeconds": 3.333,
                  "id": "generated-output-generated-output-1-mtz1s000",
                  "kind": "video_clip",
                  "label": "Generated Output #1",
                  "properties": {
                    "generatedAssetId": "asset-output",
                    "generatedOutputMediaId": "Generated Output #1",
                    "sourceIn": 0,
                    "sourceOut": 3.333,
                  },
                  "source": {
                    "mediaId": "Generated Output #1",
                    "type": "media",
                  },
                  "startSeconds": 2.25,
                },
              ],
              "targetTrackId": "track-video",
              "type": "addItems",
            },
          ],
          "itemId": "generated-output-generated-output-1-mtz1s000",
        },
      }
    `);
  });

  it("derives timeline generation source ranges", () => {
    const project = fixtureProject();
    const gapProject = fixtureProject();
    fixtureTrack(gapProject, "video").items.push({
      id: "late-clip",
      kind: "image_clip",
      startSeconds: 12,
      durationSeconds: 2.5,
      source: { type: "media", mediaId: "media-1" },
      label: "Late clip",
      properties: {},
    });
    const noVisualProject = fixtureProject();
    fixtureTrack(noVisualProject, "video").items = [];
    const lockedProject = fixtureProject();
    fixtureTrack(lockedProject, "video").locked = true;
    expect({
      nullRange: timelineGenerationSourceRange(project, null),
      visualRange: timelineGenerationSourceRange(project, { startSeconds: 1.00004, endSeconds: 3.5 }),
      gapRange: timelineGenerationSourceRange(gapProject, { startSeconds: 8.5, endSeconds: 11 }),
      invertedRange: timelineGenerationSourceRange(project, { startSeconds: 3, endSeconds: 3 }),
      gapProjectNullRange: timelineGenerationSourceRange(gapProject, null),
      noVisualNullRange: timelineGenerationSourceRange(noVisualProject, null),
      lockedNullRange: timelineGenerationSourceRange(lockedProject, { startSeconds: 0, endSeconds: 2 }),
      visualEnd: {
        project: timelineVisualSourceEndSeconds(project),
        gapProject: timelineVisualSourceEndSeconds(gapProject),
        noVisual: timelineVisualSourceEndSeconds(noVisualProject),
        locked: timelineVisualSourceEndSeconds(lockedProject),
      },
      hasVisual: [
        timelineHasVisualSourceInRange(project, 0, 8),
        timelineHasVisualSourceInRange(project, 8, 9),
        timelineHasVisualSourceInRange(gapProject, 8.5, 12),
        timelineHasVisualSourceInRange(gapProject, 8.5, 12.01),
        timelineHasVisualSourceInRange(lockedProject, 0, 8),
      ],
    }).toMatchInlineSnapshot(`
      {
        "gapProjectNullRange": {
          "endSeconds": 14.5,
          "label": "Whole timeline",
          "startSeconds": 0,
        },
        "gapRange": {
          "endSeconds": 14.5,
          "label": "Whole timeline",
          "startSeconds": 0,
        },
        "hasVisual": [
          true,
          false,
          false,
          true,
          false,
        ],
        "invertedRange": {
          "endSeconds": 8,
          "label": "Whole timeline",
          "startSeconds": 0,
        },
        "lockedNullRange": null,
        "noVisualNullRange": null,
        "nullRange": {
          "endSeconds": 8,
          "label": "Whole timeline",
          "startSeconds": 0,
        },
        "visualEnd": {
          "gapProject": 14.5,
          "locked": 0,
          "noVisual": 0,
          "project": 8,
        },
        "visualRange": {
          "endSeconds": 3.5,
          "label": "Selected timeline range",
          "startSeconds": 1,
        },
      }
    `);
  });

  it("builds media timeline actions", () => {
    const project = fixtureProject();
    project.media.push(
      mediaAsset({ id: "Still Image", kind: "image", durationSeconds: 0, relativePath: "media/still.png" }),
    );
    const lockedProject = fixtureProject();
    fixtureTrack(lockedProject, "audio").locked = true;
    expect({
      video: mediaTimelineAction(project, fixtureMedia(project, "video").id),
      audio: mediaTimelineAction(project, fixtureMedia(project, "audio").id),
      image: mediaTimelineAction(project, "Still Image"),
      missing: mediaTimelineAction(project, "missing-media"),
      lockedAudioTrack: mediaTimelineAction(lockedProject, "media-voiceover"),
    }).toMatchInlineSnapshot(`
      {
        "audio": {
          "action": {
            "items": [
              {
                "durationSeconds": 4,
                "id": "timeline-media-voiceover",
                "kind": "audio_clip",
                "label": "voiceover.m4a",
                "properties": {
                  "sourceIn": 0,
                  "sourceOut": 4,
                },
                "source": {
                  "mediaId": "media-voiceover",
                  "type": "media",
                },
                "startSeconds": 4,
              },
            ],
            "targetTrackId": "track-audio",
            "type": "addItems",
          },
          "itemId": "timeline-media-voiceover",
        },
        "image": {
          "action": {
            "items": [
              {
                "durationSeconds": 4,
                "id": "timeline-still-image",
                "kind": "video_clip",
                "label": "still.png",
                "properties": {
                  "sourceIn": 0,
                  "sourceOut": 4,
                },
                "source": {
                  "mediaId": "Still Image",
                  "type": "media",
                },
                "startSeconds": 8,
              },
            ],
            "targetTrackId": "track-video",
            "type": "addItems",
          },
          "itemId": "timeline-still-image",
        },
        "lockedAudioTrack": null,
        "missing": null,
        "video": {
          "action": {
            "items": [
              {
                "durationSeconds": 4,
                "id": "timeline-media-1",
                "kind": "video_clip",
                "label": "input.mp4",
                "properties": {
                  "sourceIn": 0,
                  "sourceOut": 4,
                },
                "source": {
                  "mediaId": "media-1",
                  "type": "media",
                },
                "startSeconds": 8,
              },
            ],
            "targetTrackId": "track-video",
            "type": "addItems",
          },
          "itemId": "timeline-media-1",
        },
      }
    `);
  });
});
