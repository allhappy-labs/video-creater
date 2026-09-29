#!/usr/bin/env node
import { spawnSync } from "node:child_process";
import {
  cpSync,
  existsSync,
  mkdirSync,
  readFileSync,
  rmSync,
  statSync,
  writeFileSync,
} from "node:fs";
import { dirname, isAbsolute, join, relative, resolve, sep } from "node:path";
import { fileURLToPath } from "node:url";

export const providerEnvVars = {
  "fal.ai": "FAL_KEY",
  openai: "OPENAI_API_KEY",
  replicate: "REPLICATE_API_TOKEN",
  xai: "XAI_API_KEY",
  elevenlabs: "ELEVENLABS_API_KEY",
  google: "GEMINI_API_KEY",
  minimax: "MINIMAX_API_KEY",
};

const liveSpendOptInEnvVar = "VIDEO_CREATER_PROVIDER_E2E_ALLOW_SPEND";

export const defaultModels = {
  "fal.ai": "fal-ai/flux/schnell",
  openai: "gpt-image-2",
  replicate: "black-forest-labs/flux-schnell",
  xai: "grok-imagine-image-quality",
  elevenlabs: "elevenlabs-tts-v3",
  google: "veo3.1-fast",
  minimax: "minimax-music-v2.6",
};

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
const videoToMusicDefaultPrompt = textToMusicDefaultSettings.styleInstructions;
const minimaxTextToMusicDefaultLyrics = "[Verse]\nBright product launch\n[Chorus]\nLoopable brand hook";
const elevenlabsTextToMusicDefaultLyrics =
  "[Verse]\nWarm synth pop in English\n[Chorus]\nBright chorus lift";
const googleLyriaTextToMusicDefaultLyrics =
  "[Verse]\nSoft synth pulse in English\n[Chorus]\nConfident vocal lift";

const replicateFluxVariantModels = {
  "replicate-flux-dev": "black-forest-labs/flux-dev",
  "replicate-flux-1.1-pro": "black-forest-labs/flux-1.1-pro",
  "replicate-flux-1.1-pro-ultra": "black-forest-labs/flux-1.1-pro-ultra",
};

export const scenarioDefaultModels = {
  "local-image-upscale": {
    provider: "fal.ai",
    model: "fal-ai/aura-sr",
  },
  "local-image-upscale-replace": {
    provider: "fal.ai",
    model: "fal-ai/aura-sr",
  },
  "local-image-upscale-retry": {
    provider: "fal.ai",
    model: "fal-ai/aura-sr",
  },
  "local-image-upscale-cancellation": {
    provider: "fal.ai",
    model: "fal-ai/aura-sr",
  },
  "local-image-upscale-failure": {
    provider: "fal.ai",
    model: "fal-ai/aura-sr",
  },
  "local-image-edit-replace": {
    models: {
      "fal.ai": "fal-ai/nano-banana-pro/edit",
      openai: "gpt-image-1.5",
      xai: "grok-imagine-image-quality",
    },
  },
  "text-to-image-replace": {
    models: {
      "fal.ai": "fal-ai/flux/schnell",
      openai: "gpt-image-2",
      replicate: "black-forest-labs/flux-schnell",
      xai: "grok-imagine-image-quality",
    },
  },
  "local-image-edit-retry": {
    provider: "fal.ai",
    model: "fal-ai/nano-banana-pro/edit",
  },
  "local-image-edit-cancellation": {
    provider: "fal.ai",
    model: "fal-ai/nano-banana-pro/edit",
  },
  "local-image-edit-failure": {
    provider: "fal.ai",
    model: "fal-ai/nano-banana-pro/edit",
  },
  "multi-image": {
    models: {
      "fal.ai": "fal-ai/flux/schnell",
      openai: "gpt-image-2",
      replicate: "black-forest-labs/flux-schnell",
    },
  },
  "local-video-upscale": {
    provider: "fal.ai",
    model: "fal-ai/video-upscaler",
  },
  "local-video-upscale-replace": {
    provider: "fal.ai",
    model: "fal-ai/video-upscaler",
  },
  "local-video-upscale-retry": {
    provider: "fal.ai",
    model: "fal-ai/video-upscaler",
  },
  "local-video-upscale-cancellation": {
    provider: "fal.ai",
    model: "fal-ai/video-upscaler",
  },
  "local-video-upscale-failure": {
    provider: "fal.ai",
    model: "fal-ai/video-upscaler",
  },
  "local-video-edit-replace": {
    models: {
      "fal.ai": "fal-ai/wan/v2.2-a14b/video-to-video",
      xai: "grok-imagine-video",
    },
  },
  "local-video-motion-control-replace": {
    provider: "fal.ai",
    model: "fal-ai/kling-video/v3/pro/motion-control",
  },
  "local-video-edit-retry": {
    provider: "fal.ai",
    model: "fal-ai/wan/v2.2-a14b/video-to-video",
  },
  "local-video-edit-cancellation": {
    provider: "fal.ai",
    model: "fal-ai/wan/v2.2-a14b/video-to-video",
  },
  "local-video-edit-failure": {
    provider: "fal.ai",
    model: "fal-ai/wan/v2.2-a14b/video-to-video",
  },
  "image-edit": {
    models: {
      "fal.ai": "fal-ai/nano-banana-pro/edit",
      openai: "gpt-image-1.5",
      xai: "grok-imagine-image-quality",
    },
  },
  "krea-text-to-image": {
    provider: "fal.ai",
    model: "fal-ai/krea-2/turbo",
  },
  "recraft-text-to-image": {
    provider: "fal.ai",
    model: "fal-ai/recraft/v3/text-to-image",
  },
  "wan-image-to-video": {
    provider: "fal.ai",
    model: "fal-ai/wan/v2.7/image-to-video",
  },
  "wan-reference-to-video": {
    provider: "fal.ai",
    model: "fal-ai/wan/v2.7/reference-to-video",
  },
  "kling-image-to-video": {
    provider: "fal.ai",
    model: "fal-ai/kling-video/v3/pro/image-to-video",
  },
  "video-to-video": {
    provider: "fal.ai",
    model: "fal-ai/wan/v2.2-a14b/video-to-video",
  },
  "text-to-audio": {
    models: {
      "fal.ai": "bytedance/seed-audio-1.0",
      openai: "gpt-4o-mini-tts",
      elevenlabs: "elevenlabs-tts-v3",
      google: "gemini-3.1-flash-tts-preview",
    },
  },
  "text-to-music": {
    models: {
      "fal.ai": "sonilo/v1.1/text-to-music",
      elevenlabs: "elevenlabs-music",
      google: "lyria3-pro",
      minimax: "minimax-music-v2.6",
    },
  },
  "text-to-music-insert": {
    models: {
      "fal.ai": "sonilo/v1.1/text-to-music",
      elevenlabs: "elevenlabs-music",
      google: "lyria3-pro",
      minimax: "minimax-music-v2.6",
    },
  },
  "video-to-music": {
    provider: "fal.ai",
    model: "sonilo/v1.1/video-to-music",
  },
  "local-video-to-music-insert": {
    provider: "fal.ai",
    model: "sonilo/v1.1/video-to-music",
  },
  "local-video-to-sfx-insert": {
    provider: "fal.ai",
    model: "mirelo-ai/sfx-v1.5/video-to-audio",
  },
  "video-to-sfx": {
    provider: "fal.ai",
    model: "mirelo-ai/sfx-v1.5/video-to-audio",
  },
  "replicate-local-file-upload": {
    provider: "replicate",
    model: "black-forest-labs/flux-schnell",
  },
  "replicate-flux-dev": {
    provider: "replicate",
    model: replicateFluxVariantModels["replicate-flux-dev"],
  },
  "replicate-flux-1.1-pro": {
    provider: "replicate",
    model: replicateFluxVariantModels["replicate-flux-1.1-pro"],
  },
  "replicate-flux-1.1-pro-ultra": {
    provider: "replicate",
    model: replicateFluxVariantModels["replicate-flux-1.1-pro-ultra"],
  },
  "replicate-video": {
    provider: "replicate",
    model: "bytedance/seedance-2.0",
  },
  "replicate-video-fast": {
    provider: "replicate",
    model: "bytedance/seedance-2.0-fast",
  },
  "xai-video": {
    provider: "xai",
    model: "grok-imagine-video",
  },
  "google-video": {
    provider: "google",
    model: "veo3.1-fast",
  },
  "text-to-video-replace": {
    models: {
      "fal.ai": "fal-ai/wan-25-preview/text-to-video",
      google: "veo3.1-fast",
      xai: "grok-imagine-video",
      replicate: "bytedance/seedance-2.0",
    },
  },
  "text-to-video-insert": {
    models: {
      "fal.ai": "fal-ai/wan-25-preview/text-to-video",
      google: "veo3.1-fast",
      xai: "grok-imagine-video",
      replicate: "bytedance/seedance-2.0",
    },
  },
  "provider-failure": null,
  "provider-cancellation": null,
  "provider-retry": null,
  "text-to-image": null,
};

const providerAliases = {
  fal: "fal.ai",
};

const scenarioDefaultOutDirSuffixes = {
  "image-edit": "image-edit",
  "krea-text-to-image": "krea-image",
  "recraft-text-to-image": "recraft-image",
  "kling-image-to-video": "kling-image-to-video",
  "local-image-upscale": "upscale",
  "local-image-upscale-replace": "upscale-replace",
  "local-image-upscale-retry": "upscale-retry",
  "local-image-upscale-cancellation": "upscale-cancellation",
  "local-image-upscale-failure": "upscale-failure",
  "local-image-edit-replace": "image-edit-replace",
  "text-to-image-replace": "text-to-image-replace",
  "local-image-edit-retry": "image-edit-retry",
  "local-image-edit-cancellation": "image-edit-cancellation",
  "local-image-edit-failure": "image-edit-failure",
  "multi-image": "multi-image",
  "local-video-upscale": "video-upscale",
  "local-video-upscale-replace": "video-upscale-replace",
  "local-video-upscale-retry": "video-upscale-retry",
  "local-video-upscale-cancellation": "video-upscale-cancellation",
  "local-video-upscale-failure": "video-upscale-failure",
  "local-video-edit-replace": "video-edit-replace",
  "local-video-motion-control-replace": "video-motion-control-replace",
  "local-video-edit-retry": "video-edit-retry",
  "local-video-edit-cancellation": "video-edit-cancellation",
  "local-video-edit-failure": "video-edit-failure",
  "provider-cancellation": "cancellation",
  "provider-failure": "failure",
  "provider-retry": "retry",
  "replicate-local-file-upload": "upload",
  "replicate-flux-dev": "flux-dev",
  "replicate-flux-1.1-pro": "flux-1.1-pro",
  "replicate-flux-1.1-pro-ultra": "flux-1.1-pro-ultra",
  "replicate-video": "video",
  "replicate-video-fast": "video-fast",
  "xai-video": "video",
  "google-video": "video",
  "text-to-video-replace": "text-to-video-replace",
  "text-to-video-insert": "text-to-video-insert",
  "text-to-audio": "audio",
  "text-to-music": "music",
  "text-to-music-insert": "music-insert",
  "video-to-music": "video-to-music",
  "local-video-to-music-insert": "video-to-music-insert",
  "local-video-to-sfx-insert": "video-to-sfx-insert",
  "video-to-sfx": "video-to-sfx",
  "video-to-video": "video-to-video",
  "wan-image-to-video": "wan-image-to-video",
  "wan-reference-to-video": "wan-reference-to-video",
};

function parseArgs(argv) {
  const options = {
    provider: "replicate",
    model: null,
    outDir: null,
    prompt: null,
    maxStatusPolls: null,
    pollIntervalMs: null,
    replayFixture: null,
    recordFixture: null,
    live: false,
    scenario: "text-to-image",
    generateAudio: true,
  };

  for (let index = 0; index < argv.length; index += 1) {
    const value = argv[index];
    if (value === "--") {
      continue;
    } else if (value === "--provider") {
      options.provider = requireValue(value, argv[++index]);
    } else if (value === "--model") {
      options.model = requireValue(value, argv[++index]);
    } else if (value === "--out-dir") {
      options.outDir = requireValue(value, argv[++index]);
    } else if (value === "--prompt") {
      options.prompt = requireValue(value, argv[++index]);
    } else if (value === "--max-status-polls") {
      options.maxStatusPolls = requirePositiveInteger(value, argv[++index]);
    } else if (value === "--poll-interval-ms") {
      options.pollIntervalMs = requirePositiveInteger(value, argv[++index]);
    } else if (value === "--generate-audio") {
      options.generateAudio = requireBoolean(value, argv[++index]);
    } else if (value === "--replay-fixture") {
      options.replayFixture = requireValue(value, argv[++index]);
    } else if (value === "--record-fixture") {
      options.recordFixture = requireValue(value, argv[++index]);
    } else if (value === "--live") {
      options.live = true;
    } else if (value === "--scenario") {
      options.scenario = requireValue(value, argv[++index]);
    } else if (value === "--help" || value === "-h") {
      printHelp();
      process.exit(0);
    } else {
      throw new Error(`Unknown argument: ${value}`);
    }
  }

  options.provider = normalizeProvider(options.provider);
  if (!Object.hasOwn(providerEnvVars, options.provider)) {
    throw new Error(`Unsupported provider: ${options.provider}`);
  }
  if (!Object.hasOwn(scenarioDefaultModels, options.scenario)) {
    throw new Error(`Unsupported provider E2E scenario: ${options.scenario}`);
  }
  const scenarioDefault = scenarioDefaultModels[options.scenario];
  const scenarioProvider = scenarioDefault?.provider;
  if (scenarioProvider && scenarioProvider !== options.provider) {
    throw new Error(
      `Provider E2E scenario ${options.scenario} requires provider ${scenarioProvider}`,
    );
  }
  const scenarioProviderModel = scenarioDefault?.models?.[options.provider];
  if (scenarioDefault?.models && !scenarioProviderModel) {
    throw new Error(
      `Provider E2E scenario ${options.scenario} does not support provider ${options.provider}`,
    );
  }
  options.model = options.model || scenarioProviderModel || scenarioDefault?.model || defaultModels[options.provider];
  options.outDir = options.outDir || defaultOutDir(options.provider, options.scenario);
  return options;
}

function normalizeProvider(provider) {
  return providerAliases[provider] || provider;
}

function defaultOutDir(provider, scenario) {
  if (scenario === "text-to-image") {
    return join("output", "provider-e2e", provider);
  }
  const suffix = scenarioDefaultOutDirSuffixes[scenario] || scenario;
  return join("output", "provider-e2e", `${provider}-${suffix}`);
}

function requireValue(flag, value) {
  if (!value || value.startsWith("--")) {
    throw new Error(`Missing value for ${flag}`);
  }
  return value;
}

function requirePositiveInteger(flag, value) {
  const rawValue = requireValue(flag, value);
  const parsed = Number.parseInt(rawValue, 10);
  if (!Number.isSafeInteger(parsed) || parsed < 1 || String(parsed) !== rawValue) {
    throw new Error(`Expected positive integer for ${flag}`);
  }
  return rawValue;
}

function requireBoolean(flag, value) {
  const rawValue = requireValue(flag, value);
  if (rawValue === "true") {
    return true;
  }
  if (rawValue === "false") {
    return false;
  }
  throw new Error(`Expected true or false for ${flag}`);
}

