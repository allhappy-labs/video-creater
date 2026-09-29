#!/usr/bin/env node
import { existsSync, readFileSync, statSync } from "node:fs";
import { isAbsolute, join, resolve, sep } from "node:path";
import { fileURLToPath } from "node:url";

export const requiredReports = [
  { provider: "fal.ai", scenario: "text-to-image" },
  { provider: "openai", scenario: "text-to-image" },
  { provider: "openai", scenario: "text-to-image-replace" },
  { provider: "openai", scenario: "multi-image" },
  { provider: "openai", scenario: "image-edit" },
  { provider: "openai", scenario: "local-image-edit-replace" },
  { provider: "openai", scenario: "text-to-audio" },
  { provider: "elevenlabs", scenario: "text-to-audio" },
  { provider: "fal.ai", scenario: "text-to-music" },
  { provider: "fal.ai", scenario: "text-to-music-insert" },
  { provider: "elevenlabs", scenario: "text-to-music" },
  { provider: "elevenlabs", scenario: "text-to-music-insert" },
  { provider: "minimax", scenario: "text-to-music" },
  { provider: "minimax", scenario: "text-to-music-insert" },
  { provider: "xai", scenario: "text-to-image" },
  { provider: "xai", scenario: "image-edit" },
  { provider: "xai", scenario: "local-image-edit-replace" },
  { provider: "xai", scenario: "xai-video" },
  { provider: "xai", scenario: "local-video-edit-replace" },
  { provider: "xai", scenario: "text-to-video-replace" },
  { provider: "xai", scenario: "text-to-video-insert" },
  { provider: "google", scenario: "google-video" },
  { provider: "google", scenario: "text-to-video-replace" },
  { provider: "google", scenario: "text-to-video-insert" },
  { provider: "google", scenario: "text-to-audio" },
  { provider: "google", scenario: "text-to-music" },
  { provider: "google", scenario: "text-to-music-insert" },
  { provider: "replicate", scenario: "text-to-image" },
  { provider: "replicate", scenario: "replicate-local-file-upload" },
  { provider: "fal.ai", scenario: "image-edit" },
  { provider: "fal.ai", scenario: "local-image-edit-replace" },
  { provider: "fal.ai", scenario: "local-image-edit-retry" },
  { provider: "fal.ai", scenario: "local-image-edit-cancellation" },
  { provider: "fal.ai", scenario: "local-image-edit-failure" },
  { provider: "fal.ai", scenario: "krea-text-to-image" },
  { provider: "fal.ai", scenario: "recraft-text-to-image" },
  { provider: "fal.ai", scenario: "multi-image" },
  { provider: "fal.ai", scenario: "wan-image-to-video" },
  { provider: "fal.ai", scenario: "wan-reference-to-video" },
  { provider: "fal.ai", scenario: "kling-image-to-video" },
  { provider: "fal.ai", scenario: "video-to-video" },
  { provider: "fal.ai", scenario: "text-to-audio" },
  { provider: "fal.ai", scenario: "video-to-music" },
  { provider: "fal.ai", scenario: "local-video-to-music-insert" },
  { provider: "fal.ai", scenario: "video-to-sfx" },
  { provider: "fal.ai", scenario: "local-video-to-sfx-insert" },
  { provider: "fal.ai", scenario: "text-to-video-replace" },
  { provider: "fal.ai", scenario: "text-to-video-insert" },
  { provider: "fal.ai", scenario: "local-image-upscale" },
  { provider: "fal.ai", scenario: "local-image-upscale-replace" },
  { provider: "fal.ai", scenario: "local-image-upscale-retry" },
  { provider: "fal.ai", scenario: "local-image-upscale-cancellation" },
  { provider: "fal.ai", scenario: "local-image-upscale-failure" },
  { provider: "fal.ai", scenario: "local-video-edit-replace" },
  { provider: "fal.ai", scenario: "local-video-motion-control-replace" },
  { provider: "fal.ai", scenario: "local-video-edit-retry" },
  { provider: "fal.ai", scenario: "local-video-edit-cancellation" },
  { provider: "fal.ai", scenario: "local-video-edit-failure" },
  { provider: "fal.ai", scenario: "local-video-upscale" },
  { provider: "fal.ai", scenario: "local-video-upscale-replace" },
  { provider: "fal.ai", scenario: "local-video-upscale-retry" },
  { provider: "fal.ai", scenario: "local-video-upscale-cancellation" },
  { provider: "fal.ai", scenario: "local-video-upscale-failure" },
  { provider: "replicate", scenario: "multi-image" },
  { provider: "replicate", scenario: "replicate-flux-dev" },
  { provider: "replicate", scenario: "replicate-flux-1.1-pro" },
  { provider: "replicate", scenario: "replicate-flux-1.1-pro-ultra" },
  { provider: "replicate", scenario: "replicate-video" },
  { provider: "replicate", scenario: "replicate-video-fast" },
  { provider: "replicate", scenario: "text-to-video-replace" },
  { provider: "replicate", scenario: "text-to-video-insert" },
  { provider: "fal.ai", scenario: "provider-failure" },
  { provider: "replicate", scenario: "provider-failure" },
  { provider: "fal.ai", scenario: "provider-cancellation" },
  { provider: "replicate", scenario: "provider-cancellation" },
  { provider: "fal.ai", scenario: "provider-retry" },
  { provider: "replicate", scenario: "provider-retry" },
];
const requiredProviders = new Set(requiredReports.map((report) => report.provider));
const requiredReportKeys = new Set(requiredReports.map(reportKey));
const textToAudioDefaultVoices = {
  openai: "alloy",
  elevenlabs: "rachel",
  google: "Kore",
};
const textToMusicDefaultSettings = {
  category: "music",
  durationSeconds: 30,
  instrumental: false,
  styleInstructions: "bright, commercial, loopable",
};
const minimaxTextToMusicDefaultLyrics = "[Verse]\nBright product launch\n[Chorus]\nLoopable brand hook";
const elevenlabsTextToMusicDefaultLyrics =
  "[Verse]\nWarm synth pop in English\n[Chorus]\nBright chorus lift";
const googleLyriaTextToMusicDefaultLyrics =
  "[Verse]\nSoft synth pulse in English\n[Chorus]\nConfident vocal lift";

function exampleReportDirectory({ provider, scenario }) {
  if (provider === "fal.ai" && scenario === "text-to-image") {
    return "fal.ai";
  }
  if (provider === "fal.ai" && scenario === "krea-text-to-image") {
    return "fal.ai-krea-image";
  }
  if (provider === "fal.ai" && scenario === "recraft-text-to-image") {
    return "fal.ai-recraft-image";
  }
  if (provider === "openai" && scenario === "text-to-image") {
    return "openai-mock-default";
  }
  if (provider === "openai" && scenario === "text-to-image-replace") {
    return "openai-text-image-replace-mock-default";
  }
  if (provider === "openai" && scenario === "multi-image") {
    return "openai-multi-image-mock-default";
  }
  if (provider === "openai" && scenario === "image-edit") {
    return "openai-image-edit-mock-default";
  }
  if (provider === "openai" && scenario === "local-image-edit-replace") {
    return "openai-image-edit-replace-mock-default";
  }
  if (provider === "openai" && scenario === "text-to-audio") {
    return "openai-audio-mock-default";
  }
  if (provider === "elevenlabs" && scenario === "text-to-audio") {
    return "elevenlabs-audio-mock-default";
  }
  if (provider === "fal.ai" && scenario === "text-to-music") {
    return "fal.ai-sonilo-music-mock-default";
  }
  if (provider === "fal.ai" && scenario === "text-to-music-insert") {
    return "fal.ai-sonilo-music-insert-mock-default";
  }
  if (provider === "elevenlabs" && scenario === "text-to-music") {
    return "elevenlabs-music-mock-default";
  }
  if (provider === "elevenlabs" && scenario === "text-to-music-insert") {
    return "elevenlabs-music-insert-mock-default";
  }
  if (provider === "minimax" && scenario === "text-to-music") {
    return "minimax-music-mock-default";
  }
  if (provider === "minimax" && scenario === "text-to-music-insert") {
    return "minimax-music-insert-mock-default";
  }
  if (provider === "xai" && scenario === "text-to-image") {
    return "xai-mock-default";
  }
  if (provider === "xai" && scenario === "image-edit") {
    return "xai-image-edit-mock-default";
  }
  if (provider === "xai" && scenario === "local-image-edit-replace") {
    return "xai-image-edit-replace-mock-default";
  }
  if (provider === "xai" && scenario === "xai-video") {
    return "xai-video-mock-default";
  }
  if (provider === "xai" && scenario === "local-video-edit-replace") {
    return "xai-video-edit-replace-mock-default";
  }
  if (provider === "xai" && scenario === "text-to-video-replace") {
    return "xai-text-video-replace-mock-default";
  }
  if (provider === "xai" && scenario === "text-to-video-insert") {
    return "xai-text-video-insert-mock-default";
  }
  if (provider === "google" && scenario === "google-video") {
    return "google-video-mock-default";
  }
  if (provider === "google" && scenario === "text-to-video-replace") {
    return "google-text-video-replace-mock-default";
  }
  if (provider === "google" && scenario === "text-to-video-insert") {
    return "google-text-video-insert-mock-default";
  }
  if (provider === "google" && scenario === "text-to-audio") {
    return "google-gemini-tts-mock-default";
  }
  if (provider === "google" && scenario === "text-to-music") {
    return "google-lyria-mock-default";
  }
  if (provider === "google" && scenario === "text-to-music-insert") {
    return "google-lyria-music-insert-mock-default";
  }
  if (provider === "replicate" && scenario === "text-to-image") {
    return "replicate";
  }
  if (provider === "replicate" && scenario === "replicate-local-file-upload") {
    return "replicate-local-file-upload";
  }
  if (provider === "replicate" && scenario === "replicate-flux-dev") {
    return "replicate-flux-dev";
  }
  if (provider === "replicate" && scenario === "replicate-flux-1.1-pro") {
    return "replicate-flux-1.1-pro";
  }
  if (provider === "replicate" && scenario === "replicate-flux-1.1-pro-ultra") {
    return "replicate-flux-1.1-pro-ultra";
  }
  if (provider === "replicate" && scenario === "replicate-video") {
    return "replicate-video";
  }
  if (provider === "replicate" && scenario === "replicate-video-fast") {
    return "replicate-video-fast";
  }
  if (scenario === "provider-failure") {
    return `${provider}-failure`;
  }
  if (scenario === "provider-cancellation") {
    return `${provider}-cancellation`;
  }
  if (scenario === "provider-retry") {
    return `${provider}-retry`;
  }
  if (provider === "fal.ai" && scenario === "local-video-to-music-insert") {
    return "fal.ai-video-to-music-insert";
  }
  if (provider === "fal.ai" && scenario === "local-image-edit-replace") {
    return "fal.ai-image-edit-replace";
  }
  if (provider === "fal.ai" && scenario === "local-image-edit-retry") {
    return "fal.ai-image-edit-retry";
  }
  if (provider === "fal.ai" && scenario === "local-image-edit-cancellation") {
    return "fal.ai-image-edit-cancellation";
  }
  if (provider === "fal.ai" && scenario === "local-image-edit-failure") {
    return "fal.ai-image-edit-failure";
  }
  if (provider === "fal.ai" && scenario === "local-video-to-sfx-insert") {
    return "fal.ai-video-to-sfx-insert";
  }
  if (provider === "fal.ai" && scenario === "text-to-video-replace") {
    return "fal.ai-text-video-replace-mock-default";
  }
  if (provider === "fal.ai" && scenario === "text-to-video-insert") {
    return "fal.ai-text-video-insert-mock-default";
  }
  if (provider === "fal.ai" && scenario === "local-image-upscale-replace") {
    return "fal.ai-upscale-replace";
  }
  if (provider === "fal.ai" && scenario === "local-image-upscale-retry") {
    return "fal.ai-upscale-retry";
  }
  if (provider === "fal.ai" && scenario === "local-image-upscale-cancellation") {
    return "fal.ai-upscale-cancellation";
  }
  if (provider === "fal.ai" && scenario === "local-image-upscale-failure") {
    return "fal.ai-upscale-failure";
  }
  if (provider === "fal.ai" && scenario === "local-image-upscale") {
    return "fal.ai-upscale";
  }
  if (provider === "fal.ai" && scenario === "local-video-upscale") {
    return "fal.ai-video-upscale";
  }
  if (provider === "fal.ai" && scenario === "local-video-upscale-replace") {
    return "fal.ai-video-upscale-replace";
  }
  if (provider === "fal.ai" && scenario === "local-video-upscale-retry") {
    return "fal.ai-video-upscale-retry";
  }
  if (provider === "fal.ai" && scenario === "local-video-upscale-cancellation") {
    return "fal.ai-video-upscale-cancellation";
  }
  if (provider === "fal.ai" && scenario === "local-video-upscale-failure") {
    return "fal.ai-video-upscale-failure";
  }
  if (provider === "fal.ai" && scenario === "local-video-edit-replace") {
    return "fal.ai-video-edit-replace";
  }
  if (provider === "fal.ai" && scenario === "local-video-motion-control-replace") {
    return "fal.ai-video-motion-control-replace";
  }
  if (provider === "fal.ai" && scenario === "local-video-edit-retry") {
    return "fal.ai-video-edit-retry";
  }
  if (provider === "fal.ai" && scenario === "local-video-edit-cancellation") {
    return "fal.ai-video-edit-cancellation";
  }
  if (provider === "fal.ai" && scenario === "local-video-edit-failure") {
    return "fal.ai-video-edit-failure";
  }
  if (provider === "fal.ai" && scenario === "text-to-audio") {
    return "fal.ai-audio";
  }
  return `${provider}-${scenario}`;
}

