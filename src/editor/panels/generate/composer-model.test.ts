import { describe, expect, it } from "vitest";
import { defaultAppPreferences } from "@/lib/app-settings";
import { generationAudioPromptCategory } from "@/lib/generation/provider-rules";
import type { GenerationModelOption } from "@/lib/generation/types";
import type { MediaAsset } from "@/lib/project";
import type { GenerationConfiguration } from "../../services/editor-environment";
import {
  buildGenerationRequest,
  composerModels,
  configurationBlocker,
  initialComposerState,
  readinessReason,
  selectedComposerModel,
} from "./composer-model";

const seedance: GenerationModelOption = { provider: "replicate", id: "bytedance/seedance-2.0-fast" };
const media: MediaAsset[] = [
  { id: "still", name: "Poster", relativePath: "media/poster.png", kind: "image", durationSeconds: 0, width: 10, height: 10, fps: null },
  { id: "clip", name: "Take", relativePath: "media/take.mp4", kind: "video", durationSeconds: 3, width: 10, height: 10, fps: 24 },
];

function configuration(overrides: Partial<GenerationConfiguration> = {}): GenerationConfiguration {
  return {
    catalog: { video: [seedance] },
    providerStatuses: [{ provider: "replicate", displayName: "Replicate", configured: true, source: "keychain" }],
    readiness: { models: "ready", providers: "ready", temporal: "ready" },
    preferences: defaultAppPreferences,
    temporalBackendReady: undefined,
    ...overrides,
  };
}

describe("composer models", () => {
  it("splits built-in audio models into music, SFX and voice by provider category", () => {
    expect(composerModels("music", null).every((model) => generationAudioPromptCategory(model) === "music")).toBe(true);
    expect(composerModels("voice", null).map((model) => model.id)).toContain("gpt-4o-mini-tts");
    expect(composerModels("sfx", null).map((model) => model.id)).toContain("bytedance/seed-audio-1.0");
  });

  it("uses only enabled catalog models once the catalog loads, falling back to the first", () => {
    expect(composerModels("image", { video: [seedance] })).toEqual([]);
    const state = { ...initialComposerState("video"), modelValues: { video: "fal.ai:gone" } };
    expect(selectedComposerModel(state, [seedance])).toBe(seedance);
  });
});

describe("configuration blocker precedence", () => {
  it("checks models, then the provider key, then Temporal", () => {
    expect(configurationBlocker(configuration({ readiness: { models: "checking", providers: "failed", temporal: "failed" } }), "video", seedance, "Video")).toEqual({ kind: "checking", message: "Checking generation configuration…" });
    expect(configurationBlocker(configuration({ catalog: {} }), "video", null, "Video")?.message).toBe("Media generation needs at least one enabled generation model.");
    expect(configurationBlocker(configuration({ catalog: { image: [seedance] } }), "video", null, "Video")?.message).toBe("Enable a video generation model to generate here.");
    expect(configurationBlocker(configuration({ readiness: { models: "ready", providers: "failed", temporal: "ready" } }), "video", seedance, "Video")).toMatchObject({ message: "Replicate provider configuration could not be checked.", label: "Configure Replicate in Settings" });
    expect(
      configurationBlocker(configuration({ providerStatuses: [{ provider: "replicate", displayName: "Replicate", configured: false, source: "unavailable" }] }), "video", seedance, "Video")?.message,
    ).toBe("Replicate video generation cannot access its provider key.");
    const temporal = configuration({ preferences: { ...defaultAppPreferences, generationExecutionBackend: "temporal" }, temporalBackendReady: false });
    expect(configurationBlocker(temporal, "video", seedance, "Video")).toMatchObject({ message: "Temporal generation is selected, but the Temporal backend is not configured.", target: { category: "advanced", item: "execution" } });
    expect(configurationBlocker(configuration({ readiness: null, catalog: null }), "video", seedance, "Video")).toBeNull();
  });
});

describe("generation request", () => {
  it("reports readiness in the pre-cut order", () => {
    const state = initialComposerState("video");
    expect(readinessReason(state, seedance, media, { kind: "checking", message: "" })).toBe("Checking configuration");
    expect(readinessReason(state, seedance, media, null)).toBe("Prompt required");
    const falImageToVideo: GenerationModelOption = { provider: "fal.ai", id: "fal-ai/wan/v2.7/image-to-video" };
    expect(readinessReason({ ...state, prompt: "Go" }, falImageToVideo, media, null)).toBe("Select first frame");
  });

  it("sends only the active pane's inputs for models that take frames or references, not both", () => {
    const base = { ...initialComposerState("video"), prompt: " Push in ", firstFrameId: "still", referenceIds: ["clip"] };
    const frames = buildGenerationRequest({ ...base, referencePane: "frames" }, seedance, media, "folder-1");
    expect(frames).toMatchObject({ kind: "generated", prompt: "Push in", targetFolderId: "folder-1", placementIntent: "library", references: { mediaIds: [], firstFrameMediaId: "still" } });
    const references = buildGenerationRequest({ ...base, referencePane: "references" }, seedance, media, null);
    expect(references?.references).toMatchObject({ mediaIds: ["clip"], firstFrameMediaId: null, referenceVideoMediaRefs: ["clip"] });
    expect(references?.settings).toMatchObject({ durationSeconds: 5, fps: 24, aspectRatio: "16:9" });
  });
});