function printHelp() {
  console.log(`Usage: pnpm e2e:providers -- --provider replicate [--model black-forest-labs/flux-schnell] [--out-dir output/provider-e2e/replicate]
       ${liveSpendOptInEnvVar}=1 pnpm e2e:providers -- --provider replicate --live [--model black-forest-labs/flux-schnell] [--out-dir output/provider-e2e/replicate] [--prompt "product launch frame"] [--max-status-polls 20] [--poll-interval-ms 2000]
       ${liveSpendOptInEnvVar}=1 pnpm e2e:providers -- --provider openai --live [--model gpt-image-2] [--out-dir output/provider-e2e/openai] [--prompt "product launch frame"] [--max-status-polls 20] [--poll-interval-ms 2000]
       ${liveSpendOptInEnvVar}=1 pnpm e2e:providers -- --provider openai --live --scenario image-edit [--out-dir output/provider-e2e/openai-image-edit]
       ${liveSpendOptInEnvVar}=1 pnpm e2e:providers -- --provider openai --live --scenario local-image-edit-replace [--out-dir output/provider-e2e/openai-image-edit-replace]
       ${liveSpendOptInEnvVar}=1 pnpm e2e:providers -- --provider xai --live [--model grok-imagine-image-quality] [--out-dir output/provider-e2e/xai] [--prompt "product launch frame"] [--max-status-polls 20] [--poll-interval-ms 2000]
       ${liveSpendOptInEnvVar}=1 pnpm e2e:providers -- --provider xai --live --scenario image-edit [--out-dir output/provider-e2e/xai-image-edit]
       ${liveSpendOptInEnvVar}=1 pnpm e2e:providers -- --provider xai --live --scenario local-image-edit-replace [--out-dir output/provider-e2e/xai-image-edit-replace]
       ${liveSpendOptInEnvVar}=1 pnpm e2e:providers -- --provider xai --live --scenario xai-video [--out-dir output/provider-e2e/xai-video]
       ${liveSpendOptInEnvVar}=1 pnpm e2e:providers -- --provider xai --live --scenario local-video-edit-replace [--out-dir output/provider-e2e/xai-video-edit-replace]
       ${liveSpendOptInEnvVar}=1 pnpm e2e:providers -- --provider xai --live --scenario text-to-video-replace [--out-dir output/provider-e2e/xai-text-video-replace]
       ${liveSpendOptInEnvVar}=1 pnpm e2e:providers -- --provider xai --live --scenario text-to-video-insert [--out-dir output/provider-e2e/xai-text-video-insert]
       ${liveSpendOptInEnvVar}=1 pnpm e2e:providers -- --provider google --live --scenario google-video [--out-dir output/provider-e2e/google-video]
       ${liveSpendOptInEnvVar}=1 pnpm e2e:providers -- --provider google --live --scenario text-to-video-replace [--out-dir output/provider-e2e/google-text-video-replace]
       ${liveSpendOptInEnvVar}=1 pnpm e2e:providers -- --provider google --live --scenario text-to-video-insert [--out-dir output/provider-e2e/google-text-video-insert]
       ${liveSpendOptInEnvVar}=1 pnpm e2e:providers -- --provider replicate --live --scenario replicate-local-file-upload [--out-dir output/provider-e2e/replicate-upload]
       ${liveSpendOptInEnvVar}=1 pnpm e2e:providers -- --provider replicate --live --scenario text-to-video-replace [--out-dir output/provider-e2e/replicate-text-video-replace]
       ${liveSpendOptInEnvVar}=1 pnpm e2e:providers -- --provider replicate --live --scenario text-to-video-insert [--out-dir output/provider-e2e/replicate-text-video-insert]
       ${liveSpendOptInEnvVar}=1 pnpm e2e:providers -- --provider replicate --live --scenario replicate-flux-dev [--out-dir output/provider-e2e/replicate-flux-dev]
       ${liveSpendOptInEnvVar}=1 pnpm e2e:providers -- --provider replicate --live --scenario replicate-flux-1.1-pro [--out-dir output/provider-e2e/replicate-flux-1.1-pro]
       ${liveSpendOptInEnvVar}=1 pnpm e2e:providers -- --provider replicate --live --scenario replicate-flux-1.1-pro-ultra [--out-dir output/provider-e2e/replicate-flux-1.1-pro-ultra]
       ${liveSpendOptInEnvVar}=1 pnpm e2e:providers -- --provider fal.ai --live --scenario multi-image [--out-dir output/provider-e2e/fal.ai-multi-image]
       ${liveSpendOptInEnvVar}=1 pnpm e2e:providers -- --provider openai --live --scenario multi-image [--out-dir output/provider-e2e/openai-multi-image]
       ${liveSpendOptInEnvVar}=1 pnpm e2e:providers -- --provider replicate --live --scenario multi-image [--out-dir output/provider-e2e/replicate-multi-image]
       ${liveSpendOptInEnvVar}=1 pnpm e2e:providers -- --provider fal.ai --live --scenario local-image-upscale [--out-dir output/provider-e2e/fal.ai-upscale]
       ${liveSpendOptInEnvVar}=1 pnpm e2e:providers -- --provider fal.ai --live --scenario local-image-upscale-replace [--out-dir output/provider-e2e/fal.ai-upscale-replace]
       ${liveSpendOptInEnvVar}=1 pnpm e2e:providers -- --provider fal.ai --live --scenario local-image-upscale-retry [--out-dir output/provider-e2e/fal.ai-upscale-retry]
       ${liveSpendOptInEnvVar}=1 pnpm e2e:providers -- --provider fal.ai --live --scenario local-image-upscale-cancellation [--out-dir output/provider-e2e/fal.ai-upscale-cancellation]
       ${liveSpendOptInEnvVar}=1 pnpm e2e:providers -- --provider fal.ai --live --scenario local-image-upscale-failure [--out-dir output/provider-e2e/fal.ai-upscale-failure]
       ${liveSpendOptInEnvVar}=1 pnpm e2e:providers -- --provider fal.ai --live --scenario local-image-edit-replace [--out-dir output/provider-e2e/fal.ai-image-edit-replace]
       ${liveSpendOptInEnvVar}=1 pnpm e2e:providers -- --provider fal.ai --live --scenario local-image-edit-retry [--out-dir output/provider-e2e/fal.ai-image-edit-retry]
       ${liveSpendOptInEnvVar}=1 pnpm e2e:providers -- --provider fal.ai --live --scenario local-image-edit-cancellation [--out-dir output/provider-e2e/fal.ai-image-edit-cancellation]
       ${liveSpendOptInEnvVar}=1 pnpm e2e:providers -- --provider fal.ai --live --scenario local-image-edit-failure [--out-dir output/provider-e2e/fal.ai-image-edit-failure]
       ${liveSpendOptInEnvVar}=1 pnpm e2e:providers -- --provider fal.ai --live --scenario local-video-upscale [--out-dir output/provider-e2e/fal.ai-video-upscale]
       ${liveSpendOptInEnvVar}=1 pnpm e2e:providers -- --provider fal.ai --live --scenario local-video-upscale-replace [--out-dir output/provider-e2e/fal.ai-video-upscale-replace]
       ${liveSpendOptInEnvVar}=1 pnpm e2e:providers -- --provider fal.ai --live --scenario local-video-upscale-retry [--out-dir output/provider-e2e/fal.ai-video-upscale-retry]
       ${liveSpendOptInEnvVar}=1 pnpm e2e:providers -- --provider fal.ai --live --scenario local-video-upscale-cancellation [--out-dir output/provider-e2e/fal.ai-video-upscale-cancellation]
       ${liveSpendOptInEnvVar}=1 pnpm e2e:providers -- --provider fal.ai --live --scenario local-video-upscale-failure [--out-dir output/provider-e2e/fal.ai-video-upscale-failure]
       ${liveSpendOptInEnvVar}=1 pnpm e2e:providers -- --provider fal.ai --live --scenario local-video-edit-replace [--out-dir output/provider-e2e/fal.ai-video-edit-replace]
       ${liveSpendOptInEnvVar}=1 pnpm e2e:providers -- --provider fal.ai --live --scenario local-video-motion-control-replace [--out-dir output/provider-e2e/fal.ai-video-motion-control-replace]
       ${liveSpendOptInEnvVar}=1 pnpm e2e:providers -- --provider fal.ai --live --scenario local-video-edit-retry [--out-dir output/provider-e2e/fal.ai-video-edit-retry]
       ${liveSpendOptInEnvVar}=1 pnpm e2e:providers -- --provider fal.ai --live --scenario local-video-edit-cancellation [--out-dir output/provider-e2e/fal.ai-video-edit-cancellation]
       ${liveSpendOptInEnvVar}=1 pnpm e2e:providers -- --provider fal.ai --live --scenario local-video-edit-failure [--out-dir output/provider-e2e/fal.ai-video-edit-failure]
       ${liveSpendOptInEnvVar}=1 pnpm e2e:providers -- --provider fal.ai --live --scenario image-edit [--out-dir output/provider-e2e/fal.ai-image-edit]
       ${liveSpendOptInEnvVar}=1 pnpm e2e:providers -- --provider fal.ai --live --scenario krea-text-to-image [--out-dir output/provider-e2e/fal.ai-krea-image]
       ${liveSpendOptInEnvVar}=1 pnpm e2e:providers -- --provider fal.ai --live --scenario recraft-text-to-image [--out-dir output/provider-e2e/fal.ai-recraft-image]
       ${liveSpendOptInEnvVar}=1 pnpm e2e:providers -- --provider fal.ai --live --scenario wan-image-to-video [--out-dir output/provider-e2e/fal.ai-wan-image-to-video]
       ${liveSpendOptInEnvVar}=1 pnpm e2e:providers -- --provider fal.ai --live --scenario wan-reference-to-video [--out-dir output/provider-e2e/fal.ai-wan-reference-to-video]
       ${liveSpendOptInEnvVar}=1 pnpm e2e:providers -- --provider fal.ai --live --scenario kling-image-to-video [--out-dir output/provider-e2e/fal.ai-kling-image-to-video]
       ${liveSpendOptInEnvVar}=1 pnpm e2e:providers -- --provider fal.ai --live --scenario video-to-video [--out-dir output/provider-e2e/fal.ai-video-to-video]
       ${liveSpendOptInEnvVar}=1 pnpm e2e:providers -- --provider fal.ai --live --scenario text-to-audio [--out-dir output/provider-e2e/fal.ai-audio]
       ${liveSpendOptInEnvVar}=1 pnpm e2e:providers -- --provider openai --live --scenario text-to-audio [--out-dir output/provider-e2e/openai-audio]
       ${liveSpendOptInEnvVar}=1 pnpm e2e:providers -- --provider elevenlabs --live --scenario text-to-audio [--out-dir output/provider-e2e/elevenlabs-audio]
       ${liveSpendOptInEnvVar}=1 pnpm e2e:providers -- --provider fal.ai --live --scenario text-to-music [--out-dir output/provider-e2e/fal.ai-sonilo-music]
       ${liveSpendOptInEnvVar}=1 pnpm e2e:providers -- --provider elevenlabs --live --scenario text-to-music [--out-dir output/provider-e2e/elevenlabs-music]
       ${liveSpendOptInEnvVar}=1 pnpm e2e:providers -- --provider google --live --scenario text-to-audio [--out-dir output/provider-e2e/google-gemini-tts]
       ${liveSpendOptInEnvVar}=1 pnpm e2e:providers -- --provider google --live --scenario text-to-music [--out-dir output/provider-e2e/google-lyria]
       ${liveSpendOptInEnvVar}=1 pnpm e2e:providers -- --provider minimax --live --scenario text-to-music [--out-dir output/provider-e2e/minimax-music]
       ${liveSpendOptInEnvVar}=1 pnpm e2e:providers -- --provider fal.ai --live --scenario text-to-music-insert [--out-dir output/provider-e2e/fal.ai-sonilo-music-insert]
       ${liveSpendOptInEnvVar}=1 pnpm e2e:providers -- --provider fal.ai --live --scenario video-to-music [--out-dir output/provider-e2e/fal.ai-video-to-music]
       ${liveSpendOptInEnvVar}=1 pnpm e2e:providers -- --provider fal.ai --live --scenario local-video-to-music-insert [--out-dir output/provider-e2e/fal.ai-video-to-music-insert]
       ${liveSpendOptInEnvVar}=1 pnpm e2e:providers -- --provider fal.ai --live --scenario video-to-sfx [--out-dir output/provider-e2e/fal.ai-video-to-sfx]
       ${liveSpendOptInEnvVar}=1 pnpm e2e:providers -- --provider fal.ai --live --scenario local-video-to-sfx-insert [--out-dir output/provider-e2e/fal.ai-video-to-sfx-insert]
       ${liveSpendOptInEnvVar}=1 pnpm e2e:providers -- --provider fal.ai --live --scenario text-to-video-replace [--out-dir output/provider-e2e/fal.ai-text-video-replace]
       ${liveSpendOptInEnvVar}=1 pnpm e2e:providers -- --provider fal.ai --live --scenario text-to-video-insert [--out-dir output/provider-e2e/fal.ai-text-video-insert]
       ${liveSpendOptInEnvVar}=1 pnpm e2e:providers -- --provider replicate --live --scenario replicate-video [--out-dir output/provider-e2e/replicate-video]
       ${liveSpendOptInEnvVar}=1 pnpm e2e:providers -- --provider replicate --live --scenario replicate-video-fast [--out-dir output/provider-e2e/replicate-video-fast]
       pnpm e2e:providers -- --provider replicate --scenario provider-failure [--out-dir output/provider-e2e/replicate-failure]
       pnpm e2e:providers -- --provider fal.ai --scenario provider-cancellation [--out-dir output/provider-e2e/fal.ai-cancellation]
       pnpm e2e:providers -- --provider replicate --scenario provider-retry [--out-dir output/provider-e2e/replicate-retry]
       pnpm e2e:providers -- --provider replicate --replay-fixture tests/fixtures/provider-e2e/replicate

Runs against a deterministic mocked provider service by default. Live provider
generation through the Rust provider E2E binary is only allowed when --live is
present and ${liveSpendOptInEnvVar}=1 is set. Live mode also requires the
selected provider credential in the environment and prints a JSON report without
echoing credential values. Use --record-fixture with an intentional --live run to
retain a replayable output tree, then --replay-fixture for no-spend local
evidence checks.`);
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
      insertedItemId: "item-video-to-sfx-inserted-audio",
      placementIntent: "insert-audio:item-video-to-sfx-source",
      insertedLabel: "Provider E2E generated SFX",
      timelineStartSeconds: 8,
      videoSourceStartSeconds: 0.5,
      videoSourceEndSeconds: 2,
    };
  }
  return {
    sourceItemId: "item-video-to-music-source",
    insertedItemId: "item-video-to-music-inserted-audio",
    placementIntent: "insert-audio:item-video-to-music-source",
    insertedLabel: "Provider E2E generated music",
    timelineStartSeconds: 12,
    videoSourceStartSeconds: 1.25,
    videoSourceEndSeconds: 5.75,
  };
}

function timelineVisualInsertConfig() {
  return {
    trackId: "track-video",
    insertedItemId: "item-text-video-inserted-video",
    placementIntent: "insert-video:track-video",
    insertedLabel: "Provider E2E generated video",
    timelineStartSeconds: 6,
  };
}

function timelineTextToMusicInsertConfig() {
  return {
    trackId: "track-audio",
    insertedItemId: "item-text-music-inserted-audio",
    placementIntent: "insert-audio:track-audio",
    insertedLabel: "Provider E2E generated music",
    timelineStartSeconds: 12,
  };
}