function exampleReportPath(requiredReport) {
  return `output/provider-e2e/${exampleReportDirectory(requiredReport)}/provider-e2e-report.json`;
}

function exampleManifest() {
  return JSON.stringify(
    {
      reports: requiredReports.map(exampleReportPath),
    },
    null,
    2,
  );
}

function usage() {
  return [
    "Usage: node scripts/provider-e2e-report-policy.mjs --manifest provider-e2e-evidence.json [--allow-mocked-core-providers]",
    "",
    "Validates retained provider E2E reports for canonical fal.ai, OpenAI, xAI, Google, Replicate, WAN/Kling/Seedance reference-video, source-video edit, audio/video-to-audio, replacement, failure, cancellation, and retry runs.",
    "The manifest must list provider-e2e-report.json paths produced by pnpm e2e:providers.",
    "--allow-mocked-core-providers is for local no-spend validation only; omit it for release evidence that must prove live fal.ai/Replicate execution.",
    "",
    "Manifest shape:",
    exampleManifest(),
  ].join("\n");
}

function parseArgs(argv) {
  const args = argv.filter((arg) => arg !== "--");
  const options = { manifest: null, allowMockedCoreProviders: false };

  for (let index = 0; index < args.length; index += 1) {
    const arg = args[index];
    if (arg === "--manifest") {
      options.manifest = requireValue(arg, args[++index]);
    } else if (arg === "--allow-mocked-core-providers") {
      options.allowMockedCoreProviders = true;
    } else if (arg === "--help" || arg === "-h") {
      console.log(usage());
      process.exit(0);
    } else {
      throw new Error(`Unknown argument: ${arg}\n\n${usage()}`);
    }
  }
  if (!options.manifest) {
    throw new Error(`Missing --manifest.\n\n${usage()}`);
  }
  return options;
}

function requireValue(flag, value) {
  if (!value || value.startsWith("--")) {
    throw new Error(`Missing value for ${flag}.`);
  }
  return value;
}

function readJson(path, label) {
  const resolved = resolve(path);
  if (!existsSync(resolved)) {
    throw new Error(`${label} was not found at ${resolved}`);
  }
  return JSON.parse(readFileSync(resolved, "utf8"));
}

function isWithinDirectory(rootPath, candidatePath) {
  const root = resolve(rootPath);
  const candidate = resolve(candidatePath);
  return candidate === root || candidate.startsWith(`${root}${sep}`);
}

function isProviderUrl(value) {
  try {
    const url = new URL(value);
    return (
      (url.protocol === "https:" || url.protocol === "http:") &&
      !["localhost", "127.0.0.1", "::1"].includes(url.hostname)
    );
  } catch {
    return false;
  }
}

function isImageDataUrl(value) {
  return /^data:image\/(png|jpeg|webp);base64,[A-Za-z0-9+/=]+$/.test(value);
}

function isValidOutputSourceUrl(provider, scenario, value) {
  if (
    (provider === "openai" ||
      provider === "elevenlabs" ||
      (provider === "fal.ai" && scenario === "text-to-music") ||
      (provider === "fal.ai" && isTextToMusicInsertScenario(scenario)) ||
      (provider === "google" &&
        (scenario === "text-to-audio" ||
          scenario === "text-to-music" ||
          isTextToMusicInsertScenario(scenario))) ||
      (provider === "minimax" && scenario === "text-to-music") ||
      (provider === "minimax" && isTextToMusicInsertScenario(scenario))) &&
    (value === undefined || value === null)
  ) {
    return true;
  }
  return isProviderUrl(value);
}

function providerRequestsEqual(left, right) {
  return (
    left?.provider === right?.provider &&
    left?.requestId === right?.requestId &&
    left?.statusUrl === right?.statusUrl &&
    left?.responseUrl === right?.responseUrl &&
    left?.cancelUrl === right?.cancelUrl &&
    left?.submittedAt === right?.submittedAt
  );
}

function normalizedScenario(report) {
  const scenario = typeof report?.scenario === "string" && report.scenario.trim() !== ""
    ? report.scenario
    : report?.result && typeof report.result.scenario === "string" && report.result.scenario.trim() !== ""
      ? report.result.scenario
      : "text-to-image";
  return scenario;
}

function providerLifecycleScenario(scenario) {
  if (scenario === "provider-failure") {
    return {
      assetStatus: "failed",
      jobStatus: "Failed",
      jobSidecarStatus: "failed",
      reason: "provider_error",
    };
  }
  if (
    scenario === "local-image-upscale-failure" ||
    scenario === "local-image-edit-failure" ||
    scenario === "local-video-upscale-failure" ||
    scenario === "local-video-edit-failure"
  ) {
    return {
      assetStatus: "failed",
      jobStatus: "Failed",
      jobSidecarStatus: "failed",
      reason: "provider_error",
    };
  }
  if (scenario === "provider-cancellation") {
    return {
      assetStatus: "failed",
      jobStatus: "Cancelled",
      jobSidecarStatus: "cancelled",
      reason: "cancelled_by_user",
    };
  }
  if (
    scenario === "local-image-upscale-cancellation" ||
    scenario === "local-image-edit-cancellation" ||
    scenario === "local-video-upscale-cancellation" ||
    scenario === "local-video-edit-cancellation"
  ) {
    return {
      assetStatus: "failed",
      jobStatus: "Cancelled",
      jobSidecarStatus: "cancelled",
      reason: "cancelled_by_user",
    };
  }
  return null;
}

function isProviderRetryScenario(scenario) {
  return (
    scenario === "provider-retry" ||
    scenario === "local-image-upscale-retry" ||
    scenario === "local-image-edit-retry" ||
    scenario === "local-video-upscale-retry" ||
    scenario === "local-video-edit-retry"
  );
}

function isVideoToAudioScenario(scenario) {
  return (
    scenario === "video-to-music" ||
    scenario === "local-video-to-music-insert" ||
    scenario === "local-video-to-sfx-insert" ||
    scenario === "video-to-sfx"
  );
}

function isVideoToMusicScenario(scenario) {
  return scenario === "video-to-music" || scenario === "local-video-to-music-insert";
}

function isTimelineAudioInsertScenario(scenario) {
  return scenario === "local-video-to-music-insert" || scenario === "local-video-to-sfx-insert";
}

function isTextToMusicInsertScenario(scenario) {
  return scenario === "text-to-music-insert";
}

function isTimelineVisualInsertScenario(scenario) {
  return scenario === "text-to-video-insert";
}

function timelineAudioInsertConfig(scenario) {
  if (scenario === "local-video-to-sfx-insert") {
    return {
      sourceItemId: "item-video-to-sfx-source",
      placementIntent: "insert-audio:item-video-to-sfx-source",
      insertedItemId: "item-video-to-sfx-inserted-audio",
      timelineStartSeconds: 8,
      videoSourceStartSeconds: 0.5,
      videoSourceEndSeconds: 2,
    };
  }
  return {
    sourceItemId: "item-video-to-music-source",
    placementIntent: "insert-audio:item-video-to-music-source",
    insertedItemId: "item-video-to-music-inserted-audio",
    timelineStartSeconds: 12,
    videoSourceStartSeconds: 1.25,
    videoSourceEndSeconds: 5.75,
  };
}

function timelineVisualInsertConfig() {
  return {
    trackId: "track-video",
    placementIntent: "insert-video:track-video",
    insertedItemId: "item-text-video-inserted-video",
    timelineStartSeconds: 6,
  };
}

function timelineTextToMusicInsertConfig() {
  return {
    trackId: "track-audio",
    placementIntent: "insert-audio:track-audio",
    insertedItemId: "item-text-music-inserted-audio",
    timelineStartSeconds: 12,
  };
}

function isImageEditScenario(scenario) {
  return (
    scenario === "image-edit" ||
    scenario === "local-image-edit-replace" ||
    scenario === "local-image-edit-retry" ||
    scenario === "local-image-edit-cancellation" ||
    scenario === "local-image-edit-failure"
  );
}

function isLocalImageUpscaleScenario(scenario) {
  return (
    scenario === "local-image-upscale" ||
    scenario === "local-image-upscale-replace" ||
    scenario === "local-image-upscale-retry" ||
    scenario === "local-image-upscale-cancellation" ||
    scenario === "local-image-upscale-failure"
  );
}

function isLocalVideoUpscaleScenario(scenario) {
  return (
    scenario === "local-video-upscale" ||
    scenario === "local-video-upscale-replace" ||
    scenario === "local-video-upscale-retry" ||
    scenario === "local-video-upscale-cancellation" ||
    scenario === "local-video-upscale-failure"
  );
}

function isMultiImageScenario(scenario) {
  return scenario === "multi-image";
}

function isReplicateFluxVariantScenario(scenario) {
  return (
    scenario === "replicate-flux-dev" ||
    scenario === "replicate-flux-1.1-pro" ||
    scenario === "replicate-flux-1.1-pro-ultra"
  );
}

function isProviderVideoGenerationScenario(scenario) {
  return (
    scenario === "wan-image-to-video" ||
    scenario === "wan-reference-to-video" ||
    scenario === "kling-image-to-video" ||
    scenario === "replicate-video" ||
    scenario === "replicate-video-fast" ||
    scenario === "xai-video" ||
    scenario === "google-video" ||
    scenario === "text-to-video-replace" ||
    scenario === "text-to-video-insert" ||
    scenario === "video-to-video" ||
    scenario === "local-video-edit-replace" ||
    scenario === "local-video-motion-control-replace" ||
    scenario === "local-video-edit-retry" ||
    scenario === "local-video-edit-cancellation" ||
    scenario === "local-video-edit-failure"
  );
}

