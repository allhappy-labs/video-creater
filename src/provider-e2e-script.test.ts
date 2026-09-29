import { cpSync, existsSync, mkdirSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join, relative } from "node:path";
import { spawnSync } from "node:child_process";
import { pathToFileURL } from "node:url";
import { describe, expect, it } from "vitest";
import { requiredAt } from "@/test-utils/required";

const repoRoot = process.cwd();

const replicateFluxVariantScenarios = [
  {
    scenario: "replicate-flux-dev",
    model: "black-forest-labs/flux-dev",
  },
  {
    scenario: "replicate-flux-1.1-pro",
    model: "black-forest-labs/flux-1.1-pro",
  },
  {
    scenario: "replicate-flux-1.1-pro-ultra",
    model: "black-forest-labs/flux-1.1-pro-ultra",
  },
] as const;

const multiImageProviderScenarios = [
  {
    provider: "fal.ai",
    model: "fal-ai/flux/schnell",
    envVar: "FAL_KEY",
    tempPrefix: "video-creater-provider-fal-multi-image-",
  },
  {
    provider: "replicate",
    model: "black-forest-labs/flux-schnell",
    envVar: "REPLICATE_API_TOKEN",
    tempPrefix: "video-creater-provider-replicate-multi-image-",
  },
  {
    provider: "openai",
    model: "gpt-image-2",
    envVar: "OPENAI_API_KEY",
    tempPrefix: "video-creater-provider-openai-multi-image-",
  },
] as const;
const textToAudioDefaultVoices = {
  openai: "alloy",
  elevenlabs: "rachel",
  google: "Kore",
} as const;

const textToMusicDefaultSettings = {
  category: "music",
  durationSeconds: 30,
  instrumental: false,
  styleInstructions: "bright, commercial, loopable",
} as const;
const videoToMusicDefaultPrompt = textToMusicDefaultSettings.styleInstructions;
const minimaxTextToMusicDefaultLyrics = "[Verse]\nBright product launch\n[Chorus]\nLoopable brand hook";
const elevenlabsTextToMusicDefaultLyrics =
  "[Verse]\nWarm synth pop in English\n[Chorus]\nBright chorus lift";
const googleLyriaTextToMusicDefaultLyrics =
  "[Verse]\nSoft synth pulse in English\n[Chorus]\nConfident vocal lift";

describe("provider E2E runner", () => {
  it("runs a mocked provider service by default before checking credentials or cargo", () => {
    const packageJson = JSON.parse(
      readFileSync(join(repoRoot, "package.json"), "utf8"),
    ) as { scripts?: Record<string, string> };
    const tempDir = mkdtempSync(join(tmpdir(), "video-creater-provider-default-"));
    const outDir = join(tempDir, "mocked-provider");
    const fakeCargo = join(tempDir, "cargo");
    writeFileSync(
      fakeCargo,
      "#!/bin/sh\necho cargo should not run >&2\nexit 42\n",
      { mode: 0o755 },
    );

    expect(packageJson.scripts?.["e2e:providers"]).toBe(
      "node scripts/provider-e2e.mjs",
    );
    expect(packageJson.scripts?.["e2e:providers-policy"]).toBe(
      "node scripts/provider-e2e-report-policy.mjs",
    );

    const result = spawnSync(
      process.execPath,
      [
        join(repoRoot, "scripts/provider-e2e.mjs"),
        "--provider",
        "replicate",
        "--out-dir",
        outDir,
      ],
      {
        cwd: repoRoot,
        encoding: "utf8",
        env: {
          ...process.env,
          PATH: `${tempDir}:${process.env.PATH ?? ""}`,
          REPLICATE_API_TOKEN: "",
        },
      },
    );

    expect(result.status).toBe(0);
    expect(result.stderr).not.toContain("cargo should not run");
    const report = JSON.parse(result.stdout) as {
      ok: boolean;
      provider: string;
      credentialStatus: string;
      command: string | null;
      mockedProviderService: boolean;
      result: { projectDir: string; artifactPath: string; providerRequest: { statusUrl: string } };
    };
    expect(report).toEqual(
      expect.objectContaining({
        ok: true,
        provider: "replicate",
        credentialStatus: "not_checked",
        command: null,
        mockedProviderService: true,
      }),
    );
    expect(report.result.projectDir).toBe(outDir);
    expect(report.result.artifactPath.startsWith(outDir)).toBe(true);
    expect(report.result.providerRequest.statusUrl).toContain("mock.replicate.example");
    expect(readFileSync(report.result.artifactPath, "utf8")).toBe("mock generated pixels");
    expect(
      JSON.parse(readFileSync(join(outDir, "provider-e2e-report.json"), "utf8")),
    ).toEqual(report);
  });

  it.each(multiImageProviderScenarios)(
    "runs a mocked $provider multi-image scenario without credentials or cargo",
    ({ provider, model, envVar, tempPrefix }) => {
      const tempDir = mkdtempSync(join(tmpdir(), tempPrefix));
      const outDir = join(tempDir, `mocked-${provider.replace(/\./g, "-")}-multi-image-provider`);
      const fakeCargo = join(tempDir, "cargo");
      writeFileSync(
        fakeCargo,
        "#!/bin/sh\necho cargo should not run >&2\nexit 42\n",
        { mode: 0o755 },
      );

      const result = spawnSync(
        process.execPath,
        [
          join(repoRoot, "scripts/provider-e2e.mjs"),
          "--provider",
          provider,
          "--scenario",
          "multi-image",
          "--out-dir",
          outDir,
        ],
        {
          cwd: repoRoot,
          encoding: "utf8",
          env: {
            ...process.env,
            PATH: `${tempDir}:${process.env.PATH ?? ""}`,
            [envVar]: "",
          },
        },
      );

      expect(result.status).toBe(0);
      expect(result.stderr).not.toContain("cargo should not run");
      const report = JSON.parse(result.stdout) as {
        ok: boolean;
        provider: string;
        model: string;
        scenario: string;
        credentialStatus: string;
        command: string | null;
        mockedProviderService: boolean;
        result: { artifactPath: string; outputCount: number };
      };
      expect(report).toEqual(
        expect.objectContaining({
          ok: true,
          provider,
          model,
          scenario: "multi-image",
          credentialStatus: "not_checked",
          command: null,
          mockedProviderService: true,
        }),
      );
      expect(report.result.outputCount).toBe(2);
      expect(report.result.artifactPath.endsWith(".png")).toBe(true);
      const sidecar = JSON.parse(
        readFileSync(
          join(outDir, "generated", "provider-e2e-generated", "asset.json"),
          "utf8",
        ),
      ) as {
        outputs: Array<{ mediaId: string; relativePath: string; width: number; height: number }>;
        settings: { numImages?: number };
      };
      expect(sidecar.settings.numImages).toBe(2);
      expect(sidecar.outputs).toHaveLength(2);
      expect(sidecar.outputs.map((output) => output.mediaId)).toEqual([
        "provider-e2e-output",
        "provider-e2e-output-2",
      ]);
      expect(sidecar.outputs.map((output) => output.relativePath)).toEqual([
        `generated/provider-e2e-generated/${provider.replace(/\./g, "-")}-mock-output.png`,
        `generated/provider-e2e-generated/${provider.replace(/\./g, "-")}-mock-output-2.png`,
      ]);
      for (const output of sidecar.outputs) {
        expect(output.width).toBe(1024);
        expect(output.height).toBe(768);
      }
      const mediaIndex = JSON.parse(readFileSync(join(outDir, "media", "index.json"), "utf8")) as {
        assets: Array<{ id: string; kind: string; relativePath: string }>;
      };
      expect(mediaIndex.assets.filter((asset) => asset.kind === "generated")).toEqual([
        expect.objectContaining({
          id: "provider-e2e-output",
          relativePath: `generated/provider-e2e-generated/${provider.replace(/\./g, "-")}-mock-output.png`,
        }),
        expect.objectContaining({
          id: "provider-e2e-output-2",
          relativePath: `generated/provider-e2e-generated/${provider.replace(/\./g, "-")}-mock-output-2.png`,
        }),
      ]);
    },
  );

  it("runs a mocked OpenAI text-to-image provider service without credentials or cargo", () => {
    const tempDir = mkdtempSync(join(tmpdir(), "video-creater-provider-openai-default-"));
    const outDir = join(tempDir, "mocked-openai-provider");
    const fakeCargo = join(tempDir, "cargo");
    writeFileSync(
      fakeCargo,
      "#!/bin/sh\necho cargo should not run >&2\nexit 42\n",
      { mode: 0o755 },
    );

    const result = spawnSync(
      process.execPath,
      [
        join(repoRoot, "scripts/provider-e2e.mjs"),
        "--provider",
        "openai",
        "--out-dir",
        outDir,
      ],
      {
        cwd: repoRoot,
        encoding: "utf8",
        env: {
          ...process.env,
          PATH: `${tempDir}:${process.env.PATH ?? ""}`,
          OPENAI_API_KEY: "",
        },
      },
    );

    expect(result.status).toBe(0);
    expect(result.stderr).not.toContain("cargo should not run");
    const report = JSON.parse(result.stdout) as {
      ok: boolean;
      provider: string;
      model: string;
      credentialStatus: string;
      command: string | null;
      mockedProviderService: boolean;
      result: {
        projectDir: string;
        artifactPath: string;
        providerRequest: { provider: string; statusUrl: string };
      };
    };
    expect(report).toEqual(
      expect.objectContaining({
        ok: true,
        provider: "openai",
        model: "gpt-image-2",
        credentialStatus: "not_checked",
        command: null,
        mockedProviderService: true,
      }),
    );
    expect(report.result.projectDir).toBe(outDir);
    expect(report.result.artifactPath.startsWith(outDir)).toBe(true);
    expect(report.result.providerRequest.provider).toBe("openai");
    expect(report.result.providerRequest.statusUrl).toContain("mock.openai.example");
    expect(readFileSync(report.result.artifactPath, "utf8")).toBe("mock generated pixels");
    const sidecar = JSON.parse(
      readFileSync(
        join(outDir, "generated", "provider-e2e-generated", "asset.json"),
        "utf8",
      ),
    ) as { outputs: Array<{ sourceUrl?: string }> };
    expect(requiredAt(sidecar.outputs, 0, "provider sidecar output")).not.toHaveProperty("sourceUrl");
  });

  it("runs a mocked xAI text-to-image provider service without credentials or cargo", () => {
    const tempDir = mkdtempSync(join(tmpdir(), "video-creater-provider-xai-default-"));
    const outDir = join(tempDir, "mocked-xai-provider");
    const fakeCargo = join(tempDir, "cargo");
    writeFileSync(
      fakeCargo,
      "#!/bin/sh\necho cargo should not run >&2\nexit 42\n",
      { mode: 0o755 },
    );

    const result = spawnSync(
      process.execPath,
      [
        join(repoRoot, "scripts/provider-e2e.mjs"),
        "--provider",
        "xai",
        "--out-dir",
        outDir,
      ],
      {
        cwd: repoRoot,
        encoding: "utf8",
        env: {
          ...process.env,
          PATH: `${tempDir}:${process.env.PATH ?? ""}`,
          XAI_API_KEY: "",
        },
      },
    );

    expect(result.status).toBe(0);
    expect(result.stderr).not.toContain("cargo should not run");
    const report = JSON.parse(result.stdout) as {
      ok: boolean;
      provider: string;
      model: string;
      credentialStatus: string;
      command: string | null;
      mockedProviderService: boolean;
      result: {
        projectDir: string;
        artifactPath: string;
        providerRequest: { provider: string; statusUrl: string };
      };
    };
    expect(report).toEqual(
      expect.objectContaining({
        ok: true,
        provider: "xai",
        model: "grok-imagine-image-quality",
        credentialStatus: "not_checked",
        command: null,
        mockedProviderService: true,
      }),
    );
    expect(report.result.projectDir).toBe(outDir);
    expect(report.result.artifactPath.startsWith(outDir)).toBe(true);
    expect(report.result.providerRequest.provider).toBe("xai");
    expect(report.result.providerRequest.statusUrl).toContain("mock.xai.example");
    expect(readFileSync(report.result.artifactPath, "utf8")).toBe("mock generated pixels");
    const sidecar = JSON.parse(
      readFileSync(
        join(outDir, "generated", "provider-e2e-generated", "asset.json"),
        "utf8",
      ),
    ) as { outputs: Array<{ sourceUrl?: string }> };
    expect(requiredAt(sidecar.outputs, 0, "provider sidecar output").sourceUrl).toContain("mock.xai.example");
  });

  it("runs a mocked xAI video provider service without credentials or cargo", () => {
    const tempDir = mkdtempSync(join(tmpdir(), "video-creater-provider-xai-video-default-"));
    const outDir = join(tempDir, "mocked-xai-video-provider");
    const fakeCargo = join(tempDir, "cargo");
    writeFileSync(
      fakeCargo,
      "#!/bin/sh\necho cargo should not run >&2\nexit 42\n",
      { mode: 0o755 },
    );

    const result = spawnSync(
      process.execPath,
      [
        join(repoRoot, "scripts/provider-e2e.mjs"),
        "--provider",
        "xai",
        "--scenario",
        "xai-video",
        "--out-dir",
        outDir,
      ],
      {
        cwd: repoRoot,
        encoding: "utf8",
        env: {
          ...process.env,
          PATH: `${tempDir}:${process.env.PATH ?? ""}`,
          XAI_API_KEY: "",
        },
      },
    );

    expect(result.status).toBe(0);
    expect(result.stderr).not.toContain("cargo should not run");
    const report = JSON.parse(result.stdout) as {
      ok: boolean;
      provider: string;
      model: string;
      scenario: string;
      credentialStatus: string;
      command: string | null;
      mockedProviderService: boolean;
      result: { artifactPath: string; outputCount: number };
    };
    expect(report).toEqual(
      expect.objectContaining({
        ok: true,
        provider: "xai",
        model: "grok-imagine-video",
        scenario: "xai-video",
        credentialStatus: "not_checked",
        command: null,
        mockedProviderService: true,
      }),
    );
    expect(report.result.outputCount).toBe(1);
    expect(report.result.artifactPath.endsWith(".mp4")).toBe(true);
    const sidecar = JSON.parse(
      readFileSync(
        join(outDir, "generated", "provider-e2e-generated", "asset.json"),
        "utf8",
      ),
    ) as {
      settings?: { generateAudio?: boolean };
      references?: {
        mediaIds?: string[];
        referenceImageMediaRefs?: string[];
        providerInputUrls?: string[];
      };
      outputs: Array<{
        relativePath: string;
        sourceUrl?: string;
        durationSeconds: number;
        fps: number;
      }>;
    };
    expect(requiredAt(sidecar.outputs, 0, "provider sidecar output")).toEqual(
      expect.objectContaining({
        relativePath: "generated/provider-e2e-generated/xai-mock-output.mp4",
        durationSeconds: 4,
        fps: 24,
      }),
    );
    expect(sidecar.settings?.generateAudio).toBe(true);
    expect(sidecar.references).toEqual(
      expect.objectContaining({
        mediaIds: ["provider-e2e-reference-image"],
        referenceImageMediaRefs: ["provider-e2e-reference-image"],
        providerInputUrls: ["https://api.x.ai/v1/files/provider-e2e/reference-image.png"],
      }),
    );
    expect(requiredAt(sidecar.outputs, 0, "provider sidecar output").sourceUrl).toContain("mock.xai.example");
  });

  it("runs a mocked xAI source-video edit replacement scenario without credentials or cargo", () => {
    const tempDir = mkdtempSync(join(tmpdir(), "video-creater-provider-xai-video-edit-replace-"));
    const outDir = join(tempDir, "mocked-xai-video-edit-replace-provider");
    const fakeCargo = join(tempDir, "cargo");
    writeFileSync(
      fakeCargo,
      "#!/bin/sh\necho cargo should not run >&2\nexit 42\n",
      { mode: 0o755 },
    );

    const result = spawnSync(
      process.execPath,
      [
        join(repoRoot, "scripts/provider-e2e.mjs"),
        "--provider",
        "xai",
        "--scenario",
        "local-video-edit-replace",
        "--out-dir",
        outDir,
      ],
      {
        cwd: repoRoot,
        encoding: "utf8",
        env: {
          ...process.env,
          PATH: `${tempDir}:${process.env.PATH ?? ""}`,
          XAI_API_KEY: "",
        },
      },
    );

    expect(result.status).toBe(0);
    expect(result.stderr).not.toContain("cargo should not run");
    const report = JSON.parse(result.stdout) as {
      ok: boolean;
      provider: string;
      model: string;
      scenario: string;
      credentialStatus: string;
      command: string | null;
      mockedProviderService: boolean;
      result: { artifactPath: string; outputCount: number };
    };
    expect(report).toEqual(
      expect.objectContaining({
        ok: true,
        provider: "xai",
        model: "grok-imagine-video",
        scenario: "local-video-edit-replace",
        credentialStatus: "not_checked",
        command: null,
        mockedProviderService: true,
      }),
    );
    expect(report.result.outputCount).toBe(1);
    expect(report.result.artifactPath.endsWith(".mp4")).toBe(true);
    const sidecar = JSON.parse(
      readFileSync(
        join(outDir, "generated", "provider-e2e-generated", "asset.json"),
        "utf8",
      ),
    ) as {
      placementIntent?: string;
      references?: {
        mediaIds?: string[];
        sourceVideoMediaRef?: string;
        referenceVideoMediaRefs?: string[];
        providerInputUrls?: string[];
      };
      settings?: {
        generateAudio?: boolean;
        videoSourceStartSeconds?: number;
        videoSourceEndSeconds?: number;
      };
      outputs: Array<{
        relativePath: string;
        sourceUrl?: string;
        durationSeconds: number;
        fps: number;
      }>;
    };
    expect(sidecar.placementIntent).toBe("replace:item-video-edit-target");
    expect(sidecar.references).toEqual(
      expect.objectContaining({
        mediaIds: ["provider-e2e-video-source"],
        sourceVideoMediaRef: "provider-e2e-video-source",
        referenceVideoMediaRefs: ["provider-e2e-video-source"],
        providerInputUrls: ["https://api.x.ai/v1/files/provider-e2e/source-video.mp4"],
      }),
    );
    expect(sidecar.settings).toEqual(
      expect.objectContaining({
        generateAudio: true,
        videoSourceStartSeconds: 0.5,
        videoSourceEndSeconds: 3.5,
      }),
    );
    expect(requiredAt(sidecar.outputs, 0, "provider sidecar output")).toEqual(
      expect.objectContaining({
        relativePath: "generated/provider-e2e-generated/xai-mock-output.mp4",
        durationSeconds: 3,
        fps: 24,
      }),
    );
    expect(requiredAt(sidecar.outputs, 0, "provider sidecar output").sourceUrl).toContain("mock.xai.example");
  });

  it("runs a mocked xAI text-to-video replacement scenario without credentials or cargo", () => {
    const tempDir = mkdtempSync(join(tmpdir(), "video-creater-provider-xai-text-video-replace-"));
    const outDir = join(tempDir, "mocked-xai-text-video-replace-provider");
    const fakeCargo = join(tempDir, "cargo");
    writeFileSync(
      fakeCargo,
      "#!/bin/sh\necho cargo should not run >&2\nexit 42\n",
      { mode: 0o755 },
    );

    const result = spawnSync(
      process.execPath,
      [
        join(repoRoot, "scripts/provider-e2e.mjs"),
        "--provider",
        "xai",
        "--scenario",
        "text-to-video-replace",
        "--out-dir",
        outDir,
      ],
      {
        cwd: repoRoot,
        encoding: "utf8",
        env: {
          ...process.env,
          PATH: `${tempDir}:${process.env.PATH ?? ""}`,
          XAI_API_KEY: "",
        },
      },
    );

    expect(result.status).toBe(0);
    expect(result.stderr).not.toContain("cargo should not run");
    const report = JSON.parse(result.stdout) as {
      ok: boolean;
      provider: string;
      model: string;
      scenario: string;
      credentialStatus: string;
      command: string | null;
      mockedProviderService: boolean;
      result: { artifactPath: string; outputCount: number };
    };
    expect(report).toEqual(
      expect.objectContaining({
        ok: true,
        provider: "xai",
        model: "grok-imagine-video",
        scenario: "text-to-video-replace",
        credentialStatus: "not_checked",
        command: null,
        mockedProviderService: true,
      }),
    );
    expect(report.result.outputCount).toBe(1);
    expect(report.result.artifactPath.endsWith(".mp4")).toBe(true);
    const sidecar = JSON.parse(
      readFileSync(
        join(outDir, "generated", "provider-e2e-generated", "asset.json"),
        "utf8",
      ),
    ) as {
      placementIntent?: string;
      settings?: { generateAudio?: boolean };
      outputs: Array<{
        relativePath: string;
        sourceUrl?: string;
        durationSeconds: number;
        fps: number;
      }>;
    };
    expect(sidecar.placementIntent).toBe("replace:item-text-video-target");
    expect(sidecar.settings).toEqual(expect.objectContaining({ generateAudio: true }));
    expect(requiredAt(sidecar.outputs, 0, "provider sidecar output")).toEqual(
      expect.objectContaining({
        relativePath: "generated/provider-e2e-generated/xai-mock-output.mp4",
        durationSeconds: 5,
        fps: 24,
      }),
    );
    expect(requiredAt(sidecar.outputs, 0, "provider sidecar output").sourceUrl).toContain("mock.xai.example");
  });

  it("runs a mocked Replicate text-to-video timeline insert scenario without credentials or cargo", () => {
    const tempDir = mkdtempSync(join(tmpdir(), "video-creater-provider-replicate-text-video-insert-"));
    const outDir = join(tempDir, "mocked-replicate-text-video-insert-provider");
    const fakeCargo = join(tempDir, "cargo");
    writeFileSync(
      fakeCargo,
      "#!/bin/sh\necho cargo should not run >&2\nexit 42\n",
      { mode: 0o755 },
    );

    const result = spawnSync(
      process.execPath,
      [
        join(repoRoot, "scripts/provider-e2e.mjs"),
        "--provider",
        "replicate",
        "--scenario",
        "text-to-video-insert",
        "--out-dir",
        outDir,
      ],
      {
        cwd: repoRoot,
        encoding: "utf8",
        env: {
          ...process.env,
          PATH: `${tempDir}:${process.env.PATH ?? ""}`,
          REPLICATE_API_TOKEN: "",
        },
      },
    );

    expect(result.status).toBe(0);
    expect(result.stderr).not.toContain("cargo should not run");
    const report = JSON.parse(result.stdout) as {
      ok: boolean;
      provider: string;
      model: string;
      scenario: string;
      credentialStatus: string;
      command: string | null;
      mockedProviderService: boolean;
      result: { artifactPath: string; outputCount: number };
    };
    expect(report).toEqual(
      expect.objectContaining({
        ok: true,
        provider: "replicate",
        model: "bytedance/seedance-2.0",
        scenario: "text-to-video-insert",
        credentialStatus: "not_checked",
        command: null,
        mockedProviderService: true,
      }),
    );
    expect(report.result.outputCount).toBe(1);
    expect(report.result.artifactPath.endsWith(".mp4")).toBe(true);
    const sidecar = JSON.parse(
      readFileSync(
        join(outDir, "generated", "provider-e2e-generated", "asset.json"),
        "utf8",
      ),
    ) as {
      placementIntent?: string;
      settings?: { timelineStartSeconds?: number; generateAudio?: boolean };
      outputs: Array<{
        mediaId: string;
        relativePath: string;
        sourceUrl?: string;
        durationSeconds: number;
        fps: number;
      }>;
    };
    expect(sidecar.placementIntent).toBe("insert-video:track-video");
    expect(sidecar.settings).toEqual(
      expect.objectContaining({
        timelineStartSeconds: 6,
        generateAudio: true,
      }),
    );
    expect(requiredAt(sidecar.outputs, 0, "provider sidecar output")).toEqual(
      expect.objectContaining({
        relativePath: "generated/provider-e2e-generated/replicate-mock-output.mp4",
        durationSeconds: 5,
        fps: 24,
      }),
    );
    expect(requiredAt(sidecar.outputs, 0, "provider sidecar output").sourceUrl).toContain("mock.replicate.example");

    const timeline = JSON.parse(readFileSync(join(outDir, "timeline.json"), "utf8")) as {
      tracks: Array<{
        id: string;
        kind: string;
        items: Array<{
          id: string;
          kind: string;
          startSeconds: number;
          durationSeconds: number;
          source?: { type?: string; mediaId?: string };
          properties?: { generatedOutputMediaId?: string };
        }>;
      }>;
    };
    const insertedVideo = timeline.tracks
      .find((track) => track.id === "track-video")
      ?.items.find((item) => item.id === "item-text-video-inserted-video");
    expect(insertedVideo).toEqual(
      expect.objectContaining({
        kind: "video_clip",
        startSeconds: 6,
        durationSeconds: 5,
        source: { type: "media", mediaId: requiredAt(sidecar.outputs, 0, "provider sidecar output")?.mediaId },
        properties: expect.objectContaining({
          generatedOutputMediaId: requiredAt(sidecar.outputs, 0, "provider sidecar output")?.mediaId,
        }),
      }),
    );
  });

  it("documents the live OpenAI text-to-image, image-edit, and text-to-audio provider E2E commands", () => {
    const result = spawnSync(
      process.execPath,
      [join(repoRoot, "scripts/provider-e2e.mjs"), "--help"],
      {
        cwd: repoRoot,
        encoding: "utf8",
      },
    );

    expect(result.status).toBe(0);
    expect(result.stdout).toContain(
      "pnpm e2e:providers -- --provider openai --live [--model gpt-image-2]",
    );
    expect(result.stdout).toContain(
      "pnpm e2e:providers -- --provider openai --live --scenario image-edit",
    );
    expect(result.stdout).toContain(
      "pnpm e2e:providers -- --provider openai --live --scenario local-image-edit-replace",
    );
    expect(result.stdout).toContain(
      "pnpm e2e:providers -- --provider openai --live --scenario text-to-audio",
    );
    expect(result.stdout).toContain(
      "pnpm e2e:providers -- --provider elevenlabs --live --scenario text-to-audio",
    );
    expect(result.stdout).toContain(
      "pnpm e2e:providers -- --provider elevenlabs --live --scenario text-to-music",
    );
    expect(result.stdout).toContain(
      "pnpm e2e:providers -- --provider xai --live [--model grok-imagine-image-quality]",
    );
    expect(result.stdout).toContain(
      "pnpm e2e:providers -- --provider xai --live --scenario image-edit",
    );
    expect(result.stdout).toContain(
      "pnpm e2e:providers -- --provider xai --live --scenario local-image-edit-replace",
    );
    expect(result.stdout).toContain(
      "pnpm e2e:providers -- --provider xai --live --scenario xai-video",
    );
    expect(result.stdout).toContain(
      "pnpm e2e:providers -- --provider xai --live --scenario local-video-edit-replace",
    );
    expect(result.stdout).toContain(
      "pnpm e2e:providers -- --provider xai --live --scenario text-to-video-replace",
    );
    expect(result.stdout).toContain(
      "pnpm e2e:providers -- --provider google --live --scenario google-video",
    );
    expect(result.stdout).toContain(
      "pnpm e2e:providers -- --provider google --live --scenario text-to-video-replace",
    );
    expect(result.stdout).toContain(
      "pnpm e2e:providers -- --provider google --live --scenario text-to-audio",
    );
    expect(result.stdout).toContain(
      "pnpm e2e:providers -- --provider google --live --scenario text-to-music",
    );
    expect(result.stdout).toContain(
      "pnpm e2e:providers -- --provider minimax --live --scenario text-to-music",
    );
    expect(result.stdout).toContain(
      "pnpm e2e:providers -- --provider fal.ai --live --scenario local-video-to-sfx-insert",
    );
    expect(result.stdout).toContain(
      "pnpm e2e:providers -- --provider fal.ai --live --scenario text-to-video-replace",
    );
    expect(result.stdout).toContain(
      "pnpm e2e:providers -- --provider fal.ai --live --scenario text-to-video-insert",
    );
    expect(result.stdout).toContain(
      "pnpm e2e:providers -- --provider fal.ai --live --scenario local-video-upscale-replace",
    );
    expect(result.stdout).toContain(
      "pnpm e2e:providers -- --provider fal.ai --live --scenario local-video-upscale-retry",
    );
    expect(result.stdout).toContain(
      "pnpm e2e:providers -- --provider fal.ai --live --scenario local-video-upscale-cancellation",
    );
    expect(result.stdout).toContain(
      "pnpm e2e:providers -- --provider fal.ai --live --scenario local-video-upscale-failure",
    );
    expect(result.stdout).toContain(
      "pnpm e2e:providers -- --provider fal.ai --live --scenario local-image-edit-replace",
    );
    expect(result.stdout).toContain(
      "pnpm e2e:providers -- --provider fal.ai --live --scenario local-image-edit-cancellation",
    );
    expect(result.stdout).toContain(
      "pnpm e2e:providers -- --provider fal.ai --live --scenario local-image-edit-failure",
    );
    expect(result.stdout).toContain(
      "pnpm e2e:providers -- --provider fal.ai --live --scenario multi-image",
    );
    expect(result.stdout).toContain(
      "pnpm e2e:providers -- --provider openai --live --scenario multi-image",
    );
    expect(result.stdout).toContain(
      "pnpm e2e:providers -- --provider replicate --live --scenario multi-image",
    );
    expect(result.stdout).toContain(
      "pnpm e2e:providers -- --provider replicate --live --scenario replicate-flux-dev",
    );
    expect(result.stdout).toContain(
      "pnpm e2e:providers -- --provider replicate --live --scenario replicate-flux-1.1-pro",
    );
    expect(result.stdout).toContain(
      "pnpm e2e:providers -- --provider replicate --live --scenario replicate-flux-1.1-pro-ultra",
    );
    expect(result.stdout).toContain(
      "pnpm e2e:providers -- --provider replicate --live --scenario text-to-video-replace",
    );
  });

  it("documents all required retained provider evidence reports in policy help", () => {
    const result = spawnSync(
      process.execPath,
      [join(repoRoot, "scripts/provider-e2e-report-policy.mjs"), "--help"],
      {
        cwd: repoRoot,
        encoding: "utf8",
      },
    );

    expect(result.status).toBe(0);
    expect(result.stdout).toContain(
      "output/provider-e2e/openai-audio-mock-default/provider-e2e-report.json",
    );
    expect(result.stdout).toContain(
      "output/provider-e2e/elevenlabs-audio-mock-default/provider-e2e-report.json",
    );
    expect(result.stdout).toContain(
      "output/provider-e2e/elevenlabs-music-mock-default/provider-e2e-report.json",
    );
    expect(result.stdout).toContain(
      "output/provider-e2e/minimax-music-mock-default/provider-e2e-report.json",
    );
    expect(result.stdout).toContain(
      "output/provider-e2e/xai-mock-default/provider-e2e-report.json",
    );
    expect(result.stdout).toContain(
      "output/provider-e2e/xai-image-edit-mock-default/provider-e2e-report.json",
    );
    expect(result.stdout).toContain(
      "output/provider-e2e/xai-video-mock-default/provider-e2e-report.json",
    );
    expect(result.stdout).toContain(
      "output/provider-e2e/google-video-mock-default/provider-e2e-report.json",
    );
    expect(result.stdout).toContain(
      "output/provider-e2e/google-gemini-tts-mock-default/provider-e2e-report.json",
    );
    expect(result.stdout).toContain(
      "output/provider-e2e/google-lyria-mock-default/provider-e2e-report.json",
    );
    expect(result.stdout).toContain(
      "output/provider-e2e/fal.ai-video-to-music-insert/provider-e2e-report.json",
    );
    expect(result.stdout).toContain(
      "output/provider-e2e/replicate-video-fast/provider-e2e-report.json",
    );
    expect(result.stdout).toContain(
      "output/provider-e2e/replicate-text-to-video-replace/provider-e2e-report.json",
    );
    expect(result.stdout).toContain(
      "output/provider-e2e/replicate-flux-dev/provider-e2e-report.json",
    );
    expect(result.stdout).toContain(
      "output/provider-e2e/replicate-flux-1.1-pro/provider-e2e-report.json",
    );
    expect(result.stdout).toContain(
      "output/provider-e2e/replicate-flux-1.1-pro-ultra/provider-e2e-report.json",
    );
    expect(result.stdout).not.toContain("output/provider-e2e/replicate-replicate-flux");
    expect(result.stdout).toContain(
      "output/provider-e2e/fal.ai-failure/provider-e2e-report.json",
    );
    expect(result.stdout).toContain(
      "output/provider-e2e/replicate-cancellation/provider-e2e-report.json",
    );
  });

  it.each([
    {
      provider: "replicate",
      scenario: "provider-failure",
      expectedJobStatus: "Failed",
      expectedReason: "provider_error",
    },
    {
      provider: "fal.ai",
      scenario: "provider-cancellation",
      expectedJobStatus: "Cancelled",
      expectedReason: "cancelled_by_user",
    },
    {
      provider: "replicate",
      scenario: "provider-retry",
      expectedJobStatus: "Completed",
      expectedReason: null,
    },
  ] as const)(
    "runs mocked $scenario lifecycle evidence without credentials or cargo",
    ({ provider, scenario, expectedJobStatus, expectedReason }) => {
      const tempDir = mkdtempSync(join(tmpdir(), `video-creater-provider-${scenario}-`));
      const outDir = join(tempDir, "mocked-lifecycle-provider");
      const fakeCargo = join(tempDir, "cargo");
      writeFileSync(
        fakeCargo,
        "#!/bin/sh\necho cargo should not run >&2\nexit 42\n",
        { mode: 0o755 },
      );

      const result = spawnSync(
        process.execPath,
        [
          join(repoRoot, "scripts/provider-e2e.mjs"),
          "--provider",
          provider,
          "--scenario",
          scenario,
          "--out-dir",
          outDir,
        ],
        {
          cwd: repoRoot,
          encoding: "utf8",
          env: {
            ...process.env,
            PATH: `${tempDir}:${process.env.PATH ?? ""}`,
            FAL_KEY: "",
            REPLICATE_API_TOKEN: "",
          },
        },
      );

      expect(result.status).toBe(0);
      expect(result.stderr).not.toContain("cargo should not run");
      const report = JSON.parse(result.stdout) as {
        ok: boolean;
        provider: string;
        scenario: string;
        credentialStatus: string;
        command: string | null;
        result: {
          ok: boolean;
          projectDir: string;
          outputCount: number;
          jobStatus: string;
          terminalReason?: string;
        };
      };
      expect(report).toEqual(
        expect.objectContaining({
          ok: true,
          provider,
          scenario,
          credentialStatus: "not_checked",
          command: null,
        }),
      );
      expect(report.result.jobStatus).toBe(expectedJobStatus);
      if (expectedReason) {
        expect(report.result).toEqual(
          expect.objectContaining({
            ok: false,
            outputCount: 0,
            terminalReason: expectedReason,
          }),
        );
      }
      const generatedIndex = JSON.parse(
        readFileSync(join(report.result.projectDir, "generated", "index.json"), "utf8"),
      ) as { assets: Array<{ assetId: string; status: string; outputCount: number; path: string }> };
      if (scenario === "provider-retry") {
        const completed = generatedIndex.assets.find((asset) => asset.status === "completed");
        expect(completed).toBeTruthy();
        const completedSidecar = JSON.parse(
          readFileSync(join(report.result.projectDir, completed?.path ?? ""), "utf8"),
        ) as { retryOfAssetId: string };
        expect(completedSidecar.retryOfAssetId).toBeTruthy();
        expect(generatedIndex.assets).toContainEqual(
          expect.objectContaining({
            assetId: completedSidecar.retryOfAssetId,
            status: "failed",
            outputCount: 0,
          }),
        );
      } else {
        expect(generatedIndex.assets).toEqual([
          expect.objectContaining({ status: "failed", outputCount: 0 }),
        ]);
      }
    },
  );

  it("runs a mocked fal WAN image-to-video scenario without credentials or cargo", () => {
    const tempDir = mkdtempSync(join(tmpdir(), "video-creater-provider-wan-default-"));
    const outDir = join(tempDir, "mocked-wan-provider");
    const fakeCargo = join(tempDir, "cargo");
    writeFileSync(
      fakeCargo,
      "#!/bin/sh\necho cargo should not run >&2\nexit 42\n",
      { mode: 0o755 },
    );

    const result = spawnSync(
      process.execPath,
      [
        join(repoRoot, "scripts/provider-e2e.mjs"),
        "--provider",
        "fal.ai",
        "--scenario",
        "wan-image-to-video",
        "--out-dir",
        outDir,
      ],
      {
        cwd: repoRoot,
        encoding: "utf8",
        env: {
          ...process.env,
          PATH: `${tempDir}:${process.env.PATH ?? ""}`,
          FAL_KEY: "",
        },
      },
    );

    expect(result.status).toBe(0);
    expect(result.stderr).not.toContain("cargo should not run");
    const report = JSON.parse(result.stdout) as {
      ok: boolean;
      provider: string;
      model: string;
      scenario: string;
      credentialStatus: string;
      command: string | null;
      mockedProviderService: boolean;
      result: { projectDir: string; artifactPath: string; outputCount: number };
    };
    expect(report).toEqual(
      expect.objectContaining({
        ok: true,
        provider: "fal.ai",
        model: "fal-ai/wan/v2.7/image-to-video",
        scenario: "wan-image-to-video",
        credentialStatus: "not_checked",
        command: null,
        mockedProviderService: true,
      }),
    );
    expect(report.result.outputCount).toBe(1);
    expect(report.result.artifactPath.endsWith(".mp4")).toBe(true);
    const sidecar = JSON.parse(
      readFileSync(
        join(outDir, "generated", "provider-e2e-generated", "asset.json"),
        "utf8",
      ),
    ) as {
      references: {
        firstFrameMediaId: string;
        lastFrameMediaId: string;
        referenceAudioMediaRefs: string[];
        providerInputUrls: string[];
      };
      outputs: Array<{ relativePath: string; durationSeconds: number; fps: number }>;
    };
    expect(sidecar.references).toEqual(
      expect.objectContaining({
        firstFrameMediaId: "provider-e2e-first-frame",
        lastFrameMediaId: "provider-e2e-last-frame",
        referenceAudioMediaRefs: ["provider-e2e-audio"],
      }),
    );
    expect(sidecar.references.providerInputUrls).toHaveLength(3);
    expect(sidecar.references.providerInputUrls.every((url) => url.startsWith("https://"))).toBe(true);
    expect(requiredAt(sidecar.outputs, 0, "provider sidecar output")).toEqual(
      expect.objectContaining({
        relativePath: "generated/provider-e2e-generated/fal-ai-mock-output.mp4",
        durationSeconds: 4,
        fps: 24,
      }),
    );
  });

  it("runs a mocked fal WAN reference-to-video scenario without credentials or cargo", () => {
    const tempDir = mkdtempSync(join(tmpdir(), "video-creater-provider-wan-reference-default-"));
    const outDir = join(tempDir, "mocked-wan-reference-provider");
    const fakeCargo = join(tempDir, "cargo");
    writeFileSync(
      fakeCargo,
      "#!/bin/sh\necho cargo should not run >&2\nexit 42\n",
      { mode: 0o755 },
    );

    const result = spawnSync(
      process.execPath,
      [
        join(repoRoot, "scripts/provider-e2e.mjs"),
        "--provider",
        "fal.ai",
        "--scenario",
        "wan-reference-to-video",
        "--out-dir",
        outDir,
      ],
      {
        cwd: repoRoot,
        encoding: "utf8",
        env: {
          ...process.env,
          PATH: `${tempDir}:${process.env.PATH ?? ""}`,
          FAL_KEY: "",
        },
      },
    );

    expect(result.status).toBe(0);
    expect(result.stderr).not.toContain("cargo should not run");
    const report = JSON.parse(result.stdout) as {
      ok: boolean;
      provider: string;
      model: string;
      scenario: string;
      credentialStatus: string;
      command: string | null;
      mockedProviderService: boolean;
      result: { projectDir: string; artifactPath: string; outputCount: number };
    };
    expect(report).toEqual(
      expect.objectContaining({
        ok: true,
        provider: "fal.ai",
        model: "fal-ai/wan/v2.7/reference-to-video",
        scenario: "wan-reference-to-video",
        credentialStatus: "not_checked",
        command: null,
        mockedProviderService: true,
      }),
    );
    expect(report.result.outputCount).toBe(1);
    expect(report.result.artifactPath.endsWith(".mp4")).toBe(true);
    const sidecar = JSON.parse(
      readFileSync(
        join(outDir, "generated", "provider-e2e-generated", "asset.json"),
        "utf8",
      ),
    ) as {
      settings?: {
        durationSeconds?: number;
        aspectRatio?: string;
        resolution?: string;
        generateAudio?: boolean;
      };
      references: {
        mediaIds: string[];
        referenceImageMediaRefs: string[];
        referenceVideoMediaRefs: string[];
        providerInputUrls: string[];
      };
      outputs: Array<{ relativePath: string; durationSeconds: number; fps: number }>;
    };
    expect(sidecar.references).toEqual(
      expect.objectContaining({
        mediaIds: ["provider-e2e-reference-image", "provider-e2e-reference-video"],
        referenceImageMediaRefs: ["provider-e2e-reference-image"],
        referenceVideoMediaRefs: ["provider-e2e-reference-video"],
      }),
    );
    expect(sidecar.settings).toEqual(
      expect.objectContaining({
        durationSeconds: 4,
        aspectRatio: "16:9",
        resolution: "720p",
        generateAudio: true,
      }),
    );
    expect(sidecar.references.providerInputUrls).toEqual([
      expect.stringMatching(/^https:\/\/v3\.fal\.media\/files\/provider-e2e\/reference-image\.png$/),
      expect.stringMatching(/^https:\/\/v3\.fal\.media\/files\/provider-e2e\/reference-video\.mp4$/),
    ]);
    expect(requiredAt(sidecar.outputs, 0, "provider sidecar output")).toEqual(
      expect.objectContaining({
        relativePath: "generated/provider-e2e-generated/fal-ai-mock-output.mp4",
        durationSeconds: 4,
        fps: 24,
      }),
    );
  });

  it("runs a mocked fal image-edit scenario without credentials or cargo", () => {
    const tempDir = mkdtempSync(join(tmpdir(), "video-creater-provider-image-edit-default-"));
    const outDir = join(tempDir, "mocked-image-edit-provider");
    const fakeCargo = join(tempDir, "cargo");
    writeFileSync(
      fakeCargo,
      "#!/bin/sh\necho cargo should not run >&2\nexit 42\n",
      { mode: 0o755 },
    );

    const result = spawnSync(
      process.execPath,
      [
        join(repoRoot, "scripts/provider-e2e.mjs"),
        "--provider",
        "fal.ai",
        "--scenario",
        "image-edit",
        "--out-dir",
        outDir,
      ],
      {
        cwd: repoRoot,
        encoding: "utf8",
        env: {
          ...process.env,
          PATH: `${tempDir}:${process.env.PATH ?? ""}`,
          FAL_KEY: "",
        },
      },
    );

    expect(result.status).toBe(0);
    expect(result.stderr).not.toContain("cargo should not run");
    const report = JSON.parse(result.stdout) as {
      ok: boolean;
      provider: string;
      model: string;
      scenario: string;
      credentialStatus: string;
      command: string | null;
      mockedProviderService: boolean;
      result: { artifactPath: string; outputCount: number };
    };
    expect(report).toEqual(
      expect.objectContaining({
        ok: true,
        provider: "fal.ai",
        model: "fal-ai/nano-banana-pro/edit",
        scenario: "image-edit",
        credentialStatus: "not_checked",
        command: null,
        mockedProviderService: true,
      }),
    );
    expect(report.result.outputCount).toBe(1);
    expect(report.result.artifactPath.endsWith(".png")).toBe(true);
    const sidecar = JSON.parse(
      readFileSync(
        join(outDir, "generated", "provider-e2e-generated", "asset.json"),
        "utf8",
      ),
    ) as {
      references: {
        mediaIds: string[];
        referenceImageMediaRefs: string[];
        providerInputUrls: string[];
      };
      outputs: Array<{
        relativePath: string;
        sourceUrl?: string;
        width: number;
        height: number;
      }>;
    };
    expect(sidecar.references).toEqual(
      expect.objectContaining({
        mediaIds: ["provider-e2e-image-ref"],
        referenceImageMediaRefs: ["provider-e2e-image-ref"],
        providerInputUrls: ["https://v3.fal.media/files/provider-e2e/image-ref.png"],
      }),
    );
    expect(requiredAt(sidecar.outputs, 0, "provider sidecar output")).toEqual(
      expect.objectContaining({
        relativePath: "generated/provider-e2e-generated/fal-ai-mock-output.png",
        width: 1024,
        height: 768,
      }),
    );
  });

  it("runs a mocked fal local image-edit replacement scenario without credentials or cargo", () => {
    const tempDir = mkdtempSync(join(tmpdir(), "video-creater-provider-image-edit-replace-default-"));
    const outDir = join(tempDir, "mocked-image-edit-replace-provider");
    const fakeCargo = join(tempDir, "cargo");
    writeFileSync(
      fakeCargo,
      "#!/bin/sh\necho cargo should not run >&2\nexit 42\n",
      { mode: 0o755 },
    );

    const result = spawnSync(
      process.execPath,
      [
        join(repoRoot, "scripts/provider-e2e.mjs"),
        "--provider",
        "fal.ai",
        "--scenario",
        "local-image-edit-replace",
        "--out-dir",
        outDir,
      ],
      {
        cwd: repoRoot,
        encoding: "utf8",
        env: {
          ...process.env,
          PATH: `${tempDir}:${process.env.PATH ?? ""}`,
          FAL_KEY: "",
        },
      },
    );

    expect(result.status).toBe(0);
    expect(result.stderr).not.toContain("cargo should not run");
    const report = JSON.parse(result.stdout) as {
      ok: boolean;
      provider: string;
      model: string;
      scenario: string;
      credentialStatus: string;
      command: string | null;
      mockedProviderService: boolean;
      result: { artifactPath: string; outputCount: number };
    };
    expect(report).toEqual(
      expect.objectContaining({
        ok: true,
        provider: "fal.ai",
        model: "fal-ai/nano-banana-pro/edit",
        scenario: "local-image-edit-replace",
        credentialStatus: "not_checked",
        command: null,
        mockedProviderService: true,
      }),
    );
    expect(report.result.outputCount).toBe(1);
    expect(report.result.artifactPath.endsWith(".png")).toBe(true);
    const sidecar = JSON.parse(
      readFileSync(
        join(outDir, "generated", "provider-e2e-generated", "asset.json"),
        "utf8",
      ),
    ) as {
      placementIntent: string;
      references: {
        mediaIds: string[];
        referenceImageMediaRefs: string[];
        providerInputUrls: string[];
      };
      outputs: Array<{
        mediaId: string;
        relativePath: string;
        sourceUrl?: string;
        width: number;
        height: number;
      }>;
    };
    expect(sidecar).toEqual(
      expect.objectContaining({
        placementIntent: "replace:item-image-edit-target",
      }),
    );
    expect(sidecar.references).toEqual(
      expect.objectContaining({
        mediaIds: ["provider-e2e-image-ref"],
        referenceImageMediaRefs: ["provider-e2e-image-ref"],
        providerInputUrls: ["https://v3.fal.media/files/provider-e2e/image-ref.png"],
      }),
    );
    expect(requiredAt(sidecar.outputs, 0, "provider sidecar output")).toEqual(
      expect.objectContaining({
        relativePath: "generated/provider-e2e-generated/fal-ai-mock-output.png",
        width: 1024,
        height: 768,
      }),
    );
    const timeline = JSON.parse(readFileSync(join(outDir, "timeline.json"), "utf8")) as {
      tracks: Array<{
        id: string;
        kind: string;
        items: Array<{
          id: string;
          source?: { type?: string; mediaId?: string };
          properties?: { generatedOutputMediaId?: string; replacementReason?: string };
        }>;
      }>;
    };
    const replacement = timeline.tracks
      .flatMap((track) => track.items)
      .find((item) => item.id === "item-image-edit-target");
    expect(replacement).toEqual(
      expect.objectContaining({
        source: { type: "media", mediaId: requiredAt(sidecar.outputs, 0, "provider sidecar output")?.mediaId },
        properties: expect.objectContaining({
          generatedOutputMediaId: requiredAt(sidecar.outputs, 0, "provider sidecar output")?.mediaId,
          replacementReason: "generated replacement",
        }),
      }),
    );
  });

  it.each([
    {
      provider: "openai",
      envVar: "OPENAI_API_KEY",
      model: "gpt-image-1.5",
      tempPrefix: "video-creater-provider-openai-image-edit-replace-",
      outputFile: "openai-mock-output.png",
      mediaIds: ["provider-e2e-image-ref", "provider-e2e-style-ref"],
      providerInputUrls: [
        "data:image/png;base64,bW9jay1vcGVuYWktaW1hZ2UtcmVm",
        "data:image/png;base64,bW9jay1vcGVuYWktc3R5bGUtcmVm",
      ],
    },
    {
      provider: "xai",
      envVar: "XAI_API_KEY",
      model: "grok-imagine-image-quality",
      tempPrefix: "video-creater-provider-xai-image-edit-replace-",
      outputFile: "xai-mock-output.png",
      mediaIds: ["provider-e2e-image-ref"],
      providerInputUrls: ["data:image/png;base64,bW9jay14YWktaW1hZ2UtcmVm"],
    },
  ])(
    "runs a mocked $provider local image-edit replacement scenario without credentials or cargo",
    ({ provider, envVar, model, tempPrefix, outputFile, mediaIds, providerInputUrls }) => {
      const tempDir = mkdtempSync(join(tmpdir(), tempPrefix));
      const outDir = join(tempDir, `mocked-${provider}-image-edit-replace-provider`);
      const fakeCargo = join(tempDir, "cargo");
      writeFileSync(
        fakeCargo,
        "#!/bin/sh\necho cargo should not run >&2\nexit 42\n",
        { mode: 0o755 },
      );

      const result = spawnSync(
        process.execPath,
        [
          join(repoRoot, "scripts/provider-e2e.mjs"),
          "--provider",
          provider,
          "--scenario",
          "local-image-edit-replace",
          "--out-dir",
          outDir,
        ],
        {
          cwd: repoRoot,
          encoding: "utf8",
          env: {
            ...process.env,
            PATH: `${tempDir}:${process.env.PATH ?? ""}`,
            [envVar]: "",
          },
        },
      );

      expect(result.status).toBe(0);
      expect(result.stderr).not.toContain("cargo should not run");
      const report = JSON.parse(result.stdout) as {
        ok: boolean;
        provider: string;
        model: string;
        scenario: string;
        credentialStatus: string;
        command: string | null;
        mockedProviderService: boolean;
        result: { artifactPath: string; outputCount: number };
      };
      expect(report).toEqual(
        expect.objectContaining({
          ok: true,
          provider,
          model,
          scenario: "local-image-edit-replace",
          credentialStatus: "not_checked",
          command: null,
          mockedProviderService: true,
        }),
      );
      expect(report.result.outputCount).toBe(1);
      expect(report.result.artifactPath.endsWith(".png")).toBe(true);
      const sidecar = JSON.parse(
        readFileSync(
          join(outDir, "generated", "provider-e2e-generated", "asset.json"),
          "utf8",
        ),
      ) as {
        placementIntent: string;
        references: {
          mediaIds: string[];
          referenceImageMediaRefs: string[];
          providerInputUrls: string[];
        };
        outputs: Array<{ mediaId: string; relativePath: string; sourceUrl?: string }>;
      };
      expect(sidecar).toEqual(
        expect.objectContaining({
          placementIntent: "replace:item-image-edit-target",
        }),
      );
      expect(sidecar.references).toEqual(
        expect.objectContaining({
          mediaIds,
          referenceImageMediaRefs: mediaIds,
          providerInputUrls,
        }),
      );
      expect(requiredAt(sidecar.outputs, 0, "provider sidecar output")).toEqual(
        expect.objectContaining({
          relativePath: `generated/provider-e2e-generated/${outputFile}`,
        }),
      );
      if (provider === "openai") {
        expect(requiredAt(sidecar.outputs, 0, "provider sidecar output")).not.toHaveProperty("sourceUrl");
      } else {
        expect(requiredAt(sidecar.outputs, 0, "provider sidecar output")?.sourceUrl).toBe(
          "https://mock.xai.example/generated/xai-mock-output.png",
        );
      }
      const timeline = JSON.parse(readFileSync(join(outDir, "timeline.json"), "utf8")) as {
        tracks: Array<{
          items: Array<{
            id: string;
            source?: { type?: string; mediaId?: string };
            properties?: { generatedOutputMediaId?: string };
          }>;
        }>;
      };
      const replacement = timeline.tracks
        .flatMap((track) => track.items)
        .find((item) => item.id === "item-image-edit-target");
      expect(replacement).toEqual(
        expect.objectContaining({
          source: { type: "media", mediaId: requiredAt(sidecar.outputs, 0, "provider sidecar output")?.mediaId },
          properties: expect.objectContaining({
            generatedOutputMediaId: requiredAt(sidecar.outputs, 0, "provider sidecar output")?.mediaId,
          }),
        }),
      );
    },
  );

  it.each([
    {
      scenario: "text-to-video-replace",
      tempPrefix: "video-creater-provider-fal-text-video-replace-",
      placementIntent: "replace:item-text-video-target",
      outputDirLabel: "mocked-fal-text-video-replace-provider",
    },
    {
      scenario: "text-to-video-insert",
      tempPrefix: "video-creater-provider-fal-text-video-insert-",
      placementIntent: "insert-video:track-video",
      outputDirLabel: "mocked-fal-text-video-insert-provider",
    },
  ])(
    "runs a mocked fal.ai $scenario scenario without credentials or cargo",
    ({ scenario, tempPrefix, placementIntent, outputDirLabel }) => {
      const tempDir = mkdtempSync(join(tmpdir(), tempPrefix));
      const outDir = join(tempDir, outputDirLabel);
      const fakeCargo = join(tempDir, "cargo");
      writeFileSync(
        fakeCargo,
        "#!/bin/sh\necho cargo should not run >&2\nexit 42\n",
        { mode: 0o755 },
      );

      const result = spawnSync(
        process.execPath,
        [
          join(repoRoot, "scripts/provider-e2e.mjs"),
          "--provider",
          "fal.ai",
          "--scenario",
          scenario,
          "--out-dir",
          outDir,
        ],
        {
          cwd: repoRoot,
          encoding: "utf8",
          env: {
            ...process.env,
            PATH: `${tempDir}:${process.env.PATH ?? ""}`,
            FAL_KEY: "",
          },
        },
      );

      expect(result.status).toBe(0);
      expect(result.stderr).not.toContain("cargo should not run");
      const report = JSON.parse(result.stdout) as {
        ok: boolean;
        provider: string;
        model: string;
        scenario: string;
        credentialStatus: string;
        command: string | null;
        mockedProviderService: boolean;
        result: { artifactPath: string; outputCount: number };
      };
      expect(report).toEqual(
        expect.objectContaining({
          ok: true,
          provider: "fal.ai",
          model: "fal-ai/wan-25-preview/text-to-video",
          scenario,
          credentialStatus: "not_checked",
          command: null,
          mockedProviderService: true,
        }),
      );
      expect(report.result.outputCount).toBe(1);
      expect(report.result.artifactPath.endsWith(".mp4")).toBe(true);
      const sidecar = JSON.parse(
        readFileSync(
          join(outDir, "generated", "provider-e2e-generated", "asset.json"),
          "utf8",
        ),
      ) as {
        placementIntent?: string;
        outputs: Array<{
          relativePath: string;
          sourceUrl?: string;
          durationSeconds: number;
          fps: number;
        }>;
      };
      expect(sidecar.placementIntent).toBe(placementIntent);
      expect(requiredAt(sidecar.outputs, 0, "provider sidecar output")).toEqual(
        expect.objectContaining({
          relativePath: "generated/provider-e2e-generated/fal-ai-mock-output.mp4",
          durationSeconds: 5,
          fps: 24,
        }),
      );
      expect(requiredAt(sidecar.outputs, 0, "provider sidecar output").sourceUrl).toContain("mock.fal-ai.example");
    },
  );

  it("runs a mocked OpenAI image-edit scenario without credentials or cargo", () => {
    const tempDir = mkdtempSync(join(tmpdir(), "video-creater-provider-openai-image-edit-"));
    const outDir = join(tempDir, "mocked-openai-image-edit-provider");
    const fakeCargo = join(tempDir, "cargo");
    writeFileSync(
      fakeCargo,
      "#!/bin/sh\necho cargo should not run >&2\nexit 42\n",
      { mode: 0o755 },
    );

    const result = spawnSync(
      process.execPath,
      [
        join(repoRoot, "scripts/provider-e2e.mjs"),
        "--provider",
        "openai",
        "--scenario",
        "image-edit",
        "--out-dir",
        outDir,
      ],
      {
        cwd: repoRoot,
        encoding: "utf8",
        env: {
          ...process.env,
          PATH: `${tempDir}:${process.env.PATH ?? ""}`,
          OPENAI_API_KEY: "",
        },
      },
    );

    expect(result.status).toBe(0);
    expect(result.stderr).not.toContain("cargo should not run");
    const report = JSON.parse(result.stdout) as {
      ok: boolean;
      provider: string;
      model: string;
      scenario: string;
      credentialStatus: string;
      command: string | null;
      mockedProviderService: boolean;
      result: { artifactPath: string; outputCount: number };
    };
    expect(report).toEqual(
      expect.objectContaining({
        ok: true,
        provider: "openai",
        model: "gpt-image-1.5",
        scenario: "image-edit",
        credentialStatus: "not_checked",
        command: null,
        mockedProviderService: true,
      }),
    );
    expect(report.result.outputCount).toBe(1);
    expect(report.result.artifactPath.endsWith(".png")).toBe(true);
    const sidecar = JSON.parse(
      readFileSync(
        join(outDir, "generated", "provider-e2e-generated", "asset.json"),
        "utf8",
      ),
    ) as {
      references: {
        mediaIds: string[];
        referenceImageMediaRefs: string[];
        providerInputUrls: string[];
      };
      outputs: Array<{ relativePath: string; width: number; height: number }>;
    };
    expect(sidecar.references).toEqual(
      expect.objectContaining({
        mediaIds: ["provider-e2e-image-ref", "provider-e2e-style-ref"],
        referenceImageMediaRefs: ["provider-e2e-image-ref", "provider-e2e-style-ref"],
        providerInputUrls: [
          "data:image/png;base64,bW9jay1vcGVuYWktaW1hZ2UtcmVm",
          "data:image/png;base64,bW9jay1vcGVuYWktc3R5bGUtcmVm",
        ],
      }),
    );
    expect(requiredAt(sidecar.outputs, 0, "provider sidecar output")).toEqual(
      expect.objectContaining({
        relativePath: "generated/provider-e2e-generated/openai-mock-output.png",
        width: 1024,
        height: 768,
      }),
    );
    expect(requiredAt(sidecar.outputs, 0, "provider sidecar output")).not.toHaveProperty("sourceUrl");
  });

  it("runs a mocked xAI image-edit scenario without credentials or cargo", () => {
    const tempDir = mkdtempSync(join(tmpdir(), "video-creater-provider-xai-image-edit-"));
    const outDir = join(tempDir, "mocked-xai-image-edit-provider");
    const fakeCargo = join(tempDir, "cargo");
    writeFileSync(
      fakeCargo,
      "#!/bin/sh\necho cargo should not run >&2\nexit 42\n",
      { mode: 0o755 },
    );

    const result = spawnSync(
      process.execPath,
      [
        join(repoRoot, "scripts/provider-e2e.mjs"),
        "--provider",
        "xai",
        "--scenario",
        "image-edit",
        "--out-dir",
        outDir,
      ],
      {
        cwd: repoRoot,
        encoding: "utf8",
        env: {
          ...process.env,
          PATH: `${tempDir}:${process.env.PATH ?? ""}`,
          XAI_API_KEY: "",
        },
      },
    );

    expect(result.status).toBe(0);
    expect(result.stderr).not.toContain("cargo should not run");
    const report = JSON.parse(result.stdout) as {
      ok: boolean;
      provider: string;
      model: string;
      scenario: string;
      credentialStatus: string;
      command: string | null;
      mockedProviderService: boolean;
      result: { artifactPath: string; outputCount: number };
    };
    expect(report).toEqual(
      expect.objectContaining({
        ok: true,
        provider: "xai",
        model: "grok-imagine-image-quality",
        scenario: "image-edit",
        credentialStatus: "not_checked",
        command: null,
        mockedProviderService: true,
      }),
    );
    expect(report.result.outputCount).toBe(1);
    expect(report.result.artifactPath.endsWith(".png")).toBe(true);
    const sidecar = JSON.parse(
      readFileSync(
        join(outDir, "generated", "provider-e2e-generated", "asset.json"),
        "utf8",
      ),
    ) as {
      references: {
        mediaIds: string[];
        referenceImageMediaRefs: string[];
        providerInputUrls: string[];
      };
      outputs: Array<{ relativePath: string; sourceUrl?: string; width: number; height: number }>;
    };
    expect(sidecar.references).toEqual(
      expect.objectContaining({
        mediaIds: ["provider-e2e-image-ref"],
        referenceImageMediaRefs: ["provider-e2e-image-ref"],
        providerInputUrls: ["data:image/png;base64,bW9jay14YWktaW1hZ2UtcmVm"],
      }),
    );
    expect(requiredAt(sidecar.outputs, 0, "provider sidecar output")).toEqual(
      expect.objectContaining({
        relativePath: "generated/provider-e2e-generated/xai-mock-output.png",
        width: 1024,
        height: 768,
      }),
    );
    expect(requiredAt(sidecar.outputs, 0, "provider sidecar output").sourceUrl).toContain("mock.xai.example");
  });

  it("uses a scenario-specific default output directory for OpenAI image-edit evidence", () => {
    const tempDir = mkdtempSync(join(tmpdir(), "video-creater-provider-openai-image-edit-default-"));
    const fakeCargo = join(tempDir, "cargo");
    writeFileSync(
      fakeCargo,
      "#!/bin/sh\necho cargo should not run >&2\nexit 42\n",
      { mode: 0o755 },
    );

    const result = spawnSync(
      process.execPath,
      [
        join(repoRoot, "scripts/provider-e2e.mjs"),
        "--provider",
        "openai",
        "--scenario",
        "image-edit",
      ],
      {
        cwd: repoRoot,
        encoding: "utf8",
        env: {
          ...process.env,
          PATH: `${tempDir}:${process.env.PATH ?? ""}`,
          OPENAI_API_KEY: "",
        },
      },
    );

    expect(result.status).toBe(0);
    expect(result.stderr).not.toContain("cargo should not run");
    const report = JSON.parse(result.stdout) as {
      outDir: string;
      result: { projectDir: string; artifactPath: string };
    };
    expect(report.outDir).toBe("output/provider-e2e/openai-image-edit");
    expect(report.result.projectDir).toBe("output/provider-e2e/openai-image-edit");
    expect(report.result.artifactPath).toContain("output/provider-e2e/openai-image-edit");
  });

  it("runs a mocked fal video-to-video scenario without credentials or cargo", () => {
    const tempDir = mkdtempSync(join(tmpdir(), "video-creater-provider-video-to-video-default-"));
    const outDir = join(tempDir, "mocked-video-to-video-provider");
    const fakeCargo = join(tempDir, "cargo");
    writeFileSync(
      fakeCargo,
      "#!/bin/sh\necho cargo should not run >&2\nexit 42\n",
      { mode: 0o755 },
    );

    const result = spawnSync(
      process.execPath,
      [
        join(repoRoot, "scripts/provider-e2e.mjs"),
        "--provider",
        "fal.ai",
        "--scenario",
        "video-to-video",
        "--out-dir",
        outDir,
      ],
      {
        cwd: repoRoot,
        encoding: "utf8",
        env: {
          ...process.env,
          PATH: `${tempDir}:${process.env.PATH ?? ""}`,
          FAL_KEY: "",
        },
      },
    );

    expect(result.status).toBe(0);
    expect(result.stderr).not.toContain("cargo should not run");
    const report = JSON.parse(result.stdout) as {
      ok: boolean;
      provider: string;
      model: string;
      scenario: string;
      credentialStatus: string;
      command: string | null;
      mockedProviderService: boolean;
      result: { artifactPath: string; outputCount: number };
    };
    expect(report).toEqual(
      expect.objectContaining({
        ok: true,
        provider: "fal.ai",
        model: "fal-ai/wan/v2.2-a14b/video-to-video",
        scenario: "video-to-video",
        credentialStatus: "not_checked",
        command: null,
        mockedProviderService: true,
      }),
    );
    expect(report.result.outputCount).toBe(1);
    expect(report.result.artifactPath.endsWith(".mp4")).toBe(true);
    const sidecar = JSON.parse(
      readFileSync(
        join(outDir, "generated", "provider-e2e-generated", "asset.json"),
        "utf8",
      ),
    ) as {
      references: {
        mediaIds: string[];
        sourceVideoMediaRef: string;
        referenceVideoMediaRefs: string[];
        providerInputUrls: string[];
      };
      outputs: Array<{ relativePath: string; durationSeconds: number; width: number; height: number; fps: number }>;
    };
    expect(sidecar.references).toEqual(
      expect.objectContaining({
        mediaIds: ["provider-e2e-video-source"],
        sourceVideoMediaRef: "provider-e2e-video-source",
        referenceVideoMediaRefs: ["provider-e2e-video-source"],
        providerInputUrls: ["https://v3.fal.media/files/provider-e2e/source-video.mp4"],
      }),
    );
    expect(requiredAt(sidecar.outputs, 0, "provider sidecar output")).toEqual(
      expect.objectContaining({
        relativePath: "generated/provider-e2e-generated/fal-ai-mock-output.mp4",
        durationSeconds: 4,
        width: 1280,
        height: 720,
        fps: 24,
      }),
    );
  });

  it("runs a mocked fal local video edit replacement scenario without credentials or cargo", () => {
    const tempDir = mkdtempSync(join(tmpdir(), "video-creater-provider-video-edit-replace-default-"));
    const outDir = join(tempDir, "mocked-video-edit-replace-provider");
    const fakeCargo = join(tempDir, "cargo");
    writeFileSync(
      fakeCargo,
      "#!/bin/sh\necho cargo should not run >&2\nexit 42\n",
      { mode: 0o755 },
    );

    const result = spawnSync(
      process.execPath,
      [
        join(repoRoot, "scripts/provider-e2e.mjs"),
        "--provider",
        "fal.ai",
        "--scenario",
        "local-video-edit-replace",
        "--out-dir",
        outDir,
      ],
      {
        cwd: repoRoot,
        encoding: "utf8",
        env: {
          ...process.env,
          PATH: `${tempDir}:${process.env.PATH ?? ""}`,
          FAL_KEY: "",
        },
      },
    );

    expect(result.status).toBe(0);
    expect(result.stderr).not.toContain("cargo should not run");
    const report = JSON.parse(result.stdout) as {
      ok: boolean;
      provider: string;
      model: string;
      scenario: string;
      credentialStatus: string;
      command: string | null;
      mockedProviderService: boolean;
      result: { artifactPath: string; outputCount: number };
    };
    expect(report).toEqual(
      expect.objectContaining({
        ok: true,
        provider: "fal.ai",
        model: "fal-ai/wan/v2.2-a14b/video-to-video",
        scenario: "local-video-edit-replace",
        credentialStatus: "not_checked",
        command: null,
        mockedProviderService: true,
      }),
    );
    expect(report.result.outputCount).toBe(1);
    expect(report.result.artifactPath.endsWith(".mp4")).toBe(true);
    const sidecar = JSON.parse(
      readFileSync(
        join(outDir, "generated", "provider-e2e-generated", "asset.json"),
        "utf8",
      ),
    ) as {
      prompt: string;
      placementIntent: string;
      references: {
        mediaIds: string[];
        sourceVideoMediaRef: string;
        referenceVideoMediaRefs: string[];
        providerInputUrls: string[];
      };
      settings: {
        videoSourceStartSeconds: number;
        videoSourceEndSeconds: number;
      };
      outputs: Array<{ mediaId: string; relativePath: string; durationSeconds: number; width: number; height: number; fps: number }>;
    };
    expect(sidecar).toEqual(
      expect.objectContaining({
        placementIntent: "replace:item-video-edit-target",
      }),
    );
    expect(sidecar.references).toEqual(
      expect.objectContaining({
        mediaIds: ["provider-e2e-video-source"],
        sourceVideoMediaRef: "provider-e2e-video-source",
        referenceVideoMediaRefs: ["provider-e2e-video-source"],
        providerInputUrls: ["https://v3.fal.media/files/provider-e2e/source-video.mp4"],
      }),
    );
    expect(sidecar.settings).toEqual(
      expect.objectContaining({
        videoSourceStartSeconds: 0.5,
        videoSourceEndSeconds: 3.5,
      }),
    );
    expect(requiredAt(sidecar.outputs, 0, "provider sidecar output")).toEqual(
      expect.objectContaining({
        relativePath: "generated/provider-e2e-generated/fal-ai-mock-output.mp4",
        durationSeconds: 3,
        width: 1280,
        height: 720,
        fps: 24,
      }),
    );
    const timeline = JSON.parse(
      readFileSync(join(outDir, "timeline.json"), "utf8"),
    ) as {
      tracks: Array<{
        items: Array<{
          id: string;
          source?: { type?: string; mediaId?: string };
          properties?: { generatedOutputMediaId?: string; sourceOut?: number };
          durationSeconds?: number;
        }>;
      }>;
    };
    const outputMediaId = requiredAt(sidecar.outputs, 0, "provider sidecar output")?.mediaId;
    const replacementItem = timeline.tracks
      .flatMap((track) => track.items)
      .find((item) => item.id === "item-video-edit-target");
    expect(replacementItem).toEqual(
      expect.objectContaining({
        source: { type: "media", mediaId: outputMediaId },
        properties: expect.objectContaining({
          generatedOutputMediaId: outputMediaId,
          sourceOut: 3,
        }),
        durationSeconds: 3,
      }),
    );
    const linkedReplacementItem = timeline.tracks
      .flatMap((track) => track.items)
      .find((item) => item.id === "item-video-edit-target-linked");
    expect(linkedReplacementItem).toEqual(
      expect.objectContaining({
        source: { type: "media", mediaId: outputMediaId },
        properties: expect.objectContaining({
          generatedOutputMediaId: outputMediaId,
          sourceOut: 2,
        }),
        durationSeconds: 2,
      }),
    );
  });

  it("runs a mocked fal local video upscale scenario without credentials or cargo", () => {
    const tempDir = mkdtempSync(join(tmpdir(), "video-creater-provider-video-upscale-default-"));
    const outDir = join(tempDir, "mocked-video-upscale-provider");
    const fakeCargo = join(tempDir, "cargo");
    writeFileSync(
      fakeCargo,
      "#!/bin/sh\necho cargo should not run >&2\nexit 42\n",
      { mode: 0o755 },
    );

    const result = spawnSync(
      process.execPath,
      [
        join(repoRoot, "scripts/provider-e2e.mjs"),
        "--provider",
        "fal.ai",
        "--scenario",
        "local-video-upscale",
        "--out-dir",
        outDir,
      ],
      {
        cwd: repoRoot,
        encoding: "utf8",
        env: {
          ...process.env,
          PATH: `${tempDir}:${process.env.PATH ?? ""}`,
          FAL_KEY: "",
        },
      },
    );

    expect(result.status).toBe(0);
    expect(result.stderr).not.toContain("cargo should not run");
    const report = JSON.parse(result.stdout) as {
      ok: boolean;
      provider: string;
      model: string;
      scenario: string;
      credentialStatus: string;
      command: string | null;
      mockedProviderService: boolean;
      result: { artifactPath: string; outputCount: number };
    };
    expect(report).toEqual(
      expect.objectContaining({
        ok: true,
        provider: "fal.ai",
        model: "fal-ai/video-upscaler",
        scenario: "local-video-upscale",
        credentialStatus: "not_checked",
        command: null,
        mockedProviderService: true,
      }),
    );
    expect(report.result.outputCount).toBe(1);
    expect(report.result.artifactPath.endsWith(".mp4")).toBe(true);
    const sidecar = JSON.parse(
      readFileSync(
        join(outDir, "generated", "provider-e2e-generated", "asset.json"),
        "utf8",
      ),
    ) as {
      references: {
        mediaIds: string[];
        sourceVideoMediaRef: string;
        referenceVideoMediaRefs: string[];
        providerInputUrls: string[];
      };
      outputs: Array<{ relativePath: string; durationSeconds: number; width: number; height: number; fps: number }>;
    };
    expect(sidecar.references).toEqual(
      expect.objectContaining({
        mediaIds: ["provider-e2e-video-source"],
        sourceVideoMediaRef: "provider-e2e-video-source",
        referenceVideoMediaRefs: ["provider-e2e-video-source"],
        providerInputUrls: ["https://v3.fal.media/files/provider-e2e/source-video.mp4"],
      }),
    );
    expect(requiredAt(sidecar.outputs, 0, "provider sidecar output")).toEqual(
      expect.objectContaining({
        relativePath: "generated/provider-e2e-generated/fal-ai-mock-output.mp4",
        durationSeconds: 4,
        width: 1280,
        height: 720,
        fps: 24,
      }),
    );
  });

  it("runs a mocked fal local video upscale replacement scenario without credentials or cargo", () => {
    const tempDir = mkdtempSync(join(tmpdir(), "video-creater-provider-video-upscale-replace-default-"));
    const outDir = join(tempDir, "mocked-video-upscale-replace-provider");
    const fakeCargo = join(tempDir, "cargo");
    writeFileSync(
      fakeCargo,
      "#!/bin/sh\necho cargo should not run >&2\nexit 42\n",
      { mode: 0o755 },
    );

    const result = spawnSync(
      process.execPath,
      [
        join(repoRoot, "scripts/provider-e2e.mjs"),
        "--provider",
        "fal.ai",
        "--scenario",
        "local-video-upscale-replace",
        "--out-dir",
        outDir,
      ],
      {
        cwd: repoRoot,
        encoding: "utf8",
        env: {
          ...process.env,
          PATH: `${tempDir}:${process.env.PATH ?? ""}`,
          FAL_KEY: "",
        },
      },
    );

    expect(result.status).toBe(0);
    expect(result.stderr).not.toContain("cargo should not run");
    const report = JSON.parse(result.stdout) as {
      ok: boolean;
      provider: string;
      model: string;
      scenario: string;
      credentialStatus: string;
      command: string | null;
      mockedProviderService: boolean;
      result: { artifactPath: string; outputCount: number };
    };
    expect(report).toEqual(
      expect.objectContaining({
        ok: true,
        provider: "fal.ai",
        model: "fal-ai/video-upscaler",
        scenario: "local-video-upscale-replace",
        credentialStatus: "not_checked",
        command: null,
        mockedProviderService: true,
      }),
    );
    expect(report.result.outputCount).toBe(1);
    expect(report.result.artifactPath.endsWith(".mp4")).toBe(true);
    const sidecar = JSON.parse(
      readFileSync(
        join(outDir, "generated", "provider-e2e-generated", "asset.json"),
        "utf8",
      ),
    ) as {
      placementIntent: string;
      references: {
        mediaIds: string[];
        sourceVideoMediaRef: string;
        referenceVideoMediaRefs: string[];
        providerInputUrls: string[];
      };
      outputs: Array<{ mediaId: string; relativePath: string; durationSeconds: number; width: number; height: number; fps: number }>;
    };
    expect(sidecar).toEqual(
      expect.objectContaining({
        placementIntent: "replace:item-video-upscale-target",
      }),
    );
    expect(sidecar.references).toEqual(
      expect.objectContaining({
        mediaIds: ["provider-e2e-video-source"],
        sourceVideoMediaRef: "provider-e2e-video-source",
        referenceVideoMediaRefs: ["provider-e2e-video-source"],
        providerInputUrls: ["https://v3.fal.media/files/provider-e2e/source-video.mp4"],
      }),
    );
    expect(requiredAt(sidecar.outputs, 0, "provider sidecar output")).toEqual(
      expect.objectContaining({
        relativePath: "generated/provider-e2e-generated/fal-ai-mock-output.mp4",
        durationSeconds: 4,
        width: 1280,
        height: 720,
        fps: 24,
      }),
    );
    const timeline = JSON.parse(readFileSync(join(outDir, "timeline.json"), "utf8")) as {
      tracks: Array<{
        items: Array<{
          id: string;
          source?: { type?: string; mediaId?: string };
          properties?: { generatedOutputMediaId?: string; sourceOut?: number };
          durationSeconds?: number;
        }>;
      }>;
    };
    const outputMediaId = requiredAt(sidecar.outputs, 0, "provider sidecar output")?.mediaId;
    const replacementItem = timeline.tracks
      .flatMap((track) => track.items)
      .find((item) => item.id === "item-video-upscale-target");
    expect(replacementItem).toEqual(
      expect.objectContaining({
        source: { type: "media", mediaId: outputMediaId },
        properties: expect.objectContaining({
          generatedOutputMediaId: outputMediaId,
          sourceOut: 4,
        }),
        durationSeconds: 4,
      }),
    );
    const linkedReplacementItem = timeline.tracks
      .flatMap((track) => track.items)
      .find((item) => item.id === "item-video-upscale-target-linked");
    expect(linkedReplacementItem).toEqual(
      expect.objectContaining({
        source: { type: "media", mediaId: outputMediaId },
        properties: expect.objectContaining({
          generatedOutputMediaId: outputMediaId,
          sourceOut: 3,
        }),
        durationSeconds: 3,
      }),
    );
  });

  it("runs a mocked fal local video upscale retry scenario without credentials or cargo", () => {
    const tempDir = mkdtempSync(join(tmpdir(), "video-creater-provider-video-upscale-retry-default-"));
    const outDir = join(tempDir, "mocked-video-upscale-retry-provider");
    const fakeCargo = join(tempDir, "cargo");
    writeFileSync(
      fakeCargo,
      "#!/bin/sh\necho cargo should not run >&2\nexit 42\n",
      { mode: 0o755 },
    );

    const result = spawnSync(
      process.execPath,
      [
        join(repoRoot, "scripts/provider-e2e.mjs"),
        "--provider",
        "fal.ai",
        "--scenario",
        "local-video-upscale-retry",
        "--out-dir",
        outDir,
      ],
      {
        cwd: repoRoot,
        encoding: "utf8",
        env: {
          ...process.env,
          PATH: `${tempDir}:${process.env.PATH ?? ""}`,
          FAL_KEY: "",
        },
      },
    );

    expect(result.status).toBe(0);
    expect(result.stderr).not.toContain("cargo should not run");
    const report = JSON.parse(result.stdout) as {
      ok: boolean;
      provider: string;
      model: string;
      scenario: string;
      credentialStatus: string;
      command: string | null;
      mockedProviderService: boolean;
      result: { artifactPath: string; outputCount: number };
    };
    expect(report).toEqual(
      expect.objectContaining({
        ok: true,
        provider: "fal.ai",
        model: "fal-ai/video-upscaler",
        scenario: "local-video-upscale-retry",
        credentialStatus: "not_checked",
        command: null,
        mockedProviderService: true,
      }),
    );
    expect(report.result.outputCount).toBe(1);
    expect(report.result.artifactPath.endsWith(".mp4")).toBe(true);
    const generatedIndex = JSON.parse(
      readFileSync(join(outDir, "generated", "index.json"), "utf8"),
    ) as {
      assets: Array<{ assetId: string; status: string; outputCount: number; path: string }>;
    };
    const completed = generatedIndex.assets.find((asset) => asset.status === "completed");
    const failed = generatedIndex.assets.find((asset) => asset.status === "failed");
    expect(completed).toEqual(expect.objectContaining({ outputCount: 1 }));
    expect(failed).toEqual(expect.objectContaining({ outputCount: 0 }));
    const completedSidecar = JSON.parse(
      readFileSync(join(outDir, completed?.path ?? ""), "utf8"),
    ) as {
      retryOfAssetId: string;
      references: {
        mediaIds: string[];
        sourceVideoMediaRef: string;
        referenceVideoMediaRefs: string[];
        providerInputUrls: string[];
      };
      settings: {
        videoSourceStartSeconds: number;
        videoSourceEndSeconds: number;
      };
      outputs: Array<{ relativePath: string; durationSeconds: number; fps: number }>;
    };
    expect(completedSidecar.retryOfAssetId).toBe(failed?.assetId);
    expect(completedSidecar.references).toEqual(
      expect.objectContaining({
        mediaIds: ["provider-e2e-video-source"],
        sourceVideoMediaRef: "provider-e2e-video-source",
        referenceVideoMediaRefs: ["provider-e2e-video-source"],
        providerInputUrls: ["https://v3.fal.media/files/provider-e2e/source-video.mp4"],
      }),
    );
    expect(completedSidecar.settings).toEqual(
      expect.objectContaining({
        videoSourceStartSeconds: 0.5,
        videoSourceEndSeconds: 4.5,
      }),
    );
    expect(completedSidecar.outputs[0]).toEqual(
      expect.objectContaining({
        relativePath: "generated/provider-e2e-generated/fal-ai-mock-output.mp4",
        durationSeconds: 4,
        fps: 24,
      }),
    );
    const failedSidecar = JSON.parse(
      readFileSync(join(outDir, failed?.path ?? ""), "utf8"),
    ) as { id: string; status: string; outputs: unknown[]; failureReason: string };
    expect(failedSidecar).toEqual(
      expect.objectContaining({
        id: failed?.assetId,
        status: "failed",
        outputs: [],
        failureReason: "provider_error",
      }),
    );
  });

  it("runs a mocked fal local image upscale retry scenario without credentials or cargo", () => {
    const tempDir = mkdtempSync(join(tmpdir(), "video-creater-provider-image-upscale-retry-default-"));
    const outDir = join(tempDir, "mocked-image-upscale-retry-provider");
    const fakeCargo = join(tempDir, "cargo");
    writeFileSync(
      fakeCargo,
      "#!/bin/sh\necho cargo should not run >&2\nexit 42\n",
      { mode: 0o755 },
    );

    const result = spawnSync(
      process.execPath,
      [
        join(repoRoot, "scripts/provider-e2e.mjs"),
        "--provider",
        "fal.ai",
        "--scenario",
        "local-image-upscale-retry",
        "--out-dir",
        outDir,
      ],
      {
        cwd: repoRoot,
        encoding: "utf8",
        env: {
          ...process.env,
          PATH: `${tempDir}:${process.env.PATH ?? ""}`,
          FAL_KEY: "",
        },
      },
    );

    expect(result.status).toBe(0);
    expect(result.stderr).not.toContain("cargo should not run");
    const report = JSON.parse(result.stdout) as {
      ok: boolean;
      provider: string;
      model: string;
      scenario: string;
      credentialStatus: string;
      command: string | null;
      mockedProviderService: boolean;
      result: { artifactPath: string; outputCount: number };
    };
    expect(report).toEqual(
      expect.objectContaining({
        ok: true,
        provider: "fal.ai",
        model: "fal-ai/aura-sr",
        scenario: "local-image-upscale-retry",
        credentialStatus: "not_checked",
        command: null,
        mockedProviderService: true,
      }),
    );
    expect(report.result.outputCount).toBe(1);
    expect(report.result.artifactPath.endsWith(".png")).toBe(true);
    const generatedIndex = JSON.parse(
      readFileSync(join(outDir, "generated", "index.json"), "utf8"),
    ) as {
      assets: Array<{ assetId: string; status: string; outputCount: number; path: string }>;
    };
    const completed = generatedIndex.assets.find((asset) => asset.status === "completed");
    const failed = generatedIndex.assets.find((asset) => asset.status === "failed");
    expect(completed).toEqual(expect.objectContaining({ outputCount: 1 }));
    expect(failed).toEqual(expect.objectContaining({ outputCount: 0 }));
    const completedSidecar = JSON.parse(
      readFileSync(join(outDir, completed?.path ?? ""), "utf8"),
    ) as {
      retryOfAssetId: string;
      references: {
        mediaIds: string[];
        providerInputUrls: string[];
      };
      outputs: Array<{ relativePath: string; width: number; height: number }>;
    };
    expect(completedSidecar.retryOfAssetId).toBe(failed?.assetId);
    expect(completedSidecar.references).toEqual(
      expect.objectContaining({
        mediaIds: ["provider-e2e-source"],
        providerInputUrls: ["https://v3.fal.media/files/provider-e2e/source-image.png"],
      }),
    );
    expect(completedSidecar.outputs[0]).toEqual(
      expect.objectContaining({
        relativePath: "generated/provider-e2e-generated/fal-ai-mock-output.png",
        width: 1024,
        height: 768,
      }),
    );
  });

  it("runs a mocked fal local image-edit retry scenario without credentials or cargo", () => {
    const tempDir = mkdtempSync(join(tmpdir(), "video-creater-provider-image-edit-retry-default-"));
    const outDir = join(tempDir, "mocked-image-edit-retry-provider");
    const fakeCargo = join(tempDir, "cargo");
    writeFileSync(
      fakeCargo,
      "#!/bin/sh\necho cargo should not run >&2\nexit 42\n",
      { mode: 0o755 },
    );

    const result = spawnSync(
      process.execPath,
      [
        join(repoRoot, "scripts/provider-e2e.mjs"),
        "--provider",
        "fal.ai",
        "--scenario",
        "local-image-edit-retry",
        "--out-dir",
        outDir,
      ],
      {
        cwd: repoRoot,
        encoding: "utf8",
        env: {
          ...process.env,
          PATH: `${tempDir}:${process.env.PATH ?? ""}`,
          FAL_KEY: "",
        },
      },
    );

    expect(result.status).toBe(0);
    expect(result.stderr).not.toContain("cargo should not run");
    const report = JSON.parse(result.stdout) as {
      ok: boolean;
      provider: string;
      model: string;
      scenario: string;
      credentialStatus: string;
      command: string | null;
      mockedProviderService: boolean;
      result: { artifactPath: string; outputCount: number };
    };
    expect(report).toEqual(
      expect.objectContaining({
        ok: true,
        provider: "fal.ai",
        model: "fal-ai/nano-banana-pro/edit",
        scenario: "local-image-edit-retry",
        credentialStatus: "not_checked",
        command: null,
        mockedProviderService: true,
      }),
    );
    expect(report.result.outputCount).toBe(1);
    expect(report.result.artifactPath.endsWith(".png")).toBe(true);
    const generatedIndex = JSON.parse(
      readFileSync(join(outDir, "generated", "index.json"), "utf8"),
    ) as {
      assets: Array<{ assetId: string; status: string; outputCount: number; path: string }>;
    };
    const completed = generatedIndex.assets.find((asset) => asset.status === "completed");
    const failed = generatedIndex.assets.find((asset) => asset.status === "failed");
    expect(completed).toEqual(expect.objectContaining({ outputCount: 1 }));
    expect(failed).toEqual(expect.objectContaining({ outputCount: 0 }));
    const completedSidecar = JSON.parse(
      readFileSync(join(outDir, completed?.path ?? ""), "utf8"),
    ) as {
      retryOfAssetId: string;
      references: {
        mediaIds: string[];
        referenceImageMediaRefs: string[];
        providerInputUrls: string[];
      };
      outputs: Array<{ relativePath: string; width: number; height: number }>;
    };
    expect(completedSidecar.retryOfAssetId).toBe(failed?.assetId);
    expect(completedSidecar.references).toEqual(
      expect.objectContaining({
        mediaIds: ["provider-e2e-image-ref"],
        referenceImageMediaRefs: ["provider-e2e-image-ref"],
        providerInputUrls: ["https://v3.fal.media/files/provider-e2e/image-ref.png"],
      }),
    );
    expect(completedSidecar.outputs[0]).toEqual(
      expect.objectContaining({
        relativePath: "generated/provider-e2e-generated/fal-ai-mock-output.png",
        width: 1024,
        height: 768,
      }),
    );
  });

  it("runs a mocked fal local video edit retry scenario without credentials or cargo", () => {
    const tempDir = mkdtempSync(join(tmpdir(), "video-creater-provider-video-edit-retry-default-"));
    const outDir = join(tempDir, "mocked-video-edit-retry-provider");
    const fakeCargo = join(tempDir, "cargo");
    writeFileSync(
      fakeCargo,
      "#!/bin/sh\necho cargo should not run >&2\nexit 42\n",
      { mode: 0o755 },
    );

    const result = spawnSync(
      process.execPath,
      [
        join(repoRoot, "scripts/provider-e2e.mjs"),
        "--provider",
        "fal.ai",
        "--scenario",
        "local-video-edit-retry",
        "--out-dir",
        outDir,
      ],
      {
        cwd: repoRoot,
        encoding: "utf8",
        env: {
          ...process.env,
          PATH: `${tempDir}:${process.env.PATH ?? ""}`,
          FAL_KEY: "",
        },
      },
    );

    expect(result.status).toBe(0);
    expect(result.stderr).not.toContain("cargo should not run");
    const report = JSON.parse(result.stdout) as {
      ok: boolean;
      provider: string;
      model: string;
      scenario: string;
      credentialStatus: string;
      command: string | null;
      mockedProviderService: boolean;
      result: { artifactPath: string; outputCount: number };
    };
    expect(report).toEqual(
      expect.objectContaining({
        ok: true,
        provider: "fal.ai",
        model: "fal-ai/wan/v2.2-a14b/video-to-video",
        scenario: "local-video-edit-retry",
        credentialStatus: "not_checked",
        command: null,
        mockedProviderService: true,
      }),
    );
    expect(report.result.outputCount).toBe(1);
    expect(report.result.artifactPath.endsWith(".mp4")).toBe(true);
    const generatedIndex = JSON.parse(
      readFileSync(join(outDir, "generated", "index.json"), "utf8"),
    ) as {
      assets: Array<{ assetId: string; status: string; outputCount: number; path: string }>;
    };
    const completed = generatedIndex.assets.find((asset) => asset.status === "completed");
    const failed = generatedIndex.assets.find((asset) => asset.status === "failed");
    expect(completed).toEqual(expect.objectContaining({ outputCount: 1 }));
    expect(failed).toEqual(expect.objectContaining({ outputCount: 0 }));
    const completedSidecar = JSON.parse(
      readFileSync(join(outDir, completed?.path ?? ""), "utf8"),
    ) as {
      retryOfAssetId: string;
      references: {
        mediaIds: string[];
        sourceVideoMediaRef: string;
        referenceVideoMediaRefs: string[];
        providerInputUrls: string[];
      };
      outputs: Array<{ relativePath: string; durationSeconds: number; fps: number }>;
    };
    expect(completedSidecar.retryOfAssetId).toBe(failed?.assetId);
    expect(completedSidecar.references).toEqual(
      expect.objectContaining({
        mediaIds: ["provider-e2e-video-source"],
        sourceVideoMediaRef: "provider-e2e-video-source",
        referenceVideoMediaRefs: ["provider-e2e-video-source"],
        providerInputUrls: ["https://v3.fal.media/files/provider-e2e/source-video.mp4"],
      }),
    );
    expect(completedSidecar.outputs[0]).toEqual(
      expect.objectContaining({
        relativePath: "generated/provider-e2e-generated/fal-ai-mock-output.mp4",
        durationSeconds: 3,
        fps: 24,
      }),
    );
  });

  it("runs mocked fal local source cancellation scenarios without credentials or cargo", () => {
    const scenarios = [
      {
        scenario: "local-image-upscale-cancellation",
        model: "fal-ai/aura-sr",
        outSuffix: "image-upscale-cancel",
        expectedReferences: {
          mediaIds: ["provider-e2e-source"],
          providerInputUrls: ["https://v3.fal.media/files/provider-e2e/source-image.png"],
        },
      },
      {
        scenario: "local-image-edit-cancellation",
        model: "fal-ai/nano-banana-pro/edit",
        outSuffix: "image-edit-cancel",
        expectedReferences: {
          mediaIds: ["provider-e2e-image-ref"],
          referenceImageMediaRefs: ["provider-e2e-image-ref"],
          providerInputUrls: ["https://v3.fal.media/files/provider-e2e/image-ref.png"],
        },
      },
      {
        scenario: "local-video-upscale-cancellation",
        model: "fal-ai/video-upscaler",
        outSuffix: "video-upscale-cancel",
        expectedReferences: {
          mediaIds: ["provider-e2e-video-source"],
          sourceVideoMediaRef: "provider-e2e-video-source",
          referenceVideoMediaRefs: ["provider-e2e-video-source"],
          providerInputUrls: ["https://v3.fal.media/files/provider-e2e/source-video.mp4"],
        },
      },
      {
        scenario: "local-video-edit-cancellation",
        model: "fal-ai/wan/v2.2-a14b/video-to-video",
        outSuffix: "video-edit-cancel",
        expectedReferences: {
          mediaIds: ["provider-e2e-video-source"],
          sourceVideoMediaRef: "provider-e2e-video-source",
          referenceVideoMediaRefs: ["provider-e2e-video-source"],
          providerInputUrls: ["https://v3.fal.media/files/provider-e2e/source-video.mp4"],
        },
        expectedSettings: {
          videoSourceStartSeconds: 0.5,
          videoSourceEndSeconds: 3.5,
        },
      },
    ];

    for (const { scenario, model, outSuffix, expectedReferences, expectedSettings } of scenarios) {
      const tempDir = mkdtempSync(join(tmpdir(), `video-creater-provider-${outSuffix}-default-`));
      const outDir = join(tempDir, `mocked-${outSuffix}-provider`);
      const fakeCargo = join(tempDir, "cargo");
      writeFileSync(
        fakeCargo,
        "#!/bin/sh\necho cargo should not run >&2\nexit 42\n",
        { mode: 0o755 },
      );

      const result = spawnSync(
        process.execPath,
        [
          join(repoRoot, "scripts/provider-e2e.mjs"),
          "--provider",
          "fal.ai",
          "--scenario",
          scenario,
          "--out-dir",
          outDir,
        ],
        {
          cwd: repoRoot,
          encoding: "utf8",
          env: {
            ...process.env,
            PATH: `${tempDir}:${process.env.PATH ?? ""}`,
            FAL_KEY: "",
          },
        },
      );

      expect(result.status).toBe(0);
      expect(result.stderr).not.toContain("cargo should not run");
      const report = JSON.parse(result.stdout) as {
        ok: boolean;
        provider: string;
        model: string;
        scenario: string;
        credentialStatus: string;
        command: string | null;
        mockedProviderService: boolean;
        result: {
          ok: boolean;
          artifactPath: string | null;
          outputCount: number;
          jobStatus: string;
          terminalReason: string;
        };
      };
      expect(report).toEqual(
        expect.objectContaining({
          ok: true,
          provider: "fal.ai",
          model,
          scenario,
          credentialStatus: "not_checked",
          command: null,
          mockedProviderService: true,
        }),
      );
      expect(report.result).toEqual(
        expect.objectContaining({
          ok: false,
          artifactPath: null,
          outputCount: 0,
          jobStatus: "Cancelled",
          terminalReason: "cancelled_by_user",
        }),
      );
      const generatedIndex = JSON.parse(
        readFileSync(join(outDir, "generated", "index.json"), "utf8"),
      ) as {
        assets: Array<{ assetId: string; status: string; outputCount: number; path: string }>;
      };
      const cancelled = generatedIndex.assets.find((asset) => asset.status === "failed");
      expect(cancelled).toEqual(expect.objectContaining({ outputCount: 0 }));
      const sidecar = JSON.parse(readFileSync(join(outDir, cancelled?.path ?? ""), "utf8")) as {
        status: string;
        failureReason: string;
        references?: Record<string, unknown>;
        settings?: Record<string, unknown>;
        outputs: unknown[];
      };
      expect(sidecar).toEqual(
        expect.objectContaining({
          status: "failed",
          failureReason: "cancelled_by_user",
          outputs: [],
        }),
      );
      expect(sidecar.references).toEqual(expect.objectContaining(expectedReferences));
      if (expectedSettings) {
        expect(sidecar.settings).toEqual(expect.objectContaining(expectedSettings));
      }
      const job = JSON.parse(
        readFileSync(join(outDir, "jobs", cancelled?.assetId ?? "", "job.json"), "utf8"),
      ) as { status: string; terminalReason: string; providerRequest?: { cancelUrl?: string } };
      expect(job).toEqual(
        expect.objectContaining({
          status: "cancelled",
          terminalReason: "cancelled_by_user",
        }),
      );
      expect(job.providerRequest?.cancelUrl).toContain("/cancel");
    }
  });

  it("runs mocked fal local source failure scenarios without credentials or cargo", () => {
    const scenarios = [
      {
        scenario: "local-image-upscale-failure",
        model: "fal-ai/aura-sr",
        outSuffix: "image-upscale-failure",
        expectedReferences: {
          mediaIds: ["provider-e2e-source"],
          providerInputUrls: ["https://v3.fal.media/files/provider-e2e/source-image.png"],
        },
      },
      {
        scenario: "local-image-edit-failure",
        model: "fal-ai/nano-banana-pro/edit",
        outSuffix: "image-edit-failure",
        expectedReferences: {
          mediaIds: ["provider-e2e-image-ref"],
          referenceImageMediaRefs: ["provider-e2e-image-ref"],
          providerInputUrls: ["https://v3.fal.media/files/provider-e2e/image-ref.png"],
        },
      },
      {
        scenario: "local-video-upscale-failure",
        model: "fal-ai/video-upscaler",
        outSuffix: "video-upscale-failure",
        expectedReferences: {
          mediaIds: ["provider-e2e-video-source"],
          sourceVideoMediaRef: "provider-e2e-video-source",
          referenceVideoMediaRefs: ["provider-e2e-video-source"],
          providerInputUrls: ["https://v3.fal.media/files/provider-e2e/source-video.mp4"],
        },
      },
      {
        scenario: "local-video-edit-failure",
        model: "fal-ai/wan/v2.2-a14b/video-to-video",
        outSuffix: "video-edit-failure",
        expectedReferences: {
          mediaIds: ["provider-e2e-video-source"],
          sourceVideoMediaRef: "provider-e2e-video-source",
          referenceVideoMediaRefs: ["provider-e2e-video-source"],
          providerInputUrls: ["https://v3.fal.media/files/provider-e2e/source-video.mp4"],
        },
        expectedSettings: {
          videoSourceStartSeconds: 0.5,
          videoSourceEndSeconds: 3.5,
        },
      },
    ];

    for (const { scenario, model, outSuffix, expectedReferences, expectedSettings } of scenarios) {
      const tempDir = mkdtempSync(join(tmpdir(), `video-creater-provider-${outSuffix}-default-`));
      const outDir = join(tempDir, `mocked-${outSuffix}-provider`);
      const fakeCargo = join(tempDir, "cargo");
      writeFileSync(
        fakeCargo,
        "#!/bin/sh\necho cargo should not run >&2\nexit 42\n",
        { mode: 0o755 },
      );

      const result = spawnSync(
        process.execPath,
        [
          join(repoRoot, "scripts/provider-e2e.mjs"),
          "--provider",
          "fal.ai",
          "--scenario",
          scenario,
          "--out-dir",
          outDir,
        ],
        {
          cwd: repoRoot,
          encoding: "utf8",
          env: {
            ...process.env,
            PATH: `${tempDir}:${process.env.PATH ?? ""}`,
            FAL_KEY: "",
          },
        },
      );

      expect(result.status).toBe(0);
      expect(result.stderr).not.toContain("cargo should not run");
      const report = JSON.parse(result.stdout) as {
        ok: boolean;
        provider: string;
        model: string;
        scenario: string;
        credentialStatus: string;
        command: string | null;
        mockedProviderService: boolean;
        result: {
          ok: boolean;
          artifactPath: string | null;
          outputCount: number;
          jobStatus: string;
          terminalReason: string;
        };
      };
      expect(report).toEqual(
        expect.objectContaining({
          ok: true,
          provider: "fal.ai",
          model,
          scenario,
          credentialStatus: "not_checked",
          command: null,
          mockedProviderService: true,
        }),
      );
      expect(report.result).toEqual(
        expect.objectContaining({
          ok: false,
          artifactPath: null,
          outputCount: 0,
          jobStatus: "Failed",
          terminalReason: "provider_error",
        }),
      );
      const generatedIndex = JSON.parse(
        readFileSync(join(outDir, "generated", "index.json"), "utf8"),
      ) as {
        assets: Array<{ assetId: string; status: string; outputCount: number; path: string }>;
      };
      const failed = generatedIndex.assets.find((asset) => asset.status === "failed");
      expect(failed).toEqual(expect.objectContaining({ outputCount: 0 }));
      const sidecar = JSON.parse(readFileSync(join(outDir, failed?.path ?? ""), "utf8")) as {
        status: string;
        failureReason: string;
        references?: Record<string, unknown>;
        settings?: Record<string, unknown>;
        outputs: unknown[];
      };
      expect(sidecar).toEqual(
        expect.objectContaining({
          status: "failed",
          failureReason: "provider_error",
          outputs: [],
        }),
      );
      expect(sidecar.references).toEqual(expect.objectContaining(expectedReferences));
      if (expectedSettings) {
        expect(sidecar.settings).toEqual(expect.objectContaining(expectedSettings));
      }
      const job = JSON.parse(
        readFileSync(join(outDir, "jobs", failed?.assetId ?? "", "job.json"), "utf8"),
      ) as { status: string; terminalReason: string; providerRequest?: { statusUrl?: string } };
      expect(job).toEqual(
        expect.objectContaining({
          status: "failed",
          terminalReason: "provider_error",
        }),
      );
      expect(job.providerRequest?.statusUrl).toContain("/v1/");
    }
  });

  it("runs a mocked Replicate Seedance video scenario without credentials or cargo", () => {
    const tempDir = mkdtempSync(join(tmpdir(), "video-creater-provider-replicate-video-default-"));
    const outDir = join(tempDir, "mocked-replicate-video-provider");
    const fakeCargo = join(tempDir, "cargo");
    writeFileSync(
      fakeCargo,
      "#!/bin/sh\necho cargo should not run >&2\nexit 42\n",
      { mode: 0o755 },
    );

    const result = spawnSync(
      process.execPath,
      [
        join(repoRoot, "scripts/provider-e2e.mjs"),
        "--provider",
        "replicate",
        "--scenario",
        "replicate-video",
        "--out-dir",
        outDir,
      ],
      {
        cwd: repoRoot,
        encoding: "utf8",
        env: {
          ...process.env,
          PATH: `${tempDir}:${process.env.PATH ?? ""}`,
          REPLICATE_API_TOKEN: "",
        },
      },
    );

    expect(result.status).toBe(0);
    expect(result.stderr).not.toContain("cargo should not run");
    const report = JSON.parse(result.stdout) as {
      ok: boolean;
      provider: string;
      model: string;
      scenario: string;
      credentialStatus: string;
      command: string | null;
      mockedProviderService: boolean;
      result: { artifactPath: string; outputCount: number };
    };
    expect(report).toEqual(
      expect.objectContaining({
        ok: true,
        provider: "replicate",
        model: "bytedance/seedance-2.0",
        scenario: "replicate-video",
        credentialStatus: "not_checked",
        command: null,
        mockedProviderService: true,
      }),
    );
    expect(report.result.outputCount).toBe(1);
    expect(report.result.artifactPath.endsWith(".mp4")).toBe(true);
    const sidecar = JSON.parse(
      readFileSync(
        join(outDir, "generated", "provider-e2e-generated", "asset.json"),
        "utf8",
      ),
    ) as {
      references: {
        firstFrameMediaId: string;
        lastFrameMediaId: string;
        referenceAudioMediaRefs: string[];
        providerInputUrls: string[];
      };
      outputs: Array<{ relativePath: string; durationSeconds: number; fps: number }>;
    };
    expect(sidecar.references).toEqual(
      expect.objectContaining({
        firstFrameMediaId: "provider-e2e-first-frame",
        lastFrameMediaId: "provider-e2e-last-frame",
        referenceAudioMediaRefs: ["provider-e2e-audio"],
      }),
    );
    expect(sidecar.references.providerInputUrls).toHaveLength(3);
    expect(requiredAt(sidecar.outputs, 0, "provider sidecar output")).toEqual(
      expect.objectContaining({
        relativePath: "generated/provider-e2e-generated/replicate-mock-output.mp4",
        durationSeconds: 4,
        fps: 24,
      }),
    );
  });

  it("runs a mocked Replicate Seedance video scenario with generated audio disabled", () => {
    const tempDir = mkdtempSync(join(tmpdir(), "video-creater-provider-replicate-video-no-audio-"));
    const outDir = join(tempDir, "mocked-replicate-video-no-audio-provider");
    const fakeCargo = join(tempDir, "cargo");
    writeFileSync(
      fakeCargo,
      "#!/bin/sh\necho cargo should not run >&2\nexit 42\n",
      { mode: 0o755 },
    );

    const result = spawnSync(
      process.execPath,
      [
        join(repoRoot, "scripts/provider-e2e.mjs"),
        "--provider",
        "replicate",
        "--scenario",
        "replicate-video",
        "--generate-audio",
        "false",
        "--out-dir",
        outDir,
      ],
      {
        cwd: repoRoot,
        encoding: "utf8",
        env: {
          ...process.env,
          PATH: `${tempDir}:${process.env.PATH ?? ""}`,
          REPLICATE_API_TOKEN: "",
          VIDEO_CREATER_PROVIDER_E2E_ALLOW_SPEND: "",
        },
      },
    );

    expect(result.status).toBe(0);
    expect(result.stderr).not.toContain("cargo should not run");
    const report = JSON.parse(result.stdout) as {
      credentialStatus: string;
      command: string | null;
      mockedProviderService: boolean;
    };
    expect(report).toEqual(
      expect.objectContaining({
        credentialStatus: "not_checked",
        command: null,
        mockedProviderService: true,
      }),
    );
    const sidecar = JSON.parse(
      readFileSync(
        join(outDir, "generated", "provider-e2e-generated", "asset.json"),
        "utf8",
      ),
    ) as { settings?: { generateAudio?: boolean } };
    expect(sidecar.settings?.generateAudio).toBe(false);
  });

  it("runs a mocked Replicate Seedance Fast video scenario without credentials or cargo", () => {
    const tempDir = mkdtempSync(join(tmpdir(), "video-creater-provider-replicate-fast-video-"));
    const outDir = join(tempDir, "mocked-replicate-fast-video-provider");
    const fakeCargo = join(tempDir, "cargo");
    writeFileSync(
      fakeCargo,
      "#!/bin/sh\necho cargo should not run >&2\nexit 42\n",
      { mode: 0o755 },
    );

    const result = spawnSync(
      process.execPath,
      [
        join(repoRoot, "scripts/provider-e2e.mjs"),
        "--provider",
        "replicate",
        "--scenario",
        "replicate-video-fast",
        "--out-dir",
        outDir,
      ],
      {
        cwd: repoRoot,
        encoding: "utf8",
        env: {
          ...process.env,
          PATH: `${tempDir}:${process.env.PATH ?? ""}`,
          REPLICATE_API_TOKEN: "",
        },
      },
    );

    expect(result.status).toBe(0);
    expect(result.stderr).not.toContain("cargo should not run");
    const report = JSON.parse(result.stdout) as {
      ok: boolean;
      provider: string;
      model: string;
      scenario: string;
      credentialStatus: string;
      command: string | null;
      mockedProviderService: boolean;
      result: { artifactPath: string; outputCount: number };
    };
    expect(report).toEqual(
      expect.objectContaining({
        ok: true,
        provider: "replicate",
        model: "bytedance/seedance-2.0-fast",
        scenario: "replicate-video-fast",
        credentialStatus: "not_checked",
        command: null,
        mockedProviderService: true,
      }),
    );
    expect(report.result.outputCount).toBe(1);
    expect(report.result.artifactPath.endsWith(".mp4")).toBe(true);
    const sidecar = JSON.parse(
      readFileSync(
        join(outDir, "generated", "provider-e2e-generated", "asset.json"),
        "utf8",
      ),
    ) as {
      outputs: Array<{ relativePath: string; durationSeconds: number; fps: number }>;
    };
    expect(requiredAt(sidecar.outputs, 0, "provider sidecar output")).toEqual(
      expect.objectContaining({
        relativePath: "generated/provider-e2e-generated/replicate-mock-output.mp4",
        durationSeconds: 4,
        fps: 24,
      }),
    );
  });

  it.each(replicateFluxVariantScenarios)(
    "runs a mocked Replicate $scenario image scenario without credentials or cargo",
    ({ scenario, model }) => {
      const tempDir = mkdtempSync(join(tmpdir(), `video-creater-provider-${scenario}-default-`));
      const outDir = join(tempDir, `mocked-${scenario}-provider`);
      const fakeCargo = join(tempDir, "cargo");
      writeFileSync(
        fakeCargo,
        "#!/bin/sh\necho cargo should not run >&2\nexit 42\n",
        { mode: 0o755 },
      );

      const result = spawnSync(
        process.execPath,
        [
          join(repoRoot, "scripts/provider-e2e.mjs"),
          "--provider",
          "replicate",
          "--scenario",
          scenario,
          "--out-dir",
          outDir,
        ],
        {
          cwd: repoRoot,
          encoding: "utf8",
          env: {
            ...process.env,
            PATH: `${tempDir}:${process.env.PATH ?? ""}`,
            REPLICATE_API_TOKEN: "",
          },
        },
      );

      expect(result.status).toBe(0);
      expect(result.stderr).not.toContain("cargo should not run");
      const report = JSON.parse(result.stdout) as {
        ok: boolean;
        provider: string;
        model: string;
        scenario: string;
        credentialStatus: string;
        command: string | null;
        mockedProviderService: boolean;
        result: { artifactPath: string; outputCount: number };
      };
      expect(report).toEqual(
        expect.objectContaining({
          ok: true,
          provider: "replicate",
          model,
          scenario,
          credentialStatus: "not_checked",
          command: null,
          mockedProviderService: true,
        }),
      );
      expect(report.result.outputCount).toBe(1);
      expect(report.result.artifactPath.endsWith(".png")).toBe(true);
      const sidecar = JSON.parse(
        readFileSync(
          join(outDir, "generated", "provider-e2e-generated", "asset.json"),
          "utf8",
        ),
      ) as {
        outputs: Array<{ relativePath: string; width: number; height: number }>;
      };
      expect(requiredAt(sidecar.outputs, 0, "provider sidecar output")).toEqual(
        expect.objectContaining({
          relativePath: "generated/provider-e2e-generated/replicate-mock-output.png",
          width: 1024,
          height: 768,
        }),
      );
    },
  );

  it("runs a mocked fal Krea text-to-image scenario without credentials or cargo", () => {
    const tempDir = mkdtempSync(join(tmpdir(), "video-creater-provider-krea-image-default-"));
    const outDir = join(tempDir, "mocked-krea-image-provider");
    const fakeCargo = join(tempDir, "cargo");
    writeFileSync(
      fakeCargo,
      "#!/bin/sh\necho cargo should not run >&2\nexit 42\n",
      { mode: 0o755 },
    );

    const result = spawnSync(
      process.execPath,
      [
        join(repoRoot, "scripts/provider-e2e.mjs"),
        "--provider",
        "fal.ai",
        "--scenario",
        "krea-text-to-image",
        "--out-dir",
        outDir,
      ],
      {
        cwd: repoRoot,
        encoding: "utf8",
        env: {
          ...process.env,
          PATH: `${tempDir}:${process.env.PATH ?? ""}`,
          FAL_KEY: "",
        },
      },
    );

    expect(result.status).toBe(0);
    expect(result.stderr).not.toContain("cargo should not run");
    const report = JSON.parse(result.stdout) as {
      ok: boolean;
      provider: string;
      model: string;
      scenario: string;
      credentialStatus: string;
      command: string | null;
      mockedProviderService: boolean;
      result: { artifactPath: string; outputCount: number };
    };
    expect(report).toEqual(
      expect.objectContaining({
        ok: true,
        provider: "fal.ai",
        model: "fal-ai/krea-2/turbo",
        scenario: "krea-text-to-image",
        credentialStatus: "not_checked",
        command: null,
        mockedProviderService: true,
      }),
    );
    expect(report.result.outputCount).toBe(1);
    expect(report.result.artifactPath.endsWith(".png")).toBe(true);
  });

  it("runs a mocked fal Recraft text-to-image scenario without credentials or cargo", () => {
    const tempDir = mkdtempSync(join(tmpdir(), "video-creater-provider-recraft-image-default-"));
    const outDir = join(tempDir, "mocked-recraft-image-provider");
    const fakeCargo = join(tempDir, "cargo");
    writeFileSync(
      fakeCargo,
      "#!/bin/sh\necho cargo should not run >&2\nexit 42\n",
      { mode: 0o755 },
    );

    const result = spawnSync(
      process.execPath,
      [
        join(repoRoot, "scripts/provider-e2e.mjs"),
        "--provider",
        "fal.ai",
        "--scenario",
        "recraft-text-to-image",
        "--out-dir",
        outDir,
      ],
      {
        cwd: repoRoot,
        encoding: "utf8",
        env: {
          ...process.env,
          PATH: `${tempDir}:${process.env.PATH ?? ""}`,
          FAL_KEY: "",
        },
      },
    );

    expect(result.status).toBe(0);
    expect(result.stderr).not.toContain("cargo should not run");
    const report = JSON.parse(result.stdout) as {
      ok: boolean;
      provider: string;
      model: string;
      scenario: string;
      credentialStatus: string;
      command: string | null;
      mockedProviderService: boolean;
      result: { artifactPath: string; outputCount: number };
    };
    expect(report).toEqual(
      expect.objectContaining({
        ok: true,
        provider: "fal.ai",
        model: "fal-ai/recraft/v3/text-to-image",
        scenario: "recraft-text-to-image",
        credentialStatus: "not_checked",
        command: null,
        mockedProviderService: true,
      }),
    );
    expect(report.result.outputCount).toBe(1);
    expect(report.result.artifactPath.endsWith(".webp")).toBe(true);
  });

  it("runs a mocked fal Kling image-to-video scenario without credentials or cargo", () => {
    const tempDir = mkdtempSync(join(tmpdir(), "video-creater-provider-kling-video-default-"));
    const outDir = join(tempDir, "mocked-kling-video-provider");
    const fakeCargo = join(tempDir, "cargo");
    writeFileSync(
      fakeCargo,
      "#!/bin/sh\necho cargo should not run >&2\nexit 42\n",
      { mode: 0o755 },
    );

    const result = spawnSync(
      process.execPath,
      [
        join(repoRoot, "scripts/provider-e2e.mjs"),
        "--provider",
        "fal.ai",
        "--scenario",
        "kling-image-to-video",
        "--out-dir",
        outDir,
      ],
      {
        cwd: repoRoot,
        encoding: "utf8",
        env: {
          ...process.env,
          PATH: `${tempDir}:${process.env.PATH ?? ""}`,
          FAL_KEY: "",
        },
      },
    );

    expect(result.status).toBe(0);
    expect(result.stderr).not.toContain("cargo should not run");
    const report = JSON.parse(result.stdout) as {
      ok: boolean;
      provider: string;
      model: string;
      scenario: string;
      credentialStatus: string;
      command: string | null;
      mockedProviderService: boolean;
      result: { artifactPath: string; outputCount: number };
    };
    expect(report).toEqual(
      expect.objectContaining({
        ok: true,
        provider: "fal.ai",
        model: "fal-ai/kling-video/v3/pro/image-to-video",
        scenario: "kling-image-to-video",
        credentialStatus: "not_checked",
        command: null,
        mockedProviderService: true,
      }),
    );
    expect(report.result.outputCount).toBe(1);
    expect(report.result.artifactPath.endsWith(".mp4")).toBe(true);
    const sidecar = JSON.parse(
      readFileSync(
        join(outDir, "generated", "provider-e2e-generated", "asset.json"),
        "utf8",
      ),
    ) as {
      references: {
        firstFrameMediaId: string;
        lastFrameMediaId: string;
        referenceAudioMediaRefs?: string[];
        providerInputUrls: string[];
      };
      outputs: Array<{ relativePath: string; durationSeconds: number; fps: number }>;
    };
    expect(sidecar.references).toEqual(
      expect.objectContaining({
        firstFrameMediaId: "provider-e2e-first-frame",
        lastFrameMediaId: "provider-e2e-last-frame",
      }),
    );
    expect(sidecar.references.referenceAudioMediaRefs).toBeUndefined();
    expect(sidecar.references.providerInputUrls).toHaveLength(2);
    expect(requiredAt(sidecar.outputs, 0, "provider sidecar output")).toEqual(
      expect.objectContaining({
        relativePath: "generated/provider-e2e-generated/fal-ai-mock-output.mp4",
        durationSeconds: 4,
        fps: 24,
      }),
    );
  });

  it("runs a mocked fal Kling Motion Control source-video edit scenario without credentials or cargo", () => {
    const tempDir = mkdtempSync(join(tmpdir(), "video-creater-provider-kling-motion-default-"));
    const outDir = join(tempDir, "mocked-kling-motion-provider");
    const fakeCargo = join(tempDir, "cargo");
    writeFileSync(
      fakeCargo,
      "#!/bin/sh\necho cargo should not run >&2\nexit 42\n",
      { mode: 0o755 },
    );

    const result = spawnSync(
      process.execPath,
      [
        join(repoRoot, "scripts/provider-e2e.mjs"),
        "--provider",
        "fal.ai",
        "--scenario",
        "local-video-motion-control-replace",
        "--out-dir",
        outDir,
      ],
      {
        cwd: repoRoot,
        encoding: "utf8",
        env: {
          ...process.env,
          PATH: `${tempDir}:${process.env.PATH ?? ""}`,
          FAL_KEY: "",
        },
      },
    );

    expect(result.status).toBe(0);
    expect(result.stderr).not.toContain("cargo should not run");
    const report = JSON.parse(result.stdout) as {
      ok: boolean;
      provider: string;
      model: string;
      scenario: string;
      credentialStatus: string;
      command: string | null;
      mockedProviderService: boolean;
      result: { artifactPath: string; outputCount: number };
    };
    expect(report).toEqual(
      expect.objectContaining({
        ok: true,
        provider: "fal.ai",
        model: "fal-ai/kling-video/v3/pro/motion-control",
        scenario: "local-video-motion-control-replace",
        credentialStatus: "not_checked",
        command: null,
        mockedProviderService: true,
      }),
    );
    expect(report.result.outputCount).toBe(1);
    expect(report.result.artifactPath.endsWith(".mp4")).toBe(true);
    const sidecar = JSON.parse(
      readFileSync(
        join(outDir, "generated", "provider-e2e-generated", "asset.json"),
        "utf8",
      ),
    ) as {
      references: {
        mediaIds: string[];
        sourceVideoMediaRef: string;
        referenceImageMediaRefs: string[];
        providerInputUrls: string[];
      };
      placementIntent: string;
      settings: {
        generateAudio: boolean;
        videoSourceStartSeconds: number;
        videoSourceEndSeconds: number;
      };
      outputs: Array<{ relativePath: string; durationSeconds: number; fps: number }>;
    };
    expect(sidecar.references).toEqual(
      expect.objectContaining({
        mediaIds: ["provider-e2e-video-source", "provider-e2e-motion-image-ref"],
        sourceVideoMediaRef: "provider-e2e-video-source",
        referenceImageMediaRefs: ["provider-e2e-motion-image-ref"],
        providerInputUrls: [
          "https://v3.fal.media/files/provider-e2e/source-video.mp4",
          "https://v3.fal.media/files/provider-e2e/motion-image-ref.png",
        ],
      }),
    );
    expect(sidecar.placementIntent).toBe("replace:item-video-motion-control-target");
    expect(sidecar.settings).toEqual(
      expect.objectContaining({
        generateAudio: true,
        videoSourceStartSeconds: 0.5,
        videoSourceEndSeconds: 3.5,
      }),
    );
    expect(requiredAt(sidecar.outputs, 0, "provider sidecar output")).toEqual(
      expect.objectContaining({
        relativePath: "generated/provider-e2e-generated/fal-ai-mock-output.mp4",
        durationSeconds: 3,
        fps: 24,
      }),
    );
  });

  it("runs a mocked fal text-to-audio scenario without credentials or cargo", () => {
    const tempDir = mkdtempSync(join(tmpdir(), "video-creater-provider-audio-default-"));
    const outDir = join(tempDir, "mocked-audio-provider");
    const fakeCargo = join(tempDir, "cargo");
    writeFileSync(
      fakeCargo,
      "#!/bin/sh\necho cargo should not run >&2\nexit 42\n",
      { mode: 0o755 },
    );

    const result = spawnSync(
      process.execPath,
      [
        join(repoRoot, "scripts/provider-e2e.mjs"),
        "--provider",
        "fal.ai",
        "--scenario",
        "text-to-audio",
        "--out-dir",
        outDir,
      ],
      {
        cwd: repoRoot,
        encoding: "utf8",
        env: {
          ...process.env,
          PATH: `${tempDir}:${process.env.PATH ?? ""}`,
          FAL_KEY: "",
        },
      },
    );

    expect(result.status).toBe(0);
    expect(result.stderr).not.toContain("cargo should not run");
    const report = JSON.parse(result.stdout) as {
      ok: boolean;
      provider: string;
      model: string;
      scenario: string;
      credentialStatus: string;
      command: string | null;
      mockedProviderService: boolean;
      result: { artifactPath: string; outputCount: number };
    };
    expect(report).toEqual(
      expect.objectContaining({
        ok: true,
        provider: "fal.ai",
        model: "bytedance/seed-audio-1.0",
        scenario: "text-to-audio",
        credentialStatus: "not_checked",
        command: null,
        mockedProviderService: true,
      }),
    );
    expect(report.result.outputCount).toBe(1);
    expect(report.result.artifactPath.endsWith(".mp3")).toBe(true);
    const sidecar = JSON.parse(
      readFileSync(
        join(outDir, "generated", "provider-e2e-generated", "asset.json"),
        "utf8",
      ),
    ) as {
      outputs: Array<{ relativePath: string; durationSeconds: number; width: number; height: number; fps: number }>;
    };
    expect(requiredAt(sidecar.outputs, 0, "provider sidecar output")).toEqual(
      expect.objectContaining({
        relativePath: "generated/provider-e2e-generated/fal-ai-mock-output.mp3",
        durationSeconds: 12,
        width: 1,
        height: 1,
        fps: 1,
      }),
    );
  });

  it("runs a mocked OpenAI text-to-audio scenario without credentials or cargo", () => {
    const tempDir = mkdtempSync(join(tmpdir(), "video-creater-provider-openai-audio-default-"));
    const outDir = join(tempDir, "mocked-openai-audio-provider");
    const fakeCargo = join(tempDir, "cargo");
    writeFileSync(
      fakeCargo,
      "#!/bin/sh\necho cargo should not run >&2\nexit 42\n",
      { mode: 0o755 },
    );

    const result = spawnSync(
      process.execPath,
      [
        join(repoRoot, "scripts/provider-e2e.mjs"),
        "--provider",
        "openai",
        "--scenario",
        "text-to-audio",
        "--out-dir",
        outDir,
      ],
      {
        cwd: repoRoot,
        encoding: "utf8",
        env: {
          ...process.env,
          PATH: `${tempDir}:${process.env.PATH ?? ""}`,
          OPENAI_API_KEY: "",
        },
      },
    );

    expect(result.status).toBe(0);
    expect(result.stderr).not.toContain("cargo should not run");
    const report = JSON.parse(result.stdout) as {
      ok: boolean;
      provider: string;
      model: string;
      scenario: string;
      credentialStatus: string;
      command: string | null;
      mockedProviderService: boolean;
      result: { artifactPath: string; outputCount: number };
    };
    expect(report).toEqual(
      expect.objectContaining({
        ok: true,
        provider: "openai",
        model: "gpt-4o-mini-tts",
        scenario: "text-to-audio",
        credentialStatus: "not_checked",
        command: null,
        mockedProviderService: true,
      }),
    );
    expect(report.result.outputCount).toBe(1);
    expect(report.result.artifactPath.endsWith(".mp3")).toBe(true);
    const sidecar = JSON.parse(
      readFileSync(
        join(outDir, "generated", "provider-e2e-generated", "asset.json"),
        "utf8",
      ),
    ) as {
      settings?: { category?: string; lyrics?: string; voice?: string };
      outputs: Array<{
        relativePath: string;
        sourceUrl?: string;
        durationSeconds: number;
        width: number;
        height: number;
        fps: number;
      }>;
    };
    expect(requiredAt(sidecar.outputs, 0, "provider sidecar output")).toEqual(
      expect.objectContaining({
        relativePath: "generated/provider-e2e-generated/openai-mock-output.mp3",
        durationSeconds: 12,
        width: 1,
        height: 1,
        fps: 1,
      }),
    );
    expect(sidecar.settings).toEqual(
      expect.objectContaining({
        category: "tts",
        voice: "alloy",
      }),
    );
    expect(requiredAt(sidecar.outputs, 0, "provider sidecar output")).not.toHaveProperty("sourceUrl");
  });

  it("runs a mocked ElevenLabs text-to-audio scenario without credentials or cargo", () => {
    const tempDir = mkdtempSync(join(tmpdir(), "video-creater-provider-elevenlabs-audio-default-"));
    const outDir = join(tempDir, "mocked-elevenlabs-audio-provider");
    const fakeCargo = join(tempDir, "cargo");
    writeFileSync(
      fakeCargo,
      "#!/bin/sh\necho cargo should not run >&2\nexit 42\n",
      { mode: 0o755 },
    );

    const result = spawnSync(
      process.execPath,
      [
        join(repoRoot, "scripts/provider-e2e.mjs"),
        "--provider",
        "elevenlabs",
        "--scenario",
        "text-to-audio",
        "--out-dir",
        outDir,
      ],
      {
        cwd: repoRoot,
        encoding: "utf8",
        env: {
          ...process.env,
          PATH: `${tempDir}:${process.env.PATH ?? ""}`,
          ELEVENLABS_API_KEY: "",
        },
      },
    );

    expect(result.status).toBe(0);
    expect(result.stderr).not.toContain("cargo should not run");
    const report = JSON.parse(result.stdout) as {
      ok: boolean;
      provider: string;
      model: string;
      scenario: string;
      credentialStatus: string;
      command: string | null;
      mockedProviderService: boolean;
      result: { artifactPath: string; outputCount: number };
    };
    expect(report).toEqual(
      expect.objectContaining({
        ok: true,
        provider: "elevenlabs",
        model: "elevenlabs-tts-v3",
        scenario: "text-to-audio",
        credentialStatus: "not_checked",
        command: null,
        mockedProviderService: true,
      }),
    );
    expect(report.result.outputCount).toBe(1);
    expect(report.result.artifactPath.endsWith(".mp3")).toBe(true);
    const sidecar = JSON.parse(
      readFileSync(
        join(outDir, "generated", "provider-e2e-generated", "asset.json"),
        "utf8",
      ),
    ) as {
      settings?: { category?: string; voice?: string };
      outputs: Array<{
        relativePath: string;
        sourceUrl?: string;
        durationSeconds: number;
        width: number;
        height: number;
        fps: number;
      }>;
    };
    expect(requiredAt(sidecar.outputs, 0, "provider sidecar output")).toEqual(
      expect.objectContaining({
        relativePath: "generated/provider-e2e-generated/elevenlabs-mock-output.mp3",
        durationSeconds: 12,
        width: 1,
        height: 1,
        fps: 1,
      }),
    );
    expect(sidecar.settings).toEqual(
      expect.objectContaining({
        category: "tts",
        voice: "rachel",
      }),
    );
    expect(requiredAt(sidecar.outputs, 0, "provider sidecar output")).not.toHaveProperty("sourceUrl");
  });

  it("runs a mocked ElevenLabs text-to-music scenario without credentials or cargo", () => {
    const tempDir = mkdtempSync(join(tmpdir(), "video-creater-provider-elevenlabs-music-default-"));
    const outDir = join(tempDir, "mocked-elevenlabs-music-provider");
    const fakeCargo = join(tempDir, "cargo");
    writeFileSync(
      fakeCargo,
      "#!/bin/sh\necho cargo should not run >&2\nexit 42\n",
      { mode: 0o755 },
    );

    const result = spawnSync(
      process.execPath,
      [
        join(repoRoot, "scripts/provider-e2e.mjs"),
        "--provider",
        "elevenlabs",
        "--scenario",
        "text-to-music",
        "--out-dir",
        outDir,
      ],
      {
        cwd: repoRoot,
        encoding: "utf8",
        env: {
          ...process.env,
          PATH: `${tempDir}:${process.env.PATH ?? ""}`,
          ELEVENLABS_API_KEY: "",
        },
      },
    );

    expect(result.status).toBe(0);
    expect(result.stderr).not.toContain("cargo should not run");
    const report = JSON.parse(result.stdout) as {
      ok: boolean;
      provider: string;
      model: string;
      scenario: string;
      credentialStatus: string;
      command: string | null;
      mockedProviderService: boolean;
      result: { artifactPath: string; outputCount: number };
    };
    expect(report).toEqual(
      expect.objectContaining({
        ok: true,
        provider: "elevenlabs",
        model: "elevenlabs-music",
        scenario: "text-to-music",
        credentialStatus: "not_checked",
        command: null,
        mockedProviderService: true,
      }),
    );
    expect(report.result.outputCount).toBe(1);
    expect(report.result.artifactPath.endsWith(".mp3")).toBe(true);
    const sidecar = JSON.parse(
      readFileSync(
        join(outDir, "generated", "provider-e2e-generated", "asset.json"),
        "utf8",
      ),
    ) as {
      settings?: { category?: string; voice?: string };
      outputs: Array<{
        relativePath: string;
        sourceUrl?: string;
        durationSeconds: number;
        width: number;
        height: number;
        fps: number;
      }>;
    };
    expect(requiredAt(sidecar.outputs, 0, "provider sidecar output")).toEqual(
      expect.objectContaining({
        relativePath: "generated/provider-e2e-generated/elevenlabs-mock-output.mp3",
        durationSeconds: 30,
        width: 1,
        height: 1,
        fps: 1,
      }),
    );
    expect(sidecar.settings).toEqual(
      expect.objectContaining({
        category: "music",
        durationSeconds: 30,
        instrumental: false,
        styleInstructions: "bright, commercial, loopable",
        lyrics: elevenlabsTextToMusicDefaultLyrics,
      }),
    );
    expect(requiredAt(sidecar.outputs, 0, "provider sidecar output")).not.toHaveProperty("sourceUrl");
  });

  it("runs a mocked fal Sonilo text-to-music scenario without credentials or cargo", () => {
    const tempDir = mkdtempSync(join(tmpdir(), "video-creater-provider-fal-music-default-"));
    const outDir = join(tempDir, "mocked-fal-music-provider");
    const fakeCargo = join(tempDir, "cargo");
    writeFileSync(
      fakeCargo,
      "#!/bin/sh\necho cargo should not run >&2\nexit 42\n",
      { mode: 0o755 },
    );

    const result = spawnSync(
      process.execPath,
      [
        join(repoRoot, "scripts/provider-e2e.mjs"),
        "--provider",
        "fal.ai",
        "--scenario",
        "text-to-music",
        "--out-dir",
        outDir,
      ],
      {
        cwd: repoRoot,
        encoding: "utf8",
        env: {
          ...process.env,
          PATH: `${tempDir}:${process.env.PATH ?? ""}`,
          FAL_KEY: "",
        },
      },
    );

    expect(result.status).toBe(0);
    expect(result.stderr).not.toContain("cargo should not run");
    const report = JSON.parse(result.stdout) as {
      ok: boolean;
      provider: string;
      model: string;
      scenario: string;
      credentialStatus: string;
      command: string | null;
      mockedProviderService: boolean;
      result: { artifactPath: string; outputCount: number };
    };
    expect(report).toEqual(
      expect.objectContaining({
        ok: true,
        provider: "fal.ai",
        model: "sonilo/v1.1/text-to-music",
        scenario: "text-to-music",
        credentialStatus: "not_checked",
        command: null,
        mockedProviderService: true,
      }),
    );
    expect(report.result.outputCount).toBe(1);
    expect(report.result.artifactPath.endsWith(".m4a")).toBe(true);
    const sidecar = JSON.parse(
      readFileSync(
        join(outDir, "generated", "provider-e2e-generated", "asset.json"),
        "utf8",
      ),
    ) as {
      settings?: { category?: string; voice?: string };
      outputs: Array<{
        relativePath: string;
        sourceUrl?: string;
        durationSeconds: number;
        width: number;
        height: number;
        fps: number;
      }>;
    };
    expect(requiredAt(sidecar.outputs, 0, "provider sidecar output")).toEqual(
      expect.objectContaining({
        relativePath: "generated/provider-e2e-generated/fal-ai-mock-output.m4a",
        durationSeconds: 30,
        width: 1,
        height: 1,
        fps: 1,
      }),
    );
    expect(sidecar.settings).toEqual(
      expect.objectContaining({
        category: "music",
        durationSeconds: 30,
        instrumental: false,
        styleInstructions: "bright, commercial, loopable",
      }),
    );
    expect(requiredAt(sidecar.outputs, 0, "provider sidecar output")).not.toHaveProperty("sourceUrl");
  });

  it("runs a mocked fal text-to-music timeline insert scenario without credentials or cargo", () => {
    const tempDir = mkdtempSync(join(tmpdir(), "video-creater-provider-fal-music-insert-"));
    const outDir = join(tempDir, "mocked-fal-music-insert-provider");
    const fakeCargo = join(tempDir, "cargo");
    writeFileSync(
      fakeCargo,
      "#!/bin/sh\necho cargo should not run >&2\nexit 42\n",
      { mode: 0o755 },
    );

    const result = spawnSync(
      process.execPath,
      [
        join(repoRoot, "scripts/provider-e2e.mjs"),
        "--provider",
        "fal.ai",
        "--scenario",
        "text-to-music-insert",
        "--out-dir",
        outDir,
      ],
      {
        cwd: repoRoot,
        encoding: "utf8",
        env: {
          ...process.env,
          PATH: `${tempDir}:${process.env.PATH ?? ""}`,
          FAL_KEY: "",
          VIDEO_CREATER_PROVIDER_E2E_ALLOW_SPEND: "",
        },
      },
    );

    expect(result.status).toBe(0);
    expect(result.stderr).not.toContain("cargo should not run");
    const report = JSON.parse(result.stdout) as {
      ok: boolean;
      provider: string;
      model: string;
      scenario: string;
      credentialStatus: string;
      command: string | null;
      mockedProviderService: boolean;
      result: { artifactPath: string; outputCount: number };
    };
    expect(report).toEqual(
      expect.objectContaining({
        ok: true,
        provider: "fal.ai",
        model: "sonilo/v1.1/text-to-music",
        scenario: "text-to-music-insert",
        credentialStatus: "not_checked",
        command: null,
        mockedProviderService: true,
      }),
    );
    expect(report.result.outputCount).toBe(1);
    expect(report.result.artifactPath.endsWith(".m4a")).toBe(true);
    const sidecar = JSON.parse(
      readFileSync(
        join(outDir, "generated", "provider-e2e-generated", "asset.json"),
        "utf8",
      ),
    ) as {
      placementIntent?: string;
      settings?: { timelineStartSeconds?: number; category?: string };
      outputs: Array<{ relativePath: string; durationSeconds: number }>;
    };
    expect(sidecar.placementIntent).toBe("insert-audio:track-audio");
    expect(sidecar.settings).toEqual(
      expect.objectContaining({
        category: "music",
        timelineStartSeconds: 12,
      }),
    );
    expect(requiredAt(sidecar.outputs, 0, "provider sidecar output")).toEqual(
      expect.objectContaining({
        relativePath: "generated/provider-e2e-generated/fal-ai-mock-output.m4a",
        durationSeconds: 30,
      }),
    );
    const timeline = JSON.parse(readFileSync(join(outDir, "timeline.json"), "utf8")) as {
      tracks: Array<{ id: string; items: Array<Record<string, unknown>> }>;
    };
    const audioItem = timeline.tracks
      .find((track) => track.id === "track-audio")
      ?.items.find((item) => item.id === "item-text-music-inserted-audio");
    expect(audioItem).toEqual(
      expect.objectContaining({
        kind: "audio_clip",
        startSeconds: 12,
        durationSeconds: 30,
        source: { type: "media", mediaId: "provider-e2e-output" },
        properties: expect.objectContaining({
          sourceIn: 0,
          sourceOut: 30,
          generatedOutputMediaId: "provider-e2e-output",
        }),
      }),
    );
  });

  it("runs a mocked Google Gemini TTS text-to-audio scenario without credentials or cargo", () => {
    const tempDir = mkdtempSync(join(tmpdir(), "video-creater-provider-google-audio-default-"));
    const outDir = join(tempDir, "mocked-google-audio-provider");
    const fakeCargo = join(tempDir, "cargo");
    writeFileSync(
      fakeCargo,
      "#!/bin/sh\necho cargo should not run >&2\nexit 42\n",
      { mode: 0o755 },
    );

    const result = spawnSync(
      process.execPath,
      [
        join(repoRoot, "scripts/provider-e2e.mjs"),
        "--provider",
        "google",
        "--scenario",
        "text-to-audio",
        "--out-dir",
        outDir,
      ],
      {
        cwd: repoRoot,
        encoding: "utf8",
        env: {
          ...process.env,
          PATH: `${tempDir}:${process.env.PATH ?? ""}`,
          GEMINI_API_KEY: "",
        },
      },
    );

    expect(result.status).toBe(0);
    expect(result.stderr).not.toContain("cargo should not run");
    const report = JSON.parse(result.stdout) as {
      ok: boolean;
      provider: string;
      model: string;
      scenario: string;
      credentialStatus: string;
      command: string | null;
      mockedProviderService: boolean;
      result: { artifactPath: string; outputCount: number };
    };
    expect(report).toEqual(
      expect.objectContaining({
        ok: true,
        provider: "google",
        model: "gemini-3.1-flash-tts-preview",
        scenario: "text-to-audio",
        credentialStatus: "not_checked",
        command: null,
        mockedProviderService: true,
      }),
    );
    expect(report.result.outputCount).toBe(1);
    expect(report.result.artifactPath.endsWith(".wav")).toBe(true);
    const sidecar = JSON.parse(
      readFileSync(
        join(outDir, "generated", "provider-e2e-generated", "asset.json"),
        "utf8",
      ),
    ) as {
      settings?: { category?: string; voice?: string };
      outputs: Array<{
        relativePath: string;
        sourceUrl?: string;
        durationSeconds: number;
        width: number;
        height: number;
        fps: number;
      }>;
    };
    expect(requiredAt(sidecar.outputs, 0, "provider sidecar output")).toEqual(
      expect.objectContaining({
        relativePath: "generated/provider-e2e-generated/google-mock-output.wav",
        durationSeconds: 12,
        width: 1,
        height: 1,
        fps: 1,
      }),
    );
    expect(sidecar.settings).toEqual(
      expect.objectContaining({
        category: "tts",
        voice: "Kore",
      }),
    );
    expect(requiredAt(sidecar.outputs, 0, "provider sidecar output")).not.toHaveProperty("sourceUrl");
  });

  it("runs a mocked Google Lyria text-to-music scenario without credentials or cargo", () => {
    const tempDir = mkdtempSync(join(tmpdir(), "video-creater-provider-google-lyria-default-"));
    const outDir = join(tempDir, "mocked-google-lyria-provider");
    const fakeCargo = join(tempDir, "cargo");
    writeFileSync(
      fakeCargo,
      "#!/bin/sh\necho cargo should not run >&2\nexit 42\n",
      { mode: 0o755 },
    );

    const result = spawnSync(
      process.execPath,
      [
        join(repoRoot, "scripts/provider-e2e.mjs"),
        "--provider",
        "google",
        "--scenario",
        "text-to-music",
        "--out-dir",
        outDir,
      ],
      {
        cwd: repoRoot,
        encoding: "utf8",
        env: {
          ...process.env,
          PATH: `${tempDir}:${process.env.PATH ?? ""}`,
          GEMINI_API_KEY: "",
        },
      },
    );

    expect(result.status).toBe(0);
    expect(result.stderr).not.toContain("cargo should not run");
    const report = JSON.parse(result.stdout) as {
      ok: boolean;
      provider: string;
      model: string;
      scenario: string;
      credentialStatus: string;
      command: string | null;
      mockedProviderService: boolean;
      result: { artifactPath: string; outputCount: number };
    };
    expect(report).toEqual(
      expect.objectContaining({
        ok: true,
        provider: "google",
        model: "lyria3-pro",
        scenario: "text-to-music",
        credentialStatus: "not_checked",
        command: null,
        mockedProviderService: true,
      }),
    );
    expect(report.result.outputCount).toBe(1);
    expect(report.result.artifactPath.endsWith(".mp3")).toBe(true);
    const sidecar = JSON.parse(
      readFileSync(
        join(outDir, "generated", "provider-e2e-generated", "asset.json"),
        "utf8",
      ),
    ) as {
      settings?: {
        category?: string;
        durationSeconds?: number;
        instrumental?: boolean;
        lyrics?: string;
        styleInstructions?: string;
      };
      outputs: Array<{
        relativePath: string;
        sourceUrl?: string;
        durationSeconds: number;
        width: number;
        height: number;
        fps: number;
      }>;
    };
    expect(requiredAt(sidecar.outputs, 0, "provider sidecar output")).toEqual(
      expect.objectContaining({
        relativePath: "generated/provider-e2e-generated/google-mock-output.mp3",
        durationSeconds: 30,
        width: 1,
        height: 1,
        fps: 1,
      }),
    );
    expect(sidecar.settings).toEqual(
      expect.objectContaining({
        category: "music",
        durationSeconds: 30,
        instrumental: false,
        lyrics: googleLyriaTextToMusicDefaultLyrics,
        styleInstructions: "bright, commercial, loopable",
      }),
    );
    expect(requiredAt(sidecar.outputs, 0, "provider sidecar output")).not.toHaveProperty("sourceUrl");
  });

  it("runs a mocked MiniMax text-to-music scenario without credentials or cargo", () => {
    const tempDir = mkdtempSync(join(tmpdir(), "video-creater-provider-minimax-music-default-"));
    const outDir = join(tempDir, "mocked-minimax-music-provider");
    const fakeCargo = join(tempDir, "cargo");
    writeFileSync(
      fakeCargo,
      "#!/bin/sh\necho cargo should not run >&2\nexit 42\n",
      { mode: 0o755 },
    );

    const result = spawnSync(
      process.execPath,
      [
        join(repoRoot, "scripts/provider-e2e.mjs"),
        "--provider",
        "minimax",
        "--scenario",
        "text-to-music",
        "--out-dir",
        outDir,
      ],
      {
        cwd: repoRoot,
        encoding: "utf8",
        env: {
          ...process.env,
          PATH: `${tempDir}:${process.env.PATH ?? ""}`,
          MINIMAX_API_KEY: "",
        },
      },
    );

    expect(result.status).toBe(0);
    expect(result.stderr).not.toContain("cargo should not run");
    const report = JSON.parse(result.stdout) as {
      ok: boolean;
      provider: string;
      model: string;
      scenario: string;
      credentialStatus: string;
      command: string | null;
      mockedProviderService: boolean;
      result: { artifactPath: string; outputCount: number };
    };
    expect(report).toEqual(
      expect.objectContaining({
        ok: true,
        provider: "minimax",
        model: "minimax-music-v2.6",
        scenario: "text-to-music",
        credentialStatus: "not_checked",
        command: null,
        mockedProviderService: true,
      }),
    );
    expect(report.result.outputCount).toBe(1);
    expect(report.result.artifactPath.endsWith(".mp3")).toBe(true);
    const sidecar = JSON.parse(
      readFileSync(
        join(outDir, "generated", "provider-e2e-generated", "asset.json"),
        "utf8",
      ),
    ) as {
      settings?: {
        category?: string;
        durationSeconds?: number;
        instrumental?: boolean;
        styleInstructions?: string;
      };
      outputs: Array<{
        relativePath: string;
        sourceUrl?: string;
        durationSeconds: number;
        width: number;
        height: number;
        fps: number;
      }>;
    };
    expect(requiredAt(sidecar.outputs, 0, "provider sidecar output")).toEqual(
      expect.objectContaining({
        relativePath: "generated/provider-e2e-generated/minimax-mock-output.mp3",
        durationSeconds: 30,
        width: 1,
        height: 1,
        fps: 1,
      }),
    );
    expect(sidecar.settings).toEqual(
      expect.objectContaining({
        category: "music",
        durationSeconds: 30,
        instrumental: false,
        lyrics: minimaxTextToMusicDefaultLyrics,
        styleInstructions: "bright, commercial, loopable",
      }),
    );
    expect(requiredAt(sidecar.outputs, 0, "provider sidecar output")).not.toHaveProperty("sourceUrl");
  });

  it("runs a mocked Google Veo video scenario without credentials or cargo", () => {
    const tempDir = mkdtempSync(join(tmpdir(), "video-creater-provider-google-video-default-"));
    const outDir = join(tempDir, "mocked-google-video-provider");
    const fakeCargo = join(tempDir, "cargo");
    writeFileSync(
      fakeCargo,
      "#!/bin/sh\necho cargo should not run >&2\nexit 42\n",
      { mode: 0o755 },
    );

    const result = spawnSync(
      process.execPath,
      [
        join(repoRoot, "scripts/provider-e2e.mjs"),
        "--provider",
        "google",
        "--scenario",
        "google-video",
        "--out-dir",
        outDir,
      ],
      {
        cwd: repoRoot,
        encoding: "utf8",
        env: {
          ...process.env,
          PATH: `${tempDir}:${process.env.PATH ?? ""}`,
          GEMINI_API_KEY: "",
        },
      },
    );

    expect(result.status).toBe(0);
    expect(result.stderr).not.toContain("cargo should not run");
    const report = JSON.parse(result.stdout) as {
      ok: boolean;
      provider: string;
      model: string;
      scenario: string;
      credentialStatus: string;
      command: string | null;
      mockedProviderService: boolean;
      result: { artifactPath: string; outputCount: number };
    };
    expect(report).toEqual(
      expect.objectContaining({
        ok: true,
        provider: "google",
        model: "veo3.1-fast",
        scenario: "google-video",
        credentialStatus: "not_checked",
        command: null,
        mockedProviderService: true,
      }),
    );
    expect(report.result.outputCount).toBe(1);
    expect(report.result.artifactPath.endsWith(".mp4")).toBe(true);
    const sidecar = JSON.parse(
      readFileSync(
        join(outDir, "generated", "provider-e2e-generated", "asset.json"),
        "utf8",
      ),
    ) as {
      settings?: { generateAudio?: boolean };
      references?: {
        firstFrameMediaId?: string;
        lastFrameMediaId?: string;
        referenceImageMediaRefs?: string[];
        providerInputUrls?: string[];
      };
      outputs: Array<{ relativePath: string; durationSeconds: number; fps: number }>;
    };
    expect(sidecar.settings?.generateAudio).toBe(true);
    expect(sidecar.references).toEqual(
      expect.objectContaining({
        firstFrameMediaId: "provider-e2e-first-frame",
        lastFrameMediaId: "provider-e2e-last-frame",
        referenceImageMediaRefs: ["provider-e2e-reference-image"],
      }),
    );
    expect(sidecar.references?.providerInputUrls).toEqual([
      expect.stringMatching(/^data:image\/png;base64,/),
      expect.stringMatching(/^data:image\/png;base64,/),
      expect.stringMatching(/^data:image\/png;base64,/),
    ]);
    expect(requiredAt(sidecar.outputs, 0, "provider sidecar output")).toEqual(
      expect.objectContaining({
        relativePath: "generated/provider-e2e-generated/google-mock-output.mp4",
        durationSeconds: 4,
        fps: 24,
      }),
    );
  });

  it.each([
    {
      scenario: "video-to-music",
      model: "sonilo/v1.1/video-to-music",
      extension: ".m4a",
    },
    {
      scenario: "video-to-sfx",
      model: "mirelo-ai/sfx-v1.5/video-to-audio",
      extension: ".wav",
    },
  ] as const)(
    "runs a mocked fal $scenario scenario without credentials or cargo",
    ({ scenario, model, extension }) => {
      const tempDir = mkdtempSync(join(tmpdir(), `video-creater-provider-${scenario}-default-`));
      const outDir = join(tempDir, "mocked-video-audio-provider");
      const fakeCargo = join(tempDir, "cargo");
      writeFileSync(
        fakeCargo,
        "#!/bin/sh\necho cargo should not run >&2\nexit 42\n",
        { mode: 0o755 },
      );

      const result = spawnSync(
        process.execPath,
        [
          join(repoRoot, "scripts/provider-e2e.mjs"),
          "--provider",
          "fal.ai",
          "--scenario",
          scenario,
          "--out-dir",
          outDir,
        ],
        {
          cwd: repoRoot,
          encoding: "utf8",
          env: {
            ...process.env,
            PATH: `${tempDir}:${process.env.PATH ?? ""}`,
            FAL_KEY: "",
          },
        },
      );

      expect(result.status).toBe(0);
      expect(result.stderr).not.toContain("cargo should not run");
      const report = JSON.parse(result.stdout) as {
        ok: boolean;
        provider: string;
        model: string;
        scenario: string;
        credentialStatus: string;
        command: string | null;
        mockedProviderService: boolean;
        result: { artifactPath: string; outputCount: number };
      };
      expect(report).toEqual(
        expect.objectContaining({
          ok: true,
          provider: "fal.ai",
          model,
          scenario,
          credentialStatus: "not_checked",
          command: null,
          mockedProviderService: true,
        }),
      );
      expect(report.result.outputCount).toBe(1);
      expect(report.result.artifactPath.endsWith(extension)).toBe(true);
      const sidecar = JSON.parse(
        readFileSync(
          join(outDir, "generated", "provider-e2e-generated", "asset.json"),
          "utf8",
        ),
      ) as {
        prompt?: string;
        settings?: {
          category?: string;
          durationSeconds?: number;
          instrumental?: boolean;
          styleInstructions?: string;
        };
        references: {
          sourceVideoMediaRef: string;
          referenceVideoMediaRefs: string[];
          providerInputUrls: string[];
        };
        outputs: Array<{ relativePath: string; durationSeconds: number; width: number; height: number; fps: number }>;
      };
      expect(sidecar.references).toEqual(
        expect.objectContaining({
          sourceVideoMediaRef: "provider-e2e-video-source",
          referenceVideoMediaRefs: ["provider-e2e-video-source"],
          providerInputUrls: ["https://v3.fal.media/files/provider-e2e/source-video.mp4"],
        }),
      );
      expect(requiredAt(sidecar.outputs, 0, "provider sidecar output")).toEqual(
        expect.objectContaining({
          relativePath: `generated/provider-e2e-generated/fal-ai-mock-output${extension}`,
          durationSeconds: 10,
          width: 1,
          height: 1,
          fps: 1,
        }),
      );
      if (scenario === "video-to-music") {
        expect(sidecar).toEqual(
          expect.objectContaining({
            prompt: videoToMusicDefaultPrompt,
            settings: expect.objectContaining(textToMusicDefaultSettings),
          }),
        );
      } else {
        expect(sidecar).toEqual(
          expect.objectContaining({
            prompt: "",
          }),
        );
      }
    },
  );

  it("runs a mocked fal local video-to-music insert scenario without credentials or cargo", () => {
    const tempDir = mkdtempSync(join(tmpdir(), "video-creater-provider-video-music-insert-default-"));
    const outDir = join(tempDir, "mocked-video-music-insert-provider");
    const fakeCargo = join(tempDir, "cargo");
    writeFileSync(
      fakeCargo,
      "#!/bin/sh\necho cargo should not run >&2\nexit 42\n",
      { mode: 0o755 },
    );

    const result = spawnSync(
      process.execPath,
      [
        join(repoRoot, "scripts/provider-e2e.mjs"),
        "--provider",
        "fal.ai",
        "--scenario",
        "local-video-to-music-insert",
        "--out-dir",
        outDir,
      ],
      {
        cwd: repoRoot,
        encoding: "utf8",
        env: {
          ...process.env,
          PATH: `${tempDir}:${process.env.PATH ?? ""}`,
          FAL_KEY: "",
        },
      },
    );

    expect(result.status).toBe(0);
    expect(result.stderr).not.toContain("cargo should not run");
    const report = JSON.parse(result.stdout) as {
      ok: boolean;
      provider: string;
      model: string;
      scenario: string;
      credentialStatus: string;
      command: string | null;
      mockedProviderService: boolean;
      result: { artifactPath: string; outputCount: number };
    };
    expect(report).toEqual(
      expect.objectContaining({
        ok: true,
        provider: "fal.ai",
        model: "sonilo/v1.1/video-to-music",
        scenario: "local-video-to-music-insert",
        credentialStatus: "not_checked",
        command: null,
        mockedProviderService: true,
      }),
    );
    expect(report.result.outputCount).toBe(1);
    expect(report.result.artifactPath.endsWith(".m4a")).toBe(true);
    const sidecar = JSON.parse(
      readFileSync(
        join(outDir, "generated", "provider-e2e-generated", "asset.json"),
        "utf8",
      ),
    ) as {
      prompt: string;
      placementIntent: string;
      references: {
        mediaIds: string[];
        sourceVideoMediaRef: string;
        referenceVideoMediaRefs: string[];
        providerInputUrls: string[];
      };
      settings: {
        category: string;
        durationSeconds: number;
        instrumental: boolean;
        styleInstructions: string;
        timelineStartSeconds: number;
        videoSourceStartSeconds: number;
        videoSourceEndSeconds: number;
      };
      outputs: Array<{ mediaId: string; relativePath: string; durationSeconds: number; width: number; height: number; fps: number }>;
    };
    expect(sidecar).toEqual(
      expect.objectContaining({
        prompt: videoToMusicDefaultPrompt,
        placementIntent: "insert-audio:item-video-to-music-source",
      }),
    );
    expect(sidecar.references).toEqual(
      expect.objectContaining({
        mediaIds: ["provider-e2e-video-source"],
        sourceVideoMediaRef: "provider-e2e-video-source",
        referenceVideoMediaRefs: ["provider-e2e-video-source"],
        providerInputUrls: ["https://v3.fal.media/files/provider-e2e/source-video.mp4"],
      }),
    );
    expect(sidecar.settings).toEqual(
      expect.objectContaining({
        ...textToMusicDefaultSettings,
        timelineStartSeconds: 12,
        videoSourceStartSeconds: 1.25,
        videoSourceEndSeconds: 5.75,
      }),
    );
    expect(requiredAt(sidecar.outputs, 0, "provider sidecar output")).toEqual(
      expect.objectContaining({
        relativePath: "generated/provider-e2e-generated/fal-ai-mock-output.m4a",
        durationSeconds: 10,
        width: 1,
        height: 1,
        fps: 1,
      }),
    );
    const timeline = JSON.parse(readFileSync(join(outDir, "timeline.json"), "utf8")) as {
      tracks: Array<{
        id: string;
        kind: string;
        items: Array<{
          id: string;
          kind: string;
          startSeconds: number;
          durationSeconds: number;
          source?: { type?: string; mediaId?: string };
          properties?: { generatedOutputMediaId?: string; sourceVideoMediaRef?: string };
        }>;
      }>;
    };
    const outputMediaId = requiredAt(sidecar.outputs, 0, "provider sidecar output")?.mediaId;
    const insertedAudio = timeline.tracks
      .find((track) => track.id === "track-audio")
      ?.items.find((item) => item.id === "item-video-to-music-inserted-audio");
    expect(insertedAudio).toEqual(
      expect.objectContaining({
        kind: "audio_clip",
        startSeconds: 12,
        durationSeconds: 10,
        source: { type: "media", mediaId: outputMediaId },
        properties: expect.objectContaining({
          generatedOutputMediaId: outputMediaId,
          sourceVideoMediaRef: "provider-e2e-video-source",
        }),
      }),
    );
  });

  it("runs a mocked fal local video-to-sfx insert scenario without credentials or cargo", () => {
    const tempDir = mkdtempSync(join(tmpdir(), "video-creater-provider-video-sfx-insert-default-"));
    const outDir = join(tempDir, "mocked-video-sfx-insert-provider");
    const fakeCargo = join(tempDir, "cargo");
    writeFileSync(
      fakeCargo,
      "#!/bin/sh\necho cargo should not run >&2\nexit 42\n",
      { mode: 0o755 },
    );

    const result = spawnSync(
      process.execPath,
      [
        join(repoRoot, "scripts/provider-e2e.mjs"),
        "--provider",
        "fal.ai",
        "--scenario",
        "local-video-to-sfx-insert",
        "--out-dir",
        outDir,
      ],
      {
        cwd: repoRoot,
        encoding: "utf8",
        env: {
          ...process.env,
          PATH: `${tempDir}:${process.env.PATH ?? ""}`,
          FAL_KEY: "",
        },
      },
    );

    expect(result.status).toBe(0);
    expect(result.stderr).not.toContain("cargo should not run");
    const report = JSON.parse(result.stdout) as {
      ok: boolean;
      provider: string;
      model: string;
      scenario: string;
      credentialStatus: string;
      command: string | null;
      mockedProviderService: boolean;
      result: { artifactPath: string; outputCount: number };
    };
    expect(report).toEqual(
      expect.objectContaining({
        ok: true,
        provider: "fal.ai",
        model: "mirelo-ai/sfx-v1.5/video-to-audio",
        scenario: "local-video-to-sfx-insert",
        credentialStatus: "not_checked",
        command: null,
        mockedProviderService: true,
      }),
    );
    expect(report.result.outputCount).toBe(1);
    expect(report.result.artifactPath.endsWith(".wav")).toBe(true);
    const sidecar = JSON.parse(
      readFileSync(
        join(outDir, "generated", "provider-e2e-generated", "asset.json"),
        "utf8",
      ),
    ) as {
      placementIntent: string;
      references: {
        mediaIds: string[];
        sourceVideoMediaRef: string;
        referenceVideoMediaRefs: string[];
        providerInputUrls: string[];
      };
      settings: {
        durationSeconds: number;
        timelineStartSeconds: number;
        videoSourceStartSeconds: number;
        videoSourceEndSeconds: number;
      };
      outputs: Array<{ mediaId: string; relativePath: string; durationSeconds: number; width: number; height: number; fps: number }>;
    };
    expect(sidecar).toEqual(
      expect.objectContaining({
        placementIntent: "insert-audio:item-video-to-sfx-source",
      }),
    );
    expect(sidecar.references).toEqual(
      expect.objectContaining({
        mediaIds: ["provider-e2e-video-source"],
        sourceVideoMediaRef: "provider-e2e-video-source",
        referenceVideoMediaRefs: ["provider-e2e-video-source"],
        providerInputUrls: ["https://v3.fal.media/files/provider-e2e/source-video.mp4"],
      }),
    );
    expect(sidecar.settings).toEqual(
      expect.objectContaining({
        durationSeconds: 10,
        timelineStartSeconds: 8,
        videoSourceStartSeconds: 0.5,
        videoSourceEndSeconds: 2,
      }),
    );
    expect(requiredAt(sidecar.outputs, 0, "provider sidecar output")).toEqual(
      expect.objectContaining({
        relativePath: "generated/provider-e2e-generated/fal-ai-mock-output.wav",
        durationSeconds: 10,
        width: 1,
        height: 1,
        fps: 1,
      }),
    );
    const timeline = JSON.parse(readFileSync(join(outDir, "timeline.json"), "utf8")) as {
      tracks: Array<{
        id: string;
        kind: string;
        items: Array<{
          id: string;
          kind: string;
          startSeconds: number;
          durationSeconds: number;
          source?: { type?: string; mediaId?: string };
          properties?: { generatedOutputMediaId?: string; sourceVideoMediaRef?: string };
        }>;
      }>;
    };
    const outputMediaId = requiredAt(sidecar.outputs, 0, "provider sidecar output")?.mediaId;
    const insertedAudio = timeline.tracks
      .find((track) => track.id === "track-audio")
      ?.items.find((item) => item.id === "item-video-to-sfx-inserted-audio");
    expect(insertedAudio).toEqual(
      expect.objectContaining({
        kind: "audio_clip",
        startSeconds: 8,
        durationSeconds: 10,
        source: { type: "media", mediaId: outputMediaId },
        properties: expect.objectContaining({
          generatedOutputMediaId: outputMediaId,
          sourceVideoMediaRef: "provider-e2e-video-source",
        }),
      }),
    );
  });

  it("runs a mocked fal local image upscale scenario without credentials or cargo", () => {
    const tempDir = mkdtempSync(join(tmpdir(), "video-creater-provider-image-upscale-default-"));
    const outDir = join(tempDir, "mocked-image-upscale-provider");
    const fakeCargo = join(tempDir, "cargo");
    writeFileSync(
      fakeCargo,
      "#!/bin/sh\necho cargo should not run >&2\nexit 42\n",
      { mode: 0o755 },
    );

    const result = spawnSync(
      process.execPath,
      [
        join(repoRoot, "scripts/provider-e2e.mjs"),
        "--provider",
        "fal.ai",
        "--scenario",
        "local-image-upscale",
        "--out-dir",
        outDir,
      ],
      {
        cwd: repoRoot,
        encoding: "utf8",
        env: {
          ...process.env,
          PATH: `${tempDir}:${process.env.PATH ?? ""}`,
          FAL_KEY: "",
        },
      },
    );

    expect(result.status).toBe(0);
    expect(result.stderr).not.toContain("cargo should not run");
    const report = JSON.parse(result.stdout) as {
      ok: boolean;
      provider: string;
      model: string;
      scenario: string;
      credentialStatus: string;
      command: string | null;
      mockedProviderService: boolean;
      result: { artifactPath: string; outputCount: number };
    };
    expect(report).toEqual(
      expect.objectContaining({
        ok: true,
        provider: "fal.ai",
        model: "fal-ai/aura-sr",
        scenario: "local-image-upscale",
        credentialStatus: "not_checked",
        command: null,
        mockedProviderService: true,
      }),
    );
    expect(report.result.outputCount).toBe(1);
    expect(report.result.artifactPath.endsWith(".png")).toBe(true);
    const sidecar = JSON.parse(
      readFileSync(
        join(outDir, "generated", "provider-e2e-generated", "asset.json"),
        "utf8",
      ),
    ) as {
      references: {
        mediaIds: string[];
        providerInputUrls: string[];
      };
      outputs: Array<{ relativePath: string; width: number; height: number }>;
    };
    expect(sidecar.references).toEqual(
      expect.objectContaining({
        mediaIds: ["provider-e2e-source"],
        providerInputUrls: ["https://v3.fal.media/files/provider-e2e/source-image.png"],
      }),
    );
    expect(requiredAt(sidecar.outputs, 0, "provider sidecar output")).toEqual(
      expect.objectContaining({
        relativePath: "generated/provider-e2e-generated/fal-ai-mock-output.png",
        width: 1024,
        height: 768,
      }),
    );
  });

  it("runs a mocked fal local image upscale replacement scenario without credentials or cargo", () => {
    const tempDir = mkdtempSync(join(tmpdir(), "video-creater-provider-replace-default-"));
    const outDir = join(tempDir, "mocked-replace-provider");
    const fakeCargo = join(tempDir, "cargo");
    writeFileSync(
      fakeCargo,
      "#!/bin/sh\necho cargo should not run >&2\nexit 42\n",
      { mode: 0o755 },
    );

    const result = spawnSync(
      process.execPath,
      [
        join(repoRoot, "scripts/provider-e2e.mjs"),
        "--provider",
        "fal.ai",
        "--scenario",
        "local-image-upscale-replace",
        "--out-dir",
        outDir,
      ],
      {
        cwd: repoRoot,
        encoding: "utf8",
        env: {
          ...process.env,
          PATH: `${tempDir}:${process.env.PATH ?? ""}`,
          FAL_KEY: "",
        },
      },
    );

    expect(result.status).toBe(0);
    expect(result.stderr).not.toContain("cargo should not run");
    const report = JSON.parse(result.stdout) as {
      ok: boolean;
      provider: string;
      model: string;
      scenario: string;
      credentialStatus: string;
      command: string | null;
      result: { outputCount: number };
    };
    expect(report).toEqual(
      expect.objectContaining({
        ok: true,
        provider: "fal.ai",
        model: "fal-ai/aura-sr",
        scenario: "local-image-upscale-replace",
        credentialStatus: "not_checked",
        command: null,
      }),
    );
    expect(report.result.outputCount).toBe(1);
    const timeline = JSON.parse(readFileSync(join(outDir, "timeline.json"), "utf8")) as {
      tracks: Array<{ items: Array<{ id: string; source: { mediaId: string }; properties: Record<string, unknown> }> }>;
    };
    const item = timeline.tracks.flatMap((track) => track.items).find((candidate) => candidate.id === "item-upscale-target");
    expect(item).toEqual(
      expect.objectContaining({
        source: { type: "media", mediaId: "provider-e2e-output" },
        properties: expect.objectContaining({
          generatedOutputMediaId: "provider-e2e-output",
          sourceOut: 4,
        }),
      }),
    );
    const linkedItem = timeline.tracks
      .flatMap((track) => track.items)
      .find((candidate) => candidate.id === "item-upscale-target-linked");
    expect(linkedItem).toEqual(
      expect.objectContaining({
        source: { type: "media", mediaId: "provider-e2e-output" },
        properties: expect.objectContaining({
          generatedOutputMediaId: "provider-e2e-output",
          sourceOut: 3,
        }),
      }),
    );
  });

  it("refuses to run live Replicate verification without credentials", () => {
    const result = spawnSync(
      process.execPath,
      [
        join(repoRoot, "scripts/provider-e2e.mjs"),
        "--provider",
        "replicate",
        "--live",
      ],
      {
        cwd: repoRoot,
        encoding: "utf8",
        env: {
          ...process.env,
          REPLICATE_API_TOKEN: "",
        },
      },
    );

    expect(result.status).toBe(1);
    expect(result.stderr).toContain("REPLICATE_API_TOKEN is required");
    const report = JSON.parse(result.stdout) as {
      ok: boolean;
      provider: string;
      credentialStatus: string;
      command: string | null;
    };
    expect(report).toEqual(
      expect.objectContaining({
        ok: false,
        provider: "replicate",
        credentialStatus: "missing",
        command: null,
      }),
    );
  });

  it("refuses to run live provider verification with credentials but without spend opt-in", () => {
    const tempDir = mkdtempSync(join(tmpdir(), "video-creater-provider-spend-gate-"));
    const fakeCargo = join(tempDir, "cargo");
    writeFileSync(
      fakeCargo,
      "#!/bin/sh\necho cargo should not run >&2\nexit 42\n",
      { mode: 0o755 },
    );

    const result = spawnSync(
      process.execPath,
      [
        join(repoRoot, "scripts/provider-e2e.mjs"),
        "--provider",
        "replicate",
        "--live",
      ],
      {
        cwd: repoRoot,
        encoding: "utf8",
        env: {
          ...process.env,
          PATH: `${tempDir}:${process.env.PATH ?? ""}`,
          REPLICATE_API_TOKEN: "present-test-token",
          VIDEO_CREATER_PROVIDER_E2E_ALLOW_SPEND: "",
        },
      },
    );

    expect(result.status).toBe(1);
    expect(result.stderr).toContain(
      "VIDEO_CREATER_PROVIDER_E2E_ALLOW_SPEND=1 is required",
    );
    expect(result.stderr).not.toContain("cargo should not run");
    const report = JSON.parse(result.stdout) as {
      ok: boolean;
      provider: string;
      credentialStatus: string;
      liveSpendOptInEnvVar: string;
      liveSpendOptInStatus: string;
      command: string | null;
    };
    expect(report).toEqual(
      expect.objectContaining({
        ok: false,
        provider: "replicate",
        credentialStatus: "present",
        liveSpendOptInEnvVar: "VIDEO_CREATER_PROVIDER_E2E_ALLOW_SPEND",
        liveSpendOptInStatus: "missing",
        command: null,
      }),
    );
  });

  it("validates retained provider E2E reports for both canonical providers", () => {
    const tempDir = mkdtempSync(join(tmpdir(), "video-creater-provider-policy-"));
    const falReport = writeProviderPolicyFixture({
      rootDir: tempDir,
      provider: "fal.ai",
      model: "fal-ai/flux/schnell",
      requestId: "fal-request-1",
    });
    const falWanReport = writeProviderPolicyFixture({
      rootDir: tempDir,
      provider: "fal.ai",
      model: "fal-ai/wan/v2.7/image-to-video",
      requestId: "fal-wan-request-1",
      scenario: "wan-image-to-video",
    });
    const falWanReferenceReport = writeProviderPolicyFixture({
      rootDir: tempDir,
      provider: "fal.ai",
      model: "fal-ai/wan/v2.7/reference-to-video",
      requestId: "fal-wan-reference-request-1",
      scenario: "wan-reference-to-video",
    });
    const falImageEditReport = writeProviderPolicyFixture({
      rootDir: tempDir,
      provider: "fal.ai",
      model: "fal-ai/nano-banana-pro/edit",
      requestId: "fal-image-edit-request-1",
      scenario: "image-edit",
    });
    const falImageEditReplaceReport = writeProviderPolicyFixture({
      rootDir: tempDir,
      provider: "fal.ai",
      model: "fal-ai/nano-banana-pro/edit",
      requestId: "fal-image-edit-replace-request-1",
      scenario: "local-image-edit-replace",
    });
    const falImageEditRetryReport = writeProviderPolicyFixture({
      rootDir: tempDir,
      provider: "fal.ai",
      model: "fal-ai/nano-banana-pro/edit",
      requestId: "fal-image-edit-retry-request-1",
      scenario: "local-image-edit-retry",
    });
    const falImageEditCancellationReport = writeProviderPolicyFixture({
      rootDir: tempDir,
      provider: "fal.ai",
      model: "fal-ai/nano-banana-pro/edit",
      requestId: "fal-image-edit-cancel-request-1",
      scenario: "local-image-edit-cancellation",
    });
    const falImageEditFailureReport = writeProviderPolicyFixture({
      rootDir: tempDir,
      provider: "fal.ai",
      model: "fal-ai/nano-banana-pro/edit",
      requestId: "fal-image-edit-failure-request-1",
      scenario: "local-image-edit-failure",
    });
    const falKreaReport = writeProviderPolicyFixture({
      rootDir: tempDir,
      provider: "fal.ai",
      model: "fal-ai/krea-2/turbo",
      requestId: "fal-krea-request-1",
      scenario: "krea-text-to-image",
    });
    const falRecraftReport = writeProviderPolicyFixture({
      rootDir: tempDir,
      provider: "fal.ai",
      model: "fal-ai/recraft/v3/text-to-image",
      requestId: "fal-recraft-request-1",
      scenario: "recraft-text-to-image",
    });
    const falMultiImageReport = writeProviderPolicyFixture({
      rootDir: tempDir,
      provider: "fal.ai",
      model: "fal-ai/flux/schnell",
      requestId: "fal-multi-image-request-1",
      scenario: "multi-image",
    });
    const falKlingReport = writeProviderPolicyFixture({
      rootDir: tempDir,
      provider: "fal.ai",
      model: "fal-ai/kling-video/v3/pro/image-to-video",
      requestId: "fal-kling-request-1",
      scenario: "kling-image-to-video",
    });
    const falVideoToVideoReport = writeProviderPolicyFixture({
      rootDir: tempDir,
      provider: "fal.ai",
      model: "fal-ai/wan/v2.2-a14b/video-to-video",
      requestId: "fal-video-to-video-request-1",
      scenario: "video-to-video",
    });
    const falVideoEditReplaceReport = writeProviderPolicyFixture({
      rootDir: tempDir,
      provider: "fal.ai",
      model: "fal-ai/wan/v2.2-a14b/video-to-video",
      requestId: "fal-video-edit-replace-request-1",
      scenario: "local-video-edit-replace",
      sourceUrl: "https://provider.example/fal-video-edit-replace-request-1/edited-video.mp4",
    });
    const falMotionControlReplaceReport = writeProviderPolicyFixture({
      rootDir: tempDir,
      provider: "fal.ai",
      model: "fal-ai/kling-video/v3/pro/motion-control",
      requestId: "fal-motion-control-replace-request-1",
      scenario: "local-video-motion-control-replace",
      sourceUrl: "https://provider.example/fal-motion-control-replace-request-1/edited-video.mp4",
    });
    const falTextVideoReplaceReport = writeProviderPolicyFixture({
      rootDir: tempDir,
      provider: "fal.ai",
      model: "fal-ai/wan-25-preview/text-to-video",
      requestId: "fal-text-video-replace-request-1",
      scenario: "text-to-video-replace",
      sourceUrl: "https://provider.example/fal-text-video-replace-request-1/generated-video.mp4",
    });
    const falTextVideoInsertReport = writeProviderPolicyFixture({
      rootDir: tempDir,
      provider: "fal.ai",
      model: "fal-ai/wan-25-preview/text-to-video",
      requestId: "fal-text-video-insert-request-1",
      scenario: "text-to-video-insert",
      sourceUrl: "https://provider.example/fal-text-video-insert-request-1/generated-video.mp4",
    });
    const falVideoEditRetryReport = writeProviderPolicyFixture({
      rootDir: tempDir,
      provider: "fal.ai",
      model: "fal-ai/wan/v2.2-a14b/video-to-video",
      requestId: "fal-video-edit-retry-request-1",
      scenario: "local-video-edit-retry",
      sourceUrl: "https://provider.example/fal-video-edit-retry-request-1/edited-video.mp4",
    });
    const falVideoEditCancellationReport = writeProviderPolicyFixture({
      rootDir: tempDir,
      provider: "fal.ai",
      model: "fal-ai/wan/v2.2-a14b/video-to-video",
      requestId: "fal-video-edit-cancel-request-1",
      scenario: "local-video-edit-cancellation",
      sourceUrl: "https://provider.example/fal-video-edit-cancel-request-1/edited-video.mp4",
    });
    const falVideoEditFailureReport = writeProviderPolicyFixture({
      rootDir: tempDir,
      provider: "fal.ai",
      model: "fal-ai/wan/v2.2-a14b/video-to-video",
      requestId: "fal-video-edit-failure-request-1",
      scenario: "local-video-edit-failure",
      sourceUrl: "https://provider.example/fal-video-edit-failure-request-1/edited-video.mp4",
    });
    const falAudioReport = writeProviderPolicyFixture({
      rootDir: tempDir,
      provider: "fal.ai",
      model: "bytedance/seed-audio-1.0",
      requestId: "fal-audio-request-1",
      scenario: "text-to-audio",
    });
    const falMusicReport = writeProviderPolicyFixture({
      rootDir: tempDir,
      provider: "fal.ai",
      model: "sonilo/v1.1/text-to-music",
      requestId: "fal-music-request-1",
      scenario: "text-to-music",
      sourceUrl: null,
    });
    const falMusicInsertReport = writeProviderPolicyFixture({
      rootDir: tempDir,
      provider: "fal.ai",
      model: "sonilo/v1.1/text-to-music",
      requestId: "fal-music-insert-request-1",
      scenario: "text-to-music-insert",
      sourceUrl: null,
    });
    const falVideoMusicReport = writeProviderPolicyFixture({
      rootDir: tempDir,
      provider: "fal.ai",
      model: "sonilo/v1.1/video-to-music",
      requestId: "fal-video-music-request-1",
      scenario: "video-to-music",
    });
    const falVideoMusicInsertReport = writeProviderPolicyFixture({
      rootDir: tempDir,
      provider: "fal.ai",
      model: "sonilo/v1.1/video-to-music",
      requestId: "fal-video-music-insert-request-1",
      scenario: "local-video-to-music-insert",
    });
    const falVideoSfxReport = writeProviderPolicyFixture({
      rootDir: tempDir,
      provider: "fal.ai",
      model: "mirelo-ai/sfx-v1.5/video-to-audio",
      requestId: "fal-video-sfx-request-1",
      scenario: "video-to-sfx",
    });
    const falVideoSfxInsertReport = writeProviderPolicyFixture({
      rootDir: tempDir,
      provider: "fal.ai",
      model: "mirelo-ai/sfx-v1.5/video-to-audio",
      requestId: "fal-video-sfx-insert-request-1",
      scenario: "local-video-to-sfx-insert",
    });
    const falReplaceReport = writeProviderPolicyFixture({
      rootDir: tempDir,
      provider: "fal.ai",
      model: "fal-ai/aura-sr",
      requestId: "fal-replace-request-1",
      scenario: "local-image-upscale-replace",
    });
    const falImageUpscaleReport = writeProviderPolicyFixture({
      rootDir: tempDir,
      provider: "fal.ai",
      model: "fal-ai/aura-sr",
      requestId: "fal-image-upscale-request-1",
      scenario: "local-image-upscale",
    });
    const falImageUpscaleRetryReport = writeProviderPolicyFixture({
      rootDir: tempDir,
      provider: "fal.ai",
      model: "fal-ai/aura-sr",
      requestId: "fal-image-upscale-retry-request-1",
      scenario: "local-image-upscale-retry",
    });
    const falImageUpscaleCancellationReport = writeProviderPolicyFixture({
      rootDir: tempDir,
      provider: "fal.ai",
      model: "fal-ai/aura-sr",
      requestId: "fal-image-upscale-cancel-request-1",
      scenario: "local-image-upscale-cancellation",
    });
    const falImageUpscaleFailureReport = writeProviderPolicyFixture({
      rootDir: tempDir,
      provider: "fal.ai",
      model: "fal-ai/aura-sr",
      requestId: "fal-image-upscale-failure-request-1",
      scenario: "local-image-upscale-failure",
    });
    const falVideoUpscaleReport = writeProviderPolicyFixture({
      rootDir: tempDir,
      provider: "fal.ai",
      model: "fal-ai/video-upscaler",
      requestId: "fal-video-upscale-request-1",
      scenario: "local-video-upscale",
      sourceUrl: "https://provider.example/fal-video-upscale-request-1/upscaled-video.mp4",
    });
    const falVideoUpscaleReplaceReport = writeProviderPolicyFixture({
      rootDir: tempDir,
      provider: "fal.ai",
      model: "fal-ai/video-upscaler",
      requestId: "fal-video-upscale-replace-request-1",
      scenario: "local-video-upscale-replace",
      sourceUrl: "https://provider.example/fal-video-upscale-replace-request-1/upscaled-video.mp4",
    });
    const falVideoUpscaleRetryReport = writeProviderPolicyFixture({
      rootDir: tempDir,
      provider: "fal.ai",
      model: "fal-ai/video-upscaler",
      requestId: "fal-video-upscale-retry-request-1",
      scenario: "local-video-upscale-retry",
      sourceUrl: "https://provider.example/fal-video-upscale-retry-request-1/upscaled-video.mp4",
    });
    const falVideoUpscaleCancellationReport = writeProviderPolicyFixture({
      rootDir: tempDir,
      provider: "fal.ai",
      model: "fal-ai/video-upscaler",
      requestId: "fal-video-upscale-cancel-request-1",
      scenario: "local-video-upscale-cancellation",
      sourceUrl: "https://provider.example/fal-video-upscale-cancel-request-1/upscaled-video.mp4",
    });
    const falVideoUpscaleFailureReport = writeProviderPolicyFixture({
      rootDir: tempDir,
      provider: "fal.ai",
      model: "fal-ai/video-upscaler",
      requestId: "fal-video-upscale-failure-request-1",
      scenario: "local-video-upscale-failure",
      sourceUrl: "https://provider.example/fal-video-upscale-failure-request-1/upscaled-video.mp4",
    });
    const falFailureReport = writeProviderPolicyFixture({
      rootDir: tempDir,
      provider: "fal.ai",
      model: "fal-ai/flux/schnell",
      requestId: "fal-failure-request-1",
      scenario: "provider-failure",
    });
    const falCancellationReport = writeProviderPolicyFixture({
      rootDir: tempDir,
      provider: "fal.ai",
      model: "fal-ai/flux/schnell",
      requestId: "fal-cancel-request-1",
      scenario: "provider-cancellation",
    });
    const falRetryReport = writeProviderPolicyFixture({
      rootDir: tempDir,
      provider: "fal.ai",
      model: "fal-ai/flux/schnell",
      requestId: "fal-retry-request-1",
      scenario: "provider-retry",
    });
    const openAiReport = writeProviderPolicyFixture({
      rootDir: tempDir,
      provider: "openai",
      model: "gpt-image-2",
      requestId: "openai-request-1",
      sourceUrl: null,
    });
    const openAiImageEditReport = writeProviderPolicyFixture({
      rootDir: tempDir,
      provider: "openai",
      model: "gpt-image-1.5",
      requestId: "openai-image-edit-request-1",
      scenario: "image-edit",
      sourceUrl: null,
    });
    const openAiImageEditReplaceReport = writeProviderPolicyFixture({
      rootDir: tempDir,
      provider: "openai",
      model: "gpt-image-1.5",
      requestId: "openai-image-edit-replace-request-1",
      scenario: "local-image-edit-replace",
      sourceUrl: null,
    });
    const openAiTextImageReplaceReport = writeProviderPolicyFixture({
      rootDir: tempDir,
      provider: "openai",
      model: "gpt-image-2",
      requestId: "openai-text-image-replace-request-1",
      scenario: "text-to-image-replace",
      sourceUrl: null,
    });
    const openAiMultiImageReport = writeProviderPolicyFixture({
      rootDir: tempDir,
      provider: "openai",
      model: "gpt-image-2",
      requestId: "openai-multi-image-request-1",
      scenario: "multi-image",
      sourceUrl: null,
    });
    const openAiAudioReport = writeProviderPolicyFixture({
      rootDir: tempDir,
      provider: "openai",
      model: "gpt-4o-mini-tts",
      requestId: "openai-audio-request-1",
      scenario: "text-to-audio",
      sourceUrl: null,
    });
    const elevenLabsAudioReport = writeProviderPolicyFixture({
      rootDir: tempDir,
      provider: "elevenlabs",
      model: "elevenlabs-tts-v3",
      requestId: "elevenlabs-audio-request-1",
      scenario: "text-to-audio",
      sourceUrl: null,
    });
    const elevenLabsMusicReport = writeProviderPolicyFixture({
      rootDir: tempDir,
      provider: "elevenlabs",
      model: "elevenlabs-music",
      requestId: "elevenlabs-music-request-1",
      scenario: "text-to-music",
      sourceUrl: null,
    });
    const elevenLabsMusicInsertReport = writeProviderPolicyFixture({
      rootDir: tempDir,
      provider: "elevenlabs",
      model: "elevenlabs-music",
      requestId: "elevenlabs-music-insert-request-1",
      scenario: "text-to-music-insert",
      sourceUrl: null,
    });
    const minimaxMusicReport = writeProviderPolicyFixture({
      rootDir: tempDir,
      provider: "minimax",
      model: "minimax-music-v2.6",
      requestId: "minimax-music-request-1",
      scenario: "text-to-music",
      sourceUrl: null,
    });
    const minimaxMusicInsertReport = writeProviderPolicyFixture({
      rootDir: tempDir,
      provider: "minimax",
      model: "minimax-music-v2.6",
      requestId: "minimax-music-insert-request-1",
      scenario: "text-to-music-insert",
      sourceUrl: null,
    });
    const xAiReport = writeProviderPolicyFixture({
      rootDir: tempDir,
      provider: "xai",
      model: "grok-imagine-image-quality",
      requestId: "xai-request-1",
    });
    const xAiImageEditReport = writeProviderPolicyFixture({
      rootDir: tempDir,
      provider: "xai",
      model: "grok-imagine-image-quality",
      requestId: "xai-image-edit-request-1",
      scenario: "image-edit",
    });
    const xAiImageEditReplaceReport = writeProviderPolicyFixture({
      rootDir: tempDir,
      provider: "xai",
      model: "grok-imagine-image-quality",
      requestId: "xai-image-edit-replace-request-1",
      scenario: "local-image-edit-replace",
    });
    const xAiVideoReport = writeProviderPolicyFixture({
      rootDir: tempDir,
      provider: "xai",
      model: "grok-imagine-video",
      requestId: "xai-video-request-1",
      scenario: "xai-video",
      sourceUrl: "https://provider.example/xai-video-request-1/generated-video.mp4",
    });
    const xAiVideoEditReplaceReport = writeProviderPolicyFixture({
      rootDir: tempDir,
      provider: "xai",
      model: "grok-imagine-video",
      requestId: "xai-video-edit-replace-request-1",
      scenario: "local-video-edit-replace",
      sourceUrl: "https://provider.example/xai-video-edit-replace-request-1/generated-video.mp4",
    });
    const xAiTextVideoReplaceReport = writeProviderPolicyFixture({
      rootDir: tempDir,
      provider: "xai",
      model: "grok-imagine-video",
      requestId: "xai-text-video-replace-request-1",
      scenario: "text-to-video-replace",
      sourceUrl: "https://provider.example/xai-text-video-replace-request-1/generated-video.mp4",
    });
    const xAiTextVideoInsertReport = writeProviderPolicyFixture({
      rootDir: tempDir,
      provider: "xai",
      model: "grok-imagine-video",
      requestId: "xai-text-video-insert-request-1",
      scenario: "text-to-video-insert",
      sourceUrl: "https://provider.example/xai-text-video-insert-request-1/generated-video.mp4",
    });
    const googleVideoReport = writeProviderPolicyFixture({
      rootDir: tempDir,
      provider: "google",
      model: "veo3.1-fast",
      requestId: "google-video-request-1",
      scenario: "google-video",
      sourceUrl: "https://provider.example/google-video-request-1/generated-video.mp4",
    });
    const googleTextVideoReplaceReport = writeProviderPolicyFixture({
      rootDir: tempDir,
      provider: "google",
      model: "veo3.1-fast",
      requestId: "google-text-video-replace-request-1",
      scenario: "text-to-video-replace",
      sourceUrl: "https://provider.example/google-text-video-replace-request-1/generated-video.mp4",
    });
    const googleTextVideoInsertReport = writeProviderPolicyFixture({
      rootDir: tempDir,
      provider: "google",
      model: "veo3.1-fast",
      requestId: "google-text-video-insert-request-1",
      scenario: "text-to-video-insert",
      sourceUrl: "https://provider.example/google-text-video-insert-request-1/generated-video.mp4",
    });
    const googleAudioReport = writeProviderPolicyFixture({
      rootDir: tempDir,
      provider: "google",
      model: "gemini-3.1-flash-tts-preview",
      requestId: "google-audio-request-1",
      scenario: "text-to-audio",
      sourceUrl: null,
    });
    const googleLyriaReport = writeProviderPolicyFixture({
      rootDir: tempDir,
      provider: "google",
      model: "lyria3-pro",
      requestId: "google-lyria-request-1",
      scenario: "text-to-music",
      sourceUrl: null,
    });
    const googleLyriaInsertReport = writeProviderPolicyFixture({
      rootDir: tempDir,
      provider: "google",
      model: "lyria3-pro",
      requestId: "google-lyria-insert-request-1",
      scenario: "text-to-music-insert",
      sourceUrl: null,
    });
    const replicateReport = writeProviderPolicyFixture({
      rootDir: tempDir,
      provider: "replicate",
      model: "black-forest-labs/flux-schnell",
      requestId: "replicate-request-1",
    });
    const replicateMultiImageReport = writeProviderPolicyFixture({
      rootDir: tempDir,
      provider: "replicate",
      model: "black-forest-labs/flux-schnell",
      requestId: "replicate-multi-image-request-1",
      scenario: "multi-image",
    });
    const replicateLocalUploadReport = writeReplicateUploadPolicyFixture({
      rootDir: tempDir,
      requestId: "replicate-local-upload-request-1",
    });
    const replicateFluxVariantReports = replicateFluxVariantScenarios.map(
      ({ scenario, model }) =>
        writeProviderPolicyFixture({
          rootDir: tempDir,
          provider: "replicate",
          model,
          requestId: `${scenario}-request-1`,
          scenario,
        }),
    );
    const replicateVideoReport = writeProviderPolicyFixture({
      rootDir: tempDir,
      provider: "replicate",
      model: "bytedance/seedance-2.0",
      requestId: "replicate-video-request-1",
      scenario: "replicate-video",
    });
    const replicateFastVideoReport = writeProviderPolicyFixture({
      rootDir: tempDir,
      provider: "replicate",
      model: "bytedance/seedance-2.0-fast",
      requestId: "replicate-video-fast-request-1",
      scenario: "replicate-video-fast",
    });
    const replicateTextVideoReplaceReport = writeProviderPolicyFixture({
      rootDir: tempDir,
      provider: "replicate",
      model: "bytedance/seedance-2.0",
      requestId: "replicate-text-video-replace-request-1",
      scenario: "text-to-video-replace",
      sourceUrl: "https://provider.example/replicate-text-video-replace-request-1/generated-video.mp4",
    });
    const replicateTextVideoInsertReport = writeProviderPolicyFixture({
      rootDir: tempDir,
      provider: "replicate",
      model: "bytedance/seedance-2.0",
      requestId: "replicate-text-video-insert-request-1",
      scenario: "text-to-video-insert",
      sourceUrl: "https://provider.example/replicate-text-video-insert-request-1/generated-video.mp4",
    });
    const replicateFailureReport = writeProviderPolicyFixture({
      rootDir: tempDir,
      provider: "replicate",
      model: "black-forest-labs/flux-schnell",
      requestId: "replicate-failure-request-1",
      scenario: "provider-failure",
    });
    const replicateCancellationReport = writeProviderPolicyFixture({
      rootDir: tempDir,
      provider: "replicate",
      model: "black-forest-labs/flux-schnell",
      requestId: "replicate-cancel-request-1",
      scenario: "provider-cancellation",
    });
    const replicateRetryReport = writeProviderPolicyFixture({
      rootDir: tempDir,
      provider: "replicate",
      model: "black-forest-labs/flux-schnell",
      requestId: "replicate-retry-request-1",
      scenario: "provider-retry",
    });
    const manifestPath = join(tempDir, "provider-e2e-evidence.json");
    writeFileSync(
      manifestPath,
      JSON.stringify(
        {
          projectRoot: tempDir,
          reports: [
            relativeTo(tempDir, falReport),
            relativeTo(tempDir, falWanReport),
            relativeTo(tempDir, falWanReferenceReport),
            relativeTo(tempDir, falImageEditReport),
            relativeTo(tempDir, falImageEditReplaceReport),
            relativeTo(tempDir, falImageEditRetryReport),
            relativeTo(tempDir, falImageEditCancellationReport),
            relativeTo(tempDir, falImageEditFailureReport),
            relativeTo(tempDir, falKreaReport),
            relativeTo(tempDir, falRecraftReport),
            relativeTo(tempDir, falMultiImageReport),
            relativeTo(tempDir, falKlingReport),
            relativeTo(tempDir, falVideoToVideoReport),
            relativeTo(tempDir, falVideoEditReplaceReport),
            relativeTo(tempDir, falMotionControlReplaceReport),
            relativeTo(tempDir, falTextVideoReplaceReport),
            relativeTo(tempDir, falTextVideoInsertReport),
            relativeTo(tempDir, falVideoEditRetryReport),
            relativeTo(tempDir, falVideoEditCancellationReport),
            relativeTo(tempDir, falVideoEditFailureReport),
            relativeTo(tempDir, falAudioReport),
            relativeTo(tempDir, falMusicReport),
            relativeTo(tempDir, falMusicInsertReport),
            relativeTo(tempDir, falVideoMusicReport),
            relativeTo(tempDir, falVideoMusicInsertReport),
            relativeTo(tempDir, falVideoSfxReport),
            relativeTo(tempDir, falVideoSfxInsertReport),
            relativeTo(tempDir, falImageUpscaleReport),
            relativeTo(tempDir, falImageUpscaleRetryReport),
            relativeTo(tempDir, falImageUpscaleCancellationReport),
            relativeTo(tempDir, falImageUpscaleFailureReport),
            relativeTo(tempDir, falReplaceReport),
            relativeTo(tempDir, falVideoUpscaleReport),
            relativeTo(tempDir, falVideoUpscaleReplaceReport),
            relativeTo(tempDir, falVideoUpscaleRetryReport),
            relativeTo(tempDir, falVideoUpscaleCancellationReport),
            relativeTo(tempDir, falVideoUpscaleFailureReport),
            relativeTo(tempDir, falFailureReport),
            relativeTo(tempDir, falCancellationReport),
            relativeTo(tempDir, falRetryReport),
            relativeTo(tempDir, openAiReport),
            relativeTo(tempDir, openAiTextImageReplaceReport),
            relativeTo(tempDir, openAiMultiImageReport),
            relativeTo(tempDir, openAiImageEditReport),
            relativeTo(tempDir, openAiImageEditReplaceReport),
            relativeTo(tempDir, openAiAudioReport),
            relativeTo(tempDir, elevenLabsAudioReport),
            relativeTo(tempDir, elevenLabsMusicReport),
            relativeTo(tempDir, elevenLabsMusicInsertReport),
            relativeTo(tempDir, minimaxMusicReport),
            relativeTo(tempDir, minimaxMusicInsertReport),
            relativeTo(tempDir, xAiReport),
            relativeTo(tempDir, xAiImageEditReport),
            relativeTo(tempDir, xAiImageEditReplaceReport),
            relativeTo(tempDir, xAiVideoReport),
            relativeTo(tempDir, xAiVideoEditReplaceReport),
            relativeTo(tempDir, xAiTextVideoReplaceReport),
            relativeTo(tempDir, xAiTextVideoInsertReport),
            relativeTo(tempDir, googleVideoReport),
            relativeTo(tempDir, googleTextVideoReplaceReport),
            relativeTo(tempDir, googleTextVideoInsertReport),
            relativeTo(tempDir, googleAudioReport),
            relativeTo(tempDir, googleLyriaReport),
            relativeTo(tempDir, googleLyriaInsertReport),
            relativeTo(tempDir, replicateReport),
            relativeTo(tempDir, replicateMultiImageReport),
            relativeTo(tempDir, replicateLocalUploadReport),
            ...replicateFluxVariantReports.map((report) => relativeTo(tempDir, report)),
            relativeTo(tempDir, replicateVideoReport),
            relativeTo(tempDir, replicateFastVideoReport),
            relativeTo(tempDir, replicateTextVideoReplaceReport),
            relativeTo(tempDir, replicateTextVideoInsertReport),
            relativeTo(tempDir, replicateFailureReport),
            relativeTo(tempDir, replicateCancellationReport),
            relativeTo(tempDir, replicateRetryReport),
          ],
        },
        null,
        2,
      ),
    );

    const result = spawnSync(
      process.execPath,
      [
        join(repoRoot, "scripts/provider-e2e-report-policy.mjs"),
        "--manifest",
        manifestPath,
      ],
      { cwd: repoRoot, encoding: "utf8" },
    );

    expect(result.status).toBe(0);
    expect(JSON.parse(result.stdout)).toMatchObject({
      status: "passed",
      providers: ["elevenlabs", "fal.ai", "google", "minimax", "openai", "replicate", "xai"],
      scenarios: [
        { provider: "elevenlabs", scenario: "text-to-audio" },
        { provider: "elevenlabs", scenario: "text-to-music" },
        { provider: "elevenlabs", scenario: "text-to-music-insert" },
        { provider: "fal.ai", scenario: "image-edit" },
        { provider: "fal.ai", scenario: "kling-image-to-video" },
        { provider: "fal.ai", scenario: "krea-text-to-image" },
        { provider: "fal.ai", scenario: "local-image-edit-cancellation" },
        { provider: "fal.ai", scenario: "local-image-edit-failure" },
        { provider: "fal.ai", scenario: "local-image-edit-replace" },
        { provider: "fal.ai", scenario: "local-image-edit-retry" },
        { provider: "fal.ai", scenario: "local-image-upscale" },
        { provider: "fal.ai", scenario: "local-image-upscale-cancellation" },
        { provider: "fal.ai", scenario: "local-image-upscale-failure" },
        { provider: "fal.ai", scenario: "local-image-upscale-replace" },
        { provider: "fal.ai", scenario: "local-image-upscale-retry" },
        { provider: "fal.ai", scenario: "local-video-edit-cancellation" },
        { provider: "fal.ai", scenario: "local-video-edit-failure" },
        { provider: "fal.ai", scenario: "local-video-edit-replace" },
        { provider: "fal.ai", scenario: "local-video-edit-retry" },
        { provider: "fal.ai", scenario: "local-video-motion-control-replace" },
        { provider: "fal.ai", scenario: "local-video-to-music-insert" },
        { provider: "fal.ai", scenario: "local-video-to-sfx-insert" },
        { provider: "fal.ai", scenario: "local-video-upscale" },
        { provider: "fal.ai", scenario: "local-video-upscale-cancellation" },
        { provider: "fal.ai", scenario: "local-video-upscale-failure" },
        { provider: "fal.ai", scenario: "local-video-upscale-replace" },
        { provider: "fal.ai", scenario: "local-video-upscale-retry" },
        { provider: "fal.ai", scenario: "multi-image" },
        { provider: "fal.ai", scenario: "provider-cancellation" },
        { provider: "fal.ai", scenario: "provider-failure" },
        { provider: "fal.ai", scenario: "provider-retry" },
        { provider: "fal.ai", scenario: "recraft-text-to-image" },
        { provider: "fal.ai", scenario: "text-to-audio" },
        { provider: "fal.ai", scenario: "text-to-image" },
        { provider: "fal.ai", scenario: "text-to-music" },
        { provider: "fal.ai", scenario: "text-to-music-insert" },
        { provider: "fal.ai", scenario: "text-to-video-insert" },
        { provider: "fal.ai", scenario: "text-to-video-replace" },
        { provider: "fal.ai", scenario: "video-to-music" },
        { provider: "fal.ai", scenario: "video-to-sfx" },
        { provider: "fal.ai", scenario: "video-to-video" },
        { provider: "fal.ai", scenario: "wan-image-to-video" },
        { provider: "fal.ai", scenario: "wan-reference-to-video" },
        { provider: "google", scenario: "google-video" },
        { provider: "google", scenario: "text-to-audio" },
        { provider: "google", scenario: "text-to-music" },
        { provider: "google", scenario: "text-to-music-insert" },
        { provider: "google", scenario: "text-to-video-insert" },
        { provider: "google", scenario: "text-to-video-replace" },
        { provider: "minimax", scenario: "text-to-music" },
        { provider: "minimax", scenario: "text-to-music-insert" },
        { provider: "openai", scenario: "image-edit" },
        { provider: "openai", scenario: "local-image-edit-replace" },
        { provider: "openai", scenario: "multi-image" },
        { provider: "openai", scenario: "text-to-audio" },
        { provider: "openai", scenario: "text-to-image" },
        { provider: "openai", scenario: "text-to-image-replace" },
        { provider: "replicate", scenario: "multi-image" },
        { provider: "replicate", scenario: "provider-cancellation" },
        { provider: "replicate", scenario: "provider-failure" },
        { provider: "replicate", scenario: "provider-retry" },
        { provider: "replicate", scenario: "replicate-flux-1.1-pro" },
        { provider: "replicate", scenario: "replicate-flux-1.1-pro-ultra" },
        { provider: "replicate", scenario: "replicate-flux-dev" },
        { provider: "replicate", scenario: "replicate-local-file-upload" },
        { provider: "replicate", scenario: "replicate-video" },
        { provider: "replicate", scenario: "replicate-video-fast" },
        { provider: "replicate", scenario: "text-to-image" },
        { provider: "replicate", scenario: "text-to-video-insert" },
        { provider: "replicate", scenario: "text-to-video-replace" },
        { provider: "xai", scenario: "image-edit" },
        { provider: "xai", scenario: "local-image-edit-replace" },
        { provider: "xai", scenario: "local-video-edit-replace" },
        { provider: "xai", scenario: "text-to-image" },
        { provider: "xai", scenario: "text-to-video-insert" },
        { provider: "xai", scenario: "text-to-video-replace" },
        { provider: "xai", scenario: "xai-video" },
      ],
      failures: [],
    });
  });

  it("rejects malformed retained provider E2E manifests with structured policy output", () => {
    const tempDir = mkdtempSync(join(tmpdir(), "video-creater-provider-policy-manifest-json-"));
    const manifestPath = join(tempDir, "provider-e2e-evidence.json");
    writeFileSync(manifestPath, "{not-json");

    const result = spawnSync(
      process.execPath,
      [
        join(repoRoot, "scripts/provider-e2e-report-policy.mjs"),
        "--manifest",
        manifestPath,
      ],
      { cwd: repoRoot, encoding: "utf8" },
    );

    expect(result.status).toBe(1);
    const report = JSON.parse(result.stdout) as { status: string; failures: string[] };
    expect(report.status).toBe("failed");
    expect(report.failures.some((failure) =>
      failure.includes("provider E2E evidence manifest is not parseable"),
    )).toBe(true);
  });

  it("rejects retained provider E2E manifests with missing providers or unsafe evidence", () => {
    const tempDir = mkdtempSync(join(tmpdir(), "video-creater-provider-policy-bad-"));
    const reportPath = writeProviderPolicyFixture({
      rootDir: tempDir,
      provider: "replicate",
      model: "black-forest-labs/flux-schnell",
      requestId: "replicate-request-1",
      sourceUrl: "http://localhost/generated-output.png",
    });
    const manifestPath = join(tempDir, "provider-e2e-evidence.json");
    writeFileSync(
      manifestPath,
      JSON.stringify(
        {
          projectRoot: tempDir,
          reports: [relativeTo(tempDir, reportPath)],
        },
        null,
        2,
      ),
    );

    const result = spawnSync(
      process.execPath,
      [
        join(repoRoot, "scripts/provider-e2e-report-policy.mjs"),
        "--manifest",
        manifestPath,
      ],
      { cwd: repoRoot, encoding: "utf8" },
    );

    expect(result.status).toBe(1);
    const report = JSON.parse(result.stdout) as { failures: string[] };
    expect(report.failures).toEqual(
      expect.arrayContaining([
        "missing retained provider E2E report for fal.ai scenario text-to-image",
        "missing retained provider E2E report for openai scenario text-to-image",
        "missing retained provider E2E report for openai scenario text-to-image-replace",
        "missing retained provider E2E report for openai scenario multi-image",
        "missing retained provider E2E report for openai scenario image-edit",
        "missing retained provider E2E report for openai scenario local-image-edit-replace",
        "missing retained provider E2E report for openai scenario text-to-audio",
        "missing retained provider E2E report for elevenlabs scenario text-to-audio",
        "missing retained provider E2E report for fal.ai scenario text-to-music",
        "missing retained provider E2E report for elevenlabs scenario text-to-music",
        "missing retained provider E2E report for minimax scenario text-to-music",
        "missing retained provider E2E report for xai scenario text-to-image",
        "missing retained provider E2E report for xai scenario image-edit",
        "missing retained provider E2E report for xai scenario local-image-edit-replace",
        "missing retained provider E2E report for xai scenario xai-video",
        "missing retained provider E2E report for xai scenario local-video-edit-replace",
        "missing retained provider E2E report for xai scenario text-to-video-replace",
        "missing retained provider E2E report for xai scenario text-to-video-insert",
        "missing retained provider E2E report for google scenario google-video",
        "missing retained provider E2E report for google scenario text-to-video-replace",
        "missing retained provider E2E report for google scenario text-to-video-insert",
        "missing retained provider E2E report for google scenario text-to-audio",
        "missing retained provider E2E report for google scenario text-to-music",
        "missing retained provider E2E report for fal.ai scenario krea-text-to-image",
        "missing retained provider E2E report for fal.ai scenario local-image-edit-cancellation",
        "missing retained provider E2E report for fal.ai scenario local-image-edit-failure",
        "missing retained provider E2E report for fal.ai scenario local-image-edit-retry",
        "missing retained provider E2E report for fal.ai scenario recraft-text-to-image",
        "missing retained provider E2E report for fal.ai scenario multi-image",
        "missing retained provider E2E report for fal.ai scenario local-image-upscale-cancellation",
        "missing retained provider E2E report for fal.ai scenario local-image-upscale-failure",
        "missing retained provider E2E report for fal.ai scenario local-image-upscale-retry",
        "missing retained provider E2E report for fal.ai scenario local-video-edit-cancellation",
        "missing retained provider E2E report for fal.ai scenario local-video-edit-failure",
        "missing retained provider E2E report for fal.ai scenario local-video-edit-retry",
        "missing retained provider E2E report for fal.ai scenario local-video-upscale-cancellation",
        "missing retained provider E2E report for fal.ai scenario local-video-upscale-failure",
        "missing retained provider E2E report for fal.ai scenario wan-reference-to-video",
        "missing retained provider E2E report for replicate scenario multi-image",
        "missing retained provider E2E report for replicate scenario replicate-local-file-upload",
        "missing retained provider E2E report for replicate scenario replicate-flux-dev",
        "missing retained provider E2E report for replicate scenario replicate-flux-1.1-pro",
        "missing retained provider E2E report for replicate scenario replicate-flux-1.1-pro-ultra",
        "missing retained provider E2E report for replicate scenario replicate-video-fast",
        "missing retained provider E2E report for replicate scenario text-to-video-insert",
        "missing retained provider E2E report for replicate scenario text-to-video-replace",
      ]),
    );
    expect(report.failures.some((failure) =>
      failure.includes("generated sidecar output sourceUrl must be a provider URL"),
    )).toBe(true);
  });

  it("rejects retained Replicate local upload reports without provider file URL evidence", () => {
    const tempDir = mkdtempSync(join(tmpdir(), "video-creater-provider-policy-upload-url-"));
    const reportPath = writeReplicateUploadPolicyFixture({
      rootDir: tempDir,
      requestId: "replicate-local-upload-request-1",
      providerFileUrl: "http://localhost/uploaded-source.png",
    });
    const manifestPath = join(tempDir, "provider-e2e-evidence.json");
    writeFileSync(
      manifestPath,
      JSON.stringify(
        {
          projectRoot: tempDir,
          reports: [relativeTo(tempDir, reportPath)],
        },
        null,
        2,
      ),
    );

    const result = spawnSync(
      process.execPath,
      [
        join(repoRoot, "scripts/provider-e2e-report-policy.mjs"),
        "--manifest",
        manifestPath,
      ],
      { cwd: repoRoot, encoding: "utf8" },
    );

    expect(result.status).toBe(1);
    const report = JSON.parse(result.stdout) as { failures: string[] };
    expect(report.failures.some((failure) =>
      failure.includes("upload-only result.providerFileUrl must be a provider URL"),
    )).toBe(true);
  });

  it("allows retained MiniMax text-to-music no-spend mocked evidence", () => {
    const tempDir = mkdtempSync(join(tmpdir(), "video-creater-provider-policy-minimax-mock-"));
    const reportPath = writeProviderPolicyFixture({
      rootDir: tempDir,
      provider: "minimax",
      model: "minimax-music-v2.6",
      requestId: "minimax-music-request-1",
      scenario: "text-to-music",
      sourceUrl: null,
    });
    const retainedReport = JSON.parse(readFileSync(reportPath, "utf8")) as {
      credentialStatus: string;
      command: string | null;
      mockedProviderService?: boolean;
    };
    retainedReport.credentialStatus = "not_checked";
    retainedReport.command = null;
    retainedReport.mockedProviderService = true;
    writeFileSync(reportPath, JSON.stringify(retainedReport, null, 2));
    const manifestPath = join(tempDir, "provider-e2e-evidence.json");
    writeFileSync(
      manifestPath,
      JSON.stringify(
        {
          projectRoot: tempDir,
          reports: [relativeTo(tempDir, reportPath)],
        },
        null,
        2,
      ),
    );

    const result = spawnSync(
      process.execPath,
      [
        join(repoRoot, "scripts/provider-e2e-report-policy.mjs"),
        "--manifest",
        manifestPath,
      ],
      { cwd: repoRoot, encoding: "utf8" },
    );

    expect(result.status).toBe(1);
    const report = JSON.parse(result.stdout) as { failures: string[] };
    expect(report.failures).not.toContain(`${reportPath}: credentialStatus must be present`);
    expect(report.failures).not.toContain(`${reportPath}: MiniMax no-spend evidence must not run a provider command`);
    expect(report.failures).not.toContain(`${reportPath}: MiniMax no-spend evidence must be mockedProviderService true`);
  });

  it("allows mocked fal evidence only in explicit no-spend core-provider policy mode", () => {
    const tempDir = mkdtempSync(join(tmpdir(), "video-creater-provider-policy-fal-mock-"));
    const reportPath = writeProviderPolicyFixture({
      rootDir: tempDir,
      provider: "fal.ai",
      model: "fal-ai/flux/schnell",
      requestId: "fal-request-1",
    });
    const retainedReport = JSON.parse(readFileSync(reportPath, "utf8")) as {
      credentialStatus: string;
      command: string | null;
      mockedProviderService?: boolean;
    };
    retainedReport.credentialStatus = "not_checked";
    retainedReport.command = null;
    retainedReport.mockedProviderService = true;
    writeFileSync(reportPath, JSON.stringify(retainedReport, null, 2));
    const manifestPath = join(tempDir, "provider-e2e-evidence.json");
    writeFileSync(
      manifestPath,
      JSON.stringify(
        {
          projectRoot: tempDir,
          reports: [relativeTo(tempDir, reportPath)],
        },
        null,
        2,
      ),
    );

    const releaseResult = spawnSync(
      process.execPath,
      [
        join(repoRoot, "scripts/provider-e2e-report-policy.mjs"),
        "--manifest",
        manifestPath,
      ],
      { cwd: repoRoot, encoding: "utf8" },
    );
    expect(releaseResult.status).toBe(1);
    const releaseReport = JSON.parse(releaseResult.stdout) as { failures: string[] };
    expect(releaseReport.failures).toContain(`${reportPath}: credentialStatus must be present`);

    const noSpendResult = spawnSync(
      process.execPath,
      [
        join(repoRoot, "scripts/provider-e2e-report-policy.mjs"),
        "--manifest",
        manifestPath,
        "--allow-mocked-core-providers",
      ],
      { cwd: repoRoot, encoding: "utf8" },
    );
    expect(noSpendResult.status).toBe(1);
    const noSpendReport = JSON.parse(noSpendResult.stdout) as { failures: string[] };
    expect(noSpendReport.failures).not.toContain(`${reportPath}: credentialStatus must be present`);
    expect(noSpendReport.failures).not.toContain(`${reportPath}: fal.ai no-spend evidence must not run a provider command`);
    expect(noSpendReport.failures).not.toContain(`${reportPath}: fal.ai no-spend evidence must be mockedProviderService true`);
  });

  it("rejects retained text-to-image reports without image artifact evidence", () => {
    const tempDir = mkdtempSync(join(tmpdir(), "video-creater-provider-policy-text-image-"));
    const reportPath = writeProviderPolicyFixture({
      rootDir: tempDir,
      provider: "replicate",
      model: "black-forest-labs/flux-schnell",
      requestId: "replicate-request-1",
    });
    const retainedReport = JSON.parse(readFileSync(reportPath, "utf8")) as {
      result: { projectDir: string; artifactPath: string };
    };
    const sidecarPath = join(
      retainedReport.result.projectDir,
      "generated",
      "replicate-generated",
      "asset.json",
    );
    const mediaIndexPath = join(retainedReport.result.projectDir, "media", "index.json");
    const invalidRelativePath = "generated/replicate-output.bin";
    const invalidOutputPath = join(retainedReport.result.projectDir, invalidRelativePath);
    const sidecar = JSON.parse(readFileSync(sidecarPath, "utf8")) as {
      outputs: Array<{ relativePath: string; width: number; height: number }>;
    };
    const mediaIndex = JSON.parse(readFileSync(mediaIndexPath, "utf8")) as {
      assets: Array<{ relativePath: string; width: number; height: number }>;
    };
    Object.assign(requiredAt(sidecar.outputs, 0, "provider sidecar output"), {
      relativePath: invalidRelativePath,
      width: 0,
      height: 0,
    });
    Object.assign(requiredAt(mediaIndex.assets, 0, "media index asset"), {
      relativePath: invalidRelativePath,
      width: 0,
      height: 0,
    });
    retainedReport.result.artifactPath = invalidOutputPath;
    writeFileSync(invalidOutputPath, "not image bytes");
    writeFileSync(sidecarPath, JSON.stringify(sidecar, null, 2));
    writeFileSync(mediaIndexPath, JSON.stringify(mediaIndex, null, 2));
    writeFileSync(reportPath, JSON.stringify(retainedReport, null, 2));
    const manifestPath = join(tempDir, "provider-e2e-evidence.json");
    writeFileSync(
      manifestPath,
      JSON.stringify(
        {
          projectRoot: tempDir,
          reports: [relativeTo(tempDir, reportPath)],
        },
        null,
        2,
      ),
    );

    const result = spawnSync(
      process.execPath,
      [
        join(repoRoot, "scripts/provider-e2e-report-policy.mjs"),
        "--manifest",
        manifestPath,
      ],
      { cwd: repoRoot, encoding: "utf8" },
    );

    expect(result.status).toBe(1);
    const report = JSON.parse(result.stdout) as { failures: string[] };
    expect(report.failures).toContain(`${reportPath}: text-to-image output must be a PNG artifact`);
    expect(report.failures).toContain(`${reportPath}: text-to-image output width must be positive`);
    expect(report.failures).toContain(`${reportPath}: text-to-image output height must be positive`);
  });

  it("rejects retained OpenAI image-edit reports without multi-reference input evidence", () => {
    const tempDir = mkdtempSync(join(tmpdir(), "video-creater-provider-policy-openai-edit-refs-"));
    const openAiImageEditReport = writeProviderPolicyFixture({
      rootDir: tempDir,
      provider: "openai",
      model: "gpt-image-1.5",
      requestId: "openai-image-edit-request-1",
      scenario: "image-edit",
      sourceUrl: null,
    });
    const retainedReport = JSON.parse(readFileSync(openAiImageEditReport, "utf8")) as {
      result: { projectDir: string };
    };
    const sidecarPath = join(
      retainedReport.result.projectDir,
      "generated",
      "openai-generated",
      "asset.json",
    );
    const sidecar = JSON.parse(readFileSync(sidecarPath, "utf8")) as {
      references?: { mediaIds?: string[]; referenceImageMediaRefs?: string[]; providerInputUrls?: string[] };
    };
    sidecar.references = {
      ...sidecar.references,
      mediaIds: ["provider-e2e-image-ref"],
      referenceImageMediaRefs: ["provider-e2e-image-ref"],
      providerInputUrls: ["data:image/png;base64,bW9jay1vcGVuYWktaW1hZ2UtcmVm"],
    };
    writeFileSync(sidecarPath, JSON.stringify(sidecar, null, 2));
    const manifestPath = join(tempDir, "provider-e2e-evidence.json");
    writeFileSync(
      manifestPath,
      JSON.stringify(
        {
          projectRoot: tempDir,
          reports: [relativeTo(tempDir, openAiImageEditReport)],
        },
        null,
        2,
      ),
    );

    const result = spawnSync(
      process.execPath,
      [
        join(repoRoot, "scripts/provider-e2e-report-policy.mjs"),
        "--manifest",
        manifestPath,
      ],
      { cwd: repoRoot, encoding: "utf8" },
    );

    expect(result.status).toBe(1);
    const report = JSON.parse(result.stdout) as { failures: string[] };
    expect(report.failures).toContain(
      `${openAiImageEditReport}: OpenAI image-edit must retain 2 referenceImageMediaRefs values`,
    );
    expect(report.failures).toContain(
      `${openAiImageEditReport}: OpenAI image-edit must retain 2 provider input URLs`,
    );
  });

  it("rejects retained xAI Grok video reports without reference image provider input evidence", () => {
    const tempDir = mkdtempSync(join(tmpdir(), "video-creater-provider-policy-xai-video-refs-"));
    const xaiVideoReport = writeProviderPolicyFixture({
      rootDir: tempDir,
      provider: "xai",
      model: "grok-imagine-video",
      requestId: "xai-video-request-1",
      scenario: "xai-video",
      sourceUrl: "https://provider.example/xai-video-request-1/generated-video.mp4",
    });
    const retainedReport = JSON.parse(readFileSync(xaiVideoReport, "utf8")) as {
      result: { projectDir: string };
    };
    const sidecarPath = join(
      retainedReport.result.projectDir,
      "generated",
      "xai-generated",
      "asset.json",
    );
    const sidecar = JSON.parse(readFileSync(sidecarPath, "utf8")) as {
      references?: { mediaIds?: string[]; referenceImageMediaRefs?: string[]; providerInputUrls?: string[] };
    };
    sidecar.references = {
      mediaIds: ["provider-e2e-reference-image"],
      providerInputUrls: [],
    };
    writeFileSync(sidecarPath, JSON.stringify(sidecar, null, 2));
    const manifestPath = join(tempDir, "provider-e2e-evidence.json");
    writeFileSync(
      manifestPath,
      JSON.stringify(
        {
          projectRoot: tempDir,
          reports: [relativeTo(tempDir, xaiVideoReport)],
        },
        null,
        2,
      ),
    );

    const result = spawnSync(
      process.execPath,
      [
        join(repoRoot, "scripts/provider-e2e-report-policy.mjs"),
        "--manifest",
        manifestPath,
      ],
      { cwd: repoRoot, encoding: "utf8" },
    );

    expect(result.status).toBe(1);
    const report = JSON.parse(result.stdout) as { failures: string[] };
    expect(report.failures).toContain(
      `${xaiVideoReport}: xAI Grok video must retain one referenceImageMediaRefs value`,
    );
    expect(report.failures).toContain(
      `${xaiVideoReport}: xAI Grok video must retain one provider input URL`,
    );
  });

  it("rejects retained WAN image-to-video reports without typed provider input evidence", () => {
    const tempDir = mkdtempSync(join(tmpdir(), "video-creater-provider-policy-wan-refs-"));
    const falReport = writeProviderPolicyFixture({
      rootDir: tempDir,
      provider: "fal.ai",
      model: "fal-ai/flux/schnell",
      requestId: "fal-request-1",
    });
    const falWanReport = writeProviderPolicyFixture({
      rootDir: tempDir,
      provider: "fal.ai",
      model: "fal-ai/wan/v2.7/image-to-video",
      requestId: "fal-wan-request-1",
      scenario: "wan-image-to-video",
    });
    const replicateReport = writeProviderPolicyFixture({
      rootDir: tempDir,
      provider: "replicate",
      model: "black-forest-labs/flux-schnell",
      requestId: "replicate-request-1",
    });
    const falWan = JSON.parse(readFileSync(falWanReport, "utf8")) as {
      result: { projectDir: string };
    };
    const sidecarPath = join(
      falWan.result.projectDir,
      "generated",
      "fal-ai-generated",
      "asset.json",
    );
    const sidecar = JSON.parse(readFileSync(sidecarPath, "utf8")) as {
      references?: { providerInputUrls?: string[] };
    };
    sidecar.references = {
      ...sidecar.references,
      providerInputUrls: [],
    };
    writeFileSync(sidecarPath, JSON.stringify(sidecar, null, 2));
    const manifestPath = join(tempDir, "provider-e2e-evidence.json");
    writeFileSync(
      manifestPath,
      JSON.stringify(
        {
          projectRoot: tempDir,
          reports: [
            relativeTo(tempDir, falReport),
            relativeTo(tempDir, falWanReport),
            relativeTo(tempDir, replicateReport),
          ],
        },
        null,
        2,
      ),
    );

    const result = spawnSync(
      process.execPath,
      [
        join(repoRoot, "scripts/provider-e2e-report-policy.mjs"),
        "--manifest",
        manifestPath,
      ],
      { cwd: repoRoot, encoding: "utf8" },
    );

    expect(result.status).toBe(1);
    const report = JSON.parse(result.stdout) as { failures: string[] };
    expect(report.failures.some((failure) =>
      failure.includes("referenced video must retain 3 provider input URLs"),
    )).toBe(true);
  });

  it("rejects retained text-to-audio reports without MP3 audio evidence", () => {
    const tempDir = mkdtempSync(join(tmpdir(), "video-creater-provider-policy-audio-output-"));
    const falReport = writeProviderPolicyFixture({
      rootDir: tempDir,
      provider: "fal.ai",
      model: "fal-ai/flux/schnell",
      requestId: "fal-request-1",
    });
    const falWanReport = writeProviderPolicyFixture({
      rootDir: tempDir,
      provider: "fal.ai",
      model: "fal-ai/wan/v2.7/image-to-video",
      requestId: "fal-wan-request-1",
      scenario: "wan-image-to-video",
    });
    const falAudioReport = writeProviderPolicyFixture({
      rootDir: tempDir,
      provider: "fal.ai",
      model: "bytedance/seed-audio-1.0",
      requestId: "fal-audio-request-1",
      scenario: "text-to-audio",
    });
    const replicateReport = writeProviderPolicyFixture({
      rootDir: tempDir,
      provider: "replicate",
      model: "black-forest-labs/flux-schnell",
      requestId: "replicate-request-1",
    });
    const falAudio = JSON.parse(readFileSync(falAudioReport, "utf8")) as {
      result: { projectDir: string };
    };
    const sidecarPath = join(
      falAudio.result.projectDir,
      "generated",
      "fal-ai-generated",
      "asset.json",
    );
    const sidecar = JSON.parse(readFileSync(sidecarPath, "utf8")) as {
      outputs: Array<{ relativePath: string }>;
    };
    requiredAt(sidecar.outputs, 0, "provider sidecar output").relativePath = "generated/fal-ai-output.png";
    writeFileSync(sidecarPath, JSON.stringify(sidecar, null, 2));
    const manifestPath = join(tempDir, "provider-e2e-evidence.json");
    writeFileSync(
      manifestPath,
      JSON.stringify(
        {
          projectRoot: tempDir,
          reports: [
            relativeTo(tempDir, falReport),
            relativeTo(tempDir, falWanReport),
            relativeTo(tempDir, falAudioReport),
            relativeTo(tempDir, replicateReport),
          ],
        },
        null,
        2,
      ),
    );

    const result = spawnSync(
      process.execPath,
      [
        join(repoRoot, "scripts/provider-e2e-report-policy.mjs"),
        "--manifest",
        manifestPath,
      ],
      { cwd: repoRoot, encoding: "utf8" },
    );

    expect(result.status).toBe(1);
    const report = JSON.parse(result.stdout) as { failures: string[] };
    expect(report.failures.some((failure) =>
      failure.includes("text-to-audio output must be an MP3 artifact"),
    )).toBe(true);
  });

  it("rejects retained local image upscale replacement reports without timeline replacement evidence", () => {
    const tempDir = mkdtempSync(join(tmpdir(), "video-creater-provider-policy-replace-"));
    const falReport = writeProviderPolicyFixture({
      rootDir: tempDir,
      provider: "fal.ai",
      model: "fal-ai/flux/schnell",
      requestId: "fal-request-1",
    });
    const falWanReport = writeProviderPolicyFixture({
      rootDir: tempDir,
      provider: "fal.ai",
      model: "fal-ai/wan/v2.7/image-to-video",
      requestId: "fal-wan-request-1",
      scenario: "wan-image-to-video",
    });
    const falAudioReport = writeProviderPolicyFixture({
      rootDir: tempDir,
      provider: "fal.ai",
      model: "bytedance/seed-audio-1.0",
      requestId: "fal-audio-request-1",
      scenario: "text-to-audio",
    });
    const falReplaceReport = writeProviderPolicyFixture({
      rootDir: tempDir,
      provider: "fal.ai",
      model: "fal-ai/aura-sr",
      requestId: "fal-replace-request-1",
      scenario: "local-image-upscale-replace",
    });
    const replicateReport = writeProviderPolicyFixture({
      rootDir: tempDir,
      provider: "replicate",
      model: "black-forest-labs/flux-schnell",
      requestId: "replicate-request-1",
    });
    const falReplace = JSON.parse(readFileSync(falReplaceReport, "utf8")) as {
      result: { projectDir: string };
    };
    const timelinePath = join(falReplace.result.projectDir, "timeline.json");
    const timeline = JSON.parse(readFileSync(timelinePath, "utf8")) as {
      tracks: Array<{ items: Array<{ id: string; source: { mediaId: string } }> }>;
    };
    const item = timeline.tracks.flatMap((track) => track.items).find((candidate) => candidate.id === "item-upscale-target");
    expect(item).toBeTruthy();
    if (item) {
      item.source.mediaId = "provider-e2e-source";
    }
    writeFileSync(timelinePath, JSON.stringify(timeline, null, 2));
    const manifestPath = join(tempDir, "provider-e2e-evidence.json");
    writeFileSync(
      manifestPath,
      JSON.stringify(
        {
          projectRoot: tempDir,
          reports: [
            relativeTo(tempDir, falReport),
            relativeTo(tempDir, falWanReport),
            relativeTo(tempDir, falAudioReport),
            relativeTo(tempDir, falReplaceReport),
            relativeTo(tempDir, replicateReport),
          ],
        },
        null,
        2,
      ),
    );

    const result = spawnSync(
      process.execPath,
      [
        join(repoRoot, "scripts/provider-e2e-report-policy.mjs"),
        "--manifest",
        manifestPath,
      ],
      { cwd: repoRoot, encoding: "utf8" },
    );

    expect(result.status).toBe(1);
    const report = JSON.parse(result.stdout) as { failures: string[] };
    expect(report.failures.some((failure) =>
      failure.includes("replacement scenario target item must point at generated output media"),
    )).toBe(true);
  });

  it("rejects retained direct text-to-image replacement reports without timeline replacement evidence", () => {
    const tempDir = mkdtempSync(join(tmpdir(), "video-creater-provider-policy-direct-image-replace-"));
    const falReport = writeProviderPolicyFixture({
      rootDir: tempDir,
      provider: "fal.ai",
      model: "fal-ai/flux/schnell",
      requestId: "fal-request-1",
    });
    const falWanReport = writeProviderPolicyFixture({
      rootDir: tempDir,
      provider: "fal.ai",
      model: "fal-ai/wan/v2.7/image-to-video",
      requestId: "fal-wan-request-1",
      scenario: "wan-image-to-video",
    });
    const falAudioReport = writeProviderPolicyFixture({
      rootDir: tempDir,
      provider: "fal.ai",
      model: "bytedance/seed-audio-1.0",
      requestId: "fal-audio-request-1",
      scenario: "text-to-audio",
    });
    const openaiReplaceReport = writeProviderPolicyFixture({
      rootDir: tempDir,
      provider: "openai",
      model: "gpt-image-2",
      requestId: "openai-replace-request-1",
      scenario: "text-to-image-replace",
    });
    const replicateReport = writeProviderPolicyFixture({
      rootDir: tempDir,
      provider: "replicate",
      model: "black-forest-labs/flux-schnell",
      requestId: "replicate-request-1",
    });
    const openaiReplace = JSON.parse(readFileSync(openaiReplaceReport, "utf8")) as {
      result: { projectDir: string };
    };
    const timelinePath = join(openaiReplace.result.projectDir, "timeline.json");
    const timeline = JSON.parse(readFileSync(timelinePath, "utf8")) as {
      tracks: Array<{ items: Array<{ id: string; source: { mediaId: string } }> }>;
    };
    const item = timeline.tracks.flatMap((track) => track.items).find((candidate) => candidate.id === "item-text-image-target");
    expect(item).toBeTruthy();
    if (item) {
      item.source.mediaId = "provider-e2e-source";
    }
    writeFileSync(timelinePath, JSON.stringify(timeline, null, 2));
    const manifestPath = join(tempDir, "provider-e2e-evidence.json");
    writeFileSync(
      manifestPath,
      JSON.stringify(
        {
          projectRoot: tempDir,
          reports: [
            relativeTo(tempDir, falReport),
            relativeTo(tempDir, falWanReport),
            relativeTo(tempDir, falAudioReport),
            relativeTo(tempDir, openaiReplaceReport),
            relativeTo(tempDir, replicateReport),
          ],
        },
        null,
        2,
      ),
    );

    const result = spawnSync(
      process.execPath,
      [
        join(repoRoot, "scripts/provider-e2e-report-policy.mjs"),
        "--manifest",
        manifestPath,
      ],
      { cwd: repoRoot, encoding: "utf8" },
    );

    expect(result.status).toBe(1);
    const report = JSON.parse(result.stdout) as { failures: string[] };
    expect(report.failures.some((failure) =>
      failure.includes("replacement scenario target item must point at generated output media"),
    )).toBe(true);
  });

  it("rejects retained local image upscale replacement reports without linked replacement evidence", () => {
    const tempDir = mkdtempSync(join(tmpdir(), "video-creater-provider-policy-linked-replace-"));
    const falReport = writeProviderPolicyFixture({
      rootDir: tempDir,
      provider: "fal.ai",
      model: "fal-ai/flux/schnell",
      requestId: "fal-request-1",
    });
    const falWanReport = writeProviderPolicyFixture({
      rootDir: tempDir,
      provider: "fal.ai",
      model: "fal-ai/wan/v2.7/image-to-video",
      requestId: "fal-wan-request-1",
      scenario: "wan-image-to-video",
    });
    const falAudioReport = writeProviderPolicyFixture({
      rootDir: tempDir,
      provider: "fal.ai",
      model: "bytedance/seed-audio-1.0",
      requestId: "fal-audio-request-1",
      scenario: "text-to-audio",
    });
    const falReplaceReport = writeProviderPolicyFixture({
      rootDir: tempDir,
      provider: "fal.ai",
      model: "fal-ai/aura-sr",
      requestId: "fal-replace-request-1",
      scenario: "local-image-upscale-replace",
    });
    const replicateReport = writeProviderPolicyFixture({
      rootDir: tempDir,
      provider: "replicate",
      model: "black-forest-labs/flux-schnell",
      requestId: "replicate-request-1",
    });
    const falReplace = JSON.parse(readFileSync(falReplaceReport, "utf8")) as {
      result: { projectDir: string };
    };
    const timelinePath = join(falReplace.result.projectDir, "timeline.json");
    const timeline = JSON.parse(readFileSync(timelinePath, "utf8")) as {
      tracks: Array<{ items: Array<{ id: string; source: { mediaId: string } }> }>;
    };
    const linkedItem = timeline.tracks
      .flatMap((track) => track.items)
      .find((candidate) => candidate.id === "item-upscale-target-linked");
    expect(linkedItem).toBeTruthy();
    if (linkedItem) {
      linkedItem.source.mediaId = "provider-e2e-source";
    }
    writeFileSync(timelinePath, JSON.stringify(timeline, null, 2));
    const manifestPath = join(tempDir, "provider-e2e-evidence.json");
    writeFileSync(
      manifestPath,
      JSON.stringify(
        {
          projectRoot: tempDir,
          reports: [
            relativeTo(tempDir, falReport),
            relativeTo(tempDir, falWanReport),
            relativeTo(tempDir, falAudioReport),
            relativeTo(tempDir, falReplaceReport),
            relativeTo(tempDir, replicateReport),
          ],
        },
        null,
        2,
      ),
    );

    const result = spawnSync(
      process.execPath,
      [
        join(repoRoot, "scripts/provider-e2e-report-policy.mjs"),
        "--manifest",
        manifestPath,
      ],
      { cwd: repoRoot, encoding: "utf8" },
    );

    expect(result.status).toBe(1);
    const report = JSON.parse(result.stdout) as { failures: string[] };
    expect(report.failures.some((failure) =>
      failure.includes("replacement scenario linked item must point at generated output media"),
    )).toBe(true);
  });

  it("rejects retained local video upscale replacement reports without linked replacement evidence", () => {
    const tempDir = mkdtempSync(join(tmpdir(), "video-creater-provider-policy-linked-video-replace-"));
    const falReport = writeProviderPolicyFixture({
      rootDir: tempDir,
      provider: "fal.ai",
      model: "fal-ai/flux/schnell",
      requestId: "fal-request-1",
    });
    const falWanReport = writeProviderPolicyFixture({
      rootDir: tempDir,
      provider: "fal.ai",
      model: "fal-ai/wan/v2.7/image-to-video",
      requestId: "fal-wan-request-1",
      scenario: "wan-image-to-video",
    });
    const falAudioReport = writeProviderPolicyFixture({
      rootDir: tempDir,
      provider: "fal.ai",
      model: "bytedance/seed-audio-1.0",
      requestId: "fal-audio-request-1",
      scenario: "text-to-audio",
    });
    const falReplaceReport = writeProviderPolicyFixture({
      rootDir: tempDir,
      provider: "fal.ai",
      model: "fal-ai/video-upscaler",
      requestId: "fal-video-replace-request-1",
      scenario: "local-video-upscale-replace",
    });
    const replicateReport = writeProviderPolicyFixture({
      rootDir: tempDir,
      provider: "replicate",
      model: "black-forest-labs/flux-schnell",
      requestId: "replicate-request-1",
    });
    const falReplace = JSON.parse(readFileSync(falReplaceReport, "utf8")) as {
      result: { projectDir: string };
    };
    const timelinePath = join(falReplace.result.projectDir, "timeline.json");
    const timeline = JSON.parse(readFileSync(timelinePath, "utf8")) as {
      tracks: Array<{ items: Array<{ id: string; source: { mediaId: string } }> }>;
    };
    const linkedItem = timeline.tracks
      .flatMap((track) => track.items)
      .find((candidate) => candidate.id === "item-video-upscale-target-linked");
    expect(linkedItem).toBeTruthy();
    if (linkedItem) {
      linkedItem.source.mediaId = "provider-e2e-video-source";
    }
    writeFileSync(timelinePath, JSON.stringify(timeline, null, 2));
    const manifestPath = join(tempDir, "provider-e2e-evidence.json");
    writeFileSync(
      manifestPath,
      JSON.stringify(
        {
          projectRoot: tempDir,
          reports: [
            relativeTo(tempDir, falReport),
            relativeTo(tempDir, falWanReport),
            relativeTo(tempDir, falAudioReport),
            relativeTo(tempDir, falReplaceReport),
            relativeTo(tempDir, replicateReport),
          ],
        },
        null,
        2,
      ),
    );

    const result = spawnSync(
      process.execPath,
      [
        join(repoRoot, "scripts/provider-e2e-report-policy.mjs"),
        "--manifest",
        manifestPath,
      ],
      { cwd: repoRoot, encoding: "utf8" },
    );

    expect(result.status).toBe(1);
    const report = JSON.parse(result.stdout) as { failures: string[] };
    expect(report.failures.some((failure) =>
      failure.includes("replacement scenario linked item must point at generated output media"),
    )).toBe(true);
  });

  it("rejects retained local image-edit replacement reports without shared replacement link group evidence", () => {
    const tempDir = mkdtempSync(join(tmpdir(), "video-creater-provider-policy-linked-edit-group-"));
    const falReport = writeProviderPolicyFixture({
      rootDir: tempDir,
      provider: "fal.ai",
      model: "fal-ai/flux/schnell",
      requestId: "fal-request-1",
    });
    const falWanReport = writeProviderPolicyFixture({
      rootDir: tempDir,
      provider: "fal.ai",
      model: "fal-ai/wan/v2.7/image-to-video",
      requestId: "fal-wan-request-1",
      scenario: "wan-image-to-video",
    });
    const falAudioReport = writeProviderPolicyFixture({
      rootDir: tempDir,
      provider: "fal.ai",
      model: "bytedance/seed-audio-1.0",
      requestId: "fal-audio-request-1",
      scenario: "text-to-audio",
    });
    const falReplaceReport = writeProviderPolicyFixture({
      rootDir: tempDir,
      provider: "fal.ai",
      model: "fal-ai/nano-banana-pro/edit",
      requestId: "fal-image-edit-replace-request-1",
      scenario: "local-image-edit-replace",
    });
    const replicateReport = writeProviderPolicyFixture({
      rootDir: tempDir,
      provider: "replicate",
      model: "black-forest-labs/flux-schnell",
      requestId: "replicate-request-1",
    });
    const falReplace = JSON.parse(readFileSync(falReplaceReport, "utf8")) as {
      result: { projectDir: string };
    };
    const timelinePath = join(falReplace.result.projectDir, "timeline.json");
    const timeline = JSON.parse(readFileSync(timelinePath, "utf8")) as {
      tracks: Array<{
        items: Array<{ id: string; properties?: { linkGroupId?: string } }>;
      }>;
    };
    for (const item of timeline.tracks.flatMap((track) => track.items)) {
      if (item.id === "item-image-edit-target" || item.id === "item-image-edit-target-linked") {
        if (item.properties) {
          delete item.properties.linkGroupId;
        }
      }
    }
    writeFileSync(timelinePath, JSON.stringify(timeline, null, 2));
    const manifestPath = join(tempDir, "provider-e2e-evidence.json");
    writeFileSync(
      manifestPath,
      JSON.stringify(
        {
          projectRoot: tempDir,
          reports: [
            relativeTo(tempDir, falReport),
            relativeTo(tempDir, falWanReport),
            relativeTo(tempDir, falAudioReport),
            relativeTo(tempDir, falReplaceReport),
            relativeTo(tempDir, replicateReport),
          ],
        },
        null,
        2,
      ),
    );

    const result = spawnSync(
      process.execPath,
      [
        join(repoRoot, "scripts/provider-e2e-report-policy.mjs"),
        "--manifest",
        manifestPath,
      ],
      { cwd: repoRoot, encoding: "utf8" },
    );

    expect(result.status).toBe(1);
    const report = JSON.parse(result.stdout) as { failures: string[] };
    expect(report.failures.some((failure) =>
      failure.includes("replacement scenario linked item must retain the replacement linkGroupId"),
    )).toBe(true);
  });

  it("rejects retained local image upscale replacement reports without replacement placement intent", () => {
    const tempDir = mkdtempSync(join(tmpdir(), "video-creater-provider-policy-replace-intent-"));
    const falReport = writeProviderPolicyFixture({
      rootDir: tempDir,
      provider: "fal.ai",
      model: "fal-ai/flux/schnell",
      requestId: "fal-request-1",
    });
    const falWanReport = writeProviderPolicyFixture({
      rootDir: tempDir,
      provider: "fal.ai",
      model: "fal-ai/wan/v2.7/image-to-video",
      requestId: "fal-wan-request-1",
      scenario: "wan-image-to-video",
    });
    const falAudioReport = writeProviderPolicyFixture({
      rootDir: tempDir,
      provider: "fal.ai",
      model: "bytedance/seed-audio-1.0",
      requestId: "fal-audio-request-1",
      scenario: "text-to-audio",
    });
    const falReplaceReport = writeProviderPolicyFixture({
      rootDir: tempDir,
      provider: "fal.ai",
      model: "fal-ai/aura-sr",
      requestId: "fal-replace-request-1",
      scenario: "local-image-upscale-replace",
    });
    const replicateReport = writeProviderPolicyFixture({
      rootDir: tempDir,
      provider: "replicate",
      model: "black-forest-labs/flux-schnell",
      requestId: "replicate-request-1",
    });
    const falReplace = JSON.parse(readFileSync(falReplaceReport, "utf8")) as {
      result: { projectDir: string };
    };
    const generatedIndexPath = join(falReplace.result.projectDir, "generated", "index.json");
    const generatedIndex = JSON.parse(readFileSync(generatedIndexPath, "utf8")) as {
      assets: Array<{ path: string; status: string }>;
    };
    const completedAsset = generatedIndex.assets.find((asset) => asset.status === "completed");
    expect(completedAsset).toBeTruthy();
    if (completedAsset) {
      const sidecarPath = join(falReplace.result.projectDir, completedAsset.path);
      const sidecar = JSON.parse(readFileSync(sidecarPath, "utf8")) as {
        placementIntent?: string;
      };
      delete sidecar.placementIntent;
      writeFileSync(sidecarPath, JSON.stringify(sidecar, null, 2));
    }
    const manifestPath = join(tempDir, "provider-e2e-evidence.json");
    writeFileSync(
      manifestPath,
      JSON.stringify(
        {
          projectRoot: tempDir,
          reports: [
            relativeTo(tempDir, falReport),
            relativeTo(tempDir, falWanReport),
            relativeTo(tempDir, falAudioReport),
            relativeTo(tempDir, falReplaceReport),
            relativeTo(tempDir, replicateReport),
          ],
        },
        null,
        2,
      ),
    );

    const result = spawnSync(
      process.execPath,
      [
        join(repoRoot, "scripts/provider-e2e-report-policy.mjs"),
        "--manifest",
        manifestPath,
      ],
      { cwd: repoRoot, encoding: "utf8" },
    );

    expect(result.status).toBe(1);
    const report = JSON.parse(result.stdout) as { failures: string[] };
    expect(report.failures.some((failure) =>
      failure.includes("replacement scenario sidecar must retain placementIntent"),
    )).toBe(true);
  });

  it("rejects retained local video-to-music insert reports without audio insertion placement intent", () => {
    const tempDir = mkdtempSync(join(tmpdir(), "video-creater-provider-policy-audio-intent-"));
    const falReport = writeProviderPolicyFixture({
      rootDir: tempDir,
      provider: "fal.ai",
      model: "fal-ai/flux/schnell",
      requestId: "fal-request-1",
    });
    const falWanReport = writeProviderPolicyFixture({
      rootDir: tempDir,
      provider: "fal.ai",
      model: "fal-ai/wan/v2.7/image-to-video",
      requestId: "fal-wan-request-1",
      scenario: "wan-image-to-video",
    });
    const falAudioReport = writeProviderPolicyFixture({
      rootDir: tempDir,
      provider: "fal.ai",
      model: "bytedance/seed-audio-1.0",
      requestId: "fal-audio-request-1",
      scenario: "text-to-audio",
    });
    const falVideoMusicInsertReport = writeProviderPolicyFixture({
      rootDir: tempDir,
      provider: "fal.ai",
      model: "sonilo/v1.1/video-to-music",
      requestId: "fal-video-music-insert-request-1",
      scenario: "local-video-to-music-insert",
    });
    const replicateReport = writeProviderPolicyFixture({
      rootDir: tempDir,
      provider: "replicate",
      model: "black-forest-labs/flux-schnell",
      requestId: "replicate-request-1",
    });
    const falVideoMusicInsert = JSON.parse(
      readFileSync(falVideoMusicInsertReport, "utf8"),
    ) as {
      result: { projectDir: string };
    };
    const generatedIndexPath = join(
      falVideoMusicInsert.result.projectDir,
      "generated",
      "index.json",
    );
    const generatedIndex = JSON.parse(readFileSync(generatedIndexPath, "utf8")) as {
      assets: Array<{ path: string; status: string }>;
    };
    const completedAsset = generatedIndex.assets.find((asset) => asset.status === "completed");
    expect(completedAsset).toBeTruthy();
    if (completedAsset) {
      const sidecarPath = join(falVideoMusicInsert.result.projectDir, completedAsset.path);
      const sidecar = JSON.parse(readFileSync(sidecarPath, "utf8")) as {
        placementIntent?: string;
      };
      delete sidecar.placementIntent;
      writeFileSync(sidecarPath, JSON.stringify(sidecar, null, 2));
    }
    const manifestPath = join(tempDir, "provider-e2e-evidence.json");
    writeFileSync(
      manifestPath,
      JSON.stringify(
        {
          projectRoot: tempDir,
          reports: [
            relativeTo(tempDir, falReport),
            relativeTo(tempDir, falWanReport),
            relativeTo(tempDir, falAudioReport),
            relativeTo(tempDir, falVideoMusicInsertReport),
            relativeTo(tempDir, replicateReport),
          ],
        },
        null,
        2,
      ),
    );

    const result = spawnSync(
      process.execPath,
      [
        join(repoRoot, "scripts/provider-e2e-report-policy.mjs"),
        "--manifest",
        manifestPath,
      ],
      { cwd: repoRoot, encoding: "utf8" },
    );

    expect(result.status).toBe(1);
    const report = JSON.parse(result.stdout) as { failures: string[] };
    expect(report.failures.some((failure) =>
      failure.includes("audio insert scenario sidecar must retain placementIntent"),
    )).toBe(true);
  });

  it("rejects retained text-to-video insert reports without visual insertion placement intent", () => {
    const tempDir = mkdtempSync(join(tmpdir(), "video-creater-provider-policy-visual-intent-"));
    const falReport = writeProviderPolicyFixture({
      rootDir: tempDir,
      provider: "fal.ai",
      model: "fal-ai/flux/schnell",
      requestId: "fal-request-1",
    });
    const falWanReport = writeProviderPolicyFixture({
      rootDir: tempDir,
      provider: "fal.ai",
      model: "fal-ai/wan/v2.7/image-to-video",
      requestId: "fal-wan-request-1",
      scenario: "wan-image-to-video",
    });
    const falAudioReport = writeProviderPolicyFixture({
      rootDir: tempDir,
      provider: "fal.ai",
      model: "bytedance/seed-audio-1.0",
      requestId: "fal-audio-request-1",
      scenario: "text-to-audio",
    });
    const replicateVisualInsertReport = writeProviderPolicyFixture({
      rootDir: tempDir,
      provider: "replicate",
      model: "bytedance/seedance-2.0",
      requestId: "replicate-video-insert-request-1",
      scenario: "text-to-video-insert",
    });
    const replicateReport = writeProviderPolicyFixture({
      rootDir: tempDir,
      provider: "replicate",
      model: "black-forest-labs/flux-schnell",
      requestId: "replicate-request-1",
    });
    const replicateVisualInsert = JSON.parse(
      readFileSync(replicateVisualInsertReport, "utf8"),
    ) as {
      result: { projectDir: string };
    };
    const generatedIndexPath = join(
      replicateVisualInsert.result.projectDir,
      "generated",
      "index.json",
    );
    const generatedIndex = JSON.parse(readFileSync(generatedIndexPath, "utf8")) as {
      assets: Array<{ path: string; status: string }>;
    };
    const completedAsset = generatedIndex.assets.find((asset) => asset.status === "completed");
    expect(completedAsset).toBeTruthy();
    if (completedAsset) {
      const sidecarPath = join(replicateVisualInsert.result.projectDir, completedAsset.path);
      const sidecar = JSON.parse(readFileSync(sidecarPath, "utf8")) as {
        placementIntent?: string;
      };
      delete sidecar.placementIntent;
      writeFileSync(sidecarPath, JSON.stringify(sidecar, null, 2));
    }
    const manifestPath = join(tempDir, "provider-e2e-evidence.json");
    writeFileSync(
      manifestPath,
      JSON.stringify(
        {
          projectRoot: tempDir,
          reports: [
            relativeTo(tempDir, falReport),
            relativeTo(tempDir, falWanReport),
            relativeTo(tempDir, falAudioReport),
            relativeTo(tempDir, replicateVisualInsertReport),
            relativeTo(tempDir, replicateReport),
          ],
        },
        null,
        2,
      ),
    );

    const result = spawnSync(
      process.execPath,
      [
        join(repoRoot, "scripts/provider-e2e-report-policy.mjs"),
        "--manifest",
        manifestPath,
      ],
      { cwd: repoRoot, encoding: "utf8" },
    );

    expect(result.status).toBe(1);
    const report = JSON.parse(result.stdout) as { failures: string[] };
    expect(report.failures.some((failure) =>
      failure.includes("visual insert scenario sidecar must retain placementIntent"),
    )).toBe(true);
  });

  it("rejects retained text-to-video insert reports whose output is not an MP4 artifact", () => {
    const tempDir = mkdtempSync(join(tmpdir(), "video-creater-provider-policy-visual-mp4-"));
    const falReport = writeProviderPolicyFixture({
      rootDir: tempDir,
      provider: "fal.ai",
      model: "fal-ai/flux/schnell",
      requestId: "fal-request-1",
    });
    const falWanReport = writeProviderPolicyFixture({
      rootDir: tempDir,
      provider: "fal.ai",
      model: "fal-ai/wan/v2.7/image-to-video",
      requestId: "fal-wan-request-1",
      scenario: "wan-image-to-video",
    });
    const falAudioReport = writeProviderPolicyFixture({
      rootDir: tempDir,
      provider: "fal.ai",
      model: "bytedance/seed-audio-1.0",
      requestId: "fal-audio-request-1",
      scenario: "text-to-audio",
    });
    const replicateVisualInsertReport = writeProviderPolicyFixture({
      rootDir: tempDir,
      provider: "replicate",
      model: "bytedance/seedance-2.0",
      requestId: "replicate-video-insert-request-1",
      scenario: "text-to-video-insert",
    });
    const replicateReport = writeProviderPolicyFixture({
      rootDir: tempDir,
      provider: "replicate",
      model: "black-forest-labs/flux-schnell",
      requestId: "replicate-request-1",
    });
    const visualInsertReport = JSON.parse(
      readFileSync(replicateVisualInsertReport, "utf8"),
    ) as {
      result: { artifactPath: string; projectDir: string };
    };
    const replacementRelativePath = "generated/replicate-output.png";
    const replacementPath = join(visualInsertReport.result.projectDir, replacementRelativePath);
    writeFileSync(replacementPath, "not mp4 bytes");
    const generatedIndexPath = join(
      visualInsertReport.result.projectDir,
      "generated",
      "index.json",
    );
    const generatedIndex = JSON.parse(readFileSync(generatedIndexPath, "utf8")) as {
      assets: Array<{ path: string; status: string }>;
    };
    const completedAsset = generatedIndex.assets.find((asset) => asset.status === "completed");
    expect(completedAsset).toBeTruthy();
    if (completedAsset) {
      const sidecarPath = join(visualInsertReport.result.projectDir, completedAsset.path);
      const sidecar = JSON.parse(readFileSync(sidecarPath, "utf8")) as {
        outputs: Array<{ mediaId: string; relativePath: string }>;
      };
      requiredAt(sidecar.outputs, 0, "provider sidecar output").relativePath = replacementRelativePath;
      writeFileSync(sidecarPath, JSON.stringify(sidecar, null, 2));
      const mediaIndexPath = join(visualInsertReport.result.projectDir, "media", "index.json");
      const mediaIndex = JSON.parse(readFileSync(mediaIndexPath, "utf8")) as {
        assets: Array<{ id: string; relativePath: string }>;
      };
      const mediaRow = mediaIndex.assets.find((asset) => asset.id === requiredAt(sidecar.outputs, 0, "provider sidecar output").mediaId);
      expect(mediaRow).toBeTruthy();
      if (mediaRow) {
        mediaRow.relativePath = replacementRelativePath;
      }
      writeFileSync(mediaIndexPath, JSON.stringify(mediaIndex, null, 2));
    }
    visualInsertReport.result.artifactPath = replacementPath;
    writeFileSync(replicateVisualInsertReport, JSON.stringify(visualInsertReport, null, 2));
    const manifestPath = join(tempDir, "provider-e2e-evidence.json");
    writeFileSync(
      manifestPath,
      JSON.stringify(
        {
          projectRoot: tempDir,
          reports: [
            relativeTo(tempDir, falReport),
            relativeTo(tempDir, falWanReport),
            relativeTo(tempDir, falAudioReport),
            relativeTo(tempDir, replicateVisualInsertReport),
            relativeTo(tempDir, replicateReport),
          ],
        },
        null,
        2,
      ),
    );

    const result = spawnSync(
      process.execPath,
      [
        join(repoRoot, "scripts/provider-e2e-report-policy.mjs"),
        "--manifest",
        manifestPath,
      ],
      { cwd: repoRoot, encoding: "utf8" },
    );

    expect(result.status).toBe(1);
    const report = JSON.parse(result.stdout) as { failures: string[] };
    expect(report.failures).toContain(
      `${replicateVisualInsertReport}: referenced video output must be an MP4 artifact`,
    );
  });

  it("rejects retained local video-to-music insert reports without selected source trim settings", () => {
    const tempDir = mkdtempSync(join(tmpdir(), "video-creater-provider-policy-audio-trim-"));
    const falReport = writeProviderPolicyFixture({
      rootDir: tempDir,
      provider: "fal.ai",
      model: "fal-ai/flux/schnell",
      requestId: "fal-request-1",
    });
    const falWanReport = writeProviderPolicyFixture({
      rootDir: tempDir,
      provider: "fal.ai",
      model: "fal-ai/wan/v2.7/image-to-video",
      requestId: "fal-wan-request-1",
      scenario: "wan-image-to-video",
    });
    const falAudioReport = writeProviderPolicyFixture({
      rootDir: tempDir,
      provider: "fal.ai",
      model: "bytedance/seed-audio-1.0",
      requestId: "fal-audio-request-1",
      scenario: "text-to-audio",
    });
    const falVideoMusicInsertReport = writeProviderPolicyFixture({
      rootDir: tempDir,
      provider: "fal.ai",
      model: "sonilo/v1.1/video-to-music",
      requestId: "fal-video-music-insert-request-1",
      scenario: "local-video-to-music-insert",
    });
    const replicateReport = writeProviderPolicyFixture({
      rootDir: tempDir,
      provider: "replicate",
      model: "black-forest-labs/flux-schnell",
      requestId: "replicate-request-1",
    });
    const falVideoMusicInsert = JSON.parse(
      readFileSync(falVideoMusicInsertReport, "utf8"),
    ) as {
      result: { projectDir: string };
    };
    const generatedIndexPath = join(
      falVideoMusicInsert.result.projectDir,
      "generated",
      "index.json",
    );
    const generatedIndex = JSON.parse(readFileSync(generatedIndexPath, "utf8")) as {
      assets: Array<{ path: string; status: string }>;
    };
    const completedAsset = generatedIndex.assets.find((asset) => asset.status === "completed");
    expect(completedAsset).toBeTruthy();
    if (completedAsset) {
      const sidecarPath = join(falVideoMusicInsert.result.projectDir, completedAsset.path);
      const sidecar = JSON.parse(readFileSync(sidecarPath, "utf8")) as {
        settings?: { videoSourceEndSeconds?: number };
      };
      if (sidecar.settings) {
        delete sidecar.settings.videoSourceEndSeconds;
      }
      writeFileSync(sidecarPath, JSON.stringify(sidecar, null, 2));
    }
    const manifestPath = join(tempDir, "provider-e2e-evidence.json");
    writeFileSync(
      manifestPath,
      JSON.stringify(
        {
          projectRoot: tempDir,
          reports: [
            relativeTo(tempDir, falReport),
            relativeTo(tempDir, falWanReport),
            relativeTo(tempDir, falAudioReport),
            relativeTo(tempDir, falVideoMusicInsertReport),
            relativeTo(tempDir, replicateReport),
          ],
        },
        null,
        2,
      ),
    );

    const result = spawnSync(
      process.execPath,
      [
        join(repoRoot, "scripts/provider-e2e-report-policy.mjs"),
        "--manifest",
        manifestPath,
      ],
      { cwd: repoRoot, encoding: "utf8" },
    );

    expect(result.status).toBe(1);
    const report = JSON.parse(result.stdout) as { failures: string[] };
    expect(report.failures.some((failure) =>
      failure.includes("audio insert scenario sidecar must retain selected source trim settings"),
    )).toBe(true);
  });

  it("rejects retained local video-to-music insert reports without prompt guidance", () => {
    const tempDir = mkdtempSync(join(tmpdir(), "video-creater-provider-policy-audio-prompt-"));
    const falReport = writeProviderPolicyFixture({
      rootDir: tempDir,
      provider: "fal.ai",
      model: "fal-ai/flux/schnell",
      requestId: "fal-request-1",
    });
    const falWanReport = writeProviderPolicyFixture({
      rootDir: tempDir,
      provider: "fal.ai",
      model: "fal-ai/wan/v2.7/image-to-video",
      requestId: "fal-wan-request-1",
      scenario: "wan-image-to-video",
    });
    const falAudioReport = writeProviderPolicyFixture({
      rootDir: tempDir,
      provider: "fal.ai",
      model: "bytedance/seed-audio-1.0",
      requestId: "fal-audio-request-1",
      scenario: "text-to-audio",
    });
    const falVideoMusicInsertReport = writeProviderPolicyFixture({
      rootDir: tempDir,
      provider: "fal.ai",
      model: "sonilo/v1.1/video-to-music",
      requestId: "fal-video-music-insert-request-1",
      scenario: "local-video-to-music-insert",
    });
    const replicateReport = writeProviderPolicyFixture({
      rootDir: tempDir,
      provider: "replicate",
      model: "black-forest-labs/flux-schnell",
      requestId: "replicate-request-1",
    });
    const falVideoMusicInsert = JSON.parse(
      readFileSync(falVideoMusicInsertReport, "utf8"),
    ) as {
      result: { projectDir: string };
    };
    const generatedIndexPath = join(
      falVideoMusicInsert.result.projectDir,
      "generated",
      "index.json",
    );
    const generatedIndex = JSON.parse(readFileSync(generatedIndexPath, "utf8")) as {
      assets: Array<{ path: string; status: string }>;
    };
    const completedAsset = generatedIndex.assets.find((asset) => asset.status === "completed");
    expect(completedAsset).toBeTruthy();
    if (completedAsset) {
      const sidecarPath = join(falVideoMusicInsert.result.projectDir, completedAsset.path);
      const sidecar = JSON.parse(readFileSync(sidecarPath, "utf8")) as {
        prompt?: string;
      };
      sidecar.prompt = "";
      writeFileSync(sidecarPath, JSON.stringify(sidecar, null, 2));
    }
    const manifestPath = join(tempDir, "provider-e2e-evidence.json");
    writeFileSync(
      manifestPath,
      JSON.stringify(
        {
          projectRoot: tempDir,
          reports: [
            relativeTo(tempDir, falReport),
            relativeTo(tempDir, falWanReport),
            relativeTo(tempDir, falAudioReport),
            relativeTo(tempDir, falVideoMusicInsertReport),
            relativeTo(tempDir, replicateReport),
          ],
        },
        null,
        2,
      ),
    );

    const result = spawnSync(
      process.execPath,
      [
        join(repoRoot, "scripts/provider-e2e-report-policy.mjs"),
        "--manifest",
        manifestPath,
      ],
      { cwd: repoRoot, encoding: "utf8" },
    );

    expect(result.status).toBe(1);
    const report = JSON.parse(result.stdout) as { failures: string[] };
    expect(report.failures.some((failure) =>
      failure.includes("video-to-music insert sidecar must retain prompt guidance"),
    )).toBe(true);
  });

  it("rejects retained local video-to-music insert reports with mismatched inserted audio duration evidence", () => {
    const tempDir = mkdtempSync(join(tmpdir(), "video-creater-provider-policy-audio-duration-"));
    const falReport = writeProviderPolicyFixture({
      rootDir: tempDir,
      provider: "fal.ai",
      model: "fal-ai/flux/schnell",
      requestId: "fal-request-1",
    });
    const falWanReport = writeProviderPolicyFixture({
      rootDir: tempDir,
      provider: "fal.ai",
      model: "fal-ai/wan/v2.7/image-to-video",
      requestId: "fal-wan-request-1",
      scenario: "wan-image-to-video",
    });
    const falAudioReport = writeProviderPolicyFixture({
      rootDir: tempDir,
      provider: "fal.ai",
      model: "bytedance/seed-audio-1.0",
      requestId: "fal-audio-request-1",
      scenario: "text-to-audio",
    });
    const falVideoMusicInsertReport = writeProviderPolicyFixture({
      rootDir: tempDir,
      provider: "fal.ai",
      model: "sonilo/v1.1/video-to-music",
      requestId: "fal-video-music-insert-request-1",
      scenario: "local-video-to-music-insert",
    });
    const replicateReport = writeProviderPolicyFixture({
      rootDir: tempDir,
      provider: "replicate",
      model: "black-forest-labs/flux-schnell",
      requestId: "replicate-request-1",
    });
    const falVideoMusicInsert = JSON.parse(
      readFileSync(falVideoMusicInsertReport, "utf8"),
    ) as {
      result: { projectDir: string };
    };
    const timelinePath = join(falVideoMusicInsert.result.projectDir, "timeline.json");
    const timeline = JSON.parse(readFileSync(timelinePath, "utf8")) as {
      tracks: Array<{
        items: Array<{
          id: string;
          properties?: { sourceOut?: number };
        }>;
      }>;
    };
    const insertedItem = timeline.tracks
      .flatMap((track) => track.items)
      .find((item) => item.id === "item-video-to-music-inserted-audio");
    expect(insertedItem).toBeTruthy();
    if (insertedItem?.properties) {
      insertedItem.properties.sourceOut = 3;
    }
    writeFileSync(timelinePath, JSON.stringify(timeline, null, 2));
    const manifestPath = join(tempDir, "provider-e2e-evidence.json");
    writeFileSync(
      manifestPath,
      JSON.stringify(
        {
          projectRoot: tempDir,
          reports: [
            relativeTo(tempDir, falReport),
            relativeTo(tempDir, falWanReport),
            relativeTo(tempDir, falAudioReport),
            relativeTo(tempDir, falVideoMusicInsertReport),
            relativeTo(tempDir, replicateReport),
          ],
        },
        null,
        2,
      ),
    );

    const result = spawnSync(
      process.execPath,
      [
        join(repoRoot, "scripts/provider-e2e-report-policy.mjs"),
        "--manifest",
        manifestPath,
      ],
      { cwd: repoRoot, encoding: "utf8" },
    );

    expect(result.status).toBe(1);
    const report = JSON.parse(result.stdout) as { failures: string[] };
    expect(report.failures.some((failure) =>
      failure.includes("audio insert scenario sourceOut must match generated output duration"),
    )).toBe(true);
  });

  it("rejects retained local video-to-music insert reports without selected source video trim evidence", () => {
    const tempDir = mkdtempSync(join(tmpdir(), "video-creater-provider-policy-source-trim-"));
    const falReport = writeProviderPolicyFixture({
      rootDir: tempDir,
      provider: "fal.ai",
      model: "fal-ai/flux/schnell",
      requestId: "fal-request-1",
    });
    const falWanReport = writeProviderPolicyFixture({
      rootDir: tempDir,
      provider: "fal.ai",
      model: "fal-ai/wan/v2.7/image-to-video",
      requestId: "fal-wan-request-1",
      scenario: "wan-image-to-video",
    });
    const falAudioReport = writeProviderPolicyFixture({
      rootDir: tempDir,
      provider: "fal.ai",
      model: "bytedance/seed-audio-1.0",
      requestId: "fal-audio-request-1",
      scenario: "text-to-audio",
    });
    const falVideoMusicInsertReport = writeProviderPolicyFixture({
      rootDir: tempDir,
      provider: "fal.ai",
      model: "sonilo/v1.1/video-to-music",
      requestId: "fal-video-music-insert-request-1",
      scenario: "local-video-to-music-insert",
    });
    const replicateReport = writeProviderPolicyFixture({
      rootDir: tempDir,
      provider: "replicate",
      model: "black-forest-labs/flux-schnell",
      requestId: "replicate-request-1",
    });
    const falVideoMusicInsert = JSON.parse(
      readFileSync(falVideoMusicInsertReport, "utf8"),
    ) as {
      result: { projectDir: string };
    };
    const timelinePath = join(falVideoMusicInsert.result.projectDir, "timeline.json");
    const timeline = JSON.parse(readFileSync(timelinePath, "utf8")) as {
      tracks: Array<{
        items: Array<{
          id: string;
          properties?: { sourceOut?: number };
        }>;
      }>;
    };
    const sourceItem = timeline.tracks
      .flatMap((track) => track.items)
      .find((item) => item.id === "item-video-to-music-source");
    expect(sourceItem).toBeTruthy();
    if (sourceItem?.properties) {
      sourceItem.properties.sourceOut = 99;
    }
    writeFileSync(timelinePath, JSON.stringify(timeline, null, 2));
    const manifestPath = join(tempDir, "provider-e2e-evidence.json");
    writeFileSync(
      manifestPath,
      JSON.stringify(
        {
          projectRoot: tempDir,
          reports: [
            relativeTo(tempDir, falReport),
            relativeTo(tempDir, falWanReport),
            relativeTo(tempDir, falAudioReport),
            relativeTo(tempDir, falVideoMusicInsertReport),
            relativeTo(tempDir, replicateReport),
          ],
        },
        null,
        2,
      ),
    );

    const result = spawnSync(
      process.execPath,
      [
        join(repoRoot, "scripts/provider-e2e-report-policy.mjs"),
        "--manifest",
        manifestPath,
      ],
      { cwd: repoRoot, encoding: "utf8" },
    );

    expect(result.status).toBe(1);
    const report = JSON.parse(result.stdout) as { failures: string[] };
    expect(report.failures.some((failure) =>
      failure.includes("audio insert scenario source item must retain selected source trim"),
    )).toBe(true);
  });

  it("rejects retained provider E2E manifests with malformed report JSON", () => {
    const tempDir = mkdtempSync(join(tmpdir(), "video-creater-provider-policy-report-json-"));
    const falReport = writeProviderPolicyFixture({
      rootDir: tempDir,
      provider: "fal.ai",
      model: "fal-ai/flux/schnell",
      requestId: "fal-request-1",
    });
    const replicateReport = writeProviderPolicyFixture({
      rootDir: tempDir,
      provider: "replicate",
      model: "black-forest-labs/flux-schnell",
      requestId: "replicate-request-1",
    });
    writeFileSync(replicateReport, "{not-json");
    const manifestPath = join(tempDir, "provider-e2e-evidence.json");
    writeFileSync(
      manifestPath,
      JSON.stringify(
        {
          projectRoot: tempDir,
          reports: [
            relativeTo(tempDir, falReport),
            relativeTo(tempDir, replicateReport),
          ],
        },
        null,
        2,
      ),
    );

    const result = spawnSync(
      process.execPath,
      [
        join(repoRoot, "scripts/provider-e2e-report-policy.mjs"),
        "--manifest",
        manifestPath,
      ],
      { cwd: repoRoot, encoding: "utf8" },
    );

    expect(result.status).toBe(1);
    const report = JSON.parse(result.stdout) as { failures: string[] };
    expect(report.failures.some((failure) =>
      failure.includes("provider E2E report is not parseable"),
    )).toBe(true);
  });

  it("rejects retained provider E2E manifests with missing report files", () => {
    const tempDir = mkdtempSync(join(tmpdir(), "video-creater-provider-policy-missing-report-"));
    const falReport = writeProviderPolicyFixture({
      rootDir: tempDir,
      provider: "fal.ai",
      model: "fal-ai/flux/schnell",
      requestId: "fal-request-1",
    });
    const missingReport = join(tempDir, "output", "provider-e2e", "replicate", "provider-e2e-report.json");
    const manifestPath = join(tempDir, "provider-e2e-evidence.json");
    writeFileSync(
      manifestPath,
      JSON.stringify(
        {
          projectRoot: tempDir,
          reports: [
            relativeTo(tempDir, falReport),
            relativeTo(tempDir, missingReport),
          ],
        },
        null,
        2,
      ),
    );

    const result = spawnSync(
      process.execPath,
      [
        join(repoRoot, "scripts/provider-e2e-report-policy.mjs"),
        "--manifest",
        manifestPath,
      ],
      { cwd: repoRoot, encoding: "utf8" },
    );

    expect(result.status).toBe(1);
    const report = JSON.parse(result.stdout) as { failures: string[] };
    expect(report.failures.some((failure) =>
      failure.includes("provider E2E report is missing"),
    )).toBe(true);
    expect(report.failures.some((failure) =>
      failure.includes("missing retained provider E2E report for fal.ai scenario local-video-upscale"),
    )).toBe(true);
  });

  it("rejects retained provider E2E reports whose artifact is not a completed sidecar output", () => {
    const tempDir = mkdtempSync(join(tmpdir(), "video-creater-provider-policy-artifact-"));
    const falReport = writeProviderPolicyFixture({
      rootDir: tempDir,
      provider: "fal.ai",
      model: "fal-ai/flux/schnell",
      requestId: "fal-request-1",
    });
    const replicateReport = writeProviderPolicyFixture({
      rootDir: tempDir,
      provider: "replicate",
      model: "black-forest-labs/flux-schnell",
      requestId: "replicate-request-1",
    });
    const replicate = JSON.parse(readFileSync(replicateReport, "utf8")) as {
      result: { projectDir: string; artifactPath: string };
    };
    const strayArtifact = join(replicate.result.projectDir, "generated", "stray-output.png");
    writeFileSync(strayArtifact, "retained but not sidecar-backed");
    replicate.result.artifactPath = strayArtifact;
    writeFileSync(replicateReport, JSON.stringify(replicate, null, 2));
    const manifestPath = join(tempDir, "provider-e2e-evidence.json");
    writeFileSync(
      manifestPath,
      JSON.stringify(
        {
          projectRoot: tempDir,
          reports: [
            relativeTo(tempDir, falReport),
            relativeTo(tempDir, replicateReport),
          ],
        },
        null,
        2,
      ),
    );

    const result = spawnSync(
      process.execPath,
      [
        join(repoRoot, "scripts/provider-e2e-report-policy.mjs"),
        "--manifest",
        manifestPath,
      ],
      { cwd: repoRoot, encoding: "utf8" },
    );

    expect(result.status).toBe(1);
    const report = JSON.parse(result.stdout) as { failures: string[] };
    expect(report.failures.some((failure) =>
      failure.includes("result.artifactPath must match a completed sidecar output"),
    )).toBe(true);
  });

  it("rejects retained provider E2E reports whose output count does not match completed sidecar outputs", () => {
    const tempDir = mkdtempSync(join(tmpdir(), "video-creater-provider-policy-output-count-"));
    const falReport = writeProviderPolicyFixture({
      rootDir: tempDir,
      provider: "fal.ai",
      model: "fal-ai/flux/schnell",
      requestId: "fal-request-1",
    });
    const replicateReport = writeProviderPolicyFixture({
      rootDir: tempDir,
      provider: "replicate",
      model: "black-forest-labs/flux-schnell",
      requestId: "replicate-request-1",
    });
    const replicate = JSON.parse(readFileSync(replicateReport, "utf8")) as {
      result: { outputCount: number };
    };
    replicate.result.outputCount = 2;
    writeFileSync(replicateReport, JSON.stringify(replicate, null, 2));
    const manifestPath = join(tempDir, "provider-e2e-evidence.json");
    writeFileSync(
      manifestPath,
      JSON.stringify(
        {
          projectRoot: tempDir,
          reports: [
            relativeTo(tempDir, falReport),
            relativeTo(tempDir, replicateReport),
          ],
        },
        null,
        2,
      ),
    );

    const result = spawnSync(
      process.execPath,
      [
        join(repoRoot, "scripts/provider-e2e-report-policy.mjs"),
        "--manifest",
        manifestPath,
      ],
      { cwd: repoRoot, encoding: "utf8" },
    );

    expect(result.status).toBe(1);
    const report = JSON.parse(result.stdout) as { failures: string[] };
    expect(report.failures.some((failure) =>
      failure.includes("result.outputCount must match completed sidecar outputs"),
    )).toBe(true);
  });

  it("rejects retained provider E2E reports with empty completed sidecar output files", () => {
    const tempDir = mkdtempSync(join(tmpdir(), "video-creater-provider-policy-empty-output-"));
    const falReport = writeProviderPolicyFixture({
      rootDir: tempDir,
      provider: "fal.ai",
      model: "fal-ai/flux/schnell",
      requestId: "fal-request-1",
    });
    const replicateReport = writeProviderPolicyFixture({
      rootDir: tempDir,
      provider: "replicate",
      model: "black-forest-labs/flux-schnell",
      requestId: "replicate-request-1",
    });
    const replicate = JSON.parse(readFileSync(replicateReport, "utf8")) as {
      result: { outputCount: number; projectDir: string };
    };
    const generatedId = "replicate-generated";
    const emptyMediaId = "replicate-empty-output";
    const emptyOutput = join(replicate.result.projectDir, "generated", `${emptyMediaId}.png`);
    const sidecarPath = join(
      replicate.result.projectDir,
      "generated",
      generatedId,
      "asset.json",
    );
    const mediaIndexPath = join(replicate.result.projectDir, "media", "index.json");
    const generatedIndexPath = join(replicate.result.projectDir, "generated", "index.json");
    writeFileSync(emptyOutput, "");
    const sidecar = JSON.parse(readFileSync(sidecarPath, "utf8")) as {
      outputs: unknown[];
    };
    sidecar.outputs.push({
      mediaId: emptyMediaId,
      relativePath: `generated/${emptyMediaId}.png`,
      sourceUrl: "https://provider.example/replicate-request-1/empty-output.png",
      width: 1024,
      height: 768,
      durationSeconds: 0,
      fps: 0,
    });
    writeFileSync(sidecarPath, JSON.stringify(sidecar, null, 2));
    const mediaIndex = JSON.parse(readFileSync(mediaIndexPath, "utf8")) as {
      assets: unknown[];
    };
    mediaIndex.assets.push({
      id: emptyMediaId,
      relativePath: `generated/${emptyMediaId}.png`,
      kind: "generated",
      durationSeconds: 0,
      width: 1024,
      height: 768,
      fps: 0,
    });
    writeFileSync(mediaIndexPath, JSON.stringify(mediaIndex, null, 2));
    const generatedIndex = JSON.parse(readFileSync(generatedIndexPath, "utf8")) as {
      assets: Array<{ outputCount: number }>;
    };
    requiredAt(generatedIndex.assets, 0, "generated index asset").outputCount = 2;
    writeFileSync(generatedIndexPath, JSON.stringify(generatedIndex, null, 2));
    replicate.result.outputCount = 2;
    writeFileSync(replicateReport, JSON.stringify(replicate, null, 2));
    const manifestPath = join(tempDir, "provider-e2e-evidence.json");
    writeFileSync(
      manifestPath,
      JSON.stringify(
        {
          projectRoot: tempDir,
          reports: [
            relativeTo(tempDir, falReport),
            relativeTo(tempDir, replicateReport),
          ],
        },
        null,
        2,
      ),
    );

    const result = spawnSync(
      process.execPath,
      [
        join(repoRoot, "scripts/provider-e2e-report-policy.mjs"),
        "--manifest",
        manifestPath,
      ],
      { cwd: repoRoot, encoding: "utf8" },
    );

    expect(result.status).toBe(1);
    const report = JSON.parse(result.stdout) as { failures: string[] };
    expect(report.failures.some((failure) =>
      failure.includes("generated sidecar output must be a retained nonempty file"),
    )).toBe(true);
  });

  it("rejects retained provider E2E reports whose sidecar output is missing a generated media row", () => {
    const tempDir = mkdtempSync(join(tmpdir(), "video-creater-provider-policy-missing-output-media-row-"));
    const falReport = writeProviderPolicyFixture({
      rootDir: tempDir,
      provider: "fal.ai",
      model: "fal-ai/flux/schnell",
      requestId: "fal-request-1",
    });
    const replicateReport = writeProviderPolicyFixture({
      rootDir: tempDir,
      provider: "replicate",
      model: "black-forest-labs/flux-schnell",
      requestId: "replicate-request-1",
    });
    const replicate = JSON.parse(readFileSync(replicateReport, "utf8")) as {
      result: { projectDir: string };
    };
    const mediaIndexPath = join(replicate.result.projectDir, "media", "index.json");
    const mediaIndex = JSON.parse(readFileSync(mediaIndexPath, "utf8")) as {
      assets: Array<{ id: string; kind: string }>;
    };
    requiredAt(mediaIndex.assets, 0, "media index asset").kind = "video";
    writeFileSync(mediaIndexPath, JSON.stringify(mediaIndex, null, 2));
    const manifestPath = join(tempDir, "provider-e2e-evidence.json");
    writeFileSync(
      manifestPath,
      JSON.stringify(
        {
          projectRoot: tempDir,
          reports: [
            relativeTo(tempDir, falReport),
            relativeTo(tempDir, replicateReport),
          ],
        },
        null,
        2,
      ),
    );

    const result = spawnSync(
      process.execPath,
      [
        join(repoRoot, "scripts/provider-e2e-report-policy.mjs"),
        "--manifest",
        manifestPath,
      ],
      { cwd: repoRoot, encoding: "utf8" },
    );

    expect(result.status).toBe(1);
    const report = JSON.parse(result.stdout) as { failures: string[] };
    expect(report.failures).toContain(
      `${replicateReport}: completed sidecar output is missing generated media row: replicate-output`,
    );
  });

  it("rejects retained provider E2E reports with duplicate generated media rows", () => {
    const tempDir = mkdtempSync(join(tmpdir(), "video-creater-provider-policy-duplicate-media-row-"));
    const falReport = writeProviderPolicyFixture({
      rootDir: tempDir,
      provider: "fal.ai",
      model: "fal-ai/flux/schnell",
      requestId: "fal-request-1",
    });
    const replicateReport = writeProviderPolicyFixture({
      rootDir: tempDir,
      provider: "replicate",
      model: "black-forest-labs/flux-schnell",
      requestId: "replicate-request-1",
    });
    const replicate = JSON.parse(readFileSync(replicateReport, "utf8")) as {
      result: { projectDir: string };
    };
    const mediaIndexPath = join(replicate.result.projectDir, "media", "index.json");
    const mediaIndex = JSON.parse(readFileSync(mediaIndexPath, "utf8")) as {
      assets: Array<{ id: string; kind: string }>;
    };
    mediaIndex.assets.push({ ...requiredAt(mediaIndex.assets, 0, "media index asset") });
    writeFileSync(mediaIndexPath, JSON.stringify(mediaIndex, null, 2));
    const manifestPath = join(tempDir, "provider-e2e-evidence.json");
    writeFileSync(
      manifestPath,
      JSON.stringify(
        {
          projectRoot: tempDir,
          reports: [
            relativeTo(tempDir, falReport),
            relativeTo(tempDir, replicateReport),
          ],
        },
        null,
        2,
      ),
    );

    const result = spawnSync(
      process.execPath,
      [
        join(repoRoot, "scripts/provider-e2e-report-policy.mjs"),
        "--manifest",
        manifestPath,
      ],
      { cwd: repoRoot, encoding: "utf8" },
    );

    expect(result.status).toBe(1);
    const report = JSON.parse(result.stdout) as { failures: string[] };
    expect(report.failures).toContain(
      `${replicateReport}: duplicate generated media row id: replicate-output`,
    );
  });

  it("rejects retained provider E2E reports whose media row path does not match the sidecar output", () => {
    const tempDir = mkdtempSync(join(tmpdir(), "video-creater-provider-policy-media-path-"));
    const falReport = writeProviderPolicyFixture({
      rootDir: tempDir,
      provider: "fal.ai",
      model: "fal-ai/flux/schnell",
      requestId: "fal-request-1",
    });
    const replicateReport = writeProviderPolicyFixture({
      rootDir: tempDir,
      provider: "replicate",
      model: "black-forest-labs/flux-schnell",
      requestId: "replicate-request-1",
    });
    const replicate = JSON.parse(readFileSync(replicateReport, "utf8")) as {
      result: { projectDir: string };
    };
    const mediaIndexPath = join(replicate.result.projectDir, "media", "index.json");
    const mediaIndex = JSON.parse(readFileSync(mediaIndexPath, "utf8")) as {
      assets: Array<{ id: string; relativePath: string }>;
    };
    requiredAt(mediaIndex.assets, 0, "media index asset").relativePath = "generated/not-the-sidecar-output.png";
    writeFileSync(mediaIndexPath, JSON.stringify(mediaIndex, null, 2));
    const manifestPath = join(tempDir, "provider-e2e-evidence.json");
    writeFileSync(
      manifestPath,
      JSON.stringify(
        {
          projectRoot: tempDir,
          reports: [
            relativeTo(tempDir, falReport),
            relativeTo(tempDir, replicateReport),
          ],
        },
        null,
        2,
      ),
    );

    const result = spawnSync(
      process.execPath,
      [
        join(repoRoot, "scripts/provider-e2e-report-policy.mjs"),
        "--manifest",
        manifestPath,
      ],
      { cwd: repoRoot, encoding: "utf8" },
    );

    expect(result.status).toBe(1);
    const report = JSON.parse(result.stdout) as { failures: string[] };
    expect(report.failures.some((failure) =>
      failure.includes("generated media row relativePath must match completed sidecar output"),
    )).toBe(true);
  });

  it("rejects retained provider E2E reports whose media row metadata does not match the sidecar output", () => {
    const tempDir = mkdtempSync(join(tmpdir(), "video-creater-provider-policy-media-metadata-"));
    const falReport = writeProviderPolicyFixture({
      rootDir: tempDir,
      provider: "fal.ai",
      model: "fal-ai/flux/schnell",
      requestId: "fal-request-1",
    });
    const replicateReport = writeProviderPolicyFixture({
      rootDir: tempDir,
      provider: "replicate",
      model: "black-forest-labs/flux-schnell",
      requestId: "replicate-request-1",
    });
    const replicate = JSON.parse(readFileSync(replicateReport, "utf8")) as {
      result: { projectDir: string };
    };
    const mediaIndexPath = join(replicate.result.projectDir, "media", "index.json");
    const mediaIndex = JSON.parse(readFileSync(mediaIndexPath, "utf8")) as {
      assets: Array<{
        id: string;
        durationSeconds: number;
        width: number;
        height: number;
        fps: number;
      }>;
    };
    Object.assign(requiredAt(mediaIndex.assets, 0, "media index asset"), {
      durationSeconds: 99,
      width: 1,
      height: 1,
      fps: 24,
    });
    writeFileSync(mediaIndexPath, JSON.stringify(mediaIndex, null, 2));
    const manifestPath = join(tempDir, "provider-e2e-evidence.json");
    writeFileSync(
      manifestPath,
      JSON.stringify(
        {
          projectRoot: tempDir,
          reports: [
            relativeTo(tempDir, falReport),
            relativeTo(tempDir, replicateReport),
          ],
        },
        null,
        2,
      ),
    );

    const result = spawnSync(
      process.execPath,
      [
        join(repoRoot, "scripts/provider-e2e-report-policy.mjs"),
        "--manifest",
        manifestPath,
      ],
      { cwd: repoRoot, encoding: "utf8" },
    );

    expect(result.status).toBe(1);
    const report = JSON.parse(result.stdout) as { failures: string[] };
    expect(report.failures).toEqual(
      expect.arrayContaining([
        `${replicateReport}: generated media row durationSeconds must match completed sidecar output: replicate-output`,
        `${replicateReport}: generated media row width must match completed sidecar output: replicate-output`,
        `${replicateReport}: generated media row height must match completed sidecar output: replicate-output`,
        `${replicateReport}: generated media row fps must match completed sidecar output: replicate-output`,
      ]),
    );
  });

  it("rejects retained provider E2E reports with duplicate completed sidecar output media ids", () => {
    const tempDir = mkdtempSync(join(tmpdir(), "video-creater-provider-policy-duplicate-output-media-"));
    const falReport = writeProviderPolicyFixture({
      rootDir: tempDir,
      provider: "fal.ai",
      model: "fal-ai/flux/schnell",
      requestId: "fal-request-1",
    });
    const replicateReport = writeProviderPolicyFixture({
      rootDir: tempDir,
      provider: "replicate",
      model: "black-forest-labs/flux-schnell",
      requestId: "replicate-request-1",
    });
    const replicate = JSON.parse(readFileSync(replicateReport, "utf8")) as {
      result: { outputCount: number; projectDir: string };
    };
    const duplicateRelativePath = "generated/replicate-duplicate-output.png";
    const duplicateOutputPath = join(replicate.result.projectDir, duplicateRelativePath);
    writeFileSync(duplicateOutputPath, "duplicate generated pixels");
    const sidecarPath = join(
      replicate.result.projectDir,
      "generated",
      "replicate-generated",
      "asset.json",
    );
    const sidecar = JSON.parse(readFileSync(sidecarPath, "utf8")) as {
      outputs: Array<{
        mediaId: string;
        relativePath: string;
        sourceUrl: string;
        width: number;
        height: number;
        durationSeconds: number;
        fps: number;
      }>;
    };
    sidecar.outputs.push({
      ...requiredAt(sidecar.outputs, 0, "provider sidecar output"),
      relativePath: duplicateRelativePath,
      sourceUrl: "https://provider.example/replicate-request-1/duplicate-output.png",
    });
    writeFileSync(sidecarPath, JSON.stringify(sidecar, null, 2));
    const generatedIndexPath = join(replicate.result.projectDir, "generated", "index.json");
    const generatedIndex = JSON.parse(readFileSync(generatedIndexPath, "utf8")) as {
      assets: Array<{ outputCount: number }>;
    };
    requiredAt(generatedIndex.assets, 0, "generated index asset").outputCount = 2;
    writeFileSync(generatedIndexPath, JSON.stringify(generatedIndex, null, 2));
    replicate.result.outputCount = 2;
    writeFileSync(replicateReport, JSON.stringify(replicate, null, 2));
    const manifestPath = join(tempDir, "provider-e2e-evidence.json");
    writeFileSync(
      manifestPath,
      JSON.stringify(
        {
          projectRoot: tempDir,
          reports: [
            relativeTo(tempDir, falReport),
            relativeTo(tempDir, replicateReport),
          ],
        },
        null,
        2,
      ),
    );

    const result = spawnSync(
      process.execPath,
      [
        join(repoRoot, "scripts/provider-e2e-report-policy.mjs"),
        "--manifest",
        manifestPath,
      ],
      { cwd: repoRoot, encoding: "utf8" },
    );

    expect(result.status).toBe(1);
    const report = JSON.parse(result.stdout) as { failures: string[] };
    expect(report.failures).toContain(
      `${replicateReport}: duplicate completed sidecar output mediaId: replicate-output`,
    );
  });

  it("rejects retained provider E2E reports with duplicate completed sidecar output paths", () => {
    const tempDir = mkdtempSync(join(tmpdir(), "video-creater-provider-policy-duplicate-output-path-"));
    const falReport = writeProviderPolicyFixture({
      rootDir: tempDir,
      provider: "fal.ai",
      model: "fal-ai/flux/schnell",
      requestId: "fal-request-1",
    });
    const replicateReport = writeProviderPolicyFixture({
      rootDir: tempDir,
      provider: "replicate",
      model: "black-forest-labs/flux-schnell",
      requestId: "replicate-request-1",
    });
    const replicate = JSON.parse(readFileSync(replicateReport, "utf8")) as {
      result: { outputCount: number; projectDir: string };
    };
    const sidecarPath = join(
      replicate.result.projectDir,
      "generated",
      "replicate-generated",
      "asset.json",
    );
    const sidecar = JSON.parse(readFileSync(sidecarPath, "utf8")) as {
      outputs: Array<{
        mediaId: string;
        relativePath: string;
        sourceUrl: string;
        width: number;
        height: number;
        durationSeconds: number;
        fps: number;
      }>;
    };
    sidecar.outputs.push({
      ...requiredAt(sidecar.outputs, 0, "provider sidecar output"),
      mediaId: "replicate-output-duplicate-path",
      sourceUrl: "https://provider.example/replicate-request-1/duplicate-path-output.png",
    });
    writeFileSync(sidecarPath, JSON.stringify(sidecar, null, 2));
    const mediaIndexPath = join(replicate.result.projectDir, "media", "index.json");
    const mediaIndex = JSON.parse(readFileSync(mediaIndexPath, "utf8")) as {
      assets: Array<{
        id: string;
        relativePath: string;
        kind: string;
        durationSeconds: number;
        width: number;
        height: number;
        fps: number;
      }>;
    };
    mediaIndex.assets.push({
      ...requiredAt(mediaIndex.assets, 0, "media index asset"),
      id: "replicate-output-duplicate-path",
    });
    writeFileSync(mediaIndexPath, JSON.stringify(mediaIndex, null, 2));
    const generatedIndexPath = join(replicate.result.projectDir, "generated", "index.json");
    const generatedIndex = JSON.parse(readFileSync(generatedIndexPath, "utf8")) as {
      assets: Array<{ outputCount: number }>;
    };
    requiredAt(generatedIndex.assets, 0, "generated index asset").outputCount = 2;
    writeFileSync(generatedIndexPath, JSON.stringify(generatedIndex, null, 2));
    replicate.result.outputCount = 2;
    writeFileSync(replicateReport, JSON.stringify(replicate, null, 2));
    const manifestPath = join(tempDir, "provider-e2e-evidence.json");
    writeFileSync(
      manifestPath,
      JSON.stringify(
        {
          projectRoot: tempDir,
          reports: [
            relativeTo(tempDir, falReport),
            relativeTo(tempDir, replicateReport),
          ],
        },
        null,
        2,
      ),
    );

    const result = spawnSync(
      process.execPath,
      [
        join(repoRoot, "scripts/provider-e2e-report-policy.mjs"),
        "--manifest",
        manifestPath,
      ],
      { cwd: repoRoot, encoding: "utf8" },
    );

    expect(result.status).toBe(1);
    const report = JSON.parse(result.stdout) as { failures: string[] };
    expect(report.failures).toContain(
      `${replicateReport}: duplicate completed sidecar output relativePath: generated/replicate-output.png`,
    );
  });

  it("rejects retained provider E2E reports with duplicate completed sidecar output source URLs", () => {
    const tempDir = mkdtempSync(join(tmpdir(), "video-creater-provider-policy-duplicate-output-source-"));
    const falReport = writeProviderPolicyFixture({
      rootDir: tempDir,
      provider: "fal.ai",
      model: "fal-ai/flux/schnell",
      requestId: "fal-request-1",
    });
    const replicateReport = writeProviderPolicyFixture({
      rootDir: tempDir,
      provider: "replicate",
      model: "black-forest-labs/flux-schnell",
      requestId: "replicate-request-1",
    });
    const replicate = JSON.parse(readFileSync(replicateReport, "utf8")) as {
      result: { outputCount: number; projectDir: string };
    };
    const duplicateRelativePath = "generated/replicate-duplicate-source-output.png";
    writeFileSync(
      join(replicate.result.projectDir, duplicateRelativePath),
      "duplicate provider source pixels",
    );
    const sidecarPath = join(
      replicate.result.projectDir,
      "generated",
      "replicate-generated",
      "asset.json",
    );
    const sidecar = JSON.parse(readFileSync(sidecarPath, "utf8")) as {
      outputs: Array<{
        mediaId: string;
        relativePath: string;
        sourceUrl: string;
        width: number;
        height: number;
        durationSeconds: number;
        fps: number;
      }>;
    };
    sidecar.outputs.push({
      ...requiredAt(sidecar.outputs, 0, "provider sidecar output"),
      mediaId: "replicate-output-duplicate-source",
      relativePath: duplicateRelativePath,
    });
    writeFileSync(sidecarPath, JSON.stringify(sidecar, null, 2));
    const mediaIndexPath = join(replicate.result.projectDir, "media", "index.json");
    const mediaIndex = JSON.parse(readFileSync(mediaIndexPath, "utf8")) as {
      assets: Array<{ id: string; relativePath: string }>;
    };
    mediaIndex.assets.push({
      ...requiredAt(mediaIndex.assets, 0, "media index asset"),
      id: "replicate-output-duplicate-source",
      relativePath: duplicateRelativePath,
    });
    writeFileSync(mediaIndexPath, JSON.stringify(mediaIndex, null, 2));
    const generatedIndexPath = join(replicate.result.projectDir, "generated", "index.json");
    const generatedIndex = JSON.parse(readFileSync(generatedIndexPath, "utf8")) as {
      assets: Array<{ outputCount: number }>;
    };
    requiredAt(generatedIndex.assets, 0, "generated index asset").outputCount = 2;
    writeFileSync(generatedIndexPath, JSON.stringify(generatedIndex, null, 2));
    replicate.result.outputCount = 2;
    writeFileSync(replicateReport, JSON.stringify(replicate, null, 2));
    const manifestPath = join(tempDir, "provider-e2e-evidence.json");
    writeFileSync(
      manifestPath,
      JSON.stringify(
        {
          projectRoot: tempDir,
          reports: [
            relativeTo(tempDir, falReport),
            relativeTo(tempDir, replicateReport),
          ],
        },
        null,
        2,
      ),
    );

    const result = spawnSync(
      process.execPath,
      [
        join(repoRoot, "scripts/provider-e2e-report-policy.mjs"),
        "--manifest",
        manifestPath,
      ],
      { cwd: repoRoot, encoding: "utf8" },
    );

    expect(result.status).toBe(1);
    const report = JSON.parse(result.stdout) as { failures: string[] };
    expect(report.failures).toContain(
      `${replicateReport}: duplicate completed sidecar output sourceUrl: https://provider.example/replicate-request-1/generated-output.png`,
    );
  });

  it("rejects retained provider E2E reports with blank completed sidecar output media ids", () => {
    const tempDir = mkdtempSync(join(tmpdir(), "video-creater-provider-policy-blank-output-media-"));
    const falReport = writeProviderPolicyFixture({
      rootDir: tempDir,
      provider: "fal.ai",
      model: "fal-ai/flux/schnell",
      requestId: "fal-request-1",
    });
    const replicateReport = writeProviderPolicyFixture({
      rootDir: tempDir,
      provider: "replicate",
      model: "black-forest-labs/flux-schnell",
      requestId: "replicate-request-1",
    });
    const replicate = JSON.parse(readFileSync(replicateReport, "utf8")) as {
      result: { projectDir: string };
    };
    const sidecarPath = join(
      replicate.result.projectDir,
      "generated",
      "replicate-generated",
      "asset.json",
    );
    const sidecar = JSON.parse(readFileSync(sidecarPath, "utf8")) as {
      outputs: Array<{ mediaId: string }>;
    };
    requiredAt(sidecar.outputs, 0, "provider sidecar output").mediaId = "";
    writeFileSync(sidecarPath, JSON.stringify(sidecar, null, 2));
    const manifestPath = join(tempDir, "provider-e2e-evidence.json");
    writeFileSync(
      manifestPath,
      JSON.stringify(
        {
          projectRoot: tempDir,
          reports: [
            relativeTo(tempDir, falReport),
            relativeTo(tempDir, replicateReport),
          ],
        },
        null,
        2,
      ),
    );

    const result = spawnSync(
      process.execPath,
      [
        join(repoRoot, "scripts/provider-e2e-report-policy.mjs"),
        "--manifest",
        manifestPath,
      ],
      { cwd: repoRoot, encoding: "utf8" },
    );

    expect(result.status).toBe(1);
    const report = JSON.parse(result.stdout) as { failures: string[] };
    expect(report.failures).toContain(
      `${replicateReport}: completed sidecar output mediaId is required`,
    );
  });

  it("rejects retained provider E2E reports with blank completed sidecar output paths", () => {
    const tempDir = mkdtempSync(join(tmpdir(), "video-creater-provider-policy-blank-output-path-"));
    const falReport = writeProviderPolicyFixture({
      rootDir: tempDir,
      provider: "fal.ai",
      model: "fal-ai/flux/schnell",
      requestId: "fal-request-1",
    });
    const replicateReport = writeProviderPolicyFixture({
      rootDir: tempDir,
      provider: "replicate",
      model: "black-forest-labs/flux-schnell",
      requestId: "replicate-request-1",
    });
    const replicate = JSON.parse(readFileSync(replicateReport, "utf8")) as {
      result: { projectDir: string };
    };
    const sidecarPath = join(
      replicate.result.projectDir,
      "generated",
      "replicate-generated",
      "asset.json",
    );
    const sidecar = JSON.parse(readFileSync(sidecarPath, "utf8")) as {
      outputs: Array<{ relativePath: string }>;
    };
    requiredAt(sidecar.outputs, 0, "provider sidecar output").relativePath = "";
    writeFileSync(sidecarPath, JSON.stringify(sidecar, null, 2));
    const manifestPath = join(tempDir, "provider-e2e-evidence.json");
    writeFileSync(
      manifestPath,
      JSON.stringify(
        {
          projectRoot: tempDir,
          reports: [
            relativeTo(tempDir, falReport),
            relativeTo(tempDir, replicateReport),
          ],
        },
        null,
        2,
      ),
    );

    const result = spawnSync(
      process.execPath,
      [
        join(repoRoot, "scripts/provider-e2e-report-policy.mjs"),
        "--manifest",
        manifestPath,
      ],
      { cwd: repoRoot, encoding: "utf8" },
    );

    expect(result.status).toBe(1);
    const report = JSON.parse(result.stdout) as { failures: string[] };
    expect(report.failures).toContain(
      `${replicateReport}: completed sidecar output relativePath is required`,
    );
  });

  it("rejects retained provider E2E reports with malformed media indexes", () => {
    const tempDir = mkdtempSync(join(tmpdir(), "video-creater-provider-policy-media-json-"));
    const falReport = writeProviderPolicyFixture({
      rootDir: tempDir,
      provider: "fal.ai",
      model: "fal-ai/flux/schnell",
      requestId: "fal-request-1",
    });
    const replicateReport = writeProviderPolicyFixture({
      rootDir: tempDir,
      provider: "replicate",
      model: "black-forest-labs/flux-schnell",
      requestId: "replicate-request-1",
    });
    const replicate = JSON.parse(readFileSync(replicateReport, "utf8")) as {
      result: { projectDir: string };
    };
    writeFileSync(join(replicate.result.projectDir, "media", "index.json"), "{not-json");
    const manifestPath = join(tempDir, "provider-e2e-evidence.json");
    writeFileSync(
      manifestPath,
      JSON.stringify(
        {
          projectRoot: tempDir,
          reports: [
            relativeTo(tempDir, falReport),
            relativeTo(tempDir, replicateReport),
          ],
        },
        null,
        2,
      ),
    );

    const result = spawnSync(
      process.execPath,
      [
        join(repoRoot, "scripts/provider-e2e-report-policy.mjs"),
        "--manifest",
        manifestPath,
      ],
      { cwd: repoRoot, encoding: "utf8" },
    );

    expect(result.status).toBe(1);
    const report = JSON.parse(result.stdout) as { failures: string[] };
    expect(report.failures.some((failure) =>
      failure.includes("media index is not parseable"),
    )).toBe(true);
  });

  it("rejects retained provider E2E reports with malformed generated indexes", () => {
    const tempDir = mkdtempSync(join(tmpdir(), "video-creater-provider-policy-generated-json-"));
    const falReport = writeProviderPolicyFixture({
      rootDir: tempDir,
      provider: "fal.ai",
      model: "fal-ai/flux/schnell",
      requestId: "fal-request-1",
    });
    const replicateReport = writeProviderPolicyFixture({
      rootDir: tempDir,
      provider: "replicate",
      model: "black-forest-labs/flux-schnell",
      requestId: "replicate-request-1",
    });
    const replicate = JSON.parse(readFileSync(replicateReport, "utf8")) as {
      result: { projectDir: string };
    };
    writeFileSync(join(replicate.result.projectDir, "generated", "index.json"), "{not-json");
    const manifestPath = join(tempDir, "provider-e2e-evidence.json");
    writeFileSync(
      manifestPath,
      JSON.stringify(
        {
          projectRoot: tempDir,
          reports: [
            relativeTo(tempDir, falReport),
            relativeTo(tempDir, replicateReport),
          ],
        },
        null,
        2,
      ),
    );

    const result = spawnSync(
      process.execPath,
      [
        join(repoRoot, "scripts/provider-e2e-report-policy.mjs"),
        "--manifest",
        manifestPath,
      ],
      { cwd: repoRoot, encoding: "utf8" },
    );

    expect(result.status).toBe(1);
    const report = JSON.parse(result.stdout) as { failures: string[] };
    expect(report.failures.some((failure) =>
      failure.includes("generated index is not parseable"),
    )).toBe(true);
  });

  it("rejects retained provider E2E reports with malformed split-project manifests", () => {
    const tempDir = mkdtempSync(join(tmpdir(), "video-creater-provider-policy-project-json-"));
    const falReport = writeProviderPolicyFixture({
      rootDir: tempDir,
      provider: "fal.ai",
      model: "fal-ai/flux/schnell",
      requestId: "fal-request-1",
    });
    const replicateReport = writeProviderPolicyFixture({
      rootDir: tempDir,
      provider: "replicate",
      model: "black-forest-labs/flux-schnell",
      requestId: "replicate-request-1",
    });
    const replicate = JSON.parse(readFileSync(replicateReport, "utf8")) as {
      result: { projectDir: string };
    };
    writeFileSync(join(replicate.result.projectDir, "video-creater.project.json"), "{not-json");
    const manifestPath = join(tempDir, "provider-e2e-evidence.json");
    writeFileSync(
      manifestPath,
      JSON.stringify(
        {
          projectRoot: tempDir,
          reports: [
            relativeTo(tempDir, falReport),
            relativeTo(tempDir, replicateReport),
          ],
        },
        null,
        2,
      ),
    );

    const result = spawnSync(
      process.execPath,
      [
        join(repoRoot, "scripts/provider-e2e-report-policy.mjs"),
        "--manifest",
        manifestPath,
      ],
      { cwd: repoRoot, encoding: "utf8" },
    );

    expect(result.status).toBe(1);
    const report = JSON.parse(result.stdout) as { failures: string[] };
    expect(report.failures.some((failure) =>
      failure.includes("split project manifest is not parseable"),
    )).toBe(true);
  });

  it("rejects retained provider E2E reports with malformed generated sidecars", () => {
    const tempDir = mkdtempSync(join(tmpdir(), "video-creater-provider-policy-sidecar-json-"));
    const falReport = writeProviderPolicyFixture({
      rootDir: tempDir,
      provider: "fal.ai",
      model: "fal-ai/flux/schnell",
      requestId: "fal-request-1",
    });
    const replicateReport = writeProviderPolicyFixture({
      rootDir: tempDir,
      provider: "replicate",
      model: "black-forest-labs/flux-schnell",
      requestId: "replicate-request-1",
    });
    const replicate = JSON.parse(readFileSync(replicateReport, "utf8")) as {
      result: { projectDir: string };
    };
    writeFileSync(
      join(replicate.result.projectDir, "generated", "replicate-generated", "asset.json"),
      "{not-json",
    );
    const manifestPath = join(tempDir, "provider-e2e-evidence.json");
    writeFileSync(
      manifestPath,
      JSON.stringify(
        {
          projectRoot: tempDir,
          reports: [
            relativeTo(tempDir, falReport),
            relativeTo(tempDir, replicateReport),
          ],
        },
        null,
        2,
      ),
    );

    const result = spawnSync(
      process.execPath,
      [
        join(repoRoot, "scripts/provider-e2e-report-policy.mjs"),
        "--manifest",
        manifestPath,
      ],
      { cwd: repoRoot, encoding: "utf8" },
    );

    expect(result.status).toBe(1);
    const report = JSON.parse(result.stdout) as { failures: string[] };
    expect(report.failures.some((failure) =>
      failure.includes("generated sidecar is not parseable"),
    )).toBe(true);
  });

  it("rejects retained provider E2E reports without matching split-project job request sidecars", () => {
    const tempDir = mkdtempSync(join(tmpdir(), "video-creater-provider-policy-job-request-"));
    const falReport = writeProviderPolicyFixture({
      rootDir: tempDir,
      provider: "fal.ai",
      model: "fal-ai/flux/schnell",
      requestId: "fal-request-1",
    });
    const replicateReport = writeProviderPolicyFixture({
      rootDir: tempDir,
      provider: "replicate",
      model: "black-forest-labs/flux-schnell",
      requestId: "replicate-request-1",
    });
    const replicate = JSON.parse(readFileSync(replicateReport, "utf8")) as {
      result: { projectDir: string };
    };
    rmSync(join(replicate.result.projectDir, "jobs", "replicate-generated"), {
      recursive: true,
      force: true,
    });
    const manifestPath = join(tempDir, "provider-e2e-evidence.json");
    writeFileSync(
      manifestPath,
      JSON.stringify(
        {
          projectRoot: tempDir,
          reports: [
            relativeTo(tempDir, falReport),
            relativeTo(tempDir, replicateReport),
          ],
        },
        null,
        2,
      ),
    );

    const result = spawnSync(
      process.execPath,
      [
        join(repoRoot, "scripts/provider-e2e-report-policy.mjs"),
        "--manifest",
        manifestPath,
      ],
      { cwd: repoRoot, encoding: "utf8" },
    );

    expect(result.status).toBe(1);
    const report = JSON.parse(result.stdout) as { failures: string[] };
    expect(report.failures.some((failure) =>
      failure.includes("split project job provider request does not match retained report"),
    )).toBe(true);
  });

  it("rejects retained provider E2E reports with malformed split-project job sidecars", () => {
    const tempDir = mkdtempSync(join(tmpdir(), "video-creater-provider-policy-job-json-"));
    const falReport = writeProviderPolicyFixture({
      rootDir: tempDir,
      provider: "fal.ai",
      model: "fal-ai/flux/schnell",
      requestId: "fal-request-1",
    });
    const replicateReport = writeProviderPolicyFixture({
      rootDir: tempDir,
      provider: "replicate",
      model: "black-forest-labs/flux-schnell",
      requestId: "replicate-request-1",
    });
    const replicate = JSON.parse(readFileSync(replicateReport, "utf8")) as {
      result: { projectDir: string };
    };
    writeFileSync(
      join(replicate.result.projectDir, "jobs", "replicate-generated", "job.json"),
      "{not-json",
    );
    const manifestPath = join(tempDir, "provider-e2e-evidence.json");
    writeFileSync(
      manifestPath,
      JSON.stringify(
        {
          projectRoot: tempDir,
          reports: [
            relativeTo(tempDir, falReport),
            relativeTo(tempDir, replicateReport),
          ],
        },
        null,
        2,
      ),
    );

    const result = spawnSync(
      process.execPath,
      [
        join(repoRoot, "scripts/provider-e2e-report-policy.mjs"),
        "--manifest",
        manifestPath,
      ],
      { cwd: repoRoot, encoding: "utf8" },
    );

    expect(result.status).toBe(1);
    const report = JSON.parse(result.stdout) as { failures: string[] };
    expect(report.failures.some((failure) =>
      failure.includes("split project job sidecar is not parseable"),
    )).toBe(true);
  });

  it("rejects retained provider E2E reports whose generated index asset id does not match the sidecar", () => {
    const tempDir = mkdtempSync(join(tmpdir(), "video-creater-provider-policy-index-id-"));
    const falReport = writeProviderPolicyFixture({
      rootDir: tempDir,
      provider: "fal.ai",
      model: "fal-ai/flux/schnell",
      requestId: "fal-request-1",
    });
    const replicateReport = writeProviderPolicyFixture({
      rootDir: tempDir,
      provider: "replicate",
      model: "black-forest-labs/flux-schnell",
      requestId: "replicate-request-1",
    });
    const replicate = JSON.parse(readFileSync(replicateReport, "utf8")) as {
      result: { projectDir: string };
    };
    const generatedIndexPath = join(replicate.result.projectDir, "generated", "index.json");
    const generatedIndex = JSON.parse(readFileSync(generatedIndexPath, "utf8")) as {
      assets: Array<{ assetId: string }>;
    };
    requiredAt(generatedIndex.assets, 0, "generated index asset").assetId = "different-generated-asset";
    writeFileSync(generatedIndexPath, JSON.stringify(generatedIndex, null, 2));
    const manifestPath = join(tempDir, "provider-e2e-evidence.json");
    writeFileSync(
      manifestPath,
      JSON.stringify(
        {
          projectRoot: tempDir,
          reports: [
            relativeTo(tempDir, falReport),
            relativeTo(tempDir, replicateReport),
          ],
        },
        null,
        2,
      ),
    );

    const result = spawnSync(
      process.execPath,
      [
        join(repoRoot, "scripts/provider-e2e-report-policy.mjs"),
        "--manifest",
        manifestPath,
      ],
      { cwd: repoRoot, encoding: "utf8" },
    );

    expect(result.status).toBe(1);
    const report = JSON.parse(result.stdout) as { failures: string[] };
    expect(report.failures.some((failure) =>
      failure.includes("generated index assetId must match sidecar id"),
    )).toBe(true);
  });

  it("rejects retained provider E2E reports with duplicate generated index asset ids", () => {
    const tempDir = mkdtempSync(join(tmpdir(), "video-creater-provider-policy-duplicate-index-id-"));
    const falReport = writeProviderPolicyFixture({
      rootDir: tempDir,
      provider: "fal.ai",
      model: "fal-ai/flux/schnell",
      requestId: "fal-request-1",
    });
    const replicateReport = writeProviderPolicyFixture({
      rootDir: tempDir,
      provider: "replicate",
      model: "black-forest-labs/flux-schnell",
      requestId: "replicate-request-1",
    });
    const replicate = JSON.parse(readFileSync(replicateReport, "utf8")) as {
      result: { projectDir: string };
    };
    const generatedIndexPath = join(replicate.result.projectDir, "generated", "index.json");
    const generatedIndex = JSON.parse(readFileSync(generatedIndexPath, "utf8")) as {
      assets: Array<{ assetId: string }>;
    };
    generatedIndex.assets.push({ ...requiredAt(generatedIndex.assets, 0, "generated index asset") });
    writeFileSync(generatedIndexPath, JSON.stringify(generatedIndex, null, 2));
    const manifestPath = join(tempDir, "provider-e2e-evidence.json");
    writeFileSync(
      manifestPath,
      JSON.stringify(
        {
          projectRoot: tempDir,
          reports: [
            relativeTo(tempDir, falReport),
            relativeTo(tempDir, replicateReport),
          ],
        },
        null,
        2,
      ),
    );

    const result = spawnSync(
      process.execPath,
      [
        join(repoRoot, "scripts/provider-e2e-report-policy.mjs"),
        "--manifest",
        manifestPath,
      ],
      { cwd: repoRoot, encoding: "utf8" },
    );

    expect(result.status).toBe(1);
    const report = JSON.parse(result.stdout) as { failures: string[] };
    expect(report.failures).toContain(
      `${replicateReport}: duplicate generated index assetId: replicate-generated`,
    );
  });

  it("rejects retained provider E2E reports with duplicate sidecar output media ids across generated assets", () => {
    const tempDir = mkdtempSync(join(tmpdir(), "video-creater-provider-policy-duplicate-cross-sidecar-output-"));
    const falReport = writeProviderPolicyFixture({
      rootDir: tempDir,
      provider: "fal.ai",
      model: "fal-ai/flux/schnell",
      requestId: "fal-request-1",
    });
    const replicateReport = writeProviderPolicyFixture({
      rootDir: tempDir,
      provider: "replicate",
      model: "black-forest-labs/flux-schnell",
      requestId: "replicate-request-1",
    });
    const replicate = JSON.parse(readFileSync(replicateReport, "utf8")) as {
      result: { outputCount: number; projectDir: string };
    };
    const duplicateAssetId = "replicate-generated-duplicate";
    const generatedIndexPath = join(replicate.result.projectDir, "generated", "index.json");
    const generatedIndex = JSON.parse(readFileSync(generatedIndexPath, "utf8")) as {
      assets: Array<{ assetId: string; path: string; outputCount: number }>;
    };
    generatedIndex.assets.push({
      ...requiredAt(generatedIndex.assets, 0, "generated index asset"),
      assetId: duplicateAssetId,
      path: `generated/${duplicateAssetId}/asset.json`,
    });
    writeFileSync(generatedIndexPath, JSON.stringify(generatedIndex, null, 2));

    const originalSidecarPath = join(
      replicate.result.projectDir,
      "generated",
      "replicate-generated",
      "asset.json",
    );
    const duplicateSidecarDir = join(replicate.result.projectDir, "generated", duplicateAssetId);
    mkdirSync(duplicateSidecarDir, { recursive: true });
    const sidecar = JSON.parse(readFileSync(originalSidecarPath, "utf8")) as { id: string };
    sidecar.id = duplicateAssetId;
    writeFileSync(
      join(duplicateSidecarDir, "asset.json"),
      JSON.stringify(sidecar, null, 2),
    );
    replicate.result.outputCount = 2;
    writeFileSync(replicateReport, JSON.stringify(replicate, null, 2));
    const manifestPath = join(tempDir, "provider-e2e-evidence.json");
    writeFileSync(
      manifestPath,
      JSON.stringify(
        {
          projectRoot: tempDir,
          reports: [
            relativeTo(tempDir, falReport),
            relativeTo(tempDir, replicateReport),
          ],
        },
        null,
        2,
      ),
    );

    const result = spawnSync(
      process.execPath,
      [
        join(repoRoot, "scripts/provider-e2e-report-policy.mjs"),
        "--manifest",
        manifestPath,
      ],
      { cwd: repoRoot, encoding: "utf8" },
    );

    expect(result.status).toBe(1);
    const report = JSON.parse(result.stdout) as { failures: string[] };
    expect(report.failures).toContain(
      `${replicateReport}: duplicate completed sidecar output mediaId across generated assets: replicate-output`,
    );
  });

  it("rejects retained provider E2E reports with duplicate sidecar output paths across generated assets", () => {
    const tempDir = mkdtempSync(join(tmpdir(), "video-creater-provider-policy-duplicate-cross-sidecar-path-"));
    const falReport = writeProviderPolicyFixture({
      rootDir: tempDir,
      provider: "fal.ai",
      model: "fal-ai/flux/schnell",
      requestId: "fal-request-1",
    });
    const replicateReport = writeProviderPolicyFixture({
      rootDir: tempDir,
      provider: "replicate",
      model: "black-forest-labs/flux-schnell",
      requestId: "replicate-request-1",
    });
    const replicate = JSON.parse(readFileSync(replicateReport, "utf8")) as {
      result: { outputCount: number; projectDir: string };
    };
    const duplicateAssetId = "replicate-generated-duplicate-path";
    const duplicateMediaId = "replicate-output-duplicate-path";
    const generatedIndexPath = join(replicate.result.projectDir, "generated", "index.json");
    const generatedIndex = JSON.parse(readFileSync(generatedIndexPath, "utf8")) as {
      assets: Array<{ assetId: string; path: string; outputCount: number }>;
    };
    generatedIndex.assets.push({
      ...requiredAt(generatedIndex.assets, 0, "generated index asset"),
      assetId: duplicateAssetId,
      path: `generated/${duplicateAssetId}/asset.json`,
    });
    writeFileSync(generatedIndexPath, JSON.stringify(generatedIndex, null, 2));

    const originalSidecarPath = join(
      replicate.result.projectDir,
      "generated",
      "replicate-generated",
      "asset.json",
    );
    const duplicateSidecarDir = join(replicate.result.projectDir, "generated", duplicateAssetId);
    mkdirSync(duplicateSidecarDir, { recursive: true });
    const sidecar = JSON.parse(readFileSync(originalSidecarPath, "utf8")) as {
      id: string;
      outputs: Array<{ mediaId: string }>;
    };
    sidecar.id = duplicateAssetId;
    requiredAt(sidecar.outputs, 0, "provider sidecar output").mediaId = duplicateMediaId;
    writeFileSync(
      join(duplicateSidecarDir, "asset.json"),
      JSON.stringify(sidecar, null, 2),
    );

    const mediaIndexPath = join(replicate.result.projectDir, "media", "index.json");
    const mediaIndex = JSON.parse(readFileSync(mediaIndexPath, "utf8")) as {
      assets: Array<{ id: string }>;
    };
    mediaIndex.assets.push({ ...requiredAt(mediaIndex.assets, 0, "media index asset"), id: duplicateMediaId });
    writeFileSync(mediaIndexPath, JSON.stringify(mediaIndex, null, 2));
    replicate.result.outputCount = 2;
    writeFileSync(replicateReport, JSON.stringify(replicate, null, 2));
    const manifestPath = join(tempDir, "provider-e2e-evidence.json");
    writeFileSync(
      manifestPath,
      JSON.stringify(
        {
          projectRoot: tempDir,
          reports: [
            relativeTo(tempDir, falReport),
            relativeTo(tempDir, replicateReport),
          ],
        },
        null,
        2,
      ),
    );

    const result = spawnSync(
      process.execPath,
      [
        join(repoRoot, "scripts/provider-e2e-report-policy.mjs"),
        "--manifest",
        manifestPath,
      ],
      { cwd: repoRoot, encoding: "utf8" },
    );

    expect(result.status).toBe(1);
    const report = JSON.parse(result.stdout) as { failures: string[] };
    expect(report.failures).toContain(
      `${replicateReport}: duplicate completed sidecar output relativePath across generated assets: generated/replicate-output.png`,
    );
  });

  it("rejects retained provider E2E reports whose generated index model metadata does not match the report", () => {
    const tempDir = mkdtempSync(join(tmpdir(), "video-creater-provider-policy-index-model-"));
    const falReport = writeProviderPolicyFixture({
      rootDir: tempDir,
      provider: "fal.ai",
      model: "fal-ai/flux/schnell",
      requestId: "fal-request-1",
    });
    const replicateReport = writeProviderPolicyFixture({
      rootDir: tempDir,
      provider: "replicate",
      model: "black-forest-labs/flux-schnell",
      requestId: "replicate-request-1",
    });
    const replicate = JSON.parse(readFileSync(replicateReport, "utf8")) as {
      result: { projectDir: string };
    };
    const generatedIndexPath = join(replicate.result.projectDir, "generated", "index.json");
    const generatedIndex = JSON.parse(readFileSync(generatedIndexPath, "utf8")) as {
      assets: Array<{ modelProvider: string; modelId: string }>;
    };
    requiredAt(generatedIndex.assets, 0, "generated index asset").modelProvider = "fal.ai";
    requiredAt(generatedIndex.assets, 0, "generated index asset").modelId = "fal-ai/flux/schnell";
    writeFileSync(generatedIndexPath, JSON.stringify(generatedIndex, null, 2));
    const manifestPath = join(tempDir, "provider-e2e-evidence.json");
    writeFileSync(
      manifestPath,
      JSON.stringify(
        {
          projectRoot: tempDir,
          reports: [
            relativeTo(tempDir, falReport),
            relativeTo(tempDir, replicateReport),
          ],
        },
        null,
        2,
      ),
    );

    const result = spawnSync(
      process.execPath,
      [
        join(repoRoot, "scripts/provider-e2e-report-policy.mjs"),
        "--manifest",
        manifestPath,
      ],
      { cwd: repoRoot, encoding: "utf8" },
    );

    expect(result.status).toBe(1);
    const report = JSON.parse(result.stdout) as { failures: string[] };
    expect(report.failures.some((failure) =>
      failure.includes("generated index model metadata must match retained report"),
    )).toBe(true);
  });

  it("rejects retained provider E2E reports whose generated index output count does not match the sidecar", () => {
    const tempDir = mkdtempSync(join(tmpdir(), "video-creater-provider-policy-index-output-count-"));
    const falReport = writeProviderPolicyFixture({
      rootDir: tempDir,
      provider: "fal.ai",
      model: "fal-ai/flux/schnell",
      requestId: "fal-request-1",
    });
    const replicateReport = writeProviderPolicyFixture({
      rootDir: tempDir,
      provider: "replicate",
      model: "black-forest-labs/flux-schnell",
      requestId: "replicate-request-1",
    });
    const replicate = JSON.parse(readFileSync(replicateReport, "utf8")) as {
      result: { projectDir: string };
    };
    const generatedIndexPath = join(replicate.result.projectDir, "generated", "index.json");
    const generatedIndex = JSON.parse(readFileSync(generatedIndexPath, "utf8")) as {
      assets: Array<{ outputCount: number }>;
    };
    requiredAt(generatedIndex.assets, 0, "generated index asset").outputCount = 2;
    writeFileSync(generatedIndexPath, JSON.stringify(generatedIndex, null, 2));
    const manifestPath = join(tempDir, "provider-e2e-evidence.json");
    writeFileSync(
      manifestPath,
      JSON.stringify(
        {
          projectRoot: tempDir,
          reports: [
            relativeTo(tempDir, falReport),
            relativeTo(tempDir, replicateReport),
          ],
        },
        null,
        2,
      ),
    );

    const result = spawnSync(
      process.execPath,
      [
        join(repoRoot, "scripts/provider-e2e-report-policy.mjs"),
        "--manifest",
        manifestPath,
      ],
      { cwd: repoRoot, encoding: "utf8" },
    );

    expect(result.status).toBe(1);
    const report = JSON.parse(result.stdout) as { failures: string[] };
    expect(report.failures.some((failure) =>
      failure.includes("generated index outputCount must match sidecar outputs"),
    )).toBe(true);
  });

  it("rejects retained provider video reports without persisted generateAudio settings", () => {
    const tempDir = mkdtempSync(join(tmpdir(), "video-creater-provider-policy-video-settings-"));
    const googleReport = writeProviderPolicyFixture({
      rootDir: tempDir,
      provider: "google",
      model: "veo3.1-fast",
      requestId: "google-video-request-1",
      scenario: "google-video",
      sourceUrl: "https://provider.example/google-video-request-1/generated-video.mp4",
    });
    const google = JSON.parse(readFileSync(googleReport, "utf8")) as {
      result: { projectDir: string };
    };
    const sidecarPath = join(
      google.result.projectDir,
      "generated",
      "google-generated",
      "asset.json",
    );
    const sidecar = JSON.parse(readFileSync(sidecarPath, "utf8")) as {
      settings?: { generateAudio?: boolean };
    };
    delete sidecar.settings?.generateAudio;
    writeFileSync(sidecarPath, JSON.stringify(sidecar, null, 2));
    const manifestPath = join(tempDir, "provider-e2e-evidence.json");
    writeFileSync(
      manifestPath,
      JSON.stringify(
        {
          projectRoot: tempDir,
          reports: [relativeTo(tempDir, googleReport)],
        },
        null,
        2,
      ),
    );

    const result = spawnSync(
      process.execPath,
      [
        join(repoRoot, "scripts/provider-e2e-report-policy.mjs"),
        "--manifest",
        manifestPath,
      ],
      { cwd: repoRoot, encoding: "utf8" },
    );

    expect(result.status).toBe(1);
    const report = JSON.parse(result.stdout) as { failures: string[] };
    expect(report.failures.some((failure) =>
      failure.includes("provider video sidecar settings.generateAudio must be retained"),
    )).toBe(true);
  });

  it("rejects retained WAN reference-to-video reports without model-specific settings", () => {
    const tempDir = mkdtempSync(join(tmpdir(), "video-creater-provider-policy-wan-reference-settings-"));
    const falWanReferenceReport = writeProviderPolicyFixture({
      rootDir: tempDir,
      provider: "fal.ai",
      model: "fal-ai/wan/v2.7/reference-to-video",
      requestId: "fal-wan-reference-request-1",
      scenario: "wan-reference-to-video",
      sourceUrl: "https://provider.example/fal-wan-reference-request-1/generated-video.mp4",
    });
    const falWanReference = JSON.parse(readFileSync(falWanReferenceReport, "utf8")) as {
      result: { projectDir: string };
    };
    const sidecarPath = join(
      falWanReference.result.projectDir,
      "generated",
      "fal-ai-generated",
      "asset.json",
    );
    const sidecar = JSON.parse(readFileSync(sidecarPath, "utf8")) as {
      settings?: { resolution?: string };
    };
    delete sidecar.settings?.resolution;
    writeFileSync(sidecarPath, JSON.stringify(sidecar, null, 2));
    const manifestPath = join(tempDir, "provider-e2e-evidence.json");
    writeFileSync(
      manifestPath,
      JSON.stringify(
        {
          projectRoot: tempDir,
          reports: [relativeTo(tempDir, falWanReferenceReport)],
        },
        null,
        2,
      ),
    );

    const result = spawnSync(
      process.execPath,
      [
        join(repoRoot, "scripts/provider-e2e-report-policy.mjs"),
        "--manifest",
        manifestPath,
      ],
      { cwd: repoRoot, encoding: "utf8" },
    );

    expect(result.status).toBe(1);
    const report = JSON.parse(result.stdout) as { failures: string[] };
    expect(report.failures.some((failure) =>
      failure.includes("WAN reference-to-video sidecar must retain duration, aspect, resolution, and generateAudio settings"),
    )).toBe(true);
  });

  it("rejects retained Google Veo reports without persisted reference inputs", () => {
    const tempDir = mkdtempSync(join(tmpdir(), "video-creater-provider-policy-google-veo-refs-"));
    const googleReport = writeProviderPolicyFixture({
      rootDir: tempDir,
      provider: "google",
      model: "veo3.1-fast",
      requestId: "google-video-request-1",
      scenario: "google-video",
      sourceUrl: "https://provider.example/google-video-request-1/generated-video.mp4",
    });
    const google = JSON.parse(readFileSync(googleReport, "utf8")) as {
      result: { projectDir: string };
    };
    const sidecarPath = join(
      google.result.projectDir,
      "generated",
      "google-generated",
      "asset.json",
    );
    const sidecar = JSON.parse(readFileSync(sidecarPath, "utf8")) as {
      references?: unknown;
    };
    delete sidecar.references;
    writeFileSync(sidecarPath, JSON.stringify(sidecar, null, 2));
    const manifestPath = join(tempDir, "provider-e2e-evidence.json");
    writeFileSync(
      manifestPath,
      JSON.stringify(
        {
          projectRoot: tempDir,
          reports: [relativeTo(tempDir, googleReport)],
        },
        null,
        2,
      ),
    );

    const result = spawnSync(
      process.execPath,
      [
        join(repoRoot, "scripts/provider-e2e-report-policy.mjs"),
        "--manifest",
        manifestPath,
      ],
      { cwd: repoRoot, encoding: "utf8" },
    );

    expect(result.status).toBe(1);
    const report = JSON.parse(result.stdout) as { failures: string[] };
    expect(report.failures.some((failure) =>
      failure.includes("Google Veo sidecar references are required"),
    )).toBe(true);
  });

  it.each([
    {
      provider: "openai",
      model: "gpt-4o-mini-tts",
      requestId: "openai-audio-request-1",
      generatedId: "openai-generated",
      expectedVoice: "alloy",
    },
    {
      provider: "elevenlabs",
      model: "elevenlabs-tts-v3",
      requestId: "elevenlabs-audio-request-1",
      generatedId: "elevenlabs-generated",
      expectedVoice: "rachel",
    },
    {
      provider: "google",
      model: "gemini-3.1-flash-tts-preview",
      requestId: "google-audio-request-1",
      generatedId: "google-generated",
      expectedVoice: "Kore",
    },
  ] as const)(
    "rejects retained $provider TTS reports without default voice settings",
    ({ provider, model, requestId, generatedId, expectedVoice }) => {
    const tempDir = mkdtempSync(join(tmpdir(), "video-creater-provider-policy-tts-settings-"));
    const audioReport = writeProviderPolicyFixture({
      rootDir: tempDir,
      provider,
      model,
      requestId,
      scenario: "text-to-audio",
      sourceUrl: null,
    });
    const audio = JSON.parse(readFileSync(audioReport, "utf8")) as {
      result: { projectDir: string };
    };
    const sidecarPath = join(
      audio.result.projectDir,
      "generated",
      generatedId,
      "asset.json",
    );
    const sidecar = JSON.parse(readFileSync(sidecarPath, "utf8")) as {
      settings?: { voice?: string };
    };
    expect(sidecar.settings?.voice).toBe(expectedVoice);
    delete sidecar.settings?.voice;
    writeFileSync(sidecarPath, JSON.stringify(sidecar, null, 2));
    const manifestPath = join(tempDir, "provider-e2e-evidence.json");
    writeFileSync(
      manifestPath,
      JSON.stringify(
        {
          projectRoot: tempDir,
          reports: [relativeTo(tempDir, audioReport)],
        },
        null,
        2,
      ),
    );

    const result = spawnSync(
      process.execPath,
      [
        join(repoRoot, "scripts/provider-e2e-report-policy.mjs"),
        "--manifest",
        manifestPath,
      ],
      { cwd: repoRoot, encoding: "utf8" },
    );

    expect(result.status).toBe(1);
    const report = JSON.parse(result.stdout) as { failures: string[] };
    expect(report.failures.some((failure) =>
      failure.includes("TTS sidecar settings.voice must retain the catalog default"),
    )).toBe(true);
    },
  );

  it.each([
    {
      provider: "elevenlabs",
      model: "elevenlabs-music",
      requestId: "elevenlabs-music-request-1",
      generatedId: "elevenlabs-generated",
      scenario: "text-to-music",
    },
    {
      provider: "google",
      model: "lyria3-pro",
      requestId: "google-lyria-request-1",
      generatedId: "google-generated",
      scenario: "text-to-music",
    },
    {
      provider: "fal.ai",
      model: "sonilo/v1.1/text-to-music",
      requestId: "fal-music-request-1",
      generatedId: "fal-ai-generated",
      scenario: "text-to-music",
    },
    {
      provider: "fal.ai",
      model: "sonilo/v1.1/video-to-music",
      requestId: "fal-video-music-request-1",
      generatedId: "fal-ai-generated",
      scenario: "video-to-music",
    },
    {
      provider: "fal.ai",
      model: "sonilo/v1.1/video-to-music",
      requestId: "fal-video-music-insert-request-1",
      generatedId: "fal-ai-generated",
      scenario: "local-video-to-music-insert",
    },
    {
      provider: "minimax",
      model: "minimax-music-v2.6",
      requestId: "minimax-music-request-1",
      generatedId: "minimax-generated",
      scenario: "text-to-music",
    },
  ] as const)(
    "rejects retained $provider $scenario reports without music settings",
    ({ provider, model, requestId, generatedId, scenario }) => {
      const tempDir = mkdtempSync(join(tmpdir(), "video-creater-provider-policy-music-settings-"));
      const musicReport = writeProviderPolicyFixture({
        rootDir: tempDir,
        provider,
        model,
        requestId,
        scenario,
        sourceUrl: null,
      });
      const music = JSON.parse(readFileSync(musicReport, "utf8")) as {
        result: { projectDir: string };
      };
      const sidecarPath = join(
        music.result.projectDir,
        "generated",
        generatedId,
        "asset.json",
      );
      const sidecar = JSON.parse(readFileSync(sidecarPath, "utf8")) as {
        settings?: { durationSeconds?: number; lyrics?: string };
      };
      expect(sidecar.settings).toEqual(
        expect.objectContaining({
          category: "music",
          durationSeconds: 30,
          instrumental: false,
          styleInstructions: "bright, commercial, loopable",
        }),
      );
      delete sidecar.settings?.durationSeconds;
      writeFileSync(sidecarPath, JSON.stringify(sidecar, null, 2));
      const manifestPath = join(tempDir, "provider-e2e-evidence.json");
      writeFileSync(
        manifestPath,
        JSON.stringify(
          {
            projectRoot: tempDir,
            reports: [relativeTo(tempDir, musicReport)],
          },
          null,
          2,
        ),
      );

      const result = spawnSync(
        process.execPath,
        [
          join(repoRoot, "scripts/provider-e2e-report-policy.mjs"),
          "--manifest",
          manifestPath,
        ],
        { cwd: repoRoot, encoding: "utf8" },
      );

      expect(result.status).toBe(1);
      const report = JSON.parse(result.stdout) as { failures: string[] };
      expect(report.failures.some((failure) =>
        failure.includes("music sidecar settings must retain music defaults"),
      )).toBe(true);
    },
  );

  it("rejects retained text-to-music timeline insert reports without timeline evidence", () => {
    const tempDir = mkdtempSync(join(tmpdir(), "video-creater-provider-policy-music-insert-"));
    const musicReport = writeProviderPolicyFixture({
      rootDir: tempDir,
      provider: "fal.ai",
      model: "sonilo/v1.1/text-to-music",
      requestId: "fal-music-insert-request-1",
      scenario: "text-to-music-insert",
      sourceUrl: null,
    });
    const music = JSON.parse(readFileSync(musicReport, "utf8")) as {
      result: { projectDir: string };
    };
    const timelinePath = join(music.result.projectDir, "timeline.json");
    if (existsSync(timelinePath)) {
      rmSync(timelinePath);
    }
    const manifestPath = join(tempDir, "provider-e2e-evidence.json");
    writeFileSync(
      manifestPath,
      JSON.stringify(
        {
          projectRoot: tempDir,
          reports: [relativeTo(tempDir, musicReport)],
        },
        null,
        2,
      ),
    );

    const result = spawnSync(
      process.execPath,
      [
        join(repoRoot, "scripts/provider-e2e-report-policy.mjs"),
        "--manifest",
        manifestPath,
      ],
      { cwd: repoRoot, encoding: "utf8" },
    );

    expect(result.status).toBe(1);
    const report = JSON.parse(result.stdout) as { failures: string[] };
    expect(report.failures.some((failure) =>
      failure.includes("text-to-music insert scenario timeline.json is missing"),
    )).toBe(true);
  });

  it("rejects retained MiniMax text-to-music reports without lyrics", () => {
    const tempDir = mkdtempSync(join(tmpdir(), "video-creater-provider-policy-minimax-lyrics-"));
    const musicReport = writeProviderPolicyFixture({
      rootDir: tempDir,
      provider: "minimax",
      model: "minimax-music-v2.6",
      requestId: "minimax-music-request-lyrics-1",
      scenario: "text-to-music",
      sourceUrl: null,
    });
    const music = JSON.parse(readFileSync(musicReport, "utf8")) as {
      result: { projectDir: string };
    };
    const sidecarPath = join(
      music.result.projectDir,
      "generated",
      "minimax-generated",
      "asset.json",
    );
    const sidecar = JSON.parse(readFileSync(sidecarPath, "utf8")) as {
      settings?: { lyrics?: string };
    };
    expect(sidecar.settings).toEqual(
      expect.objectContaining({
        lyrics: minimaxTextToMusicDefaultLyrics,
      }),
    );
    delete sidecar.settings?.lyrics;
    writeFileSync(sidecarPath, JSON.stringify(sidecar, null, 2));
    const manifestPath = join(tempDir, "provider-e2e-evidence.json");
    writeFileSync(
      manifestPath,
      JSON.stringify(
        {
          projectRoot: tempDir,
          reports: [relativeTo(tempDir, musicReport)],
        },
        null,
        2,
      ),
    );

    const result = spawnSync(
      process.execPath,
      [
        join(repoRoot, "scripts/provider-e2e-report-policy.mjs"),
        "--manifest",
        manifestPath,
      ],
      { cwd: repoRoot, encoding: "utf8" },
    );

    expect(result.status).toBe(1);
    const report = JSON.parse(result.stdout) as { failures: string[] };
    expect(report.failures.some((failure) =>
      failure.includes("MiniMax text-to-music sidecar settings.lyrics must be retained"),
    )).toBe(true);
  });

  it("rejects retained ElevenLabs text-to-music reports without lyrics", () => {
    const tempDir = mkdtempSync(join(tmpdir(), "video-creater-provider-policy-elevenlabs-lyrics-"));
    const musicReport = writeProviderPolicyFixture({
      rootDir: tempDir,
      provider: "elevenlabs",
      model: "elevenlabs-music",
      requestId: "elevenlabs-music-request-lyrics-1",
      scenario: "text-to-music",
      sourceUrl: null,
    });
    const music = JSON.parse(readFileSync(musicReport, "utf8")) as {
      result: { projectDir: string };
    };
    const sidecarPath = join(
      music.result.projectDir,
      "generated",
      "elevenlabs-generated",
      "asset.json",
    );
    const sidecar = JSON.parse(readFileSync(sidecarPath, "utf8")) as {
      settings?: { lyrics?: string };
    };
    expect(sidecar.settings).toEqual(
      expect.objectContaining({
        lyrics: elevenlabsTextToMusicDefaultLyrics,
      }),
    );
    delete sidecar.settings?.lyrics;
    writeFileSync(sidecarPath, JSON.stringify(sidecar, null, 2));
    const manifestPath = join(tempDir, "provider-e2e-evidence.json");
    writeFileSync(
      manifestPath,
      JSON.stringify(
        {
          projectRoot: tempDir,
          reports: [relativeTo(tempDir, musicReport)],
        },
        null,
        2,
      ),
    );

    const result = spawnSync(
      process.execPath,
      [
        join(repoRoot, "scripts/provider-e2e-report-policy.mjs"),
        "--manifest",
        manifestPath,
      ],
      { cwd: repoRoot, encoding: "utf8" },
    );

    expect(result.status).toBe(1);
    const report = JSON.parse(result.stdout) as { failures: string[] };
    expect(report.failures.some((failure) =>
      failure.includes("ElevenLabs text-to-music sidecar settings.lyrics must be retained"),
    )).toBe(true);
  });

  it("rejects retained Google Lyria text-to-music reports without lyrics", () => {
    const tempDir = mkdtempSync(join(tmpdir(), "video-creater-provider-policy-google-lyrics-"));
    const musicReport = writeProviderPolicyFixture({
      rootDir: tempDir,
      provider: "google",
      model: "lyria3-pro",
      requestId: "google-lyria-request-lyrics-1",
      scenario: "text-to-music",
      sourceUrl: null,
    });
    const music = JSON.parse(readFileSync(musicReport, "utf8")) as {
      result: { projectDir: string };
    };
    const sidecarPath = join(
      music.result.projectDir,
      "generated",
      "google-generated",
      "asset.json",
    );
    const sidecar = JSON.parse(readFileSync(sidecarPath, "utf8")) as {
      settings?: { lyrics?: string };
    };
    expect(sidecar.settings).toEqual(
      expect.objectContaining({
        lyrics: googleLyriaTextToMusicDefaultLyrics,
      }),
    );
    delete sidecar.settings?.lyrics;
    writeFileSync(sidecarPath, JSON.stringify(sidecar, null, 2));
    const manifestPath = join(tempDir, "provider-e2e-evidence.json");
    writeFileSync(
      manifestPath,
      JSON.stringify(
        {
          projectRoot: tempDir,
          reports: [relativeTo(tempDir, musicReport)],
        },
        null,
        2,
      ),
    );

    const result = spawnSync(
      process.execPath,
      [
        join(repoRoot, "scripts/provider-e2e-report-policy.mjs"),
        "--manifest",
        manifestPath,
      ],
      { cwd: repoRoot, encoding: "utf8" },
    );

    expect(result.status).toBe(1);
    const report = JSON.parse(result.stdout) as { failures: string[] };
    expect(report.failures.some((failure) =>
      failure.includes("Google Lyria text-to-music sidecar settings.lyrics must be retained"),
    )).toBe(true);
  });

  it("refuses to run live fal.ai verification without credentials", () => {
    const result = spawnSync(
      process.execPath,
      [
        join(repoRoot, "scripts/provider-e2e.mjs"),
        "--provider",
        "fal.ai",
        "--live",
      ],
      {
        cwd: repoRoot,
        encoding: "utf8",
        env: {
          ...process.env,
          FAL_KEY: "",
        },
      },
    );

    expect(result.status).toBe(1);
    expect(result.stderr).toContain("FAL_KEY is required");
    const report = JSON.parse(result.stdout) as {
      ok: boolean;
      provider: string;
      model: string;
      outDir: string;
      credentialEnvVar: string;
      credentialStatus: string;
      command: string | null;
    };
    expect(report).toEqual(
      expect.objectContaining({
        ok: false,
        provider: "fal.ai",
        model: "fal-ai/flux/schnell",
        outDir: "output/provider-e2e/fal.ai",
        credentialEnvVar: "FAL_KEY",
        credentialStatus: "missing",
        command: null,
      }),
    );
  });

  it("runs the provider E2E binary without leaking credential values", () => {
    const tempDir = mkdtempSync(join(tmpdir(), "video-creater-provider-e2e-"));
    const fakeCargo = join(tempDir, "cargo");
    const projectDir = join(tempDir, "project");
    const artifactPath = join(projectDir, "generated", "provider-e2e-output.png");
    mkdirSync(join(projectDir, "generated", "provider-e2e-generated"), {
      recursive: true,
    });
    writeFileSync(artifactPath, "generated pixels");
    writeFileSync(
      join(projectDir, "video-creater.project.json"),
      JSON.stringify({
        schemaVersion: 1,
        id: "provider-e2e-project",
        files: { generated: "generated", media: "media/index.json" },
      }),
    );
    mkdirSync(join(projectDir, "media"));
    writeFileSync(
      join(projectDir, "media", "index.json"),
      JSON.stringify({
        schemaVersion: 1,
        folders: [],
        assets: [
          {
            id: "provider-e2e-output",
            relativePath: "generated/provider-e2e-output.png",
            kind: "generated",
            durationSeconds: 0,
            width: 1024,
            height: 768,
            fps: 0,
          },
        ],
        analysis: [],
      }),
    );
    writeFileSync(
      join(projectDir, "generated", "index.json"),
      JSON.stringify({
        schemaVersion: 1,
        assets: [
          {
            assetId: "provider-e2e-generated",
            path: "generated/provider-e2e-generated/asset.json",
            status: "completed",
            modelProvider: "replicate",
            modelId: "black-forest-labs/flux-schnell",
            outputCount: 1,
          },
        ],
      }),
    );
    writeFileSync(
      join(projectDir, "generated", "provider-e2e-generated", "asset.json"),
      JSON.stringify({
        id: "provider-e2e-generated",
        status: "completed",
        model: {
          provider: "replicate",
          id: "black-forest-labs/flux-schnell",
        },
        outputs: [{ mediaId: "provider-e2e-output", relativePath: "generated/provider-e2e-output.png", sourceUrl: "https://provider.example/generated-output.png", width: 1024, height: 768, durationSeconds: 0, fps: 0 }],
      }),
    );
    writeFileSync(
      join(projectDir, "generated", "provider-e2e-output.png"),
      "project generated pixels",
    );
    mkdirSync(join(projectDir, "jobs", "provider-e2e-generated"), {
      recursive: true,
    });
    writeFileSync(
      join(projectDir, "jobs", "provider-e2e-generated", "job.json"),
      JSON.stringify({
        id: "provider-e2e-generated",
        kind: "generate_media",
        status: "completed",
        updatedAt: "2026-07-05T12:00:00Z",
        providerRequest: {
          provider: "replicate",
          requestId: "pred-123",
          statusUrl: "https://api.replicate.com/v1/predictions/pred-123",
          responseUrl: "https://api.replicate.com/v1/predictions/pred-123",
          cancelUrl: "https://api.replicate.com/v1/predictions/pred-123/cancel",
          submittedAt: "2026-07-05T12:00:00Z",
        },
      }),
    );
    writeFileSync(
      fakeCargo,
      [
        "#!/bin/sh",
        "printf '%s\\n' \"$*\" > \"$VIDEO_CREATER_FAKE_CARGO_ARGS\"",
        `printf '%s\\n' '{"ok":true,"provider":"replicate","model":"black-forest-labs/flux-schnell","projectDir":"${projectDir}","artifactPath":"${artifactPath}","outputCount":1,"jobStatus":"Completed","providerRequest":{"provider":"replicate","requestId":"pred-123","statusUrl":"https://api.replicate.com/v1/predictions/pred-123","responseUrl":"https://api.replicate.com/v1/predictions/pred-123","cancelUrl":"https://api.replicate.com/v1/predictions/pred-123/cancel","submittedAt":"2026-07-05T12:00:00Z"}}'`,
      ].join("\n"),
      { mode: 0o755 },
    );

    const argsPath = join(tempDir, "cargo-args.txt");
    const outDir = join(tempDir, "provider-out");
    const result = spawnSync(
      process.execPath,
      [
        join(repoRoot, "scripts/provider-e2e.mjs"),
        "--provider",
        "replicate",
        "--live",
        "--model",
        "black-forest-labs/flux-schnell",
        "--out-dir",
        outDir,
        "--live",
        "--prompt",
        "neon product launch frame",
        "--generate-audio",
        "false",
        "--max-status-polls",
        "3",
        "--poll-interval-ms",
        "10",
      ],
      {
        cwd: repoRoot,
        encoding: "utf8",
        env: {
          ...process.env,
          PATH: `${tempDir}:${process.env.PATH ?? ""}`,
          VIDEO_CREATER_FAKE_CARGO_ARGS: argsPath,
          VIDEO_CREATER_PROVIDER_E2E_ALLOW_SPEND: "1",
          REPLICATE_API_TOKEN: "super-secret-replicate-token",
        },
      },
    );

    expect(result.status).toBe(0);
    expect(result.stdout).not.toContain("super-secret-replicate-token");
    expect(result.stderr).not.toContain("super-secret-replicate-token");
    const cargoArgs = readFileSync(argsPath, "utf8");
    expect(cargoArgs).toContain(
      "run --manifest-path src-tauri/crates/provider-e2e-harness/Cargo.toml",
    );
    expect(cargoArgs).not.toContain("--bin video-creater-provider-e2e");
    expect(cargoArgs).toContain("--provider replicate");
    expect(cargoArgs).toContain("--model black-forest-labs/flux-schnell");
    expect(cargoArgs).toContain(`--out-dir ${outDir}`);
    expect(cargoArgs).toContain("--prompt neon product launch frame");
    expect(cargoArgs).toContain("--generate-audio false");
    expect(cargoArgs).toContain("--max-status-polls 3");
    expect(cargoArgs).toContain("--poll-interval-ms 10");
    const report = JSON.parse(result.stdout) as {
      ok: boolean;
      provider: string;
      credentialStatus: string;
      command: string;
      result: { ok: boolean; provider: string };
    };
    expect(report).toEqual(
      expect.objectContaining({
        ok: true,
        provider: "replicate",
        credentialStatus: "present",
        result: expect.objectContaining({ ok: true, provider: "replicate" }),
      }),
    );
    expect(report.command).not.toContain("super-secret-replicate-token");

    const writtenReport = JSON.parse(
      readFileSync(join(outDir, "provider-e2e-report.json"), "utf8"),
    ) as typeof report;
    expect(writtenReport).toEqual(report);
    expect(JSON.stringify(writtenReport)).not.toContain(
      "super-secret-replicate-token",
    );
  });

  it("keeps the provider E2E harness off the Temporal worker runtime", () => {
    const manifest = readFileSync(
      join(repoRoot, "src-tauri/crates/provider-e2e-harness/Cargo.toml"),
      "utf8",
    );

    expect(manifest).not.toContain('features = ["temporal-worker"]');
  });

  it("keeps the provider E2E harness off the Tauri app runtime", () => {
    const rootManifest = readFileSync(join(repoRoot, "src-tauri/Cargo.toml"), "utf8");
    const harnessManifest = readFileSync(
      join(repoRoot, "src-tauri/crates/provider-e2e-harness/Cargo.toml"),
      "utf8",
    );

    expect(rootManifest).toContain(
      'default = ["app-runtime", "coreml-inspect", "ges-render", "gpu-render", "graphics-render", "temporal-worker"]',
    );
    expect(rootManifest).toContain('app-runtime = ["dep:tauri"]');
    expect(rootManifest).toContain(
      'tauri = { version = "2.11.2", features = ["protocol-asset"], optional = true }',
    );
    expect(harnessManifest).not.toContain("app-runtime");
  });

  it("keeps the provider E2E harness off the GPU renderer runtime", () => {
    const rootManifest = readFileSync(join(repoRoot, "src-tauri/Cargo.toml"), "utf8");
    const rootLib = readFileSync(join(repoRoot, "src-tauri/src/gpu_graphics/mod.rs"), "utf8");
    const harnessManifest = readFileSync(
      join(repoRoot, "src-tauri/crates/provider-e2e-harness/Cargo.toml"),
      "utf8",
    );

    expect(rootManifest).toContain(
      'default = ["app-runtime", "coreml-inspect", "ges-render", "gpu-render", "graphics-render", "temporal-worker"]',
    );
    expect(rootManifest).toContain('gpu-render = ["dep:pollster", "dep:wgpu"]');
    expect(rootManifest).toContain('pollster = { version = "0.4.0", optional = true }');
    expect(rootManifest).toContain('wgpu = { version = "29.0.3", features = ["glsl", "wgsl"], optional = true }');
    expect(rootLib).toContain('#[cfg(feature = "gpu-render")]');
    expect(rootLib).toContain('#[cfg(not(feature = "gpu-render"))]');
    expect(harnessManifest).not.toContain("gpu-render");
  });

  it("keeps the provider E2E harness off the graphics rasterizer runtime", () => {
    const rootManifest = readFileSync(join(repoRoot, "src-tauri/Cargo.toml"), "utf8");
    const graphicsModule = readFileSync(join(repoRoot, "src-tauri/src/graphics/mod.rs"), "utf8");
    const harnessManifest = readFileSync(
      join(repoRoot, "src-tauri/crates/provider-e2e-harness/Cargo.toml"),
      "utf8",
    );

    expect(rootManifest).toContain(
      'default = ["app-runtime", "coreml-inspect", "ges-render", "gpu-render", "graphics-render", "temporal-worker"]',
    );
    expect(rootManifest).toContain(
      'graphics-render = ["dep:cosmic-text", "dep:tiny-skia"]',
    );
    expect(rootManifest).toContain(
      'cosmic-text = { version = "0.19.0", optional = true }',
    );
    expect(rootManifest).toContain(
      'tiny-skia = { version = "0.12.0", optional = true }',
    );
    expect(graphicsModule).toContain('#[cfg(feature = "graphics-render")]');
    expect(graphicsModule).toContain('#[cfg(not(feature = "graphics-render"))]');
    expect(harnessManifest).not.toContain("graphics-render");
  });

  it("keeps unused GPU math crates out of the provider E2E harness", () => {
    const rootManifest = readFileSync(join(repoRoot, "src-tauri/Cargo.toml"), "utf8");

    expect(rootManifest).not.toContain('glam = "');
  });

  it("keeps the provider E2E harness local PNG writer off the image crate", () => {
    const harnessManifest = readFileSync(
      join(repoRoot, "src-tauri/crates/provider-e2e-harness/Cargo.toml"),
      "utf8",
    );

    expect(harnessManifest).toContain('png = "0.18.1"');
    expect(harnessManifest).not.toContain('image = ');
  });

  it("keeps the provider E2E harness off the Core ML inspection runtime", () => {
    const rootManifest = readFileSync(join(repoRoot, "src-tauri/Cargo.toml"), "utf8");
    const inspectorBin = readFileSync(
      join(repoRoot, "src-tauri/src/bin/video-creater-inspect-coreml-parakeet.rs"),
      "utf8",
    );
    const harnessManifest = readFileSync(
      join(repoRoot, "src-tauri/crates/provider-e2e-harness/Cargo.toml"),
      "utf8",
    );

    expect(rootManifest).toContain(
      'default = ["app-runtime", "coreml-inspect", "ges-render", "gpu-render", "graphics-render", "temporal-worker"]',
    );
    expect(rootManifest).toContain('coreml-inspect = ["dep:coreml-native"]');
    expect(rootManifest).toContain('coreml-native = { version = "0.2.0", optional = true }');
    expect(inspectorBin).toContain(
      '#[cfg(all(target_os = "macos", feature = "coreml-inspect"))]',
    );
    expect(inspectorBin).toContain(
      '#[cfg(all(target_os = "macos", not(feature = "coreml-inspect")))]',
    );
    expect(harnessManifest).not.toContain("coreml-inspect");
  });

  it("keeps retained provider E2E policy aligned with wrapper and Rust live scenarios", () => {
    const tempDir = mkdtempSync(join(tmpdir(), "video-creater-provider-matrix-"));
    const wrapperUrl = pathToFileURL(join(repoRoot, "scripts/provider-e2e.mjs")).href;
    const policyUrl = pathToFileURL(join(repoRoot, "scripts/provider-e2e-report-policy.mjs")).href;
    const rustHarnessPath = join(repoRoot, "src-tauri/src/bin/video-creater-provider-e2e.rs");
    const result = spawnSync(
      process.execPath,
      ["--input-type=module"],
      {
        cwd: tempDir,
        input: `
          import { readFileSync } from "node:fs";
          const wrapper = await import(${JSON.stringify(wrapperUrl)});
          const policy = await import(${JSON.stringify(policyUrl)});
          const rustSource = readFileSync(${JSON.stringify(rustHarnessPath)}, "utf8");
          const providerNames = Object.keys(wrapper.providerEnvVars ?? {});
          const supportedPairs = [];
          for (const [scenario, value] of Object.entries(wrapper.scenarioDefaultModels ?? {})) {
            const provider = value?.provider;
            const models = value?.models;
            if (provider) {
              supportedPairs.push(provider + "\\u0000" + scenario);
            } else if (models) {
              for (const providerName of Object.keys(models)) {
                supportedPairs.push(providerName + "\\u0000" + scenario);
              }
            } else {
              for (const providerName of providerNames) {
                supportedPairs.push(providerName + "\\u0000" + scenario);
              }
            }
          }
          const requiredReports = policy.requiredReports ?? [];
          const requiredPairs = requiredReports.map((report) => report.provider + "\\u0000" + report.scenario);
          const duplicatePairs = requiredPairs.filter((pair, index) => requiredPairs.indexOf(pair) !== index);
          const missingWrapperPairs = requiredPairs.filter((pair) => !supportedPairs.includes(pair));
          const rustScenarios = [...rustSource.matchAll(/"([^"]+)" => Ok\\(Self::/g)].map((match) => match[1]);
          const syntheticScenarios = new Set(["provider-failure", "provider-cancellation", "provider-retry"]);
          const missingRustScenarios = [...new Set(requiredReports.map((report) => report.scenario))]
            .filter((scenario) => !syntheticScenarios.has(scenario) && !rustScenarios.includes(scenario));
          if (providerNames.length === 0 || requiredReports.length === 0 || duplicatePairs.length > 0 || missingWrapperPairs.length > 0 || missingRustScenarios.length > 0) {
            throw new Error(JSON.stringify({
              providerNames,
              requiredReportCount: requiredReports.length,
              duplicatePairs,
              missingWrapperPairs,
              missingRustScenarios,
            }));
          }
          console.log(JSON.stringify({
            marker: "provider-e2e-matrix-ok",
            providerCount: providerNames.length,
            requiredReportCount: requiredReports.length,
            supportedPairCount: supportedPairs.length,
          }));
        `,
        encoding: "utf8",
        env: {
          ...process.env,
          FAL_KEY: "",
          REPLICATE_API_TOKEN: "",
          OPENAI_API_KEY: "",
          XAI_API_KEY: "",
          GEMINI_API_KEY: "",
          ELEVENLABS_API_KEY: "",
          MINIMAX_API_KEY: "",
          VIDEO_CREATER_PROVIDER_E2E_ALLOW_SPEND: "",
        },
      },
    );

    expect(result.status).toBe(0);
    expect(result.stderr).toBe("");
    expect(result.stdout).toContain('"marker":"provider-e2e-matrix-ok"');
  });

  it("forwards the local image upscale provider E2E scenario to the Rust binary", () => {
    const tempDir = mkdtempSync(join(tmpdir(), "video-creater-provider-upscale-e2e-"));
    const fakeCargo = join(tempDir, "cargo");
    const projectDir = join(tempDir, "project");
    const artifactPath = join(projectDir, "generated", "provider-e2e-upscale-output.png");
    mkdirSync(join(projectDir, "generated", "provider-e2e-generated"), {
      recursive: true,
    });
    mkdirSync(join(projectDir, "media"), { recursive: true });
    mkdirSync(join(projectDir, "jobs", "provider-e2e-generated"), {
      recursive: true,
    });
    writeFileSync(artifactPath, "upscaled pixels");
    writeFileSync(
      join(projectDir, "video-creater.project.json"),
      JSON.stringify({
        schemaVersion: 1,
        id: "provider-e2e-project",
        files: { generated: "generated", media: "media/index.json" },
      }),
    );
    writeFileSync(
      join(projectDir, "media", "index.json"),
      JSON.stringify({
        schemaVersion: 1,
        folders: [],
        assets: [
          {
            id: "provider-e2e-upscale-output",
            relativePath: "generated/provider-e2e-upscale-output.png",
            kind: "generated",
            durationSeconds: 0,
            width: 1024,
            height: 768,
            fps: 0,
          },
        ],
        analysis: [],
      }),
    );
    writeFileSync(
      join(projectDir, "generated", "index.json"),
      JSON.stringify({
        schemaVersion: 1,
        assets: [
          {
            assetId: "provider-e2e-generated",
            path: "generated/provider-e2e-generated/asset.json",
            status: "completed",
            modelProvider: "fal.ai",
            modelId: "fal-ai/aura-sr",
            outputCount: 1,
          },
        ],
      }),
    );
    writeFileSync(
      join(projectDir, "generated", "provider-e2e-generated", "asset.json"),
      JSON.stringify({
        id: "provider-e2e-generated",
        status: "completed",
        model: { provider: "fal.ai", id: "fal-ai/aura-sr" },
        references: {
          mediaIds: ["provider-e2e-source"],
          firstFrameMediaId: null,
          lastFrameMediaId: null,
          providerInputUrls: ["https://fal.media/uploads/provider-e2e-source.png"],
        },
        outputs: [
          {
            mediaId: "provider-e2e-upscale-output",
            relativePath: "generated/provider-e2e-upscale-output.png",
            sourceUrl: "https://fal.media/generated/upscale-output.png",
            width: 1024,
            height: 768,
            durationSeconds: 0,
            fps: 0,
          },
        ],
      }),
    );
    writeFileSync(
      join(projectDir, "jobs", "provider-e2e-generated", "job.json"),
      JSON.stringify({
        id: "provider-e2e-generated",
        kind: "generate_media",
        status: "completed",
        updatedAt: "2026-07-05T12:00:00Z",
        providerRequest: {
          provider: "fal.ai",
          requestId: "fal-upscale-123",
          statusUrl: "https://queue.fal.run/fal-ai/aura-sr/requests/fal-upscale-123/status",
          responseUrl: "https://queue.fal.run/fal-ai/aura-sr/requests/fal-upscale-123",
          cancelUrl: "https://queue.fal.run/fal-ai/aura-sr/requests/fal-upscale-123/cancel",
          submittedAt: "2026-07-05T12:00:00Z",
        },
      }),
    );
    const argsPath = join(tempDir, "cargo-args.txt");
    const cargoResponsePath = join(tempDir, "cargo-response.json");
    writeFileSync(
      cargoResponsePath,
      `${JSON.stringify({
        ok: true,
        scenario: "local-image-upscale",
        provider: "fal.ai",
        model: "fal-ai/aura-sr",
        projectDir,
        artifactPath,
        outputCount: 1,
        jobStatus: "Completed",
        providerRequest: {
          provider: "fal.ai",
          requestId: "fal-upscale-123",
          statusUrl: "https://queue.fal.run/fal-ai/aura-sr/requests/fal-upscale-123/status",
          responseUrl: "https://queue.fal.run/fal-ai/aura-sr/requests/fal-upscale-123",
          cancelUrl: "https://queue.fal.run/fal-ai/aura-sr/requests/fal-upscale-123/cancel",
          submittedAt: "2026-07-05T12:00:00Z",
        },
      })}\n`,
    );
    writeFileSync(
      fakeCargo,
      [
        "#!/usr/bin/env node",
        "const fs = require('node:fs');",
        "fs.writeFileSync(process.env.VIDEO_CREATER_FAKE_CARGO_ARGS, process.argv.slice(2).join(' ') + '\\n');",
        "process.stdout.write(fs.readFileSync(process.env.VIDEO_CREATER_FAKE_CARGO_RESPONSE, 'utf8'));",
      ].join("\n") + "\n",
      { mode: 0o755 },
    );
    const outDir = join(tempDir, "provider-out");
    const result = spawnSync(
      process.execPath,
      [
        join(repoRoot, "scripts/provider-e2e.mjs"),
        "--provider",
        "fal.ai",
        "--live",
        "--scenario",
        "local-image-upscale",
        "--out-dir",
        outDir,
      ],
      {
        cwd: repoRoot,
        encoding: "utf8",
        env: {
          ...process.env,
          PATH: `${tempDir}:${process.env.PATH ?? ""}`,
          VIDEO_CREATER_FAKE_CARGO_ARGS: argsPath,
          VIDEO_CREATER_FAKE_CARGO_RESPONSE: cargoResponsePath,
          VIDEO_CREATER_PROVIDER_E2E_ALLOW_SPEND: "1",
          FAL_KEY: "super-secret-fal-token",
        },
      },
    );

    expect(result.status).toBe(0);
    expect(result.stdout).not.toContain("super-secret-fal-token");
    expect(result.stderr).not.toContain("super-secret-fal-token");
    const cargoArgs = readFileSync(argsPath, "utf8");
    expect(cargoArgs).toContain("--provider fal.ai");
    expect(cargoArgs).toContain("--model fal-ai/aura-sr");
    expect(cargoArgs).toContain("--scenario local-image-upscale");
    const report = JSON.parse(result.stdout) as {
      ok: boolean;
      provider: string;
      model: string;
      scenario: string;
      result: { scenario: string; provider: string; model: string };
    };
    expect(report).toEqual(
      expect.objectContaining({
        ok: true,
        provider: "fal.ai",
        model: "fal-ai/aura-sr",
        scenario: "local-image-upscale",
        result: expect.objectContaining({
          scenario: "local-image-upscale",
          provider: "fal.ai",
          model: "fal-ai/aura-sr",
        }),
      }),
    );
  });

  it("forwards the local image upscale replacement provider E2E scenario to the Rust binary", () => {
    const tempDir = mkdtempSync(join(tmpdir(), "video-creater-provider-replace-e2e-"));
    const fakeCargo = join(tempDir, "cargo");
    const reportPath = writeProviderPolicyFixture({
      rootDir: tempDir,
      provider: "fal.ai",
      model: "fal-ai/aura-sr",
      requestId: "fal-replace-123",
      scenario: "local-image-upscale-replace",
    });
    const fixtureReport = JSON.parse(readFileSync(reportPath, "utf8")) as {
      result: unknown;
    };
    const argsPath = join(tempDir, "cargo-args.txt");
    const cargoResponsePath = join(tempDir, "cargo-response.json");
    writeFileSync(cargoResponsePath, `${JSON.stringify(fixtureReport.result)}\n`);
    writeFileSync(
      fakeCargo,
      [
        "#!/usr/bin/env node",
        "const fs = require('node:fs');",
        "fs.writeFileSync(process.env.VIDEO_CREATER_FAKE_CARGO_ARGS, process.argv.slice(2).join(' ') + '\\n');",
        "process.stdout.write(fs.readFileSync(process.env.VIDEO_CREATER_FAKE_CARGO_RESPONSE, 'utf8'));",
      ].join("\n") + "\n",
      { mode: 0o755 },
    );
    const outDir = join(tempDir, "provider-out");
    const result = spawnSync(
      process.execPath,
      [
        join(repoRoot, "scripts/provider-e2e.mjs"),
        "--provider",
        "fal.ai",
        "--live",
        "--scenario",
        "local-image-upscale-replace",
        "--out-dir",
        outDir,
      ],
      {
        cwd: repoRoot,
        encoding: "utf8",
        env: {
          ...process.env,
          PATH: `${tempDir}:${process.env.PATH ?? ""}`,
          VIDEO_CREATER_FAKE_CARGO_ARGS: argsPath,
          VIDEO_CREATER_FAKE_CARGO_RESPONSE: cargoResponsePath,
          VIDEO_CREATER_PROVIDER_E2E_ALLOW_SPEND: "1",
          FAL_KEY: "super-secret-fal-token",
        },
      },
    );

    expect(result.status).toBe(0);
    expect(result.stdout).not.toContain("super-secret-fal-token");
    expect(result.stderr).not.toContain("super-secret-fal-token");
    const cargoArgs = readFileSync(argsPath, "utf8");
    expect(cargoArgs).toContain("--provider fal.ai");
    expect(cargoArgs).toContain("--model fal-ai/aura-sr");
    expect(cargoArgs).toContain("--scenario local-image-upscale-replace");
    const report = JSON.parse(result.stdout) as {
      ok: boolean;
      provider: string;
      model: string;
      scenario: string;
      result: { scenario: string; outputCount: number };
    };
    expect(report).toEqual(
      expect.objectContaining({
        ok: true,
        provider: "fal.ai",
        model: "fal-ai/aura-sr",
        scenario: "local-image-upscale-replace",
        result: expect.objectContaining({
          scenario: "local-image-upscale-replace",
          outputCount: 1,
        }),
      }),
    );
  });

  it("forwards the text-to-image replacement provider E2E scenario to the Rust binary", () => {
    const tempDir = mkdtempSync(join(tmpdir(), "video-creater-provider-text-image-replace-e2e-"));
    const fakeCargo = join(tempDir, "cargo");
    const reportPath = writeProviderPolicyFixture({
      rootDir: tempDir,
      provider: "openai",
      model: "gpt-image-2",
      requestId: "openai-text-image-replace-123",
      scenario: "text-to-image-replace",
    });
    const fixtureReport = JSON.parse(readFileSync(reportPath, "utf8")) as {
      result: unknown;
    };
    const argsPath = join(tempDir, "cargo-args.txt");
    const cargoResponsePath = join(tempDir, "cargo-response.json");
    writeFileSync(cargoResponsePath, `${JSON.stringify(fixtureReport.result)}\n`);
    writeFileSync(
      fakeCargo,
      [
        "#!/usr/bin/env node",
        "const fs = require('node:fs');",
        "fs.writeFileSync(process.env.VIDEO_CREATER_FAKE_CARGO_ARGS, process.argv.slice(2).join(' ') + '\\n');",
        "process.stdout.write(fs.readFileSync(process.env.VIDEO_CREATER_FAKE_CARGO_RESPONSE, 'utf8'));",
      ].join("\n") + "\n",
      { mode: 0o755 },
    );
    const outDir = join(tempDir, "provider-out");
    const result = spawnSync(
      process.execPath,
      [
        join(repoRoot, "scripts/provider-e2e.mjs"),
        "--provider",
        "openai",
        "--live",
        "--scenario",
        "text-to-image-replace",
        "--out-dir",
        outDir,
      ],
      {
        cwd: repoRoot,
        encoding: "utf8",
        env: {
          ...process.env,
          PATH: `${tempDir}:${process.env.PATH ?? ""}`,
          VIDEO_CREATER_FAKE_CARGO_ARGS: argsPath,
          VIDEO_CREATER_FAKE_CARGO_RESPONSE: cargoResponsePath,
          VIDEO_CREATER_PROVIDER_E2E_ALLOW_SPEND: "1",
          OPENAI_API_KEY: "super-secret-openai-token",
        },
      },
    );

    expect(result.status).toBe(0);
    expect(result.stdout).not.toContain("super-secret-openai-token");
    expect(result.stderr).not.toContain("super-secret-openai-token");
    const cargoArgs = readFileSync(argsPath, "utf8");
    expect(cargoArgs).toContain("--provider openai");
    expect(cargoArgs).toContain("--model gpt-image-2");
    expect(cargoArgs).toContain("--scenario text-to-image-replace");
    const report = JSON.parse(result.stdout) as {
      ok: boolean;
      provider: string;
      model: string;
      scenario: string;
      result: { scenario: string; outputCount: number };
    };
    expect(report).toEqual(
      expect.objectContaining({
        ok: true,
        provider: "openai",
        model: "gpt-image-2",
        scenario: "text-to-image-replace",
        result: expect.objectContaining({
          scenario: "text-to-image-replace",
          outputCount: 1,
        }),
      }),
    );
  });

  it("forwards the text-to-video replacement provider E2E scenario to the Rust binary", () => {
    const tempDir = mkdtempSync(join(tmpdir(), "video-creater-provider-text-video-replace-e2e-"));
    const fakeCargo = join(tempDir, "cargo");
    const reportPath = writeProviderPolicyFixture({
      rootDir: tempDir,
      provider: "google",
      model: "veo3.1-fast",
      requestId: "google-text-video-replace-123",
      scenario: "text-to-video-replace",
      sourceUrl: "https://provider.example/google-text-video-replace-123/generated-video.mp4",
    });
    const fixtureReport = JSON.parse(readFileSync(reportPath, "utf8")) as {
      result: unknown;
    };
    const argsPath = join(tempDir, "cargo-args.txt");
    const cargoResponsePath = join(tempDir, "cargo-response.json");
    writeFileSync(cargoResponsePath, `${JSON.stringify(fixtureReport.result)}\n`);
    writeFileSync(
      fakeCargo,
      [
        "#!/usr/bin/env node",
        "const fs = require('node:fs');",
        "fs.writeFileSync(process.env.VIDEO_CREATER_FAKE_CARGO_ARGS, process.argv.slice(2).join(' ') + '\\n');",
        "process.stdout.write(fs.readFileSync(process.env.VIDEO_CREATER_FAKE_CARGO_RESPONSE, 'utf8'));",
      ].join("\n") + "\n",
      { mode: 0o755 },
    );
    const outDir = join(tempDir, "provider-out");
    const result = spawnSync(
      process.execPath,
      [
        join(repoRoot, "scripts/provider-e2e.mjs"),
        "--provider",
        "google",
        "--live",
        "--scenario",
        "text-to-video-replace",
        "--out-dir",
        outDir,
      ],
      {
        cwd: repoRoot,
        encoding: "utf8",
        env: {
          ...process.env,
          PATH: `${tempDir}:${process.env.PATH ?? ""}`,
          VIDEO_CREATER_FAKE_CARGO_ARGS: argsPath,
          VIDEO_CREATER_FAKE_CARGO_RESPONSE: cargoResponsePath,
          VIDEO_CREATER_PROVIDER_E2E_ALLOW_SPEND: "1",
          GEMINI_API_KEY: "super-secret-google-token",
        },
      },
    );

    expect(result.status).toBe(0);
    expect(result.stdout).not.toContain("super-secret-google-token");
    expect(result.stderr).not.toContain("super-secret-google-token");
    const cargoArgs = readFileSync(argsPath, "utf8");
    expect(cargoArgs).toContain("--provider google");
    expect(cargoArgs).toContain("--model veo3.1-fast");
    expect(cargoArgs).toContain("--scenario text-to-video-replace");
    const report = JSON.parse(result.stdout) as {
      ok: boolean;
      provider: string;
      model: string;
      scenario: string;
      result: { scenario: string; outputCount: number };
    };
    expect(report).toEqual(
      expect.objectContaining({
        ok: true,
        provider: "google",
        model: "veo3.1-fast",
        scenario: "text-to-video-replace",
        result: expect.objectContaining({
          scenario: "text-to-video-replace",
          outputCount: 1,
        }),
      }),
    );
  });

  it("forwards the local video edit replacement provider E2E scenario to the Rust binary", () => {
    const tempDir = mkdtempSync(join(tmpdir(), "video-creater-provider-video-edit-replace-e2e-"));
    const fakeCargo = join(tempDir, "cargo");
    const reportPath = writeProviderPolicyFixture({
      rootDir: tempDir,
      provider: "fal.ai",
      model: "fal-ai/wan/v2.2-a14b/video-to-video",
      requestId: "fal-video-edit-replace-123",
      scenario: "local-video-edit-replace",
      sourceUrl: "https://provider.example/fal-video-edit-replace-123/edited-video.mp4",
    });
    const fixtureReport = JSON.parse(readFileSync(reportPath, "utf8")) as {
      result: unknown;
    };
    const argsPath = join(tempDir, "cargo-args.txt");
    const cargoResponsePath = join(tempDir, "cargo-response.json");
    writeFileSync(cargoResponsePath, `${JSON.stringify(fixtureReport.result)}\n`);
    writeFileSync(
      fakeCargo,
      [
        "#!/usr/bin/env node",
        "const fs = require('node:fs');",
        "fs.writeFileSync(process.env.VIDEO_CREATER_FAKE_CARGO_ARGS, process.argv.slice(2).join(' ') + '\\n');",
        "process.stdout.write(fs.readFileSync(process.env.VIDEO_CREATER_FAKE_CARGO_RESPONSE, 'utf8'));",
      ].join("\n") + "\n",
      { mode: 0o755 },
    );
    const outDir = join(tempDir, "provider-out");
    const result = spawnSync(
      process.execPath,
      [
        join(repoRoot, "scripts/provider-e2e.mjs"),
        "--provider",
        "fal.ai",
        "--live",
        "--scenario",
        "local-video-edit-replace",
        "--out-dir",
        outDir,
      ],
      {
        cwd: repoRoot,
        encoding: "utf8",
        env: {
          ...process.env,
          PATH: `${tempDir}:${process.env.PATH ?? ""}`,
          VIDEO_CREATER_FAKE_CARGO_ARGS: argsPath,
          VIDEO_CREATER_FAKE_CARGO_RESPONSE: cargoResponsePath,
          VIDEO_CREATER_PROVIDER_E2E_ALLOW_SPEND: "1",
          FAL_KEY: "super-secret-fal-token",
        },
      },
    );

    expect(result.status).toBe(0);
    expect(result.stdout).not.toContain("super-secret-fal-token");
    expect(result.stderr).not.toContain("super-secret-fal-token");
    const cargoArgs = readFileSync(argsPath, "utf8");
    expect(cargoArgs).toContain("--provider fal.ai");
    expect(cargoArgs).toContain("--model fal-ai/wan/v2.2-a14b/video-to-video");
    expect(cargoArgs).toContain("--scenario local-video-edit-replace");
    const report = JSON.parse(result.stdout) as {
      ok: boolean;
      provider: string;
      model: string;
      scenario: string;
      result: { scenario: string; outputCount: number };
    };
    expect(report).toEqual(
      expect.objectContaining({
        ok: true,
        provider: "fal.ai",
        model: "fal-ai/wan/v2.2-a14b/video-to-video",
        scenario: "local-video-edit-replace",
        result: expect.objectContaining({
          scenario: "local-video-edit-replace",
          outputCount: 1,
        }),
      }),
    );
  });

  it("forwards the local video upscale provider E2E scenario to the Rust binary", () => {
    const tempDir = mkdtempSync(join(tmpdir(), "video-creater-provider-video-upscale-e2e-"));
    const fakeCargo = join(tempDir, "cargo");
    const reportPath = writeProviderPolicyFixture({
      rootDir: tempDir,
      provider: "fal.ai",
      model: "fal-ai/video-upscaler",
      requestId: "fal-video-upscale-123",
      scenario: "local-video-upscale",
      sourceUrl: "https://provider.example/fal-video-upscale-123/upscaled-video.mp4",
    });
    const fixtureReport = JSON.parse(readFileSync(reportPath, "utf8")) as {
      result: unknown;
    };
    const argsPath = join(tempDir, "cargo-args.txt");
    const cargoResponsePath = join(tempDir, "cargo-response.json");
    writeFileSync(cargoResponsePath, `${JSON.stringify(fixtureReport.result)}\n`);
    writeFileSync(
      fakeCargo,
      [
        "#!/usr/bin/env node",
        "const fs = require('node:fs');",
        "fs.writeFileSync(process.env.VIDEO_CREATER_FAKE_CARGO_ARGS, process.argv.slice(2).join(' ') + '\\n');",
        "process.stdout.write(fs.readFileSync(process.env.VIDEO_CREATER_FAKE_CARGO_RESPONSE, 'utf8'));",
      ].join("\n") + "\n",
      { mode: 0o755 },
    );
    const outDir = join(tempDir, "provider-out");
    const result = spawnSync(
      process.execPath,
      [
        join(repoRoot, "scripts/provider-e2e.mjs"),
        "--provider",
        "fal.ai",
        "--live",
        "--scenario",
        "local-video-upscale",
        "--out-dir",
        outDir,
      ],
      {
        cwd: repoRoot,
        encoding: "utf8",
        env: {
          ...process.env,
          PATH: `${tempDir}:${process.env.PATH ?? ""}`,
          VIDEO_CREATER_FAKE_CARGO_ARGS: argsPath,
          VIDEO_CREATER_FAKE_CARGO_RESPONSE: cargoResponsePath,
          VIDEO_CREATER_PROVIDER_E2E_ALLOW_SPEND: "1",
          FAL_KEY: "super-secret-fal-token",
        },
      },
    );

    expect(result.status).toBe(0);
    expect(result.stdout).not.toContain("super-secret-fal-token");
    expect(result.stderr).not.toContain("super-secret-fal-token");
    const cargoArgs = readFileSync(argsPath, "utf8");
    expect(cargoArgs).toContain("--provider fal.ai");
    expect(cargoArgs).toContain("--model fal-ai/video-upscaler");
    expect(cargoArgs).toContain("--scenario local-video-upscale");
    const report = JSON.parse(result.stdout) as {
      ok: boolean;
      provider: string;
      model: string;
      scenario: string;
      result: { scenario: string; outputCount: number; artifactPath: string };
    };
    expect(report).toEqual(
      expect.objectContaining({
        ok: true,
        provider: "fal.ai",
        model: "fal-ai/video-upscaler",
        scenario: "local-video-upscale",
        result: expect.objectContaining({
          scenario: "local-video-upscale",
          outputCount: 1,
        }),
      }),
    );
    expect(report.result.artifactPath.endsWith(".mp4")).toBe(true);
  });

  it("forwards the WAN image-to-video provider E2E scenario to the Rust binary", () => {
    const tempDir = mkdtempSync(join(tmpdir(), "video-creater-provider-wan-e2e-"));
    const fakeCargo = join(tempDir, "cargo");
    const projectDir = join(tempDir, "project");
    const artifactPath = join(projectDir, "generated", "provider-e2e-generated", "fal-wan-output.mp4");
    mkdirSync(join(projectDir, "generated", "provider-e2e-generated"), {
      recursive: true,
    });
    mkdirSync(join(projectDir, "media"), { recursive: true });
    mkdirSync(join(projectDir, "jobs", "provider-e2e-generated"), {
      recursive: true,
    });
    writeFileSync(artifactPath, "wan video bytes");
    writeFileSync(
      join(projectDir, "video-creater.project.json"),
      JSON.stringify({
        schemaVersion: 1,
        id: "provider-e2e-project",
        files: { generated: "generated", media: "media/index.json" },
      }),
    );
    writeFileSync(
      join(projectDir, "media", "index.json"),
      JSON.stringify({
        schemaVersion: 1,
        folders: [],
        assets: [
          {
            id: "provider-e2e-wan-output",
            relativePath: "generated/provider-e2e-generated/fal-wan-output.mp4",
            kind: "generated",
            durationSeconds: 4,
            width: 1280,
            height: 720,
            fps: 24,
          },
        ],
        analysis: [],
      }),
    );
    writeFileSync(
      join(projectDir, "generated", "index.json"),
      JSON.stringify({
        schemaVersion: 1,
        assets: [
          {
            assetId: "provider-e2e-generated",
            path: "generated/provider-e2e-generated/asset.json",
            status: "completed",
            modelProvider: "fal.ai",
            modelId: "fal-ai/wan/v2.7/image-to-video",
            outputCount: 1,
          },
        ],
      }),
    );
    writeFileSync(
      join(projectDir, "generated", "provider-e2e-generated", "asset.json"),
      JSON.stringify({
        id: "provider-e2e-generated",
        status: "completed",
        model: { provider: "fal.ai", id: "fal-ai/wan/v2.7/image-to-video" },
        references: {
          mediaIds: ["provider-e2e-first-frame", "provider-e2e-last-frame", "provider-e2e-audio"],
          firstFrameMediaId: "provider-e2e-first-frame",
          lastFrameMediaId: "provider-e2e-last-frame",
          referenceAudioMediaRefs: ["provider-e2e-audio"],
          providerInputUrls: [
            "https://v3.fal.media/files/provider-e2e/first-frame.png",
            "https://v3.fal.media/files/provider-e2e/last-frame.png",
            "https://v3.fal.media/files/provider-e2e/audio.wav",
          ],
        },
        outputs: [
          {
            mediaId: "provider-e2e-wan-output",
            relativePath: "generated/provider-e2e-generated/fal-wan-output.mp4",
            sourceUrl: "https://fal.media/generated/wan-output.mp4",
            width: 1280,
            height: 720,
            durationSeconds: 4,
            fps: 24,
          },
        ],
      }),
    );
    writeFileSync(
      join(projectDir, "jobs", "provider-e2e-generated", "job.json"),
      JSON.stringify({
        id: "provider-e2e-generated",
        kind: "generate_media",
        status: "completed",
        updatedAt: "2026-07-05T12:00:00Z",
        providerRequest: {
          provider: "fal.ai",
          requestId: "fal-wan-123",
          statusUrl: "https://queue.fal.run/fal-ai/wan/v2.7/image-to-video/requests/fal-wan-123/status",
          responseUrl: "https://queue.fal.run/fal-ai/wan/v2.7/image-to-video/requests/fal-wan-123",
          cancelUrl: "https://queue.fal.run/fal-ai/wan/v2.7/image-to-video/requests/fal-wan-123/cancel",
          submittedAt: "2026-07-05T12:00:00Z",
        },
      }),
    );
    const argsPath = join(tempDir, "cargo-args.txt");
    const cargoResponsePath = join(tempDir, "cargo-response.json");
    writeFileSync(
      cargoResponsePath,
      `${JSON.stringify({
        ok: true,
        scenario: "wan-image-to-video",
        provider: "fal.ai",
        model: "fal-ai/wan/v2.7/image-to-video",
        projectDir,
        artifactPath,
        outputCount: 1,
        jobStatus: "Completed",
        providerRequest: {
          provider: "fal.ai",
          requestId: "fal-wan-123",
          statusUrl: "https://queue.fal.run/fal-ai/wan/v2.7/image-to-video/requests/fal-wan-123/status",
          responseUrl: "https://queue.fal.run/fal-ai/wan/v2.7/image-to-video/requests/fal-wan-123",
          cancelUrl: "https://queue.fal.run/fal-ai/wan/v2.7/image-to-video/requests/fal-wan-123/cancel",
          submittedAt: "2026-07-05T12:00:00Z",
        },
      })}\n`,
    );
    writeFileSync(
      fakeCargo,
      [
        "#!/usr/bin/env node",
        "const fs = require('node:fs');",
        "fs.writeFileSync(process.env.VIDEO_CREATER_FAKE_CARGO_ARGS, process.argv.slice(2).join(' ') + '\\n');",
        "process.stdout.write(fs.readFileSync(process.env.VIDEO_CREATER_FAKE_CARGO_RESPONSE, 'utf8'));",
      ].join("\n") + "\n",
      { mode: 0o755 },
    );
    const outDir = join(tempDir, "provider-out");
    const result = spawnSync(
      process.execPath,
      [
        join(repoRoot, "scripts/provider-e2e.mjs"),
        "--provider",
        "fal.ai",
        "--live",
        "--scenario",
        "wan-image-to-video",
        "--out-dir",
        outDir,
      ],
      {
        cwd: repoRoot,
        encoding: "utf8",
        env: {
          ...process.env,
          PATH: `${tempDir}:${process.env.PATH ?? ""}`,
          VIDEO_CREATER_FAKE_CARGO_ARGS: argsPath,
          VIDEO_CREATER_FAKE_CARGO_RESPONSE: cargoResponsePath,
          VIDEO_CREATER_PROVIDER_E2E_ALLOW_SPEND: "1",
          FAL_KEY: "super-secret-fal-token",
        },
      },
    );

    expect(result.status).toBe(0);
    expect(result.stdout).not.toContain("super-secret-fal-token");
    expect(result.stderr).not.toContain("super-secret-fal-token");
    const cargoArgs = readFileSync(argsPath, "utf8");
    expect(cargoArgs).toContain("--provider fal.ai");
    expect(cargoArgs).toContain("--model fal-ai/wan/v2.7/image-to-video");
    expect(cargoArgs).toContain("--scenario wan-image-to-video");
    const report = JSON.parse(result.stdout) as {
      ok: boolean;
      provider: string;
      model: string;
      scenario: string;
      result: { scenario: string; outputCount: number; artifactPath: string };
    };
    expect(report).toEqual(
      expect.objectContaining({
        ok: true,
        provider: "fal.ai",
        model: "fal-ai/wan/v2.7/image-to-video",
        scenario: "wan-image-to-video",
        result: expect.objectContaining({
          scenario: "wan-image-to-video",
          outputCount: 1,
          artifactPath,
        }),
      }),
    );
  });

  it("forwards the WAN reference-to-video provider E2E scenario to the Rust binary", () => {
    const tempDir = mkdtempSync(join(tmpdir(), "video-creater-provider-wan-reference-e2e-"));
    const fakeCargo = join(tempDir, "cargo");
    const projectDir = join(tempDir, "project");
    const artifactPath = join(projectDir, "generated", "provider-e2e-generated", "fal-wan-reference-output.mp4");
    mkdirSync(join(projectDir, "generated", "provider-e2e-generated"), {
      recursive: true,
    });
    mkdirSync(join(projectDir, "media"), { recursive: true });
    mkdirSync(join(projectDir, "jobs", "provider-e2e-generated"), {
      recursive: true,
    });
    writeFileSync(artifactPath, "wan reference video bytes");
    writeFileSync(
      join(projectDir, "video-creater.project.json"),
      JSON.stringify({
        schemaVersion: 1,
        id: "provider-e2e-project",
        files: { generated: "generated", media: "media/index.json" },
      }),
    );
    writeFileSync(
      join(projectDir, "media", "index.json"),
      JSON.stringify({
        schemaVersion: 1,
        folders: [],
        assets: [
          {
            id: "provider-e2e-wan-reference-output",
            relativePath: "generated/provider-e2e-generated/fal-wan-reference-output.mp4",
            kind: "generated",
            durationSeconds: 4,
            width: 1280,
            height: 720,
            fps: 24,
          },
        ],
        analysis: [],
      }),
    );
    writeFileSync(
      join(projectDir, "generated", "index.json"),
      JSON.stringify({
        schemaVersion: 1,
        assets: [
          {
            assetId: "provider-e2e-generated",
            path: "generated/provider-e2e-generated/asset.json",
            status: "completed",
            modelProvider: "fal.ai",
            modelId: "fal-ai/wan/v2.7/reference-to-video",
            outputCount: 1,
          },
        ],
      }),
    );
    writeFileSync(
      join(projectDir, "generated", "provider-e2e-generated", "asset.json"),
      JSON.stringify({
        id: "provider-e2e-generated",
        status: "completed",
        model: { provider: "fal.ai", id: "fal-ai/wan/v2.7/reference-to-video" },
        references: {
          mediaIds: ["provider-e2e-reference-image", "provider-e2e-reference-video"],
          referenceImageMediaRefs: ["provider-e2e-reference-image"],
          referenceVideoMediaRefs: ["provider-e2e-reference-video"],
          providerInputUrls: [
            "https://v3.fal.media/files/provider-e2e/reference-image.png",
            "https://v3.fal.media/files/provider-e2e/reference-video.mp4",
          ],
        },
        settings: {
          durationSeconds: 4,
          fps: 24,
          aspectRatio: "16:9",
          resolution: "720p",
          generateAudio: true,
        },
        outputs: [
          {
            mediaId: "provider-e2e-wan-reference-output",
            relativePath: "generated/provider-e2e-generated/fal-wan-reference-output.mp4",
            sourceUrl: "https://fal.media/generated/wan-reference-output.mp4",
            width: 1280,
            height: 720,
            durationSeconds: 4,
            fps: 24,
          },
        ],
      }),
    );
    writeFileSync(
      join(projectDir, "jobs", "provider-e2e-generated", "job.json"),
      JSON.stringify({
        id: "provider-e2e-generated",
        kind: "generate_media",
        status: "completed",
        updatedAt: "2026-07-05T12:00:00Z",
        providerRequest: {
          provider: "fal.ai",
          requestId: "fal-wan-reference-123",
          statusUrl: "https://queue.fal.run/fal-ai/wan/v2.7/reference-to-video/requests/fal-wan-reference-123/status",
          responseUrl: "https://queue.fal.run/fal-ai/wan/v2.7/reference-to-video/requests/fal-wan-reference-123",
          cancelUrl: "https://queue.fal.run/fal-ai/wan/v2.7/reference-to-video/requests/fal-wan-reference-123/cancel",
          submittedAt: "2026-07-05T12:00:00Z",
        },
      }),
    );
    const argsPath = join(tempDir, "cargo-args.txt");
    const cargoResponsePath = join(tempDir, "cargo-response.json");
    writeFileSync(
      cargoResponsePath,
      `${JSON.stringify({
        ok: true,
        scenario: "wan-reference-to-video",
        provider: "fal.ai",
        model: "fal-ai/wan/v2.7/reference-to-video",
        projectDir,
        artifactPath,
        outputCount: 1,
        jobStatus: "Completed",
        providerRequest: {
          provider: "fal.ai",
          requestId: "fal-wan-reference-123",
          statusUrl: "https://queue.fal.run/fal-ai/wan/v2.7/reference-to-video/requests/fal-wan-reference-123/status",
          responseUrl: "https://queue.fal.run/fal-ai/wan/v2.7/reference-to-video/requests/fal-wan-reference-123",
          cancelUrl: "https://queue.fal.run/fal-ai/wan/v2.7/reference-to-video/requests/fal-wan-reference-123/cancel",
          submittedAt: "2026-07-05T12:00:00Z",
        },
      })}\n`,
    );
    writeFileSync(
      fakeCargo,
      [
        "#!/usr/bin/env node",
        "const fs = require('node:fs');",
        "fs.writeFileSync(process.env.VIDEO_CREATER_FAKE_CARGO_ARGS, process.argv.slice(2).join(' ') + '\\n');",
        "process.stdout.write(fs.readFileSync(process.env.VIDEO_CREATER_FAKE_CARGO_RESPONSE, 'utf8'));",
      ].join("\n") + "\n",
      { mode: 0o755 },
    );
    const outDir = join(tempDir, "provider-out");
    const result = spawnSync(
      process.execPath,
      [
        join(repoRoot, "scripts/provider-e2e.mjs"),
        "--provider",
        "fal.ai",
        "--live",
        "--scenario",
        "wan-reference-to-video",
        "--out-dir",
        outDir,
      ],
      {
        cwd: repoRoot,
        encoding: "utf8",
        env: {
          ...process.env,
          PATH: `${tempDir}:${process.env.PATH ?? ""}`,
          VIDEO_CREATER_FAKE_CARGO_ARGS: argsPath,
          VIDEO_CREATER_FAKE_CARGO_RESPONSE: cargoResponsePath,
          VIDEO_CREATER_PROVIDER_E2E_ALLOW_SPEND: "1",
          FAL_KEY: "super-secret-fal-token",
        },
      },
    );

    expect(result.status).toBe(0);
    expect(result.stdout).not.toContain("super-secret-fal-token");
    expect(result.stderr).not.toContain("super-secret-fal-token");
    const cargoArgs = readFileSync(argsPath, "utf8");
    expect(cargoArgs).toContain("--provider fal.ai");
    expect(cargoArgs).toContain("--model fal-ai/wan/v2.7/reference-to-video");
    expect(cargoArgs).toContain("--scenario wan-reference-to-video");
    const report = JSON.parse(result.stdout) as {
      ok: boolean;
      provider: string;
      model: string;
      scenario: string;
      result: { scenario: string; outputCount: number; artifactPath: string };
    };
    expect(report).toEqual(
      expect.objectContaining({
        ok: true,
        provider: "fal.ai",
        model: "fal-ai/wan/v2.7/reference-to-video",
        scenario: "wan-reference-to-video",
        result: expect.objectContaining({
          scenario: "wan-reference-to-video",
          outputCount: 1,
          artifactPath,
        }),
      }),
    );
  });

  it("forwards the text-to-audio provider E2E scenario to the Rust binary", () => {
    const tempDir = mkdtempSync(join(tmpdir(), "video-creater-provider-audio-e2e-"));
    const fakeCargo = join(tempDir, "cargo");
    const projectDir = join(tempDir, "project");
    const artifactPath = join(projectDir, "generated", "provider-e2e-generated", "fal-audio-output.mp3");
    mkdirSync(join(projectDir, "generated", "provider-e2e-generated"), {
      recursive: true,
    });
    mkdirSync(join(projectDir, "media"), { recursive: true });
    mkdirSync(join(projectDir, "jobs", "provider-e2e-generated"), {
      recursive: true,
    });
    writeFileSync(artifactPath, "audio bytes");
    writeFileSync(
      join(projectDir, "video-creater.project.json"),
      JSON.stringify({
        schemaVersion: 1,
        id: "provider-e2e-project",
        files: { generated: "generated", media: "media/index.json" },
      }),
    );
    writeFileSync(
      join(projectDir, "media", "index.json"),
      JSON.stringify({
        schemaVersion: 1,
        folders: [],
        assets: [
          {
            id: "provider-e2e-audio-output",
            relativePath: "generated/provider-e2e-generated/fal-audio-output.mp3",
            kind: "generated",
            durationSeconds: 12,
            width: 1,
            height: 1,
            fps: 1,
          },
        ],
        analysis: [],
      }),
    );
    writeFileSync(
      join(projectDir, "generated", "index.json"),
      JSON.stringify({
        schemaVersion: 1,
        assets: [
          {
            assetId: "provider-e2e-generated",
            path: "generated/provider-e2e-generated/asset.json",
            status: "completed",
            modelProvider: "fal.ai",
            modelId: "bytedance/seed-audio-1.0",
            outputCount: 1,
          },
        ],
      }),
    );
    writeFileSync(
      join(projectDir, "generated", "provider-e2e-generated", "asset.json"),
      JSON.stringify({
        id: "provider-e2e-generated",
        status: "completed",
        model: { provider: "fal.ai", id: "bytedance/seed-audio-1.0" },
        outputs: [
          {
            mediaId: "provider-e2e-audio-output",
            relativePath: "generated/provider-e2e-generated/fal-audio-output.mp3",
            sourceUrl: "https://fal.media/generated/audio-output.mp3",
            width: 1,
            height: 1,
            durationSeconds: 12,
            fps: 1,
          },
        ],
      }),
    );
    writeFileSync(
      join(projectDir, "jobs", "provider-e2e-generated", "job.json"),
      JSON.stringify({
        id: "provider-e2e-generated",
        kind: "generate_media",
        status: "completed",
        updatedAt: "2026-07-05T12:00:00Z",
        providerRequest: {
          provider: "fal.ai",
          requestId: "fal-audio-123",
          statusUrl: "https://queue.fal.run/bytedance/seed-audio-1.0/requests/fal-audio-123/status",
          responseUrl: "https://queue.fal.run/bytedance/seed-audio-1.0/requests/fal-audio-123",
          cancelUrl: "https://queue.fal.run/bytedance/seed-audio-1.0/requests/fal-audio-123/cancel",
          submittedAt: "2026-07-05T12:00:00Z",
        },
      }),
    );
    const argsPath = join(tempDir, "cargo-args.txt");
    const cargoResponsePath = join(tempDir, "cargo-response.json");
    writeFileSync(
      cargoResponsePath,
      `${JSON.stringify({
        ok: true,
        scenario: "text-to-audio",
        provider: "fal.ai",
        model: "bytedance/seed-audio-1.0",
        projectDir,
        artifactPath,
        outputCount: 1,
        jobStatus: "Completed",
        providerRequest: {
          provider: "fal.ai",
          requestId: "fal-audio-123",
          statusUrl: "https://queue.fal.run/bytedance/seed-audio-1.0/requests/fal-audio-123/status",
          responseUrl: "https://queue.fal.run/bytedance/seed-audio-1.0/requests/fal-audio-123",
          cancelUrl: "https://queue.fal.run/bytedance/seed-audio-1.0/requests/fal-audio-123/cancel",
          submittedAt: "2026-07-05T12:00:00Z",
        },
      })}\n`,
    );
    writeFileSync(
      fakeCargo,
      [
        "#!/usr/bin/env node",
        "const fs = require('node:fs');",
        "fs.writeFileSync(process.env.VIDEO_CREATER_FAKE_CARGO_ARGS, process.argv.slice(2).join(' ') + '\\n');",
        "process.stdout.write(fs.readFileSync(process.env.VIDEO_CREATER_FAKE_CARGO_RESPONSE, 'utf8'));",
      ].join("\n") + "\n",
      { mode: 0o755 },
    );
    const outDir = join(tempDir, "provider-out");
    const result = spawnSync(
      process.execPath,
      [
        join(repoRoot, "scripts/provider-e2e.mjs"),
        "--provider",
        "fal.ai",
        "--live",
        "--scenario",
        "text-to-audio",
        "--out-dir",
        outDir,
      ],
      {
        cwd: repoRoot,
        encoding: "utf8",
        env: {
          ...process.env,
          PATH: `${tempDir}:${process.env.PATH ?? ""}`,
          VIDEO_CREATER_FAKE_CARGO_ARGS: argsPath,
          VIDEO_CREATER_FAKE_CARGO_RESPONSE: cargoResponsePath,
          VIDEO_CREATER_PROVIDER_E2E_ALLOW_SPEND: "1",
          FAL_KEY: "super-secret-fal-token",
        },
      },
    );

    expect(result.status).toBe(0);
    expect(result.stdout).not.toContain("super-secret-fal-token");
    expect(result.stderr).not.toContain("super-secret-fal-token");
    const cargoArgs = readFileSync(argsPath, "utf8");
    expect(cargoArgs).toContain("--provider fal.ai");
    expect(cargoArgs).toContain("--model bytedance/seed-audio-1.0");
    expect(cargoArgs).toContain("--scenario text-to-audio");
    const report = JSON.parse(result.stdout) as {
      ok: boolean;
      provider: string;
      model: string;
      scenario: string;
      result: { scenario: string; artifactPath: string; outputCount: number };
    };
    expect(report).toEqual(
      expect.objectContaining({
        ok: true,
        provider: "fal.ai",
        model: "bytedance/seed-audio-1.0",
        scenario: "text-to-audio",
        result: expect.objectContaining({
          scenario: "text-to-audio",
          artifactPath,
          outputCount: 1,
        }),
      }),
    );
  });

  it("forwards the replicate local file upload provider E2E scenario to the Rust binary", () => {
    const tempDir = mkdtempSync(join(tmpdir(), "video-creater-provider-upload-e2e-"));
    const fakeCargo = join(tempDir, "cargo");
    const projectDir = join(tempDir, "project");
    const artifactPath = join(projectDir, "media", "provider-e2e-source.png");
    mkdirSync(join(projectDir, "media"), { recursive: true });
    writeFileSync(artifactPath, "source pixels");
    const argsPath = join(tempDir, "cargo-args.txt");
    const cargoResponsePath = join(tempDir, "cargo-response.json");
    writeFileSync(
      cargoResponsePath,
      `${JSON.stringify({
        ok: true,
        scenario: "replicate-local-file-upload",
        provider: "replicate",
        model: "black-forest-labs/flux-schnell",
        projectDir,
        artifactPath,
        outputCount: 0,
        jobStatus: "Uploaded",
        providerFileUrl: "https://api.replicate.com/v1/files/file-123",
      })}\n`,
    );
    writeFileSync(
      fakeCargo,
      [
        "#!/usr/bin/env node",
        "const fs = require('node:fs');",
        "fs.writeFileSync(process.env.VIDEO_CREATER_FAKE_CARGO_ARGS, process.argv.slice(2).join(' ') + '\\n');",
        "process.stdout.write(fs.readFileSync(process.env.VIDEO_CREATER_FAKE_CARGO_RESPONSE, 'utf8'));",
      ].join("\n") + "\n",
      { mode: 0o755 },
    );
    const outDir = join(tempDir, "provider-out");
    const result = spawnSync(
      process.execPath,
      [
        join(repoRoot, "scripts/provider-e2e.mjs"),
        "--provider",
        "replicate",
        "--live",
        "--scenario",
        "replicate-local-file-upload",
        "--out-dir",
        outDir,
      ],
      {
        cwd: repoRoot,
        encoding: "utf8",
        env: {
          ...process.env,
          PATH: `${tempDir}:${process.env.PATH ?? ""}`,
          VIDEO_CREATER_FAKE_CARGO_ARGS: argsPath,
          VIDEO_CREATER_FAKE_CARGO_RESPONSE: cargoResponsePath,
          VIDEO_CREATER_PROVIDER_E2E_ALLOW_SPEND: "1",
          REPLICATE_API_TOKEN: "super-secret-replicate-token",
        },
      },
    );

    expect(result.status).toBe(0);
    expect(result.stdout).not.toContain("super-secret-replicate-token");
    expect(result.stderr).not.toContain("super-secret-replicate-token");
    const cargoArgs = readFileSync(argsPath, "utf8");
    expect(cargoArgs).toContain("--provider replicate");
    expect(cargoArgs).toContain("--model black-forest-labs/flux-schnell");
    expect(cargoArgs).toContain("--scenario replicate-local-file-upload");
    const report = JSON.parse(result.stdout) as {
      ok: boolean;
      provider: string;
      model: string;
      scenario: string;
      result: { scenario: string; outputCount: number; jobStatus: string; providerFileUrl: string };
    };
    expect(report).toEqual(
      expect.objectContaining({
        ok: true,
        provider: "replicate",
        model: "black-forest-labs/flux-schnell",
        scenario: "replicate-local-file-upload",
        result: expect.objectContaining({
          scenario: "replicate-local-file-upload",
          outputCount: 0,
          jobStatus: "Uploaded",
          providerFileUrl: "https://api.replicate.com/v1/files/file-123",
        }),
      }),
    );
  }, 15_000);

  it("uses a non-placeholder source image for the live local image upscale scenario", () => {
    const source = readFileSync(
      join(repoRoot, "src-tauri/src/bin/video-creater-provider-e2e.rs"),
      "utf8",
    );

    expect(source).toContain("PROVIDER_E2E_SOURCE_WIDTH");
    expect(source).toContain("PROVIDER_E2E_SOURCE_HEIGHT");
    expect(source).not.toContain("width: Some(1)");
    expect(source).not.toContain("height: Some(1)");
    expect(source).not.toContain("fn provider_e2e_source_png() -> &'static [u8]");
  });

  it("replays a retained provider fixture without credentials or cargo", () => {
    const tempDir = mkdtempSync(join(tmpdir(), "video-creater-provider-fixture-"));
    const fixtureReport = writeProviderPolicyFixture({
      rootDir: tempDir,
      provider: "replicate",
      model: "black-forest-labs/flux-schnell",
      requestId: "replicate-fixture-1",
    });
    const fixtureDir = join(fixtureReport, "..");
    const outDir = join(tempDir, "replayed-provider");
    const fakeCargo = join(tempDir, "cargo");
    writeFileSync(
      fakeCargo,
      "#!/bin/sh\necho cargo should not run >&2\nexit 42\n",
      { mode: 0o755 },
    );

    const result = spawnSync(
      process.execPath,
      [
        join(repoRoot, "scripts/provider-e2e.mjs"),
        "--provider",
        "replicate",
        "--live",
        "--model",
        "black-forest-labs/flux-schnell",
        "--out-dir",
        outDir,
        "--replay-fixture",
        fixtureDir,
      ],
      {
        cwd: repoRoot,
        encoding: "utf8",
        env: {
          ...process.env,
          PATH: `${tempDir}:${process.env.PATH ?? ""}`,
          REPLICATE_API_TOKEN: "",
        },
      },
    );

    expect(result.stderr).not.toContain("cargo should not run");
    expect(result.status).toBe(0);
    const report = JSON.parse(result.stdout) as {
      ok: boolean;
      provider: string;
      outDir: string;
      reportPath: string;
      fixtureReplay: boolean;
      result: { projectDir: string; artifactPath: string };
    };
    expect(report).toEqual(
      expect.objectContaining({
        ok: true,
        provider: "replicate",
        outDir,
        reportPath: "provider-e2e-report.json",
        fixtureReplay: true,
      }),
    );
    expect(report.result.projectDir.startsWith(outDir)).toBe(true);
    expect(report.result.artifactPath.startsWith(outDir)).toBe(true);
    expect(readFileSync(report.result.artifactPath, "utf8")).toBe(
      "generated pixels",
    );
    expect(
      JSON.parse(readFileSync(join(outDir, "provider-e2e-report.json"), "utf8")),
    ).toEqual(report);
  });

  it("replays a fixture whose split project root is the fixture root", () => {
    const tempDir = mkdtempSync(join(tmpdir(), "video-creater-provider-root-fixture-"));
    const fixtureDir = join(tempDir, "replicate-fixture");
    const reportPath = writeProviderPolicyFixture({
      rootDir: tempDir,
      provider: "replicate",
      model: "black-forest-labs/flux-schnell",
      requestId: "replicate-root-fixture-1",
    });
    rmSync(fixtureDir, { recursive: true, force: true });
    mkdirSync(fixtureDir, { recursive: true });
    const originalFixtureDir = join(reportPath, "..");
    cpSync(join(originalFixtureDir, "project"), fixtureDir, { recursive: true });
    const report = JSON.parse(
      readFileSync(reportPath, "utf8"),
    ) as { result: { projectDir: string; artifactPath: string } };
    report.result.projectDir = fixtureDir;
    report.result.artifactPath = join(
      fixtureDir,
      "generated",
      "replicate-output.png",
    );
    writeFileSync(
      join(fixtureDir, "provider-e2e-report.json"),
      `${JSON.stringify(report, null, 2)}\n`,
    );

    const outDir = join(tempDir, "replayed-root-fixture");
    const result = spawnSync(
      process.execPath,
      [
        join(repoRoot, "scripts/provider-e2e.mjs"),
        "--provider",
        "replicate",
        "--live",
        "--model",
        "black-forest-labs/flux-schnell",
        "--out-dir",
        outDir,
        "--replay-fixture",
        fixtureDir,
      ],
      { cwd: repoRoot, encoding: "utf8" },
    );

    expect(result.status).toBe(0);
    const replayed = JSON.parse(result.stdout) as {
      result: { projectDir: string; artifactPath: string };
    };
    expect(replayed.result.projectDir).toBe(outDir);
    expect(replayed.result.artifactPath).toBe(
      join(outDir, "generated", "replicate-output.png"),
    );
  });

  it("records a successful provider run as a replayable fixture", () => {
    const tempDir = mkdtempSync(join(tmpdir(), "video-creater-provider-record-fixture-"));
    const fakeCargo = join(tempDir, "cargo");
    const projectDir = join(tempDir, "live-provider-out");
    const artifactPath = join(projectDir, "generated", "provider-e2e-output.png");
    mkdirSync(join(projectDir, "generated", "provider-e2e-generated"), {
      recursive: true,
    });
    mkdirSync(join(projectDir, "media"), { recursive: true });
    mkdirSync(join(projectDir, "jobs", "provider-e2e-generated"), {
      recursive: true,
    });
    writeFileSync(artifactPath, "generated pixels");
    writeFileSync(
      join(projectDir, "video-creater.project.json"),
      JSON.stringify({
        schemaVersion: 1,
        id: "provider-e2e-project",
        files: { generated: "generated", media: "media/index.json" },
      }),
    );
    writeFileSync(
      join(projectDir, "media", "index.json"),
      JSON.stringify({
        schemaVersion: 1,
        folders: [],
        assets: [
          {
            id: "provider-e2e-output",
            relativePath: "generated/provider-e2e-output.png",
            kind: "generated",
            durationSeconds: 0,
            width: 1024,
            height: 768,
            fps: 0,
          },
        ],
        analysis: [],
      }),
    );
    writeFileSync(
      join(projectDir, "generated", "index.json"),
      JSON.stringify({
        schemaVersion: 1,
        assets: [
          {
            assetId: "provider-e2e-generated",
            path: "generated/provider-e2e-generated/asset.json",
            status: "completed",
            modelProvider: "replicate",
            modelId: "black-forest-labs/flux-schnell",
            outputCount: 1,
          },
        ],
      }),
    );
    writeFileSync(
      join(projectDir, "generated", "provider-e2e-generated", "asset.json"),
      JSON.stringify({
        id: "provider-e2e-generated",
        status: "completed",
        model: { provider: "replicate", id: "black-forest-labs/flux-schnell" },
        outputs: [
          {
            mediaId: "provider-e2e-output",
            relativePath: "generated/provider-e2e-output.png",
            sourceUrl: "https://provider.example/generated-output.png",
            width: 1024,
            height: 768,
            durationSeconds: 0,
            fps: 0,
          },
        ],
      }),
    );
    writeFileSync(
      join(projectDir, "jobs", "provider-e2e-generated", "job.json"),
      JSON.stringify({
        id: "provider-e2e-generated",
        kind: "generate_media",
        status: "completed",
        updatedAt: "2026-07-05T12:00:00Z",
        providerRequest: {
          provider: "replicate",
          requestId: "pred-123",
          statusUrl: "https://api.replicate.com/v1/predictions/pred-123",
          responseUrl: "https://api.replicate.com/v1/predictions/pred-123",
          cancelUrl: "https://api.replicate.com/v1/predictions/pred-123/cancel",
          submittedAt: "2026-07-05T12:00:00Z",
        },
      }),
    );
    writeFileSync(
      fakeCargo,
      [
        "#!/bin/sh",
        `printf '%s\\n' '{"ok":true,"provider":"replicate","model":"black-forest-labs/flux-schnell","projectDir":"${projectDir}","artifactPath":"${artifactPath}","outputCount":1,"jobStatus":"Completed","providerRequest":{"provider":"replicate","requestId":"pred-123","statusUrl":"https://api.replicate.com/v1/predictions/pred-123","responseUrl":"https://api.replicate.com/v1/predictions/pred-123","cancelUrl":"https://api.replicate.com/v1/predictions/pred-123/cancel","submittedAt":"2026-07-05T12:00:00Z"}}'`,
      ].join("\n"),
      { mode: 0o755 },
    );

    const fixtureDir = join(tempDir, "fixtures", "provider-e2e", "replicate");
    const result = spawnSync(
      process.execPath,
      [
        join(repoRoot, "scripts/provider-e2e.mjs"),
        "--provider",
        "replicate",
        "--live",
        "--model",
        "black-forest-labs/flux-schnell",
        "--out-dir",
        projectDir,
        "--live",
        "--record-fixture",
        fixtureDir,
      ],
      {
        cwd: repoRoot,
        encoding: "utf8",
        env: {
          ...process.env,
          PATH: `${tempDir}:${process.env.PATH ?? ""}`,
          REPLICATE_API_TOKEN: "super-secret-replicate-token",
          VIDEO_CREATER_PROVIDER_E2E_ALLOW_SPEND: "1",
        },
      },
    );

    expect(result.status).toBe(0);
    expect(readFileSync(join(fixtureDir, "provider-e2e-report.json"), "utf8"))
      .not.toContain("super-secret-replicate-token");
    expect(readFileSync(join(fixtureDir, "generated", "provider-e2e-output.png"), "utf8"))
      .toBe("generated pixels");
  });

  it("fails closed when the provider E2E binary does not report provider request metadata", () => {
    const tempDir = mkdtempSync(join(tmpdir(), "video-creater-provider-e2e-"));
    const fakeCargo = join(tempDir, "cargo");
    const projectDir = join(tempDir, "project");
    const artifactPath = join(projectDir, "generated", "provider-e2e-output.png");
    mkdirSync(join(projectDir, "generated", "provider-e2e-generated"), {
      recursive: true,
    });
    mkdirSync(join(projectDir, "media"));
    writeFileSync(artifactPath, "generated pixels");
    writeFileSync(
      join(projectDir, "video-creater.project.json"),
      JSON.stringify({
        schemaVersion: 1,
        id: "provider-e2e-project",
        files: { generated: "generated", media: "media/index.json" },
      }),
    );
    writeFileSync(
      join(projectDir, "media", "index.json"),
      JSON.stringify({
        schemaVersion: 1,
        folders: [],
        assets: [
          {
            id: "provider-e2e-output",
            relativePath: "generated/provider-e2e-output.png",
            kind: "generated",
            durationSeconds: 0,
            width: 1024,
            height: 768,
            fps: 0,
          },
        ],
        analysis: [],
      }),
    );
    writeFileSync(
      join(projectDir, "generated", "index.json"),
      JSON.stringify({
        schemaVersion: 1,
        assets: [
          {
            assetId: "provider-e2e-generated",
            path: "generated/provider-e2e-generated/asset.json",
            status: "completed",
            modelProvider: "replicate",
            modelId: "black-forest-labs/flux-schnell",
            outputCount: 1,
          },
        ],
      }),
    );
    writeFileSync(
      join(projectDir, "generated", "provider-e2e-generated", "asset.json"),
      JSON.stringify({
        id: "provider-e2e-generated",
        status: "completed",
        model: {
          provider: "replicate",
          id: "black-forest-labs/flux-schnell",
        },
        outputs: [
          {
            mediaId: "provider-e2e-output",
            relativePath: "generated/provider-e2e-output.png",
            sourceUrl: "https://provider.example/generated-output.png",
            width: 1024,
            height: 768,
            durationSeconds: 0,
            fps: 0,
          },
        ],
      }),
    );
    writeFileSync(
      fakeCargo,
      [
        "#!/bin/sh",
        `printf '%s\\n' '{"ok":true,"provider":"replicate","model":"black-forest-labs/flux-schnell","projectDir":"${projectDir}","artifactPath":"${artifactPath}","outputCount":1,"jobStatus":"Completed"}'`,
      ].join("\n"),
      { mode: 0o755 },
    );

    const outDir = join(tempDir, "provider-out");
    const result = spawnSync(
      process.execPath,
      [
        join(repoRoot, "scripts/provider-e2e.mjs"),
        "--provider",
        "replicate",
        "--live",
        "--out-dir",
        outDir,
      ],
      {
        cwd: repoRoot,
        encoding: "utf8",
        env: {
          ...process.env,
          PATH: `${tempDir}:${process.env.PATH ?? ""}`,
          REPLICATE_API_TOKEN: "super-secret-replicate-token",
          VIDEO_CREATER_PROVIDER_E2E_ALLOW_SPEND: "1",
        },
      },
    );

    expect(result.status).toBe(1);
    const report = JSON.parse(result.stdout) as {
      ok: boolean;
      error: string;
      result: { projectDir: string };
    };
    expect(report).toEqual(
      expect.objectContaining({
        ok: false,
        error: "Provider E2E binary did not report provider request metadata",
        result: expect.objectContaining({ projectDir }),
      }),
    );
  });

  it("fails closed when the provider E2E split project job request metadata does not match the child report", () => {
    const tempDir = mkdtempSync(join(tmpdir(), "video-creater-provider-e2e-"));
    const fakeCargo = join(tempDir, "cargo");
    const projectDir = join(tempDir, "project");
    const artifactPath = join(projectDir, "generated", "provider-e2e-output.png");
    mkdirSync(join(projectDir, "generated", "provider-e2e-generated"), {
      recursive: true,
    });
    mkdirSync(join(projectDir, "media"));
    mkdirSync(join(projectDir, "jobs", "provider-e2e-generated"), {
      recursive: true,
    });
    writeFileSync(artifactPath, "generated pixels");
    writeFileSync(
      join(projectDir, "video-creater.project.json"),
      JSON.stringify({
        schemaVersion: 1,
        id: "provider-e2e-project",
        files: { generated: "generated", media: "media/index.json" },
      }),
    );
    writeFileSync(
      join(projectDir, "media", "index.json"),
      JSON.stringify({
        schemaVersion: 1,
        folders: [],
        assets: [
          {
            id: "provider-e2e-output",
            relativePath: "generated/provider-e2e-output.png",
            kind: "generated",
            durationSeconds: 0,
            width: 1024,
            height: 768,
            fps: 0,
          },
        ],
        analysis: [],
      }),
    );
    writeFileSync(
      join(projectDir, "generated", "index.json"),
      JSON.stringify({
        schemaVersion: 1,
        assets: [
          {
            assetId: "provider-e2e-generated",
            path: "generated/provider-e2e-generated/asset.json",
            status: "completed",
            modelProvider: "replicate",
            modelId: "black-forest-labs/flux-schnell",
            outputCount: 1,
          },
        ],
      }),
    );
    writeFileSync(
      join(projectDir, "generated", "provider-e2e-generated", "asset.json"),
      JSON.stringify({
        id: "provider-e2e-generated",
        status: "completed",
        model: {
          provider: "replicate",
          id: "black-forest-labs/flux-schnell",
        },
        outputs: [
          {
            mediaId: "provider-e2e-output",
            relativePath: "generated/provider-e2e-output.png",
            sourceUrl: "https://provider.example/generated-output.png",
            width: 1024,
            height: 768,
            durationSeconds: 0,
            fps: 0,
          },
        ],
      }),
    );
    writeFileSync(
      join(projectDir, "jobs", "provider-e2e-generated", "job.json"),
      JSON.stringify({
        id: "provider-e2e-generated",
        kind: "generate_media",
        status: "completed",
        updatedAt: "2026-07-05T12:00:00Z",
        providerRequest: {
          provider: "replicate",
          requestId: "different-prediction",
          statusUrl: "https://api.replicate.com/v1/predictions/different-prediction",
          responseUrl: "https://api.replicate.com/v1/predictions/different-prediction",
          cancelUrl: "https://api.replicate.com/v1/predictions/different-prediction/cancel",
          submittedAt: "2026-07-05T12:00:00Z",
        },
      }),
    );
    writeFileSync(
      fakeCargo,
      [
        "#!/bin/sh",
        `printf '%s\\n' '{"ok":true,"provider":"replicate","model":"black-forest-labs/flux-schnell","projectDir":"${projectDir}","artifactPath":"${artifactPath}","outputCount":1,"jobStatus":"Completed","providerRequest":{"provider":"replicate","requestId":"pred-123","statusUrl":"https://api.replicate.com/v1/predictions/pred-123","responseUrl":"https://api.replicate.com/v1/predictions/pred-123","cancelUrl":"https://api.replicate.com/v1/predictions/pred-123/cancel","submittedAt":"2026-07-05T12:00:00Z"}}'`,
      ].join("\n"),
      { mode: 0o755 },
    );

    const outDir = join(tempDir, "provider-out");
    const result = spawnSync(
      process.execPath,
      [
        join(repoRoot, "scripts/provider-e2e.mjs"),
        "--provider",
        "replicate",
        "--live",
        "--out-dir",
        outDir,
      ],
      {
        cwd: repoRoot,
        encoding: "utf8",
        env: {
          ...process.env,
          PATH: `${tempDir}:${process.env.PATH ?? ""}`,
          REPLICATE_API_TOKEN: "super-secret-replicate-token",
          VIDEO_CREATER_PROVIDER_E2E_ALLOW_SPEND: "1",
        },
      },
    );

    expect(result.status).toBe(1);
    const report = JSON.parse(result.stdout) as {
      ok: boolean;
      error: string;
      result: { projectDir: string };
    };
    expect(report).toEqual(
      expect.objectContaining({
        ok: false,
        error:
          "Provider E2E split project job provider request does not match child report",
        result: expect.objectContaining({ projectDir }),
      }),
    );
  });

  it("fails closed when the provider E2E split project job is not completed", () => {
    const tempDir = mkdtempSync(join(tmpdir(), "video-creater-provider-e2e-"));
    const fakeCargo = join(tempDir, "cargo");
    const projectDir = join(tempDir, "project");
    const artifactPath = join(projectDir, "generated", "provider-e2e-output.png");
    mkdirSync(join(projectDir, "generated", "provider-e2e-generated"), {
      recursive: true,
    });
    mkdirSync(join(projectDir, "media"));
    mkdirSync(join(projectDir, "jobs", "provider-e2e-generated"), {
      recursive: true,
    });
    writeFileSync(artifactPath, "generated pixels");
    writeFileSync(
      join(projectDir, "video-creater.project.json"),
      JSON.stringify({
        schemaVersion: 1,
        id: "provider-e2e-project",
        files: { generated: "generated", media: "media/index.json" },
      }),
    );
    writeFileSync(
      join(projectDir, "media", "index.json"),
      JSON.stringify({
        schemaVersion: 1,
        folders: [],
        assets: [
          {
            id: "provider-e2e-output",
            relativePath: "generated/provider-e2e-output.png",
            kind: "generated",
            durationSeconds: 0,
            width: 1024,
            height: 768,
            fps: 0,
          },
        ],
        analysis: [],
      }),
    );
    writeFileSync(
      join(projectDir, "generated", "index.json"),
      JSON.stringify({
        schemaVersion: 1,
        assets: [
          {
            assetId: "provider-e2e-generated",
            path: "generated/provider-e2e-generated/asset.json",
            status: "completed",
            modelProvider: "replicate",
            modelId: "black-forest-labs/flux-schnell",
            outputCount: 1,
          },
        ],
      }),
    );
    writeFileSync(
      join(projectDir, "generated", "provider-e2e-generated", "asset.json"),
      JSON.stringify({
        id: "provider-e2e-generated",
        status: "completed",
        model: {
          provider: "replicate",
          id: "black-forest-labs/flux-schnell",
        },
        outputs: [
          {
            mediaId: "provider-e2e-output",
            relativePath: "generated/provider-e2e-output.png",
            sourceUrl: "https://provider.example/generated-output.png",
            width: 1024,
            height: 768,
            durationSeconds: 0,
            fps: 0,
          },
        ],
      }),
    );
    writeFileSync(
      join(projectDir, "jobs", "provider-e2e-generated", "job.json"),
      JSON.stringify({
        id: "provider-e2e-generated",
        kind: "generate_media",
        status: "running",
        updatedAt: "2026-07-05T12:00:00Z",
        providerRequest: {
          provider: "replicate",
          requestId: "pred-123",
          statusUrl: "https://api.replicate.com/v1/predictions/pred-123",
          responseUrl: "https://api.replicate.com/v1/predictions/pred-123",
          cancelUrl: "https://api.replicate.com/v1/predictions/pred-123/cancel",
          submittedAt: "2026-07-05T12:00:00Z",
        },
      }),
    );
    writeFileSync(
      fakeCargo,
      [
        "#!/bin/sh",
        `printf '%s\\n' '{"ok":true,"provider":"replicate","model":"black-forest-labs/flux-schnell","projectDir":"${projectDir}","artifactPath":"${artifactPath}","outputCount":1,"jobStatus":"Completed","providerRequest":{"provider":"replicate","requestId":"pred-123","statusUrl":"https://api.replicate.com/v1/predictions/pred-123","responseUrl":"https://api.replicate.com/v1/predictions/pred-123","cancelUrl":"https://api.replicate.com/v1/predictions/pred-123/cancel","submittedAt":"2026-07-05T12:00:00Z"}}'`,
      ].join("\n"),
      { mode: 0o755 },
    );

    const outDir = join(tempDir, "provider-out");
    const result = spawnSync(
      process.execPath,
      [
        join(repoRoot, "scripts/provider-e2e.mjs"),
        "--provider",
        "replicate",
        "--live",
        "--out-dir",
        outDir,
      ],
      {
        cwd: repoRoot,
        encoding: "utf8",
        env: {
          ...process.env,
          PATH: `${tempDir}:${process.env.PATH ?? ""}`,
          REPLICATE_API_TOKEN: "super-secret-replicate-token",
          VIDEO_CREATER_PROVIDER_E2E_ALLOW_SPEND: "1",
        },
      },
    );

    expect(result.status).toBe(1);
    const report = JSON.parse(result.stdout) as {
      ok: boolean;
      error: string;
      result: { projectDir: string };
    };
    expect(report).toEqual(
      expect.objectContaining({
        ok: false,
        error:
          "Provider E2E split project job is not completed: provider-e2e-generated",
        result: expect.objectContaining({ projectDir }),
      }),
    );
  });

  it("fails closed when the provider E2E binary does not emit JSON", () => {
    const tempDir = mkdtempSync(join(tmpdir(), "video-creater-provider-e2e-"));
    const fakeCargo = join(tempDir, "cargo");
    writeFileSync(
      fakeCargo,
      [
        "#!/bin/sh",
        "printf '%s\\n' 'completed without a structured report'",
      ].join("\n"),
      { mode: 0o755 },
    );

    const outDir = join(tempDir, "provider-out");
    const result = spawnSync(
      process.execPath,
      [
        join(repoRoot, "scripts/provider-e2e.mjs"),
        "--provider",
        "replicate",
        "--live",
        "--out-dir",
        outDir,
      ],
      {
        cwd: repoRoot,
        encoding: "utf8",
        env: {
          ...process.env,
          PATH: `${tempDir}:${process.env.PATH ?? ""}`,
          REPLICATE_API_TOKEN: "super-secret-replicate-token",
          VIDEO_CREATER_PROVIDER_E2E_ALLOW_SPEND: "1",
        },
      },
    );

    expect(result.status).toBe(1);
    expect(result.stdout).not.toContain("super-secret-replicate-token");
    expect(result.stderr).not.toContain("super-secret-replicate-token");
    const report = JSON.parse(result.stdout) as {
      ok: boolean;
      result: unknown;
      error: string;
    };
    expect(report).toEqual(
      expect.objectContaining({
        ok: false,
        result: null,
        error: "Provider E2E binary did not emit a JSON report",
      }),
    );
    const writtenReport = JSON.parse(
      readFileSync(join(outDir, "provider-e2e-report.json"), "utf8"),
    ) as typeof report;
    expect(writtenReport).toEqual(report);
  });

  it("fails closed when the provider E2E binary reports a missing artifact", () => {
    const tempDir = mkdtempSync(join(tmpdir(), "video-creater-provider-e2e-"));
    const fakeCargo = join(tempDir, "cargo");
    const missingArtifactPath = join(tempDir, "generated", "missing-output.png");
    writeFileSync(
      fakeCargo,
      [
        "#!/bin/sh",
        `printf '%s\\n' '{"ok":true,"provider":"replicate","model":"black-forest-labs/flux-schnell","artifactPath":"${missingArtifactPath}"}'`,
      ].join("\n"),
      { mode: 0o755 },
    );

    const outDir = join(tempDir, "provider-out");
    const result = spawnSync(
      process.execPath,
      [
        join(repoRoot, "scripts/provider-e2e.mjs"),
        "--provider",
        "replicate",
        "--live",
        "--out-dir",
        outDir,
      ],
      {
        cwd: repoRoot,
        encoding: "utf8",
        env: {
          ...process.env,
          PATH: `${tempDir}:${process.env.PATH ?? ""}`,
          REPLICATE_API_TOKEN: "super-secret-replicate-token",
          VIDEO_CREATER_PROVIDER_E2E_ALLOW_SPEND: "1",
        },
      },
    );

    expect(result.status).toBe(1);
    expect(result.stdout).not.toContain("super-secret-replicate-token");
    expect(result.stderr).not.toContain("super-secret-replicate-token");
    const report = JSON.parse(result.stdout) as {
      ok: boolean;
      error: string;
      result: { artifactPath: string };
    };
    expect(report).toEqual(
      expect.objectContaining({
        ok: false,
        error: `Provider E2E artifact is missing: ${missingArtifactPath}`,
        result: expect.objectContaining({ artifactPath: missingArtifactPath }),
      }),
    );
    const writtenReport = JSON.parse(
      readFileSync(join(outDir, "provider-e2e-report.json"), "utf8"),
    ) as typeof report;
    expect(writtenReport).toEqual(report);
  });

  it("fails closed when the provider E2E binary did not attach an output", () => {
    const tempDir = mkdtempSync(join(tmpdir(), "video-creater-provider-e2e-"));
    const fakeCargo = join(tempDir, "cargo");
    const artifactPath = join(tempDir, "generated", "replicate-output.png");
    mkdirSync(join(tempDir, "generated"));
    writeFileSync(artifactPath, "generated pixels");
    writeFileSync(
      fakeCargo,
      [
        "#!/bin/sh",
        `printf '%s\\n' '{"ok":true,"provider":"replicate","model":"black-forest-labs/flux-schnell","artifactPath":"${artifactPath}","outputCount":0,"jobStatus":"Completed"}'`,
      ].join("\n"),
      { mode: 0o755 },
    );

    const outDir = join(tempDir, "provider-out");
    const result = spawnSync(
      process.execPath,
      [
        join(repoRoot, "scripts/provider-e2e.mjs"),
        "--provider",
        "replicate",
        "--live",
        "--out-dir",
        outDir,
      ],
      {
        cwd: repoRoot,
        encoding: "utf8",
        env: {
          ...process.env,
          PATH: `${tempDir}:${process.env.PATH ?? ""}`,
          REPLICATE_API_TOKEN: "super-secret-replicate-token",
          VIDEO_CREATER_PROVIDER_E2E_ALLOW_SPEND: "1",
        },
      },
    );

    expect(result.status).toBe(1);
    const report = JSON.parse(result.stdout) as {
      ok: boolean;
      error: string;
      result: { outputCount: number };
    };
    expect(report).toEqual(
      expect.objectContaining({
        ok: false,
        error: "Provider E2E did not attach any generated outputs",
        result: expect.objectContaining({ outputCount: 0 }),
      }),
    );
  });

  it("fails closed when the provider E2E job did not complete", () => {
    const tempDir = mkdtempSync(join(tmpdir(), "video-creater-provider-e2e-"));
    const fakeCargo = join(tempDir, "cargo");
    const artifactPath = join(tempDir, "generated", "replicate-output.png");
    mkdirSync(join(tempDir, "generated"));
    writeFileSync(artifactPath, "generated pixels");
    writeFileSync(
      fakeCargo,
      [
        "#!/bin/sh",
        `printf '%s\\n' '{"ok":true,"provider":"replicate","model":"black-forest-labs/flux-schnell","artifactPath":"${artifactPath}","outputCount":1,"jobStatus":"Running"}'`,
      ].join("\n"),
      { mode: 0o755 },
    );

    const outDir = join(tempDir, "provider-out");
    const result = spawnSync(
      process.execPath,
      [
        join(repoRoot, "scripts/provider-e2e.mjs"),
        "--provider",
        "replicate",
        "--live",
        "--out-dir",
        outDir,
      ],
      {
        cwd: repoRoot,
        encoding: "utf8",
        env: {
          ...process.env,
          PATH: `${tempDir}:${process.env.PATH ?? ""}`,
          REPLICATE_API_TOKEN: "super-secret-replicate-token",
          VIDEO_CREATER_PROVIDER_E2E_ALLOW_SPEND: "1",
        },
      },
    );

    expect(result.status).toBe(1);
    const report = JSON.parse(result.stdout) as {
      ok: boolean;
      error: string;
      result: { jobStatus: string };
    };
    expect(report).toEqual(
      expect.objectContaining({
        ok: false,
        error: "Provider E2E job did not complete: Running",
        result: expect.objectContaining({ jobStatus: "Running" }),
      }),
    );
  });

  it("fails closed when the provider E2E split project manifest is missing", () => {
    const tempDir = mkdtempSync(join(tmpdir(), "video-creater-provider-e2e-"));
    const fakeCargo = join(tempDir, "cargo");
    const projectDir = join(tempDir, "project");
    const artifactPath = join(tempDir, "generated", "replicate-output.png");
    mkdirSync(join(tempDir, "generated"));
    mkdirSync(projectDir);
    writeFileSync(artifactPath, "generated pixels");
    writeFileSync(
      fakeCargo,
      [
        "#!/bin/sh",
        `printf '%s\\n' '{"ok":true,"provider":"replicate","model":"black-forest-labs/flux-schnell","projectDir":"${projectDir}","artifactPath":"${artifactPath}","outputCount":1,"jobStatus":"Completed"}'`,
      ].join("\n"),
      { mode: 0o755 },
    );

    const outDir = join(tempDir, "provider-out");
    const result = spawnSync(
      process.execPath,
      [
        join(repoRoot, "scripts/provider-e2e.mjs"),
        "--provider",
        "replicate",
        "--live",
        "--out-dir",
        outDir,
      ],
      {
        cwd: repoRoot,
        encoding: "utf8",
        env: {
          ...process.env,
          PATH: `${tempDir}:${process.env.PATH ?? ""}`,
          REPLICATE_API_TOKEN: "super-secret-replicate-token",
          VIDEO_CREATER_PROVIDER_E2E_ALLOW_SPEND: "1",
        },
      },
    );

    expect(result.status).toBe(1);
    const report = JSON.parse(result.stdout) as {
      ok: boolean;
      error: string;
      result: { projectDir: string };
    };
    expect(report).toEqual(
      expect.objectContaining({
        ok: false,
        error: `Provider E2E split project manifest is missing: ${join(projectDir, "video-creater.project.json")}`,
        result: expect.objectContaining({ projectDir }),
      }),
    );
  });

  it("fails closed when the provider E2E split project manifest is not parseable", () => {
    const tempDir = mkdtempSync(join(tmpdir(), "video-creater-provider-e2e-"));
    const fakeCargo = join(tempDir, "cargo");
    const projectDir = join(tempDir, "project");
    const artifactPath = join(tempDir, "generated", "replicate-output.png");
    mkdirSync(join(tempDir, "generated"));
    mkdirSync(projectDir);
    writeFileSync(artifactPath, "generated pixels");
    writeFileSync(join(projectDir, "video-creater.project.json"), "{not-json");
    writeFileSync(
      fakeCargo,
      [
        "#!/bin/sh",
        `printf '%s\\n' '{"ok":true,"provider":"replicate","model":"black-forest-labs/flux-schnell","projectDir":"${projectDir}","artifactPath":"${artifactPath}","outputCount":1,"jobStatus":"Completed"}'`,
      ].join("\n"),
      { mode: 0o755 },
    );

    const outDir = join(tempDir, "provider-out");
    const result = spawnSync(
      process.execPath,
      [
        join(repoRoot, "scripts/provider-e2e.mjs"),
        "--provider",
        "replicate",
        "--live",
        "--out-dir",
        outDir,
      ],
      {
        cwd: repoRoot,
        encoding: "utf8",
        env: {
          ...process.env,
          PATH: `${tempDir}:${process.env.PATH ?? ""}`,
          REPLICATE_API_TOKEN: "super-secret-replicate-token",
          VIDEO_CREATER_PROVIDER_E2E_ALLOW_SPEND: "1",
        },
      },
    );

    expect(result.status).toBe(1);
    const report = JSON.parse(result.stdout) as {
      ok: boolean;
      error: string;
      result: { projectDir: string };
    };
    expect(report).toEqual(
      expect.objectContaining({
        ok: false,
        error: `Provider E2E split project manifest is not valid JSON: ${join(projectDir, "video-creater.project.json")}`,
        result: expect.objectContaining({ projectDir }),
      }),
    );
  });

  it("fails closed when the provider E2E generated asset index is missing", () => {
    const tempDir = mkdtempSync(join(tmpdir(), "video-creater-provider-e2e-"));
    const fakeCargo = join(tempDir, "cargo");
    const projectDir = join(tempDir, "project");
    const artifactPath = join(tempDir, "generated", "replicate-output.png");
    mkdirSync(join(tempDir, "generated"));
    mkdirSync(projectDir);
    writeFileSync(artifactPath, "generated pixels");
    writeFileSync(
      join(projectDir, "video-creater.project.json"),
      JSON.stringify({
        schemaVersion: 1,
        id: "provider-e2e-project",
        files: { generated: "generated" },
      }),
    );
    writeFileSync(
      fakeCargo,
      [
        "#!/bin/sh",
        `printf '%s\\n' '{"ok":true,"provider":"replicate","model":"black-forest-labs/flux-schnell","projectDir":"${projectDir}","artifactPath":"${artifactPath}","outputCount":1,"jobStatus":"Completed"}'`,
      ].join("\n"),
      { mode: 0o755 },
    );

    const outDir = join(tempDir, "provider-out");
    const result = spawnSync(
      process.execPath,
      [
        join(repoRoot, "scripts/provider-e2e.mjs"),
        "--provider",
        "replicate",
        "--live",
        "--out-dir",
        outDir,
      ],
      {
        cwd: repoRoot,
        encoding: "utf8",
        env: {
          ...process.env,
          PATH: `${tempDir}:${process.env.PATH ?? ""}`,
          REPLICATE_API_TOKEN: "super-secret-replicate-token",
          VIDEO_CREATER_PROVIDER_E2E_ALLOW_SPEND: "1",
        },
      },
    );

    expect(result.status).toBe(1);
    const report = JSON.parse(result.stdout) as {
      ok: boolean;
      error: string;
      result: { projectDir: string };
    };
    expect(report).toEqual(
      expect.objectContaining({
        ok: false,
        error: `Provider E2E generated asset index is missing: ${join(projectDir, "generated", "index.json")}`,
        result: expect.objectContaining({ projectDir }),
      }),
    );
  });

  it("fails closed when the provider E2E generated asset index is not parseable", () => {
    const tempDir = mkdtempSync(join(tmpdir(), "video-creater-provider-e2e-"));
    const fakeCargo = join(tempDir, "cargo");
    const artifactPath = join(tempDir, "generated", "replicate-output.png");
    const projectDir = join(tempDir, "project");
    mkdirSync(join(tempDir, "generated"));
    mkdirSync(join(projectDir, "generated"), { recursive: true });
    writeFileSync(artifactPath, "generated pixels");
    writeFileSync(
      join(projectDir, "video-creater.project.json"),
      JSON.stringify({
        schemaVersion: 1,
        id: "provider-e2e-project",
        files: { generated: "generated" },
      }),
    );
    writeFileSync(join(projectDir, "generated", "index.json"), "{not-json");
    writeFileSync(
      fakeCargo,
      [
        "#!/bin/sh",
        `printf '%s\\n' '{"ok":true,"provider":"replicate","model":"black-forest-labs/flux-schnell","projectDir":"${projectDir}","artifactPath":"${artifactPath}","outputCount":1,"jobStatus":"Completed"}'`,
      ].join("\n"),
      { mode: 0o755 },
    );

    const outDir = join(tempDir, "provider-out");
    const result = spawnSync(
      process.execPath,
      [
        join(repoRoot, "scripts/provider-e2e.mjs"),
        "--provider",
        "replicate",
        "--live",
        "--out-dir",
        outDir,
      ],
      {
        cwd: repoRoot,
        encoding: "utf8",
        env: {
          ...process.env,
          PATH: `${tempDir}:${process.env.PATH ?? ""}`,
          REPLICATE_API_TOKEN: "super-secret-replicate-token",
          VIDEO_CREATER_PROVIDER_E2E_ALLOW_SPEND: "1",
        },
      },
    );

    expect(result.status).toBe(1);
    const report = JSON.parse(result.stdout) as {
      ok: boolean;
      error: string;
      result: { projectDir: string };
    };
    expect(report).toEqual(
      expect.objectContaining({
        ok: false,
        error: `Provider E2E generated asset index is not valid JSON: ${join(projectDir, "generated", "index.json")}`,
        result: expect.objectContaining({ projectDir }),
      }),
    );
  });

  it("fails closed when the provider E2E generated asset index path escapes the split project", () => {
    const tempDir = mkdtempSync(join(tmpdir(), "video-creater-provider-e2e-"));
    const fakeCargo = join(tempDir, "cargo");
    const projectDir = join(tempDir, "project");
    const externalGeneratedDir = join(tempDir, "external-generated");
    const artifactPath = join(projectDir, "generated", "provider-e2e-output.png");
    const sidecarPath = join(
      projectDir,
      "generated",
      "provider-e2e-generated",
      "asset.json",
    );
    mkdirSync(externalGeneratedDir);
    mkdirSync(join(projectDir, "generated", "provider-e2e-generated"), {
      recursive: true,
    });
    mkdirSync(join(projectDir, "media"));
    writeFileSync(artifactPath, "project generated pixels");
    writeFileSync(
      join(projectDir, "video-creater.project.json"),
      JSON.stringify({
        schemaVersion: 1,
        id: "provider-e2e-project",
        files: { generated: "../external-generated", media: "media/index.json" },
      }),
    );
    writeFileSync(
      join(projectDir, "media", "index.json"),
      JSON.stringify({
        schemaVersion: 1,
        folders: [],
        assets: [
          {
            id: "provider-e2e-output",
            relativePath: "generated/provider-e2e-output.png",
            kind: "generated",
            durationSeconds: 0,
            width: 1024,
            height: 768,
            fps: 0,
          },
        ],
        analysis: [],
      }),
    );
    writeFileSync(
      join(externalGeneratedDir, "index.json"),
      JSON.stringify({
        schemaVersion: 1,
        assets: [
          {
            assetId: "provider-e2e-generated",
            path: "generated/provider-e2e-generated/asset.json",
            status: "completed",
            modelProvider: "replicate",
            modelId: "black-forest-labs/flux-schnell",
            outputCount: 1,
          },
        ],
      }),
    );
    writeFileSync(
      sidecarPath,
      JSON.stringify({
        id: "provider-e2e-generated",
        status: "completed",
        model: {
          provider: "replicate",
          id: "black-forest-labs/flux-schnell",
        },
        outputs: [
          {
            mediaId: "provider-e2e-output",
            relativePath: "generated/provider-e2e-output.png",
            sourceUrl: "https://provider.example/generated-output.png",
            width: 1024,
            height: 768,
            durationSeconds: 0,
            fps: 0,
          },
        ],
      }),
    );
    writeFileSync(
      fakeCargo,
      [
        "#!/bin/sh",
        `printf '%s\\n' '{"ok":true,"provider":"replicate","model":"black-forest-labs/flux-schnell","projectDir":"${projectDir}","artifactPath":"${artifactPath}","outputCount":1,"jobStatus":"Completed"}'`,
      ].join("\n"),
      { mode: 0o755 },
    );

    const outDir = join(tempDir, "provider-out");
    const result = spawnSync(
      process.execPath,
      [
        join(repoRoot, "scripts/provider-e2e.mjs"),
        "--provider",
        "replicate",
        "--live",
        "--out-dir",
        outDir,
      ],
      {
        cwd: repoRoot,
        encoding: "utf8",
        env: {
          ...process.env,
          PATH: `${tempDir}:${process.env.PATH ?? ""}`,
          REPLICATE_API_TOKEN: "super-secret-replicate-token",
          VIDEO_CREATER_PROVIDER_E2E_ALLOW_SPEND: "1",
        },
      },
    );

    expect(result.status).toBe(1);
    const report = JSON.parse(result.stdout) as {
      ok: boolean;
      error: string;
      result: { projectDir: string };
    };
    expect(report).toEqual(
      expect.objectContaining({
        ok: false,
        error: "Provider E2E generated asset index path escapes the split project: ../external-generated",
        result: expect.objectContaining({ projectDir }),
      }),
    );
  });

  it("fails closed when the provider E2E generated asset index has no output-backed assets", () => {
    const tempDir = mkdtempSync(join(tmpdir(), "video-creater-provider-e2e-"));
    const fakeCargo = join(tempDir, "cargo");
    const artifactPath = join(tempDir, "generated", "replicate-output.png");
    const projectDir = join(tempDir, "project");
    mkdirSync(join(tempDir, "generated"));
    mkdirSync(join(projectDir, "generated"), { recursive: true });
    writeFileSync(artifactPath, "generated pixels");
    writeFileSync(
      join(projectDir, "video-creater.project.json"),
      JSON.stringify({
        schemaVersion: 1,
        id: "provider-e2e-project",
        files: { generated: "generated" },
      }),
    );
    writeFileSync(
      join(projectDir, "generated", "index.json"),
      JSON.stringify({
        schemaVersion: 1,
        assets: [{ assetId: "provider-e2e-generated", outputCount: 0 }],
      }),
    );
    writeFileSync(
      fakeCargo,
      [
        "#!/bin/sh",
        `printf '%s\\n' '{"ok":true,"provider":"replicate","model":"black-forest-labs/flux-schnell","projectDir":"${projectDir}","artifactPath":"${artifactPath}","outputCount":1,"jobStatus":"Completed"}'`,
      ].join("\n"),
      { mode: 0o755 },
    );

    const outDir = join(tempDir, "provider-out");
    const result = spawnSync(
      process.execPath,
      [
        join(repoRoot, "scripts/provider-e2e.mjs"),
        "--provider",
        "replicate",
        "--live",
        "--out-dir",
        outDir,
      ],
      {
        cwd: repoRoot,
        encoding: "utf8",
        env: {
          ...process.env,
          PATH: `${tempDir}:${process.env.PATH ?? ""}`,
          REPLICATE_API_TOKEN: "super-secret-replicate-token",
          VIDEO_CREATER_PROVIDER_E2E_ALLOW_SPEND: "1",
        },
      },
    );

    expect(result.status).toBe(1);
    const report = JSON.parse(result.stdout) as {
      ok: boolean;
      error: string;
      result: { projectDir: string };
    };
    expect(report).toEqual(
      expect.objectContaining({
        ok: false,
        error: `Provider E2E generated asset index has no output-backed assets: ${join(projectDir, "generated", "index.json")}`,
        result: expect.objectContaining({ projectDir }),
      }),
    );
  });

  it("fails closed when the provider E2E generated asset sidecar is missing", () => {
    const tempDir = mkdtempSync(join(tmpdir(), "video-creater-provider-e2e-"));
    const fakeCargo = join(tempDir, "cargo");
    const artifactPath = join(tempDir, "generated", "replicate-output.png");
    const projectDir = join(tempDir, "project");
    mkdirSync(join(tempDir, "generated"));
    mkdirSync(join(projectDir, "generated"), { recursive: true });
    writeFileSync(artifactPath, "generated pixels");
    writeFileSync(
      join(projectDir, "video-creater.project.json"),
      JSON.stringify({
        schemaVersion: 1,
        id: "provider-e2e-project",
        files: { generated: "generated" },
      }),
    );
    writeFileSync(
      join(projectDir, "generated", "index.json"),
      JSON.stringify({
        schemaVersion: 1,
        assets: [
          {
            assetId: "provider-e2e-generated",
            path: "generated/provider-e2e-generated/asset.json",
            status: "completed",
            modelProvider: "replicate",
            modelId: "black-forest-labs/flux-schnell",
            outputCount: 1,
          },
        ],
      }),
    );
    writeFileSync(
      fakeCargo,
      [
        "#!/bin/sh",
        `printf '%s\\n' '{"ok":true,"provider":"replicate","model":"black-forest-labs/flux-schnell","projectDir":"${projectDir}","artifactPath":"${artifactPath}","outputCount":1,"jobStatus":"Completed"}'`,
      ].join("\n"),
      { mode: 0o755 },
    );

    const outDir = join(tempDir, "provider-out");
    const result = spawnSync(
      process.execPath,
      [
        join(repoRoot, "scripts/provider-e2e.mjs"),
        "--provider",
        "replicate",
        "--live",
        "--out-dir",
        outDir,
      ],
      {
        cwd: repoRoot,
        encoding: "utf8",
        env: {
          ...process.env,
          PATH: `${tempDir}:${process.env.PATH ?? ""}`,
          REPLICATE_API_TOKEN: "super-secret-replicate-token",
          VIDEO_CREATER_PROVIDER_E2E_ALLOW_SPEND: "1",
        },
      },
    );

    const sidecarPath = join(
      projectDir,
      "generated",
      "provider-e2e-generated",
      "asset.json",
    );
    expect(result.status).toBe(1);
    const report = JSON.parse(result.stdout) as {
      ok: boolean;
      error: string;
      result: { projectDir: string };
    };
    expect(report).toEqual(
      expect.objectContaining({
        ok: false,
        error: `Provider E2E generated asset sidecar is missing: ${sidecarPath}`,
        result: expect.objectContaining({ projectDir }),
      }),
    );
  });

  it("fails closed when the provider E2E generated asset sidecar path escapes the split project", () => {
    const tempDir = mkdtempSync(join(tmpdir(), "video-creater-provider-e2e-"));
    const fakeCargo = join(tempDir, "cargo");
    const artifactPath = join(tempDir, "generated", "replicate-output.png");
    const projectDir = join(tempDir, "project");
    const escapedSidecarPath = join(tempDir, "escaped-asset.json");
    mkdirSync(join(tempDir, "generated"));
    mkdirSync(join(projectDir, "generated"), { recursive: true });
    mkdirSync(join(projectDir, "media"));
    writeFileSync(artifactPath, "generated pixels");
    writeFileSync(
      join(projectDir, "video-creater.project.json"),
      JSON.stringify({
        schemaVersion: 1,
        id: "provider-e2e-project",
        files: { generated: "generated", media: "media/index.json" },
      }),
    );
    writeFileSync(
      join(projectDir, "media", "index.json"),
      JSON.stringify({
        schemaVersion: 1,
        folders: [],
        assets: [
          {
            id: "provider-e2e-output",
            relativePath: "generated/provider-e2e-output.png",
            kind: "generated",
            durationSeconds: 0,
            width: 1024,
            height: 768,
            fps: 0,
          },
        ],
        analysis: [],
      }),
    );
    writeFileSync(
      join(projectDir, "generated", "index.json"),
      JSON.stringify({
        schemaVersion: 1,
        assets: [
          {
            assetId: "provider-e2e-generated",
            path: "../escaped-asset.json",
            status: "completed",
            modelProvider: "replicate",
            modelId: "black-forest-labs/flux-schnell",
            outputCount: 1,
          },
        ],
      }),
    );
    writeFileSync(
      escapedSidecarPath,
      JSON.stringify({
        id: "provider-e2e-generated",
        status: "completed",
        model: {
          provider: "replicate",
          id: "black-forest-labs/flux-schnell",
        },
        outputs: [
          {
            mediaId: "provider-e2e-output",
            relativePath: "generated/provider-e2e-output.png",
            sourceUrl: "https://provider.example/generated-output.png",
            width: 1024,
            height: 768,
            durationSeconds: 0,
            fps: 0,
          },
        ],
      }),
    );
    writeFileSync(
      join(projectDir, "generated", "provider-e2e-output.png"),
      "project generated pixels",
    );
    writeFileSync(
      fakeCargo,
      [
        "#!/bin/sh",
        `printf '%s\\n' '{"ok":true,"provider":"replicate","model":"black-forest-labs/flux-schnell","projectDir":"${projectDir}","artifactPath":"${join(projectDir, "generated", "provider-e2e-output.png")}","outputCount":1,"jobStatus":"Completed"}'`,
      ].join("\n"),
      { mode: 0o755 },
    );

    const outDir = join(tempDir, "provider-out");
    const result = spawnSync(
      process.execPath,
      [
        join(repoRoot, "scripts/provider-e2e.mjs"),
        "--provider",
        "replicate",
        "--live",
        "--out-dir",
        outDir,
      ],
      {
        cwd: repoRoot,
        encoding: "utf8",
        env: {
          ...process.env,
          PATH: `${tempDir}:${process.env.PATH ?? ""}`,
          REPLICATE_API_TOKEN: "super-secret-replicate-token",
          VIDEO_CREATER_PROVIDER_E2E_ALLOW_SPEND: "1",
        },
      },
    );

    expect(result.status).toBe(1);
    const report = JSON.parse(result.stdout) as {
      ok: boolean;
      error: string;
      result: { projectDir: string };
    };
    expect(report).toEqual(
      expect.objectContaining({
        ok: false,
        error: "Provider E2E generated asset sidecar path escapes the split project: ../escaped-asset.json",
        result: expect.objectContaining({ projectDir }),
      }),
    );
  });

  it("fails closed when the provider E2E generated asset sidecar path is outside the generated directory", () => {
    const tempDir = mkdtempSync(join(tmpdir(), "video-creater-provider-e2e-"));
    const fakeCargo = join(tempDir, "cargo");
    const projectDir = join(tempDir, "project");
    const artifactPath = join(projectDir, "generated", "provider-e2e-output.png");
    const sidecarPath = join(
      projectDir,
      "media",
      "provider-e2e-generated",
      "asset.json",
    );
    mkdirSync(join(projectDir, "generated"), { recursive: true });
    mkdirSync(join(projectDir, "media", "provider-e2e-generated"), {
      recursive: true,
    });
    writeFileSync(artifactPath, "project generated pixels");
    writeFileSync(
      join(projectDir, "video-creater.project.json"),
      JSON.stringify({
        schemaVersion: 1,
        id: "provider-e2e-project",
        files: { generated: "generated", media: "media/index.json" },
      }),
    );
    writeFileSync(
      join(projectDir, "media", "index.json"),
      JSON.stringify({
        schemaVersion: 1,
        folders: [],
        assets: [
          {
            id: "provider-e2e-output",
            relativePath: "generated/provider-e2e-output.png",
            kind: "generated",
            durationSeconds: 0,
            width: 1024,
            height: 768,
            fps: 0,
          },
        ],
        analysis: [],
      }),
    );
    writeFileSync(
      join(projectDir, "generated", "index.json"),
      JSON.stringify({
        schemaVersion: 1,
        assets: [
          {
            assetId: "provider-e2e-generated",
            path: "media/provider-e2e-generated/asset.json",
            status: "completed",
            modelProvider: "replicate",
            modelId: "black-forest-labs/flux-schnell",
            outputCount: 1,
          },
        ],
      }),
    );
    writeFileSync(
      sidecarPath,
      JSON.stringify({
        id: "provider-e2e-generated",
        status: "completed",
        model: {
          provider: "replicate",
          id: "black-forest-labs/flux-schnell",
        },
        outputs: [
          {
            mediaId: "provider-e2e-output",
            relativePath: "generated/provider-e2e-output.png",
            sourceUrl: "https://provider.example/generated-output.png",
            width: 1024,
            height: 768,
            durationSeconds: 0,
            fps: 0,
          },
        ],
      }),
    );
    writeFileSync(
      fakeCargo,
      [
        "#!/bin/sh",
        `printf '%s\\n' '{"ok":true,"provider":"replicate","model":"black-forest-labs/flux-schnell","projectDir":"${projectDir}","artifactPath":"${artifactPath}","outputCount":1,"jobStatus":"Completed"}'`,
      ].join("\n"),
      { mode: 0o755 },
    );

    const outDir = join(tempDir, "provider-out");
    const result = spawnSync(
      process.execPath,
      [
        join(repoRoot, "scripts/provider-e2e.mjs"),
        "--provider",
        "replicate",
        "--live",
        "--out-dir",
        outDir,
      ],
      {
        cwd: repoRoot,
        encoding: "utf8",
        env: {
          ...process.env,
          PATH: `${tempDir}:${process.env.PATH ?? ""}`,
          REPLICATE_API_TOKEN: "super-secret-replicate-token",
          VIDEO_CREATER_PROVIDER_E2E_ALLOW_SPEND: "1",
        },
      },
    );

    expect(result.status).toBe(1);
    const report = JSON.parse(result.stdout) as {
      ok: boolean;
      error: string;
      result: { projectDir: string };
    };
    expect(report).toEqual(
      expect.objectContaining({
        ok: false,
        error: "Provider E2E generated asset sidecar path is outside the generated asset directory: media/provider-e2e-generated/asset.json",
        result: expect.objectContaining({ projectDir }),
      }),
    );
  });

  it("fails closed when the provider E2E generated asset sidecar is not parseable", () => {
    const tempDir = mkdtempSync(join(tmpdir(), "video-creater-provider-e2e-"));
    const fakeCargo = join(tempDir, "cargo");
    const artifactPath = join(tempDir, "generated", "replicate-output.png");
    const projectDir = join(tempDir, "project");
    const sidecarPath = join(
      projectDir,
      "generated",
      "provider-e2e-generated",
      "asset.json",
    );
    mkdirSync(join(tempDir, "generated"));
    mkdirSync(join(projectDir, "generated", "provider-e2e-generated"), {
      recursive: true,
    });
    mkdirSync(join(projectDir, "media"));
    writeFileSync(artifactPath, "generated pixels");
    writeFileSync(
      join(projectDir, "video-creater.project.json"),
      JSON.stringify({
        schemaVersion: 1,
        id: "provider-e2e-project",
        files: { generated: "generated", media: "media/index.json" },
      }),
    );
    writeFileSync(
      join(projectDir, "media", "index.json"),
      JSON.stringify({
        schemaVersion: 1,
        folders: [],
        assets: [
          {
            id: "provider-e2e-output",
            relativePath: "generated/provider-e2e-output.png",
            kind: "generated",
            durationSeconds: 0,
            width: 1024,
            height: 768,
            fps: 0,
          },
        ],
        analysis: [],
      }),
    );
    writeFileSync(
      join(projectDir, "generated", "index.json"),
      JSON.stringify({
        schemaVersion: 1,
        assets: [
          {
            assetId: "provider-e2e-generated",
            path: "generated/provider-e2e-generated/asset.json",
            status: "completed",
            modelProvider: "replicate",
            modelId: "black-forest-labs/flux-schnell",
            outputCount: 1,
          },
        ],
      }),
    );
    writeFileSync(sidecarPath, "{not-json");
    writeFileSync(
      fakeCargo,
      [
        "#!/bin/sh",
        `printf '%s\\n' '{"ok":true,"provider":"replicate","model":"black-forest-labs/flux-schnell","projectDir":"${projectDir}","artifactPath":"${artifactPath}","outputCount":1,"jobStatus":"Completed"}'`,
      ].join("\n"),
      { mode: 0o755 },
    );

    const outDir = join(tempDir, "provider-out");
    const result = spawnSync(
      process.execPath,
      [
        join(repoRoot, "scripts/provider-e2e.mjs"),
        "--provider",
        "replicate",
        "--live",
        "--out-dir",
        outDir,
      ],
      {
        cwd: repoRoot,
        encoding: "utf8",
        env: {
          ...process.env,
          PATH: `${tempDir}:${process.env.PATH ?? ""}`,
          REPLICATE_API_TOKEN: "super-secret-replicate-token",
          VIDEO_CREATER_PROVIDER_E2E_ALLOW_SPEND: "1",
        },
      },
    );

    expect(result.status).toBe(1);
    const report = JSON.parse(result.stdout) as {
      ok: boolean;
      error: string;
      result: { projectDir: string };
    };
    expect(report).toEqual(
      expect.objectContaining({
        ok: false,
        error: `Provider E2E generated asset sidecar is not valid JSON: ${sidecarPath}`,
        result: expect.objectContaining({ projectDir }),
      }),
    );
  });

  it("fails closed when the provider E2E generated asset sidecar is not completed", () => {
    const tempDir = mkdtempSync(join(tmpdir(), "video-creater-provider-e2e-"));
    const fakeCargo = join(tempDir, "cargo");
    const artifactPath = join(tempDir, "generated", "replicate-output.png");
    const projectDir = join(tempDir, "project");
    const sidecarPath = join(
      projectDir,
      "generated",
      "provider-e2e-generated",
      "asset.json",
    );
    mkdirSync(join(tempDir, "generated"));
    mkdirSync(join(projectDir, "generated", "provider-e2e-generated"), {
      recursive: true,
    });
    mkdirSync(join(projectDir, "media"));
    writeFileSync(artifactPath, "generated pixels");
    writeFileSync(
      join(projectDir, "video-creater.project.json"),
      JSON.stringify({
        schemaVersion: 1,
        id: "provider-e2e-project",
        files: { generated: "generated", media: "media/index.json" },
      }),
    );
    writeFileSync(
      join(projectDir, "media", "index.json"),
      JSON.stringify({
        schemaVersion: 1,
        folders: [],
        assets: [
          {
            id: "provider-e2e-output",
            relativePath: "generated/provider-e2e-output.png",
            kind: "generated",
            durationSeconds: 0,
            width: 1024,
            height: 768,
            fps: 0,
          },
        ],
        analysis: [],
      }),
    );
    writeFileSync(
      join(projectDir, "generated", "index.json"),
      JSON.stringify({
        schemaVersion: 1,
        assets: [
          {
            assetId: "provider-e2e-generated",
            path: "generated/provider-e2e-generated/asset.json",
            status: "completed",
            modelProvider: "replicate",
            modelId: "black-forest-labs/flux-schnell",
            outputCount: 1,
          },
        ],
      }),
    );
    writeFileSync(
      sidecarPath,
      JSON.stringify({
        id: "provider-e2e-generated",
        status: "running",
        model: {
          provider: "replicate",
          id: "black-forest-labs/flux-schnell",
        },
        outputs: [{ mediaId: "provider-e2e-output", relativePath: "generated/provider-e2e-output.png", sourceUrl: "https://provider.example/generated-output.png", width: 1024, height: 768, durationSeconds: 0, fps: 0 }],
      }),
    );
    writeFileSync(
      fakeCargo,
      [
        "#!/bin/sh",
        `printf '%s\\n' '{"ok":true,"provider":"replicate","model":"black-forest-labs/flux-schnell","projectDir":"${projectDir}","artifactPath":"${artifactPath}","outputCount":1,"jobStatus":"Completed"}'`,
      ].join("\n"),
      { mode: 0o755 },
    );

    const outDir = join(tempDir, "provider-out");
    const result = spawnSync(
      process.execPath,
      [
        join(repoRoot, "scripts/provider-e2e.mjs"),
        "--provider",
        "replicate",
        "--live",
        "--out-dir",
        outDir,
      ],
      {
        cwd: repoRoot,
        encoding: "utf8",
        env: {
          ...process.env,
          PATH: `${tempDir}:${process.env.PATH ?? ""}`,
          REPLICATE_API_TOKEN: "super-secret-replicate-token",
          VIDEO_CREATER_PROVIDER_E2E_ALLOW_SPEND: "1",
        },
      },
    );

    expect(result.status).toBe(1);
    const report = JSON.parse(result.stdout) as {
      ok: boolean;
      error: string;
      result: { projectDir: string };
    };
    expect(report).toEqual(
      expect.objectContaining({
        ok: false,
        error: `Provider E2E generated asset sidecar is not completed: ${sidecarPath}`,
        result: expect.objectContaining({ projectDir }),
      }),
    );
  });

  it("fails closed when the provider E2E generated asset sidecar has no outputs", () => {
    const tempDir = mkdtempSync(join(tmpdir(), "video-creater-provider-e2e-"));
    const fakeCargo = join(tempDir, "cargo");
    const artifactPath = join(tempDir, "generated", "replicate-output.png");
    const projectDir = join(tempDir, "project");
    const sidecarPath = join(
      projectDir,
      "generated",
      "provider-e2e-generated",
      "asset.json",
    );
    mkdirSync(join(tempDir, "generated"));
    mkdirSync(join(projectDir, "generated", "provider-e2e-generated"), {
      recursive: true,
    });
    writeFileSync(artifactPath, "generated pixels");
    writeFileSync(
      join(projectDir, "video-creater.project.json"),
      JSON.stringify({
        schemaVersion: 1,
        id: "provider-e2e-project",
        files: { generated: "generated" },
      }),
    );
    writeFileSync(
      join(projectDir, "generated", "index.json"),
      JSON.stringify({
        schemaVersion: 1,
        assets: [
          {
            assetId: "provider-e2e-generated",
            path: "generated/provider-e2e-generated/asset.json",
            status: "completed",
            modelProvider: "replicate",
            modelId: "black-forest-labs/flux-schnell",
            outputCount: 1,
          },
        ],
      }),
    );
    writeFileSync(
      sidecarPath,
      JSON.stringify({
        id: "provider-e2e-generated",
        status: "completed",
        model: {
          provider: "replicate",
          id: "black-forest-labs/flux-schnell",
        },
        outputs: [],
      }),
    );
    writeFileSync(
      fakeCargo,
      [
        "#!/bin/sh",
        `printf '%s\\n' '{"ok":true,"provider":"replicate","model":"black-forest-labs/flux-schnell","projectDir":"${projectDir}","artifactPath":"${artifactPath}","outputCount":1,"jobStatus":"Completed"}'`,
      ].join("\n"),
      { mode: 0o755 },
    );

    const outDir = join(tempDir, "provider-out");
    const result = spawnSync(
      process.execPath,
      [
        join(repoRoot, "scripts/provider-e2e.mjs"),
        "--provider",
        "replicate",
        "--live",
        "--out-dir",
        outDir,
      ],
      {
        cwd: repoRoot,
        encoding: "utf8",
        env: {
          ...process.env,
          PATH: `${tempDir}:${process.env.PATH ?? ""}`,
          REPLICATE_API_TOKEN: "super-secret-replicate-token",
          VIDEO_CREATER_PROVIDER_E2E_ALLOW_SPEND: "1",
        },
      },
    );

    expect(result.status).toBe(1);
    const report = JSON.parse(result.stdout) as {
      ok: boolean;
      error: string;
      result: { projectDir: string };
    };
    expect(report).toEqual(
      expect.objectContaining({
        ok: false,
        error: `Provider E2E generated asset sidecar has no outputs: ${sidecarPath}`,
        result: expect.objectContaining({ projectDir }),
      }),
    );
  });

  it("fails closed when the provider E2E audio insert sidecar loses placement intent", () => {
    const tempDir = mkdtempSync(join(tmpdir(), "video-creater-provider-e2e-audio-intent-"));
    const fakeCargo = join(tempDir, "cargo");
    const fixtureReportPath = writeProviderPolicyFixture({
      rootDir: tempDir,
      provider: "fal.ai",
      model: "sonilo/v1.1/video-to-music",
      requestId: "fal-video-music-insert-request-1",
      scenario: "local-video-to-music-insert",
    });
    const fixtureReport = JSON.parse(readFileSync(fixtureReportPath, "utf8")) as {
      result: { projectDir: string };
    };
    const generatedIndexPath = join(fixtureReport.result.projectDir, "generated", "index.json");
    const generatedIndex = JSON.parse(readFileSync(generatedIndexPath, "utf8")) as {
      assets: Array<{ path: string; status: string }>;
    };
    const completedAsset = generatedIndex.assets.find((asset) => asset.status === "completed");
    expect(completedAsset).toBeTruthy();
    const sidecarPath = completedAsset
      ? join(fixtureReport.result.projectDir, completedAsset.path)
      : join(fixtureReport.result.projectDir, "generated", "provider-e2e-generated", "asset.json");
    const sidecar = JSON.parse(readFileSync(sidecarPath, "utf8")) as {
      placementIntent?: string;
    };
    delete sidecar.placementIntent;
    writeFileSync(sidecarPath, JSON.stringify(sidecar, null, 2));
    writeFileSync(
      fakeCargo,
      [
        "#!/usr/bin/env node",
        `console.log(${JSON.stringify(JSON.stringify(fixtureReport.result))});`,
      ].join("\n"),
      { mode: 0o755 },
    );

    const result = spawnSync(
      process.execPath,
      [
        join(repoRoot, "scripts/provider-e2e.mjs"),
        "--provider",
        "fal.ai",
        "--live",
        "--scenario",
        "local-video-to-music-insert",
        "--out-dir",
        join(tempDir, "provider-out"),
      ],
      {
        cwd: repoRoot,
        encoding: "utf8",
        env: {
          ...process.env,
          PATH: `${tempDir}:${process.env.PATH ?? ""}`,
          FAL_KEY: "super-secret-fal-token",
          VIDEO_CREATER_PROVIDER_E2E_ALLOW_SPEND: "1",
        },
      },
    );

    expect(result.status).toBe(1);
    const report = JSON.parse(result.stdout) as {
      ok: boolean;
      error: string;
      result: { projectDir: string };
    };
    expect(report).toEqual(
      expect.objectContaining({
        ok: false,
        error: `Provider E2E audio insert sidecar must retain placementIntent: ${sidecarPath}`,
        result: expect.objectContaining({ projectDir: fixtureReport.result.projectDir }),
      }),
    );
  });

  it("fails closed when the provider E2E audio insert sidecar loses selected source trim settings", () => {
    const tempDir = mkdtempSync(join(tmpdir(), "video-creater-provider-e2e-audio-trim-"));
    const fakeCargo = join(tempDir, "cargo");
    const fixtureReportPath = writeProviderPolicyFixture({
      rootDir: tempDir,
      provider: "fal.ai",
      model: "sonilo/v1.1/video-to-music",
      requestId: "fal-video-music-insert-request-1",
      scenario: "local-video-to-music-insert",
    });
    const fixtureReport = JSON.parse(readFileSync(fixtureReportPath, "utf8")) as {
      result: { projectDir: string };
    };
    const generatedIndexPath = join(fixtureReport.result.projectDir, "generated", "index.json");
    const generatedIndex = JSON.parse(readFileSync(generatedIndexPath, "utf8")) as {
      assets: Array<{ path: string; status: string }>;
    };
    const completedAsset = generatedIndex.assets.find((asset) => asset.status === "completed");
    expect(completedAsset).toBeTruthy();
    const sidecarPath = completedAsset
      ? join(fixtureReport.result.projectDir, completedAsset.path)
      : join(fixtureReport.result.projectDir, "generated", "provider-e2e-generated", "asset.json");
    const sidecar = JSON.parse(readFileSync(sidecarPath, "utf8")) as {
      settings?: { videoSourceStartSeconds?: number };
    };
    if (sidecar.settings) {
      delete sidecar.settings.videoSourceStartSeconds;
    }
    writeFileSync(sidecarPath, JSON.stringify(sidecar, null, 2));
    writeFileSync(
      fakeCargo,
      [
        "#!/usr/bin/env node",
        `console.log(${JSON.stringify(JSON.stringify(fixtureReport.result))});`,
      ].join("\n"),
      { mode: 0o755 },
    );

    const result = spawnSync(
      process.execPath,
      [
        join(repoRoot, "scripts/provider-e2e.mjs"),
        "--provider",
        "fal.ai",
        "--live",
        "--scenario",
        "local-video-to-music-insert",
        "--out-dir",
        join(tempDir, "provider-out"),
      ],
      {
        cwd: repoRoot,
        encoding: "utf8",
        env: {
          ...process.env,
          PATH: `${tempDir}:${process.env.PATH ?? ""}`,
          FAL_KEY: "super-secret-fal-token",
          VIDEO_CREATER_PROVIDER_E2E_ALLOW_SPEND: "1",
        },
      },
    );

    expect(result.status).toBe(1);
    const report = JSON.parse(result.stdout) as {
      ok: boolean;
      error: string;
      result: { projectDir: string };
    };
    expect(report).toEqual(
      expect.objectContaining({
        ok: false,
        error: `Provider E2E audio insert sidecar must retain selected source trim settings: ${sidecarPath}`,
        result: expect.objectContaining({ projectDir: fixtureReport.result.projectDir }),
      }),
    );
  });

  it("fails closed when the provider E2E audio insert sidecar loses source video reference bucket", () => {
    const tempDir = mkdtempSync(join(tmpdir(), "video-creater-provider-e2e-audio-ref-"));
    const fakeCargo = join(tempDir, "cargo");
    const fixtureReportPath = writeProviderPolicyFixture({
      rootDir: tempDir,
      provider: "fal.ai",
      model: "sonilo/v1.1/video-to-music",
      requestId: "fal-video-music-insert-request-1",
      scenario: "local-video-to-music-insert",
    });
    const fixtureReport = JSON.parse(readFileSync(fixtureReportPath, "utf8")) as {
      result: { projectDir: string };
    };
    const generatedIndexPath = join(fixtureReport.result.projectDir, "generated", "index.json");
    const generatedIndex = JSON.parse(readFileSync(generatedIndexPath, "utf8")) as {
      assets: Array<{ path: string; status: string }>;
    };
    const completedAsset = generatedIndex.assets.find((asset) => asset.status === "completed");
    expect(completedAsset).toBeTruthy();
    const sidecarPath = completedAsset
      ? join(fixtureReport.result.projectDir, completedAsset.path)
      : join(fixtureReport.result.projectDir, "generated", "provider-e2e-generated", "asset.json");
    const sidecar = JSON.parse(readFileSync(sidecarPath, "utf8")) as {
      references?: { referenceVideoMediaRefs?: string[] };
    };
    if (sidecar.references) {
      delete sidecar.references.referenceVideoMediaRefs;
    }
    writeFileSync(sidecarPath, JSON.stringify(sidecar, null, 2));
    writeFileSync(
      fakeCargo,
      [
        "#!/usr/bin/env node",
        `console.log(${JSON.stringify(JSON.stringify(fixtureReport.result))});`,
      ].join("\n"),
      { mode: 0o755 },
    );

    const result = spawnSync(
      process.execPath,
      [
        join(repoRoot, "scripts/provider-e2e.mjs"),
        "--provider",
        "fal.ai",
        "--live",
        "--scenario",
        "local-video-to-music-insert",
        "--out-dir",
        join(tempDir, "provider-out"),
      ],
      {
        cwd: repoRoot,
        encoding: "utf8",
        env: {
          ...process.env,
          PATH: `${tempDir}:${process.env.PATH ?? ""}`,
          FAL_KEY: "super-secret-fal-token",
          VIDEO_CREATER_PROVIDER_E2E_ALLOW_SPEND: "1",
        },
      },
    );

    expect(result.status).toBe(1);
    const report = JSON.parse(result.stdout) as {
      ok: boolean;
      error: string;
      result: { projectDir: string };
    };
    expect(report).toEqual(
      expect.objectContaining({
        ok: false,
        error: `Provider E2E video-to-audio sidecar must retain one referenceVideoMediaRefs value: ${sidecarPath}`,
        result: expect.objectContaining({ projectDir: fixtureReport.result.projectDir }),
      }),
    );
  });

  it("fails closed when the provider E2E local video upscale retry sidecar loses selected source trim settings", () => {
    const tempDir = mkdtempSync(join(tmpdir(), "video-creater-provider-e2e-video-upscale-trim-"));
    const fakeCargo = join(tempDir, "cargo");
    const fixtureReportPath = writeProviderPolicyFixture({
      rootDir: tempDir,
      provider: "fal.ai",
      model: "fal-ai/video-upscaler",
      requestId: "fal-video-upscale-retry-request-1",
      scenario: "local-video-upscale-retry",
      sourceUrl: "https://provider.example/fal-video-upscale-retry-request-1/upscaled-video.mp4",
    });
    const fixtureReport = JSON.parse(readFileSync(fixtureReportPath, "utf8")) as {
      result: { projectDir: string };
    };
    const generatedIndexPath = join(fixtureReport.result.projectDir, "generated", "index.json");
    const generatedIndex = JSON.parse(readFileSync(generatedIndexPath, "utf8")) as {
      assets: Array<{ path: string; status: string }>;
    };
    const completedAsset = generatedIndex.assets.find((asset) => asset.status === "completed");
    expect(completedAsset).toBeTruthy();
    const sidecarPath = completedAsset
      ? join(fixtureReport.result.projectDir, completedAsset.path)
      : join(fixtureReport.result.projectDir, "generated", "provider-e2e-generated", "asset.json");
    const sidecar = JSON.parse(readFileSync(sidecarPath, "utf8")) as {
      settings?: { videoSourceStartSeconds?: number };
    };
    if (sidecar.settings) {
      delete sidecar.settings.videoSourceStartSeconds;
    }
    writeFileSync(sidecarPath, JSON.stringify(sidecar, null, 2));
    writeFileSync(
      fakeCargo,
      [
        "#!/usr/bin/env node",
        `console.log(${JSON.stringify(JSON.stringify(fixtureReport.result))});`,
      ].join("\n"),
      { mode: 0o755 },
    );

    const result = spawnSync(
      process.execPath,
      [
        join(repoRoot, "scripts/provider-e2e.mjs"),
        "--provider",
        "fal.ai",
        "--live",
        "--scenario",
        "local-video-upscale-retry",
        "--out-dir",
        join(tempDir, "provider-out"),
      ],
      {
        cwd: repoRoot,
        encoding: "utf8",
        env: {
          ...process.env,
          PATH: `${tempDir}:${process.env.PATH ?? ""}`,
          FAL_KEY: "super-secret-fal-token",
          VIDEO_CREATER_PROVIDER_E2E_ALLOW_SPEND: "1",
        },
      },
    );

    expect(result.status).toBe(1);
    const report = JSON.parse(result.stdout) as {
      ok: boolean;
      error: string;
      result: { projectDir: string };
    };
    expect(report).toEqual(
      expect.objectContaining({
        ok: false,
        error: `Provider E2E video-to-video sidecar must retain selected source trim settings: ${sidecarPath}`,
        result: expect.objectContaining({ projectDir: fixtureReport.result.projectDir }),
      }),
    );
  });

  it("fails closed when the provider E2E replacement timeline loses shared link group evidence", () => {
    const tempDir = mkdtempSync(join(tmpdir(), "video-creater-provider-e2e-linked-replace-group-"));
    const fakeCargo = join(tempDir, "cargo");
    const fixtureReportPath = writeProviderPolicyFixture({
      rootDir: tempDir,
      provider: "fal.ai",
      model: "fal-ai/nano-banana-pro/edit",
      requestId: "fal-image-edit-replace-request-1",
      scenario: "local-image-edit-replace",
    });
    const fixtureReport = JSON.parse(readFileSync(fixtureReportPath, "utf8")) as {
      result: { projectDir: string };
    };
    const timelinePath = join(fixtureReport.result.projectDir, "timeline.json");
    const timeline = JSON.parse(readFileSync(timelinePath, "utf8")) as {
      tracks: Array<{
        items: Array<{ id: string; properties?: { linkGroupId?: string } }>;
      }>;
    };
    for (const item of timeline.tracks.flatMap((track) => track.items)) {
      if (item.id === "item-image-edit-target" || item.id === "item-image-edit-target-linked") {
        if (item.properties) {
          delete item.properties.linkGroupId;
        }
      }
    }
    writeFileSync(timelinePath, JSON.stringify(timeline, null, 2));
    writeFileSync(
      fakeCargo,
      [
        "#!/usr/bin/env node",
        `console.log(${JSON.stringify(JSON.stringify(fixtureReport.result))});`,
      ].join("\n"),
      { mode: 0o755 },
    );

    const result = spawnSync(
      process.execPath,
      [
        join(repoRoot, "scripts/provider-e2e.mjs"),
        "--provider",
        "fal.ai",
        "--live",
        "--scenario",
        "local-image-edit-replace",
        "--out-dir",
        join(tempDir, "provider-out"),
      ],
      {
        cwd: repoRoot,
        encoding: "utf8",
        env: {
          ...process.env,
          PATH: `${tempDir}:${process.env.PATH ?? ""}`,
          FAL_KEY: "super-secret-fal-token",
          VIDEO_CREATER_PROVIDER_E2E_ALLOW_SPEND: "1",
        },
      },
    );

    expect(result.status).toBe(1);
    const report = JSON.parse(result.stdout) as {
      ok: boolean;
      error: string;
      result: { projectDir: string };
    };
    expect(report).toEqual(
      expect.objectContaining({
        ok: false,
        error: "Provider E2E replacement scenario linked item is missing replacement linkGroupId evidence",
        result: expect.objectContaining({ projectDir: fixtureReport.result.projectDir }),
      }),
    );
  });

  it("fails closed when the provider E2E audio insert timeline duration mismatches the generated output", () => {
    const tempDir = mkdtempSync(join(tmpdir(), "video-creater-provider-e2e-audio-duration-"));
    const fakeCargo = join(tempDir, "cargo");
    const fixtureReportPath = writeProviderPolicyFixture({
      rootDir: tempDir,
      provider: "fal.ai",
      model: "sonilo/v1.1/video-to-music",
      requestId: "fal-video-music-insert-request-1",
      scenario: "local-video-to-music-insert",
    });
    const fixtureReport = JSON.parse(readFileSync(fixtureReportPath, "utf8")) as {
      result: { projectDir: string };
    };
    const timelinePath = join(fixtureReport.result.projectDir, "timeline.json");
    const timeline = JSON.parse(readFileSync(timelinePath, "utf8")) as {
      tracks: Array<{
        items: Array<{
          id: string;
          durationSeconds?: number;
        }>;
      }>;
    };
    const insertedItem = timeline.tracks
      .flatMap((track) => track.items)
      .find((item) => item.id === "item-video-to-music-inserted-audio");
    expect(insertedItem).toBeTruthy();
    if (insertedItem) {
      insertedItem.durationSeconds = 3;
    }
    writeFileSync(timelinePath, JSON.stringify(timeline, null, 2));
    writeFileSync(
      fakeCargo,
      [
        "#!/usr/bin/env node",
        `console.log(${JSON.stringify(JSON.stringify(fixtureReport.result))});`,
      ].join("\n"),
      { mode: 0o755 },
    );

    const result = spawnSync(
      process.execPath,
      [
        join(repoRoot, "scripts/provider-e2e.mjs"),
        "--provider",
        "fal.ai",
        "--live",
        "--scenario",
        "local-video-to-music-insert",
        "--out-dir",
        join(tempDir, "provider-out"),
      ],
      {
        cwd: repoRoot,
        encoding: "utf8",
        env: {
          ...process.env,
          PATH: `${tempDir}:${process.env.PATH ?? ""}`,
          FAL_KEY: "super-secret-fal-token",
          VIDEO_CREATER_PROVIDER_E2E_ALLOW_SPEND: "1",
        },
      },
    );

    expect(result.status).toBe(1);
    const report = JSON.parse(result.stdout) as {
      ok: boolean;
      error: string;
      result: { projectDir: string };
    };
    expect(report).toEqual(
      expect.objectContaining({
        ok: false,
        error: "Provider E2E audio insert scenario item duration does not match generated output",
        result: expect.objectContaining({ projectDir: fixtureReport.result.projectDir }),
      }),
    );
  });

  it("fails closed when the provider E2E audio insert source video trim is stale", () => {
    const tempDir = mkdtempSync(join(tmpdir(), "video-creater-provider-e2e-source-trim-"));
    const fakeCargo = join(tempDir, "cargo");
    const fixtureReportPath = writeProviderPolicyFixture({
      rootDir: tempDir,
      provider: "fal.ai",
      model: "sonilo/v1.1/video-to-music",
      requestId: "fal-video-music-insert-request-1",
      scenario: "local-video-to-music-insert",
    });
    const fixtureReport = JSON.parse(readFileSync(fixtureReportPath, "utf8")) as {
      result: { projectDir: string };
    };
    const timelinePath = join(fixtureReport.result.projectDir, "timeline.json");
    const timeline = JSON.parse(readFileSync(timelinePath, "utf8")) as {
      tracks: Array<{
        items: Array<{
          id: string;
          durationSeconds?: number;
        }>;
      }>;
    };
    const sourceItem = timeline.tracks
      .flatMap((track) => track.items)
      .find((item) => item.id === "item-video-to-music-source");
    expect(sourceItem).toBeTruthy();
    if (sourceItem) {
      sourceItem.durationSeconds = 99;
    }
    writeFileSync(timelinePath, JSON.stringify(timeline, null, 2));
    writeFileSync(
      fakeCargo,
      [
        "#!/usr/bin/env node",
        `console.log(${JSON.stringify(JSON.stringify(fixtureReport.result))});`,
      ].join("\n"),
      { mode: 0o755 },
    );

    const result = spawnSync(
      process.execPath,
      [
        join(repoRoot, "scripts/provider-e2e.mjs"),
        "--provider",
        "fal.ai",
        "--live",
        "--scenario",
        "local-video-to-music-insert",
        "--out-dir",
        join(tempDir, "provider-out"),
      ],
      {
        cwd: repoRoot,
        encoding: "utf8",
        env: {
          ...process.env,
          PATH: `${tempDir}:${process.env.PATH ?? ""}`,
          FAL_KEY: "super-secret-fal-token",
          VIDEO_CREATER_PROVIDER_E2E_ALLOW_SPEND: "1",
        },
      },
    );

    expect(result.status).toBe(1);
    const report = JSON.parse(result.stdout) as {
      ok: boolean;
      error: string;
      result: { projectDir: string };
    };
    expect(report).toEqual(
      expect.objectContaining({
        ok: false,
        error: "Provider E2E audio insert scenario source item duration does not match selected source trim",
        result: expect.objectContaining({ projectDir: fixtureReport.result.projectDir }),
      }),
    );
  });

  it("fails closed when the provider E2E visual insert output is not an MP4 artifact", () => {
    const tempDir = mkdtempSync(join(tmpdir(), "video-creater-provider-e2e-visual-mp4-"));
    const fakeCargo = join(tempDir, "cargo");
    const fixtureReportPath = writeProviderPolicyFixture({
      rootDir: tempDir,
      provider: "replicate",
      model: "bytedance/seedance-2.0",
      requestId: "replicate-video-insert-request-1",
      scenario: "text-to-video-insert",
    });
    const fixtureReport = JSON.parse(readFileSync(fixtureReportPath, "utf8")) as {
      result: { artifactPath: string; projectDir: string };
    };
    const replacementRelativePath = "generated/replicate-output.png";
    const replacementPath = join(fixtureReport.result.projectDir, replacementRelativePath);
    writeFileSync(replacementPath, "not mp4 bytes");
    const generatedIndexPath = join(fixtureReport.result.projectDir, "generated", "index.json");
    const generatedIndex = JSON.parse(readFileSync(generatedIndexPath, "utf8")) as {
      assets: Array<{ path: string; status: string }>;
    };
    const completedAsset = generatedIndex.assets.find((asset) => asset.status === "completed");
    expect(completedAsset).toBeTruthy();
    const sidecarPath = completedAsset
      ? join(fixtureReport.result.projectDir, completedAsset.path)
      : join(fixtureReport.result.projectDir, "generated", "replicate-generated", "asset.json");
    const sidecar = JSON.parse(readFileSync(sidecarPath, "utf8")) as {
      outputs: Array<{ mediaId: string; relativePath: string }>;
    };
    requiredAt(sidecar.outputs, 0, "provider sidecar output").relativePath = replacementRelativePath;
    writeFileSync(sidecarPath, JSON.stringify(sidecar, null, 2));
    const mediaIndexPath = join(fixtureReport.result.projectDir, "media", "index.json");
    const mediaIndex = JSON.parse(readFileSync(mediaIndexPath, "utf8")) as {
      assets: Array<{ id: string; relativePath: string }>;
    };
    const mediaRow = mediaIndex.assets.find((asset) => asset.id === requiredAt(sidecar.outputs, 0, "provider sidecar output").mediaId);
    expect(mediaRow).toBeTruthy();
    if (mediaRow) {
      mediaRow.relativePath = replacementRelativePath;
    }
    writeFileSync(mediaIndexPath, JSON.stringify(mediaIndex, null, 2));
    fixtureReport.result.artifactPath = replacementPath;
    writeFileSync(
      fakeCargo,
      [
        "#!/usr/bin/env node",
        `console.log(${JSON.stringify(JSON.stringify(fixtureReport.result))});`,
      ].join("\n"),
      { mode: 0o755 },
    );

    const result = spawnSync(
      process.execPath,
      [
        join(repoRoot, "scripts/provider-e2e.mjs"),
        "--provider",
        "replicate",
        "--live",
        "--scenario",
        "text-to-video-insert",
        "--out-dir",
        join(tempDir, "provider-out"),
      ],
      {
        cwd: repoRoot,
        encoding: "utf8",
        env: {
          ...process.env,
          PATH: `${tempDir}:${process.env.PATH ?? ""}`,
          REPLICATE_API_TOKEN: "super-secret-replicate-token",
          VIDEO_CREATER_PROVIDER_E2E_ALLOW_SPEND: "1",
        },
      },
    );

    expect(result.status).toBe(1);
    const report = JSON.parse(result.stdout) as {
      ok: boolean;
      error: string;
      result: { projectDir: string };
    };
    expect(report).toEqual(
      expect.objectContaining({
        ok: false,
        error: `Provider E2E video output is not an MP4 artifact: ${sidecarPath}`,
        result: expect.objectContaining({ projectDir: fixtureReport.result.projectDir }),
      }),
    );
  });

  it("fails closed when the provider E2E visual insert timeline duration mismatches the generated output", () => {
    const tempDir = mkdtempSync(join(tmpdir(), "video-creater-provider-e2e-visual-duration-"));
    const fakeCargo = join(tempDir, "cargo");
    const fixtureReportPath = writeProviderPolicyFixture({
      rootDir: tempDir,
      provider: "replicate",
      model: "bytedance/seedance-2.0",
      requestId: "replicate-video-insert-request-1",
      scenario: "text-to-video-insert",
    });
    const fixtureReport = JSON.parse(readFileSync(fixtureReportPath, "utf8")) as {
      result: { projectDir: string };
    };
    const timelinePath = join(fixtureReport.result.projectDir, "timeline.json");
    const timeline = JSON.parse(readFileSync(timelinePath, "utf8")) as {
      tracks: Array<{
        items: Array<{
          id: string;
          durationSeconds?: number;
        }>;
      }>;
    };
    const insertedItem = timeline.tracks
      .flatMap((track) => track.items)
      .find((item) => item.id === "item-text-video-inserted-video");
    expect(insertedItem).toBeTruthy();
    if (insertedItem) {
      insertedItem.durationSeconds = 3;
    }
    writeFileSync(timelinePath, JSON.stringify(timeline, null, 2));
    writeFileSync(
      fakeCargo,
      [
        "#!/usr/bin/env node",
        `console.log(${JSON.stringify(JSON.stringify(fixtureReport.result))});`,
      ].join("\n"),
      { mode: 0o755 },
    );

    const result = spawnSync(
      process.execPath,
      [
        join(repoRoot, "scripts/provider-e2e.mjs"),
        "--provider",
        "replicate",
        "--live",
        "--scenario",
        "text-to-video-insert",
        "--out-dir",
        join(tempDir, "provider-out"),
      ],
      {
        cwd: repoRoot,
        encoding: "utf8",
        env: {
          ...process.env,
          PATH: `${tempDir}:${process.env.PATH ?? ""}`,
          REPLICATE_API_TOKEN: "super-secret-replicate-token",
          VIDEO_CREATER_PROVIDER_E2E_ALLOW_SPEND: "1",
        },
      },
    );

    expect(result.status).toBe(1);
    const report = JSON.parse(result.stdout) as {
      ok: boolean;
      error: string;
      result: { projectDir: string };
    };
    expect(report).toEqual(
      expect.objectContaining({
        ok: false,
        error: "Provider E2E visual insert scenario item duration does not match generated output",
        result: expect.objectContaining({ projectDir: fixtureReport.result.projectDir }),
      }),
    );
  });

  it("fails closed when the provider E2E generated asset sidecar output is missing a relativePath", () => {
    const tempDir = mkdtempSync(join(tmpdir(), "video-creater-provider-e2e-"));
    const fakeCargo = join(tempDir, "cargo");
    const artifactPath = join(tempDir, "generated", "replicate-output.png");
    const projectDir = join(tempDir, "project");
    const sidecarPath = join(
      projectDir,
      "generated",
      "provider-e2e-generated",
      "asset.json",
    );
    mkdirSync(join(tempDir, "generated"));
    mkdirSync(join(projectDir, "generated", "provider-e2e-generated"), {
      recursive: true,
    });
    writeFileSync(artifactPath, "generated pixels");
    writeFileSync(
      join(projectDir, "video-creater.project.json"),
      JSON.stringify({
        schemaVersion: 1,
        id: "provider-e2e-project",
        files: { generated: "generated" },
      }),
    );
    writeFileSync(
      join(projectDir, "generated", "index.json"),
      JSON.stringify({
        schemaVersion: 1,
        assets: [
          {
            assetId: "provider-e2e-generated",
            path: "generated/provider-e2e-generated/asset.json",
            status: "completed",
            modelProvider: "replicate",
            modelId: "black-forest-labs/flux-schnell",
            outputCount: 1,
          },
        ],
      }),
    );
    writeFileSync(
      sidecarPath,
      JSON.stringify({
        id: "provider-e2e-generated",
        status: "completed",
        model: {
          provider: "replicate",
          id: "black-forest-labs/flux-schnell",
        },
        outputs: [{ mediaId: "provider-e2e-output" }],
      }),
    );
    writeFileSync(
      fakeCargo,
      [
        "#!/bin/sh",
        `printf '%s\\n' '{"ok":true,"provider":"replicate","model":"black-forest-labs/flux-schnell","projectDir":"${projectDir}","artifactPath":"${artifactPath}","outputCount":1,"jobStatus":"Completed"}'`,
      ].join("\n"),
      { mode: 0o755 },
    );

    const outDir = join(tempDir, "provider-out");
    const result = spawnSync(
      process.execPath,
      [
        join(repoRoot, "scripts/provider-e2e.mjs"),
        "--provider",
        "replicate",
        "--live",
        "--out-dir",
        outDir,
      ],
      {
        cwd: repoRoot,
        encoding: "utf8",
        env: {
          ...process.env,
          PATH: `${tempDir}:${process.env.PATH ?? ""}`,
          REPLICATE_API_TOKEN: "super-secret-replicate-token",
          VIDEO_CREATER_PROVIDER_E2E_ALLOW_SPEND: "1",
        },
      },
    );

    expect(result.status).toBe(1);
    const report = JSON.parse(result.stdout) as {
      ok: boolean;
      error: string;
      result: { projectDir: string };
    };
    expect(report).toEqual(
      expect.objectContaining({
        ok: false,
        error: `Provider E2E generated asset sidecar output is missing a relativePath: ${sidecarPath}`,
        result: expect.objectContaining({ projectDir }),
      }),
    );
  });

  it("fails closed when the provider E2E generated asset sidecar output is missing a sourceUrl", () => {
    const tempDir = mkdtempSync(join(tmpdir(), "video-creater-provider-e2e-"));
    const fakeCargo = join(tempDir, "cargo");
    const projectDir = join(tempDir, "project");
    const artifactPath = join(projectDir, "generated", "provider-e2e-output.png");
    const sidecarPath = join(
      projectDir,
      "generated",
      "provider-e2e-generated",
      "asset.json",
    );
    mkdirSync(join(projectDir, "generated", "provider-e2e-generated"), {
      recursive: true,
    });
    writeFileSync(artifactPath, "generated pixels");
    writeFileSync(
      join(projectDir, "video-creater.project.json"),
      JSON.stringify({
        schemaVersion: 1,
        id: "provider-e2e-project",
        files: { generated: "generated" },
      }),
    );
    writeFileSync(
      join(projectDir, "generated", "index.json"),
      JSON.stringify({
        schemaVersion: 1,
        assets: [
          {
            assetId: "provider-e2e-generated",
            path: "generated/provider-e2e-generated/asset.json",
            status: "completed",
            modelProvider: "replicate",
            modelId: "black-forest-labs/flux-schnell",
            outputCount: 1,
          },
        ],
      }),
    );
    writeFileSync(
      sidecarPath,
      JSON.stringify({
        id: "provider-e2e-generated",
        status: "completed",
        model: {
          provider: "replicate",
          id: "black-forest-labs/flux-schnell",
        },
        outputs: [
          {
            mediaId: "provider-e2e-output",
            relativePath: "generated/provider-e2e-output.png",
          },
        ],
      }),
    );
    writeFileSync(
      fakeCargo,
      [
        "#!/bin/sh",
        `printf '%s\\n' '{"ok":true,"provider":"replicate","model":"black-forest-labs/flux-schnell","projectDir":"${projectDir}","artifactPath":"${artifactPath}","outputCount":1,"jobStatus":"Completed"}'`,
      ].join("\n"),
      { mode: 0o755 },
    );

    const outDir = join(tempDir, "provider-out");
    const result = spawnSync(
      process.execPath,
      [
        join(repoRoot, "scripts/provider-e2e.mjs"),
        "--provider",
        "replicate",
        "--live",
        "--out-dir",
        outDir,
      ],
      {
        cwd: repoRoot,
        encoding: "utf8",
        env: {
          ...process.env,
          PATH: `${tempDir}:${process.env.PATH ?? ""}`,
          REPLICATE_API_TOKEN: "super-secret-replicate-token",
          VIDEO_CREATER_PROVIDER_E2E_ALLOW_SPEND: "1",
        },
      },
    );

    expect(result.status).toBe(1);
    const report = JSON.parse(result.stdout) as {
      ok: boolean;
      error: string;
      result: { projectDir: string };
    };
    expect(report).toEqual(
      expect.objectContaining({
        ok: false,
        error: `Provider E2E generated asset sidecar output is missing a provider sourceUrl: ${sidecarPath}`,
        result: expect.objectContaining({ projectDir }),
      }),
    );
  });

  it("allows live OpenAI generated asset sidecar outputs without sourceUrl", () => {
    const tempDir = mkdtempSync(join(tmpdir(), "video-creater-openai-local-output-e2e-"));
    const fakeCargo = join(tempDir, "cargo");
    const reportPath = writeProviderPolicyFixture({
      rootDir: tempDir,
      provider: "openai",
      model: "gpt-image-2",
      requestId: "openai-live-request-1",
      sourceUrl: null,
    });
    const fixtureReport = JSON.parse(readFileSync(reportPath, "utf8")) as {
      result: unknown;
    };
    const argsPath = join(tempDir, "cargo-args.txt");
    const cargoResponsePath = join(tempDir, "cargo-response.json");
    writeFileSync(cargoResponsePath, `${JSON.stringify(fixtureReport.result)}\n`);
    writeFileSync(
      fakeCargo,
      [
        "#!/usr/bin/env node",
        "const fs = require('node:fs');",
        "fs.writeFileSync(process.env.VIDEO_CREATER_FAKE_CARGO_ARGS, process.argv.slice(2).join(' ') + '\\n');",
        "process.stdout.write(fs.readFileSync(process.env.VIDEO_CREATER_FAKE_CARGO_RESPONSE, 'utf8'));",
      ].join("\n") + "\n",
      { mode: 0o755 },
    );
    const outDir = join(tempDir, "provider-out");

    const result = spawnSync(
      process.execPath,
      [
        join(repoRoot, "scripts/provider-e2e.mjs"),
        "--provider",
        "openai",
        "--live",
        "--out-dir",
        outDir,
      ],
      {
        cwd: repoRoot,
        encoding: "utf8",
        env: {
          ...process.env,
          PATH: `${tempDir}:${process.env.PATH ?? ""}`,
          VIDEO_CREATER_FAKE_CARGO_ARGS: argsPath,
          VIDEO_CREATER_FAKE_CARGO_RESPONSE: cargoResponsePath,
          VIDEO_CREATER_PROVIDER_E2E_ALLOW_SPEND: "1",
          OPENAI_API_KEY: "super-secret-openai-token",
        },
      },
    );

    expect(result.status).toBe(0);
    expect(result.stdout).not.toContain("super-secret-openai-token");
    expect(result.stderr).not.toContain("super-secret-openai-token");
    const cargoArgs = readFileSync(argsPath, "utf8");
    expect(cargoArgs).toContain("--provider openai");
    expect(cargoArgs).toContain("--model gpt-image-2");
    const report = JSON.parse(result.stdout) as {
      ok: boolean;
      provider: string;
      credentialStatus: string;
      error: string | null;
      result: { provider: string; outputCount: number };
    };
    expect(report).toEqual(
      expect.objectContaining({
        ok: true,
        provider: "openai",
        credentialStatus: "present",
        error: null,
        result: expect.objectContaining({
          provider: "openai",
          outputCount: 1,
        }),
      }),
    );
  });

  it("fails closed when the provider E2E generated asset sidecar output sourceUrl is not a URL", () => {
    const tempDir = mkdtempSync(join(tmpdir(), "video-creater-provider-e2e-"));
    const fakeCargo = join(tempDir, "cargo");
    const projectDir = join(tempDir, "project");
    const artifactPath = join(projectDir, "generated", "provider-e2e-output.png");
    const sidecarPath = join(
      projectDir,
      "generated",
      "provider-e2e-generated",
      "asset.json",
    );
    mkdirSync(join(projectDir, "generated", "provider-e2e-generated"), {
      recursive: true,
    });
    mkdirSync(join(projectDir, "media"));
    writeFileSync(artifactPath, "generated pixels");
    writeFileSync(
      join(projectDir, "video-creater.project.json"),
      JSON.stringify({
        schemaVersion: 1,
        id: "provider-e2e-project",
        files: { generated: "generated", media: "media/index.json" },
      }),
    );
    writeFileSync(
      join(projectDir, "media", "index.json"),
      JSON.stringify({
        schemaVersion: 1,
        folders: [],
        assets: [
          {
            id: "provider-e2e-output",
            relativePath: "generated/provider-e2e-output.png",
            kind: "generated",
            durationSeconds: 0,
            width: 1024,
            height: 768,
            fps: 0,
          },
        ],
        analysis: [],
      }),
    );
    writeFileSync(
      join(projectDir, "generated", "index.json"),
      JSON.stringify({
        schemaVersion: 1,
        assets: [
          {
            assetId: "provider-e2e-generated",
            path: "generated/provider-e2e-generated/asset.json",
            status: "completed",
            modelProvider: "replicate",
            modelId: "black-forest-labs/flux-schnell",
            outputCount: 1,
          },
        ],
      }),
    );
    writeFileSync(
      sidecarPath,
      JSON.stringify({
        id: "provider-e2e-generated",
        status: "completed",
        model: {
          provider: "replicate",
          id: "black-forest-labs/flux-schnell",
        },
        outputs: [
          {
            mediaId: "provider-e2e-output",
            relativePath: "generated/provider-e2e-output.png",
            sourceUrl: "provider-output.png",
            width: 1024,
            height: 768,
            durationSeconds: 0,
            fps: 0,
          },
        ],
      }),
    );
    writeFileSync(
      fakeCargo,
      [
        "#!/bin/sh",
        `printf '%s\\n' '{"ok":true,"provider":"replicate","model":"black-forest-labs/flux-schnell","projectDir":"${projectDir}","artifactPath":"${artifactPath}","outputCount":1,"jobStatus":"Completed"}'`,
      ].join("\n"),
      { mode: 0o755 },
    );

    const outDir = join(tempDir, "provider-out");
    const result = spawnSync(
      process.execPath,
      [
        join(repoRoot, "scripts/provider-e2e.mjs"),
        "--provider",
        "replicate",
        "--live",
        "--out-dir",
        outDir,
      ],
      {
        cwd: repoRoot,
        encoding: "utf8",
        env: {
          ...process.env,
          PATH: `${tempDir}:${process.env.PATH ?? ""}`,
          REPLICATE_API_TOKEN: "super-secret-replicate-token",
          VIDEO_CREATER_PROVIDER_E2E_ALLOW_SPEND: "1",
        },
      },
    );

    expect(result.status).toBe(1);
    const report = JSON.parse(result.stdout) as {
      ok: boolean;
      error: string;
      result: { projectDir: string };
    };
    expect(report).toEqual(
      expect.objectContaining({
        ok: false,
        error: `Provider E2E generated asset sidecar output sourceUrl is not a provider URL: ${sidecarPath}`,
        result: expect.objectContaining({ projectDir }),
      }),
    );
  });

  it("fails closed when the provider E2E generated asset sidecar output sourceUrl is local", () => {
    const tempDir = mkdtempSync(join(tmpdir(), "video-creater-provider-e2e-"));
    const fakeCargo = join(tempDir, "cargo");
    const projectDir = join(tempDir, "project");
    const artifactPath = join(projectDir, "generated", "provider-e2e-output.png");
    const sidecarPath = join(
      projectDir,
      "generated",
      "provider-e2e-generated",
      "asset.json",
    );
    mkdirSync(join(projectDir, "generated", "provider-e2e-generated"), {
      recursive: true,
    });
    mkdirSync(join(projectDir, "media"));
    writeFileSync(artifactPath, "generated pixels");
    writeFileSync(
      join(projectDir, "video-creater.project.json"),
      JSON.stringify({
        schemaVersion: 1,
        id: "provider-e2e-project",
        files: { generated: "generated", media: "media/index.json" },
      }),
    );
    writeFileSync(
      join(projectDir, "media", "index.json"),
      JSON.stringify({
        schemaVersion: 1,
        folders: [],
        assets: [
          {
            id: "provider-e2e-output",
            relativePath: "generated/provider-e2e-output.png",
            kind: "generated",
            durationSeconds: 0,
            width: 1024,
            height: 768,
            fps: 0,
          },
        ],
        analysis: [],
      }),
    );
    writeFileSync(
      join(projectDir, "generated", "index.json"),
      JSON.stringify({
        schemaVersion: 1,
        assets: [
          {
            assetId: "provider-e2e-generated",
            path: "generated/provider-e2e-generated/asset.json",
            status: "completed",
            modelProvider: "replicate",
            modelId: "black-forest-labs/flux-schnell",
            outputCount: 1,
          },
        ],
      }),
    );
    writeFileSync(
      sidecarPath,
      JSON.stringify({
        id: "provider-e2e-generated",
        status: "completed",
        model: {
          provider: "replicate",
          id: "black-forest-labs/flux-schnell",
        },
        outputs: [
          {
            mediaId: "provider-e2e-output",
            relativePath: "generated/provider-e2e-output.png",
            sourceUrl: "http://127.0.0.1:4123/generated-output.png",
            width: 1024,
            height: 768,
            durationSeconds: 0,
            fps: 0,
          },
        ],
      }),
    );
    writeFileSync(
      fakeCargo,
      [
        "#!/bin/sh",
        `printf '%s\\n' '{"ok":true,"provider":"replicate","model":"black-forest-labs/flux-schnell","projectDir":"${projectDir}","artifactPath":"${artifactPath}","outputCount":1,"jobStatus":"Completed"}'`,
      ].join("\n"),
      { mode: 0o755 },
    );

    const outDir = join(tempDir, "provider-out");
    const result = spawnSync(
      process.execPath,
      [
        join(repoRoot, "scripts/provider-e2e.mjs"),
        "--provider",
        "replicate",
        "--live",
        "--out-dir",
        outDir,
      ],
      {
        cwd: repoRoot,
        encoding: "utf8",
        env: {
          ...process.env,
          PATH: `${tempDir}:${process.env.PATH ?? ""}`,
          REPLICATE_API_TOKEN: "super-secret-replicate-token",
          VIDEO_CREATER_PROVIDER_E2E_ALLOW_SPEND: "1",
        },
      },
    );

    expect(result.status).toBe(1);
    const report = JSON.parse(result.stdout) as {
      ok: boolean;
      error: string;
      result: { projectDir: string };
    };
    expect(report).toEqual(
      expect.objectContaining({
        ok: false,
        error: `Provider E2E generated asset sidecar output sourceUrl is not a provider URL: ${sidecarPath}`,
        result: expect.objectContaining({ projectDir }),
      }),
    );
  });

  it("fails closed when the provider E2E generated asset sidecar output is missing a mediaId", () => {
    const tempDir = mkdtempSync(join(tmpdir(), "video-creater-provider-e2e-"));
    const fakeCargo = join(tempDir, "cargo");
    const projectDir = join(tempDir, "project");
    const artifactPath = join(projectDir, "generated", "provider-e2e-output.png");
    const sidecarPath = join(
      projectDir,
      "generated",
      "provider-e2e-generated",
      "asset.json",
    );
    mkdirSync(join(projectDir, "generated", "provider-e2e-generated"), {
      recursive: true,
    });
    writeFileSync(artifactPath, "generated pixels");
    writeFileSync(
      join(projectDir, "video-creater.project.json"),
      JSON.stringify({
        schemaVersion: 1,
        id: "provider-e2e-project",
        files: { generated: "generated" },
      }),
    );
    writeFileSync(
      join(projectDir, "generated", "index.json"),
      JSON.stringify({
        schemaVersion: 1,
        assets: [
          {
            assetId: "provider-e2e-generated",
            path: "generated/provider-e2e-generated/asset.json",
            status: "completed",
            modelProvider: "replicate",
            modelId: "black-forest-labs/flux-schnell",
            outputCount: 1,
          },
        ],
      }),
    );
    writeFileSync(
      sidecarPath,
      JSON.stringify({
        id: "provider-e2e-generated",
        status: "completed",
        model: {
          provider: "replicate",
          id: "black-forest-labs/flux-schnell",
        },
        outputs: [
          {
            relativePath: "generated/provider-e2e-output.png",
            sourceUrl: "https://provider.example/generated-output.png",
          },
        ],
      }),
    );
    writeFileSync(
      fakeCargo,
      [
        "#!/bin/sh",
        `printf '%s\\n' '{"ok":true,"provider":"replicate","model":"black-forest-labs/flux-schnell","projectDir":"${projectDir}","artifactPath":"${artifactPath}","outputCount":1,"jobStatus":"Completed"}'`,
      ].join("\n"),
      { mode: 0o755 },
    );

    const outDir = join(tempDir, "provider-out");
    const result = spawnSync(
      process.execPath,
      [
        join(repoRoot, "scripts/provider-e2e.mjs"),
        "--provider",
        "replicate",
        "--live",
        "--out-dir",
        outDir,
      ],
      {
        cwd: repoRoot,
        encoding: "utf8",
        env: {
          ...process.env,
          PATH: `${tempDir}:${process.env.PATH ?? ""}`,
          REPLICATE_API_TOKEN: "super-secret-replicate-token",
          VIDEO_CREATER_PROVIDER_E2E_ALLOW_SPEND: "1",
        },
      },
    );

    expect(result.status).toBe(1);
    const report = JSON.parse(result.stdout) as {
      ok: boolean;
      error: string;
      result: { projectDir: string };
    };
    expect(report).toEqual(
      expect.objectContaining({
        ok: false,
        error: `Provider E2E generated asset sidecar output is missing a mediaId: ${sidecarPath}`,
        result: expect.objectContaining({ projectDir }),
      }),
    );
  });

  it("fails closed when the provider E2E generated asset sidecar output is missing media metadata", () => {
    const tempDir = mkdtempSync(join(tmpdir(), "video-creater-provider-e2e-"));
    const fakeCargo = join(tempDir, "cargo");
    const projectDir = join(tempDir, "project");
    const artifactPath = join(projectDir, "generated", "provider-e2e-output.png");
    const sidecarPath = join(
      projectDir,
      "generated",
      "provider-e2e-generated",
      "asset.json",
    );
    mkdirSync(join(projectDir, "generated", "provider-e2e-generated"), {
      recursive: true,
    });
    writeFileSync(artifactPath, "generated pixels");
    writeFileSync(
      join(projectDir, "video-creater.project.json"),
      JSON.stringify({
        schemaVersion: 1,
        id: "provider-e2e-project",
        files: { generated: "generated" },
      }),
    );
    writeFileSync(
      join(projectDir, "generated", "index.json"),
      JSON.stringify({
        schemaVersion: 1,
        assets: [
          {
            assetId: "provider-e2e-generated",
            path: "generated/provider-e2e-generated/asset.json",
            status: "completed",
            modelProvider: "replicate",
            modelId: "black-forest-labs/flux-schnell",
            outputCount: 1,
          },
        ],
      }),
    );
    writeFileSync(
      sidecarPath,
      JSON.stringify({
        id: "provider-e2e-generated",
        status: "completed",
        model: {
          provider: "replicate",
          id: "black-forest-labs/flux-schnell",
        },
        outputs: [
          {
            mediaId: "provider-e2e-output",
            relativePath: "generated/provider-e2e-output.png",
            sourceUrl: "https://provider.example/generated-output.png",
          },
        ],
      }),
    );
    writeFileSync(
      fakeCargo,
      [
        "#!/bin/sh",
        `printf '%s\\n' '{"ok":true,"provider":"replicate","model":"black-forest-labs/flux-schnell","projectDir":"${projectDir}","artifactPath":"${artifactPath}","outputCount":1,"jobStatus":"Completed"}'`,
      ].join("\n"),
      { mode: 0o755 },
    );

    const outDir = join(tempDir, "provider-out");
    const result = spawnSync(
      process.execPath,
      [
        join(repoRoot, "scripts/provider-e2e.mjs"),
        "--provider",
        "replicate",
        "--live",
        "--out-dir",
        outDir,
      ],
      {
        cwd: repoRoot,
        encoding: "utf8",
        env: {
          ...process.env,
          PATH: `${tempDir}:${process.env.PATH ?? ""}`,
          REPLICATE_API_TOKEN: "super-secret-replicate-token",
          VIDEO_CREATER_PROVIDER_E2E_ALLOW_SPEND: "1",
        },
      },
    );

    expect(result.status).toBe(1);
    const report = JSON.parse(result.stdout) as {
      ok: boolean;
      error: string;
      result: { projectDir: string };
    };
    expect(report).toEqual(
      expect.objectContaining({
        ok: false,
        error: `Provider E2E generated asset sidecar output is missing media metadata: ${sidecarPath}`,
        result: expect.objectContaining({ projectDir }),
      }),
    );
  });

  it("fails closed when the provider E2E generated asset sidecar output file is missing", () => {
    const tempDir = mkdtempSync(join(tmpdir(), "video-creater-provider-e2e-"));
    const fakeCargo = join(tempDir, "cargo");
    const artifactPath = join(tempDir, "generated", "replicate-output.png");
    const projectDir = join(tempDir, "project");
    const sidecarPath = join(
      projectDir,
      "generated",
      "provider-e2e-generated",
      "asset.json",
    );
    const missingOutputPath = join(projectDir, "generated", "missing-output.png");
    mkdirSync(join(tempDir, "generated"));
    mkdirSync(join(projectDir, "generated", "provider-e2e-generated"), {
      recursive: true,
    });
    writeFileSync(artifactPath, "generated pixels");
    writeFileSync(
      join(projectDir, "video-creater.project.json"),
      JSON.stringify({
        schemaVersion: 1,
        id: "provider-e2e-project",
        files: { generated: "generated" },
      }),
    );
    writeFileSync(
      join(projectDir, "generated", "index.json"),
      JSON.stringify({
        schemaVersion: 1,
        assets: [
          {
            assetId: "provider-e2e-generated",
            path: "generated/provider-e2e-generated/asset.json",
            status: "completed",
            modelProvider: "replicate",
            modelId: "black-forest-labs/flux-schnell",
            outputCount: 1,
          },
        ],
      }),
    );
    writeFileSync(
      sidecarPath,
      JSON.stringify({
        id: "provider-e2e-generated",
        status: "completed",
        model: {
          provider: "replicate",
          id: "black-forest-labs/flux-schnell",
        },
        outputs: [{ mediaId: "provider-e2e-output", relativePath: "generated/missing-output.png", sourceUrl: "https://provider.example/generated-output.png", width: 1024, height: 768, durationSeconds: 0, fps: 0 }],
      }),
    );
    writeFileSync(
      fakeCargo,
      [
        "#!/bin/sh",
        `printf '%s\\n' '{"ok":true,"provider":"replicate","model":"black-forest-labs/flux-schnell","projectDir":"${projectDir}","artifactPath":"${artifactPath}","outputCount":1,"jobStatus":"Completed"}'`,
      ].join("\n"),
      { mode: 0o755 },
    );

    const outDir = join(tempDir, "provider-out");
    const result = spawnSync(
      process.execPath,
      [
        join(repoRoot, "scripts/provider-e2e.mjs"),
        "--provider",
        "replicate",
        "--live",
        "--out-dir",
        outDir,
      ],
      {
        cwd: repoRoot,
        encoding: "utf8",
        env: {
          ...process.env,
          PATH: `${tempDir}:${process.env.PATH ?? ""}`,
          REPLICATE_API_TOKEN: "super-secret-replicate-token",
          VIDEO_CREATER_PROVIDER_E2E_ALLOW_SPEND: "1",
        },
      },
    );

    expect(result.status).toBe(1);
    const report = JSON.parse(result.stdout) as {
      ok: boolean;
      error: string;
      result: { projectDir: string };
    };
    expect(report).toEqual(
      expect.objectContaining({
        ok: false,
        error: `Provider E2E generated asset sidecar output file is missing: ${missingOutputPath}`,
        result: expect.objectContaining({ projectDir }),
      }),
    );
  });

  it("fails closed when the provider E2E generated asset sidecar output escapes the split project", () => {
    const tempDir = mkdtempSync(join(tmpdir(), "video-creater-provider-e2e-"));
    const fakeCargo = join(tempDir, "cargo");
    const projectDir = join(tempDir, "project");
    const escapedOutputPath = join(tempDir, "escaped-provider-output.png");
    const sidecarPath = join(
      projectDir,
      "generated",
      "provider-e2e-generated",
      "asset.json",
    );
    mkdirSync(join(projectDir, "generated", "provider-e2e-generated"), {
      recursive: true,
    });
    mkdirSync(join(projectDir, "media"));
    writeFileSync(escapedOutputPath, "escaped generated pixels");
    writeFileSync(
      join(projectDir, "video-creater.project.json"),
      JSON.stringify({
        schemaVersion: 1,
        id: "provider-e2e-project",
        files: { generated: "generated", media: "media/index.json" },
      }),
    );
    writeFileSync(
      join(projectDir, "media", "index.json"),
      JSON.stringify({
        schemaVersion: 1,
        folders: [],
        assets: [
          {
            id: "provider-e2e-output",
            relativePath: "../escaped-provider-output.png",
            kind: "generated",
            durationSeconds: 0,
            width: 1024,
            height: 768,
            fps: 0,
          },
        ],
        analysis: [],
      }),
    );
    writeFileSync(
      join(projectDir, "generated", "index.json"),
      JSON.stringify({
        schemaVersion: 1,
        assets: [
          {
            assetId: "provider-e2e-generated",
            path: "generated/provider-e2e-generated/asset.json",
            status: "completed",
            modelProvider: "replicate",
            modelId: "black-forest-labs/flux-schnell",
            outputCount: 1,
          },
        ],
      }),
    );
    writeFileSync(
      sidecarPath,
      JSON.stringify({
        id: "provider-e2e-generated",
        status: "completed",
        model: {
          provider: "replicate",
          id: "black-forest-labs/flux-schnell",
        },
        outputs: [
          {
            mediaId: "provider-e2e-output",
            relativePath: "../escaped-provider-output.png",
            sourceUrl: "https://provider.example/generated-output.png",
            width: 1024,
            height: 768,
            durationSeconds: 0,
            fps: 0,
          },
        ],
      }),
    );
    writeFileSync(
      fakeCargo,
      [
        "#!/bin/sh",
        `printf '%s\\n' '{"ok":true,"provider":"replicate","model":"black-forest-labs/flux-schnell","projectDir":"${projectDir}","artifactPath":"${escapedOutputPath}","outputCount":1,"jobStatus":"Completed"}'`,
      ].join("\n"),
      { mode: 0o755 },
    );

    const outDir = join(tempDir, "provider-out");
    const result = spawnSync(
      process.execPath,
      [
        join(repoRoot, "scripts/provider-e2e.mjs"),
        "--provider",
        "replicate",
        "--live",
        "--out-dir",
        outDir,
      ],
      {
        cwd: repoRoot,
        encoding: "utf8",
        env: {
          ...process.env,
          PATH: `${tempDir}:${process.env.PATH ?? ""}`,
          REPLICATE_API_TOKEN: "super-secret-replicate-token",
          VIDEO_CREATER_PROVIDER_E2E_ALLOW_SPEND: "1",
        },
      },
    );

    expect(result.status).toBe(1);
    const report = JSON.parse(result.stdout) as {
      ok: boolean;
      error: string;
      result: { projectDir: string };
    };
    expect(report).toEqual(
      expect.objectContaining({
        ok: false,
        error: "Provider E2E generated asset sidecar output path escapes the split project: ../escaped-provider-output.png",
        result: expect.objectContaining({ projectDir }),
      }),
    );
  });

  it("fails closed when the provider E2E generated asset sidecar output is outside the generated directory", () => {
    const tempDir = mkdtempSync(join(tmpdir(), "video-creater-provider-e2e-"));
    const fakeCargo = join(tempDir, "cargo");
    const projectDir = join(tempDir, "project");
    const outputPath = join(projectDir, "media", "provider-e2e-output.png");
    const sidecarPath = join(
      projectDir,
      "generated",
      "provider-e2e-generated",
      "asset.json",
    );
    mkdirSync(join(projectDir, "generated", "provider-e2e-generated"), {
      recursive: true,
    });
    mkdirSync(join(projectDir, "media"));
    writeFileSync(outputPath, "media generated pixels");
    writeFileSync(
      join(projectDir, "video-creater.project.json"),
      JSON.stringify({
        schemaVersion: 1,
        id: "provider-e2e-project",
        files: { generated: "generated", media: "media/index.json" },
      }),
    );
    writeFileSync(
      join(projectDir, "media", "index.json"),
      JSON.stringify({
        schemaVersion: 1,
        folders: [],
        assets: [
          {
            id: "provider-e2e-output",
            relativePath: "media/provider-e2e-output.png",
            kind: "generated",
            durationSeconds: 0,
            width: 1024,
            height: 768,
            fps: 0,
          },
        ],
        analysis: [],
      }),
    );
    writeFileSync(
      join(projectDir, "generated", "index.json"),
      JSON.stringify({
        schemaVersion: 1,
        assets: [
          {
            assetId: "provider-e2e-generated",
            path: "generated/provider-e2e-generated/asset.json",
            status: "completed",
            modelProvider: "replicate",
            modelId: "black-forest-labs/flux-schnell",
            outputCount: 1,
          },
        ],
      }),
    );
    writeFileSync(
      sidecarPath,
      JSON.stringify({
        id: "provider-e2e-generated",
        status: "completed",
        model: {
          provider: "replicate",
          id: "black-forest-labs/flux-schnell",
        },
        outputs: [
          {
            mediaId: "provider-e2e-output",
            relativePath: "media/provider-e2e-output.png",
            sourceUrl: "https://provider.example/generated-output.png",
            width: 1024,
            height: 768,
            durationSeconds: 0,
            fps: 0,
          },
        ],
      }),
    );
    writeFileSync(
      fakeCargo,
      [
        "#!/bin/sh",
        `printf '%s\\n' '{"ok":true,"provider":"replicate","model":"black-forest-labs/flux-schnell","projectDir":"${projectDir}","artifactPath":"${outputPath}","outputCount":1,"jobStatus":"Completed"}'`,
      ].join("\n"),
      { mode: 0o755 },
    );

    const outDir = join(tempDir, "provider-out");
    const result = spawnSync(
      process.execPath,
      [
        join(repoRoot, "scripts/provider-e2e.mjs"),
        "--provider",
        "replicate",
        "--live",
        "--out-dir",
        outDir,
      ],
      {
        cwd: repoRoot,
        encoding: "utf8",
        env: {
          ...process.env,
          PATH: `${tempDir}:${process.env.PATH ?? ""}`,
          REPLICATE_API_TOKEN: "super-secret-replicate-token",
          VIDEO_CREATER_PROVIDER_E2E_ALLOW_SPEND: "1",
        },
      },
    );

    expect(result.status).toBe(1);
    const report = JSON.parse(result.stdout) as {
      ok: boolean;
      error: string;
      result: { projectDir: string };
    };
    expect(report).toEqual(
      expect.objectContaining({
        ok: false,
        error: "Provider E2E generated asset sidecar output is outside the generated asset directory: media/provider-e2e-output.png",
        result: expect.objectContaining({ projectDir }),
      }),
    );
  });

  it("fails closed when the provider E2E artifact is not one of the generated sidecar outputs", () => {
    const tempDir = mkdtempSync(join(tmpdir(), "video-creater-provider-e2e-"));
    const fakeCargo = join(tempDir, "cargo");
    const artifactPath = join(tempDir, "generated", "untracked-output.png");
    const projectDir = join(tempDir, "project");
    const sidecarPath = join(
      projectDir,
      "generated",
      "provider-e2e-generated",
      "asset.json",
    );
    mkdirSync(join(tempDir, "generated"));
    mkdirSync(join(projectDir, "generated", "provider-e2e-generated"), {
      recursive: true,
    });
    writeFileSync(artifactPath, "untracked pixels");
    writeFileSync(
      join(projectDir, "video-creater.project.json"),
      JSON.stringify({
        schemaVersion: 1,
        id: "provider-e2e-project",
        files: { generated: "generated" },
      }),
    );
    writeFileSync(
      join(projectDir, "generated", "index.json"),
      JSON.stringify({
        schemaVersion: 1,
        assets: [
          {
            assetId: "provider-e2e-generated",
            path: "generated/provider-e2e-generated/asset.json",
            status: "completed",
            modelProvider: "replicate",
            modelId: "black-forest-labs/flux-schnell",
            outputCount: 1,
          },
        ],
      }),
    );
    writeFileSync(
      sidecarPath,
      JSON.stringify({
        id: "provider-e2e-generated",
        status: "completed",
        model: {
          provider: "replicate",
          id: "black-forest-labs/flux-schnell",
        },
        outputs: [{ mediaId: "provider-e2e-output", relativePath: "generated/provider-e2e-output.png", sourceUrl: "https://provider.example/generated-output.png", width: 1024, height: 768, durationSeconds: 0, fps: 0 }],
      }),
    );
    writeFileSync(
      join(projectDir, "generated", "provider-e2e-output.png"),
      "project generated pixels",
    );
    writeFileSync(
      fakeCargo,
      [
        "#!/bin/sh",
        `printf '%s\\n' '{"ok":true,"provider":"replicate","model":"black-forest-labs/flux-schnell","projectDir":"${projectDir}","artifactPath":"${artifactPath}","outputCount":1,"jobStatus":"Completed"}'`,
      ].join("\n"),
      { mode: 0o755 },
    );

    const outDir = join(tempDir, "provider-out");
    const result = spawnSync(
      process.execPath,
      [
        join(repoRoot, "scripts/provider-e2e.mjs"),
        "--provider",
        "replicate",
        "--live",
        "--out-dir",
        outDir,
      ],
      {
        cwd: repoRoot,
        encoding: "utf8",
        env: {
          ...process.env,
          PATH: `${tempDir}:${process.env.PATH ?? ""}`,
          REPLICATE_API_TOKEN: "super-secret-replicate-token",
          VIDEO_CREATER_PROVIDER_E2E_ALLOW_SPEND: "1",
        },
      },
    );

    expect(result.status).toBe(1);
    const report = JSON.parse(result.stdout) as {
      ok: boolean;
      error: string;
      result: { projectDir: string };
    };
    expect(report).toEqual(
      expect.objectContaining({
        ok: false,
        error: `Provider E2E artifact is not recorded in generated asset sidecar outputs: ${artifactPath}`,
        result: expect.objectContaining({ projectDir }),
      }),
    );
  });

  it("fails closed when the provider E2E generated asset index output count does not match the sidecar", () => {
    const tempDir = mkdtempSync(join(tmpdir(), "video-creater-provider-e2e-"));
    const fakeCargo = join(tempDir, "cargo");
    const projectDir = join(tempDir, "project");
    const artifactPath = join(projectDir, "generated", "provider-e2e-output.png");
    const sidecarPath = join(
      projectDir,
      "generated",
      "provider-e2e-generated",
      "asset.json",
    );
    mkdirSync(join(projectDir, "generated", "provider-e2e-generated"), {
      recursive: true,
    });
    writeFileSync(artifactPath, "project generated pixels");
    writeFileSync(
      join(projectDir, "video-creater.project.json"),
      JSON.stringify({
        schemaVersion: 1,
        id: "provider-e2e-project",
        files: { generated: "generated" },
      }),
    );
    writeFileSync(
      join(projectDir, "generated", "index.json"),
      JSON.stringify({
        schemaVersion: 1,
        assets: [
          {
            assetId: "provider-e2e-generated",
            path: "generated/provider-e2e-generated/asset.json",
            status: "completed",
            modelProvider: "replicate",
            modelId: "black-forest-labs/flux-schnell",
            outputCount: 2,
          },
        ],
      }),
    );
    writeFileSync(
      sidecarPath,
      JSON.stringify({
        id: "provider-e2e-generated",
        status: "completed",
        model: {
          provider: "replicate",
          id: "black-forest-labs/flux-schnell",
        },
        outputs: [{ mediaId: "provider-e2e-output", relativePath: "generated/provider-e2e-output.png", sourceUrl: "https://provider.example/generated-output.png", width: 1024, height: 768, durationSeconds: 0, fps: 0 }],
      }),
    );
    writeFileSync(
      fakeCargo,
      [
        "#!/bin/sh",
        `printf '%s\\n' '{"ok":true,"provider":"replicate","model":"black-forest-labs/flux-schnell","projectDir":"${projectDir}","artifactPath":"${artifactPath}","outputCount":1,"jobStatus":"Completed"}'`,
      ].join("\n"),
      { mode: 0o755 },
    );

    const outDir = join(tempDir, "provider-out");
    const result = spawnSync(
      process.execPath,
      [
        join(repoRoot, "scripts/provider-e2e.mjs"),
        "--provider",
        "replicate",
        "--live",
        "--out-dir",
        outDir,
      ],
      {
        cwd: repoRoot,
        encoding: "utf8",
        env: {
          ...process.env,
          PATH: `${tempDir}:${process.env.PATH ?? ""}`,
          REPLICATE_API_TOKEN: "super-secret-replicate-token",
          VIDEO_CREATER_PROVIDER_E2E_ALLOW_SPEND: "1",
        },
      },
    );

    expect(result.status).toBe(1);
    const report = JSON.parse(result.stdout) as {
      ok: boolean;
      error: string;
      result: { projectDir: string };
    };
    expect(report).toEqual(
      expect.objectContaining({
        ok: false,
        error: `Provider E2E generated asset index output count does not match sidecar outputs: ${sidecarPath}`,
        result: expect.objectContaining({ projectDir }),
      }),
    );
  });

  it("fails closed when provider E2E generated index entries duplicate a completed sidecar output", () => {
    const tempDir = mkdtempSync(join(tmpdir(), "video-creater-provider-e2e-"));
    const fakeCargo = join(tempDir, "cargo");
    const projectDir = join(tempDir, "project");
    const artifactPath = join(projectDir, "generated", "provider-e2e-output.png");
    const firstSidecarPath = join(
      projectDir,
      "generated",
      "provider-e2e-generated-one",
      "asset.json",
    );
    const secondSidecarPath = join(
      projectDir,
      "generated",
      "provider-e2e-generated-two",
      "asset.json",
    );
    mkdirSync(join(projectDir, "generated", "provider-e2e-generated-one"), {
      recursive: true,
    });
    mkdirSync(join(projectDir, "generated", "provider-e2e-generated-two"), {
      recursive: true,
    });
    mkdirSync(join(projectDir, "media"));
    writeFileSync(artifactPath, "project generated pixels");
    writeFileSync(
      join(projectDir, "video-creater.project.json"),
      JSON.stringify({
        schemaVersion: 1,
        id: "provider-e2e-project",
        files: { generated: "generated", media: "media/index.json" },
      }),
    );
    writeFileSync(
      join(projectDir, "media", "index.json"),
      JSON.stringify({
        schemaVersion: 1,
        folders: [],
        assets: [
          {
            id: "provider-e2e-output",
            relativePath: "generated/provider-e2e-output.png",
            kind: "generated",
            durationSeconds: 0,
            width: 1024,
            height: 768,
            fps: 0,
          },
        ],
        analysis: [],
      }),
    );
    writeFileSync(
      join(projectDir, "generated", "index.json"),
      JSON.stringify({
        schemaVersion: 1,
        assets: [
          {
            assetId: "provider-e2e-generated-one",
            path: "generated/provider-e2e-generated-one/asset.json",
            status: "completed",
            modelProvider: "replicate",
            modelId: "black-forest-labs/flux-schnell",
            outputCount: 1,
          },
          {
            assetId: "provider-e2e-generated-two",
            path: "generated/provider-e2e-generated-two/asset.json",
            status: "completed",
            modelProvider: "replicate",
            modelId: "black-forest-labs/flux-schnell",
            outputCount: 1,
          },
        ],
      }),
    );
    writeFileSync(
      firstSidecarPath,
      JSON.stringify({
        id: "provider-e2e-generated-one",
        status: "completed",
        model: {
          provider: "replicate",
          id: "black-forest-labs/flux-schnell",
        },
        outputs: [
          {
            mediaId: "provider-e2e-output",
            relativePath: "generated/provider-e2e-output.png",
            sourceUrl: "https://provider.example/generated-output.png",
            width: 1024,
            height: 768,
            durationSeconds: 0,
            fps: 0,
          },
        ],
      }),
    );
    writeFileSync(
      secondSidecarPath,
      JSON.stringify({
        id: "provider-e2e-generated-two",
        status: "completed",
        model: {
          provider: "replicate",
          id: "black-forest-labs/flux-schnell",
        },
        outputs: [
          {
            mediaId: "provider-e2e-output",
            relativePath: "generated/provider-e2e-output.png",
            sourceUrl: "https://provider.example/generated-output.png",
            width: 1024,
            height: 768,
            durationSeconds: 0,
            fps: 0,
          },
        ],
      }),
    );
    writeFileSync(
      fakeCargo,
      [
        "#!/bin/sh",
        `printf '%s\\n' '{"ok":true,"provider":"replicate","model":"black-forest-labs/flux-schnell","projectDir":"${projectDir}","artifactPath":"${artifactPath}","outputCount":2,"jobStatus":"Completed"}'`,
      ].join("\n"),
      { mode: 0o755 },
    );

    const outDir = join(tempDir, "provider-out");
    const result = spawnSync(
      process.execPath,
      [
        join(repoRoot, "scripts/provider-e2e.mjs"),
        "--provider",
        "replicate",
        "--live",
        "--out-dir",
        outDir,
      ],
      {
        cwd: repoRoot,
        encoding: "utf8",
        env: {
          ...process.env,
          PATH: `${tempDir}:${process.env.PATH ?? ""}`,
          REPLICATE_API_TOKEN: "super-secret-replicate-token",
          VIDEO_CREATER_PROVIDER_E2E_ALLOW_SPEND: "1",
        },
      },
    );

    expect(result.status).toBe(1);
    const report = JSON.parse(result.stdout) as {
      ok: boolean;
      error: string;
      result: { projectDir: string };
    };
    expect(report).toEqual(
      expect.objectContaining({
        ok: false,
        error: "Provider E2E generated sidecar output mediaId is duplicated across completed sidecars: provider-e2e-output",
        result: expect.objectContaining({ projectDir }),
      }),
    );
  });

  it("fails closed when provider E2E generated index entries duplicate an asset id across sidecars", () => {
    const tempDir = mkdtempSync(join(tmpdir(), "video-creater-provider-e2e-"));
    const fakeCargo = join(tempDir, "cargo");
    const projectDir = join(tempDir, "project");
    const artifactPath = join(projectDir, "generated", "provider-e2e-output-one.png");
    const firstSidecarPath = join(
      projectDir,
      "generated",
      "provider-e2e-generated-one",
      "asset.json",
    );
    const secondSidecarPath = join(
      projectDir,
      "generated",
      "provider-e2e-generated-two",
      "asset.json",
    );
    mkdirSync(join(projectDir, "generated", "provider-e2e-generated-one"), {
      recursive: true,
    });
    mkdirSync(join(projectDir, "generated", "provider-e2e-generated-two"), {
      recursive: true,
    });
    mkdirSync(join(projectDir, "media"));
    writeFileSync(artifactPath, "project generated pixels one");
    writeFileSync(
      join(projectDir, "generated", "provider-e2e-output-two.png"),
      "project generated pixels two",
    );
    writeFileSync(
      join(projectDir, "video-creater.project.json"),
      JSON.stringify({
        schemaVersion: 1,
        id: "provider-e2e-project",
        files: { generated: "generated", media: "media/index.json" },
      }),
    );
    writeFileSync(
      join(projectDir, "media", "index.json"),
      JSON.stringify({
        schemaVersion: 1,
        folders: [],
        assets: [
          {
            id: "provider-e2e-output-one",
            relativePath: "generated/provider-e2e-output-one.png",
            kind: "generated",
            durationSeconds: 0,
            width: 1024,
            height: 768,
            fps: 0,
          },
          {
            id: "provider-e2e-output-two",
            relativePath: "generated/provider-e2e-output-two.png",
            kind: "generated",
            durationSeconds: 0,
            width: 1024,
            height: 768,
            fps: 0,
          },
        ],
        analysis: [],
      }),
    );
    writeFileSync(
      join(projectDir, "generated", "index.json"),
      JSON.stringify({
        schemaVersion: 1,
        assets: [
          {
            assetId: "provider-e2e-generated",
            path: "generated/provider-e2e-generated-one/asset.json",
            status: "completed",
            modelProvider: "replicate",
            modelId: "black-forest-labs/flux-schnell",
            outputCount: 1,
          },
          {
            assetId: "provider-e2e-generated",
            path: "generated/provider-e2e-generated-two/asset.json",
            status: "completed",
            modelProvider: "replicate",
            modelId: "black-forest-labs/flux-schnell",
            outputCount: 1,
          },
        ],
      }),
    );
    writeFileSync(
      firstSidecarPath,
      JSON.stringify({
        id: "provider-e2e-generated",
        status: "completed",
        model: {
          provider: "replicate",
          id: "black-forest-labs/flux-schnell",
        },
        outputs: [
          {
            mediaId: "provider-e2e-output-one",
            relativePath: "generated/provider-e2e-output-one.png",
            sourceUrl: "https://provider.example/generated-output-one.png",
            width: 1024,
            height: 768,
            durationSeconds: 0,
            fps: 0,
          },
        ],
      }),
    );
    writeFileSync(
      secondSidecarPath,
      JSON.stringify({
        id: "provider-e2e-generated",
        status: "completed",
        model: {
          provider: "replicate",
          id: "black-forest-labs/flux-schnell",
        },
        outputs: [
          {
            mediaId: "provider-e2e-output-two",
            relativePath: "generated/provider-e2e-output-two.png",
            sourceUrl: "https://provider.example/generated-output-two.png",
            width: 1024,
            height: 768,
            durationSeconds: 0,
            fps: 0,
          },
        ],
      }),
    );
    writeFileSync(
      fakeCargo,
      [
        "#!/bin/sh",
        `printf '%s\\n' '{"ok":true,"provider":"replicate","model":"black-forest-labs/flux-schnell","projectDir":"${projectDir}","artifactPath":"${artifactPath}","outputCount":2,"jobStatus":"Completed"}'`,
      ].join("\n"),
      { mode: 0o755 },
    );

    const outDir = join(tempDir, "provider-out");
    const result = spawnSync(
      process.execPath,
      [
        join(repoRoot, "scripts/provider-e2e.mjs"),
        "--provider",
        "replicate",
        "--live",
        "--out-dir",
        outDir,
      ],
      {
        cwd: repoRoot,
        encoding: "utf8",
        env: {
          ...process.env,
          PATH: `${tempDir}:${process.env.PATH ?? ""}`,
          REPLICATE_API_TOKEN: "super-secret-replicate-token",
          VIDEO_CREATER_PROVIDER_E2E_ALLOW_SPEND: "1",
        },
      },
    );

    expect(result.status).toBe(1);
    const report = JSON.parse(result.stdout) as {
      ok: boolean;
      error: string;
      result: { projectDir: string };
    };
    expect(report).toEqual(
      expect.objectContaining({
        ok: false,
        error: "Provider E2E generated asset index has duplicate assetId: provider-e2e-generated",
        result: expect.objectContaining({ projectDir }),
      }),
    );
  });

  it("fails closed when the provider E2E report output count does not match the sidecar outputs", () => {
    const tempDir = mkdtempSync(join(tmpdir(), "video-creater-provider-e2e-"));
    const fakeCargo = join(tempDir, "cargo");
    const projectDir = join(tempDir, "project");
    const artifactPath = join(projectDir, "generated", "provider-e2e-output.png");
    const sidecarPath = join(
      projectDir,
      "generated",
      "provider-e2e-generated",
      "asset.json",
    );
    mkdirSync(join(projectDir, "generated", "provider-e2e-generated"), {
      recursive: true,
    });
    writeFileSync(artifactPath, "project generated pixels");
    writeFileSync(
      join(projectDir, "video-creater.project.json"),
      JSON.stringify({
        schemaVersion: 1,
        id: "provider-e2e-project",
        files: { generated: "generated" },
      }),
    );
    writeFileSync(
      join(projectDir, "generated", "index.json"),
      JSON.stringify({
        schemaVersion: 1,
        assets: [
          {
            assetId: "provider-e2e-generated",
            path: "generated/provider-e2e-generated/asset.json",
            status: "completed",
            modelProvider: "replicate",
            modelId: "black-forest-labs/flux-schnell",
            outputCount: 1,
          },
        ],
      }),
    );
    writeFileSync(
      sidecarPath,
      JSON.stringify({
        id: "provider-e2e-generated",
        status: "completed",
        model: {
          provider: "replicate",
          id: "black-forest-labs/flux-schnell",
        },
        outputs: [{ mediaId: "provider-e2e-output", relativePath: "generated/provider-e2e-output.png", sourceUrl: "https://provider.example/generated-output.png", width: 1024, height: 768, durationSeconds: 0, fps: 0 }],
      }),
    );
    writeFileSync(
      fakeCargo,
      [
        "#!/bin/sh",
        `printf '%s\\n' '{"ok":true,"provider":"replicate","model":"black-forest-labs/flux-schnell","projectDir":"${projectDir}","artifactPath":"${artifactPath}","outputCount":2,"jobStatus":"Completed"}'`,
      ].join("\n"),
      { mode: 0o755 },
    );

    const outDir = join(tempDir, "provider-out");
    const result = spawnSync(
      process.execPath,
      [
        join(repoRoot, "scripts/provider-e2e.mjs"),
        "--provider",
        "replicate",
        "--live",
        "--out-dir",
        outDir,
      ],
      {
        cwd: repoRoot,
        encoding: "utf8",
        env: {
          ...process.env,
          PATH: `${tempDir}:${process.env.PATH ?? ""}`,
          REPLICATE_API_TOKEN: "super-secret-replicate-token",
          VIDEO_CREATER_PROVIDER_E2E_ALLOW_SPEND: "1",
        },
      },
    );

    expect(result.status).toBe(1);
    const report = JSON.parse(result.stdout) as {
      ok: boolean;
      error: string;
      result: { projectDir: string };
    };
    expect(report).toEqual(
      expect.objectContaining({
        ok: false,
        error: "Provider E2E report output count does not match generated sidecar outputs",
        result: expect.objectContaining({ projectDir }),
      }),
    );
  });

  it("fails closed when the provider E2E media index path escapes the split project", () => {
    const tempDir = mkdtempSync(join(tmpdir(), "video-creater-provider-e2e-"));
    const fakeCargo = join(tempDir, "cargo");
    const projectDir = join(tempDir, "project");
    const mediaIndexPath = join(tempDir, "external-media-index.json");
    const artifactPath = join(projectDir, "generated", "provider-e2e-output.png");
    const sidecarPath = join(
      projectDir,
      "generated",
      "provider-e2e-generated",
      "asset.json",
    );
    mkdirSync(join(projectDir, "generated", "provider-e2e-generated"), {
      recursive: true,
    });
    writeFileSync(artifactPath, "project generated pixels");
    writeFileSync(
      join(projectDir, "video-creater.project.json"),
      JSON.stringify({
        schemaVersion: 1,
        id: "provider-e2e-project",
        files: { generated: "generated", media: "../external-media-index.json" },
      }),
    );
    writeFileSync(
      join(projectDir, "generated", "index.json"),
      JSON.stringify({
        schemaVersion: 1,
        assets: [
          {
            assetId: "provider-e2e-generated",
            path: "generated/provider-e2e-generated/asset.json",
            status: "completed",
            modelProvider: "replicate",
            modelId: "black-forest-labs/flux-schnell",
            outputCount: 1,
          },
        ],
      }),
    );
    writeFileSync(
      sidecarPath,
      JSON.stringify({
        id: "provider-e2e-generated",
        status: "completed",
        model: {
          provider: "replicate",
          id: "black-forest-labs/flux-schnell",
        },
        outputs: [
          {
            mediaId: "provider-e2e-output",
            relativePath: "generated/provider-e2e-output.png",
            sourceUrl: "https://provider.example/generated-output.png",
            width: 1024,
            height: 768,
            durationSeconds: 0,
            fps: 0,
          },
        ],
      }),
    );
    writeFileSync(
      mediaIndexPath,
      JSON.stringify({
        schemaVersion: 1,
        folders: [],
        assets: [
          {
            id: "provider-e2e-output",
            relativePath: "generated/provider-e2e-output.png",
            kind: "generated",
            durationSeconds: 0,
            width: 1024,
            height: 768,
            fps: 0,
          },
        ],
        analysis: [],
      }),
    );
    writeFileSync(
      fakeCargo,
      [
        "#!/bin/sh",
        `printf '%s\\n' '{"ok":true,"provider":"replicate","model":"black-forest-labs/flux-schnell","projectDir":"${projectDir}","artifactPath":"${artifactPath}","outputCount":1,"jobStatus":"Completed"}'`,
      ].join("\n"),
      { mode: 0o755 },
    );

    const outDir = join(tempDir, "provider-out");
    const result = spawnSync(
      process.execPath,
      [
        join(repoRoot, "scripts/provider-e2e.mjs"),
        "--provider",
        "replicate",
        "--live",
        "--out-dir",
        outDir,
      ],
      {
        cwd: repoRoot,
        encoding: "utf8",
        env: {
          ...process.env,
          PATH: `${tempDir}:${process.env.PATH ?? ""}`,
          REPLICATE_API_TOKEN: "super-secret-replicate-token",
          VIDEO_CREATER_PROVIDER_E2E_ALLOW_SPEND: "1",
        },
      },
    );

    expect(result.status).toBe(1);
    const report = JSON.parse(result.stdout) as {
      ok: boolean;
      error: string;
      result: { projectDir: string };
    };
    expect(report).toEqual(
      expect.objectContaining({
        ok: false,
        error: "Provider E2E media index path escapes the split project: ../external-media-index.json",
        result: expect.objectContaining({ projectDir }),
      }),
    );
  });

  it("fails closed when the provider E2E generated sidecar output is missing from the media index", () => {
    const tempDir = mkdtempSync(join(tmpdir(), "video-creater-provider-e2e-"));
    const fakeCargo = join(tempDir, "cargo");
    const projectDir = join(tempDir, "project");
    const artifactPath = join(projectDir, "generated", "provider-e2e-output.png");
    const sidecarPath = join(
      projectDir,
      "generated",
      "provider-e2e-generated",
      "asset.json",
    );
    mkdirSync(join(projectDir, "generated", "provider-e2e-generated"), {
      recursive: true,
    });
    mkdirSync(join(projectDir, "media"));
    writeFileSync(artifactPath, "project generated pixels");
    writeFileSync(
      join(projectDir, "video-creater.project.json"),
      JSON.stringify({
        schemaVersion: 1,
        id: "provider-e2e-project",
        files: { generated: "generated", media: "media/index.json" },
      }),
    );
    writeFileSync(
      join(projectDir, "media", "index.json"),
      JSON.stringify({
        schemaVersion: 1,
        folders: [],
        assets: [],
        analysis: [],
      }),
    );
    writeFileSync(
      join(projectDir, "generated", "index.json"),
      JSON.stringify({
        schemaVersion: 1,
        assets: [
          {
            assetId: "provider-e2e-generated",
            path: "generated/provider-e2e-generated/asset.json",
            status: "completed",
            modelProvider: "replicate",
            modelId: "black-forest-labs/flux-schnell",
            outputCount: 1,
          },
        ],
      }),
    );
    writeFileSync(
      sidecarPath,
      JSON.stringify({
        id: "provider-e2e-generated",
        status: "completed",
        model: {
          provider: "replicate",
          id: "black-forest-labs/flux-schnell",
        },
        outputs: [
          {
            mediaId: "provider-e2e-output",
            relativePath: "generated/provider-e2e-output.png",
            sourceUrl: "https://provider.example/generated-output.png",
            width: 1024,
            height: 768,
            durationSeconds: 0,
            fps: 0,
          },
        ],
      }),
    );
    writeFileSync(
      fakeCargo,
      [
        "#!/bin/sh",
        `printf '%s\\n' '{"ok":true,"provider":"replicate","model":"black-forest-labs/flux-schnell","projectDir":"${projectDir}","artifactPath":"${artifactPath}","outputCount":1,"jobStatus":"Completed"}'`,
      ].join("\n"),
      { mode: 0o755 },
    );

    const outDir = join(tempDir, "provider-out");
    const result = spawnSync(
      process.execPath,
      [
        join(repoRoot, "scripts/provider-e2e.mjs"),
        "--provider",
        "replicate",
        "--live",
        "--out-dir",
        outDir,
      ],
      {
        cwd: repoRoot,
        encoding: "utf8",
        env: {
          ...process.env,
          PATH: `${tempDir}:${process.env.PATH ?? ""}`,
          REPLICATE_API_TOKEN: "super-secret-replicate-token",
          VIDEO_CREATER_PROVIDER_E2E_ALLOW_SPEND: "1",
        },
      },
    );

    expect(result.status).toBe(1);
    const report = JSON.parse(result.stdout) as {
      ok: boolean;
      error: string;
      result: { projectDir: string };
    };
    expect(report).toEqual(
      expect.objectContaining({
        ok: false,
        error: "Provider E2E generated sidecar output is missing from media index: provider-e2e-output",
        result: expect.objectContaining({ projectDir }),
      }),
    );
  });

  it("fails closed when the provider E2E generated sidecar output has duplicate media index rows", () => {
    const tempDir = mkdtempSync(join(tmpdir(), "video-creater-provider-e2e-"));
    const fakeCargo = join(tempDir, "cargo");
    const projectDir = join(tempDir, "project");
    const artifactPath = join(projectDir, "generated", "provider-e2e-output.png");
    const sidecarPath = join(
      projectDir,
      "generated",
      "provider-e2e-generated",
      "asset.json",
    );
    mkdirSync(join(projectDir, "generated", "provider-e2e-generated"), {
      recursive: true,
    });
    mkdirSync(join(projectDir, "media"));
    writeFileSync(artifactPath, "project generated pixels");
    writeFileSync(
      join(projectDir, "video-creater.project.json"),
      JSON.stringify({
        schemaVersion: 1,
        id: "provider-e2e-project",
        files: { generated: "generated", media: "media/index.json" },
      }),
    );
    writeFileSync(
      join(projectDir, "media", "index.json"),
      JSON.stringify({
        schemaVersion: 1,
        folders: [],
        assets: [
          {
            id: "provider-e2e-output",
            relativePath: "generated/stale-provider-e2e-output.png",
            kind: "generated",
            durationSeconds: 0,
            width: 1024,
            height: 768,
            fps: 0,
          },
          {
            id: "provider-e2e-output",
            relativePath: "generated/provider-e2e-output.png",
            kind: "generated",
            durationSeconds: 0,
            width: 1024,
            height: 768,
            fps: 0,
          },
        ],
        analysis: [],
      }),
    );
    writeFileSync(
      join(projectDir, "generated", "index.json"),
      JSON.stringify({
        schemaVersion: 1,
        assets: [
          {
            assetId: "provider-e2e-generated",
            path: "generated/provider-e2e-generated/asset.json",
            status: "completed",
            modelProvider: "replicate",
            modelId: "black-forest-labs/flux-schnell",
            outputCount: 1,
          },
        ],
      }),
    );
    writeFileSync(
      sidecarPath,
      JSON.stringify({
        id: "provider-e2e-generated",
        status: "completed",
        model: {
          provider: "replicate",
          id: "black-forest-labs/flux-schnell",
        },
        outputs: [
          {
            mediaId: "provider-e2e-output",
            relativePath: "generated/provider-e2e-output.png",
            sourceUrl: "https://provider.example/generated-output.png",
            width: 1024,
            height: 768,
            durationSeconds: 0,
            fps: 0,
          },
        ],
      }),
    );
    writeFileSync(
      fakeCargo,
      [
        "#!/bin/sh",
        `printf '%s\\n' '{"ok":true,"provider":"replicate","model":"black-forest-labs/flux-schnell","projectDir":"${projectDir}","artifactPath":"${artifactPath}","outputCount":1,"jobStatus":"Completed"}'`,
      ].join("\n"),
      { mode: 0o755 },
    );

    const outDir = join(tempDir, "provider-out");
    const result = spawnSync(
      process.execPath,
      [
        join(repoRoot, "scripts/provider-e2e.mjs"),
        "--provider",
        "replicate",
        "--live",
        "--out-dir",
        outDir,
      ],
      {
        cwd: repoRoot,
        encoding: "utf8",
        env: {
          ...process.env,
          PATH: `${tempDir}:${process.env.PATH ?? ""}`,
          REPLICATE_API_TOKEN: "super-secret-replicate-token",
          VIDEO_CREATER_PROVIDER_E2E_ALLOW_SPEND: "1",
        },
      },
    );

    expect(result.status).toBe(1);
    const report = JSON.parse(result.stdout) as {
      ok: boolean;
      error: string;
      result: { projectDir: string };
    };
    expect(report).toEqual(
      expect.objectContaining({
        ok: false,
        error: "Provider E2E media index has duplicate generated output id: provider-e2e-output",
        result: expect.objectContaining({ projectDir }),
      }),
    );
  });

  it("fails closed when the provider E2E media index has generated media without a sidecar output", () => {
    const tempDir = mkdtempSync(join(tmpdir(), "video-creater-provider-e2e-"));
    const fakeCargo = join(tempDir, "cargo");
    const projectDir = join(tempDir, "project");
    const artifactPath = join(projectDir, "generated", "provider-e2e-output.png");
    const sidecarPath = join(
      projectDir,
      "generated",
      "provider-e2e-generated",
      "asset.json",
    );
    mkdirSync(join(projectDir, "generated", "provider-e2e-generated"), {
      recursive: true,
    });
    mkdirSync(join(projectDir, "media"));
    writeFileSync(artifactPath, "project generated pixels");
    writeFileSync(
      join(projectDir, "video-creater.project.json"),
      JSON.stringify({
        schemaVersion: 1,
        id: "provider-e2e-project",
        files: { generated: "generated", media: "media/index.json" },
      }),
    );
    writeFileSync(
      join(projectDir, "media", "index.json"),
      JSON.stringify({
        schemaVersion: 1,
        folders: [],
        assets: [
          {
            id: "provider-e2e-output",
            relativePath: "generated/provider-e2e-output.png",
            kind: "generated",
            durationSeconds: 0,
            width: 1024,
            height: 768,
            fps: 0,
          },
          {
            id: "stale-generated-output",
            relativePath: "generated/stale-generated-output.png",
            kind: "generated",
            durationSeconds: 0,
            width: 1024,
            height: 768,
            fps: 0,
          },
        ],
        analysis: [],
      }),
    );
    writeFileSync(
      join(projectDir, "generated", "index.json"),
      JSON.stringify({
        schemaVersion: 1,
        assets: [
          {
            assetId: "provider-e2e-generated",
            path: "generated/provider-e2e-generated/asset.json",
            status: "completed",
            modelProvider: "replicate",
            modelId: "black-forest-labs/flux-schnell",
            outputCount: 1,
          },
        ],
      }),
    );
    writeFileSync(
      sidecarPath,
      JSON.stringify({
        id: "provider-e2e-generated",
        status: "completed",
        model: {
          provider: "replicate",
          id: "black-forest-labs/flux-schnell",
        },
        outputs: [
          {
            mediaId: "provider-e2e-output",
            relativePath: "generated/provider-e2e-output.png",
            sourceUrl: "https://provider.example/generated-output.png",
            width: 1024,
            height: 768,
            durationSeconds: 0,
            fps: 0,
          },
        ],
      }),
    );
    writeFileSync(
      fakeCargo,
      [
        "#!/bin/sh",
        `printf '%s\\n' '{"ok":true,"provider":"replicate","model":"black-forest-labs/flux-schnell","projectDir":"${projectDir}","artifactPath":"${artifactPath}","outputCount":1,"jobStatus":"Completed"}'`,
      ].join("\n"),
      { mode: 0o755 },
    );

    const outDir = join(tempDir, "provider-out");
    const result = spawnSync(
      process.execPath,
      [
        join(repoRoot, "scripts/provider-e2e.mjs"),
        "--provider",
        "replicate",
        "--live",
        "--out-dir",
        outDir,
      ],
      {
        cwd: repoRoot,
        encoding: "utf8",
        env: {
          ...process.env,
          PATH: `${tempDir}:${process.env.PATH ?? ""}`,
          REPLICATE_API_TOKEN: "super-secret-replicate-token",
          VIDEO_CREATER_PROVIDER_E2E_ALLOW_SPEND: "1",
        },
      },
    );

    expect(result.status).toBe(1);
    const report = JSON.parse(result.stdout) as {
      ok: boolean;
      error: string;
      result: { projectDir: string };
    };
    expect(report).toEqual(
      expect.objectContaining({
        ok: false,
        error: "Provider E2E generated media is not backed by a completed sidecar output: stale-generated-output",
        result: expect.objectContaining({ projectDir }),
      }),
    );
  });

  it("fails closed when a provider E2E generated sidecar output file is empty", () => {
    const tempDir = mkdtempSync(join(tmpdir(), "video-creater-provider-e2e-"));
    const fakeCargo = join(tempDir, "cargo");
    const projectDir = join(tempDir, "project");
    const artifactPath = join(projectDir, "generated", "provider-e2e-output.png");
    const emptyOutputPath = join(projectDir, "generated", "provider-e2e-empty.png");
    const sidecarPath = join(
      projectDir,
      "generated",
      "provider-e2e-generated",
      "asset.json",
    );
    mkdirSync(join(projectDir, "generated", "provider-e2e-generated"), {
      recursive: true,
    });
    mkdirSync(join(projectDir, "media"));
    writeFileSync(artifactPath, "project generated pixels");
    writeFileSync(emptyOutputPath, "");
    writeFileSync(
      join(projectDir, "video-creater.project.json"),
      JSON.stringify({
        schemaVersion: 1,
        id: "provider-e2e-project",
        files: { generated: "generated", media: "media/index.json" },
      }),
    );
    writeFileSync(
      join(projectDir, "media", "index.json"),
      JSON.stringify({
        schemaVersion: 1,
        folders: [],
        assets: [
          {
            id: "provider-e2e-output",
            relativePath: "generated/provider-e2e-output.png",
            kind: "generated",
            durationSeconds: 0,
            width: 1024,
            height: 768,
            fps: 0,
          },
          {
            id: "provider-e2e-empty",
            relativePath: "generated/provider-e2e-empty.png",
            kind: "generated",
            durationSeconds: 0,
            width: 1024,
            height: 768,
            fps: 0,
          },
        ],
        analysis: [],
      }),
    );
    writeFileSync(
      join(projectDir, "generated", "index.json"),
      JSON.stringify({
        schemaVersion: 1,
        assets: [
          {
            assetId: "provider-e2e-generated",
            path: "generated/provider-e2e-generated/asset.json",
            status: "completed",
            modelProvider: "replicate",
            modelId: "black-forest-labs/flux-schnell",
            outputCount: 2,
          },
        ],
      }),
    );
    writeFileSync(
      sidecarPath,
      JSON.stringify({
        id: "provider-e2e-generated",
        status: "completed",
        model: {
          provider: "replicate",
          id: "black-forest-labs/flux-schnell",
        },
        outputs: [
          {
            mediaId: "provider-e2e-output",
            relativePath: "generated/provider-e2e-output.png",
            sourceUrl: "https://provider.example/generated-output.png",
            width: 1024,
            height: 768,
            durationSeconds: 0,
            fps: 0,
          },
          {
            mediaId: "provider-e2e-empty",
            relativePath: "generated/provider-e2e-empty.png",
            sourceUrl: "https://provider.example/generated-empty.png",
            width: 1024,
            height: 768,
            durationSeconds: 0,
            fps: 0,
          },
        ],
      }),
    );
    writeFileSync(
      fakeCargo,
      [
        "#!/bin/sh",
        `printf '%s\\n' '{"ok":true,"provider":"replicate","model":"black-forest-labs/flux-schnell","projectDir":"${projectDir}","artifactPath":"${artifactPath}","outputCount":2,"jobStatus":"Completed"}'`,
      ].join("\n"),
      { mode: 0o755 },
    );

    const outDir = join(tempDir, "provider-out");
    const result = spawnSync(
      process.execPath,
      [
        join(repoRoot, "scripts/provider-e2e.mjs"),
        "--provider",
        "replicate",
        "--live",
        "--out-dir",
        outDir,
      ],
      {
        cwd: repoRoot,
        encoding: "utf8",
        env: {
          ...process.env,
          PATH: `${tempDir}:${process.env.PATH ?? ""}`,
          REPLICATE_API_TOKEN: "super-secret-replicate-token",
          VIDEO_CREATER_PROVIDER_E2E_ALLOW_SPEND: "1",
        },
      },
    );

    expect(result.status).toBe(1);
    const report = JSON.parse(result.stdout) as {
      ok: boolean;
      error: string;
      result: { projectDir: string };
    };
    expect(report).toEqual(
      expect.objectContaining({
        ok: false,
        error: `Provider E2E generated asset sidecar output file is empty or not a file: ${emptyOutputPath}`,
        result: expect.objectContaining({ projectDir }),
      }),
    );
  });

  it("fails closed when the provider E2E generated asset index asset id does not match the sidecar", () => {
    const tempDir = mkdtempSync(join(tmpdir(), "video-creater-provider-e2e-"));
    const fakeCargo = join(tempDir, "cargo");
    const projectDir = join(tempDir, "project");
    const artifactPath = join(projectDir, "generated", "provider-e2e-output.png");
    const sidecarPath = join(
      projectDir,
      "generated",
      "provider-e2e-generated",
      "asset.json",
    );
    mkdirSync(join(projectDir, "generated", "provider-e2e-generated"), {
      recursive: true,
    });
    writeFileSync(artifactPath, "project generated pixels");
    writeFileSync(
      join(projectDir, "video-creater.project.json"),
      JSON.stringify({
        schemaVersion: 1,
        id: "provider-e2e-project",
        files: { generated: "generated" },
      }),
    );
    writeFileSync(
      join(projectDir, "generated", "index.json"),
      JSON.stringify({
        schemaVersion: 1,
        assets: [
          {
            assetId: "provider-e2e-generated",
            path: "generated/provider-e2e-generated/asset.json",
            status: "completed",
            modelProvider: "replicate",
            modelId: "black-forest-labs/flux-schnell",
            outputCount: 1,
          },
        ],
      }),
    );
    writeFileSync(
      sidecarPath,
      JSON.stringify({
        id: "different-generated-asset",
        status: "completed",
        model: {
          provider: "replicate",
          id: "black-forest-labs/flux-schnell",
        },
        outputs: [{ mediaId: "provider-e2e-output", relativePath: "generated/provider-e2e-output.png", sourceUrl: "https://provider.example/generated-output.png", width: 1024, height: 768, durationSeconds: 0, fps: 0 }],
      }),
    );
    writeFileSync(
      fakeCargo,
      [
        "#!/bin/sh",
        `printf '%s\\n' '{"ok":true,"provider":"replicate","model":"black-forest-labs/flux-schnell","projectDir":"${projectDir}","artifactPath":"${artifactPath}","outputCount":1,"jobStatus":"Completed"}'`,
      ].join("\n"),
      { mode: 0o755 },
    );

    const outDir = join(tempDir, "provider-out");
    const result = spawnSync(
      process.execPath,
      [
        join(repoRoot, "scripts/provider-e2e.mjs"),
        "--provider",
        "replicate",
        "--live",
        "--out-dir",
        outDir,
      ],
      {
        cwd: repoRoot,
        encoding: "utf8",
        env: {
          ...process.env,
          PATH: `${tempDir}:${process.env.PATH ?? ""}`,
          REPLICATE_API_TOKEN: "super-secret-replicate-token",
          VIDEO_CREATER_PROVIDER_E2E_ALLOW_SPEND: "1",
        },
      },
    );

    expect(result.status).toBe(1);
    const report = JSON.parse(result.stdout) as {
      ok: boolean;
      error: string;
      result: { projectDir: string };
    };
    expect(report).toEqual(
      expect.objectContaining({
        ok: false,
        error: `Provider E2E generated asset index asset id does not match sidecar id: ${sidecarPath}`,
        result: expect.objectContaining({ projectDir }),
      }),
    );
  });

  it("fails closed when the provider E2E generated asset sidecar model does not match the request", () => {
    const tempDir = mkdtempSync(join(tmpdir(), "video-creater-provider-e2e-"));
    const fakeCargo = join(tempDir, "cargo");
    const projectDir = join(tempDir, "project");
    const artifactPath = join(projectDir, "generated", "provider-e2e-output.png");
    const sidecarPath = join(
      projectDir,
      "generated",
      "provider-e2e-generated",
      "asset.json",
    );
    mkdirSync(join(projectDir, "generated", "provider-e2e-generated"), {
      recursive: true,
    });
    writeFileSync(artifactPath, "project generated pixels");
    writeFileSync(
      join(projectDir, "video-creater.project.json"),
      JSON.stringify({
        schemaVersion: 1,
        id: "provider-e2e-project",
        files: { generated: "generated" },
      }),
    );
    writeFileSync(
      join(projectDir, "generated", "index.json"),
      JSON.stringify({
        schemaVersion: 1,
        assets: [
          {
            assetId: "provider-e2e-generated",
            path: "generated/provider-e2e-generated/asset.json",
            status: "completed",
            modelProvider: "replicate",
            modelId: "black-forest-labs/flux-schnell",
            outputCount: 1,
          },
        ],
      }),
    );
    writeFileSync(
      sidecarPath,
      JSON.stringify({
        id: "provider-e2e-generated",
        status: "completed",
        model: {
          provider: "replicate",
          id: "different/model",
        },
        outputs: [{ mediaId: "provider-e2e-output", relativePath: "generated/provider-e2e-output.png", sourceUrl: "https://provider.example/generated-output.png", width: 1024, height: 768, durationSeconds: 0, fps: 0 }],
      }),
    );
    writeFileSync(
      fakeCargo,
      [
        "#!/bin/sh",
        `printf '%s\\n' '{"ok":true,"provider":"replicate","model":"black-forest-labs/flux-schnell","projectDir":"${projectDir}","artifactPath":"${artifactPath}","outputCount":1,"jobStatus":"Completed"}'`,
      ].join("\n"),
      { mode: 0o755 },
    );

    const outDir = join(tempDir, "provider-out");
    const result = spawnSync(
      process.execPath,
      [
        join(repoRoot, "scripts/provider-e2e.mjs"),
        "--provider",
        "replicate",
        "--live",
        "--model",
        "black-forest-labs/flux-schnell",
        "--out-dir",
        outDir,
      ],
      {
        cwd: repoRoot,
        encoding: "utf8",
        env: {
          ...process.env,
          PATH: `${tempDir}:${process.env.PATH ?? ""}`,
          REPLICATE_API_TOKEN: "super-secret-replicate-token",
          VIDEO_CREATER_PROVIDER_E2E_ALLOW_SPEND: "1",
        },
      },
    );

    expect(result.status).toBe(1);
    const report = JSON.parse(result.stdout) as {
      ok: boolean;
      error: string;
      result: { projectDir: string };
    };
    expect(report).toEqual(
      expect.objectContaining({
        ok: false,
        error: `Provider E2E generated asset sidecar model does not match requested model: ${sidecarPath}`,
        result: expect.objectContaining({ projectDir }),
      }),
    );
  });

  it("fails closed when the provider E2E generated asset index status does not match the sidecar", () => {
    const tempDir = mkdtempSync(join(tmpdir(), "video-creater-provider-e2e-"));
    const fakeCargo = join(tempDir, "cargo");
    const projectDir = join(tempDir, "project");
    const artifactPath = join(projectDir, "generated", "provider-e2e-output.png");
    const sidecarPath = join(
      projectDir,
      "generated",
      "provider-e2e-generated",
      "asset.json",
    );
    mkdirSync(join(projectDir, "generated", "provider-e2e-generated"), {
      recursive: true,
    });
    writeFileSync(artifactPath, "project generated pixels");
    writeFileSync(
      join(projectDir, "video-creater.project.json"),
      JSON.stringify({
        schemaVersion: 1,
        id: "provider-e2e-project",
        files: { generated: "generated" },
      }),
    );
    writeFileSync(
      join(projectDir, "generated", "index.json"),
      JSON.stringify({
        schemaVersion: 1,
        assets: [
          {
            assetId: "provider-e2e-generated",
            path: "generated/provider-e2e-generated/asset.json",
            status: "running",
            modelProvider: "replicate",
            modelId: "black-forest-labs/flux-schnell",
            outputCount: 1,
          },
        ],
      }),
    );
    writeFileSync(
      sidecarPath,
      JSON.stringify({
        id: "provider-e2e-generated",
        status: "completed",
        model: {
          provider: "replicate",
          id: "black-forest-labs/flux-schnell",
        },
        outputs: [{ mediaId: "provider-e2e-output", relativePath: "generated/provider-e2e-output.png", sourceUrl: "https://provider.example/generated-output.png", width: 1024, height: 768, durationSeconds: 0, fps: 0 }],
      }),
    );
    writeFileSync(
      fakeCargo,
      [
        "#!/bin/sh",
        `printf '%s\\n' '{"ok":true,"provider":"replicate","model":"black-forest-labs/flux-schnell","projectDir":"${projectDir}","artifactPath":"${artifactPath}","outputCount":1,"jobStatus":"Completed"}'`,
      ].join("\n"),
      { mode: 0o755 },
    );

    const outDir = join(tempDir, "provider-out");
    const result = spawnSync(
      process.execPath,
      [
        join(repoRoot, "scripts/provider-e2e.mjs"),
        "--provider",
        "replicate",
        "--live",
        "--out-dir",
        outDir,
      ],
      {
        cwd: repoRoot,
        encoding: "utf8",
        env: {
          ...process.env,
          PATH: `${tempDir}:${process.env.PATH ?? ""}`,
          REPLICATE_API_TOKEN: "super-secret-replicate-token",
          VIDEO_CREATER_PROVIDER_E2E_ALLOW_SPEND: "1",
        },
      },
    );

    expect(result.status).toBe(1);
    const report = JSON.parse(result.stdout) as {
      ok: boolean;
      error: string;
      result: { projectDir: string };
    };
    expect(report).toEqual(
      expect.objectContaining({
        ok: false,
        error: `Provider E2E generated asset index status does not match completed sidecar: ${sidecarPath}`,
        result: expect.objectContaining({ projectDir }),
      }),
    );
  });

  it("fails closed when the provider E2E binary reports a different provider", () => {
    const tempDir = mkdtempSync(join(tmpdir(), "video-creater-provider-e2e-"));
    const fakeCargo = join(tempDir, "cargo");
    const projectDir = join(tempDir, "project");
    const artifactPath = join(projectDir, "generated", "provider-e2e-output.png");
    mkdirSync(join(projectDir, "generated", "provider-e2e-generated"), {
      recursive: true,
    });
    writeFileSync(artifactPath, "project generated pixels");
    writeFileSync(
      join(projectDir, "video-creater.project.json"),
      JSON.stringify({
        schemaVersion: 1,
        id: "provider-e2e-project",
        files: { generated: "generated" },
      }),
    );
    writeFileSync(
      join(projectDir, "generated", "index.json"),
      JSON.stringify({
        schemaVersion: 1,
        assets: [
          {
            assetId: "provider-e2e-generated",
            path: "generated/provider-e2e-generated/asset.json",
            status: "completed",
            modelProvider: "replicate",
            modelId: "black-forest-labs/flux-schnell",
            outputCount: 1,
          },
        ],
      }),
    );
    writeFileSync(
      join(projectDir, "generated", "provider-e2e-generated", "asset.json"),
      JSON.stringify({
        id: "provider-e2e-generated",
        status: "completed",
        model: {
          provider: "replicate",
          id: "black-forest-labs/flux-schnell",
        },
        outputs: [{ mediaId: "provider-e2e-output", relativePath: "generated/provider-e2e-output.png", sourceUrl: "https://provider.example/generated-output.png", width: 1024, height: 768, durationSeconds: 0, fps: 0 }],
      }),
    );
    writeFileSync(
      fakeCargo,
      [
        "#!/bin/sh",
        `printf '%s\\n' '{"ok":true,"provider":"fal.ai","model":"black-forest-labs/flux-schnell","projectDir":"${projectDir}","artifactPath":"${artifactPath}","outputCount":1,"jobStatus":"Completed"}'`,
      ].join("\n"),
      { mode: 0o755 },
    );

    const outDir = join(tempDir, "provider-out");
    const result = spawnSync(
      process.execPath,
      [
        join(repoRoot, "scripts/provider-e2e.mjs"),
        "--provider",
        "replicate",
        "--live",
        "--out-dir",
        outDir,
      ],
      {
        cwd: repoRoot,
        encoding: "utf8",
        env: {
          ...process.env,
          PATH: `${tempDir}:${process.env.PATH ?? ""}`,
          REPLICATE_API_TOKEN: "super-secret-replicate-token",
          VIDEO_CREATER_PROVIDER_E2E_ALLOW_SPEND: "1",
        },
      },
    );

    expect(result.status).toBe(1);
    const report = JSON.parse(result.stdout) as {
      ok: boolean;
      error: string;
      result: { provider: string };
    };
    expect(report).toEqual(
      expect.objectContaining({
        ok: false,
        error: "Provider E2E binary reported provider fal.ai for requested provider replicate",
        result: expect.objectContaining({ provider: "fal.ai" }),
      }),
    );
  });

  it("fails closed when the provider E2E binary reports a different model", () => {
    const tempDir = mkdtempSync(join(tmpdir(), "video-creater-provider-e2e-"));
    const fakeCargo = join(tempDir, "cargo");
    const projectDir = join(tempDir, "project");
    const artifactPath = join(projectDir, "generated", "provider-e2e-output.png");
    mkdirSync(join(projectDir, "generated", "provider-e2e-generated"), {
      recursive: true,
    });
    writeFileSync(artifactPath, "project generated pixels");
    writeFileSync(
      join(projectDir, "video-creater.project.json"),
      JSON.stringify({
        schemaVersion: 1,
        id: "provider-e2e-project",
        files: { generated: "generated" },
      }),
    );
    writeFileSync(
      join(projectDir, "generated", "index.json"),
      JSON.stringify({
        schemaVersion: 1,
        assets: [
          {
            assetId: "provider-e2e-generated",
            path: "generated/provider-e2e-generated/asset.json",
            status: "completed",
            modelProvider: "replicate",
            modelId: "black-forest-labs/flux-schnell",
            outputCount: 1,
          },
        ],
      }),
    );
    writeFileSync(
      join(projectDir, "generated", "provider-e2e-generated", "asset.json"),
      JSON.stringify({
        id: "provider-e2e-generated",
        status: "completed",
        model: {
          provider: "replicate",
          id: "black-forest-labs/flux-schnell",
        },
        outputs: [{ mediaId: "provider-e2e-output", relativePath: "generated/provider-e2e-output.png", sourceUrl: "https://provider.example/generated-output.png", width: 1024, height: 768, durationSeconds: 0, fps: 0 }],
      }),
    );
    writeFileSync(
      fakeCargo,
      [
        "#!/bin/sh",
        `printf '%s\\n' '{"ok":true,"provider":"replicate","model":"different/model","projectDir":"${projectDir}","artifactPath":"${artifactPath}","outputCount":1,"jobStatus":"Completed"}'`,
      ].join("\n"),
      { mode: 0o755 },
    );

    const outDir = join(tempDir, "provider-out");
    const result = spawnSync(
      process.execPath,
      [
        join(repoRoot, "scripts/provider-e2e.mjs"),
        "--provider",
        "replicate",
        "--live",
        "--model",
        "black-forest-labs/flux-schnell",
        "--out-dir",
        outDir,
      ],
      {
        cwd: repoRoot,
        encoding: "utf8",
        env: {
          ...process.env,
          PATH: `${tempDir}:${process.env.PATH ?? ""}`,
          REPLICATE_API_TOKEN: "super-secret-replicate-token",
          VIDEO_CREATER_PROVIDER_E2E_ALLOW_SPEND: "1",
        },
      },
    );

    expect(result.status).toBe(1);
    const report = JSON.parse(result.stdout) as {
      ok: boolean;
      error: string;
      result: { model: string };
    };
    expect(report).toEqual(
      expect.objectContaining({
        ok: false,
        error:
          "Provider E2E binary reported model different/model for requested model black-forest-labs/flux-schnell",
        result: expect.objectContaining({ model: "different/model" }),
      }),
    );
  });
});

function relativeTo(root: string, path: string) {
  return relative(root, path).replace(/\\/g, "/");
}

function providerLifecycleScenarioForTest(scenario: string) {
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

function isProviderVideoGenerationScenarioForTest(scenario: string) {
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
    scenario === "local-video-edit-cancellation" ||
    scenario === "local-video-edit-failure"
  );
}

function imageEditReferencesForPolicyFixture(
  provider: "elevenlabs" | "fal.ai" | "google" | "minimax" | "openai" | "replicate" | "xai",
) {
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

function googleVeoReferencesForPolicyFixture() {
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

function xaiVideoReferencesForPolicyFixture() {
  return {
    mediaIds: ["provider-e2e-reference-image"],
    referenceImageMediaRefs: ["provider-e2e-reference-image"],
    providerInputUrls: ["https://api.x.ai/v1/files/provider-e2e/reference-image.png"],
  };
}

function writeProviderPolicyFixture({
  rootDir,
  provider,
  model,
  requestId,
  scenario = "text-to-image",
  sourceUrl = `https://provider.example/${requestId}/generated-output.png`,
}: {
  rootDir: string;
  provider: "elevenlabs" | "fal.ai" | "google" | "minimax" | "openai" | "replicate" | "xai";
  model: string;
  requestId: string;
  scenario?: string;
  sourceUrl?: string | null;
}) {
  const safeProvider = provider.replace(/\./g, "-");
  const outDir = join(
    rootDir,
    "output",
    "provider-e2e",
    scenario === "text-to-image" ? provider : `${provider}-${scenario}`,
  );
  const projectDir = join(outDir, "project");
  const generatedId = `${safeProvider}-generated`;
  const mediaId = `${safeProvider}-output`;
  const isWanImageToVideo = scenario === "wan-image-to-video";
  const isWanReferenceToVideo = scenario === "wan-reference-to-video";
  const isKlingImageToVideo = scenario === "kling-image-to-video";
  const isReplicateVideo =
    scenario === "replicate-video" || scenario === "replicate-video-fast";
  const isXaiVideo = scenario === "xai-video";
  const isGoogleVideo = scenario === "google-video";
  const isTextToVideoReplace = scenario === "text-to-video-replace";
  const isTextToVideoInsert = scenario === "text-to-video-insert";
  const isTextToImageReplace = scenario === "text-to-image-replace";
  const isReferencedVideo =
    isWanImageToVideo || isWanReferenceToVideo || isKlingImageToVideo || isReplicateVideo;
  const isVideoToVideo =
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
    scenario === "local-video-edit-failure";
  const isVideoOutputScenario =
    isReferencedVideo ||
    isVideoToVideo ||
    isXaiVideo ||
    isGoogleVideo ||
    isTextToVideoReplace ||
    isTextToVideoInsert;
  const isImageEdit =
    scenario === "image-edit" ||
    scenario === "local-image-edit-replace" ||
    scenario === "local-image-edit-retry" ||
    scenario === "local-image-edit-cancellation" ||
    scenario === "local-image-edit-failure";
  const isRecraftTextToImage = scenario === "recraft-text-to-image";
  const isTextToAudio = scenario === "text-to-audio";
  const isTextToMusic = scenario === "text-to-music" || scenario === "text-to-music-insert";
  const isVideoToMusic =
    scenario === "video-to-music" || scenario === "local-video-to-music-insert";
  const isVideoToSfx = scenario === "video-to-sfx" || scenario === "local-video-to-sfx-insert";
  const isAudioScenario = isTextToAudio || isTextToMusic || isVideoToMusic || isVideoToSfx;
  const isLocalImageUpscale =
    scenario === "local-image-upscale" ||
    scenario === "local-image-upscale-replace" ||
    scenario === "local-image-upscale-retry" ||
    scenario === "local-image-upscale-cancellation" ||
    scenario === "local-image-upscale-failure";
  const isLocalVideoUpscale =
    scenario === "local-video-upscale" ||
    scenario === "local-video-upscale-replace" ||
    scenario === "local-video-upscale-retry" ||
    scenario === "local-video-upscale-cancellation" ||
    scenario === "local-video-upscale-failure";
  const isMultiImage = scenario === "multi-image";
  const isReplacement =
    scenario === "local-image-upscale-replace" ||
    scenario === "local-image-edit-replace" ||
    scenario === "local-video-upscale-replace" ||
    scenario === "local-video-edit-replace" ||
    scenario === "local-video-motion-control-replace" ||
    isTextToImageReplace ||
    isTextToVideoReplace;
  const isTimelineAudioInsert =
    scenario === "local-video-to-music-insert" || scenario === "local-video-to-sfx-insert";
  const isTextToMusicInsert = scenario === "text-to-music-insert";
  const isTimelineVisualInsert = scenario === "text-to-video-insert";
  const timelineAudioInsertConfig =
    scenario === "local-video-to-sfx-insert"
      ? {
          sourceItemId: "item-video-to-sfx-source",
          insertedItemId: "item-video-to-sfx-inserted-audio",
          placementIntent: "insert-audio:item-video-to-sfx-source",
          label: "Provider E2E generated SFX",
          timelineStartSeconds: 8,
          videoSourceStartSeconds: 0.5,
          videoSourceEndSeconds: 2,
        }
      : {
          sourceItemId: "item-video-to-music-source",
          insertedItemId: "item-video-to-music-inserted-audio",
          placementIntent: "insert-audio:item-video-to-music-source",
          label: "Provider E2E generated music",
          timelineStartSeconds: 12,
          videoSourceStartSeconds: 1.25,
          videoSourceEndSeconds: 5.75,
      };
  const timelineVisualInsertConfig = {
    trackId: "track-video",
    insertedItemId: "item-text-video-inserted-video",
    placementIntent: "insert-video:track-video",
    label: "Provider E2E generated video",
    timelineStartSeconds: 6,
  };
  const timelineTextToMusicInsertConfig = {
    trackId: "track-audio",
    insertedItemId: "item-text-music-inserted-audio",
    placementIntent: "insert-audio:track-audio",
    label: "Provider E2E generated music",
    timelineStartSeconds: 12,
  };
  const replacementItemId =
    isTextToImageReplace
      ? "item-text-image-target"
      : isTextToVideoReplace
        ? "item-text-video-target"
    : scenario === "local-image-edit-replace"
      ? "item-image-edit-target"
    : scenario === "local-video-edit-replace"
      ? "item-video-edit-target"
      : scenario === "local-video-motion-control-replace"
      ? "item-video-motion-control-target"
      : scenario === "local-video-upscale-replace"
        ? "item-video-upscale-target"
      : "item-upscale-target";
  const isRetry =
    scenario === "provider-retry" ||
    scenario === "local-image-upscale-retry" ||
    scenario === "local-image-edit-retry" ||
    scenario === "local-video-upscale-retry" ||
    scenario === "local-video-edit-retry";
  const generatedDurationSeconds = scenario === "text-to-video-replace" ||
    scenario === "text-to-video-insert"
    ? 5
    : scenario === "local-video-edit-replace" ||
    scenario === "local-video-motion-control-replace" ||
    scenario === "local-video-edit-retry" ||
    scenario === "local-video-edit-cancellation" ||
    scenario === "local-video-edit-failure"
    ? 3
    : scenario === "text-to-image-replace" ||
      scenario === "local-image-upscale-replace" ||
      scenario === "local-image-upscale-retry" ||
      scenario === "local-image-upscale-cancellation" ||
      scenario === "local-image-upscale-failure" ||
      scenario === "local-image-edit-replace" ||
      scenario === "local-image-edit-retry" ||
      scenario === "local-image-edit-cancellation" ||
      scenario === "local-image-edit-failure"
      ? 4
    : isVideoOutputScenario
      ? 4
      : isAudioScenario
        ? (isTextToMusic ? 30 : isTextToAudio ? 12 : 10)
        : 0;
  const lifecycleScenario = providerLifecycleScenarioForTest(scenario);
  const failedRetryAssetId = `${safeProvider}-failed-attempt`;
  const outputExtension = isVideoOutputScenario
    ? "mp4"
    : isVideoToMusic
      ? "m4a"
      : isVideoToSfx
        ? "wav"
        : isTextToAudio
          ? provider === "google"
            ? "wav"
            : "mp3"
          : isTextToMusic
            ? provider === "fal.ai"
              ? "m4a"
              : "mp3"
          : isRecraftTextToImage
            ? "webp"
          : "png";
  const outputPath = join(projectDir, "generated", `${mediaId}.${outputExtension}`);
  const sidecarRelativePath = `generated/${generatedId}/asset.json`;
  const outputRelativePath = `generated/${mediaId}.${outputExtension}`;
  const outputCount = isMultiImage ? 2 : 1;
  const outputMediaIds = Array.from({ length: outputCount }, (_, index) =>
    index === 0 ? mediaId : `${mediaId}-${index + 1}`,
  );
  const outputRelativePaths = Array.from({ length: outputCount }, (_, index) =>
    index === 0 ? outputRelativePath : `generated/${mediaId}-${index + 1}.${outputExtension}`,
  );
  const reportPath = join(outDir, "provider-e2e-report.json");
  const providerRequest = {
    provider,
    requestId,
    statusUrl: `https://api.${safeProvider}.example/v1/${requestId}`,
    responseUrl: `https://api.${safeProvider}.example/v1/${requestId}`,
    cancelUrl: `https://api.${safeProvider}.example/v1/${requestId}/cancel`,
    submittedAt: "2026-07-05T12:00:00Z",
  };
  const credentialEnvVar =
    provider === "fal.ai"
      ? "FAL_KEY"
      : provider === "openai"
        ? "OPENAI_API_KEY"
      : provider === "xai"
        ? "XAI_API_KEY"
      : provider === "google"
        ? "GEMINI_API_KEY"
        : provider === "elevenlabs"
          ? "ELEVENLABS_API_KEY"
          : provider === "minimax"
            ? "MINIMAX_API_KEY"
          : "REPLICATE_API_TOKEN";
  const isNoSpendMockedProvider = ["openai", "xai", "elevenlabs", "google", "minimax"].includes(provider);

  if (lifecycleScenario) {
    mkdirSync(join(projectDir, "generated", generatedId), { recursive: true });
    mkdirSync(join(projectDir, "media"), { recursive: true });
    mkdirSync(join(projectDir, "jobs", generatedId), { recursive: true });
    writeFileSync(
      join(projectDir, "video-creater.project.json"),
      JSON.stringify({
        schemaVersion: 1,
        id: `${safeProvider}-provider-e2e-project`,
        files: { generated: "generated", media: "media/index.json" },
      }),
    );
    writeFileSync(
      join(projectDir, "media", "index.json"),
      JSON.stringify({ schemaVersion: 1, folders: [], assets: [], analysis: [] }),
    );
    writeFileSync(
      join(projectDir, "generated", "index.json"),
      JSON.stringify({
        schemaVersion: 1,
        assets: [
          {
            assetId: generatedId,
            path: sidecarRelativePath,
            status: lifecycleScenario.assetStatus,
            modelProvider: provider,
            modelId: model,
            outputCount: 0,
          },
        ],
      }),
    );
    writeFileSync(
      join(projectDir, "generated", generatedId, "asset.json"),
      JSON.stringify({
        id: generatedId,
        status: lifecycleScenario.assetStatus,
        model: { provider, id: model },
        parentAssetId: null,
        retryOfAssetId: null,
        failureReason: lifecycleScenario.reason,
        references: isLocalImageUpscale
          ? {
              mediaIds: ["provider-e2e-source"],
              providerInputUrls: ["https://v3.fal.media/files/provider-e2e/source-image.png"],
            }
          : isVideoToVideo
            ? {
                mediaIds: ["provider-e2e-video-source"],
                sourceVideoMediaRef: "provider-e2e-video-source",
                referenceVideoMediaRefs: ["provider-e2e-video-source"],
                providerInputUrls: [
                  provider === "xai"
                    ? "https://api.x.ai/v1/files/provider-e2e/source-video.mp4"
                    : "https://v3.fal.media/files/provider-e2e/source-video.mp4",
                ],
              }
          : isImageEdit
            ? imageEditReferencesForPolicyFixture(provider)
          : isGoogleVideo
            ? googleVeoReferencesForPolicyFixture()
          : isXaiVideo
            ? xaiVideoReferencesForPolicyFixture()
            : undefined,
        settings:
          isLocalVideoUpscale
            ? {
                width: 1280,
                height: 720,
                durationSeconds: generatedDurationSeconds,
                fps: 24,
                aspectRatio: "16:9",
                videoSourceStartSeconds: 0.5,
                videoSourceEndSeconds: 4.5,
              }
          : scenario === "local-video-edit-cancellation" ||
          scenario === "local-video-edit-failure"
            ? {
                width: 1280,
                height: 720,
                durationSeconds: generatedDurationSeconds,
                fps: 24,
                aspectRatio: "16:9",
                generateAudio: true,
                videoSourceStartSeconds: 0.5,
                videoSourceEndSeconds: 3.5,
              }
            : undefined,
        outputs: [],
      }),
    );
    writeFileSync(
      join(projectDir, "jobs", generatedId, "job.json"),
      JSON.stringify({
        id: generatedId,
        kind: "generate_media",
        status: lifecycleScenario.jobSidecarStatus,
        updatedAt: "2026-07-05T12:00:00Z",
        providerRequest,
        terminalReason: lifecycleScenario.reason,
      }),
    );
    writeFileSync(
      reportPath,
      JSON.stringify(
        {
          ok: true,
          provider,
          model,
          scenario,
          outDir,
          reportPath,
          credentialEnvVar,
          credentialStatus: "not_checked",
          command: null,
          exitCode: 0,
          signal: null,
          result: {
            ok: false,
            provider,
            model,
            scenario,
            projectDir,
            artifactPath: null,
            outputCount: 0,
            jobStatus: lifecycleScenario.jobStatus,
            providerRequest,
            terminalReason: lifecycleScenario.reason,
          },
          error: null,
        },
        null,
        2,
      ),
    );
    return reportPath;
  }

  mkdirSync(join(projectDir, "generated", generatedId), { recursive: true });
  mkdirSync(join(projectDir, "media"), { recursive: true });
  mkdirSync(join(projectDir, "jobs", generatedId), { recursive: true });
  if (isRetry) {
    mkdirSync(join(projectDir, "generated", failedRetryAssetId), { recursive: true });
    mkdirSync(join(projectDir, "jobs", failedRetryAssetId), { recursive: true });
  }
  for (const [index, relativePath] of outputRelativePaths.entries()) {
    writeFileSync(
      join(projectDir, relativePath),
      isVideoOutputScenario
        ? outputCount === 1
          ? "generated video bytes"
          : `generated video bytes ${index + 1}`
        : isAudioScenario
          ? outputCount === 1
            ? "generated audio bytes"
            : `generated audio bytes ${index + 1}`
          : outputCount === 1
            ? "generated pixels"
            : `generated pixels ${index + 1}`,
    );
  }
  writeFileSync(
    join(projectDir, "video-creater.project.json"),
    JSON.stringify({
      schemaVersion: 1,
      id: `${safeProvider}-provider-e2e-project`,
      files: {
        ...(isReplacement || isTimelineAudioInsert || isTextToMusicInsert || isTimelineVisualInsert
          ? { timeline: "timeline.json" }
          : {}),
        generated: "generated",
        media: "media/index.json",
      },
    }),
  );
  writeFileSync(
    join(projectDir, "media", "index.json"),
    JSON.stringify({
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
    }),
  );
  if (isReplacement) {
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
                  label:
                    scenario === "local-video-edit-replace" ||
                    scenario === "local-video-upscale-replace" ||
                    scenario === "text-to-video-replace"
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
  if (isTimelineAudioInsert) {
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
                  id: timelineAudioInsertConfig.sourceItemId,
                  kind: "video_clip",
                  startSeconds: timelineAudioInsertConfig.timelineStartSeconds,
                  durationSeconds:
                    timelineAudioInsertConfig.videoSourceEndSeconds -
                    timelineAudioInsertConfig.videoSourceStartSeconds,
                  source: { type: "media", mediaId: "provider-e2e-video-source" },
                  label: "Provider E2E source video",
                  properties: {
                    sourceIn: timelineAudioInsertConfig.videoSourceStartSeconds,
                    sourceOut: timelineAudioInsertConfig.videoSourceEndSeconds,
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
                  id: timelineAudioInsertConfig.insertedItemId,
                  kind: "audio_clip",
                  startSeconds: timelineAudioInsertConfig.timelineStartSeconds,
                  durationSeconds: generatedDurationSeconds,
                  source: { type: "media", mediaId },
                  label: timelineAudioInsertConfig.label,
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
    writeFileSync(
      join(projectDir, "timeline.json"),
      JSON.stringify(
        {
          schemaVersion: 1,
          durationSeconds:
            timelineTextToMusicInsertConfig.timelineStartSeconds + generatedDurationSeconds,
          tracks: [
            {
              id: timelineTextToMusicInsertConfig.trackId,
              name: "Audio",
              kind: "audio",
              locked: false,
              enabled: true,
              items: [
                {
                  id: timelineTextToMusicInsertConfig.insertedItemId,
                  kind: "audio_clip",
                  startSeconds: timelineTextToMusicInsertConfig.timelineStartSeconds,
                  durationSeconds: generatedDurationSeconds,
                  source: { type: "media", mediaId },
                  label: timelineTextToMusicInsertConfig.label,
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
    writeFileSync(
      join(projectDir, "timeline.json"),
      JSON.stringify(
        {
          schemaVersion: 1,
          durationSeconds:
            timelineVisualInsertConfig.timelineStartSeconds + generatedDurationSeconds,
          tracks: [
            {
              id: timelineVisualInsertConfig.trackId,
              name: "Video",
              kind: "video",
              locked: false,
              enabled: true,
              items: [
                {
                  id: timelineVisualInsertConfig.insertedItemId,
                  kind: "video_clip",
                  startSeconds: timelineVisualInsertConfig.timelineStartSeconds,
                  durationSeconds: generatedDurationSeconds,
                  source: { type: "media", mediaId },
                  label: timelineVisualInsertConfig.label,
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
  writeFileSync(
    join(projectDir, "generated", "index.json"),
    JSON.stringify({
      schemaVersion: 1,
      assets: [
        ...(isRetry
          ? [
              {
                assetId: failedRetryAssetId,
                path: `generated/${failedRetryAssetId}/asset.json`,
                status: "failed",
                modelProvider: provider,
                modelId: model,
                outputCount: 0,
              },
            ]
          : []),
        {
          assetId: generatedId,
          path: sidecarRelativePath,
          status: "completed",
          modelProvider: provider,
          modelId: model,
          outputCount,
        },
      ],
    }),
  );
  writeFileSync(
    join(projectDir, "generated", generatedId, "asset.json"),
    JSON.stringify({
      id: generatedId,
      status: "completed",
      model: { provider, id: model },
      prompt: isVideoToMusic
        ? videoToMusicDefaultPrompt
        : isVideoToSfx
          ? ""
          : undefined,
      placementIntent: isReplacement
        ? `replace:${replacementItemId}`
        : isTimelineAudioInsert
          ? timelineAudioInsertConfig.placementIntent
          : isTextToMusicInsert
            ? timelineTextToMusicInsertConfig.placementIntent
        : isTimelineVisualInsert
          ? timelineVisualInsertConfig.placementIntent
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
        : isLocalImageUpscale
          ? {
              mediaIds: ["provider-e2e-source"],
              providerInputUrls: ["https://v3.fal.media/files/provider-e2e/source-image.png"],
            }
        : scenario === "local-video-motion-control-replace"
          ? {
              mediaIds: ["provider-e2e-video-source", "provider-e2e-motion-image-ref"],
              sourceVideoMediaRef: "provider-e2e-video-source",
              referenceImageMediaRefs: ["provider-e2e-motion-image-ref"],
              providerInputUrls: [
                "https://v3.fal.media/files/provider-e2e/source-video.mp4",
                "https://v3.fal.media/files/provider-e2e/motion-image-ref.png",
              ],
            }
        : isVideoToMusic || isVideoToSfx || isVideoToVideo
          ? {
              mediaIds: ["provider-e2e-video-source"],
              sourceVideoMediaRef: "provider-e2e-video-source",
              referenceVideoMediaRefs: ["provider-e2e-video-source"],
              providerInputUrls: [
                provider === "xai"
                  ? "https://api.x.ai/v1/files/provider-e2e/source-video.mp4"
                  : "https://v3.fal.media/files/provider-e2e/source-video.mp4",
              ],
            }
          : isImageEdit
            ? imageEditReferencesForPolicyFixture(provider)
          : isGoogleVideo
            ? googleVeoReferencesForPolicyFixture()
          : isXaiVideo
            ? xaiVideoReferencesForPolicyFixture()
            : undefined,
      outputs: outputRelativePaths.map((relativePath, index) => ({
          mediaId: outputMediaIds[index],
          relativePath,
          ...(sourceUrl === null
            ? {}
            : {
                sourceUrl:
                  isMultiImage
                    ? `https://provider.example/${requestId}/generated-output${index === 0 ? "" : `-${index + 1}`}.${outputExtension}`
                    : sourceUrl,
              }),
          width: isVideoOutputScenario ? 1280 : isAudioScenario ? 1 : 1024,
          height: isVideoOutputScenario ? 720 : isAudioScenario ? 1 : 768,
          durationSeconds: generatedDurationSeconds,
          fps: isVideoOutputScenario ? 24 : isAudioScenario ? 1 : 0,
        })),
      settings:
        isWanReferenceToVideo
          ? {
              durationSeconds: generatedDurationSeconds,
              fps: 24,
              aspectRatio: "16:9",
              resolution: "720p",
              generateAudio: true,
            }
        : isLocalVideoUpscale
          ? {
              width: 1280,
              height: 720,
              durationSeconds: generatedDurationSeconds,
              fps: 24,
              aspectRatio: "16:9",
              videoSourceStartSeconds: 0.5,
              videoSourceEndSeconds: 4.5,
            }
        : scenario === "local-video-edit-replace" ||
        scenario === "local-video-motion-control-replace" ||
        scenario === "local-video-edit-retry"
          ? {
              width: 1280,
              height: 720,
              durationSeconds: generatedDurationSeconds,
              fps: 24,
              aspectRatio: "16:9",
              generateAudio: true,
              videoSourceStartSeconds: 0.5,
              videoSourceEndSeconds: 3.5,
            }
        : scenario === "local-video-to-music-insert"
          ? {
              ...textToMusicDefaultSettings,
              timelineStartSeconds: timelineAudioInsertConfig.timelineStartSeconds,
              videoSourceStartSeconds: timelineAudioInsertConfig.videoSourceStartSeconds,
              videoSourceEndSeconds: timelineAudioInsertConfig.videoSourceEndSeconds,
            }
        : isTimelineAudioInsert
          ? {
              durationSeconds: generatedDurationSeconds,
              timelineStartSeconds: timelineAudioInsertConfig.timelineStartSeconds,
              videoSourceStartSeconds: timelineAudioInsertConfig.videoSourceStartSeconds,
              videoSourceEndSeconds: timelineAudioInsertConfig.videoSourceEndSeconds,
            }
        : isTextToMusicInsert
          ? {
              ...textToMusicDefaultSettings,
              timelineStartSeconds: timelineTextToMusicInsertConfig.timelineStartSeconds,
              ...(provider === "elevenlabs"
                ? { lyrics: elevenlabsTextToMusicDefaultLyrics }
                : {}),
              ...(provider === "google"
                ? { lyrics: googleLyriaTextToMusicDefaultLyrics }
                : {}),
              ...(provider === "minimax"
                ? { lyrics: minimaxTextToMusicDefaultLyrics }
                : {}),
            }
        : isTimelineVisualInsert
          ? {
                durationSeconds: generatedDurationSeconds,
                timelineStartSeconds: timelineVisualInsertConfig.timelineStartSeconds,
                generateAudio: true,
              }
          : isMultiImage
            ? {
                numImages: outputCount,
              }
          : isTextToAudio &&
              (provider === "openai" || provider === "elevenlabs" || provider === "google")
            ? {
                category: "tts",
                voice: textToAudioDefaultVoices[provider],
              }
          : isVideoToMusic
            ? {
                ...textToMusicDefaultSettings,
              }
          : isTextToMusic
            ? {
                ...textToMusicDefaultSettings,
                ...(provider === "elevenlabs"
                  ? { lyrics: elevenlabsTextToMusicDefaultLyrics }
                  : {}),
                ...(provider === "google"
                  ? { lyrics: googleLyriaTextToMusicDefaultLyrics }
                  : {}),
                ...(provider === "minimax"
                  ? { lyrics: minimaxTextToMusicDefaultLyrics }
                  : {}),
              }
          : isProviderVideoGenerationScenarioForTest(scenario)
            ? {
                generateAudio: true,
              }
          : undefined,
    }),
  );
  if (isRetry) {
    const failedProviderRequest = {
      ...providerRequest,
      requestId: `${requestId}-failed-attempt`,
      statusUrl: `https://api.${safeProvider}.example/v1/${requestId}-failed-attempt`,
      responseUrl: `https://api.${safeProvider}.example/v1/${requestId}-failed-attempt`,
      cancelUrl: `https://api.${safeProvider}.example/v1/${requestId}-failed-attempt/cancel`,
    };
    writeFileSync(
      join(projectDir, "generated", failedRetryAssetId, "asset.json"),
      JSON.stringify({
        id: failedRetryAssetId,
        status: "failed",
        model: { provider, id: model },
        parentAssetId: null,
        retryOfAssetId: null,
        failureReason: "provider_error",
        outputs: [],
      }),
    );
    writeFileSync(
      join(projectDir, "jobs", failedRetryAssetId, "job.json"),
      JSON.stringify({
        id: failedRetryAssetId,
        kind: "generate_media",
        status: "failed",
        updatedAt: "2026-07-05T12:00:00Z",
        providerRequest: failedProviderRequest,
        terminalReason: "provider_error",
      }),
    );
  }
  writeFileSync(
    join(projectDir, "jobs", generatedId, "job.json"),
    JSON.stringify({
      id: generatedId,
      kind: "generate_media",
      status: "completed",
      updatedAt: "2026-07-05T12:00:00Z",
      providerRequest,
    }),
  );
  writeFileSync(
    reportPath,
    JSON.stringify(
      {
        ok: true,
        provider,
        model,
        scenario,
        outDir,
        reportPath,
        credentialEnvVar,
        credentialStatus: isNoSpendMockedProvider ? "not_checked" : "present",
        command: isNoSpendMockedProvider
          ? null
          : "cargo run --manifest-path src-tauri/crates/provider-e2e-harness/Cargo.toml --",
        mockedProviderService: isNoSpendMockedProvider,
        exitCode: 0,
        signal: null,
        result: {
          ok: true,
          provider,
          model,
          scenario,
          projectDir,
          artifactPath: outputPath,
          outputCount,
          jobStatus: "Completed",
          providerRequest,
        },
        error: null,
      },
      null,
      2,
    ),
  );

  return reportPath;
}

function writeReplicateUploadPolicyFixture({
  rootDir,
  requestId,
  providerFileUrl = "https://api.replicate.example/v1/files/mock-file",
}: {
  rootDir: string;
  requestId: string;
  providerFileUrl?: string;
}) {
  const provider = "replicate";
  const model = "black-forest-labs/flux-schnell";
  const scenario = "replicate-local-file-upload";
  const outDir = join(rootDir, "output", "provider-e2e", scenario);
  const artifactPath = join(outDir, "media", "provider-e2e-source.png");
  const reportPath = join(outDir, "provider-e2e-report.json");
  mkdirSync(dirname(artifactPath), { recursive: true });
  writeFileSync(artifactPath, "mock source pixels");
  writeFileSync(
    reportPath,
    JSON.stringify(
      {
        ok: true,
        provider,
        model,
        scenario,
        outDir,
        reportPath,
        credentialEnvVar: "REPLICATE_API_TOKEN",
        credentialStatus: "not_checked",
        command: null,
        mockedProviderService: true,
        exitCode: 0,
        signal: null,
        result: {
          ok: true,
          provider,
          model,
          scenario,
          projectDir: outDir,
          artifactPath,
          outputCount: 0,
          jobStatus: "Uploaded",
          providerFileUrl,
        },
        error: null,
      },
      null,
      2,
    ),
  );
  return reportPath;
}
