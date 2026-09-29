import { describe, expect, it } from "vitest";
import { defaultAppPreferences, type AppSettingsPreferences } from "@/lib/app-settings";
import type { GenerationModelCatalogPayload } from "@/lib/project";
import {
  generationModelCatalogFromPayload,
  isGenerationCatalogKind,
  isMediaGenerationMode,
} from "@/lib/generation/catalog";

type PayloadModel = GenerationModelCatalogPayload["generationModels"][number];

function payloadModel(overrides: Partial<PayloadModel>): PayloadModel {
  return {
    provider: "fal.ai",
    id: "fal-ai/model",
    kind: "video",
    allowedEndpoints: [],
    responseShape: "video",
    uiCapabilities: {
      speed: "fast",
      p75DurationSeconds: 12,
      supportedTypes: ["video"],
    },
    paidOnly: false,
    ...overrides,
  };
}

function payload(models: PayloadModel[], loaded = true): GenerationModelCatalogPayload {
  return { loaded, generationModels: models, providerCredentialsExposed: false };
}

const enabledModels = [
  payloadModel({
    provider: "fal.ai",
    id: "fal-ai/wan-25-preview/text-to-video",
    kind: "video",
    displayName: "Wan Text to Video",
    uiCapabilities: {
      durations: [5, 10],
      resolutions: ["720p"],
      aspectRatios: ["16:9"],
      supportsFirstFrame: true,
      supportsLastFrame: false,
      maxReferenceImages: 1,
      maxReferenceVideos: 0,
      maxReferenceAudios: 0,
      maxTotalReferences: 1,
      maxCombinedVideoRefSeconds: null,
      maxCombinedAudioRefSeconds: null,
      framesAndReferencesExclusive: false,
      referenceTagNoun: "image",
      requiresSourceVideo: false,
      requiresReferenceImage: false,
    },
  }),
  payloadModel({
    provider: "replicate",
    id: "black-forest-labs/flux",
    kind: "image",
    responseShape: "images",
    cancellationCapability: "none",
    uiCapabilities: {
      resolutions: null,
      aspectRatios: ["1:1"],
      qualities: null,
      supportsImageReference: true,
      maxImages: 4,
    },
  }),
  payloadModel({
    provider: "minimax",
    id: "minimax-speech",
    kind: "audio",
    responseShape: "audio",
    uiCapabilities: {
      category: "tts",
      voices: ["narrator"],
      defaultVoice: "narrator",
      supportsLyrics: false,
      supportsInstrumental: false,
      supportsStyleInstructions: false,
      durations: null,
      minPromptLength: 3,
      inputs: ["text"],
      promptLabel: "Script",
      minSeconds: 1,
      maxSeconds: 60,
    },
  }),
  payloadModel({
    provider: "fal.ai",
    id: "fal-ai/aura-sr",
    kind: "upscale",
    responseShape: "upscaledImage",
  }),
  payloadModel({ provider: "mock", id: "mock-video", kind: "video" }),
  payloadModel({
    provider: "local",
    id: "array-capabilities",
    kind: "image",
    uiCapabilities: ["not", "a", "record"] as unknown as PayloadModel["uiCapabilities"],
  }),
];

const preferences: AppSettingsPreferences = {
  ...defaultAppPreferences,
  enabledGenerationModelIds: [
    "fal.ai:fal-ai/wan-25-preview/text-to-video",
    "replicate:black-forest-labs/flux",
    "minimax:minimax-speech",
    "fal.ai:fal-ai/aura-sr",
    "local:array-capabilities",
    "fal.ai:unknown-kind",
  ],
};