function isReplacementScenario(scenario) {
  return (
    scenario === "text-to-image-replace" ||
    scenario === "text-to-video-replace" ||
    scenario === "local-image-upscale-replace" ||
    scenario === "local-image-edit-replace" ||
    scenario === "local-video-upscale-replace" ||
    scenario === "local-video-edit-replace" ||
    scenario === "local-video-motion-control-replace"
  );
}

function replacementScenarioTargetItemId(scenario) {
  if (scenario === "text-to-image-replace") {
    return "item-text-image-target";
  }
  if (scenario === "text-to-video-replace") {
    return "item-text-video-target";
  }
  if (scenario === "local-image-edit-replace") {
    return "item-image-edit-target";
  }
  if (scenario === "local-video-edit-replace") {
    return "item-video-edit-target";
  }
  if (scenario === "local-video-motion-control-replace") {
    return "item-video-motion-control-target";
  }
  if (scenario === "local-video-upscale-replace") {
    return "item-video-upscale-target";
  }
  return "item-upscale-target";
}

function replacementScenarioLinkedItemId(scenario) {
  if (isReplacementScenario(scenario)) {
    return `${replacementScenarioTargetItemId(scenario)}-linked`;
  }
  return null;
}

function allowsNoSpendMockedEvidence(report, options = {}) {
  return (
    (report.provider === "openai" ||
      report.provider === "xai" ||
      report.provider === "google" ||
      report.provider === "elevenlabs" ||
      report.provider === "minimax" ||
      (options.allowMockedCoreProviders === true &&
        (report.provider === "fal.ai" || report.provider === "replicate")) ||
      (report.provider === "replicate" && normalizedScenario(report) === "replicate-local-file-upload")) &&
    report.mockedProviderService === true &&
    report.command === null
  );
}

function reportKey({ provider, scenario }) {
  return `${provider}\u0000${scenario}`;
}

function validateManifest(manifestPath, options = {}) {
  let manifest;
  try {
    manifest = readJson(manifestPath, "Provider E2E evidence manifest");
  } catch {
    const rootDir = process.cwd();
    return {
      manifest: resolve(manifestPath),
      projectRoot: rootDir,
      status: "failed",
      failures: [`${resolve(manifestPath)}: provider E2E evidence manifest is not parseable`],
      providers: [],
      requiredProviders: Array.from(requiredProviders),
      requiredReports,
    };
  }
  const rootDir = resolve(manifest.projectRoot || process.cwd());
  const reportPaths = Array.isArray(manifest.reports) ? manifest.reports : [];
  const failures = [];
  const seenProviders = new Set();
  const seenReportKeys = new Set();

  if (reportPaths.length === 0) {
    failures.push("manifest.reports must list retained provider-e2e-report.json files");
  }

  for (const reportPathValue of reportPaths) {
    if (typeof reportPathValue !== "string" || reportPathValue.trim() === "") {
      failures.push("manifest.reports entries must be non-empty strings");
      continue;
    }
    if (isAbsolute(reportPathValue) || reportPathValue.split(/[\\/]/).includes("..")) {
      failures.push(`report path must be project-relative and must not escape the project: ${reportPathValue}`);
      continue;
    }
    const reportPath = join(rootDir, reportPathValue);
    if (!existsSync(reportPath)) {
      failures.push(`${reportPath}: provider E2E report is missing`);
      continue;
    }
    let report;
    try {
      report = readJson(reportPath, `Provider E2E report ${reportPathValue}`);
    } catch {
      failures.push(`${reportPath}: provider E2E report is not parseable`);
      continue;
    }
    validateReport(report, reportPath, failures, seenProviders, seenReportKeys, options);
  }

  for (const requiredReport of requiredReports) {
    if (!seenReportKeys.has(reportKey(requiredReport))) {
      failures.push(
        `missing retained provider E2E report for ${requiredReport.provider} scenario ${requiredReport.scenario}`,
      );
    }
  }

  return {
    manifest: resolve(manifestPath),
    projectRoot: rootDir,
    status: failures.length === 0 ? "passed" : "failed",
    failures,
    providers: Array.from(seenProviders).sort(),
    requiredProviders: Array.from(requiredProviders),
    scenarios: Array.from(seenReportKeys)
      .map((key) => {
        const [provider, scenario] = key.split("\u0000");
        return { provider, scenario };
      })
      .sort((left, right) =>
        left.provider === right.provider
          ? left.scenario.localeCompare(right.scenario)
          : left.provider.localeCompare(right.provider),
      ),
    requiredReports,
  };
}

function validateReport(report, reportPath, failures, seenProviders, seenReportKeys, options = {}) {
  const reportDir = resolve(reportPath, "..");
  const scenario = normalizedScenario(report);
  if (report.ok !== true) {
    failures.push(`${reportPath}: ok must be true`);
  }
  if (!requiredProviders.has(report.provider)) {
    failures.push(`${reportPath}: unsupported provider ${JSON.stringify(report.provider)}`);
  } else {
    seenProviders.add(report.provider);
  }
  const key = reportKey({ provider: report.provider, scenario });
  if (seenReportKeys.has(key)) {
    failures.push(`${reportPath}: duplicate retained report for ${report.provider} scenario ${scenario}`);
  } else {
    seenReportKeys.add(key);
  }
  const allowsNoSpendLifecycleEvidence =
    providerLifecycleScenario(scenario) || isProviderRetryScenario(scenario);
  if (
    report.credentialStatus !== "present" &&
    !(allowsNoSpendLifecycleEvidence && report.credentialStatus === "not_checked") &&
    !(allowsNoSpendMockedEvidence(report, options) && report.credentialStatus === "not_checked")
  ) {
    failures.push(`${reportPath}: credentialStatus must be present`);
  }
  if (
    (report.provider === "openai" ||
      report.provider === "xai" ||
      report.provider === "google" ||
      report.provider === "elevenlabs" ||
      report.provider === "minimax" ||
      (options.allowMockedCoreProviders === true &&
        (report.provider === "fal.ai" || report.provider === "replicate"))) &&
    report.credentialStatus === "not_checked"
  ) {
    const label =
      report.provider === "openai"
        ? "OpenAI"
        : report.provider === "xai"
          ? "xAI"
          : report.provider === "google"
          ? "Google"
          : report.provider === "elevenlabs"
            ? "ElevenLabs"
            : report.provider === "minimax"
              ? "MiniMax"
              : report.provider;
    if (report.mockedProviderService !== true) {
      failures.push(`${reportPath}: ${label} no-spend evidence must be mockedProviderService true`);
    }
    if (report.command !== null) {
      failures.push(`${reportPath}: ${label} no-spend evidence must not run a provider command`);
    }
  }
  if (typeof report.model !== "string" || report.model.trim() === "") {
    failures.push(`${reportPath}: model must be non-empty`);
  }
  if (report.exitCode !== 0) {
    failures.push(`${reportPath}: exitCode must be 0`);
  }
  if (report.error !== null) {
    failures.push(`${reportPath}: error must be null`);
  }
  if (typeof report.reportPath === "string" && report.reportPath.trim() !== "") {
    const retainedPath = isAbsolute(report.reportPath)
      ? report.reportPath
      : join(reportDir, report.reportPath);
    if (!existsSync(retainedPath)) {
      failures.push(`${reportPath}: reportPath does not point to retained JSON`);
    }
  }
  validateChildResult(report, reportPath, failures, scenario);
}

function validateChildResult(report, reportPath, failures, scenario) {
  const result = report.result;
  if (!result || typeof result !== "object") {
    failures.push(`${reportPath}: result must be an object`);
    return;
  }
  const lifecycleScenario = providerLifecycleScenario(scenario);
  if (lifecycleScenario) {
    validateProviderLifecycleResult(report, reportPath, failures, lifecycleScenario);
    return;
  }
  if (result.ok !== true) {
    failures.push(`${reportPath}: result.ok must be true`);
  }
  if (result.provider !== report.provider) {
    failures.push(`${reportPath}: result.provider must match report.provider`);
  }
  if (result.model !== report.model) {
    failures.push(`${reportPath}: result.model must match report.model`);
  }
  if (
    typeof result.scenario === "string" &&
    result.scenario.trim() !== "" &&
    result.scenario !== scenario
  ) {
    failures.push(`${reportPath}: result.scenario must match report.scenario`);
  }
  if (isUploadOnlyScenario(report.provider, scenario)) {
    validateUploadOnlyResult(report, reportPath, failures);
    return;
  }
  if (result.jobStatus !== "Completed") {
    failures.push(`${reportPath}: result.jobStatus must be Completed`);
  }
  if (!Number.isInteger(result.outputCount) || result.outputCount < 1) {
    failures.push(`${reportPath}: result.outputCount must be at least 1`);
  }
  if (isMultiImageScenario(scenario) && result.outputCount < 2) {
    failures.push(`${reportPath}: multi-image result.outputCount must be at least 2`);
  }
  if (typeof result.projectDir !== "string" || result.projectDir.trim() === "") {
    failures.push(`${reportPath}: result.projectDir is required`);
    return;
  }
  if (typeof result.artifactPath !== "string" || result.artifactPath.trim() === "") {
    failures.push(`${reportPath}: result.artifactPath is required`);
    return;
  }
  if (!isWithinDirectory(result.projectDir, result.artifactPath)) {
    failures.push(`${reportPath}: result.artifactPath must stay inside result.projectDir`);
    return;
  }
  if (!existsSync(result.artifactPath) || !statSync(result.artifactPath).isFile() || statSync(result.artifactPath).size < 1) {
    failures.push(`${reportPath}: result.artifactPath must be a retained nonempty file`);
  }
  validateProviderRequest(report, reportPath, failures);
  validateSplitProjectGeneratedOutput(report, reportPath, failures, scenario);
}

function isUploadOnlyScenario(provider, scenario) {
  return provider === "replicate" && scenario === "replicate-local-file-upload";
}

function validateUploadOnlyResult(report, reportPath, failures) {
  const result = report.result;
  if (result.jobStatus !== "Uploaded") {
    failures.push(`${reportPath}: upload-only result.jobStatus must be Uploaded`);
  }
  if (result.outputCount !== 0) {
    failures.push(`${reportPath}: upload-only result.outputCount must be 0`);
  }
  if (typeof result.projectDir !== "string" || result.projectDir.trim() === "") {
    failures.push(`${reportPath}: result.projectDir is required`);
    return;
  }
  if (typeof result.artifactPath !== "string" || result.artifactPath.trim() === "") {
    failures.push(`${reportPath}: result.artifactPath is required`);
    return;
  }
  if (!isWithinDirectory(result.projectDir, result.artifactPath)) {
    failures.push(`${reportPath}: result.artifactPath must stay inside result.projectDir`);
    return;
  }
  if (!existsSync(result.artifactPath) || !statSync(result.artifactPath).isFile() || statSync(result.artifactPath).size < 1) {
    failures.push(`${reportPath}: result.artifactPath must be a retained nonempty file`);
  }
  if (!isProviderUrl(result.providerFileUrl)) {
    failures.push(`${reportPath}: upload-only result.providerFileUrl must be a provider URL`);
  }
}

