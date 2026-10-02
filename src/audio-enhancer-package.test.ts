import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import { pathToFileURL } from "node:url";
import { describe, expect, it } from "vitest";

describe("DeepFilterNet3 audio enhancer package", () => {
  it("pins Palmier's reviewed speech-swift runtime and packages the native helper", async () => {
    const swiftPackage = readFileSync("src-tauri/native/audio-enhance/Package.swift", "utf8");
    const helper = readFileSync("src-tauri/native/audio-enhance/Sources/main.swift", "utf8");
    const tauri = JSON.parse(readFileSync("src-tauri/tauri.conf.json", "utf8")) as {
      build: { beforeBuildCommand: string };
      bundle: { externalBin: string[] };
    };
    const packageJson = JSON.parse(readFileSync("package.json", "utf8")) as {
      scripts: Record<string, string>;
    };

    expect(swiftPackage).toContain("7609977be837a6529bd04300c6b963e735300070");
    expect(swiftPackage).toContain('.product(name: "SpeechEnhancement", package: "speech-swift")');
    expect(helper).toContain("SpeechEnhancer.sampleRate * 30");
    expect(helper).toContain('pinnedModelRevision = "937bad9811f1ffc1a06ea0d676461b080b2bdc93"');
    expect(helper).toContain("6b6adf3f78972bd1caed73e9e9e4eabd65590dcc9a86e41ae00e22b7c63a0018");
    expect(helper).toContain("verifyPinnedModel(at: staging)");
    expect(helper).toContain("offlineMode: true");
    expect(helper).toContain("drySample * Float(1 - arguments.strength)");
    expect(helper).toContain("wetSample * Float(arguments.strength)");
    expect(tauri.bundle.externalBin).toContain("binaries/video-creater-audio-enhance");
    expect(tauri.build.beforeBuildCommand).toContain("build:audio-enhancer");
    const { helperPreparationPlan } = await import(/* @vite-ignore */ pathToFileURL(resolve("scripts/prepare-desktop-development.mjs")).href) as { helperPreparationPlan(platform: string): string[][] };
    expect(packageJson.scripts["prepare:tauri:dev"]).toBe("node scripts/prepare-desktop-development.mjs --platform darwin");
    expect(helperPreparationPlan("darwin")).toContainEqual(["pnpm", "build:audio-enhancer:dev"]);
    expect(packageJson.scripts["tauri:dev"]).toBe("node scripts/tauri-dev.mjs");
    expect(packageJson.scripts["prepare:tauri:dev:linux"]).toBe("node scripts/prepare-desktop-development.mjs --platform linux");
    expect(helperPreparationPlan("linux")).toContainEqual(["pnpm", "build:linux-audio-enhancer:dev"]);
  });
});
