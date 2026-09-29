#!/usr/bin/env node
import { existsSync, mkdirSync, readdirSync, rmSync, writeFileSync } from "node:fs";
import { homedir } from "node:os";
import { basename, join, relative, resolve } from "node:path";
import { spawn, spawnSync } from "node:child_process";

const repoRoot = resolve(import.meta.dirname, "..");
const outputDir = resolve(repoRoot, "output/playwright/preview-playback-performance");
const frameDir = join(outputDir, "frames");
const reportPath = join(outputDir, "report.json");
const screenshotPath = join(outputDir, "preview.png");
const appUrl = "http://127.0.0.1:1420";
// Vite serves files under the project root, so the generated frames load from their output path.
const frameUrl = `${appUrl}/${relative(repoRoot, frameDir).split("\\").join("/")}`;
const durationSeconds = 5;
const warmupSeconds = 1;
const fixtureDurationSeconds = 6;
const fixtureFps = 60;
const frameCount = fixtureDurationSeconds * fixtureFps;
const session = `video-creater-preview-performance-${Date.now()}`;
const pwcli = process.env.PWCLI || join(
  process.env.CODEX_HOME || join(homedir(), ".codex"),
  "skills/playwright/scripts/playwright_cli.sh",
);

function run(command, args, options = {}) {
  const result = spawnSync(command, args, {
    cwd: repoRoot,
    encoding: "utf8",
    stdio: options.stdio || "pipe",
  });
  if (result.status !== 0) {
    throw new Error(
      `${command} ${args.join(" ")} failed\n${result.stdout || ""}${result.stderr || ""}`,
    );
  }
  return `${result.stdout || ""}${result.stderr || ""}`;
}

async function waitForUrl(url, timeoutMs = 20_000) {
  const deadline = Date.now() + timeoutMs;
  while (Date.now() < deadline) {
    try {
      const response = await fetch(url);
      if (response.ok) return;
    } catch {
      // Server is still starting.
    }
    await new Promise((resolveWait) => setTimeout(resolveWait, 100));
  }
  throw new Error(`Timed out waiting for ${url}`);
}

function ensureFrames() {
  const existingFrames = existsSync(frameDir)
    ? readdirSync(frameDir).filter((name) => name.endsWith(".png"))
    : [];
  if (existingFrames.length === frameCount) return;

  rmSync(frameDir, { recursive: true, force: true });
  mkdirSync(frameDir, { recursive: true });
  run("ffmpeg", [
    "-hide_banner",
    "-loglevel",
    "error",
    "-f",
    "lavfi",
    "-i",
    `testsrc2=size=1920x1080:rate=${fixtureFps}:duration=${fixtureDurationSeconds}`,
    "-frames:v",
    String(frameCount),
    "-compression_level",
    "4",
    join(frameDir, "frame-%06d.png"),
  ]);
}

function benchmarkProject(mediaId, blendMode = "multiply") {
  const timeline = {
    durationSeconds: fixtureDurationSeconds,
    tracks: [{
      id: "track-video",
      name: "Video",
      kind: "video",
      locked: false,
      syncLocked: false,
      enabled: true,
      items: [{
        id: "benchmark-clip",
        kind: "video_clip",
        startSeconds: 0,
        durationSeconds: fixtureDurationSeconds,
        source: { type: "media", mediaId },
        label: "60 fps prepared preview",
        properties: {
          sourceIn: 0,
          sourceOut: fixtureDurationSeconds,
          speed: 1,
          opacity: 1,
          blendMode,
        },
      }],
    }],
  };
  return {
    schemaVersion: 2,
    id: "preview-performance-project",
    name: "Preview performance benchmark",
    createdAt: "2026-07-11T00:00:00Z",
    updatedAt: "2026-07-11T00:00:00Z",
    media: [{
      id: mediaId,
      relativePath: `cache/${mediaId}.mov`,
      kind: "video",
      durationSeconds: fixtureDurationSeconds,
      width: 1920,
      height: 1080,
      fps: fixtureFps,
      folderId: null,
    }],
    mediaFolders: [],
    generatedAssets: [],
    renderReports: [],
    transcripts: [],
    timeline,
    timelines: [{ id: "main", name: "Timeline 1", timeline }],
    activeTimelineId: "main",
    renderSettings: {
      width: 1920,
      height: 1080,
      fps: fixtureFps,
      loudnessLufs: -14,
      captions: "burn_in",
    },
    codexThreadId: null,
    jobs: [],
  };
}

function runPwcli(args) {
  return run(pwcli, ["--session", session, ...args]);
}

function parseBenchmark(output) {
  const match = output.match(/VIDEO_CREATER_BENCHMARK:([A-Za-z0-9+/=]+)/);
  if (!match) {
    throw new Error(`Playwright output did not contain a benchmark result\n${output}`);
  }
  return JSON.parse(Buffer.from(match[1], "base64").toString("utf8"));
}

