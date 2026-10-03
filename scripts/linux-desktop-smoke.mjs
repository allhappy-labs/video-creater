#!/usr/bin/env node
// Drives the real Linux desktop app (WebKitGTK webview + Rust backend) through tauri-driver
// and WebKitWebDriver on an Xvfb display, recording screenshots and backend evidence.
//
// Usage: dbus-run-session -- node scripts/linux-desktop-smoke.mjs \
//   --app src-tauri/target/debug/video-creater \
//   --tauri-driver ~/.cargo/bin/tauri-driver --native-driver /usr/bin/WebKitWebDriver \
//   [--dev-server] [--out output/linux-desktop-smoke]
//
// With --dev-server the Vite development server is started for debug builds that load the
// development URL. Release builds embed the frontend and do not need it.

import { spawnSync } from "node:child_process";
import { existsSync, mkdirSync, mkdtempSync, readdirSync, readFileSync, readlinkSync, rmSync, statSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";

import { agentFlowStepNames, agentSelfTestComponentIds, agentSelfTestVerdict, runAgentFlowSteps } from "./linux-desktop-smoke-agent.mjs";
import { audioDenoiseEvidence } from "./linux-desktop-smoke-denoise.mjs";
import { exportTaskStepNames, runExportTaskSteps } from "./linux-desktop-smoke-export-tasks.mjs";
import { createDriver, keys, poll, sleep, waitForPort } from "./linux-desktop-smoke-driver.mjs";
import {
  backgroundTasks,
  backgroundTasksLabel,
  editorMenu,
  editorWorkspace,
  exportButton,
  exportChoice,
  exportPopover,
  openExportPopover,
  openSampleEditor,
  projectHome,
  settingsTab,
  waitForEditor,
} from "./linux-desktop-smoke-selectors.mjs";
import { nativeMenuStepNames, runNativeMenuSteps } from "./linux-desktop-smoke-native-menu.mjs";
import { createProcessGroup } from "./linux-desktop-smoke-processes.mjs";
import {
  newExportArtifacts,
  readRenderPipelineReport,
  renderPipelineReportPath,
  requireSucceededRenderReport,
  resolveArtifactPath,
  retainProjectFiles,
} from "./linux-desktop-smoke-retain.mjs";
import { createStepRunner, exitCodeFor, parseSmokeOptions, redactSmokeMediaTokens, skipped, smokeRunContext, summarizeSteps } from "./linux-desktop-smoke-steps.mjs";
import { runTemporalSteps, temporalStepNames } from "./linux-desktop-smoke-temporal.mjs";
import { extractedToolEnvironment, extractedToolPath, stopExtractedToolProcesses } from "./linux-desktop-smoke-tools.mjs";

const repoRoot = resolve(import.meta.dirname, "..");

const options = parseSmokeOptions(process.argv.slice(2));
const { app, display, out: outDir } = options;
const fixtureMedia = resolve(repoRoot, "src-tauri/tests/fixtures/media/edison-speech-1920s-30s.mp4");
const processes = createProcessGroup({ cwd: repoRoot });
const { start } = processes;
const evidence = { app, display, steps: [] };
const driver = createDriver({ port: 4444, outDir });
const { attribute, click, execute, find, invoke, pressKeys, screenshot, type } = driver;

mkdirSync(outDir, { recursive: true });

const step = createStepRunner({
  evidence,
  only: options.only,
  onFailure: (name) => screenshot(`failed-${name.replace(/[^a-z0-9]+/gi, "-")}`),
});

async function main() {
  start("Xvfb", [display, "-screen", "0", "1600x1000x24", "-nolisten", "tcp"]);
  await sleep(1000);
  if (options.devServer) {
    start("pnpm", ["dev:web-runtime"]);
    await waitForPort(1420, 60_000);
  }
  if (options.temporal) {
    start(options.temporalCli, ["server", "start-dev", "--headless", "--log-level", "error"]);
    await waitForPort(7233, 60_000);
    start(options.temporalWorker, []);
    await sleep(3000);
  }
  start(options.tauriDriver, ["--port", "4444", "--native-driver", options.nativeDriver], {
    ...(process.env.SMOKE_GST_DEBUG ? { GST_DEBUG: process.env.SMOKE_GST_DEBUG, WEBKIT_DEBUG: "Media" } : {}),
    DISPLAY: display,
    // Headless hosts have no audio server; route autoaudiosink to a fake sink for the smoke run.
    ...(options.fakeAudio
      ? { GST_PLUGIN_FEATURE_RANK: "fakeaudiosink:MAX,pulsesink:NONE,alsasink:NONE,openalsink:NONE,oss4sink:NONE,osssink:NONE" }
      : {}),
    WEBKIT_DISABLE_COMPOSITING_MODE: "1",
    LIBGL_ALWAYS_SOFTWARE: "1",
  });
  if (options.keyringRoot) {
    // A private unlocked GNOME Keyring provides org.freedesktop.secrets on this session bus.
    const root = resolve(options.keyringRoot);
    const keyring = spawnSync(extractedToolPath(root, "gnome-keyring-daemon"), ["--unlock", "--components=secrets", "--daemonize"], {
      input: "smoke-test-password",
      env: extractedToolEnvironment(root, process.env, { XDG_DATA_HOME: mkdtempSync(join(tmpdir(), "vc-smoke-keyring-")) }),
      encoding: "utf8",
    });
    if (keyring.status !== 0) throw new Error(`gnome-keyring-daemon failed: ${keyring.error?.message ?? keyring.stderr}`);
  }
  await waitForPort(4444, 30_000);
  await driver.startSession({ alwaysMatch: { "tauri:options": { application: app } } });

  await step("project home renders", async () => {
    await find(projectHome, 60_000);
    // Preferences persist across runs; start from the default desktop execution backend.
    await invoke("update_app_preferences", { patch: { generationExecutionBackend: "inProcess" } });
    await execute("window.location.reload();");
    await find(projectHome, 60_000);
    return { screenshot: await screenshot("01-project-home") };
  });

  const platform = await step("backend reports Linux platform", async () => {
    const info = await invoke("get_platform_info");
    if (info?.platform !== "linux") throw new Error(`unexpected platform ${JSON.stringify(info)}`);
    return info;
  });
  void platform;

  await step("render system health is ready with Linux delivery", async () =>
    poll(
      () => invoke("get_render_system_health"),
      (health) =>
        health.compositionReady &&
        health.items.some((item) => item.id === "render.linuxDelivery" && item.state === "ready"),
      120_000,
      2000,
    ),
  );

  await step("export profiles report Linux codecs", async () => {
    const report = await invoke("get_export_profile_availability_report");
    const byProfile = Object.fromEntries(report.map((entry) => [entry.profile, entry]));
    for (const profile of ["webm", "mp4H264", "proResMov"]) {
      if (!byProfile[profile]?.available) throw new Error(`${profile} unavailable: ${byProfile[profile]?.unavailableReason}`);
    }
    return report.map(({ profile, available, requiredRuntime, unavailableReason }) => ({ profile, available, requiredRuntime, unavailableReason }));
  });

  await step("desktop capabilities are reported", async () => ({
    notifications: await invoke("get_notification_capability"),
    transcriptionRuntime: await invoke("get_transcription_runtime_status"),
    credentials: await invoke("list_provider_credential_statuses"),
  }));

  const projectDir = mkdtempSync(join(tmpdir(), "video-creater-smoke-"));
  const projectPath = join(projectDir, "Linux Smoke.palmier");
  await step("create a project from the project home", async () => {
    await click("//button[@aria-label='New project']");
    await type("//input[@id='project-folder-path']", projectPath);
    await click("//form//button[@type='submit' and @aria-label='Create project']");
    await waitForEditor(driver);
    await sleep(2000);
    return { projectPath, screenshot: await screenshot("02-empty-editor") };
  });

  const imported = await step("import H.264/AAC media through the backend", async () => {
    const project = await invoke("load_split_project_from_folder", { projectDir: projectPath });
    const result = await invoke("import_media_to_project", {
      projectDir: projectPath,
      project,
      sourcePaths: [fixtureMedia],
    });
    const media = result.project.media.at(-1);
    if (!media || !(media.durationSeconds > 29) || !media.width) {
      throw new Error(`unexpected imported media ${JSON.stringify(media)}`);
    }

    return { mediaId: media.id, durationSeconds: media.durationSeconds, width: media.width, height: media.height, fps: media.fps };
  });
  void imported;

  await step("open the bundled sample in the editor", async () => {
    await openSampleEditor(driver);
    await sleep(4000);
    return { screenshot: await screenshot("03-sample-editor") };
  });

  await step("webview media playback decodes through the reviewed runtime", async () => {
    const state = await poll(
      () =>
        execute(`const videos = [...document.querySelectorAll('video')];
          return videos.map((video) => ({ src: video.currentSrc, readyState: video.readyState, width: video.videoWidth, duration: video.duration, error: video.error && video.error.code }));`),
      (videos) =>
        videos.length > 0 &&
        videos.every((video) => video.src.startsWith("http://127.0.0.1:") && video.error === null) &&
        videos.some((video) => video.readyState >= 2 && video.width > 0),
      60_000,
    );
    await execute(`const video = [...document.querySelectorAll('video')].find((v) => v.readyState >= 2);
      video.muted = true; video.play();`);
    await sleep(3000);
    const advanced = await execute(`return [...document.querySelectorAll('video')].map((video) => ({ time: video.currentTime, error: video.error && video.error.code }));`);
    if (!advanced.some((entry) => entry.time > 0.5)) throw new Error(`playback did not advance: ${JSON.stringify(advanced)}`);
    const seek = await driver.executeAsync(
      `const done = arguments[arguments.length - 1];
        const video = [...document.querySelectorAll('video')].find((v) => v.readyState >= 2);
        video.pause();
        video.addEventListener('seeked', () => done({ currentTime: video.currentTime, readyState: video.readyState }), { once: true });
        video.currentTime = Math.min(2.5, video.duration / 2);
        setTimeout(() => done({ timeout: true, currentTime: video.currentTime }), 10000);`,
    );
    if (seek.timeout) throw new Error(`seek did not complete: ${JSON.stringify(seek)}`);
    return { videos: state, playback: advanced, seek, screenshot: await screenshot("04-sample-playback") };
  });

  await step("export the sample to MP4 H.264 from the export popover", async () => {
    const projectDir = "/tmp/video-creater-editor-project";
    const loadProject = () => invoke("load_split_project_from_folder", { projectDir });
    await execute(`if (!window.__vcInvokeFailures) {
      window.__vcInvokeFailures = [];
      const original = window.__TAURI_INTERNALS__.invoke.bind(window.__TAURI_INTERNALS__);
      window.__TAURI_INTERNALS__.invoke = (command, args, options) => original(command, args, options).catch((error) => {
        window.__vcInvokeFailures.push({ command, args: JSON.stringify(args).slice(0, 4000), error: typeof error === 'string' ? error : JSON.stringify(error) });
        throw error;
      });
    }`);
    const before = (await loadProject()).exportArtifacts;
    await openExportPopover(driver);
    await click(exportChoice("MP4"));
    await click(exportChoice("720p"));
    const choices = await poll(
      () =>
        execute(`const popover = document.querySelector("[role='dialog'][aria-label='Export']");
          return { checked: [...popover.querySelectorAll("[role='radio'][aria-checked='true']")].map((radio) => radio.textContent.trim()), text: popover.innerText };`),
      (state) => state.checked.includes("MP4") && state.checked.includes("720p") && state.text.includes("H.264"),
      10_000,
      500,
    );
    await sleep(500);
    await screenshot("05-export-sheet");
    await click(`${exportPopover}//button[normalize-space()='Export video']`);
    // Progress labels are informational: the pill may skip straight to "complete" for short renders.
    const progressLabels = [];
    const sampleLabel = async () => {
      const label = await backgroundTasksLabel(driver);
      if (label && !progressLabels.includes(label)) progressLabels.push(label);
      return label;
    };
    const artifact = await poll(
      async () => {
        await sampleLabel();
        const [found] = newExportArtifacts(before, (await loadProject()).exportArtifacts, "mp4");
        const path = found && resolveArtifactPath(projectDir, found.path);
        return path && existsSync(path) && statSync(path).size > 0 ? found : undefined;
      },
      async (found) => {
        if (found) return true;
        const failures = await execute("return window.__vcInvokeFailures || [];");
        if (failures.length > 0) throw new Error(`backend failures: ${JSON.stringify(failures)}`);
        return false;
      },
      300_000,
      2000,
    );
    const output = resolveArtifactPath(projectDir, artifact.path);
    // The export is only verified by its pipeline report; a missing report fails the step.
    const { path: reportPath, report } = requireSucceededRenderReport(
      await poll(() => readRenderPipelineReport(projectDir, artifact.jobId), Boolean, 30_000, 1000).catch(() => null),
      artifact.jobId,
    );
    // The Background tasks pill is the editor's only job surface; it reports the finished export.
    const tasksLabel = await poll(
      sampleLabel,
      (label) => {
        if (label && /failed/i.test(label)) throw new Error(`Background tasks reports ${label}`);
        return Boolean(label && / complete$/.test(label));
      },
      60_000,
    );
    await click(backgroundTasks);
    await find("//*[@role='dialog' and @aria-label='Background tasks']");
    await sleep(1000);
    const shot = await screenshot("06-export-complete");
    await pressKeys(keys.escape);
    return {
      artifact,
      output,
      bytes: statSync(output).size,
      choices: choices.checked,
      tasksLabel,
      progressLabels,
      summary: report.summary,
      streams: report.streams ?? null,
      reportPath,
      retained: retainProjectFiles(projectDir, [reportPath, `renders/${artifact.jobId}/report.json`], join(outDir, "export-mp4")),
      screenshot: shot,
    };
  });

  await step("settings reports Linux system health", async () => {
    await find(editorWorkspace);
    await click(editorMenu);
    await click("//*[@role='menuitem' and normalize-space()='App settings']");
    await find(settingsTab("Advanced"), 30_000);
    await sleep(3000);
    const text = await execute("return document.body.innerText;");
    const shot = await screenshot("07-settings");
    if (/Keychain|Finder|Core ML|AVFoundation/.test(text)) {
      throw new Error(`macOS-only wording visible on Linux: ${text.match(/.{0,60}(Keychain|Finder|Core ML|AVFoundation).{0,60}/)?.[0]}`);
    }
    return { screenshot: shot };
  });

  await step("audio denoise renders through the Linux DeepFilterNet3 helper", async () => {
    const projectDir = "/tmp/video-creater-editor-project";
    const project = await invoke("load_split_project_from_folder", { projectDir });
    const item = project.timeline.tracks
      .flatMap((track) => track.items)
      .find((candidate) => candidate.kind === "audio_clip");
    if (!item) throw new Error("sample audio clip is missing");
    await invoke("apply_project_actions_to_split_project_folder", {
      projectDir,
      actions: [
        {
          type: "updateItemEffects",
          itemIds: [item.id],
          effects: [
            {
              effectInstanceId: "smoke-denoise",
              effectType: "audio.denoise",
              enabled: true,
              params: { amount: 0.6 },
            },
          ],
        },
      ],
    });
    const jobId = `render-denoise-${Date.now()}`;
    const result = await invoke("render_media_to_split_project_folder", {
      projectDir,
      projectId: project.id,
      profile: "webm",
      quality: "draft",
      width: 640,
      height: 360,
      jobId,
      attemptId: `render-attempt/${jobId}`,
      updatedAt: new Date().toISOString(),
    });
    const pipelineReport = JSON.parse(readFileSync(join(projectDir, renderPipelineReportPath(projectDir, jobId)), "utf8"));
    const latest = await invoke("load_split_project_from_folder", { projectDir });
    const denoised = audioDenoiseEvidence(projectDir, pipelineReport);
    if (result.renderReport.summary.status !== "succeeded") throw new Error(`render status ${result.renderReport.summary.status}`);
    if (denoised.length === 0) {
      throw new Error("render report lists no committed audio denoise cache entry");
    }
    await invoke("apply_project_actions_to_split_project_folder", {
      projectDir,
      actions: [{ type: "updateItemEffects", itemIds: [item.id], effects: [] }],
    });
    const retained = retainProjectFiles(
      projectDir,
      [renderPipelineReportPath(projectDir, jobId), ...denoised.map((entry) => entry.manifest)],
      join(outDir, "denoise"),
    );
    return {
      output: result.outputPath,
      denoised,
      retained,
      jobs: latest.jobs.filter((job) => job.id === jobId).map(({ id, status }) => ({ id, status })),
    };
  });

  await step("NLE XML export from the export popover footer", async () => {
    const projectDir = "/tmp/video-creater-editor-project";
    const exportsDir = join(projectDir, "exports");
    const startedAt = Date.now();
    await openSampleEditor(driver);
    await sleep(2000);
    await openExportPopover(driver);
    const premiereXml = `${exportPopover}//button[normalize-space()='Premiere XML']`;
    await find(`${exportPopover}//button[normalize-space()='Project package']`);
    evidence.exportPopoverLayout = await execute(`const popover = document.querySelector("[role='dialog'][aria-label='Export']");
      const links = [...popover.querySelectorAll('button')]
        .filter((button) => ['Premiere XML', 'DaVinci XML', 'Project package'].includes(button.textContent.trim()))
        .map((button) => ({ label: button.textContent.trim(), rect: button.getBoundingClientRect().toJSON(), disabled: button.getAttribute('aria-disabled') === 'true' }));
      return { rect: popover.getBoundingClientRect().toJSON(), links, viewport: [innerWidth, innerHeight] };`);
    if (evidence.exportPopoverLayout.links.some((link) => link.disabled)) {
      throw new Error(`export footer links are disabled: ${JSON.stringify(evidence.exportPopoverLayout.links)}`);
    }
    await screenshot("export-popover");
    await click(premiereXml);
    const output = await poll(
      () =>
        (existsSync(exportsDir) ? readdirSync(exportsDir) : [])
          .filter((name) => /\.(xml|fcpxml)$/.test(name))
          .map((name) => join(exportsDir, name))
          .find((path) => statSync(path).mtimeMs >= startedAt),
      Boolean,
      60_000,
    );
    const xml = readFileSync(output, "utf8");
    if (!xml.includes("<xmeml")) throw new Error("Premiere XML output is not XMEML");
    return { output, bytes: xml.length };
  });

  await step("Palmier project package export from the export popover footer", async () => {
    const exportsDir = "/tmp/video-creater-editor-project/exports";
    const startedAt = Date.now();
    await find(exportButton, 60_000);
    // A footer link closes the popover when its export starts; open it again.
    await openExportPopover(driver);
    await click(`${exportPopover}//button[normalize-space()='Project package']`);
    const output = await poll(
      () =>
        (existsSync(exportsDir) ? readdirSync(exportsDir) : [])
          .filter((name) => name.endsWith(".palmier"))
          .map((name) => join(exportsDir, name))
          .find((path) => statSync(path).mtimeMs >= startedAt),
      Boolean,
      120_000,
    );
    const files = readdirSync(output);
    if (!files.includes("video-creater.project.json") || !files.includes("project.json")) {
      throw new Error(`Palmier package is missing its manifest: ${files.join(", ")}`);
    }
    return { output, files };
  });

  await step("Ctrl+Z in the webview undoes a timeline delete", async () => {
    // Linux Undo/Redo menu items carry no accelerator, so Ctrl+Z must reach the webview keydown handler.
    const projectDir = "/tmp/video-creater-editor-project";
    const savedItemIds = async () =>
      (await invoke("load_split_project_from_folder", { projectDir })).timeline.tracks.flatMap((track) => track.items.map((item) => item.id));
    await openSampleEditor(driver);
    await sleep(2000);
    const clip = await find(`${editorWorkspace}//*[@role='option' and @data-item-id]`);
    const itemId = await attribute(clip, "data-item-id");
    const clipSelector = `main[aria-label='Video editor workspace'] [role='option'][data-item-id='${itemId}']`;
    if (!(await savedItemIds()).includes(itemId)) throw new Error(`clip ${itemId} is not in the saved sample project`);
    await driver.clickElement(clip);
    await poll(() => attribute(clip, "aria-selected"), (selected) => selected === "true", 10_000, 250);
    await pressKeys(keys.delete);
    await poll(() => execute(`return document.querySelector(${JSON.stringify(clipSelector)}) === null;`), Boolean, 15_000, 250);
    await poll(savedItemIds, (ids) => !ids.includes(itemId), 30_000, 500);
    const deleted = await screenshot("undo-01-clip-deleted");
    await pressKeys(keys.control, "z");
    await poll(() => execute(`return document.querySelector(${JSON.stringify(clipSelector)}) !== null;`), Boolean, 15_000, 250);
    await poll(savedItemIds, (ids) => ids.includes(itemId), 30_000, 500);
    return { itemId, screenshots: [deleted, await screenshot("undo-02-clip-restored")] };
  });

  if (options.nativeMenu) await runNativeMenuSteps({ driver, step, options, outDir });
  else for (const name of nativeMenuStepNames) await step(name, async () => skipped("run with --native-menu --xdotool-root <dir>"));

  await step("provider credentials round-trip through the Secret Service", async () => {
    if (!options.keyringRoot) return skipped("run with --keyring-root <extracted gnome-keyring root>");
    const saved = await invoke("set_provider_credential", { provider: "openai", credential: "sk-linux-smoke-test" });
    const listed = (await invoke("list_provider_credential_statuses")).find((entry) => entry.provider === "openai");
    const settingsText = await execute("return document.body.innerText;");
    if (settingsText.includes("sk-linux-smoke-test")) throw new Error("secret leaked into the UI");
    const deleted = await invoke("delete_provider_credential", { provider: "openai" });
    const after = (await invoke("list_provider_credential_statuses")).find((entry) => entry.provider === "openai");
    if (!listed?.configured || listed.source !== "keychain" || after?.configured) {
      throw new Error(`unexpected credential states ${JSON.stringify({ saved, listed, deleted, after })}`);
    }
    return { saved, listed, after };
  });

  await step("agent and MCP self-tests pass", async () => {
    const health = await invoke("get_agent_settings_health");
    const results = {};
    for (const componentId of agentSelfTestComponentIds) {
      const started = await invoke("run_agent_component_self_test", { componentId });
      const operation = await poll(
        async () => (await invoke("list_settings_operations")).find((entry) => entry.id === started.id),
        (entry) => entry && ["succeeded", "failed", "cancelled"].includes(entry.state),
        120_000,
      );
      results[componentId] = { state: operation.state, message: operation.message ?? operation.summary ?? null, error: operation.error ?? null };
    }
    // The support rows must pass; the two backends are alternatives, so one ready backend is a
    // pass even when the other is missing or rate-limited.
    const verdict = agentSelfTestVerdict(results);
    if (!verdict.ok) throw new Error(`${verdict.reason}: ${JSON.stringify({ results, health })}`);
    // Every row's diagnosis is recorded, not just its state: a row that is not ready is only
    // actionable evidence when the run says why.
    return {
      results,
      readyBackends: verdict.readyBackends,
      unavailableBackends: verdict.unavailableBackends,
      items: health.items?.map(({ id, state, diagnosticCode, summary, detail, provenance }) => ({ id, state, diagnosticCode, summary, detail, provenance })),
    };
  });

  if (options.exportTasks) await runExportTaskSteps({ driver, step, options, fixtureMedia, outDir });
  else for (const name of exportTaskStepNames) await step(name, async () => skipped("run with --export-tasks"));

  if (options.agentFlows) await runAgentFlowSteps({ driver, step, outDir, backend: options.agentBackend, claudeModel: options.claudeModel });
  else for (const name of agentFlowStepNames) await step(name, async () => skipped("run with --agent-flows"));

  await step("semantic search reports its Linux encoder status", async () => {
    const result = await invoke("search_project_media", {
      projectDir: "/tmp/video-creater-editor-project",
      query: "man speaking",
      limit: 5,
    });
    return JSON.parse(JSON.stringify(result, (key, value) => (key === "embedding" ? undefined : value)));
  });

  await step("running app processes load no GPL media libraries", async () => {
    // Preview playback and export have loaded codecs by now; inspect every related process.
    const denied = /(?:^|\/)(?:libavcodec|libavformat|libavutil|libavfilter|libswscale|libswresample|libpostproc)\.so|libx264|libx265|libfaad|libmpeg2|libdvdread|libdvdnav|libgstfaad|libgstx265|libgstx264|\/usr\/lib\/[^ ]*gstreamer-1\.0\/libgstlibav\.so/;
    const inspected = [];
    const findings = [];
    for (const pid of readdirSync("/proc").filter((entry) => /^\d+$/.test(entry))) {
      let cmdline;
      let maps;
      try {
        cmdline = readFileSync(`/proc/${pid}/cmdline`, "utf8").replaceAll("\0", " ");
        const executable = readlinkSync(`/proc/${pid}/exe`).split("/").at(-1);
        if (!/^video-creater|^WebKit(?:Web|Network|GPU)Process$/.test(executable)) continue;
        maps = readFileSync(`/proc/${pid}/maps`, "utf8");
      } catch {
        continue;
      }
      const libraries = [...new Set(maps.split("\n").map((line) => line.split(/\s+/).slice(5).join(" ")).filter((path) => path.includes(".so")))];
      inspected.push({ pid: Number(pid), command: cmdline.slice(0, 120), libraries: libraries.length });
      for (const library of libraries.filter((path) => denied.test(path))) findings.push({ pid: Number(pid), library });
    }
    if (inspected.length === 0) throw new Error("no app processes were found to inspect");
    if (findings.length > 0) throw new Error(`GPL media libraries loaded: ${JSON.stringify(findings)}`);
    return { inspected };
  });


  if (options.temporal) await runTemporalSteps({ driver, step, projectPath, outDir });
  else for (const name of temporalStepNames) await step(name, async () => skipped("run with --temporal"));
}

// The smoke changes app preferences (for example Temporal execution); restore the user's copy afterwards.
const preferencesPath = join(
  process.env.XDG_DATA_HOME ?? join(process.env.HOME ?? "", ".local/share"),
  "com.olhapi.video-creater/settings/preferences.json",
);
const savedPreferences = existsSync(preferencesPath) ? readFileSync(preferencesPath) : null;

let exitCode = 0;
try {
  evidence.runContext = smokeRunContext({
    options,
    env: process.env,
    commit: spawnSync("git", ["rev-parse", "HEAD"], { cwd: repoRoot, encoding: "utf8" }).stdout?.trim() || null,
    releaseReport: options.releaseReport ? JSON.parse(readFileSync(resolve(options.releaseReport), "utf8")) : undefined,
  });
  // A background process that fails to start (for example a missing binary) ends the run.
  await Promise.race([main(), processes.failure]);
} catch (error) {
  evidence.fatal = String(error);
  exitCode = 1;
} finally {
  // Every cleanup runs even when an earlier one fails, and the evidence is always written.
  const cleanup = async (name, action) => {
    try {
      await action();
    } catch (error) {
      (evidence.cleanupErrors ??= []).push(`${name}: ${String(error)}`);
    }
  };
  await cleanup("end WebDriver session", () => driver.endSession());
  processes.stopAll(outDir);
  if (options.keyringRoot) await cleanup("stop keyring", () => stopExtractedToolProcesses(options.keyringRoot));
  await sleep(1000);
  await cleanup("restore preferences", () => {
    if (savedPreferences) writeFileSync(preferencesPath, savedPreferences);
    else rmSync(preferencesPath, { force: true });
  });
  evidence.summary = summarizeSteps(evidence.steps);
  exitCode = exitCodeFor(evidence);
  writeFileSync(join(outDir, "evidence.json"), `${JSON.stringify(redactSmokeMediaTokens(evidence), null, 2)}\n`);
  console.log(JSON.stringify({ exitCode, evidence: join(outDir, "evidence.json") }));
  process.exit(exitCode);
}
