import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import { describe, expect, it } from "vitest";

const repoRoot = resolve(import.meta.dirname, "..");

describe("FluidAudio production speech helper packaging", () => {
  it("builds and bundles the pinned native helper without mutable runtime downloads", () => {
    const packageJson = JSON.parse(readFileSync(resolve(repoRoot, "package.json"), "utf8"));
    const tauriConfig = JSON.parse(
      readFileSync(resolve(repoRoot, "src-tauri/tauri.conf.json"), "utf8"),
    );
    const helper = readFileSync(
      resolve(
        repoRoot,
        "src-tauri/native/fluidaudio-parakeet/Sources/VideoCreaterFluidAudioTranscribeCore/SpeechAnalysis.swift",
      ),
      "utf8",
    );

    expect(packageJson.scripts["build:fluidaudio-helper"]).toContain("--release");
    expect(tauriConfig.bundle.externalBin).toContain(
      "binaries/video-creater-fluidaudio-transcribe",
    );
    expect(tauriConfig.build.beforeBuildCommand).toContain("build:fluidaudio-helper");
    expect(helper).toContain("fluid_audio_speech_analysis");
    expect(helper).toContain("OfflineDiarizerManager");
    expect(helper).toContain("VadManager(config: .default, vadModel: vadModel)");
    expect(helper).not.toContain("downloadIfNeeded");
    expect(helper).not.toContain("prepareModels(");
  });
});
