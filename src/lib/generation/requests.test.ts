import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { MediaAsset } from "@/lib/project";
import {
  fixtureGeneratedAsset,
  fixtureItem,
  fixtureMedia,
  fixtureProject,
} from "@/test-utils/editor-fixtures";
import {
  generatedMediaAssetId,
  generatedUpscaleLimitReasonForAsset,
  generatedVariationId,
  generatedVariationSetId,
  importedUpscaleLimitReasonForMedia,
  importedVideoEditLimitReasonForMedia,
  sourceClipGenerationContextForItem,
  sourceClipUpscaleContextForItem,
  upscaleGenerationPrompt,
  upscaleGenerationRequest,
  videoAudioGenerationRequest,
  videoToMusicGenerationPrompt,
  videoToSfxGenerationPrompt,
} from "@/lib/generation/requests";
import type { TimelineItem } from "@/lib/timeline";

function imageMedia(overrides: Partial<MediaAsset> = {}): MediaAsset {
  return {
    id: "media-still",
    relativePath: "media/still.png",
    kind: "image",
    durationSeconds: 0,
    width: 1024,
    height: 768,
    fps: null,
    folderId: null,
    ...overrides,
  };
}

describe("generation request characterization", () => {
  beforeEach(() => {
    vi.useFakeTimers();
    vi.setSystemTime(new Date("2026-09-13T00:00:00Z"));
  });

  afterEach(() => {
    vi.useRealTimers();
  });

  it("exposes the fixed generation prompts", () => {
    expect({
      upscaleGenerationPrompt,
      videoToMusicGenerationPrompt,
      videoToSfxGenerationPrompt,
    }).toMatchInlineSnapshot(`
      {
        "upscaleGenerationPrompt": "Upscale this clip while preserving composition, timing, motion, and subject details.",
        "videoToMusicGenerationPrompt": "Generate music that fits the video.",
        "videoToSfxGenerationPrompt": "Create matching sound for the video.",
      }
    `);
  });

  it("derives time-based generated ids", () => {
    expect({
      variation: generatedVariationId("asset-1"),
      variationSet: [0, 1, 9].map((index) => generatedVariationSetId("asset-1", index)),
      media: generatedMediaAssetId(),
    }).toMatchInlineSnapshot(`
      {
        "media": "generated-media-mtz1s000",
        "variation": "asset-1-variation-mtz1s000",
        "variationSet": [
          "asset-1-variation-mtz1s000-1",
          "asset-1-variation-mtz1s000-2",
          "asset-1-variation-mtz1s000-10",
        ],
      }
    `);
  });

  it("builds image upscale requests with and without context", () => {
    const project = fixtureProject();
    const unsizedImage = imageMedia({ id: "media-unsized", width: null, height: 0 });
    expect({
      image: upscaleGenerationRequest(imageMedia()),
      imageWithContext: upscaleGenerationRequest(imageMedia(), {
        itemId: "item-still",
        sourceIn: 0,
        sourceOut: 2.3333,
      }),
      unsizedImage: upscaleGenerationRequest(unsizedImage),
      generatedMedia: upscaleGenerationRequest(fixtureMedia(project, "generated")),
    }).toMatchInlineSnapshot(`
      {
        "generatedMedia": {
          "kind": "generated",
          "model": {
            "id": "fal-ai/aura-sr",
            "provider": "fal.ai",
          },
          "name": null,
          "placementIntent": "library",
          "prompt": "Upscale this clip while preserving composition, timing, motion, and subject details.",
          "references": {
            "firstFrameMediaId": null,
            "lastFrameMediaId": null,
            "mediaIds": [
              "sample-generated-output",
            ],
          },
          "settings": {
            "aspectRatio": "16:9",
            "durationSeconds": 4,
            "fps": 24,
            "height": 720,
            "width": 1280,
          },
          "targetFolderId": null,
        },
        "image": {
          "kind": "generated",
          "model": {
            "id": "fal-ai/aura-sr",
            "provider": "fal.ai",
          },
          "name": null,
          "placementIntent": "library",
          "prompt": "Upscale this clip while preserving composition, timing, motion, and subject details.",
          "references": {
            "firstFrameMediaId": null,
            "lastFrameMediaId": null,
            "mediaIds": [
              "media-still",
            ],
          },
          "settings": {
            "aspectRatio": "4:3",
            "durationSeconds": 4,
            "fps": 24,
            "height": 1536,
            "width": 2048,
          },
          "targetFolderId": null,
        },
        "imageWithContext": {
          "kind": "generated",
          "model": {
            "id": "fal-ai/aura-sr",
            "provider": "fal.ai",
          },
          "name": null,
          "placementIntent": "library",
          "prompt": "Upscale this clip while preserving composition, timing, motion, and subject details.",
          "references": {
            "firstFrameMediaId": null,
            "lastFrameMediaId": null,
            "mediaIds": [
              "media-still",
            ],
          },
          "settings": {
            "aspectRatio": "4:3",
            "durationSeconds": 2.333,
            "fps": 24,
            "height": 1536,
            "width": 2048,
          },
          "targetFolderId": null,
        },
        "unsizedImage": {
          "kind": "generated",
          "model": {
            "id": "fal-ai/aura-sr",
            "provider": "fal.ai",
          },
          "name": null,
          "placementIntent": "library",
          "prompt": "Upscale this clip while preserving composition, timing, motion, and subject details.",
          "references": {
            "firstFrameMediaId": null,
            "lastFrameMediaId": null,
            "mediaIds": [
              "media-unsized",
            ],
          },
          "settings": {
            "aspectRatio": "16:9",
            "durationSeconds": 4,
            "fps": 24,
            "height": 1080,
            "width": 1920,
          },
          "targetFolderId": null,
        },
      }
    `);
  });

  it("builds video upscale requests with and without context", () => {
    const project = fixtureProject();
    const video = fixtureMedia(project, "video");
    expect({
      video: upscaleGenerationRequest(video),
      videoWithContext: upscaleGenerationRequest(video, {
        itemId: "item-1",
        sourceIn: 0.5,
        sourceOut: 3.25,
      }),
      videoWithInvertedContext: upscaleGenerationRequest(video, {
        itemId: "item-1",
        sourceIn: 3,
        sourceOut: 1,
      }),
      videoWithNonFiniteContext: upscaleGenerationRequest(video, {
        itemId: "item-1",
        sourceIn: Number.NaN,
        sourceOut: 2,
      }),
      zeroDurationVideo: upscaleGenerationRequest({
        ...video,
        durationSeconds: 0,
        fps: null,
      }),
    }).toMatchInlineSnapshot(`
      {
        "video": {
          "kind": "generated",
          "model": {
            "id": "fal-ai/video-upscaler",
            "provider": "fal.ai",
          },
          "name": null,
          "placementIntent": "library",
          "prompt": "Upscale this clip while preserving composition, timing, motion, and subject details.",
          "references": {
            "firstFrameMediaId": null,
            "lastFrameMediaId": null,
            "mediaIds": [
              "media-1",
            ],
          },
          "settings": {
            "aspectRatio": "16:9",
            "durationSeconds": 4,
            "fps": 24,
            "height": 720,
            "width": 1280,
          },
          "targetFolderId": null,
        },
        "videoWithContext": {
          "kind": "generated",
          "model": {
            "id": "fal-ai/video-upscaler",
            "provider": "fal.ai",
          },
          "name": null,
          "placementIntent": "library",
          "prompt": "Upscale this clip while preserving composition, timing, motion, and subject details.",
          "references": {
            "firstFrameMediaId": null,
            "lastFrameMediaId": null,
            "mediaIds": [
              "media-1",
            ],
            "sourceVideoMediaRef": "media-1",
          },
          "settings": {
            "aspectRatio": "16:9",
            "durationSeconds": 2.75,
            "fps": 24,
            "height": 720,
            "videoSourceEndSeconds": 3.25,
            "videoSourceStartSeconds": 0.5,
            "width": 1280,
          },
          "targetFolderId": null,
        },
        "videoWithInvertedContext": {
          "kind": "generated",
          "model": {
            "id": "fal-ai/video-upscaler",
            "provider": "fal.ai",
          },
          "name": null,
          "placementIntent": "library",
          "prompt": "Upscale this clip while preserving composition, timing, motion, and subject details.",
          "references": {
            "firstFrameMediaId": null,
            "lastFrameMediaId": null,
            "mediaIds": [
              "media-1",
            ],
          },
          "settings": {
            "aspectRatio": "16:9",
            "durationSeconds": 4,
            "fps": 24,
            "height": 720,
            "width": 1280,
          },
          "targetFolderId": null,
        },
        "videoWithNonFiniteContext": {
          "kind": "generated",
          "model": {
            "id": "fal-ai/video-upscaler",
            "provider": "fal.ai",
          },
          "name": null,
          "placementIntent": "library",
          "prompt": "Upscale this clip while preserving composition, timing, motion, and subject details.",
          "references": {
            "firstFrameMediaId": null,
            "lastFrameMediaId": null,
            "mediaIds": [
              "media-1",
            ],
          },
          "settings": {
            "aspectRatio": "16:9",
            "durationSeconds": 4,
            "fps": 24,
            "height": 720,
            "width": 1280,
          },
          "targetFolderId": null,
        },
        "zeroDurationVideo": {
          "kind": "generated",
          "model": {
            "id": "fal-ai/video-upscaler",
            "provider": "fal.ai",
          },
          "name": null,
          "placementIntent": "library",
          "prompt": "Upscale this clip while preserving composition, timing, motion, and subject details.",
          "references": {
            "firstFrameMediaId": null,
            "lastFrameMediaId": null,
            "mediaIds": [
              "media-1",
            ],
          },
          "settings": {
            "aspectRatio": "16:9",
            "durationSeconds": 4,
            "fps": 24,
            "height": 720,
            "width": 1280,
          },
          "targetFolderId": null,
        },
      }
    `);
  });

  it("builds video-to-audio requests for music and sound effects", () => {
    const project = fixtureProject();
    const video = fixtureMedia(project, "video");
    expect({
      music: videoAudioGenerationRequest(video, "music", {
        itemId: "item-1",
        timelineStartSeconds: 1.23456,
        durationSeconds: 3,
        sourceIn: 0.25,
        sourceOut: 2.75,
      }),
      sfx: videoAudioGenerationRequest(video, "sfx", {
        itemId: "item-1",
        timelineStartSeconds: 0,
        durationSeconds: 2.5,
      }),
      sfxMediaDurationFallback: videoAudioGenerationRequest(video, "sfx", {
        itemId: "item-1",
        timelineStartSeconds: 4,
        durationSeconds: 0,
        sourceIn: 2,
        sourceOut: 2,
      }),
      musicDefaultDuration: videoAudioGenerationRequest(
        { ...video, durationSeconds: 0 },
        "music",
        { itemId: "item-1", timelineStartSeconds: 0, durationSeconds: -1 },
      ),
    }).toMatchInlineSnapshot(`
      {
        "music": {
          "kind": "audio",
          "model": {
            "id": "sonilo/v1.1/video-to-music",
            "provider": "fal.ai",
          },
          "name": "Generated music",
          "placementIntent": "timeline",
          "prompt": "Generate music that fits the video.",
          "references": {
            "firstFrameMediaId": null,
            "lastFrameMediaId": null,
            "mediaIds": [
              "media-1",
            ],
            "sourceVideoMediaRef": "media-1",
          },
          "settings": {
            "aspectRatio": null,
            "category": "music",
            "durationSeconds": 2.5,
            "fps": null,
            "height": null,
            "timelineStartSeconds": 1.235,
            "videoSourceEndSeconds": 2.75,
            "videoSourceStartSeconds": 0.25,
            "width": null,
          },
          "targetFolderId": null,
        },
        "musicDefaultDuration": {
          "kind": "audio",
          "model": {
            "id": "sonilo/v1.1/video-to-music",
            "provider": "fal.ai",
          },
          "name": "Generated music",
          "placementIntent": "timeline",
          "prompt": "Generate music that fits the video.",
          "references": {
            "firstFrameMediaId": null,
            "lastFrameMediaId": null,
            "mediaIds": [
              "media-1",
            ],
            "sourceVideoMediaRef": "media-1",
          },
          "settings": {
            "aspectRatio": null,
            "category": "music",
            "durationSeconds": 4,
            "fps": null,
            "height": null,
            "timelineStartSeconds": 0,
            "width": null,
          },
          "targetFolderId": null,
        },
        "sfx": {
          "kind": "audio",
          "model": {
            "id": "mirelo-ai/sfx-v1.5/video-to-audio",
            "provider": "fal.ai",
          },
          "name": "Generated sound effects",
          "placementIntent": "timeline",
          "prompt": "Create matching sound for the video.",
          "references": {
            "firstFrameMediaId": null,
            "lastFrameMediaId": null,
            "mediaIds": [
              "media-1",
            ],
            "sourceVideoMediaRef": "media-1",
          },
          "settings": {
            "aspectRatio": null,
            "category": "sfx",
            "durationSeconds": 2.5,
            "fps": null,
            "height": null,
            "timelineStartSeconds": 0,
            "width": null,
          },
          "targetFolderId": null,
        },
        "sfxMediaDurationFallback": {
          "kind": "audio",
          "model": {
            "id": "mirelo-ai/sfx-v1.5/video-to-audio",
            "provider": "fal.ai",
          },
          "name": "Generated sound effects",
          "placementIntent": "timeline",
          "prompt": "Create matching sound for the video.",
          "references": {
            "firstFrameMediaId": null,
            "lastFrameMediaId": null,
            "mediaIds": [
              "media-1",
            ],
            "sourceVideoMediaRef": "media-1",
          },
          "settings": {
            "aspectRatio": null,
            "category": "sfx",
            "durationSeconds": 4,
            "fps": null,
            "height": null,
            "timelineStartSeconds": 4,
            "width": null,
          },
          "targetFolderId": null,
        },
      }
    `);
  });
});