function validateProviderRequest(report, reportPath, failures) {
  const request = report.result?.providerRequest;
  if (!request || typeof request !== "object") {
    failures.push(`${reportPath}: result.providerRequest is required`);
    return;
  }
  if (request.provider !== report.provider) {
    failures.push(`${reportPath}: providerRequest.provider must match report.provider`);
  }
  for (const field of ["requestId", "statusUrl", "responseUrl", "cancelUrl", "submittedAt"]) {
    if (typeof request[field] !== "string" || request[field].trim() === "") {
      failures.push(`${reportPath}: providerRequest.${field} must be non-empty`);
    }
  }
  for (const field of ["statusUrl", "responseUrl", "cancelUrl"]) {
    if (typeof request[field] === "string" && !isProviderUrl(request[field])) {
      failures.push(`${reportPath}: providerRequest.${field} must be a provider URL`);
    }
  }
}

function validateSplitProjectGeneratedOutput(report, reportPath, failures, scenario) {
  const projectDir = report.result?.projectDir;
  if (typeof projectDir !== "string" || projectDir.trim() === "") {
    return;
  }
  const manifestPath = join(projectDir, "video-creater.project.json");
  if (!existsSync(manifestPath)) {
    failures.push(`${reportPath}: split project manifest is missing`);
    return;
  }
  let manifest;
  try {
    manifest = JSON.parse(readFileSync(manifestPath, "utf8"));
  } catch {
    failures.push(`${reportPath}: split project manifest is not parseable`);
    return;
  }
  const generatedDir = manifest.files?.generated;
  const mediaPathValue = manifest.files?.media;
  if (typeof generatedDir !== "string" || generatedDir.trim() === "") {
    failures.push(`${reportPath}: manifest.files.generated is required`);
    return;
  }
  if (typeof mediaPathValue !== "string" || mediaPathValue.trim() === "") {
    failures.push(`${reportPath}: manifest.files.media is required`);
    return;
  }
  const generatedRoot = join(projectDir, generatedDir);
  const generatedIndexPath = join(generatedRoot, "index.json");
  if (!isWithinDirectory(projectDir, generatedIndexPath) || !existsSync(generatedIndexPath)) {
    failures.push(`${reportPath}: generated index is missing or escapes project`);
    return;
  }
  let generatedIndex;
  try {
    generatedIndex = JSON.parse(readFileSync(generatedIndexPath, "utf8"));
  } catch {
    failures.push(`${reportPath}: generated index is not parseable`);
    return;
  }
  const completedAssets = Array.isArray(generatedIndex.assets)
    ? generatedIndex.assets.filter((asset) => asset.status === "completed" && asset.outputCount > 0)
    : [];
  if (completedAssets.length === 0) {
    failures.push(`${reportPath}: generated index has no completed output-backed assets`);
  }
  const outputMediaIds = new Set();
  const outputRelativePathsByMediaId = new Map();
  const outputMetadataByMediaId = new Map();
  const completedOutputPaths = new Set();
  const completedOutputRelativePaths = new Set();
  const completedOutputSourceUrls = new Set();
  const completedAssetIds = new Set();
  let completedOutputCount = 0;
  for (const asset of completedAssets) {
    if (typeof asset.assetId === "string" && asset.assetId.trim() !== "") {
      if (completedAssetIds.has(asset.assetId)) {
        failures.push(`${reportPath}: duplicate generated index assetId: ${asset.assetId}`);
      } else {
        completedAssetIds.add(asset.assetId);
      }
    }
    if (asset.modelProvider !== report.provider || asset.modelId !== report.model) {
      failures.push(`${reportPath}: generated index model metadata must match retained report`);
    }
    const sidecarPath = join(projectDir, asset.path || "");
    if (!isWithinDirectory(generatedRoot, sidecarPath) || !existsSync(sidecarPath)) {
      failures.push(`${reportPath}: generated sidecar is missing or outside generated directory`);
      continue;
    }
    let sidecar;
    try {
      sidecar = JSON.parse(readFileSync(sidecarPath, "utf8"));
    } catch {
      failures.push(`${reportPath}: generated sidecar is not parseable`);
      continue;
    }
    if (sidecar.id !== asset.assetId) {
      failures.push(`${reportPath}: generated index assetId must match sidecar id`);
    }
    if (sidecar.status !== asset.status) {
      failures.push(`${reportPath}: generated index status must match sidecar status`);
    }
    if (sidecar.model?.provider !== report.provider || sidecar.model?.id !== report.model) {
      failures.push(`${reportPath}: generated sidecar model does not match retained report`);
    }
    if (
      scenario === "wan-image-to-video" ||
      scenario === "wan-reference-to-video" ||
      scenario === "kling-image-to-video" ||
      scenario === "replicate-video" ||
      scenario === "replicate-video-fast"
    ) {
      validateReferencedVideoSidecar(reportPath, sidecar, scenario, failures);
    }
    if (scenario === "wan-reference-to-video") {
      validateWanReferenceVideoSettings(reportPath, sidecar, failures);
    }
    if (
      scenario === "video-to-video" ||
      scenario === "local-video-upscale" ||
      scenario === "local-video-upscale-replace" ||
      scenario === "local-video-upscale-retry" ||
      scenario === "local-video-edit-replace" ||
      scenario === "local-video-motion-control-replace" ||
      scenario === "local-video-edit-retry"
    ) {
      if (scenario === "local-video-motion-control-replace") {
        validateMotionControlSidecar(reportPath, sidecar, failures);
      } else {
        validateSourceVideoSidecar(reportPath, sidecar, "video-to-video", failures);
        if (isLocalVideoUpscaleScenario(scenario)) {
          validateSourceVideoTrimSettings(reportPath, sidecar, "video-to-video", 0.5, 4.5, failures);
        }
      }
    }
    if (isVideoToAudioScenario(scenario)) {
      validateSourceVideoSidecar(reportPath, sidecar, "video-to-audio", failures);
    }
    if (isImageEditScenario(scenario)) {
      validateImageEditSidecar(reportPath, report.provider, sidecar, failures);
    }
    if (scenario === "google-video") {
      validateGoogleVeoSidecar(reportPath, sidecar, failures);
    }
    if (scenario === "xai-video") {
      validateXaiGrokVideoSidecar(reportPath, sidecar, failures);
    }
    if (isLocalImageUpscaleScenario(scenario)) {
      validateImageUpscaleSidecar(reportPath, sidecar, failures);
    }
    if (isReplacementScenario(scenario)) {
      const expectedPlacementIntent = `replace:${replacementScenarioTargetItemId(scenario)}`;
      if (sidecar.placementIntent !== expectedPlacementIntent) {
        failures.push(`${reportPath}: replacement scenario sidecar must retain placementIntent`);
      }
    }
    if (isTimelineAudioInsertScenario(scenario)) {
      const insertConfig = timelineAudioInsertConfig(scenario);
      const expectedPlacementIntent = insertConfig.placementIntent;
      if (scenario === "local-video-to-sfx-insert" && sidecar.prompt !== "") {
        failures.push(`${reportPath}: audio insert scenario sidecar must retain a promptless request`);
      }
      if (
        scenario === "local-video-to-music-insert" &&
        (typeof sidecar.prompt !== "string" || sidecar.prompt.trim() === "")
      ) {
        failures.push(`${reportPath}: video-to-music insert sidecar must retain prompt guidance`);
      }
      if (sidecar.placementIntent !== expectedPlacementIntent) {
        failures.push(`${reportPath}: audio insert scenario sidecar must retain placementIntent`);
      }
      if (
        sidecar.settings?.timelineStartSeconds !== insertConfig.timelineStartSeconds ||
        sidecar.settings?.videoSourceStartSeconds !== insertConfig.videoSourceStartSeconds ||
        sidecar.settings?.videoSourceEndSeconds !== insertConfig.videoSourceEndSeconds
      ) {
        failures.push(`${reportPath}: audio insert scenario sidecar must retain selected source trim settings`);
      }
    }
    if (
      scenario === "video-to-music" &&
      (typeof sidecar.prompt !== "string" || sidecar.prompt.trim() === "")
    ) {
      failures.push(`${reportPath}: video-to-music sidecar must retain prompt guidance`);
    }
    if (isTextToMusicInsertScenario(scenario)) {
      const insertConfig = timelineTextToMusicInsertConfig();
      if (sidecar.placementIntent !== insertConfig.placementIntent) {
        failures.push(`${reportPath}: text-to-music insert scenario sidecar must retain placementIntent`);
      }
      if (sidecar.settings?.timelineStartSeconds !== insertConfig.timelineStartSeconds) {
        failures.push(`${reportPath}: text-to-music insert scenario sidecar must retain timelineStartSeconds`);
      }
    }
    if (isTimelineVisualInsertScenario(scenario)) {
      const insertConfig = timelineVisualInsertConfig();
      if (sidecar.placementIntent !== insertConfig.placementIntent) {
        failures.push(`${reportPath}: visual insert scenario sidecar must retain placementIntent`);
      }
      if (
        sidecar.settings?.timelineStartSeconds !== insertConfig.timelineStartSeconds ||
        typeof sidecar.settings?.generateAudio !== "boolean"
      ) {
        failures.push(`${reportPath}: visual insert scenario sidecar must retain timeline settings`);
      }
    }
    if (isMultiImageScenario(scenario) && sidecar.settings?.numImages !== 2) {
      failures.push(`${reportPath}: multi-image sidecar settings.numImages must be 2`);
    }
    if (
      isProviderVideoGenerationScenario(scenario) &&
      typeof sidecar.settings?.generateAudio !== "boolean"
    ) {
      failures.push(`${reportPath}: provider video sidecar settings.generateAudio must be retained`);
    }
    const expectedTextToAudioVoice =
      scenario === "text-to-audio" ? textToAudioDefaultVoices[report.provider] : undefined;
    if (
      expectedTextToAudioVoice &&
      (sidecar.settings?.category !== "tts" || sidecar.settings?.voice !== expectedTextToAudioVoice)
    ) {
      failures.push(`${reportPath}: TTS sidecar settings.voice must retain the catalog default`);
    }
    if (
      (scenario === "text-to-music" ||
        isTextToMusicInsertScenario(scenario) ||
        isVideoToMusicScenario(scenario)) &&
      (sidecar.settings?.category !== textToMusicDefaultSettings.category ||
        sidecar.settings?.durationSeconds !== textToMusicDefaultSettings.durationSeconds ||
        sidecar.settings?.instrumental !== textToMusicDefaultSettings.instrumental ||
        sidecar.settings?.styleInstructions !== textToMusicDefaultSettings.styleInstructions)
    ) {
      failures.push(`${reportPath}: music sidecar settings must retain music defaults`);
    }
    if (
      report.provider === "elevenlabs" &&
      (scenario === "text-to-music" || isTextToMusicInsertScenario(scenario)) &&
      sidecar.settings?.lyrics !== elevenlabsTextToMusicDefaultLyrics
    ) {
      failures.push(`${reportPath}: ElevenLabs text-to-music sidecar settings.lyrics must be retained`);
    }
    if (
      report.provider === "google" &&
      (scenario === "text-to-music" || isTextToMusicInsertScenario(scenario)) &&
      sidecar.settings?.lyrics !== googleLyriaTextToMusicDefaultLyrics
    ) {
      failures.push(`${reportPath}: Google Lyria text-to-music sidecar settings.lyrics must be retained`);
    }
    if (
      report.provider === "minimax" &&
      (scenario === "text-to-music" || isTextToMusicInsertScenario(scenario)) &&
      sidecar.settings?.lyrics !== minimaxTextToMusicDefaultLyrics
    ) {
      failures.push(`${reportPath}: MiniMax text-to-music sidecar settings.lyrics must be retained`);
    }
    const sidecarOutputs = Array.isArray(sidecar.outputs) ? sidecar.outputs : [];
    if (asset.outputCount !== sidecarOutputs.length) {
      failures.push(`${reportPath}: generated index outputCount must match sidecar outputs`);
    }
    const sidecarOutputMediaIds = new Set();
    const sidecarOutputRelativePaths = new Set();
    for (const output of sidecarOutputs) {
      if (typeof output.mediaId !== "string" || output.mediaId.trim() === "") {
        failures.push(`${reportPath}: completed sidecar output mediaId is required`);
      } else {
        if (sidecarOutputMediaIds.has(output.mediaId)) {
          failures.push(`${reportPath}: duplicate completed sidecar output mediaId: ${output.mediaId}`);
        } else {
          sidecarOutputMediaIds.add(output.mediaId);
          if (outputMediaIds.has(output.mediaId)) {
            failures.push(`${reportPath}: duplicate completed sidecar output mediaId across generated assets: ${output.mediaId}`);
          } else {
            outputMediaIds.add(output.mediaId);
          }
        }
      }
      if (typeof output.relativePath !== "string" || output.relativePath.trim() === "") {
        failures.push(`${reportPath}: completed sidecar output relativePath is required`);
      } else {
        if (sidecarOutputRelativePaths.has(output.relativePath)) {
          failures.push(`${reportPath}: duplicate completed sidecar output relativePath: ${output.relativePath}`);
        } else {
          sidecarOutputRelativePaths.add(output.relativePath);
          if (completedOutputRelativePaths.has(output.relativePath)) {
            failures.push(`${reportPath}: duplicate completed sidecar output relativePath across generated assets: ${output.relativePath}`);
          } else {
            completedOutputRelativePaths.add(output.relativePath);
          }
        }
      }
      if (typeof output.mediaId === "string" && typeof output.relativePath === "string") {
        outputRelativePathsByMediaId.set(output.mediaId, output.relativePath);
        outputMetadataByMediaId.set(output.mediaId, {
          durationSeconds: output.durationSeconds,
          width: output.width,
          height: output.height,
          fps: output.fps,
        });
      }
      completedOutputCount += 1;
      const outputPath = join(projectDir, output.relativePath || "");
      if (!isWithinDirectory(generatedRoot, outputPath) || !existsSync(outputPath)) {
        failures.push(`${reportPath}: generated sidecar output is missing or outside generated directory`);
      } else if (!statSync(outputPath).isFile() || statSync(outputPath).size < 1) {
        failures.push(`${reportPath}: generated sidecar output must be a retained nonempty file`);
      } else {
        completedOutputPaths.add(resolve(outputPath));
      }
      if (!isValidOutputSourceUrl(report.provider, scenario, output.sourceUrl)) {
        failures.push(
          report.provider === "openai"
            ? `${reportPath}: OpenAI generated sidecar output sourceUrl must be omitted or a provider URL`
            : report.provider === "elevenlabs"
              ? `${reportPath}: ElevenLabs generated sidecar output sourceUrl must be omitted or a provider URL`
            : report.provider === "fal.ai" && scenario === "text-to-music"
              ? `${reportPath}: fal.ai text-to-music generated sidecar output sourceUrl must be omitted or a provider URL`
              : report.provider === "minimax"
                ? `${reportPath}: MiniMax generated sidecar output sourceUrl must be omitted or a provider URL`
              : report.provider === "google" && scenario === "text-to-audio"
                ? `${reportPath}: Google TTS generated sidecar output sourceUrl must be omitted or a provider URL`
            : `${reportPath}: generated sidecar output sourceUrl must be a provider URL`,
        );
      } else if (typeof output.sourceUrl === "string" && output.sourceUrl.trim() !== "") {
        if (completedOutputSourceUrls.has(output.sourceUrl)) {
          failures.push(`${reportPath}: duplicate completed sidecar output sourceUrl: ${output.sourceUrl}`);
        } else {
          completedOutputSourceUrls.add(output.sourceUrl);
        }
      }
      if (isProviderVideoGenerationScenario(scenario)) {
        if (typeof output.relativePath !== "string" || !output.relativePath.endsWith(".mp4")) {
          failures.push(`${reportPath}: referenced video output must be an MP4 artifact`);
        }
        if (!Number.isFinite(output.durationSeconds) || output.durationSeconds <= 0) {
          failures.push(`${reportPath}: referenced video output durationSeconds must be positive`);
        }
        if (!Number.isFinite(output.fps) || output.fps <= 0) {
          failures.push(`${reportPath}: referenced video output fps must be positive`);
        }
      }
      if (scenario === "text-to-audio") {
        const expectedExtension = report.provider === "google" ? ".wav" : ".mp3";
        const expectedLabel = report.provider === "google" ? "WAV" : "MP3";
        const expectedArticle = report.provider === "google" ? "a" : "an";
        if (
          typeof output.relativePath !== "string" ||
          !output.relativePath.endsWith(expectedExtension)
        ) {
          failures.push(
            `${reportPath}: text-to-audio output must be ${expectedArticle} ${expectedLabel} artifact`,
          );
        }
        if (!Number.isFinite(output.durationSeconds) || output.durationSeconds <= 0) {
          failures.push(`${reportPath}: text-to-audio output durationSeconds must be positive`);
        }
      }
      if (scenario === "text-to-image") {
        if (typeof output.relativePath !== "string" || !output.relativePath.endsWith(".png")) {
          failures.push(`${reportPath}: text-to-image output must be a PNG artifact`);
        }
        if (!Number.isFinite(output.width) || output.width <= 0) {
          failures.push(`${reportPath}: text-to-image output width must be positive`);
        }
        if (!Number.isFinite(output.height) || output.height <= 0) {
          failures.push(`${reportPath}: text-to-image output height must be positive`);
        }
      }
      if (scenario === "text-to-music" || isTextToMusicInsertScenario(scenario)) {
        const expectedExtension = report.provider === "fal.ai" ? ".m4a" : ".mp3";
        const expectedLabel = report.provider === "fal.ai" ? "M4A" : "MP3";
        if (
          typeof output.relativePath !== "string" ||
          !output.relativePath.endsWith(expectedExtension)
        ) {
          failures.push(`${reportPath}: text-to-music output must be a ${expectedLabel} artifact`);
        }
        if (!Number.isFinite(output.durationSeconds) || output.durationSeconds <= 0) {
          failures.push(`${reportPath}: text-to-music output durationSeconds must be positive`);
        }
      }
      if (isVideoToAudioScenario(scenario)) {
        const expectedExtension =
          scenario === "video-to-music" || scenario === "local-video-to-music-insert"
            ? ".m4a"
            : ".wav";
        if (
          typeof output.relativePath !== "string" ||
          !output.relativePath.endsWith(expectedExtension)
        ) {
          failures.push(`${reportPath}: video-to-audio output must be a ${expectedExtension} artifact`);
        }
        if (!Number.isFinite(output.durationSeconds) || output.durationSeconds <= 0) {
          failures.push(`${reportPath}: video-to-audio output durationSeconds must be positive`);
        }
      }
      if (isImageEditScenario(scenario)) {
        if (typeof output.relativePath !== "string" || !output.relativePath.endsWith(".png")) {
          failures.push(`${reportPath}: image-edit output must be a PNG artifact`);
        }
        if (!Number.isFinite(output.width) || output.width <= 0) {
          failures.push(`${reportPath}: image-edit output width must be positive`);
        }
        if (!Number.isFinite(output.height) || output.height <= 0) {
          failures.push(`${reportPath}: image-edit output height must be positive`);
        }
      }
      if (isLocalImageUpscaleScenario(scenario)) {
        if (typeof output.relativePath !== "string" || !output.relativePath.endsWith(".png")) {
          failures.push(`${reportPath}: image-upscale output must be a PNG artifact`);
        }
        if (!Number.isFinite(output.width) || output.width <= 0) {
          failures.push(`${reportPath}: image-upscale output width must be positive`);
        }
        if (!Number.isFinite(output.height) || output.height <= 0) {
          failures.push(`${reportPath}: image-upscale output height must be positive`);
        }
      }
      if (isMultiImageScenario(scenario)) {
        if (typeof output.relativePath !== "string" || !output.relativePath.endsWith(".png")) {
          failures.push(`${reportPath}: multi-image output must be a PNG artifact`);
        }
        if (!Number.isFinite(output.width) || output.width <= 0) {
          failures.push(`${reportPath}: multi-image output width must be positive`);
        }
        if (!Number.isFinite(output.height) || output.height <= 0) {
          failures.push(`${reportPath}: multi-image output height must be positive`);
        }
      }
      if (isReplicateFluxVariantScenario(scenario)) {
        if (typeof output.relativePath !== "string" || !output.relativePath.endsWith(".png")) {
          failures.push(`${reportPath}: Replicate Flux output must be a PNG artifact`);
        }
        if (!Number.isFinite(output.width) || output.width <= 0) {
          failures.push(`${reportPath}: Replicate Flux output width must be positive`);
        }
        if (!Number.isFinite(output.height) || output.height <= 0) {
          failures.push(`${reportPath}: Replicate Flux output height must be positive`);
        }
      }
      if (scenario === "krea-text-to-image" || scenario === "recraft-text-to-image") {
        const expectedExtension = scenario === "recraft-text-to-image" ? ".webp" : ".png";
        if (
          typeof output.relativePath !== "string" ||
          !output.relativePath.endsWith(expectedExtension)
        ) {
          failures.push(`${reportPath}: ${scenario} output must be a ${expectedExtension} artifact`);
        }
        if (!Number.isFinite(output.width) || output.width <= 0) {
          failures.push(`${reportPath}: ${scenario} output width must be positive`);
        }
        if (!Number.isFinite(output.height) || output.height <= 0) {
          failures.push(`${reportPath}: ${scenario} output height must be positive`);
        }
      }
      if (isReplacementScenario(scenario)) {
        validateReplacementTimelineEvidence(
          reportPath,
          projectDir,
          output.mediaId,
          failures,
          replacementScenarioTargetItemId(scenario),
          replacementScenarioLinkedItemId(scenario),
        );
      }
      if (isTimelineAudioInsertScenario(scenario)) {
        validateTimelineAudioInsertEvidence(
          reportPath,
          projectDir,
          output.mediaId,
          output.durationSeconds,
          scenario,
          failures,
        );
      }
      if (isTextToMusicInsertScenario(scenario)) {
        validateTimelineTextToMusicInsertEvidence(
          reportPath,
          projectDir,
          output.mediaId,
          output.durationSeconds,
          failures,
        );
      }
      if (isTimelineVisualInsertScenario(scenario)) {
        validateTimelineVisualInsertEvidence(
          reportPath,
          projectDir,
          output.mediaId,
          output.durationSeconds,
          failures,
        );
      }
    }
  }
  const artifactPath = report.result?.artifactPath;
  if (
    Number.isInteger(report.result?.outputCount) &&
    completedOutputCount > 0 &&
    report.result.outputCount !== completedOutputCount
  ) {
    failures.push(
      `${reportPath}: result.outputCount must match completed sidecar outputs (${completedOutputCount})`,
    );
  }
  if (
    typeof artifactPath === "string" &&
    artifactPath.trim() !== "" &&
    completedOutputPaths.size > 0 &&
    !completedOutputPaths.has(resolve(artifactPath))
  ) {
    failures.push(`${reportPath}: result.artifactPath must match a completed sidecar output`);
  }
  const mediaIndexPath = join(projectDir, mediaPathValue);
  if (!isWithinDirectory(projectDir, mediaIndexPath) || !existsSync(mediaIndexPath)) {
    failures.push(`${reportPath}: media index is missing or escapes project`);
    return;
  }
  let mediaIndex;
  try {
    mediaIndex = JSON.parse(readFileSync(mediaIndexPath, "utf8"));
  } catch {
    failures.push(`${reportPath}: media index is not parseable`);
    return;
  }
  const generatedMedia = Array.isArray(mediaIndex.assets)
    ? mediaIndex.assets.filter((asset) => asset.kind === "generated")
    : [];
  if (generatedMedia.length === 0) {
    failures.push(`${reportPath}: media index has no generated media row`);
  }
  const generatedMediaIds = new Set();
  for (const media of generatedMedia) {
    if (typeof media.id === "string" && media.id.trim() !== "") {
      if (generatedMediaIds.has(media.id)) {
        failures.push(`${reportPath}: duplicate generated media row id: ${media.id}`);
      } else {
        generatedMediaIds.add(media.id);
      }
    }
    if (!outputMediaIds.has(media.id)) {
      failures.push(`${reportPath}: generated media row is not backed by a completed sidecar output: ${media.id}`);
      continue;
    }
    const expectedRelativePath = outputRelativePathsByMediaId.get(media.id);
    if (expectedRelativePath && media.relativePath !== expectedRelativePath) {
      failures.push(`${reportPath}: generated media row relativePath must match completed sidecar output: ${media.id}`);
    }
    const expectedMetadata = outputMetadataByMediaId.get(media.id);
    if (expectedMetadata) {
      for (const field of ["durationSeconds", "width", "height", "fps"]) {
        if (media[field] !== expectedMetadata[field]) {
          failures.push(`${reportPath}: generated media row ${field} must match completed sidecar output: ${media.id}`);
        }
      }
    }
  }
  for (const outputMediaId of outputMediaIds) {
    if (
      typeof outputMediaId === "string" &&
      outputMediaId.trim() !== "" &&
      !generatedMediaIds.has(outputMediaId)
    ) {
      failures.push(`${reportPath}: completed sidecar output is missing generated media row: ${outputMediaId}`);
    }
  }
  validateSplitProjectJobRequest(report, reportPath, projectDir, completedAssetIds, failures);
  if (isProviderRetryScenario(scenario)) {
    validateProviderRetryEvidence(report, reportPath, projectDir, failures);
  }
}