describe("generation catalog characterization", () => {
  it("guards generation modes and catalog kinds", () => {
    const values: unknown[] = ["image", "video", "audio", "upscale", "generated", "", null, 3];
    expect(
      values.map((value) => [value, isMediaGenerationMode(value), isGenerationCatalogKind(value)]),
    ).toMatchInlineSnapshot(`
      [
        [
          "image",
          true,
          true,
        ],
        [
          "video",
          true,
          true,
        ],
        [
          "audio",
          true,
          true,
        ],
        [
          "upscale",
          false,
          true,
        ],
        [
          "generated",
          false,
          false,
        ],
        [
          "",
          false,
          false,
        ],
        [
          null,
          false,
          false,
        ],
        [
          3,
          false,
          false,
        ],
      ]
    `);
  });

  it("normalizes enabled catalog models", () => {
    expect(generationModelCatalogFromPayload(payload(enabledModels), preferences)).toMatchInlineSnapshot(`
      {
        "audio": [
          {
            "allowedEndpoints": [],
            "cancellationCapability": "local",
            "category": "tts",
            "defaultVoice": "narrator",
            "durations": null,
            "id": "minimax-speech",
            "inputs": [
              "text",
            ],
            "kind": "audio",
            "maxSeconds": 60,
            "minPromptLength": 3,
            "minSeconds": 1,
            "paidOnly": false,
            "promptLabel": "Script",
            "provider": "minimax",
            "responseShape": "audio",
            "supportsInstrumental": false,
            "supportsLyrics": false,
            "supportsStyleInstructions": false,
            "uiCapabilities": {
              "category": "tts",
              "defaultVoice": "narrator",
              "durations": null,
              "inputs": [
                "text",
              ],
              "maxSeconds": 60,
              "minPromptLength": 3,
              "minSeconds": 1,
              "promptLabel": "Script",
              "supportsInstrumental": false,
              "supportsLyrics": false,
              "supportsStyleInstructions": false,
              "voices": [
                "narrator",
              ],
            },
            "voices": [
              "narrator",
            ],
          },
        ],
        "image": [
          {
            "allowedEndpoints": [],
            "aspectRatios": [
              "1:1",
            ],
            "cancellationCapability": "none",
            "id": "black-forest-labs/flux",
            "kind": "image",
            "maxImages": 4,
            "paidOnly": false,
            "provider": "replicate",
            "qualities": null,
            "resolutions": null,
            "responseShape": "images",
            "supportsImageReference": true,
            "uiCapabilities": {
              "aspectRatios": [
                "1:1",
              ],
              "maxImages": 4,
              "qualities": null,
              "resolutions": null,
              "supportsImageReference": true,
            },
          },
          {
            "allowedEndpoints": [],
            "cancellationCapability": "local",
            "id": "array-capabilities",
            "kind": "image",
            "paidOnly": false,
            "provider": "local",
            "responseShape": "video",
            "uiCapabilities": [
              "not",
              "a",
              "record",
            ],
          },
        ],
        "upscale": [
          {
            "allowedEndpoints": [],
            "cancellationCapability": "provider",
            "id": "fal-ai/aura-sr",
            "kind": "upscale",
            "p75DurationSeconds": 12,
            "paidOnly": false,
            "provider": "fal.ai",
            "responseShape": "upscaledImage",
            "speed": "fast",
            "supportedTypes": [
              "video",
            ],
            "uiCapabilities": {
              "p75DurationSeconds": 12,
              "speed": "fast",
              "supportedTypes": [
                "video",
              ],
            },
          },
        ],
        "video": [
          {
            "allowedEndpoints": [],
            "aspectRatios": [
              "16:9",
            ],
            "cancellationCapability": "provider",
            "displayName": "Wan Text to Video",
            "durations": [
              5,
              10,
            ],
            "framesAndReferencesExclusive": false,
            "id": "fal-ai/wan-25-preview/text-to-video",
            "kind": "video",
            "maxCombinedAudioRefSeconds": null,
            "maxCombinedVideoRefSeconds": null,
            "maxReferenceAudios": 0,
            "maxReferenceImages": 1,
            "maxReferenceVideos": 0,
            "maxTotalReferences": 1,
            "paidOnly": false,
            "provider": "fal.ai",
            "referenceTagNoun": "image",
            "requiresReferenceImage": false,
            "requiresSourceVideo": false,
            "resolutions": [
              "720p",
            ],
            "responseShape": "video",
            "supportsFirstFrame": true,
            "supportsLastFrame": false,
            "uiCapabilities": {
              "aspectRatios": [
                "16:9",
              ],
              "durations": [
                5,
                10,
              ],
              "framesAndReferencesExclusive": false,
              "maxCombinedAudioRefSeconds": null,
              "maxCombinedVideoRefSeconds": null,
              "maxReferenceAudios": 0,
              "maxReferenceImages": 1,
              "maxReferenceVideos": 0,
              "maxTotalReferences": 1,
              "referenceTagNoun": "image",
              "requiresReferenceImage": false,
              "requiresSourceVideo": false,
              "resolutions": [
                "720p",
              ],
              "supportsFirstFrame": true,
              "supportsLastFrame": false,
            },
          },
          {
            "allowedEndpoints": [],
            "cancellationCapability": "local",
            "id": "mock-video",
            "kind": "video",
            "p75DurationSeconds": 12,
            "paidOnly": false,
            "provider": "mock",
            "responseShape": "video",
            "speed": "fast",
            "supportedTypes": [
              "video",
            ],
            "uiCapabilities": {
              "p75DurationSeconds": 12,
              "speed": "fast",
              "supportedTypes": [
                "video",
              ],
            },
          },
        ],
      }
    `);
  });

  it("drops models disabled by preferences", () => {
    expect(
      generationModelCatalogFromPayload(payload(enabledModels), {
        ...preferences,
        enabledGenerationModelIds: ["replicate:black-forest-labs/flux"],
      }),
    ).toMatchInlineSnapshot(`
      {
        "image": [
          {
            "allowedEndpoints": [],
            "aspectRatios": [
              "1:1",
            ],
            "cancellationCapability": "none",
            "id": "black-forest-labs/flux",
            "kind": "image",
            "maxImages": 4,
            "paidOnly": false,
            "provider": "replicate",
            "qualities": null,
            "resolutions": null,
            "responseShape": "images",
            "supportsImageReference": true,
            "uiCapabilities": {
              "aspectRatios": [
                "1:1",
              ],
              "maxImages": 4,
              "qualities": null,
              "resolutions": null,
              "supportsImageReference": true,
            },
          },
        ],
        "video": [
          {
            "allowedEndpoints": [],
            "cancellationCapability": "local",
            "id": "mock-video",
            "kind": "video",
            "p75DurationSeconds": 12,
            "paidOnly": false,
            "provider": "mock",
            "responseShape": "video",
            "speed": "fast",
            "supportedTypes": [
              "video",
            ],
            "uiCapabilities": {
              "p75DurationSeconds": 12,
              "speed": "fast",
              "supportedTypes": [
                "video",
              ],
            },
          },
        ],
      }
    `);
  });

  it("skips unknown kinds and malformed identities", () => {
    expect(
      generationModelCatalogFromPayload(
        payload([
          payloadModel({ provider: "fal.ai", id: "unknown-kind", kind: "generated" }),
          payloadModel({ provider: "fal.ai", id: "missing-kind", kind: null }),
          { ...payloadModel({}), provider: 7 as unknown as string },
          { ...payloadModel({}), id: undefined as unknown as string },
        ]),
        preferences,
      ),
    ).toMatchInlineSnapshot(`{}`);
  });

  it("returns null for missing or unloaded payloads", () => {
    expect({
      null: generationModelCatalogFromPayload(null, preferences),
      undefined: generationModelCatalogFromPayload(undefined, preferences),
      unloaded: generationModelCatalogFromPayload(payload(enabledModels, false), preferences),
      nonArray: generationModelCatalogFromPayload(
        {
          loaded: true,
          generationModels: null as unknown as PayloadModel[],
          providerCredentialsExposed: false,
        },
        preferences,
      ),
      empty: generationModelCatalogFromPayload(payload([]), preferences),
    }).toMatchInlineSnapshot(`
      {
        "empty": {},
        "nonArray": null,
        "null": null,
        "undefined": null,
        "unloaded": null,
      }
    `);
  });
});
