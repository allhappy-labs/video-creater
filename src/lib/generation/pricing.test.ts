import { describe, expect, it } from "vitest";
import {
  formatGenerationCreditEstimate,
  mockGenerationCreditBalance,
  selectedGenerationCost,
} from "@/lib/generation/pricing";
import { generationModelOptions } from "@/lib/generation/provider-rules";
import {
  generationDurationOptionsForModel,
  generationModelMaxImages,
  generationQualityOptionsForModel,
  generationResolutionOptionsForModel,
} from "@/lib/generation/settings-options";
import type { GenerationModelOption, MediaGenerationMode } from "@/lib/generation/types";

const longPrompt = "x".repeat(1200);

function costRow(mode: MediaGenerationMode, option: GenerationModelOption) {
  const durations = generationDurationOptionsForModel(mode, option);
  const resolutions = generationResolutionOptionsForModel(mode, option);
  const qualities = generationQualityOptionsForModel(mode, option);
  const firstDuration = durations[0]?.value ?? "";
  const lastDuration = durations[durations.length - 1]?.value ?? "";
  const firstResolution = resolutions[0]?.value ?? "";
  const lastResolution = resolutions[resolutions.length - 1]?.value ?? "";
  const firstQuality = qualities[0] ?? "";
  const lastQuality = qualities[qualities.length - 1] ?? "";
  const maxImages = String(generationModelMaxImages(mode, option));
  const cost = (
    duration: string,
    resolution: string,
    imageCount: string,
    generateAudio: boolean,
    prompt: string,
    quality: string,
  ) =>
    selectedGenerationCost(
      mode,
      option,
      duration,
      resolution,
      imageCount,
      generateAudio,
      prompt,
      quality,
    );
  return {
    model: `${option.provider}:${option.id}`,
    base: cost(firstDuration, firstResolution, "1", true, "", firstQuality),
    lastDuration: cost(lastDuration, firstResolution, "1", true, "", firstQuality),
    lastResolution: cost(firstDuration, lastResolution, "1", true, "", firstQuality),
    maxImages: cost(firstDuration, firstResolution, maxImages, true, "", firstQuality),
    longPrompt: cost(firstDuration, firstResolution, "1", true, longPrompt, firstQuality),
    withoutAudio: cost(firstDuration, firstResolution, "1", false, "", firstQuality),
    lastQuality: cost(firstDuration, firstResolution, "1", true, "", lastQuality),
    allLast: cost(lastDuration, lastResolution, maxImages, false, longPrompt, lastQuality),
  };
}

function matrix(mode: MediaGenerationMode) {
  return generationModelOptions[mode].map((option) => costRow(mode, option));
}