function validateReferencedVideoSidecar(reportPath, sidecar, scenario, failures) {
  const references = sidecar.references && typeof sidecar.references === "object"
    ? sidecar.references
    : null;
  if (!references) {
    failures.push(`${reportPath}: referenced video sidecar references are required`);
    return;
  }
  if (scenario === "wan-reference-to-video") {
    if (
      !Array.isArray(references.referenceImageMediaRefs) ||
      references.referenceImageMediaRefs.length !== 1
    ) {
      failures.push(`${reportPath}: referenced video referenceImageMediaRefs are required`);
    }
    if (
      !Array.isArray(references.referenceVideoMediaRefs) ||
      references.referenceVideoMediaRefs.length !== 1
    ) {
      failures.push(`${reportPath}: referenced video referenceVideoMediaRefs are required`);
    }
  } else {
    if (typeof references.firstFrameMediaId !== "string" || references.firstFrameMediaId.trim() === "") {
      failures.push(`${reportPath}: referenced video firstFrameMediaId is required`);
    }
    if (typeof references.lastFrameMediaId !== "string" || references.lastFrameMediaId.trim() === "") {
      failures.push(`${reportPath}: referenced video lastFrameMediaId is required`);
    }
    if (
      scenario !== "kling-image-to-video" &&
      (!Array.isArray(references.referenceAudioMediaRefs) ||
        references.referenceAudioMediaRefs.length === 0)
    ) {
      failures.push(`${reportPath}: referenced video referenceAudioMediaRefs are required`);
    }
  }
  const expectedProviderInputUrlCount =
    scenario === "wan-reference-to-video" || scenario === "kling-image-to-video" ? 2 : 3;
  if (
    !Array.isArray(references.providerInputUrls) ||
    references.providerInputUrls.length !== expectedProviderInputUrlCount
  ) {
    failures.push(
      `${reportPath}: referenced video must retain ${expectedProviderInputUrlCount} provider input URLs`,
    );
    return;
  }
  for (const url of references.providerInputUrls) {
    if (!isProviderUrl(url)) {
      failures.push(`${reportPath}: referenced video providerInputUrls must be provider URLs`);
    }
  }
}