describe("source clip generation eligibility characterization", () => {
  function videoMedia(overrides: Partial<MediaAsset> = {}): MediaAsset {
    return {
      id: "media-video",
      relativePath: "media/clip.mp4",
      kind: "video",
      durationSeconds: 20,
      width: 1920,
      height: 1080,
      fps: 30,
      folderId: null,
      ...overrides,
    };
  }

  function clip(properties: Record<string, unknown>, overrides: Partial<TimelineItem> = {}): TimelineItem {
    return {
      ...fixtureItem(fixtureProject(), "video"),
      id: "clip-1",
      startSeconds: 3,
      durationSeconds: 6,
      properties,
      ...overrides,
    };
  }

  it("explains imported upscale limits", () => {
    expect({
      none: importedUpscaleLimitReasonForMedia(null),
      image: importedUpscaleLimitReasonForMedia(imageMedia()),
      audio: importedUpscaleLimitReasonForMedia(fixtureMedia(fixtureProject(), "audio")),
      videoNoHeight: importedUpscaleLimitReasonForMedia(videoMedia({ height: null })),
      videoZeroHeight: importedUpscaleLimitReasonForMedia(videoMedia({ height: 0 })),
      videoHd: importedUpscaleLimitReasonForMedia(videoMedia()),
      videoJustBelow4k: importedUpscaleLimitReasonForMedia(videoMedia({ height: 2159 })),
      video4k: importedUpscaleLimitReasonForMedia(videoMedia({ height: 2160 })),
      video8k: importedUpscaleLimitReasonForMedia(videoMedia({ height: 4320 })),
    }).toMatchInlineSnapshot(`
      {
        "audio": null,
        "image": null,
        "none": null,
        "video4k": "Already 4K or higher",
        "video8k": "Already 4K or higher",
        "videoHd": null,
        "videoJustBelow4k": null,
        "videoNoHeight": "Loading video metadata...",
        "videoZeroHeight": "Loading video metadata...",
      }
    `);
  });

  it("explains generated upscale limits", () => {
    const asset = fixtureGeneratedAsset(fixtureProject());
    expect({
      none: generatedUpscaleLimitReasonForAsset(null),
      sample: generatedUpscaleLimitReasonForAsset(asset),
      auraSr: generatedUpscaleLimitReasonForAsset({
        ...asset,
        model: { ...asset.model, id: "fal-ai/aura-sr" },
      }),
      videoUpscaler: generatedUpscaleLimitReasonForAsset({
        ...asset,
        model: { ...asset.model, id: "fal-ai/video-upscaler" },
      }),
    }).toMatchInlineSnapshot(`
      {
        "auraSr": "Already upscaled",
        "none": null,
        "sample": null,
        "videoUpscaler": "Already upscaled",
      }
    `);
  });

  it("derives source clip upscale context", () => {
    expect({
      noItem: sourceClipUpscaleContextForItem(null, videoMedia()),
      noRange: sourceClipUpscaleContextForItem(clip({}), videoMedia()),
      partialRange: sourceClipUpscaleContextForItem(clip({ sourceIn: 2 }), videoMedia()),
      invertedRange: sourceClipUpscaleContextForItem(clip({ sourceIn: 5, sourceOut: 5 }), videoMedia()),
      subRange: sourceClipUpscaleContextForItem(clip({ sourceIn: 2, sourceOut: 8 }), videoMedia()),
      fullRange: sourceClipUpscaleContextForItem(clip({ sourceIn: 0, sourceOut: 20 }), videoMedia()),
      nearFullRange: sourceClipUpscaleContextForItem(
        clip({ sourceIn: 0.0005, sourceOut: 19.9995 }),
        videoMedia(),
      ),
      fullRangeUnknownDuration: sourceClipUpscaleContextForItem(
        clip({ sourceIn: 0, sourceOut: 20 }),
        videoMedia({ durationSeconds: 0 }),
      ),
      fullRangeNoMedia: sourceClipUpscaleContextForItem(clip({ sourceIn: 0, sourceOut: 20 }), null),
      nonNumericRange: sourceClipUpscaleContextForItem(clip({ sourceIn: "1", sourceOut: 4 }), videoMedia()),
    }).toMatchInlineSnapshot(`
      {
        "fullRange": undefined,
        "fullRangeNoMedia": {
          "itemId": "clip-1",
          "sourceIn": 0,
          "sourceOut": 20,
        },
        "fullRangeUnknownDuration": {
          "itemId": "clip-1",
          "sourceIn": 0,
          "sourceOut": 20,
        },
        "invertedRange": undefined,
        "nearFullRange": undefined,
        "noItem": undefined,
        "noRange": undefined,
        "nonNumericRange": undefined,
        "partialRange": undefined,
        "subRange": {
          "itemId": "clip-1",
          "sourceIn": 2,
          "sourceOut": 8,
        },
      }
    `);
  });

  it("derives source clip generation context", () => {
    expect({
      noItem: sourceClipGenerationContextForItem(null),
      noRange: sourceClipGenerationContextForItem(clip({})),
      partialRange: sourceClipGenerationContextForItem(clip({ sourceOut: 4 })),
      invertedRange: sourceClipGenerationContextForItem(clip({ sourceIn: 6, sourceOut: 4 })),
      validRange: sourceClipGenerationContextForItem(clip({ sourceIn: 1.5, sourceOut: 7.5 })),
    }).toMatchInlineSnapshot(`
      {
        "invertedRange": {
          "durationSeconds": 6,
          "itemId": "clip-1",
          "timelineStartSeconds": 3,
        },
        "noItem": null,
        "noRange": {
          "durationSeconds": 6,
          "itemId": "clip-1",
          "timelineStartSeconds": 3,
        },
        "partialRange": {
          "durationSeconds": 6,
          "itemId": "clip-1",
          "timelineStartSeconds": 3,
        },
        "validRange": {
          "durationSeconds": 6,
          "itemId": "clip-1",
          "sourceIn": 1.5,
          "sourceOut": 7.5,
          "timelineStartSeconds": 3,
        },
      }
    `);
  });

  it("explains imported video edit duration limits", () => {
    const shortClip = sourceClipGenerationContextForItem(clip({}, { durationSeconds: 10 }));
    const longClip = sourceClipGenerationContextForItem(clip({}, { durationSeconds: 12.6 }));
    expect({
      noMedia: importedVideoEditLimitReasonForMedia(null, longClip),
      image: importedVideoEditLimitReasonForMedia(imageMedia(), longClip),
      longVideoNoClip: importedVideoEditLimitReasonForMedia(videoMedia(), null),
      shortVideoNoClip: importedVideoEditLimitReasonForMedia(videoMedia({ durationSeconds: 9 }), null),
      exactLimitClip: importedVideoEditLimitReasonForMedia(videoMedia(), shortClip),
      longClip: importedVideoEditLimitReasonForMedia(videoMedia({ durationSeconds: 5 }), longClip),
      infiniteDuration: importedVideoEditLimitReasonForMedia(
        videoMedia({ durationSeconds: Number.POSITIVE_INFINITY }),
        null,
      ),
      nanClip: importedVideoEditLimitReasonForMedia(
        videoMedia(),
        sourceClipGenerationContextForItem(clip({}, { durationSeconds: Number.NaN })),
      ),
    }).toMatchInlineSnapshot(`
      {
        "exactLimitClip": null,
        "image": null,
        "infiniteDuration": null,
        "longClip": "Edit supports up to 10s (this is 13s)",
        "longVideoNoClip": "Edit supports up to 10s (this is 20s)",
        "nanClip": null,
        "noMedia": null,
        "shortVideoNoClip": null,
      }
    `);
  });
});