describe("generation cost matrix", () => {
  it("records image costs", () => {
    expect(matrix("image")).toMatchInlineSnapshot(`
      [
        {
          "allLast": 8,
          "base": 2,
          "lastDuration": 2,
          "lastQuality": 2,
          "lastResolution": 2,
          "longPrompt": 2,
          "maxImages": 8,
          "model": "openai:gpt-image-2",
          "withoutAudio": 2,
        },
        {
          "allLast": 1,
          "base": 2,
          "lastDuration": 2,
          "lastQuality": 2,
          "lastResolution": 1,
          "longPrompt": 2,
          "maxImages": 2,
          "model": "fal.ai:fal-ai/nano-banana-pro/edit",
          "withoutAudio": 2,
        },
        {
          "allLast": 4,
          "base": 1,
          "lastDuration": 1,
          "lastQuality": 1,
          "lastResolution": 1,
          "longPrompt": 1,
          "maxImages": 4,
          "model": "fal.ai:fal-ai/flux/schnell",
          "withoutAudio": 1,
        },
        {
          "allLast": 1,
          "base": 1,
          "lastDuration": 1,
          "lastQuality": 1,
          "lastResolution": 1,
          "longPrompt": 1,
          "maxImages": 1,
          "model": "fal.ai:fal-ai/krea-2/turbo",
          "withoutAudio": 1,
        },
        {
          "allLast": 2,
          "base": 2,
          "lastDuration": 2,
          "lastQuality": 2,
          "lastResolution": 2,
          "longPrompt": 2,
          "maxImages": 2,
          "model": "fal.ai:fal-ai/recraft/v3/text-to-image",
          "withoutAudio": 2,
        },
        {
          "allLast": 4,
          "base": 1,
          "lastDuration": 1,
          "lastQuality": 1,
          "lastResolution": 1,
          "longPrompt": 1,
          "maxImages": 4,
          "model": "replicate:black-forest-labs/flux-schnell",
          "withoutAudio": 1,
        },
        {
          "allLast": 2,
          "base": 2,
          "lastDuration": 2,
          "lastQuality": 2,
          "lastResolution": 2,
          "longPrompt": 2,
          "maxImages": 2,
          "model": "replicate:black-forest-labs/flux-dev",
          "withoutAudio": 2,
        },
        {
          "allLast": 3,
          "base": 3,
          "lastDuration": 3,
          "lastQuality": 3,
          "lastResolution": 3,
          "longPrompt": 3,
          "maxImages": 3,
          "model": "replicate:black-forest-labs/flux-1.1-pro",
          "withoutAudio": 3,
        },
        {
          "allLast": 4,
          "base": 4,
          "lastDuration": 4,
          "lastQuality": 4,
          "lastResolution": 4,
          "longPrompt": 4,
          "maxImages": 4,
          "model": "replicate:black-forest-labs/flux-1.1-pro-ultra",
          "withoutAudio": 4,
        },
        {
          "allLast": 2,
          "base": 2,
          "lastDuration": 2,
          "lastQuality": 2,
          "lastResolution": 2,
          "longPrompt": 2,
          "maxImages": 2,
          "model": "openai:gpt-image-1.5",
          "withoutAudio": 2,
        },
        {
          "allLast": 2,
          "base": 2,
          "lastDuration": 2,
          "lastQuality": 2,
          "lastResolution": 2,
          "longPrompt": 2,
          "maxImages": 2,
          "model": "xai:grok-imagine-image-quality",
          "withoutAudio": 2,
        },
      ]
    `);
  });

  it("records video costs", () => {
    expect(matrix("video")).toMatchInlineSnapshot(`
      [
        {
          "allLast": 8,
          "base": 5,
          "lastDuration": 10,
          "lastQuality": 5,
          "lastResolution": 5,
          "longPrompt": 5,
          "maxImages": 5,
          "model": "replicate:bytedance/seedance-2.0-fast",
          "withoutAudio": 4,
        },
        {
          "allLast": 8,
          "base": 4,
          "lastDuration": 7,
          "lastQuality": 4,
          "lastResolution": 5,
          "longPrompt": 4,
          "maxImages": 4,
          "model": "fal.ai:fal-ai/wan-25-preview/text-to-video",
          "withoutAudio": 3,
        },
        {
          "allLast": 8,
          "base": 4,
          "lastDuration": 7,
          "lastQuality": 4,
          "lastResolution": 5,
          "longPrompt": 4,
          "maxImages": 4,
          "model": "fal.ai:fal-ai/wan/v2.7/image-to-video",
          "withoutAudio": 3,
        },
        {
          "allLast": 8,
          "base": 2,
          "lastDuration": 10,
          "lastQuality": 2,
          "lastResolution": 2,
          "longPrompt": 2,
          "maxImages": 2,
          "model": "fal.ai:fal-ai/wan/v2.7/reference-to-video",
          "withoutAudio": 2,
        },
        {
          "allLast": "varies",
          "base": "varies",
          "lastDuration": "varies",
          "lastQuality": "varies",
          "lastResolution": "varies",
          "longPrompt": "varies",
          "maxImages": "varies",
          "model": "fal.ai:fal-ai/wan/v2.2-a14b/video-to-video",
          "withoutAudio": "varies",
        },
        {
          "allLast": 8,
          "base": 5,
          "lastDuration": 10,
          "lastQuality": 5,
          "lastResolution": 5,
          "longPrompt": 5,
          "maxImages": 5,
          "model": "fal.ai:fal-ai/kling-video/v3/standard/text-to-video",
          "withoutAudio": 4,
        },
        {
          "allLast": 8,
          "base": 5,
          "lastDuration": 10,
          "lastQuality": 5,
          "lastResolution": 5,
          "longPrompt": 5,
          "maxImages": 5,
          "model": "fal.ai:fal-ai/kling-video/v3/pro/image-to-video",
          "withoutAudio": 4,
        },
        {
          "allLast": 8,
          "base": 5,
          "lastDuration": 10,
          "lastQuality": 5,
          "lastResolution": 5,
          "longPrompt": 5,
          "maxImages": 5,
          "model": "fal.ai:fal-ai/kling-video/v3/pro/motion-control",
          "withoutAudio": 4,
        },
        {
          "allLast": 8,
          "base": 5,
          "lastDuration": 10,
          "lastQuality": 5,
          "lastResolution": 5,
          "longPrompt": 5,
          "maxImages": 5,
          "model": "replicate:bytedance/seedance-2.0",
          "withoutAudio": 4,
        },
        {
          "allLast": 15,
          "base": 1,
          "lastDuration": 15,
          "lastQuality": 1,
          "lastResolution": 1,
          "longPrompt": 1,
          "maxImages": 1,
          "model": "xai:grok-imagine-video",
          "withoutAudio": 1,
        },
        {
          "allLast": 13,
          "base": 8,
          "lastDuration": 8,
          "lastQuality": 8,
          "lastResolution": 16,
          "longPrompt": 8,
          "maxImages": 8,
          "model": "google:veo3.1-fast",
          "withoutAudio": 7,
        },
      ]
    `);
  });

  it("records audio costs", () => {
    expect(matrix("audio")).toMatchInlineSnapshot(`
      [
        {
          "allLast": "varies",
          "base": "varies",
          "lastDuration": "varies",
          "lastQuality": "varies",
          "lastResolution": "varies",
          "longPrompt": "varies",
          "maxImages": "varies",
          "model": "fal.ai:bytedance/seed-audio-1.0",
          "withoutAudio": "varies",
        },
        {
          "allLast": "varies",
          "base": "varies",
          "lastDuration": "varies",
          "lastQuality": "varies",
          "lastResolution": "varies",
          "longPrompt": "varies",
          "maxImages": "varies",
          "model": "fal.ai:sonilo/v1.1/text-to-music",
          "withoutAudio": "varies",
        },
        {
          "allLast": "varies",
          "base": "varies",
          "lastDuration": "varies",
          "lastQuality": "varies",
          "lastResolution": "varies",
          "longPrompt": "varies",
          "maxImages": "varies",
          "model": "fal.ai:sonilo/v1.1/video-to-music",
          "withoutAudio": "varies",
        },
        {
          "allLast": "varies",
          "base": "varies",
          "lastDuration": "varies",
          "lastQuality": "varies",
          "lastResolution": "varies",
          "longPrompt": "varies",
          "maxImages": "varies",
          "model": "fal.ai:mirelo-ai/sfx-v1.5/video-to-audio",
          "withoutAudio": "varies",
        },
        {
          "allLast": 18,
          "base": "varies",
          "lastDuration": "varies",
          "lastQuality": "varies",
          "lastResolution": "varies",
          "longPrompt": 18,
          "maxImages": "varies",
          "model": "openai:gpt-4o-mini-tts",
          "withoutAudio": "varies",
        },
        {
          "allLast": 18,
          "base": "varies",
          "lastDuration": "varies",
          "lastQuality": "varies",
          "lastResolution": "varies",
          "longPrompt": 18,
          "maxImages": "varies",
          "model": "elevenlabs:elevenlabs-tts-v3",
          "withoutAudio": "varies",
        },
        {
          "allLast": 10,
          "base": 10,
          "lastDuration": 10,
          "lastQuality": 10,
          "lastResolution": 10,
          "longPrompt": 10,
          "maxImages": 10,
          "model": "elevenlabs:elevenlabs-music",
          "withoutAudio": 10,
        },
        {
          "allLast": 10,
          "base": 10,
          "lastDuration": 10,
          "lastQuality": 10,
          "lastResolution": 10,
          "longPrompt": 10,
          "maxImages": 10,
          "model": "minimax:minimax-music-v2.6",
          "withoutAudio": 10,
        },
        {
          "allLast": 18,
          "base": "varies",
          "lastDuration": "varies",
          "lastQuality": "varies",
          "lastResolution": "varies",
          "longPrompt": 18,
          "maxImages": "varies",
          "model": "google:gemini-3.1-flash-tts-preview",
          "withoutAudio": "varies",
        },
        {
          "allLast": 10,
          "base": 10,
          "lastDuration": 10,
          "lastQuality": 10,
          "lastResolution": 10,
          "longPrompt": 10,
          "maxImages": 10,
          "model": "google:lyria3-pro",
          "withoutAudio": 10,
        },
      ]
    `);
  });

  it("records catalog pricing overrides and unknown models", () => {
    const catalogOptions: Array<[MediaGenerationMode, GenerationModelOption]> = [
      [
        "video",
        {
          provider: "catalog",
          id: "priced-video",
          durations: [4, 9],
          resolutions: ["480p", "1080p"],
          creditsPerSecond: { "480p": 0.5, "": 2 },
          audioDiscountRate: { "1080p": 0.5 },
        },
      ],
      [
        "video",
        {
          provider: "catalog",
          id: "unpriced-resolution",
          durations: [5],
          resolutions: ["720p"],
          creditsPerSecond: { "1080p": 1 },
        },
      ],
      ["video", { provider: "catalog", id: "empty-durations", durations: [] }],
      [
        "image",
        {
          provider: "catalog",
          id: "priced-image",
          resolutions: ["1K", "2K"],
          qualities: ["low", "high"],
          creditsPerImage: { "1K|high": 3, low: 0.4, "2K": 5 },
          maxImages: 4,
        },
      ],
      [
        "image",
        {
          provider: "catalog",
          id: "unpriced-image",
          creditsPerImage: { "4K": 5 },
        },
      ],
      [
        "audio",
        {
          provider: "catalog",
          id: "per-second-audio",
          audioPricing: { mode: "perSecond", rate: 0.25 },
          minSeconds: 10,
          maxSeconds: 60,
        },
      ],
      [
        "audio",
        {
          provider: "catalog",
          id: "per-second-audio-without-bounds",
          audioPricing: { mode: "perSecond", rate: 0.25 },
        },
      ],
      [
        "audio",
        {
          provider: "catalog",
          id: "per-char-audio",
          audioPricing: { mode: "perThousandChars", rate: 7 },
        },
      ],
      [
        "audio",
        {
          provider: "catalog",
          id: "flat-audio",
          audioPricing: { mode: "flat", price: 2.5 },
        },
      ],
      ["audio", { provider: "catalog", id: "unknown-audio" }],
    ];
    expect({
      rows: catalogOptions.map(([mode, option]) => costRow(mode, option)),
      unknownDurationValue: selectedGenerationCost(
        "video",
        catalogOptions[0]![1],
        "missing",
        "missing",
        "1",
        false,
        "",
        "",
      ),
      boundedAudioDuration: ["", "30", "9000", "8s"].map((duration) =>
        selectedGenerationCost("audio", catalogOptions[5]![1], duration, "", "1", true, "", ""),
      ),
      whitespacePrompt: selectedGenerationCost(
        "audio",
        catalogOptions[7]![1],
        "",
        "",
        "1",
        true,
        "   ",
        "",
      ),
      imageCountOverflow: ["0", "abc", "99"].map((count) =>
        selectedGenerationCost("image", catalogOptions[3]![1], "", "2K", count, true, "", "high"),
      ),
    }).toMatchInlineSnapshot(`
      {
        "boundedAudioDuration": [
          15,
          8,
          15,
          15,
        ],
        "imageCountOverflow": [
          5,
          5,
          20,
        ],
        "rows": [
          {
            "allLast": 9,
            "base": 2,
            "lastDuration": 5,
            "lastQuality": 2,
            "lastResolution": 8,
            "longPrompt": 2,
            "maxImages": 2,
            "model": "catalog:priced-video",
            "withoutAudio": 2,
          },
          {
            "allLast": "varies",
            "base": "varies",
            "lastDuration": "varies",
            "lastQuality": "varies",
            "lastResolution": "varies",
            "longPrompt": "varies",
            "maxImages": "varies",
            "model": "catalog:unpriced-resolution",
            "withoutAudio": "varies",
          },
          {
            "allLast": "varies",
            "base": "varies",
            "lastDuration": "varies",
            "lastQuality": "varies",
            "lastResolution": "varies",
            "longPrompt": "varies",
            "maxImages": "varies",
            "model": "catalog:empty-durations",
            "withoutAudio": "varies",
          },
          {
            "allLast": 20,
            "base": 1,
            "lastDuration": 1,
            "lastQuality": 3,
            "lastResolution": 1,
            "longPrompt": 1,
            "maxImages": 2,
            "model": "catalog:priced-image",
            "withoutAudio": 1,
          },
          {
            "allLast": "varies",
            "base": "varies",
            "lastDuration": "varies",
            "lastQuality": "varies",
            "lastResolution": "varies",
            "longPrompt": "varies",
            "maxImages": "varies",
            "model": "catalog:unpriced-image",
            "withoutAudio": "varies",
          },
          {
            "allLast": 15,
            "base": 15,
            "lastDuration": 15,
            "lastQuality": 15,
            "lastResolution": 15,
            "longPrompt": 15,
            "maxImages": 15,
            "model": "catalog:per-second-audio",
            "withoutAudio": 15,
          },
          {
            "allLast": "varies",
            "base": "varies",
            "lastDuration": "varies",
            "lastQuality": "varies",
            "lastResolution": "varies",
            "longPrompt": "varies",
            "maxImages": "varies",
            "model": "catalog:per-second-audio-without-bounds",
            "withoutAudio": "varies",
          },
          {
            "allLast": 9,
            "base": "varies",
            "lastDuration": "varies",
            "lastQuality": "varies",
            "lastResolution": "varies",
            "longPrompt": 9,
            "maxImages": "varies",
            "model": "catalog:per-char-audio",
            "withoutAudio": "varies",
          },
          {
            "allLast": 3,
            "base": 3,
            "lastDuration": 3,
            "lastQuality": 3,
            "lastResolution": 3,
            "longPrompt": 3,
            "maxImages": 3,
            "model": "catalog:flat-audio",
            "withoutAudio": 3,
          },
          {
            "allLast": "varies",
            "base": "varies",
            "lastDuration": "varies",
            "lastQuality": "varies",
            "lastResolution": "varies",
            "longPrompt": "varies",
            "maxImages": "varies",
            "model": "catalog:unknown-audio",
            "withoutAudio": "varies",
          },
        ],
        "unknownDurationValue": 2,
        "whitespacePrompt": "varies",
      }
    `);
  });
});

describe("formatGenerationCreditEstimate", () => {
  it("records credit estimate labels and the mock balance", () => {
    expect({
      labels: [0, 1, 12, 12.5, "varies" as const].map((value) =>
        formatGenerationCreditEstimate(value),
      ),
      mockGenerationCreditBalance,
    }).toMatchInlineSnapshot(`
      {
        "labels": [
          "Est. 0 credits",
          "Est. 1 credit",
          "Est. 12 credits",
          "Est. 12.5 credits",
          "Est. varies",
        ],
        "mockGenerationCreditBalance": 4400,
      }
    `);
  });
});