function validateWanReferenceVideoSettings(reportPath, sidecar, failures) {
  if (
    sidecar.settings?.durationSeconds !== 4 ||
    sidecar.settings?.aspectRatio !== "16:9" ||
    sidecar.settings?.resolution !== "720p" ||
    typeof sidecar.settings?.generateAudio !== "boolean"
  ) {
    failures.push(
      `${reportPath}: WAN reference-to-video sidecar must retain duration, aspect, resolution, and generateAudio settings`,
    );
  }
}

function validateSourceVideoSidecar(reportPath, sidecar, scenarioLabel, failures) {
  const references = sidecar.references && typeof sidecar.references === "object"
    ? sidecar.references
    : null;
  if (!references) {
    failures.push(`${reportPath}: ${scenarioLabel} sidecar references are required`);
    return;
  }
  if (
    typeof references.sourceVideoMediaRef !== "string" ||
    references.sourceVideoMediaRef.trim() === ""
  ) {
    failures.push(`${reportPath}: ${scenarioLabel} sourceVideoMediaRef is required`);
  }
  if (
    !Array.isArray(references.providerInputUrls) ||
    references.providerInputUrls.length !== 1
  ) {
    failures.push(`${reportPath}: ${scenarioLabel} must retain one provider input URL`);
    return;
  }
  if (!isProviderUrl(references.providerInputUrls[0])) {
    failures.push(`${reportPath}: ${scenarioLabel} providerInputUrls must be provider URLs`);
  }
}

function validateSourceVideoTrimSettings(
  reportPath,
  sidecar,
  scenarioLabel,
  expectedStartSeconds,
  expectedEndSeconds,
  failures,
) {
  if (
    sidecar.settings?.videoSourceStartSeconds !== expectedStartSeconds ||
    sidecar.settings?.videoSourceEndSeconds !== expectedEndSeconds
  ) {
    failures.push(`${reportPath}: ${scenarioLabel} sidecar must retain selected source trim settings`);
  }
}

function validateGoogleVeoSidecar(reportPath, sidecar, failures) {
  const references = sidecar.references && typeof sidecar.references === "object"
    ? sidecar.references
    : null;
  if (!references) {
    failures.push(`${reportPath}: Google Veo sidecar references are required`);
    return;
  }
  if (typeof references.firstFrameMediaId !== "string" || references.firstFrameMediaId.trim() === "") {
    failures.push(`${reportPath}: Google Veo firstFrameMediaId is required`);
  }
  if (typeof references.lastFrameMediaId !== "string" || references.lastFrameMediaId.trim() === "") {
    failures.push(`${reportPath}: Google Veo lastFrameMediaId is required`);
  }
  if (
    !Array.isArray(references.referenceImageMediaRefs) ||
    references.referenceImageMediaRefs.length !== 1
  ) {
    failures.push(`${reportPath}: Google Veo must retain one referenceImageMediaRefs value`);
  }
  if (
    !Array.isArray(references.providerInputUrls) ||
    references.providerInputUrls.length !== 3
  ) {
    failures.push(`${reportPath}: Google Veo must retain first, last, and reference image provider inputs`);
    return;
  }
  if (!references.providerInputUrls.every((url) => isImageDataUrl(url))) {
    failures.push(`${reportPath}: Google Veo providerInputUrls must be image data URLs`);
  }
}

function validateXaiGrokVideoSidecar(reportPath, sidecar, failures) {
  const references = sidecar.references && typeof sidecar.references === "object"
    ? sidecar.references
    : null;
  if (!references) {
    failures.push(`${reportPath}: xAI Grok video sidecar references are required`);
    return;
  }
  if (
    !Array.isArray(references.referenceImageMediaRefs) ||
    references.referenceImageMediaRefs.length !== 1
  ) {
    failures.push(`${reportPath}: xAI Grok video must retain one referenceImageMediaRefs value`);
  }
  if (
    !Array.isArray(references.providerInputUrls) ||
    references.providerInputUrls.length !== 1
  ) {
    failures.push(`${reportPath}: xAI Grok video must retain one provider input URL`);
    return;
  }
  if (!isProviderUrl(references.providerInputUrls[0])) {
    failures.push(`${reportPath}: xAI Grok video providerInputUrls must be provider URLs`);
  }
}

function validateMotionControlSidecar(reportPath, sidecar, failures) {
  const references = sidecar.references && typeof sidecar.references === "object"
    ? sidecar.references
    : null;
  if (!references) {
    failures.push(`${reportPath}: Motion Control sidecar references are required`);
    return;
  }
  if (
    typeof references.sourceVideoMediaRef !== "string" ||
    references.sourceVideoMediaRef.trim() === ""
  ) {
    failures.push(`${reportPath}: Motion Control sourceVideoMediaRef is required`);
  }
  if (
    !Array.isArray(references.referenceImageMediaRefs) ||
    references.referenceImageMediaRefs.length !== 1
  ) {
    failures.push(`${reportPath}: Motion Control must retain one referenceImageMediaRefs value`);
  }
  if (
    !Array.isArray(references.providerInputUrls) ||
    references.providerInputUrls.length !== 2
  ) {
    failures.push(`${reportPath}: Motion Control must retain source-video and image provider input URLs`);
    return;
  }
  for (const url of references.providerInputUrls) {
    if (!isProviderUrl(url)) {
      failures.push(`${reportPath}: Motion Control providerInputUrls must be provider URLs`);
    }
  }
}

function validateImageEditSidecar(reportPath, provider, sidecar, failures) {
  const references = sidecar.references && typeof sidecar.references === "object"
    ? sidecar.references
    : null;
  if (!references) {
    failures.push(`${reportPath}: image-edit sidecar references are required`);
    return;
  }
  const expectedReferenceCount = provider === "openai" ? 2 : 1;
  const providerLabel = provider === "openai" ? "OpenAI" : provider === "xai" ? "xAI" : null;
  if (
    !Array.isArray(references.referenceImageMediaRefs) ||
    references.referenceImageMediaRefs.length !== expectedReferenceCount
  ) {
    failures.push(
      providerLabel
        ? `${reportPath}: ${providerLabel} image-edit must retain ${expectedReferenceCount} referenceImageMediaRefs values`
        : `${reportPath}: image-edit must retain one referenceImageMediaRefs value`,
    );
  }
  if (
    !Array.isArray(references.providerInputUrls) ||
    references.providerInputUrls.length !== expectedReferenceCount
  ) {
    failures.push(
      providerLabel
        ? `${reportPath}: ${providerLabel} image-edit must retain ${expectedReferenceCount} provider input URLs`
        : `${reportPath}: image-edit must retain one provider input URL`,
    );
    return;
  }
  const validProviderInput =
    provider === "openai" || provider === "xai"
      ? references.providerInputUrls.every((url) => isImageDataUrl(url))
      : references.providerInputUrls.every((url) => isProviderUrl(url));
  if (!validProviderInput) {
    failures.push(
      providerLabel
        ? `${reportPath}: ${providerLabel} image-edit providerInputUrls must be image data URLs`
        : `${reportPath}: image-edit providerInputUrls must be provider URLs`,
    );
  }
}

function validateImageUpscaleSidecar(reportPath, sidecar, failures) {
  const refs = sidecar.references;
  if (!refs || typeof refs !== "object") {
    failures.push(`${reportPath}: image-upscale sidecar references are required`);
    return;
  }
  if (!Array.isArray(refs.mediaIds) || refs.mediaIds.length !== 1) {
    failures.push(`${reportPath}: image-upscale must retain one mediaIds source`);
  }
  if (
    !Array.isArray(refs.providerInputUrls) ||
    refs.providerInputUrls.length !== 1
  ) {
    failures.push(`${reportPath}: image-upscale must retain one provider input URL`);
    return;
  }
  if (!isProviderUrl(refs.providerInputUrls[0])) {
    failures.push(`${reportPath}: image-upscale providerInputUrls must be provider URLs`);
  }
}