async function main() {
  mkdirSync(outputDir, { recursive: true });
  ensureFrames();
  const frameNames = readdirSync(frameDir).filter((name) => name.endsWith(".png")).sort();
  const sourceProject = benchmarkProject("source-media");
  const preparedProject = benchmarkProject("prepared-media", "normal");
  const childProcesses = [];

  try {
    let ownsAppServer = false;
    try {
      await waitForUrl(appUrl, 500);
    } catch {
      const appServer = spawn(
        "pnpm",
        ["exec", "vite", "--host", "127.0.0.1", "--port", "1420", "--strictPort"],
        { cwd: repoRoot, stdio: "ignore" },
      );
      childProcesses.push(appServer);
      ownsAppServer = true;
      await waitForUrl(appUrl);
    }

    await waitForUrl(`${frameUrl}/${frameNames[0]}`);

    runPwcli(["open", appUrl]);
    const setupPayload = {
      preferences: {
        schemaVersion: 2,
        projectLocation: { mode: "ask" },
        requireProviderUploadConfirmation: true,
        renderCompletionNotifications: false,
        newProjectDefaults: { width: 1920, height: 1080, fps: 30, loudnessLufs: -14, captions: "burn_in" },
        enabledGenerationModelIds: [],
        generationExecutionBackend: "inProcess",
      },
      sourceProject,
      preparedProject,
      framePaths: frameNames.map((name) => `cache/${name}`),
      frameUrl,
    };
    runPwcli(["run-code", `async (page) => {
      await page.addInitScript((payload) => {
        let callbackId = 1;
        window.__TAURI_EVENT_PLUGIN_INTERNALS__ = { unregisterListener() {} };
        window.__TAURI_INTERNALS__ = {
          convertFileSrc(filePath) {
            return payload.frameUrl + "/" + filePath.split("/").pop();
          },
          transformCallback() { return callbackId++; },
          unregisterCallback() {},
          async invoke(command) {
            if (command === "load_split_project_from_folder") {
              return structuredClone(payload.sourceProject);
            }
            if (command === "prepare_project_preview") {
              return {
                project: structuredClone(payload.preparedProject),
                reports: [],
                frameSequences: [{
                  itemId: "benchmark-clip",
                  preparedMediaId: "prepared-media",
                  startSeconds: 0,
                  durationSeconds: ${fixtureDurationSeconds},
                  fps: ${fixtureFps},
                  framePaths: payload.framePaths,
                }],
              };
            }
            if (command === "get_platform_info") return { platform: "macos" };
            if (command === "get_app_preferences") return payload.preferences;
            if (command === "sync_native_menu_state" || command === "get_settings_acceptance_context") return null;
            if (command === "list_transcription_models" || command === "list_visual_effect_catalog") return [];
            if (command === "get_active_transcription_model") return null;
            if (command === "get_transcription_runtime_status") return "unavailable";
            if (command.startsWith("plugin:")) return null;
            throw new Error("__TAURI_INTERNALS__ benchmark stub does not implement " + command);
          },
        };
      }, ${JSON.stringify(setupPayload)});
      await page.evaluate(() => {
        window.localStorage.setItem("video-creater.recentProjects", JSON.stringify([{
          id: "recent:preview-performance",
          name: "Preview performance benchmark",
          projectDir: "/preview-performance",
          updatedAtLabel: "Benchmark fixture",
          statusLabel: "Ready",
        }]));
      });
      await page.reload();
      await page.waitForSelector("[aria-label='Project home']", { timeout: 10000 });
      const recent = page.getByLabel("Recent project Preview performance benchmark");
      await recent.getByRole("button", { name: "Open project" }).click();
      await page.waitForSelector("[aria-label='Video editor workspace']", { timeout: 10000 });
      await page.waitForSelector("[data-testid='canonical-prepared-preview-frame']", { timeout: 20000 });
      await page.getByText("Canonical ready").waitFor({ state: "visible", timeout: 10000 });
    }`]);
    runPwcli(["resize", "1920", "1080"]);

    const rawResult = runPwcli(["run-code", `async (page) => {
      const transport = page.getByLabel("Preview transport");
      await transport.getByRole("button", { name: "Play preview" }).click();
      await page.waitForTimeout(${warmupSeconds * 1000});
      await transport.getByRole("button", { name: "Pause preview" }).click();
      const scrubberControl = page.getByLabel("Preview scrubber");
      await scrubberControl.focus();
      await page.keyboard.press("Home");
      await page.waitForTimeout(250);
      const result = await page.evaluate(async ({ durationSeconds, expectedFps }) => {
        const image = document.querySelector("[data-testid='canonical-prepared-preview-frame']");
        if (!(image instanceof HTMLImageElement)) throw new Error("prepared preview image is missing");
        const uniqueFrames = new Set([image.dataset.preparedFrameUrl]);
        const loadedFrames = new Set();
        let frameLoadErrors = 0;
        const recordLoadedFrame = () => {
          if (image.naturalWidth === 1920 && image.naturalHeight === 1080) {
            loadedFrames.add(image.dataset.preparedFrameUrl);
          }
        };
        const recordFrameLoadError = () => { frameLoadErrors += 1; };
        image.addEventListener("load", recordLoadedFrame);
        image.addEventListener("error", recordFrameLoadError);
        if (image.complete) recordLoadedFrame();
        const frameTimes = [];
        let maximumRafIntervalMs = 0;
        let previousRaf = performance.now();
        const observer = new MutationObserver(() => {
          uniqueFrames.add(image.dataset.preparedFrameUrl);
        });
        observer.observe(image, { attributes: true, attributeFilter: ["data-prepared-frame-url"] });
        const sampleRaf = (timestamp) => {
          const interval = timestamp - previousRaf;
          previousRaf = timestamp;
          frameTimes.push(interval);
          maximumRafIntervalMs = Math.max(maximumRafIntervalMs, interval);
          if (performance.now() < deadline) requestAnimationFrame(sampleRaf);
        };
        const startedAt = performance.now();
        const deadline = startedAt + durationSeconds * 1000;
        document.querySelector("button[aria-label='Play preview']")?.click();
        requestAnimationFrame(sampleRaf);
        await new Promise((resolveWait) => setTimeout(resolveWait, durationSeconds * 1000));
        observer.disconnect();
        image.removeEventListener("load", recordLoadedFrame);
        image.removeEventListener("error", recordFrameLoadError);
        const elapsedSeconds = (performance.now() - startedAt) / 1000;
        const sortedIntervals = frameTimes.slice(1).sort((a, b) => a - b);
        const p95RafIntervalMs = sortedIntervals[Math.floor(sortedIntervals.length * 0.95)] || 0;
        return {
          schemaVersion: 1,
          status: "measured",
          viewport: { width: innerWidth, height: innerHeight },
          fixture: { width: image.naturalWidth, height: image.naturalHeight, fps: expectedFps },
          warmupSeconds: ${warmupSeconds},
          elapsedSeconds,
          rafSamples: frameTimes.length,
          meanRafFps: frameTimes.length / elapsedSeconds,
          p95RafIntervalMs,
          maximumRafIntervalMs,
          uniquePreparedFrames: uniqueFrames.size,
          effectivePreparedFps: uniqueFrames.size / elapsedSeconds,
          loadedPreparedFrames: loadedFrames.size,
          effectiveLoadedPreparedFps: loadedFrames.size / elapsedSeconds,
          frameLoadErrors,
        };
      }, { durationSeconds: ${durationSeconds}, expectedFps: ${fixtureFps} });
      const encoded = await page.evaluate((value) => btoa(JSON.stringify(value)), result);
      return "VIDEO_CREATER_BENCHMARK:" + encoded;
    }`]);
    const report = parseBenchmark(rawResult);
    report.thresholds = {
      minimumMeanRafFps: 50,
      minimumEffectivePreparedFps: 45,
      minimumEffectiveLoadedPreparedFps: 45,
      maximumP95RafIntervalMs: 25,
      maximumRafIntervalMs: 120,
    };
    const failures = [];
    if (report.fixture.width !== 1920 || report.fixture.height !== 1080) failures.push("fixture did not decode at 1920x1080");
    if (report.meanRafFps < report.thresholds.minimumMeanRafFps) failures.push("mean rAF cadence was below 50 fps");
    if (report.effectivePreparedFps < report.thresholds.minimumEffectivePreparedFps) failures.push("prepared-frame cadence was below 45 fps");
    if (report.effectiveLoadedPreparedFps < report.thresholds.minimumEffectiveLoadedPreparedFps) failures.push("decoded prepared-frame cadence was below 45 fps");
    if (report.frameLoadErrors !== 0) failures.push("prepared frames emitted load errors");
    if (report.p95RafIntervalMs > report.thresholds.maximumP95RafIntervalMs) failures.push("p95 rAF interval exceeded 25 ms");
    if (report.maximumRafIntervalMs > report.thresholds.maximumRafIntervalMs) failures.push("maximum rAF interval exceeded 120 ms");
    report.failures = failures;
    report.status = failures.length === 0 ? "passed" : "failed";
    report.appServerStartedByBenchmark = ownsAppServer;
    writeFileSync(reportPath, `${JSON.stringify(report, null, 2)}\n`);
    runPwcli(["screenshot", "--filename", screenshotPath]);
    console.log(JSON.stringify({ reportPath, screenshotPath, ...report }, null, 2));
    if (failures.length > 0) process.exitCode = 1;
  } finally {
    try { runPwcli(["close"]); } catch { /* Best-effort browser cleanup. */ }
    for (const child of childProcesses.reverse()) child.kill("SIGTERM");
  }
}

await main();