function isSourceVideoGenerationScenario(scenario) {
  return (
    scenario === "video-to-video" ||
    scenario === "local-video-upscale" ||
    scenario === "local-video-upscale-replace" ||
    scenario === "local-video-upscale-retry" ||
    scenario === "local-video-upscale-cancellation" ||
    scenario === "local-video-upscale-failure" ||
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
  return Object.hasOwn(replicateFluxVariantModels, scenario);
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

function isImageEditScenario(scenario) {
  return (
    scenario === "image-edit" ||
    scenario === "local-image-edit-replace" ||
    scenario === "local-image-edit-retry" ||
    scenario === "local-image-edit-cancellation" ||
    scenario === "local-image-edit-failure"
  );
}

function imageEditReferencesForProvider(provider) {
  if (provider === "openai") {
    return {
      mediaIds: ["provider-e2e-image-ref", "provider-e2e-style-ref"],
      referenceImageMediaRefs: ["provider-e2e-image-ref", "provider-e2e-style-ref"],
      providerInputUrls: [
        "data:image/png;base64,bW9jay1vcGVuYWktaW1hZ2UtcmVm",
        "data:image/png;base64,bW9jay1vcGVuYWktc3R5bGUtcmVm",
      ],
    };
  }
  if (provider === "xai") {
    return {
      mediaIds: ["provider-e2e-image-ref"],
      referenceImageMediaRefs: ["provider-e2e-image-ref"],
      providerInputUrls: ["data:image/png;base64,bW9jay14YWktaW1hZ2UtcmVm"],
    };
  }
  return {
    mediaIds: ["provider-e2e-image-ref"],
    referenceImageMediaRefs: ["provider-e2e-image-ref"],
    providerInputUrls: ["https://v3.fal.media/files/provider-e2e/image-ref.png"],
  };
}

function googleVeoImageReferences() {
  return {
    mediaIds: [
      "provider-e2e-first-frame",
      "provider-e2e-last-frame",
      "provider-e2e-reference-image",
    ],
    firstFrameMediaId: "provider-e2e-first-frame",
    lastFrameMediaId: "provider-e2e-last-frame",
    referenceImageMediaRefs: ["provider-e2e-reference-image"],
    providerInputUrls: [
      "data:image/png;base64,bW9jay1nb29nbGUtZmlyc3QtZnJhbWU=",
      "data:image/png;base64,bW9jay1nb29nbGUtbGFzdC1mcmFtZQ==",
      "data:image/png;base64,bW9jay1nb29nbGUtcmVmZXJlbmNlLWltYWdl",
    ],
  };
}

function xaiVideoImageReferences() {
  return {
    mediaIds: ["provider-e2e-reference-image"],
    referenceImageMediaRefs: ["provider-e2e-reference-image"],
    providerInputUrls: ["https://api.x.ai/v1/files/provider-e2e/reference-image.png"],
  };
}

function sourceContextReferencesForScenario(provider, scenario) {
  if (isLocalImageUpscaleScenario(scenario)) {
    return {
      mediaIds: ["provider-e2e-source"],
      providerInputUrls: ["https://v3.fal.media/files/provider-e2e/source-image.png"],
    };
  }
  if (isSourceVideoGenerationScenario(scenario) || isVideoToAudioScenario(scenario)) {
    if (scenario === "local-video-motion-control-replace") {
      return {
        mediaIds: ["provider-e2e-video-source", "provider-e2e-motion-image-ref"],
        sourceVideoMediaRef: "provider-e2e-video-source",
        referenceImageMediaRefs: ["provider-e2e-motion-image-ref"],
        providerInputUrls: [
          "https://v3.fal.media/files/provider-e2e/source-video.mp4",
          "https://v3.fal.media/files/provider-e2e/motion-image-ref.png",
        ],
      };
    }
    return {
      mediaIds: ["provider-e2e-video-source"],
      sourceVideoMediaRef: "provider-e2e-video-source",
      referenceVideoMediaRefs: ["provider-e2e-video-source"],
      providerInputUrls: [
        provider === "xai"
          ? "https://api.x.ai/v1/files/provider-e2e/source-video.mp4"
          : "https://v3.fal.media/files/provider-e2e/source-video.mp4",
      ],
    };
  }
  if (scenario === "xai-video") {
    return xaiVideoImageReferences();
  }
  if (scenario === "google-video") {
    return googleVeoImageReferences();
  }
  if (isImageEditScenario(scenario)) {
    return imageEditReferencesForProvider(provider);
  }
  return undefined;
}

function sourceContextSettingsForScenario(scenario, generateAudio = true) {
  if (scenario === "wan-reference-to-video") {
    return {
      durationSeconds: 4,
      fps: 24,
      aspectRatio: "16:9",
      resolution: "720p",
      generateAudio,
    };
  }
  if (isLocalVideoUpscaleScenario(scenario)) {
    return {
      width: 1280,
      height: 720,
      durationSeconds: 4,
      fps: 24,
      aspectRatio: "16:9",
      videoSourceStartSeconds: 0.5,
      videoSourceEndSeconds: 4.5,
    };
  }
  if (
    scenario === "local-video-edit-replace" ||
    scenario === "local-video-motion-control-replace" ||
    scenario === "local-video-edit-retry" ||
    scenario === "local-video-edit-cancellation" ||
    scenario === "local-video-edit-failure"
  ) {
    return {
      width: 1280,
      height: 720,
      durationSeconds: 3,
      fps: 24,
      aspectRatio: "16:9",
      generateAudio,
      videoSourceStartSeconds: 0.5,
      videoSourceEndSeconds: 3.5,
    };
  }
  if (isProviderVideoGenerationScenario(scenario)) {
    return {
      generateAudio,
    };
  }
  return undefined;
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

function redactCredential(text, credential) {
  if (!credential) {
    return text || "";
  }
  return (text || "").split(credential).join("[redacted]");
}

function safeJsonParse(text) {
  try {
    return JSON.parse(text);
  } catch {
    return null;
  }
}

function isPrivateIpv4Hostname(hostname) {
  const parts = hostname.split(".");
  if (parts.length !== 4) {
    return false;
  }
  const octets = parts.map((part) => Number.parseInt(part, 10));
  if (
    octets.some(
      (octet, index) =>
        !Number.isInteger(octet) ||
        String(octet) !== parts[index] ||
        octet < 0 ||
        octet > 255,
    )
  ) {
    return false;
  }
  const [first, second] = octets;
  return (
    first === 0 ||
    first === 10 ||
    first === 127 ||
    (first === 100 && second >= 64 && second <= 127) ||
    (first === 169 && second === 254) ||
    (first === 172 && second >= 16 && second <= 31) ||
    (first === 192 && second === 168)
  );
}

function isLocalProviderHostname(hostname) {
  const normalized = hostname.toLowerCase();
  const ipv6Hostname = normalized.replace(/^\[/, "").replace(/\]$/, "");
  return (
    normalized === "localhost" ||
    normalized.endsWith(".localhost") ||
    normalized.endsWith(".local") ||
    isPrivateIpv4Hostname(normalized) ||
    ipv6Hostname === "::" ||
    ipv6Hostname === "::1" ||
    ipv6Hostname.startsWith("fc") ||
    ipv6Hostname.startsWith("fd") ||
    ipv6Hostname.startsWith("fe80:")
  );
}

function isProviderSourceUrl(value) {
  try {
    const url = new URL(value);
    return (
      (url.protocol === "http:" || url.protocol === "https:") &&
      !isLocalProviderHostname(url.hostname)
    );
  } catch {
    return false;
  }
}

function providerRequestError(providerRequest, options) {
  if (!providerRequest || typeof providerRequest !== "object") {
    return "Provider E2E binary did not report provider request metadata";
  }
  if (providerRequest.provider !== options.provider) {
    return `Provider E2E provider request provider does not match requested provider: ${providerRequest.provider ?? "unknown"}`;
  }
  for (const [field, label] of [
    ["requestId", "request id"],
    ["statusUrl", "status URL"],
    ["responseUrl", "response URL"],
    ["cancelUrl", "cancel URL"],
    ["submittedAt", "submission timestamp"],
  ]) {
    if (typeof providerRequest[field] !== "string" || providerRequest[field].trim() === "") {
      return `Provider E2E provider request is missing a ${label}`;
    }
  }
  for (const [field, label] of [
    ["statusUrl", "status URL"],
    ["responseUrl", "response URL"],
    ["cancelUrl", "cancel URL"],
  ]) {
    if (!isProviderSourceUrl(providerRequest[field])) {
      return `Provider E2E provider request ${label} is not a provider URL`;
    }
  }
  return null;
}

function providerRequestsEqual(left, right) {
  return (
    left.provider === right.provider &&
    left.requestId === right.requestId &&
    left.statusUrl === right.statusUrl &&
    left.responseUrl === right.responseUrl &&
    left.cancelUrl === right.cancelUrl &&
    left.submittedAt === right.submittedAt
  );
}

function isWithinDirectory(rootPath, candidatePath) {
  const root = resolve(rootPath);
  const candidate = resolve(candidatePath);
  return candidate === root || candidate.startsWith(`${root}${sep}`);
}

function copyDirectory(sourceDir, targetDir) {
  rmSync(targetDir, { recursive: true, force: true });
  mkdirSync(dirname(targetDir), { recursive: true });
  cpSync(sourceDir, targetDir, { recursive: true });
}

function rewriteFixtureReport(report, options) {
  const nextReport = {
    ...report,
    ok: true,
    provider: options.provider,
    model: options.model,
    scenario: options.scenario,
    outDir: options.outDir,
    reportPath: "provider-e2e-report.json",
    credentialStatus: report.credentialStatus || "present",
    command: null,
    exitCode: 0,
    signal: null,
    fixtureReplay: true,
    error: null,
  };
  const result = report.result && typeof report.result === "object"
    ? { ...report.result }
    : null;
  if (result) {
    const fixtureRoot = resolve(options.replayFixture);
    const resolvedOldProjectDir = resolvePathFromFixture(report.result.projectDir, fixtureRoot);
    const sourceProjectDir = fixtureProjectRoot(fixtureRoot, resolvedOldProjectDir);
    const resolvedOldArtifactPath = resolvePathFromFixture(report.result.artifactPath, fixtureRoot);
    const sourceArtifactPath = isWithinDirectory(fixtureRoot, resolvedOldArtifactPath) && existsSync(resolvedOldArtifactPath)
      ? resolvedOldArtifactPath
      : join(
          sourceProjectDir,
          relative(resolve(report.result.projectDir), resolve(report.result.artifactPath)),
        );
    const newProjectDir = join(options.outDir, relative(fixtureRoot, sourceProjectDir));
    result.projectDir = newProjectDir;
    if (typeof report.result.artifactPath === "string" && typeof report.result.projectDir === "string") {
      result.artifactPath = join(newProjectDir, relative(sourceProjectDir, sourceArtifactPath));
    }
    nextReport.result = result;
  }
  return nextReport;
}

function resolvePathFromFixture(value, fixtureRoot) {
  if (typeof value !== "string" || value.trim() === "") {
    return fixtureRoot;
  }
  return isAbsolute(value) ? resolve(value) : resolve(fixtureRoot, value);
}

function fixtureProjectRoot(fixtureRoot, candidateProjectDir) {
  if (existsSync(join(candidateProjectDir, "video-creater.project.json"))) {
    return candidateProjectDir;
  }
  if (existsSync(join(fixtureRoot, "video-creater.project.json"))) {
    return fixtureRoot;
  }
  return isWithinDirectory(fixtureRoot, candidateProjectDir)
    ? candidateProjectDir
    : fixtureRoot;
}

function replayFixture(options) {
  copyDirectory(options.replayFixture, options.outDir);
  const reportPath = join(options.outDir, "provider-e2e-report.json");
  const sourceReport = safeJsonParse(readFileSync(reportPath, "utf8"));
  const report = rewriteFixtureReport(sourceReport, options);
  const error = validateChildResult(report.result, options);
  report.ok = error === null;
  report.error = error;
  const reportJson = JSON.stringify(report, null, 2);
  writeFileSync(reportPath, `${reportJson}\n`);
  console.log(reportJson);
  process.exit(report.ok ? 0 : 1);
}

function createMockProviderResult(options) {
  rmSync(options.outDir, { recursive: true, force: true });
  if (options.scenario === "replicate-local-file-upload") {
    const sourcePath = join(options.outDir, "media", "provider-e2e-source.png");
    mkdirSync(dirname(sourcePath), { recursive: true });
    writeFileSync(sourcePath, "mock source pixels");
    return {
      ok: true,
      provider: options.provider,
      model: options.model,
      scenario: options.scenario,
      projectDir: options.outDir,
      artifactPath: sourcePath,
      outputCount: 0,
      jobStatus: "Uploaded",
      providerFileUrl: "https://api.replicate.com/v1/files/mock-file",
    };
  }
  const lifecycleScenario = providerLifecycleScenario(options.scenario);
  if (lifecycleScenario) {
    return createMockProviderLifecycleResult(options, lifecycleScenario);
  }
  const safeProvider = options.provider.replace(/\./g, "-");
  const projectDir = options.outDir;
  const generatedId = "provider-e2e-generated";
  const failedRetryAssetId = "provider-e2e-generated-failed-attempt";
  const mediaId = "provider-e2e-output";
  const isWanImageToVideo = options.scenario === "wan-image-to-video";
  const isWanReferenceToVideo = options.scenario === "wan-reference-to-video";
  const isKlingImageToVideo = options.scenario === "kling-image-to-video";
  const isReplicateVideo =
    options.scenario === "replicate-video" || options.scenario === "replicate-video-fast";
  const isXaiVideo = options.scenario === "xai-video";
  const isGoogleVideo = options.scenario === "google-video";
  const isTextToVideoReplace = options.scenario === "text-to-video-replace";
  const isTextToVideoInsert = options.scenario === "text-to-video-insert";
  const isReferencedVideo =
    isWanImageToVideo || isWanReferenceToVideo || isKlingImageToVideo || isReplicateVideo;
  const isVideoToVideo = isSourceVideoGenerationScenario(options.scenario);
  const isVideoOutputScenario =
    isReferencedVideo ||
    isVideoToVideo ||
    isXaiVideo ||
    isGoogleVideo ||
    isTextToVideoReplace ||
    isTextToVideoInsert;
  const isImageEdit = isImageEditScenario(options.scenario);
  const isTextToAudio = options.scenario === "text-to-audio";
  const isTextToMusic =
    options.scenario === "text-to-music" || isTextToMusicInsertScenario(options.scenario);
  const isVideoToMusic = isVideoToMusicScenario(options.scenario);
  const isVideoToSfx =
    options.scenario === "video-to-sfx" || options.scenario === "local-video-to-sfx-insert";
  const isAudioScenario = isTextToAudio || isTextToMusic || isVideoToMusic || isVideoToSfx;
  const isRecraftTextToImage = options.scenario === "recraft-text-to-image";
  const isReplacement = isReplacementScenario(options.scenario);
  const isTimelineAudioInsert = isTimelineAudioInsertScenario(options.scenario);
  const isTextToMusicInsert = isTextToMusicInsertScenario(options.scenario);
  const isTimelineVisualInsert = isTimelineVisualInsertScenario(options.scenario);
  const isRetry = isProviderRetryScenario(options.scenario);
  const outputCount = isMultiImageScenario(options.scenario) ? 2 : 1;
  const generatedDurationSeconds = options.scenario === "text-to-video-replace" ||
    options.scenario === "text-to-video-insert"
    ? 5
    : options.scenario === "local-video-edit-replace" ||
    options.scenario === "local-video-motion-control-replace" ||
    options.scenario === "local-video-edit-retry"
    ? 3
    : options.scenario === "text-to-image-replace" ||
      options.scenario === "local-image-upscale-replace" ||
      options.scenario === "local-image-edit-replace" ||
      options.scenario === "local-image-edit-retry"
      ? 4
    : isVideoOutputScenario
      ? 4
      : isAudioScenario
        ? (isTextToMusic ? 30 : isTextToAudio ? 12 : 10)
        : 0;
  const outputExtension = isVideoOutputScenario
    ? "mp4"
    : isVideoToMusic
      ? "m4a"
      : isVideoToSfx
        ? "wav"
        : isTextToAudio
          ? options.provider === "google"
            ? "wav"
            : "mp3"
          : isTextToMusic
            ? options.provider === "fal.ai"
              ? "m4a"
              : "mp3"
          : isRecraftTextToImage
            ? "webp"
            : "png";
  const outputRelativePath = `generated/${generatedId}/${safeProvider}-mock-output.${outputExtension}`;
  const outputPath = join(projectDir, outputRelativePath);
  const outputRelativePaths = Array.from({ length: outputCount }, (_, index) =>
    index === 0
      ? outputRelativePath
      : `generated/${generatedId}/${safeProvider}-mock-output-${index + 1}.${outputExtension}`,
  );
  const outputPaths = outputRelativePaths.map((relativePath) => join(projectDir, relativePath));
  const outputMediaIds = Array.from({ length: outputCount }, (_, index) =>
    index === 0 ? mediaId : `${mediaId}-${index + 1}`,
  );
  const sidecarRelativePath = `generated/${generatedId}/asset.json`;
  const submittedAt = new Date(0).toISOString();
  const requestId = `${safeProvider}-mock-request`;
  const imageEditReferences = imageEditReferencesForProvider(options.provider);
  const providerRequest = {
    provider: options.provider,
    requestId,
    statusUrl: `https://mock.${safeProvider}.example/v1/${requestId}`,
    responseUrl: `https://mock.${safeProvider}.example/v1/${requestId}/response`,
    cancelUrl: `https://mock.${safeProvider}.example/v1/${requestId}/cancel`,
    submittedAt,
  };
  const failedRetryProviderRequest = {
    ...providerRequest,
    requestId: `${requestId}-failed-attempt`,
    statusUrl: `https://mock.${safeProvider}.example/v1/${requestId}-failed-attempt`,
    responseUrl: `https://mock.${safeProvider}.example/v1/${requestId}-failed-attempt/response`,
    cancelUrl: `https://mock.${safeProvider}.example/v1/${requestId}-failed-attempt/cancel`,
  };

  mkdirSync(join(projectDir, "generated", generatedId), { recursive: true });
  mkdirSync(join(projectDir, "media"), { recursive: true });
  mkdirSync(join(projectDir, "jobs", generatedId), { recursive: true });
  if (isRetry) {
    mkdirSync(join(projectDir, "generated", failedRetryAssetId), { recursive: true });
    mkdirSync(join(projectDir, "jobs", failedRetryAssetId), { recursive: true });
  }
  for (const [index, currentOutputPath] of outputPaths.entries()) {
    writeFileSync(
      currentOutputPath,
      isVideoOutputScenario
        ? outputCount === 1
          ? "mock generated video bytes"
          : `mock generated video bytes ${index + 1}`
        : isAudioScenario
          ? outputCount === 1
            ? "mock generated audio bytes"
            : `mock generated audio bytes ${index + 1}`
          : outputCount === 1
            ? "mock generated pixels"
            : `mock generated pixels ${index + 1}`,
    );
  }
  writeFileSync(
    join(projectDir, "video-creater.project.json"),
    JSON.stringify(
      {
        schemaVersion: 2,
        id: "provider-e2e-project",
        name: "Provider E2E Mock",
        createdAt: submittedAt,
        updatedAt: submittedAt,
        layout: "split",
        files: {
          ...(isReplacement || isTimelineAudioInsert || isTextToMusicInsert || isTimelineVisualInsert
            ? { timeline: "timeline.json" }
            : {}),
          media: "media/index.json",
          generated: "generated",
        },
        jobs: [
          ...(isRetry
            ? [
                {
                  id: failedRetryAssetId,
                  kind: "generate_media",
                  status: "failed",
                  updatedAt: submittedAt,
                  providerRequest: failedRetryProviderRequest,
                },
              ]
            : []),
          {
            id: generatedId,
            kind: "generate_media",
            status: "completed",
            updatedAt: submittedAt,
            providerRequest,
          },
        ],
        exportArtifacts: [],
      },
      null,
      2,
    ),
  );
  writeFileSync(
    join(projectDir, "media", "index.json"),
    JSON.stringify(
      {
        schemaVersion: 1,
        folders: [],
        assets: outputRelativePaths.map((relativePath, index) => ({
          id: outputMediaIds[index],
          relativePath,
          kind: "generated",
          durationSeconds: generatedDurationSeconds,
          width: isVideoOutputScenario ? 1280 : isAudioScenario ? 1 : 1024,
          height: isVideoOutputScenario ? 720 : isAudioScenario ? 1 : 768,
          fps: isVideoOutputScenario ? 24 : isAudioScenario ? 1 : 0,
        })),
        analysis: [],
      },
      null,
      2,
    ),
  );
  if (isTimelineAudioInsert) {
    const insertConfig = timelineAudioInsertConfig(options.scenario);
    writeFileSync(
      join(projectDir, "timeline.json"),
      JSON.stringify(
        {
          schemaVersion: 1,
          durationSeconds: 24,
          tracks: [
            {
              id: "track-video",
              name: "Video",
              kind: "video",
              locked: false,
              enabled: true,
              items: [
                {
                  id: insertConfig.sourceItemId,
                  kind: "video_clip",
                  startSeconds: insertConfig.timelineStartSeconds,
                  durationSeconds:
                    insertConfig.videoSourceEndSeconds - insertConfig.videoSourceStartSeconds,
                  source: { type: "media", mediaId: "provider-e2e-video-source" },
                  label: "Provider E2E source video",
                  properties: {
                    sourceIn: insertConfig.videoSourceStartSeconds,
                    sourceOut: insertConfig.videoSourceEndSeconds,
                  },
                },
              ],
            },
            {
              id: "track-audio",
              name: "Audio",
              kind: "audio",
              locked: false,
              enabled: true,
              items: [
                {
                  id: insertConfig.insertedItemId,
                  kind: "audio_clip",
                  startSeconds: insertConfig.timelineStartSeconds,
                  durationSeconds: generatedDurationSeconds,
                  source: { type: "media", mediaId },
                  label: insertConfig.insertedLabel,
                  properties: {
                    sourceIn: 0,
                    sourceOut: generatedDurationSeconds,
                    generatedOutputMediaId: mediaId,
                    sourceVideoMediaRef: "provider-e2e-video-source",
                  },
                },
              ],
            },
          ],
        },
        null,
        2,
      ),
    );
  }
  if (isTextToMusicInsert) {
    const insertConfig = timelineTextToMusicInsertConfig();
    writeFileSync(
      join(projectDir, "timeline.json"),
      JSON.stringify(
        {
          schemaVersion: 1,
          durationSeconds: insertConfig.timelineStartSeconds + generatedDurationSeconds,
          tracks: [
            {
              id: insertConfig.trackId,
              name: "Audio",
              kind: "audio",
              locked: false,
              enabled: true,
              items: [
                {
                  id: insertConfig.insertedItemId,
                  kind: "audio_clip",
                  startSeconds: insertConfig.timelineStartSeconds,
                  durationSeconds: generatedDurationSeconds,
                  source: { type: "media", mediaId },
                  label: insertConfig.insertedLabel,
                  properties: {
                    sourceIn: 0,
                    sourceOut: generatedDurationSeconds,
                    generatedOutputMediaId: mediaId,
                  },
                },
              ],
            },
          ],
        },
        null,
        2,
      ),
    );
  }
  if (isTimelineVisualInsert) {
    const insertConfig = timelineVisualInsertConfig();
    writeFileSync(
      join(projectDir, "timeline.json"),
      JSON.stringify(
        {
          schemaVersion: 1,
          durationSeconds: insertConfig.timelineStartSeconds + generatedDurationSeconds,
          tracks: [
            {
              id: insertConfig.trackId,
              name: "Video",
              kind: "video",
              locked: false,
              enabled: true,
              items: [
                {
                  id: insertConfig.insertedItemId,
                  kind: "video_clip",
                  startSeconds: insertConfig.timelineStartSeconds,
                  durationSeconds: generatedDurationSeconds,
                  source: { type: "media", mediaId },
                  label: insertConfig.insertedLabel,
                  properties: {
                    sourceIn: 0,
                    sourceOut: generatedDurationSeconds,
                    generatedOutputMediaId: mediaId,
                  },
                },
              ],
            },
          ],
        },
        null,
        2,
      ),
    );
  }
  if (isReplacement) {
    const replacementItemId = replacementScenarioTargetItemId(options.scenario);
    const linkedReplacementItemId = `${replacementItemId}-linked`;
    const linkGroupId = `${replacementItemId}-link`;
    const linkedDurationSeconds = Math.max(0.001, generatedDurationSeconds - 1);
    writeFileSync(
      join(projectDir, "timeline.json"),
      JSON.stringify(
        {
          schemaVersion: 1,
          durationSeconds: generatedDurationSeconds,
          tracks: [
            {
              id: "track-video",
              name: "Video",
              kind: "video",
              locked: false,
              enabled: true,
              items: [
                {
                  id: replacementItemId,
                  kind: "video_clip",
                  startSeconds: 0,
                  durationSeconds: generatedDurationSeconds,
                  source: { type: "media", mediaId },
                  label: options.scenario === "local-video-edit-replace" ||
                    options.scenario === "local-video-upscale-replace" ||
                    options.scenario === "text-to-video-replace"
                    ? "Provider E2E source video"
                    : "Provider E2E source image",
                  properties: {
                    sourceIn: 0,
                    sourceOut: generatedDurationSeconds,
                    generatedOutputMediaId: mediaId,
                    linkGroupId,
                    replacementReason: "generated replacement",
                  },
                },
                {
                  id: linkedReplacementItemId,
                  kind: "video_clip",
                  startSeconds: generatedDurationSeconds,
                  durationSeconds: linkedDurationSeconds,
                  source: { type: "media", mediaId },
                  label: "Provider E2E linked replacement",
                  properties: {
                    sourceIn: 0,
                    sourceOut: linkedDurationSeconds,
                    generatedOutputMediaId: mediaId,
                    linkGroupId,
                    replacementReason: "generated replacement",
                  },
                },
              ],
            },
          ],
        },
        null,
        2,
      ),
    );
  }
  writeFileSync(
    join(projectDir, "generated", "index.json"),
    JSON.stringify(
      {
        schemaVersion: 1,
        assets: [
          ...(isRetry
            ? [
                {
                  assetId: failedRetryAssetId,
                  path: `generated/${failedRetryAssetId}/asset.json`,
                  status: "failed",
                  modelProvider: options.provider,
                  modelId: options.model,
                  outputCount: 0,
                },
              ]
            : []),
          {
            assetId: generatedId,
            path: sidecarRelativePath,
            status: "completed",
            modelProvider: options.provider,
            modelId: options.model,
            outputCount,
          },
        ],
      },
      null,
      2,
    ),
  );
  writeFileSync(
    join(projectDir, "generated", generatedId, "asset.json"),
    JSON.stringify(
      {
        id: generatedId,
        status: "completed",
        model: { provider: options.provider, id: options.model },
        prompt: isVideoToMusic
          ? (options.prompt ?? videoToMusicDefaultPrompt)
          : isVideoToSfx
            ? (options.prompt ?? "")
            : undefined,
        placementIntent: isReplacement
          ? `replace:${replacementScenarioTargetItemId(options.scenario)}`
          : isTimelineAudioInsert
            ? timelineAudioInsertConfig(options.scenario).placementIntent
            : isTextToMusicInsert
              ? timelineTextToMusicInsertConfig().placementIntent
            : isTimelineVisualInsert
              ? timelineVisualInsertConfig().placementIntent
            : undefined,
        parentAssetId: null,
        retryOfAssetId: isRetry ? failedRetryAssetId : null,
        references: isReferencedVideo
          ? isWanReferenceToVideo
            ? {
                mediaIds: ["provider-e2e-reference-image", "provider-e2e-reference-video"],
                referenceImageMediaRefs: ["provider-e2e-reference-image"],
                referenceVideoMediaRefs: ["provider-e2e-reference-video"],
                providerInputUrls: [
                  "https://v3.fal.media/files/provider-e2e/reference-image.png",
                  "https://v3.fal.media/files/provider-e2e/reference-video.mp4",
                ],
              }
            : {
              mediaIds: isKlingImageToVideo
                ? ["provider-e2e-first-frame", "provider-e2e-last-frame"]
                : ["provider-e2e-first-frame", "provider-e2e-last-frame", "provider-e2e-audio"],
              firstFrameMediaId: "provider-e2e-first-frame",
              lastFrameMediaId: "provider-e2e-last-frame",
              ...(isKlingImageToVideo
                ? {}
                : { referenceAudioMediaRefs: ["provider-e2e-audio"] }),
              providerInputUrls: isKlingImageToVideo
                ? [
                    "https://v3.fal.media/files/provider-e2e/first-frame.png",
                    "https://v3.fal.media/files/provider-e2e/last-frame.png",
                  ]
                : [
                    "https://v3.fal.media/files/provider-e2e/first-frame.png",
                    "https://v3.fal.media/files/provider-e2e/last-frame.png",
                    "https://v3.fal.media/files/provider-e2e/audio.wav",
                  ],
            }
          : isGoogleVideo
            ? googleVeoImageReferences()
          : isXaiVideo
            ? xaiVideoImageReferences()
          : isLocalImageUpscaleScenario(options.scenario)
            ? {
                mediaIds: ["provider-e2e-source"],
                providerInputUrls: ["https://v3.fal.media/files/provider-e2e/source-image.png"],
              }
          : options.scenario === "local-video-motion-control-replace"
            ? {
                mediaIds: ["provider-e2e-video-source", "provider-e2e-motion-image-ref"],
                sourceVideoMediaRef: "provider-e2e-video-source",
                referenceImageMediaRefs: ["provider-e2e-motion-image-ref"],
                providerInputUrls: [
                  "https://v3.fal.media/files/provider-e2e/source-video.mp4",
                  "https://v3.fal.media/files/provider-e2e/motion-image-ref.png",
                ],
              }
          : isVideoToMusic || isVideoToSfx
            ? {
                mediaIds: ["provider-e2e-video-source"],
                sourceVideoMediaRef: "provider-e2e-video-source",
                referenceVideoMediaRefs: ["provider-e2e-video-source"],
                providerInputUrls: [
                  options.provider === "xai"
                    ? "https://api.x.ai/v1/files/provider-e2e/source-video.mp4"
                    : "https://v3.fal.media/files/provider-e2e/source-video.mp4",
                ],
              }
            : isVideoToVideo
            ? {
                mediaIds: ["provider-e2e-video-source"],
                sourceVideoMediaRef: "provider-e2e-video-source",
                referenceVideoMediaRefs: ["provider-e2e-video-source"],
                providerInputUrls: [
                  options.provider === "xai"
                    ? "https://api.x.ai/v1/files/provider-e2e/source-video.mp4"
                    : "https://v3.fal.media/files/provider-e2e/source-video.mp4",
                ],
              }
          : isImageEdit
            ? imageEditReferences
          : undefined,
        outputs: outputRelativePaths.map((relativePath, index) => ({
            mediaId: outputMediaIds[index],
            relativePath,
            ...(options.provider === "openai" ||
            options.provider === "elevenlabs" ||
            (options.provider === "fal.ai" && isTextToMusic) ||
            (options.provider === "google" && (isTextToAudio || isTextToMusic)) ||
            (options.provider === "minimax" && isTextToMusic)
              ? {}
              : {
                  sourceUrl: `https://mock.${safeProvider}.example/generated/${safeProvider}-mock-output${index === 0 ? "" : `-${index + 1}`}.${outputExtension}`,
                }),
            width: isVideoOutputScenario ? 1280 : isAudioScenario ? 1 : 1024,
            height: isVideoOutputScenario ? 720 : isAudioScenario ? 1 : 768,
            durationSeconds: generatedDurationSeconds,
            fps: isVideoOutputScenario ? 24 : isAudioScenario ? 1 : 0,
          })),
        settings: options.scenario === "wan-reference-to-video"
          ? {
              durationSeconds: generatedDurationSeconds,
              fps: 24,
              aspectRatio: "16:9",
              resolution: "720p",
              generateAudio: options.generateAudio,
            }
          : isLocalVideoUpscaleScenario(options.scenario)
          ? {
              width: 1280,
              height: 720,
              durationSeconds: generatedDurationSeconds,
              fps: 24,
              aspectRatio: "16:9",
              videoSourceStartSeconds: 0.5,
              videoSourceEndSeconds: 4.5,
            }
          : options.scenario === "local-video-edit-replace" ||
          options.scenario === "local-video-motion-control-replace" ||
          options.scenario === "local-video-edit-retry"
          ? {
              width: 1280,
              height: 720,
              durationSeconds: generatedDurationSeconds,
              fps: 24,
              aspectRatio: "16:9",
              generateAudio: options.generateAudio,
              videoSourceStartSeconds: 0.5,
              videoSourceEndSeconds: 3.5,
            }
          : options.scenario === "local-video-to-music-insert"
            ? {
                ...textToMusicDefaultSettings,
                timelineStartSeconds: timelineAudioInsertConfig(options.scenario).timelineStartSeconds,
                videoSourceStartSeconds: timelineAudioInsertConfig(options.scenario).videoSourceStartSeconds,
                videoSourceEndSeconds: timelineAudioInsertConfig(options.scenario).videoSourceEndSeconds,
              }
          : isTimelineAudioInsert
            ? {
                durationSeconds: generatedDurationSeconds,
                timelineStartSeconds: timelineAudioInsertConfig(options.scenario).timelineStartSeconds,
                videoSourceStartSeconds: timelineAudioInsertConfig(options.scenario).videoSourceStartSeconds,
                videoSourceEndSeconds: timelineAudioInsertConfig(options.scenario).videoSourceEndSeconds,
              }
          : isTextToMusicInsert
            ? {
                ...textToMusicDefaultSettings,
                timelineStartSeconds: timelineTextToMusicInsertConfig().timelineStartSeconds,
                ...(options.provider === "elevenlabs"
                  ? { lyrics: elevenlabsTextToMusicDefaultLyrics }
                  : {}),
                ...(options.provider === "google"
                  ? { lyrics: googleLyriaTextToMusicDefaultLyrics }
                  : {}),
                ...(options.provider === "minimax"
                  ? { lyrics: minimaxTextToMusicDefaultLyrics }
                  : {}),
              }
          : isTimelineVisualInsert
            ? {
                durationSeconds: generatedDurationSeconds,
                timelineStartSeconds: timelineVisualInsertConfig().timelineStartSeconds,
                generateAudio: options.generateAudio,
              }
          : isMultiImageScenario(options.scenario)
            ? {
                numImages: outputCount,
              }
          : isTextToAudio && textToAudioDefaultVoices[options.provider]
            ? {
                category: "tts",
                voice: textToAudioDefaultVoices[options.provider],
              }
          : isVideoToMusic
            ? {
                ...textToMusicDefaultSettings,
              }
          : isTextToMusic
            ? {
                ...textToMusicDefaultSettings,
                ...(options.provider === "elevenlabs"
                  ? { lyrics: elevenlabsTextToMusicDefaultLyrics }
                  : {}),
                ...(options.provider === "google"
                  ? { lyrics: googleLyriaTextToMusicDefaultLyrics }
                  : {}),
                ...(options.provider === "minimax"
                  ? { lyrics: minimaxTextToMusicDefaultLyrics }
                  : {}),
              }
          : isProviderVideoGenerationScenario(options.scenario)
            ? {
                generateAudio: options.generateAudio,
              }
          : undefined,
      },
      null,
      2,
    ),
  );
  if (isRetry) {
    writeFileSync(
      join(projectDir, "generated", failedRetryAssetId, "asset.json"),
      JSON.stringify(
        {
          id: failedRetryAssetId,
          status: "failed",
          model: { provider: options.provider, id: options.model },
          parentAssetId: null,
          retryOfAssetId: null,
          failureReason: "provider_error",
          outputs: [],
        },
        null,
        2,
      ),
    );
    writeFileSync(
      join(projectDir, "jobs", failedRetryAssetId, "job.json"),
      JSON.stringify(
        {
          id: failedRetryAssetId,
          kind: "generate_media",
          status: "failed",
          updatedAt: submittedAt,
          providerRequest: failedRetryProviderRequest,
          terminalReason: "provider_error",
        },
        null,
        2,
      ),
    );
  }
  writeFileSync(
    join(projectDir, "jobs", generatedId, "job.json"),
    JSON.stringify(
      {
        id: generatedId,
        kind: "generate_media",
        status: "completed",
        updatedAt: submittedAt,
        providerRequest,
      },
      null,
      2,
    ),
  );

  return {
    ok: true,
    provider: options.provider,
    model: options.model,
    scenario: options.scenario,
    projectDir,
    artifactPath: outputPath,
    outputCount,
    jobStatus: "Completed",
    providerRequest,
  };
}

function createMockProviderLifecycleResult(options, lifecycleScenario) {
  const safeProvider = options.provider.replace(/\./g, "-");
  const projectDir = options.outDir;
  const generatedId = `provider-e2e-${lifecycleScenario.assetStatus}-${lifecycleScenario.reason}`;
  const submittedAt = new Date(0).toISOString();
  const requestId = `${safeProvider}-mock-${lifecycleScenario.reason}`;
  const providerRequest = {
    provider: options.provider,
    requestId,
    statusUrl: `https://mock.${safeProvider}.example/v1/${requestId}`,
    responseUrl: `https://mock.${safeProvider}.example/v1/${requestId}/response`,
    cancelUrl: `https://mock.${safeProvider}.example/v1/${requestId}/cancel`,
    submittedAt,
  };

  mkdirSync(join(projectDir, "generated", generatedId), { recursive: true });
  mkdirSync(join(projectDir, "media"), { recursive: true });
  mkdirSync(join(projectDir, "jobs", generatedId), { recursive: true });
  writeFileSync(
    join(projectDir, "video-creater.project.json"),
    JSON.stringify(
      {
        schemaVersion: 2,
        id: "provider-e2e-project",
        name: "Provider E2E Mock",
        createdAt: submittedAt,
        updatedAt: submittedAt,
        layout: "split",
        files: {
          media: "media/index.json",
          generated: "generated",
        },
        jobs: [
          {
            id: generatedId,
            kind: "generate_media",
            status: lifecycleScenario.jobSidecarStatus,
            updatedAt: submittedAt,
            providerRequest,
            terminalReason: lifecycleScenario.reason,
          },
        ],
        exportArtifacts: [],
      },
      null,
      2,
    ),
  );
  writeFileSync(
    join(projectDir, "media", "index.json"),
    JSON.stringify(
      {
        schemaVersion: 1,
        folders: [],
        assets: [],
        analysis: [],
      },
      null,
      2,
    ),
  );
  writeFileSync(
    join(projectDir, "generated", "index.json"),
    JSON.stringify(
      {
        schemaVersion: 1,
        assets: [
          {
            assetId: generatedId,
            path: `generated/${generatedId}/asset.json`,
            status: lifecycleScenario.assetStatus,
            modelProvider: options.provider,
            modelId: options.model,
            outputCount: 0,
          },
        ],
      },
      null,
      2,
    ),
  );
  writeFileSync(
    join(projectDir, "generated", generatedId, "asset.json"),
    JSON.stringify(
      {
        id: generatedId,
        status: lifecycleScenario.assetStatus,
        model: { provider: options.provider, id: options.model },
        parentAssetId: null,
        retryOfAssetId: null,
        failureReason: lifecycleScenario.reason,
        references: sourceContextReferencesForScenario(options.provider, options.scenario),
        settings: sourceContextSettingsForScenario(options.scenario, options.generateAudio),
        outputs: [],
      },
      null,
      2,
    ),
  );
  writeFileSync(
    join(projectDir, "jobs", generatedId, "job.json"),
    JSON.stringify(
      {
        id: generatedId,
        kind: "generate_media",
        status: lifecycleScenario.jobSidecarStatus,
        updatedAt: submittedAt,
        providerRequest,
        terminalReason: lifecycleScenario.reason,
      },
      null,
      2,
    ),
  );

  return {
    ok: false,
    provider: options.provider,
    model: options.model,
    scenario: options.scenario,
    projectDir,
    artifactPath: null,
    outputCount: 0,
    jobStatus: lifecycleScenario.jobStatus,
    providerRequest,
    terminalReason: lifecycleScenario.reason,
  };
}

function runMockProvider(options) {
  const childResult = createMockProviderResult(options);
  const error = validateChildResult(childResult, options);
  const ok = error === null;
  const reportPath = join(options.outDir, "provider-e2e-report.json");
  const report = {
    ok,
    provider: options.provider,
    model: options.model,
    scenario: options.scenario,
    outDir: options.outDir,
    reportPath: "provider-e2e-report.json",
    credentialStatus: "not_checked",
    command: null,
    exitCode: 0,
    signal: null,
    mockedProviderService: true,
    result: childResult,
    error,
  };
  const reportJson = JSON.stringify(report, null, 2);
  writeFileSync(reportPath, `${reportJson}\n`);
  console.log(reportJson);
  process.exit(ok ? 0 : 1);
}

function main() {
  const options = parseArgs(process.argv.slice(2));
  if (options.replayFixture) {
    replayFixture(options);
  }

  if (!options.live) {
    runMockProvider(options);
  }

  const credentialEnvVar = providerEnvVars[options.provider];
  const credential = process.env[credentialEnvVar]?.trim() || "";
  if (!credential) {
    console.log(
      JSON.stringify(
        {
          ok: false,
          provider: options.provider,
          model: options.model,
          outDir: options.outDir,
          credentialEnvVar,
          credentialStatus: "missing",
          command: null,
        },
        null,
        2,
      ),
    );
    console.error(`${credentialEnvVar} is required for ${options.provider} provider E2E`);
    process.exit(1);
  }
  if (process.env[liveSpendOptInEnvVar] !== "1") {
    console.log(
      JSON.stringify(
        {
          ok: false,
          provider: options.provider,
          model: options.model,
          scenario: options.scenario,
          outDir: options.outDir,
          credentialEnvVar,
          credentialStatus: "present",
          liveSpendOptInEnvVar,
          liveSpendOptInStatus: "missing",
          command: null,
        },
        null,
        2,
      ),
    );
    console.error(
      `${liveSpendOptInEnvVar}=1 is required for live ${options.provider} provider E2E`,
    );
    process.exit(1);
  }

  const cargoArgs = [
    "run",
    "--manifest-path",
    "src-tauri/crates/provider-e2e-harness/Cargo.toml",
    "--",
    "--provider",
    options.provider,
    "--model",
    options.model,
    "--scenario",
    options.scenario,
    "--out-dir",
    options.outDir,
  ];
  if (options.prompt) {
    cargoArgs.push("--prompt", options.prompt);
  }
  if (options.generateAudio !== true) {
    cargoArgs.push("--generate-audio", String(options.generateAudio));
  }
  if (options.maxStatusPolls) {
    cargoArgs.push("--max-status-polls", options.maxStatusPolls);
  }
  if (options.pollIntervalMs) {
    cargoArgs.push("--poll-interval-ms", options.pollIntervalMs);
  }
  const command = `cargo ${cargoArgs.join(" ")}`;
  const result = spawnSync("cargo", cargoArgs, {
    cwd: process.cwd(),
    encoding: "utf8",
    env: process.env,
    stdio: "pipe",
  });
  const stdout = redactCredential(result.stdout, credential);
  const stderr = redactCredential(result.stderr, credential);
  const childResult = safeJsonParse(stdout);
  const error = validateChildResult(childResult, options);
  const ok = result.status === 0 && childResult !== null && childResult.ok !== false && error === null;
  const reportPath = join(options.outDir, "provider-e2e-report.json");
  const report = {
    ok,
    provider: options.provider,
    model: options.model,
    scenario: options.scenario,
    outDir: options.outDir,
    reportPath: "provider-e2e-report.json",
    credentialEnvVar,
    credentialStatus: "present",
    command,
    exitCode: result.status,
    signal: result.signal,
    result: childResult,
    error,
  };
  const reportJson = JSON.stringify(report, null, 2);
  mkdirSync(options.outDir, { recursive: true });
  writeFileSync(reportPath, `${reportJson}\n`);
  if (ok && options.recordFixture) {
    copyDirectory(options.outDir, options.recordFixture);
  }

  console.log(reportJson);
  if (!ok && stderr) {
    console.error(stderr);
  }
  process.exit(ok ? 0 : 1);
}

function validateChildResult(childResult, options) {
  if (!childResult) {
    return "Provider E2E binary did not emit a JSON report";
  }
  if (childResult.provider !== options.provider) {
    return `Provider E2E binary reported provider ${childResult.provider ?? "unknown"} for requested provider ${options.provider}`;
  }
  if (childResult.model !== options.model) {
    return `Provider E2E binary reported model ${childResult.model ?? "unknown"} for requested model ${options.model}`;
  }
  if (
    typeof childResult.scenario === "string" &&
    childResult.scenario.trim() !== "" &&
    childResult.scenario !== options.scenario
  ) {
    return `Provider E2E binary reported scenario ${childResult.scenario} for requested scenario ${options.scenario}`;
  }
  const lifecycleScenario = providerLifecycleScenario(options.scenario);
  if (lifecycleScenario) {
    return validateProviderLifecycleResult(childResult, options, lifecycleScenario);
  }
  if (childResult.ok === false) {
    return `Provider E2E reported failure for non-lifecycle scenario ${options.scenario}`;
  }
  if (typeof childResult.artifactPath !== "string" || childResult.artifactPath.trim() === "") {
    return "Provider E2E binary did not report an artifact path";
  }
  if (!existsSync(childResult.artifactPath)) {
    return `Provider E2E artifact is missing: ${childResult.artifactPath}`;
  }
  const artifactStat = statSync(childResult.artifactPath);
  if (!artifactStat.isFile() || artifactStat.size < 1) {
    return `Provider E2E artifact is empty or not a file: ${childResult.artifactPath}`;
  }
  if (options.scenario === "replicate-local-file-upload") {
    if (childResult.outputCount !== 0) {
      return "Provider E2E upload-only scenario should not attach generated outputs";
    }
    if (childResult.jobStatus !== "Uploaded") {
      return `Provider E2E upload-only scenario did not report Uploaded status: ${childResult.jobStatus ?? "unknown"}`;
    }
    if (!isProviderSourceUrl(childResult.providerFileUrl)) {
      return "Provider E2E upload-only scenario did not report a provider file URL";
    }
    return null;
  }
  if (!Number.isInteger(childResult.outputCount) || childResult.outputCount < 1) {
    return "Provider E2E did not attach any generated outputs";
  }
  if (childResult.jobStatus !== "Completed") {
    return `Provider E2E job did not complete: ${childResult.jobStatus ?? "unknown"}`;
  }
  if (typeof childResult.projectDir !== "string" || childResult.projectDir.trim() === "") {
    return "Provider E2E binary did not report a project directory";
  }
  const manifestPath = join(childResult.projectDir, "video-creater.project.json");
  if (!existsSync(manifestPath)) {
    return `Provider E2E split project manifest is missing: ${manifestPath}`;
  }
  const manifest = safeJsonParse(readFileSync(manifestPath, "utf8"));
  if (!manifest) {
    return `Provider E2E split project manifest is not valid JSON: ${manifestPath}`;
  }
  const generatedDir = manifest.files?.generated;
  if (typeof generatedDir !== "string" || generatedDir.trim() === "") {
    return `Provider E2E split project manifest does not declare a generated asset directory: ${manifestPath}`;
  }
  const generatedRootPath = join(childResult.projectDir, generatedDir);
  const generatedIndexPath = join(generatedRootPath, "index.json");
  if (!isWithinDirectory(childResult.projectDir, generatedIndexPath)) {
    return `Provider E2E generated asset index path escapes the split project: ${generatedDir}`;
  }
  if (!existsSync(generatedIndexPath)) {
    return `Provider E2E generated asset index is missing: ${generatedIndexPath}`;
  }
  const generatedIndex = safeJsonParse(readFileSync(generatedIndexPath, "utf8"));
  if (!generatedIndex) {
    return `Provider E2E generated asset index is not valid JSON: ${generatedIndexPath}`;
  }
  const outputBackedAssets = Array.isArray(generatedIndex.assets)
    ? generatedIndex.assets.filter(
        (asset) => Number.isInteger(asset?.outputCount) && asset.outputCount > 0,
      )
    : [];
  if (outputBackedAssets.length === 0) {
    return `Provider E2E generated asset index has no output-backed assets: ${generatedIndexPath}`;
  }
  let artifactIsRecordedOutput = false;
  let sidecarOutputCount = 0;
  const sidecarOutputs = [];
  const completedAssetIds = [];
  const generatedAssetIds = new Set();
  const sidecarOutputMediaIds = new Set();
  for (const asset of outputBackedAssets) {
    if (
      typeof asset.assetId === "string" &&
      asset.assetId.trim() !== "" &&
      generatedAssetIds.has(asset.assetId)
    ) {
      return `Provider E2E generated asset index has duplicate assetId: ${asset.assetId}`;
    }
    if (typeof asset.assetId === "string" && asset.assetId.trim() !== "") {
      generatedAssetIds.add(asset.assetId);
    }
    if (typeof asset.path !== "string" || asset.path.trim() === "") {
      return `Provider E2E generated asset index entry is missing a sidecar path: ${generatedIndexPath}`;
    }
    const sidecarPath = join(childResult.projectDir, asset.path);
    if (!isWithinDirectory(childResult.projectDir, sidecarPath)) {
      return `Provider E2E generated asset sidecar path escapes the split project: ${asset.path}`;
    }
    if (!isWithinDirectory(generatedRootPath, sidecarPath)) {
      return `Provider E2E generated asset sidecar path is outside the generated asset directory: ${asset.path}`;
    }
    if (!existsSync(sidecarPath)) {
      return `Provider E2E generated asset sidecar is missing: ${sidecarPath}`;
    }
    const sidecar = safeJsonParse(readFileSync(sidecarPath, "utf8"));
    if (!sidecar) {
      return `Provider E2E generated asset sidecar is not valid JSON: ${sidecarPath}`;
    }
    if (sidecar.id !== asset.assetId) {
      return `Provider E2E generated asset index asset id does not match sidecar id: ${sidecarPath}`;
    }
    if (asset.modelProvider !== options.provider) {
      return `Provider E2E generated asset index provider does not match requested provider: ${generatedIndexPath}`;
    }
    if (asset.modelId !== options.model) {
      return `Provider E2E generated asset index model does not match requested model: ${generatedIndexPath}`;
    }
    if (sidecar.model?.provider !== options.provider) {
      return `Provider E2E generated asset sidecar provider does not match requested provider: ${sidecarPath}`;
    }
    if (sidecar.model?.id !== options.model) {
      return `Provider E2E generated asset sidecar model does not match requested model: ${sidecarPath}`;
    }
    if (asset.status !== "completed") {
      return `Provider E2E generated asset index status does not match completed sidecar: ${sidecarPath}`;
    }
    if (sidecar.status !== "completed") {
      return `Provider E2E generated asset sidecar is not completed: ${sidecarPath}`;
    }
    completedAssetIds.push(sidecar.id);
    if (!Array.isArray(sidecar.outputs) || sidecar.outputs.length === 0) {
      return `Provider E2E generated asset sidecar has no outputs: ${sidecarPath}`;
    }
    if (
      options.scenario === "wan-image-to-video" ||
      options.scenario === "wan-reference-to-video" ||
      options.scenario === "kling-image-to-video" ||
      options.scenario === "replicate-video"
    ) {
      const refs = sidecar.references && typeof sidecar.references === "object"
        ? sidecar.references
        : null;
      if (!refs) {
        return `Provider E2E video sidecar is missing typed references: ${sidecarPath}`;
      }
      if (options.scenario === "wan-reference-to-video") {
        if (
          !Array.isArray(refs.referenceImageMediaRefs) ||
          refs.referenceImageMediaRefs.length !== 1
        ) {
          return `Provider E2E video sidecar must retain one referenceImageMediaRefs value: ${sidecarPath}`;
        }
        if (
          !Array.isArray(refs.referenceVideoMediaRefs) ||
          refs.referenceVideoMediaRefs.length !== 1
        ) {
          return `Provider E2E video sidecar must retain one referenceVideoMediaRefs value: ${sidecarPath}`;
        }
      } else {
        if (typeof refs.firstFrameMediaId !== "string" || refs.firstFrameMediaId.trim() === "") {
          return `Provider E2E video sidecar is missing firstFrameMediaId: ${sidecarPath}`;
        }
        if (typeof refs.lastFrameMediaId !== "string" || refs.lastFrameMediaId.trim() === "") {
          return `Provider E2E video sidecar is missing lastFrameMediaId: ${sidecarPath}`;
        }
        if (
          options.scenario !== "kling-image-to-video" &&
          (!Array.isArray(refs.referenceAudioMediaRefs) || refs.referenceAudioMediaRefs.length === 0)
        ) {
          return `Provider E2E video sidecar is missing referenceAudioMediaRefs: ${sidecarPath}`;
        }
      }
      const expectedProviderInputUrlCount = options.scenario === "wan-reference-to-video" ||
        options.scenario === "kling-image-to-video"
        ? 2
        : 3;
      if (
        !Array.isArray(refs.providerInputUrls) ||
        refs.providerInputUrls.length !== expectedProviderInputUrlCount
      ) {
        return `Provider E2E video sidecar must retain ${expectedProviderInputUrlCount} provider input URLs: ${sidecarPath}`;
      }
      for (const url of refs.providerInputUrls) {
        if (!isProviderSourceUrl(url)) {
          return `Provider E2E video provider input URL is not a provider URL: ${sidecarPath}`;
        }
      }
      if (options.scenario === "wan-reference-to-video") {
        const settings = sidecar.settings && typeof sidecar.settings === "object"
          ? sidecar.settings
          : null;
        if (
          settings?.durationSeconds !== 4 ||
          settings?.aspectRatio !== "16:9" ||
          settings?.resolution !== "720p" ||
          typeof settings?.generateAudio !== "boolean"
        ) {
          return `Provider E2E WAN reference-to-video sidecar must retain duration, aspect, resolution, and generateAudio settings: ${sidecarPath}`;
        }
      }
    }
    if (options.scenario === "google-video") {
      const refs = sidecar.references && typeof sidecar.references === "object"
        ? sidecar.references
        : null;
      if (!refs) {
        return `Provider E2E Google video sidecar is missing typed references: ${sidecarPath}`;
      }
      if (typeof refs.firstFrameMediaId !== "string" || refs.firstFrameMediaId.trim() === "") {
        return `Provider E2E Google video sidecar is missing firstFrameMediaId: ${sidecarPath}`;
      }
      if (typeof refs.lastFrameMediaId !== "string" || refs.lastFrameMediaId.trim() === "") {
        return `Provider E2E Google video sidecar is missing lastFrameMediaId: ${sidecarPath}`;
      }
      if (!Array.isArray(refs.referenceImageMediaRefs) || refs.referenceImageMediaRefs.length !== 1) {
        return `Provider E2E Google video sidecar must retain one referenceImageMediaRefs value: ${sidecarPath}`;
      }
      if (!Array.isArray(refs.providerInputUrls) || refs.providerInputUrls.length !== 3) {
        return `Provider E2E Google video sidecar must retain three image provider input data URLs: ${sidecarPath}`;
      }
      if (
        !refs.providerInputUrls.every((url) =>
          /^data:image\/(png|jpeg|webp);base64,[A-Za-z0-9+/=]+$/.test(url),
        )
      ) {
        return `Provider E2E Google video provider inputs must be image data URLs: ${sidecarPath}`;
      }
    }
    if (options.scenario === "xai-video") {
      const refs = sidecar.references && typeof sidecar.references === "object"
        ? sidecar.references
        : null;
      if (!refs) {
        return `Provider E2E xAI Grok video sidecar is missing typed references: ${sidecarPath}`;
      }
      if (!Array.isArray(refs.referenceImageMediaRefs) || refs.referenceImageMediaRefs.length !== 1) {
        return `Provider E2E xAI Grok video sidecar must retain one referenceImageMediaRefs value: ${sidecarPath}`;
      }
      if (!Array.isArray(refs.providerInputUrls) || refs.providerInputUrls.length !== 1) {
        return `Provider E2E xAI Grok video sidecar must retain one provider input URL: ${sidecarPath}`;
      }
      if (!isProviderSourceUrl(refs.providerInputUrls[0])) {
        return `Provider E2E xAI Grok video provider input URL is not a provider URL: ${sidecarPath}`;
      }
    }
    if (options.scenario === "local-video-motion-control-replace") {
      const refs = sidecar.references && typeof sidecar.references === "object"
        ? sidecar.references
        : null;
      if (!refs) {
        return `Provider E2E Motion Control sidecar is missing typed references: ${sidecarPath}`;
      }
      if (
        typeof refs.sourceVideoMediaRef !== "string" ||
        refs.sourceVideoMediaRef.trim() === ""
      ) {
        return `Provider E2E Motion Control sidecar is missing sourceVideoMediaRef: ${sidecarPath}`;
      }
      if (
        !Array.isArray(refs.referenceImageMediaRefs) ||
        refs.referenceImageMediaRefs.length !== 1
      ) {
        return `Provider E2E Motion Control sidecar must retain one referenceImageMediaRefs value: ${sidecarPath}`;
      }
      if (
        !Array.isArray(refs.providerInputUrls) ||
        refs.providerInputUrls.length !== 2
      ) {
        return `Provider E2E Motion Control sidecar must retain source-video and image provider input URLs: ${sidecarPath}`;
      }
      if (!refs.providerInputUrls.every((url) => isProviderSourceUrl(url))) {
        return `Provider E2E Motion Control provider input URLs are not provider URLs: ${sidecarPath}`;
      }
    } else if (
      isVideoToAudioScenario(options.scenario) ||
      isSourceVideoGenerationScenario(options.scenario)
    ) {
      const scenarioLabel = isVideoToAudioScenario(options.scenario)
        ? "video-to-audio"
        : "video-to-video";
      const refs = sidecar.references && typeof sidecar.references === "object"
        ? sidecar.references
        : null;
      if (!refs) {
        return `Provider E2E ${scenarioLabel} sidecar is missing typed references: ${sidecarPath}`;
      }
      if (
        typeof refs.sourceVideoMediaRef !== "string" ||
        refs.sourceVideoMediaRef.trim() === ""
      ) {
        return `Provider E2E ${scenarioLabel} sidecar is missing sourceVideoMediaRef: ${sidecarPath}`;
      }
      if (
        isVideoToAudioScenario(options.scenario) &&
        (!Array.isArray(refs.referenceVideoMediaRefs) ||
          refs.referenceVideoMediaRefs.length !== 1 ||
          refs.referenceVideoMediaRefs[0] !== refs.sourceVideoMediaRef)
      ) {
        return `Provider E2E video-to-audio sidecar must retain one referenceVideoMediaRefs value: ${sidecarPath}`;
      }
      if (
        !Array.isArray(refs.providerInputUrls) ||
        refs.providerInputUrls.length !== 1
      ) {
        return `Provider E2E ${scenarioLabel} sidecar must retain one provider input URL: ${sidecarPath}`;
      }
      if (!isProviderSourceUrl(refs.providerInputUrls[0])) {
        return `Provider E2E ${scenarioLabel} provider input URL is not a provider URL: ${sidecarPath}`;
      }
      if (
        isLocalVideoUpscaleScenario(options.scenario) &&
        (sidecar.settings?.videoSourceStartSeconds !== 0.5 ||
          sidecar.settings?.videoSourceEndSeconds !== 4.5)
      ) {
        return `Provider E2E ${scenarioLabel} sidecar must retain selected source trim settings: ${sidecarPath}`;
      }
    }
    if (isImageEditScenario(options.scenario)) {
      const expectedImageEditReferences = imageEditReferencesForProvider(options.provider);
      const refs = sidecar.references && typeof sidecar.references === "object"
        ? sidecar.references
        : null;
      if (!refs) {
        return `Provider E2E image-edit sidecar is missing typed references: ${sidecarPath}`;
      }
      if (
        !Array.isArray(refs.referenceImageMediaRefs) ||
        refs.referenceImageMediaRefs.length !== expectedImageEditReferences.referenceImageMediaRefs.length
      ) {
        return `Provider E2E image-edit sidecar must retain ${expectedImageEditReferences.referenceImageMediaRefs.length} referenceImageMediaRefs value(s): ${sidecarPath}`;
      }
      if (
        !Array.isArray(refs.providerInputUrls) ||
        refs.providerInputUrls.length !== expectedImageEditReferences.providerInputUrls.length
      ) {
        return `Provider E2E image-edit sidecar must retain ${expectedImageEditReferences.providerInputUrls.length} provider input URL(s): ${sidecarPath}`;
      }
      const imageEditProviderInputValid =
        options.provider === "openai" || options.provider === "xai"
          ? refs.providerInputUrls.every((url) =>
              /^data:image\/(png|jpeg|webp);base64,[A-Za-z0-9+/=]+$/.test(url),
            )
          : refs.providerInputUrls.every((url) => isProviderSourceUrl(url));
      if (!imageEditProviderInputValid) {
        return `Provider E2E image-edit provider input URL is not a provider URL: ${sidecarPath}`;
      }
    }
    if (isLocalImageUpscaleScenario(options.scenario)) {
      const refs = sidecar.references && typeof sidecar.references === "object"
        ? sidecar.references
        : null;
      if (!refs) {
        return `Provider E2E image-upscale sidecar is missing typed references: ${sidecarPath}`;
      }
      if (!Array.isArray(refs.mediaIds) || refs.mediaIds.length !== 1) {
        return `Provider E2E image-upscale sidecar must retain one mediaIds value: ${sidecarPath}`;
      }
      if (
        !Array.isArray(refs.providerInputUrls) ||
        refs.providerInputUrls.length !== 1
      ) {
        return `Provider E2E image-upscale sidecar must retain one provider input URL: ${sidecarPath}`;
      }
      if (!isProviderSourceUrl(refs.providerInputUrls[0])) {
        return `Provider E2E image-upscale provider input URL is not a provider URL: ${sidecarPath}`;
      }
    }
    if (isTimelineAudioInsertScenario(options.scenario)) {
      const insertConfig = timelineAudioInsertConfig(options.scenario);
      const expectedPlacementIntent = insertConfig.placementIntent;
      if (sidecar.placementIntent !== expectedPlacementIntent) {
        return `Provider E2E audio insert sidecar must retain placementIntent: ${sidecarPath}`;
      }
      if (
        sidecar.settings?.timelineStartSeconds !== insertConfig.timelineStartSeconds ||
        sidecar.settings?.videoSourceStartSeconds !== insertConfig.videoSourceStartSeconds ||
        sidecar.settings?.videoSourceEndSeconds !== insertConfig.videoSourceEndSeconds
      ) {
        return `Provider E2E audio insert sidecar must retain selected source trim settings: ${sidecarPath}`;
      }
    }
    if (isTextToMusicInsertScenario(options.scenario)) {
      const insertConfig = timelineTextToMusicInsertConfig();
      if (sidecar.placementIntent !== insertConfig.placementIntent) {
        return `Provider E2E text-to-music insert sidecar must retain placementIntent: ${sidecarPath}`;
      }
      if (sidecar.settings?.timelineStartSeconds !== insertConfig.timelineStartSeconds) {
        return `Provider E2E text-to-music insert sidecar must retain timelineStartSeconds: ${sidecarPath}`;
      }
    }
    if (asset.outputCount !== sidecar.outputs.length) {
      return `Provider E2E generated asset index output count does not match sidecar outputs: ${sidecarPath}`;
    }
    sidecarOutputCount += sidecar.outputs.length;
    for (const output of sidecar.outputs) {
      if (typeof output?.mediaId !== "string" || output.mediaId.trim() === "") {
        return `Provider E2E generated asset sidecar output is missing a mediaId: ${sidecarPath}`;
      }
      if (typeof output?.relativePath !== "string" || output.relativePath.trim() === "") {
        return `Provider E2E generated asset sidecar output is missing a relativePath: ${sidecarPath}`;
      }
      const outputPath = join(childResult.projectDir, output.relativePath);
      if (!isWithinDirectory(childResult.projectDir, outputPath)) {
        return `Provider E2E generated asset sidecar output path escapes the split project: ${output.relativePath}`;
      }
      if (!isWithinDirectory(generatedRootPath, outputPath)) {
        return `Provider E2E generated asset sidecar output is outside the generated asset directory: ${output.relativePath}`;
      }
      if (!existsSync(outputPath)) {
        return `Provider E2E generated asset sidecar output file is missing: ${outputPath}`;
      }
      const outputStat = statSync(outputPath);
      if (!outputStat.isFile() || outputStat.size < 1) {
        return `Provider E2E generated asset sidecar output file is empty or not a file: ${outputPath}`;
      }
      if (
        options.provider !== "openai" &&
        options.provider !== "elevenlabs" &&
        !(
          (options.provider === "google" &&
            (options.scenario === "text-to-audio" ||
              options.scenario === "text-to-music" ||
              isTextToMusicInsertScenario(options.scenario))) ||
          (options.provider === "elevenlabs" && isTextToMusicInsertScenario(options.scenario)) ||
          (options.provider === "fal.ai" && options.scenario === "text-to-music") ||
          (options.provider === "fal.ai" && isTextToMusicInsertScenario(options.scenario)) ||
          (options.provider === "minimax" && options.scenario === "text-to-music") ||
          (options.provider === "minimax" && isTextToMusicInsertScenario(options.scenario))
        ) &&
        (typeof output.sourceUrl !== "string" || output.sourceUrl.trim() === "")
      ) {
        return `Provider E2E generated asset sidecar output is missing a provider sourceUrl: ${sidecarPath}`;
      }
      if (
        typeof output.sourceUrl === "string" &&
        output.sourceUrl.trim() !== "" &&
        !isProviderSourceUrl(output.sourceUrl)
      ) {
        return `Provider E2E generated asset sidecar output sourceUrl is not a provider URL: ${sidecarPath}`;
      }
      if (
        !Number.isFinite(output.width) ||
        output.width < 1 ||
        !Number.isFinite(output.height) ||
        output.height < 1 ||
        !Number.isFinite(output.durationSeconds) ||
        output.durationSeconds < 0 ||
        !Number.isFinite(output.fps) ||
        output.fps < 0
      ) {
        return `Provider E2E generated asset sidecar output is missing media metadata: ${sidecarPath}`;
      }
      if (outputPath === childResult.artifactPath) {
        artifactIsRecordedOutput = true;
      }
      if (isProviderVideoGenerationScenario(options.scenario)) {
        if (!output.relativePath.endsWith(".mp4")) {
          return `Provider E2E video output is not an MP4 artifact: ${sidecarPath}`;
        }
        if (output.durationSeconds <= 0 || output.fps <= 0) {
          return `Provider E2E video output is missing video timing metadata: ${sidecarPath}`;
        }
      }
      if (options.scenario === "text-to-audio") {
        const expectedAudioExtension = options.provider === "google" ? ".wav" : ".mp3";
        if (!output.relativePath.endsWith(expectedAudioExtension)) {
          return `Provider E2E text-to-audio output is not a ${expectedAudioExtension.slice(1).toUpperCase()} artifact: ${sidecarPath}`;
        }
        if (output.durationSeconds <= 0) {
          return `Provider E2E text-to-audio output is missing audio duration metadata: ${sidecarPath}`;
        }
      }
      if (options.scenario === "text-to-music" || isTextToMusicInsertScenario(options.scenario)) {
        const expectedMusicExtension = options.provider === "fal.ai" ? ".m4a" : ".mp3";
        const expectedMusicLabel = expectedMusicExtension.slice(1).toUpperCase();
        if (!output.relativePath.endsWith(expectedMusicExtension)) {
          return `Provider E2E text-to-music output is not a ${expectedMusicLabel} artifact: ${sidecarPath}`;
        }
        if (output.durationSeconds <= 0) {
          return `Provider E2E text-to-music output is missing audio duration metadata: ${sidecarPath}`;
        }
      }
      if (isVideoToAudioScenario(options.scenario)) {
        const expectedExtension =
          options.scenario === "video-to-music" ||
          options.scenario === "local-video-to-music-insert"
            ? ".m4a"
            : ".wav";
        if (!output.relativePath.endsWith(expectedExtension)) {
          return `Provider E2E video-to-audio output is not a ${expectedExtension} artifact: ${sidecarPath}`;
        }
        if (output.durationSeconds <= 0) {
          return `Provider E2E video-to-audio output is missing audio duration metadata: ${sidecarPath}`;
        }
      }
      if (isImageEditScenario(options.scenario)) {
        if (!output.relativePath.endsWith(".png")) {
          return `Provider E2E image-edit output is not a PNG artifact: ${sidecarPath}`;
        }
        if (output.width <= 0 || output.height <= 0) {
          return `Provider E2E image-edit output is missing image dimensions: ${sidecarPath}`;
        }
      }
      if (isLocalImageUpscaleScenario(options.scenario)) {
        if (!output.relativePath.endsWith(".png")) {
          return `Provider E2E image-upscale output is not a PNG artifact: ${sidecarPath}`;
        }
        if (output.width <= 0 || output.height <= 0) {
          return `Provider E2E image-upscale output is missing image dimensions: ${sidecarPath}`;
        }
      }
      if (isReplicateFluxVariantScenario(options.scenario)) {
        if (!output.relativePath.endsWith(".png")) {
          return `Provider E2E Replicate Flux output is not a PNG artifact: ${sidecarPath}`;
        }
        if (output.width <= 0 || output.height <= 0) {
          return `Provider E2E Replicate Flux output is missing image dimensions: ${sidecarPath}`;
        }
      }
      if (isReplacementScenario(options.scenario)) {
        const replacementError = validateReplacementTimelineEvidence(
          childResult.projectDir,
          output.mediaId,
          replacementScenarioTargetItemId(options.scenario),
          replacementScenarioLinkedItemId(options.scenario),
        );
        if (replacementError) {
          return replacementError;
        }
      }
      if (isTimelineAudioInsertScenario(options.scenario)) {
        const insertError = validateTimelineAudioInsertEvidence(
          childResult.projectDir,
          output.mediaId,
          output.durationSeconds,
          options.scenario,
        );
        if (insertError) {
          return insertError;
        }
      }
      if (isTextToMusicInsertScenario(options.scenario)) {
        const insertError = validateTimelineTextToMusicInsertEvidence(
          childResult.projectDir,
          output.mediaId,
          output.durationSeconds,
        );
        if (insertError) {
          return insertError;
        }
      }
      if (isTimelineVisualInsertScenario(options.scenario)) {
        const insertError = validateTimelineVisualInsertEvidence(
          childResult.projectDir,
          output.mediaId,
          output.durationSeconds,
        );
        if (insertError) {
          return insertError;
        }
      }
      if (sidecarOutputMediaIds.has(output.mediaId)) {
        return `Provider E2E generated sidecar output mediaId is duplicated across completed sidecars: ${output.mediaId}`;
      }
      sidecarOutputMediaIds.add(output.mediaId);
      sidecarOutputs.push(output);
    }
  }
  if (!artifactIsRecordedOutput) {
    return `Provider E2E artifact is not recorded in generated asset sidecar outputs: ${childResult.artifactPath}`;
  }
  if (childResult.outputCount !== sidecarOutputCount) {
    return "Provider E2E report output count does not match generated sidecar outputs";
  }
  const mediaIndexPathValue = manifest.files?.media;
  if (typeof mediaIndexPathValue !== "string" || mediaIndexPathValue.trim() === "") {
    return `Provider E2E split project manifest does not declare a media index: ${manifestPath}`;
  }
  const mediaIndexPath = join(childResult.projectDir, mediaIndexPathValue);
  if (!isWithinDirectory(childResult.projectDir, mediaIndexPath)) {
    return `Provider E2E media index path escapes the split project: ${mediaIndexPathValue}`;
  }
  if (!existsSync(mediaIndexPath)) {
    return `Provider E2E media index is missing: ${mediaIndexPath}`;
  }
  const mediaIndex = safeJsonParse(readFileSync(mediaIndexPath, "utf8"));
  if (!mediaIndex) {
    return `Provider E2E media index is not valid JSON: ${mediaIndexPath}`;
  }
  const mediaAssets = Array.isArray(mediaIndex.assets) ? mediaIndex.assets : [];
  const mediaAssetById = new Map();
  for (const asset of mediaAssets) {
    if (typeof asset?.id !== "string" || asset.id.trim() === "") {
      continue;
    }
    if (mediaAssetById.has(asset.id)) {
      return `Provider E2E media index has duplicate generated output id: ${asset.id}`;
    }
    mediaAssetById.set(asset.id, asset);
  }
  for (const asset of mediaAssetById.values()) {
    if (asset.kind === "generated" && !sidecarOutputMediaIds.has(asset.id)) {
      return `Provider E2E generated media is not backed by a completed sidecar output: ${asset.id}`;
    }
  }
  for (const output of sidecarOutputs) {
    const mediaAsset = mediaAssetById.get(output.mediaId);
    if (!mediaAsset) {
      return `Provider E2E generated sidecar output is missing from media index: ${output.mediaId}`;
    }
    if (mediaAsset.kind !== "generated") {
      return `Provider E2E generated sidecar output media is not marked generated: ${output.mediaId}`;
    }
    if (mediaAsset.relativePath !== output.relativePath) {
      return `Provider E2E generated sidecar output path does not match media index: ${output.mediaId}`;
    }
    if (
      mediaAsset.width !== output.width ||
      mediaAsset.height !== output.height ||
      mediaAsset.durationSeconds !== output.durationSeconds ||
      mediaAsset.fps !== output.fps
    ) {
      return `Provider E2E generated sidecar output metadata does not match media index: ${output.mediaId}`;
    }
  }
  const providerRequestValidationError = providerRequestError(
    childResult.providerRequest,
    options,
  );
  if (providerRequestValidationError) {
    return providerRequestValidationError;
  }
  let matchingJobRequestCount = 0;
  for (const assetId of completedAssetIds) {
    const jobPath = join(childResult.projectDir, "jobs", assetId, "job.json");
    if (!isWithinDirectory(childResult.projectDir, jobPath) || !existsSync(jobPath)) {
      continue;
    }
    const job = safeJsonParse(readFileSync(jobPath, "utf8"));
    if (!job) {
      continue;
    }
    if (!providerRequestsEqual(job.providerRequest ?? {}, childResult.providerRequest)) {
      continue;
    }
    if (job.status !== "completed") {
      return `Provider E2E split project job is not completed: ${assetId}`;
    }
    matchingJobRequestCount += 1;
  }
  if (matchingJobRequestCount !== 1) {
    return "Provider E2E split project job provider request does not match child report";
  }
  if (isProviderRetryScenario(options.scenario)) {
    const retryError = validateProviderRetryEvidence(
      childResult.projectDir,
      options,
      childResult.providerRequest,
    );
    if (retryError) {
      return retryError;
    }
  }
  return null;
}

function validateProviderLifecycleResult(childResult, options, lifecycleScenario) {
  if (childResult.ok !== false) {
    return `Provider E2E ${options.scenario} result must preserve a failed child outcome`;
  }
  if (childResult.outputCount !== 0) {
    return `Provider E2E ${options.scenario} result must not attach generated outputs`;
  }
  if (childResult.artifactPath !== null) {
    return `Provider E2E ${options.scenario} result must not report an artifact path`;
  }
  if (childResult.jobStatus !== lifecycleScenario.jobStatus) {
    return `Provider E2E ${options.scenario} job status must be ${lifecycleScenario.jobStatus}`;
  }
  if (childResult.terminalReason !== lifecycleScenario.reason) {
    return `Provider E2E ${options.scenario} terminal reason must be ${lifecycleScenario.reason}`;
  }
  if (typeof childResult.projectDir !== "string" || childResult.projectDir.trim() === "") {
    return `Provider E2E ${options.scenario} did not report a project directory`;
  }
  const providerRequestValidationError = providerRequestError(
    childResult.providerRequest,
    options,
  );
  if (providerRequestValidationError) {
    return providerRequestValidationError;
  }
  const manifestPath = join(childResult.projectDir, "video-creater.project.json");
  if (!existsSync(manifestPath)) {
    return `Provider E2E ${options.scenario} split project manifest is missing: ${manifestPath}`;
  }
  const manifest = safeJsonParse(readFileSync(manifestPath, "utf8"));
  if (!manifest) {
    return `Provider E2E ${options.scenario} split project manifest is not valid JSON: ${manifestPath}`;
  }
  const generatedDir = manifest.files?.generated;
  if (typeof generatedDir !== "string" || generatedDir.trim() === "") {
    return `Provider E2E ${options.scenario} manifest does not declare a generated asset directory`;
  }
  const generatedRootPath = join(childResult.projectDir, generatedDir);
  const generatedIndexPath = join(generatedRootPath, "index.json");
  if (!isWithinDirectory(childResult.projectDir, generatedIndexPath) || !existsSync(generatedIndexPath)) {
    return `Provider E2E ${options.scenario} generated asset index is missing or escapes project`;
  }
  const generatedIndex = safeJsonParse(readFileSync(generatedIndexPath, "utf8"));
  if (!generatedIndex) {
    return `Provider E2E ${options.scenario} generated asset index is not valid JSON`;
  }
  const terminalAssets = Array.isArray(generatedIndex.assets)
    ? generatedIndex.assets.filter((asset) => asset.status === lifecycleScenario.assetStatus)
    : [];
  if (terminalAssets.length !== 1) {
    return `Provider E2E ${options.scenario} must retain exactly one terminal generated asset`;
  }
  const asset = terminalAssets[0];
  if (asset.modelProvider !== options.provider || asset.modelId !== options.model) {
    return `Provider E2E ${options.scenario} terminal asset model metadata does not match`;
  }
  if (asset.outputCount !== 0) {
    return `Provider E2E ${options.scenario} terminal asset outputCount must be 0`;
  }
  const sidecarPath = join(childResult.projectDir, asset.path || "");
  if (!isWithinDirectory(generatedRootPath, sidecarPath) || !existsSync(sidecarPath)) {
    return `Provider E2E ${options.scenario} terminal generated sidecar is missing or outside generated directory`;
  }
  const sidecar = safeJsonParse(readFileSync(sidecarPath, "utf8"));
  if (!sidecar) {
    return `Provider E2E ${options.scenario} terminal generated sidecar is not valid JSON`;
  }
  if (sidecar.id !== asset.assetId || sidecar.status !== lifecycleScenario.assetStatus) {
    return `Provider E2E ${options.scenario} terminal generated sidecar does not match the index`;
  }
  if (sidecar.model?.provider !== options.provider || sidecar.model?.id !== options.model) {
    return `Provider E2E ${options.scenario} terminal generated sidecar model does not match`;
  }
  if (!Array.isArray(sidecar.outputs) || sidecar.outputs.length !== 0) {
    return `Provider E2E ${options.scenario} terminal generated sidecar must not retain outputs`;
  }
  if (sidecar.failureReason !== lifecycleScenario.reason) {
    return `Provider E2E ${options.scenario} terminal generated sidecar must retain failureReason`;
  }
  if (isSourceVideoGenerationScenario(options.scenario)) {
    const refs = sidecar.references && typeof sidecar.references === "object"
      ? sidecar.references
      : null;
    if (!refs) {
      return `Provider E2E ${options.scenario} sidecar is missing source-video references`;
    }
    if (
      typeof refs.sourceVideoMediaRef !== "string" ||
      refs.sourceVideoMediaRef.trim() === ""
    ) {
      return `Provider E2E ${options.scenario} sidecar is missing sourceVideoMediaRef`;
    }
    if (options.scenario === "local-video-motion-control-replace") {
      if (
        !Array.isArray(refs.referenceImageMediaRefs) ||
        refs.referenceImageMediaRefs.length !== 1
      ) {
        return `Provider E2E ${options.scenario} sidecar must retain one referenceImageMediaRefs value`;
      }
      if (!Array.isArray(refs.providerInputUrls) || refs.providerInputUrls.length !== 2) {
        return `Provider E2E ${options.scenario} sidecar must retain source-video and image provider input URLs`;
      }
    } else {
      if (!Array.isArray(refs.referenceVideoMediaRefs) || refs.referenceVideoMediaRefs.length !== 1) {
        return `Provider E2E ${options.scenario} sidecar must retain one referenceVideoMediaRefs value`;
      }
      if (!Array.isArray(refs.providerInputUrls) || refs.providerInputUrls.length !== 1) {
        return `Provider E2E ${options.scenario} sidecar must retain one provider input URL`;
      }
    }
    if (!refs.providerInputUrls.every((url) => isProviderSourceUrl(url))) {
      return `Provider E2E ${options.scenario} provider input URL is not a provider URL`;
    }
    if (
      isLocalVideoUpscaleScenario(options.scenario) &&
      (sidecar.settings?.videoSourceStartSeconds !== 0.5 ||
        sidecar.settings?.videoSourceEndSeconds !== 4.5)
    ) {
      return `Provider E2E ${options.scenario} sidecar must retain selected source trim settings`;
    }
    if (
      (options.scenario === "local-video-edit-cancellation" ||
        options.scenario === "local-video-edit-failure") &&
      (sidecar.settings?.videoSourceStartSeconds !== 0.5 ||
        sidecar.settings?.videoSourceEndSeconds !== 3.5)
    ) {
      return `Provider E2E ${options.scenario} sidecar must retain selected source trim settings`;
    }
  }
  if (isImageEditScenario(options.scenario)) {
    const refs = sidecar.references && typeof sidecar.references === "object"
      ? sidecar.references
      : null;
    if (!refs) {
      return `Provider E2E ${options.scenario} sidecar is missing image-edit references`;
    }
    if (!Array.isArray(refs.referenceImageMediaRefs) || refs.referenceImageMediaRefs.length !== 1) {
      return `Provider E2E ${options.scenario} sidecar must retain one referenceImageMediaRefs value`;
    }
    if (!Array.isArray(refs.providerInputUrls) || refs.providerInputUrls.length !== 1) {
      return `Provider E2E ${options.scenario} sidecar must retain one provider input URL`;
    }
    if (!isProviderSourceUrl(refs.providerInputUrls[0])) {
      return `Provider E2E ${options.scenario} provider input URL is not a provider URL`;
    }
  }
  if (isLocalImageUpscaleScenario(options.scenario)) {
    const refs = sidecar.references && typeof sidecar.references === "object"
      ? sidecar.references
      : null;
    if (!refs) {
      return `Provider E2E ${options.scenario} sidecar is missing image-upscale references`;
    }
    if (!Array.isArray(refs.mediaIds) || refs.mediaIds.length !== 1) {
      return `Provider E2E ${options.scenario} sidecar must retain one mediaIds value`;
    }
    if (!Array.isArray(refs.providerInputUrls) || refs.providerInputUrls.length !== 1) {
      return `Provider E2E ${options.scenario} sidecar must retain one provider input URL`;
    }
    if (!isProviderSourceUrl(refs.providerInputUrls[0])) {
      return `Provider E2E ${options.scenario} provider input URL is not a provider URL`;
    }
  }
  const jobPath = join(childResult.projectDir, "jobs", asset.assetId, "job.json");
  if (!isWithinDirectory(childResult.projectDir, jobPath) || !existsSync(jobPath)) {
    return `Provider E2E ${options.scenario} job sidecar is missing`;
  }
  const job = safeJsonParse(readFileSync(jobPath, "utf8"));
  if (!job) {
    return `Provider E2E ${options.scenario} job sidecar is not valid JSON`;
  }
  if (job.status !== lifecycleScenario.jobSidecarStatus) {
    return `Provider E2E ${options.scenario} job sidecar status must be ${lifecycleScenario.jobSidecarStatus}`;
  }
  if (job.terminalReason !== lifecycleScenario.reason) {
    return `Provider E2E ${options.scenario} job sidecar must retain terminalReason`;
  }
  if (!providerRequestsEqual(job.providerRequest ?? {}, childResult.providerRequest)) {
    return `Provider E2E ${options.scenario} job sidecar provider request does not match child report`;
  }
  return null;
}

function validateProviderRetryEvidence(projectDir, options, completedProviderRequest) {
  const generatedIndexPath = join(projectDir, "generated", "index.json");
  const generatedIndex = safeJsonParse(readFileSync(generatedIndexPath, "utf8"));
  if (!generatedIndex || !Array.isArray(generatedIndex.assets)) {
    return "Provider E2E retry scenario generated index is not valid";
  }
  const completedAssets = generatedIndex.assets.filter(
    (asset) => asset.status === "completed" && asset.outputCount > 0,
  );
  if (completedAssets.length !== 1) {
    return "Provider E2E retry scenario must retain one completed retry asset";
  }
  const completedAsset = completedAssets[0];
  const completedSidecarPath = join(projectDir, completedAsset.path || "");
  const completedSidecar = safeJsonParse(readFileSync(completedSidecarPath, "utf8"));
  if (!completedSidecar) {
    return "Provider E2E retry scenario completed sidecar is not valid";
  }
  const retryOfAssetId = completedSidecar.retryOfAssetId;
  if (typeof retryOfAssetId !== "string" || retryOfAssetId.trim() === "") {
    return "Provider E2E retry scenario completed sidecar must retain retryOfAssetId";
  }
  const failedAsset = generatedIndex.assets.find((asset) => asset.assetId === retryOfAssetId);
  if (!failedAsset || failedAsset.status !== "failed" || failedAsset.outputCount !== 0) {
    return "Provider E2E retry scenario retryOfAssetId must point at a failed outputless asset";
  }
  if (failedAsset.modelProvider !== options.provider || failedAsset.modelId !== options.model) {
    return "Provider E2E retry scenario failed asset model metadata must match";
  }
  const failedSidecarPath = join(projectDir, failedAsset.path || "");
  const failedSidecar = safeJsonParse(readFileSync(failedSidecarPath, "utf8"));
  if (!failedSidecar) {
    return "Provider E2E retry scenario failed sidecar is not valid";
  }
  if (
    failedSidecar.id !== retryOfAssetId ||
    failedSidecar.status !== "failed" ||
    !Array.isArray(failedSidecar.outputs) ||
    failedSidecar.outputs.length !== 0
  ) {
    return "Provider E2E retry scenario failed sidecar must be failed and outputless";
  }
  const failedJobPath = join(projectDir, "jobs", retryOfAssetId, "job.json");
  const failedJob = safeJsonParse(readFileSync(failedJobPath, "utf8"));
  if (!failedJob) {
    return "Provider E2E retry scenario failed job sidecar is not valid";
  }
  if (failedJob.status !== "failed" || failedJob.terminalReason !== "provider_error") {
    return "Provider E2E retry scenario failed job must retain provider_error terminal evidence";
  }
  if (failedJob.providerRequest?.provider !== options.provider) {
    return "Provider E2E retry scenario failed job provider request must match provider";
  }
  if (failedJob.providerRequest?.requestId === completedProviderRequest?.requestId) {
    return "Provider E2E retry scenario failed and completed jobs must use distinct provider requests";
  }
  return null;
}

function validateReplacementTimelineEvidence(
  projectDir,
  mediaId,
  itemId = "item-upscale-target",
  linkedItemId = null,
) {
  const timelinePath = join(projectDir, "timeline.json");
  if (!existsSync(timelinePath)) {
    return "Provider E2E replacement scenario is missing timeline.json";
  }
  const timeline = safeJsonParse(readFileSync(timelinePath, "utf8"));
  if (!timeline) {
    return "Provider E2E replacement scenario timeline.json is not parseable";
  }
  const tracks = Array.isArray(timeline.tracks) ? timeline.tracks : [];
  const item = tracks
    .flatMap((track) => Array.isArray(track.items) ? track.items : [])
    .find((candidate) => candidate?.id === itemId);
  if (!item) {
    return "Provider E2E replacement scenario target item is missing";
  }
  if (item.source?.type !== "media" || item.source?.mediaId !== mediaId) {
    return "Provider E2E replacement scenario target item does not point at generated output media";
  }
  if (item.properties?.generatedOutputMediaId !== mediaId) {
    return "Provider E2E replacement scenario target item is missing generatedOutputMediaId evidence";
  }
  if (item.properties?.sourceOut !== item.durationSeconds) {
    return "Provider E2E replacement scenario target item sourceOut does not match replacement duration";
  }
  if (linkedItemId) {
    const linkedItem = tracks
      .flatMap((track) => Array.isArray(track.items) ? track.items : [])
      .find((candidate) => candidate?.id === linkedItemId);
    if (!linkedItem) {
      return "Provider E2E replacement scenario linked item is missing";
    }
    const replacementLinkGroupId = item.properties?.linkGroupId;
    if (typeof replacementLinkGroupId !== "string" || replacementLinkGroupId.trim() === "") {
      return "Provider E2E replacement scenario linked item is missing replacement linkGroupId evidence";
    }
    if (linkedItem.source?.type !== "media" || linkedItem.source?.mediaId !== mediaId) {
      return "Provider E2E replacement scenario linked item does not point at generated output media";
    }
    if (linkedItem.properties?.generatedOutputMediaId !== mediaId) {
      return "Provider E2E replacement scenario linked item is missing generatedOutputMediaId evidence";
    }
    if (linkedItem.properties?.sourceOut !== linkedItem.durationSeconds) {
      return "Provider E2E replacement scenario linked item sourceOut does not match replacement duration";
    }
    if (replacementLinkGroupId !== linkedItem.properties?.linkGroupId) {
      return "Provider E2E replacement scenario linked item is missing replacement linkGroupId evidence";
    }
  }
  return null;
}

function validateTimelineAudioInsertEvidence(projectDir, mediaId, outputDurationSeconds, scenario) {
  const insertConfig = timelineAudioInsertConfig(scenario);
  const timelinePath = join(projectDir, "timeline.json");
  if (!existsSync(timelinePath)) {
    return "Provider E2E audio insert scenario is missing timeline.json";
  }
  const timeline = safeJsonParse(readFileSync(timelinePath, "utf8"));
  if (!timeline) {
    return "Provider E2E audio insert scenario timeline.json is not parseable";
  }
  const tracks = Array.isArray(timeline.tracks) ? timeline.tracks : [];
  const sourceItem = tracks
    .flatMap((track) => Array.isArray(track.items) ? track.items : [])
    .find((candidate) => candidate?.id === insertConfig.sourceItemId);
  if (!sourceItem) {
    return "Provider E2E audio insert scenario source video item is missing";
  }
  if (sourceItem.kind !== "video_clip") {
    return "Provider E2E audio insert scenario source item is not a video clip";
  }
  if (
    sourceItem.source?.type !== "media" ||
    sourceItem.source?.mediaId !== "provider-e2e-video-source"
  ) {
    return "Provider E2E audio insert scenario source item does not point at selected video media";
  }
  if (sourceItem.startSeconds !== insertConfig.timelineStartSeconds) {
    return "Provider E2E audio insert scenario source item start does not match selected clip start";
  }
  if (
    sourceItem.durationSeconds !==
    insertConfig.videoSourceEndSeconds - insertConfig.videoSourceStartSeconds
  ) {
    return "Provider E2E audio insert scenario source item duration does not match selected source trim";
  }
  if (
    sourceItem.properties?.sourceIn !== insertConfig.videoSourceStartSeconds ||
    sourceItem.properties?.sourceOut !== insertConfig.videoSourceEndSeconds
  ) {
    return "Provider E2E audio insert scenario source item is missing selected source trim evidence";
  }
  const audioTrack = tracks.find((track) => track?.id === "track-audio" || track?.kind === "audio");
  const item = Array.isArray(audioTrack?.items)
    ? audioTrack.items.find((candidate) => candidate?.id === insertConfig.insertedItemId)
    : null;
  if (!item) {
    return "Provider E2E audio insert scenario generated audio item is missing";
  }
  if (item.kind !== "audio_clip") {
    return "Provider E2E audio insert scenario generated item is not an audio clip";
  }
  if (item.source?.type !== "media" || item.source?.mediaId !== mediaId) {
    return "Provider E2E audio insert scenario item does not point at generated output media";
  }
  if (item.startSeconds !== insertConfig.timelineStartSeconds) {
    return "Provider E2E audio insert scenario item start does not match selected clip start";
  }
  if (item.durationSeconds !== outputDurationSeconds) {
    return "Provider E2E audio insert scenario item duration does not match generated output";
  }
  if (item.properties?.sourceOut !== outputDurationSeconds) {
    return "Provider E2E audio insert scenario item sourceOut does not match generated output";
  }
  if (item.properties?.generatedOutputMediaId !== mediaId) {
    return "Provider E2E audio insert scenario item is missing generatedOutputMediaId evidence";
  }
  if (item.properties?.sourceVideoMediaRef !== "provider-e2e-video-source") {
    return "Provider E2E audio insert scenario item is missing sourceVideoMediaRef evidence";
  }
  return null;
}

function validateTimelineTextToMusicInsertEvidence(projectDir, mediaId, outputDurationSeconds) {
  const insertConfig = timelineTextToMusicInsertConfig();
  const timelinePath = join(projectDir, "timeline.json");
  if (!existsSync(timelinePath)) {
    return "Provider E2E text-to-music insert scenario timeline.json is missing";
  }
  let timeline;
  try {
    timeline = JSON.parse(readFileSync(timelinePath, "utf8"));
  } catch {
    return "Provider E2E text-to-music insert scenario timeline.json is not parseable";
  }
  const tracks = Array.isArray(timeline.tracks) ? timeline.tracks : [];
  const audioTrack = tracks.find((track) => track?.id === insertConfig.trackId || track?.kind === "audio");
  const item = Array.isArray(audioTrack?.items)
    ? audioTrack.items.find((candidate) => candidate?.id === insertConfig.insertedItemId)
    : null;
  if (!item) {
    return "Provider E2E text-to-music insert scenario generated audio item is missing";
  }
  if (item.kind !== "audio_clip") {
    return "Provider E2E text-to-music insert scenario generated item is not an audio clip";
  }
  if (item.source?.type !== "media" || item.source?.mediaId !== mediaId) {
    return "Provider E2E text-to-music insert scenario item does not point at generated output media";
  }
  if (item.startSeconds !== insertConfig.timelineStartSeconds) {
    return "Provider E2E text-to-music insert scenario item start does not match placement start";
  }
  if (item.durationSeconds !== outputDurationSeconds) {
    return "Provider E2E text-to-music insert scenario item duration does not match generated output";
  }
  if (item.properties?.sourceOut !== outputDurationSeconds) {
    return "Provider E2E text-to-music insert scenario sourceOut does not match generated output";
  }
  if (item.properties?.generatedOutputMediaId !== mediaId) {
    return "Provider E2E text-to-music insert scenario item is missing generatedOutputMediaId evidence";
  }
  return null;
}

function validateTimelineVisualInsertEvidence(projectDir, mediaId, outputDurationSeconds) {
  const insertConfig = timelineVisualInsertConfig();
  const timelinePath = join(projectDir, "timeline.json");
  if (!existsSync(timelinePath)) {
    return "Provider E2E visual insert scenario is missing timeline.json";
  }
  const timeline = safeJsonParse(readFileSync(timelinePath, "utf8"));
  if (!timeline) {
    return "Provider E2E visual insert scenario timeline.json is not parseable";
  }
  const tracks = Array.isArray(timeline.tracks) ? timeline.tracks : [];
  const videoTrack = tracks.find((track) =>
    track?.id === insertConfig.trackId || track?.kind === "video",
  );
  const item = Array.isArray(videoTrack?.items)
    ? videoTrack.items.find((candidate) => candidate?.id === insertConfig.insertedItemId)
    : null;
  if (!item) {
    return "Provider E2E visual insert scenario generated video item is missing";
  }
  if (item.kind !== "video_clip" && item.kind !== "generated_clip" && item.kind !== "image_clip") {
    return "Provider E2E visual insert scenario generated item is not a visual clip";
  }
  if (item.source?.type !== "media" || item.source?.mediaId !== mediaId) {
    return "Provider E2E visual insert scenario item does not point at generated output media";
  }
  if (item.startSeconds !== insertConfig.timelineStartSeconds) {
    return "Provider E2E visual insert scenario item start does not match selected clip start";
  }
  if (item.durationSeconds !== outputDurationSeconds) {
    return "Provider E2E visual insert scenario item duration does not match generated output";
  }
  if (item.properties?.sourceIn !== 0) {
    return "Provider E2E visual insert scenario item sourceIn does not start at generated output zero";
  }
  if (item.properties?.sourceOut !== outputDurationSeconds) {
    return "Provider E2E visual insert scenario item sourceOut does not match generated output";
  }
  if (item.properties?.generatedOutputMediaId !== mediaId) {
    return "Provider E2E visual insert scenario item is missing generatedOutputMediaId evidence";
  }
  return null;
}

if (process.argv[1] && resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  try {
    main();
  } catch (error) {
    console.error(error instanceof Error ? error.message : String(error));
    process.exit(2);
  }
}