function validateProviderLifecycleResult(report, reportPath, failures, lifecycleScenario) {
  const scenario = normalizedScenario(report);
  const result = report.result;
  if (result.ok !== false) {
    failures.push(`${reportPath}: lifecycle result.ok must be false`);
  }
  if (result.provider !== report.provider) {
    failures.push(`${reportPath}: lifecycle result.provider must match report.provider`);
  }
  if (result.model !== report.model) {
    failures.push(`${reportPath}: lifecycle result.model must match report.model`);
  }
  if (result.outputCount !== 0) {
    failures.push(`${reportPath}: lifecycle result.outputCount must be 0`);
  }
  if (result.artifactPath !== null) {
    failures.push(`${reportPath}: lifecycle result.artifactPath must be null`);
  }
  if (result.jobStatus !== lifecycleScenario.jobStatus) {
    failures.push(`${reportPath}: lifecycle result.jobStatus must be ${lifecycleScenario.jobStatus}`);
  }
  if (result.terminalReason !== lifecycleScenario.reason) {
    failures.push(`${reportPath}: lifecycle result.terminalReason must be ${lifecycleScenario.reason}`);
  }
  if (typeof result.projectDir !== "string" || result.projectDir.trim() === "") {
    failures.push(`${reportPath}: lifecycle result.projectDir is required`);
    return;
  }
  validateProviderRequest(report, reportPath, failures);
  const projectDir = result.projectDir;
  const manifestPath = join(projectDir, "video-creater.project.json");
  if (!existsSync(manifestPath)) {
    failures.push(`${reportPath}: lifecycle split project manifest is missing`);
    return;
  }
  let manifest;
  try {
    manifest = JSON.parse(readFileSync(manifestPath, "utf8"));
  } catch {
    failures.push(`${reportPath}: lifecycle split project manifest is not parseable`);
    return;
  }
  const generatedDir = manifest.files?.generated;
  if (typeof generatedDir !== "string" || generatedDir.trim() === "") {
    failures.push(`${reportPath}: lifecycle manifest.files.generated is required`);
    return;
  }
  const generatedRoot = join(projectDir, generatedDir);
  const generatedIndexPath = join(generatedRoot, "index.json");
  if (!isWithinDirectory(projectDir, generatedIndexPath) || !existsSync(generatedIndexPath)) {
    failures.push(`${reportPath}: lifecycle generated index is missing or escapes project`);
    return;
  }
  let generatedIndex;
  try {
    generatedIndex = JSON.parse(readFileSync(generatedIndexPath, "utf8"));
  } catch {
    failures.push(`${reportPath}: lifecycle generated index is not parseable`);
    return;
  }
  const terminalAssets = Array.isArray(generatedIndex.assets)
    ? generatedIndex.assets.filter((asset) => asset.status === lifecycleScenario.assetStatus)
    : [];
  if (terminalAssets.length !== 1) {
    failures.push(`${reportPath}: lifecycle report must retain exactly one terminal generated asset`);
    return;
  }
  const asset = terminalAssets[0];
  if (asset.modelProvider !== report.provider || asset.modelId !== report.model) {
    failures.push(`${reportPath}: lifecycle generated index model metadata must match retained report`);
  }
  if (asset.outputCount !== 0) {
    failures.push(`${reportPath}: lifecycle terminal generated asset outputCount must be 0`);
  }
  const sidecarPath = join(projectDir, asset.path || "");
  if (!isWithinDirectory(generatedRoot, sidecarPath) || !existsSync(sidecarPath)) {
    failures.push(`${reportPath}: lifecycle generated sidecar is missing or outside generated directory`);
    return;
  }
  let sidecar;
  try {
    sidecar = JSON.parse(readFileSync(sidecarPath, "utf8"));
  } catch {
    failures.push(`${reportPath}: lifecycle generated sidecar is not parseable`);
    return;
  }
  if (sidecar.id !== asset.assetId || sidecar.status !== lifecycleScenario.assetStatus) {
    failures.push(`${reportPath}: lifecycle generated sidecar must match index terminal state`);
  }
  if (sidecar.model?.provider !== report.provider || sidecar.model?.id !== report.model) {
    failures.push(`${reportPath}: lifecycle generated sidecar model must match retained report`);
  }
  if (!Array.isArray(sidecar.outputs) || sidecar.outputs.length !== 0) {
    failures.push(`${reportPath}: lifecycle generated sidecar must be outputless`);
  }
  if (sidecar.failureReason !== lifecycleScenario.reason) {
    failures.push(`${reportPath}: lifecycle generated sidecar must retain failureReason`);
  }
  if (
    scenario === "local-video-upscale-cancellation" ||
    scenario === "local-video-edit-cancellation" ||
    scenario === "local-video-upscale-failure" ||
    scenario === "local-video-edit-failure"
  ) {
    validateSourceVideoSidecar(reportPath, sidecar, "video-to-video", failures);
    if (
      isLocalVideoUpscaleScenario(scenario)
    ) {
      validateSourceVideoTrimSettings(reportPath, sidecar, "video-to-video", 0.5, 4.5, failures);
    }
    if (
      (scenario === "local-video-edit-cancellation" ||
        scenario === "local-video-edit-failure") &&
      (sidecar.settings?.videoSourceStartSeconds !== 0.5 ||
        sidecar.settings?.videoSourceEndSeconds !== 3.5)
    ) {
      failures.push(`${reportPath}: lifecycle generated sidecar must retain selected source trim settings`);
    }
  }
  if (
    scenario === "local-image-edit-cancellation" ||
    scenario === "local-image-edit-failure"
  ) {
    validateImageEditSidecar(reportPath, report.provider, sidecar, failures);
  }
  if (
    scenario === "local-image-upscale-cancellation" ||
    scenario === "local-image-upscale-failure"
  ) {
    validateImageUpscaleSidecar(reportPath, sidecar, failures);
  }
  const jobPath = join(projectDir, "jobs", asset.assetId, "job.json");
  if (!isWithinDirectory(projectDir, jobPath) || !existsSync(jobPath)) {
    failures.push(`${reportPath}: lifecycle job sidecar is missing`);
    return;
  }
  let job;
  try {
    job = JSON.parse(readFileSync(jobPath, "utf8"));
  } catch {
    failures.push(`${reportPath}: lifecycle job sidecar is not parseable`);
    return;
  }
  if (job.status !== lifecycleScenario.jobSidecarStatus) {
    failures.push(`${reportPath}: lifecycle job sidecar status must be ${lifecycleScenario.jobSidecarStatus}`);
  }
  if (job.terminalReason !== lifecycleScenario.reason) {
    failures.push(`${reportPath}: lifecycle job sidecar must retain terminalReason`);
  }
  if (!providerRequestsEqual(job.providerRequest ?? {}, result.providerRequest ?? {})) {
    failures.push(`${reportPath}: lifecycle job provider request must match retained report`);
  }
}

function validateProviderRetryEvidence(report, reportPath, projectDir, failures) {
  const generatedIndexPath = join(projectDir, "generated", "index.json");
  let generatedIndex;
  try {
    generatedIndex = JSON.parse(readFileSync(generatedIndexPath, "utf8"));
  } catch {
    failures.push(`${reportPath}: retry generated index is not parseable`);
    return;
  }
  const completedAssets = Array.isArray(generatedIndex.assets)
    ? generatedIndex.assets.filter((asset) => asset.status === "completed" && asset.outputCount > 0)
    : [];
  if (completedAssets.length !== 1) {
    failures.push(`${reportPath}: retry scenario must retain one completed retry asset`);
    return;
  }
  const completedAsset = completedAssets[0];
  let completedSidecar;
  try {
    completedSidecar = JSON.parse(readFileSync(join(projectDir, completedAsset.path || ""), "utf8"));
  } catch {
    failures.push(`${reportPath}: retry completed sidecar is not parseable`);
    return;
  }
  const retryOfAssetId = completedSidecar.retryOfAssetId;
  if (typeof retryOfAssetId !== "string" || retryOfAssetId.trim() === "") {
    failures.push(`${reportPath}: retry completed sidecar must retain retryOfAssetId`);
    return;
  }
  const failedAsset = generatedIndex.assets.find((asset) => asset.assetId === retryOfAssetId);
  if (!failedAsset || failedAsset.status !== "failed" || failedAsset.outputCount !== 0) {
    failures.push(`${reportPath}: retryOfAssetId must point at a failed outputless asset`);
    return;
  }
  if (failedAsset.modelProvider !== report.provider || failedAsset.modelId !== report.model) {
    failures.push(`${reportPath}: retry failed asset model metadata must match retained report`);
  }
  let failedSidecar;
  try {
    failedSidecar = JSON.parse(readFileSync(join(projectDir, failedAsset.path || ""), "utf8"));
  } catch {
    failures.push(`${reportPath}: retry failed sidecar is not parseable`);
    return;
  }
  if (
    failedSidecar.id !== retryOfAssetId ||
    failedSidecar.status !== "failed" ||
    !Array.isArray(failedSidecar.outputs) ||
    failedSidecar.outputs.length !== 0
  ) {
    failures.push(`${reportPath}: retry failed sidecar must be failed and outputless`);
  }
  const failedJobPath = join(projectDir, "jobs", retryOfAssetId, "job.json");
  let failedJob;
  try {
    failedJob = JSON.parse(readFileSync(failedJobPath, "utf8"));
  } catch {
    failures.push(`${reportPath}: retry failed job sidecar is not parseable`);
    return;
  }
  if (failedJob.status !== "failed" || failedJob.terminalReason !== "provider_error") {
    failures.push(`${reportPath}: retry failed job must retain provider_error terminal evidence`);
  }
  if (failedJob.providerRequest?.provider !== report.provider) {
    failures.push(`${reportPath}: retry failed job provider request must match provider`);
  }
  if (failedJob.providerRequest?.requestId === report.result?.providerRequest?.requestId) {
    failures.push(`${reportPath}: retry failed and completed jobs must use distinct provider requests`);
  }
}

function validateReplacementTimelineEvidence(
  reportPath,
  projectDir,
  mediaId,
  failures,
  itemId = "item-upscale-target",
  linkedItemId = null,
) {
  if (typeof mediaId !== "string" || mediaId.trim() === "") {
    failures.push(`${reportPath}: replacement scenario output mediaId is required`);
    return;
  }
  const timelinePath = join(projectDir, "timeline.json");
  if (!existsSync(timelinePath)) {
    failures.push(`${reportPath}: replacement scenario timeline.json is missing`);
    return;
  }
  let timeline;
  try {
    timeline = JSON.parse(readFileSync(timelinePath, "utf8"));
  } catch {
    failures.push(`${reportPath}: replacement scenario timeline.json is not parseable`);
    return;
  }
  const tracks = Array.isArray(timeline.tracks) ? timeline.tracks : [];
  const item = tracks
    .flatMap((track) => Array.isArray(track.items) ? track.items : [])
    .find((candidate) => candidate?.id === itemId);
  if (!item) {
    failures.push(`${reportPath}: replacement scenario target timeline item is missing`);
    return;
  }
  if (item.source?.type !== "media" || item.source?.mediaId !== mediaId) {
    failures.push(`${reportPath}: replacement scenario target item must point at generated output media`);
  }
  if (item.properties?.generatedOutputMediaId !== mediaId) {
    failures.push(`${reportPath}: replacement scenario target item must retain generatedOutputMediaId`);
  }
  if (item.properties?.sourceOut !== item.durationSeconds) {
    failures.push(`${reportPath}: replacement scenario sourceOut must match replacement duration`);
  }
  if (linkedItemId) {
    const linkedItem = tracks
      .flatMap((track) => Array.isArray(track.items) ? track.items : [])
      .find((candidate) => candidate?.id === linkedItemId);
    if (!linkedItem) {
      failures.push(`${reportPath}: replacement scenario linked timeline item is missing`);
      return;
    }
    const replacementLinkGroupId = item.properties?.linkGroupId;
    if (typeof replacementLinkGroupId !== "string" || replacementLinkGroupId.trim() === "") {
      failures.push(`${reportPath}: replacement scenario linked item must retain the replacement linkGroupId`);
      return;
    }
    if (linkedItem.source?.type !== "media" || linkedItem.source?.mediaId !== mediaId) {
      failures.push(`${reportPath}: replacement scenario linked item must point at generated output media`);
    }
    if (linkedItem.properties?.generatedOutputMediaId !== mediaId) {
      failures.push(`${reportPath}: replacement scenario linked item must retain generatedOutputMediaId`);
    }
    if (linkedItem.properties?.sourceOut !== linkedItem.durationSeconds) {
      failures.push(`${reportPath}: replacement scenario linked sourceOut must match replacement duration`);
    }
    if (replacementLinkGroupId !== linkedItem.properties?.linkGroupId) {
      failures.push(`${reportPath}: replacement scenario linked item must retain the replacement linkGroupId`);
    }
  }
}

function validateTimelineAudioInsertEvidence(
  reportPath,
  projectDir,
  mediaId,
  outputDurationSeconds,
  scenario,
  failures,
) {
  const insertConfig = timelineAudioInsertConfig(scenario);
  if (typeof mediaId !== "string" || mediaId.trim() === "") {
    failures.push(`${reportPath}: audio insert scenario output mediaId is required`);
    return;
  }
  const timelinePath = join(projectDir, "timeline.json");
  if (!existsSync(timelinePath)) {
    failures.push(`${reportPath}: audio insert scenario timeline.json is missing`);
    return;
  }
  let timeline;
  try {
    timeline = JSON.parse(readFileSync(timelinePath, "utf8"));
  } catch {
    failures.push(`${reportPath}: audio insert scenario timeline.json is not parseable`);
    return;
  }
  const tracks = Array.isArray(timeline.tracks) ? timeline.tracks : [];
  const sourceItem = tracks
    .flatMap((track) => Array.isArray(track.items) ? track.items : [])
    .find((candidate) => candidate?.id === insertConfig.sourceItemId);
  if (!sourceItem) {
    failures.push(`${reportPath}: audio insert scenario source video item is missing`);
    return;
  }
  if (sourceItem.kind !== "video_clip") {
    failures.push(`${reportPath}: audio insert scenario source item must be a video clip`);
  }
  if (
    sourceItem.source?.type !== "media" ||
    sourceItem.source?.mediaId !== "provider-e2e-video-source"
  ) {
    failures.push(`${reportPath}: audio insert scenario source item must point at selected video media`);
  }
  if (sourceItem.startSeconds !== insertConfig.timelineStartSeconds) {
    failures.push(`${reportPath}: audio insert scenario source item start must match selected clip start`);
  }
  if (
    sourceItem.durationSeconds !==
    insertConfig.videoSourceEndSeconds - insertConfig.videoSourceStartSeconds
  ) {
    failures.push(`${reportPath}: audio insert scenario source item duration must match selected source trim`);
  }
  if (
    sourceItem.properties?.sourceIn !== insertConfig.videoSourceStartSeconds ||
    sourceItem.properties?.sourceOut !== insertConfig.videoSourceEndSeconds
  ) {
    failures.push(`${reportPath}: audio insert scenario source item must retain selected source trim`);
  }
  const audioTrack = tracks.find((track) => track?.id === "track-audio" || track?.kind === "audio");
  const item = Array.isArray(audioTrack?.items)
    ? audioTrack.items.find((candidate) => candidate?.id === insertConfig.insertedItemId)
    : null;
  if (!item) {
    failures.push(`${reportPath}: audio insert scenario generated audio item is missing`);
    return;
  }
  if (item.kind !== "audio_clip") {
    failures.push(`${reportPath}: audio insert scenario generated item must be an audio clip`);
  }
  if (item.source?.type !== "media" || item.source?.mediaId !== mediaId) {
    failures.push(`${reportPath}: audio insert scenario item must point at generated output media`);
  }
  if (item.startSeconds !== insertConfig.timelineStartSeconds) {
    failures.push(`${reportPath}: audio insert scenario item start must match selected clip start`);
  }
  if (item.durationSeconds !== outputDurationSeconds) {
    failures.push(`${reportPath}: audio insert scenario item duration must match generated output duration`);
  }
  if (item.properties?.sourceOut !== outputDurationSeconds) {
    failures.push(`${reportPath}: audio insert scenario sourceOut must match generated output duration`);
  }
  if (item.properties?.generatedOutputMediaId !== mediaId) {
    failures.push(`${reportPath}: audio insert scenario item must retain generatedOutputMediaId`);
  }
  if (item.properties?.sourceVideoMediaRef !== "provider-e2e-video-source") {
    failures.push(`${reportPath}: audio insert scenario item must retain sourceVideoMediaRef`);
  }
}

function validateTimelineTextToMusicInsertEvidence(
  reportPath,
  projectDir,
  mediaId,
  outputDurationSeconds,
  failures,
) {
  const insertConfig = timelineTextToMusicInsertConfig();
  if (typeof mediaId !== "string" || mediaId.trim() === "") {
    failures.push(`${reportPath}: text-to-music insert scenario output mediaId is required`);
    return;
  }
  const timelinePath = join(projectDir, "timeline.json");
  if (!existsSync(timelinePath)) {
    failures.push(`${reportPath}: text-to-music insert scenario timeline.json is missing`);
    return;
  }
  let timeline;
  try {
    timeline = JSON.parse(readFileSync(timelinePath, "utf8"));
  } catch {
    failures.push(`${reportPath}: text-to-music insert scenario timeline.json is not parseable`);
    return;
  }
  const tracks = Array.isArray(timeline.tracks) ? timeline.tracks : [];
  const audioTrack = tracks.find((track) => track?.id === insertConfig.trackId || track?.kind === "audio");
  const item = Array.isArray(audioTrack?.items)
    ? audioTrack.items.find((candidate) => candidate?.id === insertConfig.insertedItemId)
    : null;
  if (!item) {
    failures.push(`${reportPath}: text-to-music insert scenario generated audio item is missing`);
    return;
  }
  if (item.kind !== "audio_clip") {
    failures.push(`${reportPath}: text-to-music insert scenario generated item must be an audio clip`);
  }
  if (item.source?.type !== "media" || item.source?.mediaId !== mediaId) {
    failures.push(`${reportPath}: text-to-music insert scenario item must point at generated output media`);
  }
  if (item.startSeconds !== insertConfig.timelineStartSeconds) {
    failures.push(`${reportPath}: text-to-music insert scenario item start must match placement start`);
  }
  if (item.durationSeconds !== outputDurationSeconds) {
    failures.push(`${reportPath}: text-to-music insert scenario item duration must match generated output duration`);
  }
  if (item.properties?.sourceOut !== outputDurationSeconds) {
    failures.push(`${reportPath}: text-to-music insert scenario sourceOut must match generated output duration`);
  }
  if (item.properties?.generatedOutputMediaId !== mediaId) {
    failures.push(`${reportPath}: text-to-music insert scenario item must retain generatedOutputMediaId`);
  }
}

function validateTimelineVisualInsertEvidence(
  reportPath,
  projectDir,
  mediaId,
  outputDurationSeconds,
  failures,
) {
  const insertConfig = timelineVisualInsertConfig();
  if (typeof mediaId !== "string" || mediaId.trim() === "") {
    failures.push(`${reportPath}: visual insert scenario output mediaId is required`);
    return;
  }
  const timelinePath = join(projectDir, "timeline.json");
  if (!existsSync(timelinePath)) {
    failures.push(`${reportPath}: visual insert scenario timeline.json is missing`);
    return;
  }
  let timeline;
  try {
    timeline = JSON.parse(readFileSync(timelinePath, "utf8"));
  } catch {
    failures.push(`${reportPath}: visual insert scenario timeline.json is not parseable`);
    return;
  }
  const tracks = Array.isArray(timeline.tracks) ? timeline.tracks : [];
  const videoTrack = tracks.find((track) =>
    track?.id === insertConfig.trackId || track?.kind === "video",
  );
  const item = Array.isArray(videoTrack?.items)
    ? videoTrack.items.find((candidate) => candidate?.id === insertConfig.insertedItemId)
    : null;
  if (!item) {
    failures.push(`${reportPath}: visual insert scenario generated video item is missing`);
    return;
  }
  if (item.kind !== "video_clip" && item.kind !== "generated_clip" && item.kind !== "image_clip") {
    failures.push(`${reportPath}: visual insert scenario generated item must be a visual clip`);
  }
  if (item.source?.type !== "media" || item.source?.mediaId !== mediaId) {
    failures.push(`${reportPath}: visual insert scenario item must point at generated output media`);
  }
  if (item.startSeconds !== insertConfig.timelineStartSeconds) {
    failures.push(`${reportPath}: visual insert scenario item start must match requested timeline start`);
  }
  if (item.durationSeconds !== outputDurationSeconds) {
    failures.push(`${reportPath}: visual insert scenario item duration must match generated output duration`);
  }
  if (item.properties?.sourceIn !== 0) {
    failures.push(`${reportPath}: visual insert scenario sourceIn must start at generated output zero`);
  }
  if (item.properties?.sourceOut !== outputDurationSeconds) {
    failures.push(`${reportPath}: visual insert scenario sourceOut must match generated output duration`);
  }
  if (item.properties?.generatedOutputMediaId !== mediaId) {
    failures.push(`${reportPath}: visual insert scenario item must retain generatedOutputMediaId`);
  }
}

function validateSplitProjectJobRequest(report, reportPath, projectDir, completedAssetIds, failures) {
  const request = report.result?.providerRequest;
  if (!request || typeof request !== "object" || completedAssetIds.size === 0) {
    return;
  }
  let matchingJobRequestCount = 0;
  for (const assetId of completedAssetIds) {
    const jobPath = join(projectDir, "jobs", assetId, "job.json");
    if (!isWithinDirectory(projectDir, jobPath) || !existsSync(jobPath)) {
      continue;
    }
    let job;
    try {
      job = JSON.parse(readFileSync(jobPath, "utf8"));
    } catch {
      failures.push(`${reportPath}: split project job sidecar is not parseable: ${assetId}`);
      continue;
    }
    if (!providerRequestsEqual(job.providerRequest ?? {}, request)) {
      continue;
    }
    if (job.status !== "completed") {
      failures.push(`${reportPath}: split project job sidecar must be completed: ${assetId}`);
      continue;
    }
    matchingJobRequestCount += 1;
  }
  if (matchingJobRequestCount !== 1) {
    failures.push(`${reportPath}: split project job provider request does not match retained report`);
  }
}

if (process.argv[1] && resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  try {
    const options = parseArgs(process.argv.slice(2));
    const result = validateManifest(options.manifest, options);
    console.log(JSON.stringify(result, null, 2));
    if (result.failures.length > 0) {
      process.exitCode = 1;
    }
  } catch (error) {
    console.error(error instanceof Error ? error.message : String(error));
    process.exitCode = 2;
  }
}
