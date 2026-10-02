#!/usr/bin/env node
import {
  existsSync,
  mkdirSync,
  statSync,
  unlinkSync,
} from "node:fs";
import { basename, dirname, join, resolve } from "node:path";
import { spawn, spawnSync } from "node:child_process";
import { fileURLToPath } from "node:url";
import { tmpdir } from "node:os";
import { createServer as createTcpServer } from "node:net";

const defaultOutputDir = "output/playwright/browser-visual-qa";
const defaultUrl = "http://127.0.0.1:4179";
const recentProjectsStorageKey = "video-creater.recentProjects";
const settingsVisualQaSessionKey = "video-creater:settings-visual-qa";

const browserVisualQaAppPreferences = {
  schemaVersion: 2,
  projectLocation: { mode: "ask" },
  requireProviderUploadConfirmation: true,
  renderCompletionNotifications: false,
  newProjectDefaults: {
    width: 1920,
    height: 1080,
    fps: 30,
    loudnessLufs: -14,
    captions: "burn_in",
  },
  enabledGenerationModelIds: [],
  generationExecutionBackend: "inProcess",
};

const visualExportCapabilities = [
  {
    profile: "mp4H264",
    label: "MP4 / H.264",
    available: true,
    container: "mp4",
    extension: "mp4",
    mimeType: "video/mp4",
    videoCodec: "h264",
    audioCodec: "aac",
    requiredRuntime: ["visual-qa:avfoundation-h264"],
    policyStatus: "approved",
    unavailableReason: null,
    qualityAvailability: { draft: true, final: true },
    qualityUnavailableReasons: { draft: null, final: null },
  },
  {
    profile: "mp4H265",
    label: "MP4 / H.265",
    available: true,
    container: "mp4",
    extension: "mp4",
    mimeType: "video/mp4",
    videoCodec: "h265",
    audioCodec: "aac",
    requiredRuntime: ["visual-qa:avfoundation-hevc"],
    policyStatus: "approved",
    unavailableReason: null,
    qualityAvailability: { draft: true, final: true },
    qualityUnavailableReasons: { draft: null, final: null },
  },
  {
    profile: "proResMov",
    label: "ProRes MOV",
    available: true,
    container: "mov",
    extension: "mov",
    mimeType: "video/quicktime",
    videoCodec: "prores",
    audioCodec: "pcm",
    requiredRuntime: ["visual-qa:avfoundation-prores"],
    policyStatus: "approved",
    unavailableReason: null,
    qualityAvailability: { draft: false, final: true },
    qualityUnavailableReasons: {
      draft: "ProRes Proxy is unavailable in this native visual-QA fixture.",
      final: null,
    },
  },
  {
    profile: "webm",
    label: "WebM",
    available: true,
    container: "webm",
    extension: "webm",
    mimeType: "video/webm",
    videoCodec: "vp9",
    audioCodec: "opus",
    requiredRuntime: ["visual-qa:packaged-webm"],
    policyStatus: "approved",
    unavailableReason: null,
    qualityAvailability: { draft: true, final: true },
    qualityUnavailableReasons: { draft: null, final: null },
  },
];

const settingsVisualQaScenarioDefinitions = [
  { fixtureId: "settings-general-no-project", fixtureSurface: "appSettings", category: "general", categoryLabel: "General", projectContext: "none", expectedMarker: "Provider upload confirmation" },
  { fixtureId: "settings-projects-no-project", fixtureSurface: "appSettings", category: "projects", categoryLabel: "Projects", projectContext: "none", expectedMarker: "New project defaults" },
  {
    fixtureId: "settings-ai-models-no-project",
    fixtureSurface: "appSettings",
    category: "aiModels",
    categoryLabel: "AI & Models",
    projectContext: "none",
    expectedMarker: "Generation models",
    captureState: "generationModelSelectorOpen",
    captureContext: { marker: "GPT Image 1", actionName: "GPT Image 1, OpenAI" },
  },
  { fixtureId: "settings-integrations-no-project", fixtureSurface: "appSettings", category: "integrations", categoryLabel: "Integrations", projectContext: "none", expectedMarker: "Integration status" },
  { fixtureId: "settings-storage-no-project", fixtureSurface: "appSettings", category: "storage", categoryLabel: "Storage", projectContext: "none", expectedMarker: "Storage inventory" },
  { fixtureId: "settings-advanced-no-project", fixtureSurface: "appSettings", category: "advanced", categoryLabel: "Advanced", projectContext: "none", expectedMarker: "Open a project to copy MCP configuration" },
  {
    fixtureId: "project-settings-render-cleanup",
    fixtureSurface: "projectSettings",
    category: null,
    projectContext: "active",
    expectedMarker: "Confirm render artifact cleanup",
    captureState: "renderArtifactCleanupOpen",
    captureContext: { marker: "Confirm render artifact cleanup", actionName: "Delete reviewed render files" },
  },
  { fixtureId: "system-health-project-failed", fixtureSurface: "systemHealth", category: null, projectContext: "active", expectedMarker: "GStreamer composition probe failed locally" },
];

export const settingsVisualQaScenarios = settingsVisualQaScenarioDefinitions.flatMap(
  (definition) => [
    { ...definition, captureState: definition.captureState ?? null, captureContext: definition.captureContext ?? null, id: `${definition.fixtureId}-desktop`, surface: "settings-state", viewport: "desktop", width: 1440, height: 960 },
    { ...definition, captureState: definition.captureState ?? null, captureContext: definition.captureContext ?? null, id: `${definition.fixtureId}-narrow`, surface: "settings-state", viewport: "narrow", width: 390, height: 844 },
  ],
);

const defaultSettingsVisualQaScenarios = settingsVisualQaScenarios.filter(
  (scenario) => scenario.fixtureId === "settings-ai-models-no-project",
).map((scenario) => ({
  ...scenario,
  id: scenario.viewport === "desktop" ? "settings-desktop" : "settings-narrow",
}));

export const visualQaScenarios = [
  { surface: "home", viewport: "desktop", width: 1440, height: 960 },
  { surface: "home", viewport: "narrow", width: 390, height: 844 },
  { surface: "home", viewport: "desktop", width: 1440, height: 960, state: "missing-recent" },
  { surface: "home", viewport: "narrow", width: 390, height: 844, state: "missing-recent" },
  { surface: "home", viewport: "desktop", width: 1440, height: 960, state: "palmier" },
  ...defaultSettingsVisualQaScenarios,
];

const selectableVisualQaScenarios = [
  ...visualQaScenarios.filter((scenario) => scenario.surface !== "settings-state"),
  ...settingsVisualQaScenarios,
];

function parseArgs(argv) {
  const options = {
    url: process.env.VISUAL_QA_BASE_URL || defaultUrl,
    manageViteServer: !process.env.VISUAL_QA_BASE_URL,
    outDir: defaultOutputDir,
    session: `video-creater-visual-qa-${Date.now()}`,
    baselineDir: null,
    diffDir: null,
    comparisonOut: null,
    threshold: 0,
    channelThreshold: 0,
    failOnMismatch: false,
    only: null,
  };

  for (let index = 0; index < argv.length; index += 1) {
    const value = argv[index];
    if (value === "--") {
      continue;
    } else if (value === "--url") {
      options.url = argv[++index];
      options.manageViteServer = false;
    } else if (value === "--out") {
      options.outDir = argv[++index];
    } else if (value === "--session") {
      options.session = argv[++index];
    } else if (value === "--baseline") {
      options.baselineDir = argv[++index];
    } else if (value === "--diff-dir") {
      options.diffDir = argv[++index];
    } else if (value === "--comparison-out") {
      options.comparisonOut = argv[++index];
    } else if (value === "--threshold") {
      options.threshold = parseNumberArg(value, argv[++index]);
    } else if (value === "--channel-threshold") {
      options.channelThreshold = parseNumberArg(value, argv[++index]);
    } else if (value === "--fail-on-mismatch") {
      options.failOnMismatch = true;
    } else if (value === "--only") {
      options.only = argv[++index];
    } else if (value === "--help" || value === "-h") {
      printHelp();
      process.exit(0);
    } else {
      throw new Error(`Unknown argument: ${value}`);
    }
  }

  return options;
}

function parseNumberArg(flag, value) {
  if (!value || value.startsWith("--")) {
    throw new Error(`Missing value for ${flag}`);
  }
  const parsed = Number(value);
  if (!Number.isFinite(parsed)) {
    throw new Error(`${flag} must be a finite number`);
  }
  return parsed;
}

function printHelp() {
  console.log(`Usage: pnpm visual:qa:browser -- [--url ${defaultUrl}] [--out ${defaultOutputDir}]
       [--only surface[:state][:viewport]]
       [--baseline baseline-dir] [--threshold 0.01] [--fail-on-mismatch]

Runs browser visual QA against a repository-local Vite app and writes screenshots plus
basic layout assertions for project home and Settings states.
Project home captures include home-desktop.png, home-narrow.png, and home-palmier-desktop.png.
Project home missing-recent captures seed deterministic local storage and include
home-missing-recent-desktop.png and home-missing-recent-narrow.png.
Default Settings captures include settings-desktop.png and settings-narrow.png, backed by
the current AI & Models fixture.
Deterministic operational Settings states are available with --only settings-state;
the 16 captures use <fixture-id>-desktop.png and <fixture-id>-narrow.png.
When --baseline is provided, screenshots are compared against matching baseline
filenames with scripts/compare-preview-render-frames.mjs and optional mismatch failure.
Pass --url only when intentionally targeting an already-running app.`);
}

async function requireAvailablePort(host, port) {
  const probe = createTcpServer();
  await new Promise((resolveProbe, rejectProbe) => {
    probe.once("error", (error) => rejectProbe(error.code === "EADDRINUSE"
      ? new Error(`Repository Vite port ${port} is already in use`)
      : error));
    probe.listen({ host, port: Number(port), exclusive: true }, () => {
      probe.close((error) => error ? rejectProbe(error) : resolveProbe());
    });
  });
}

export async function startViteServer(url, {
  spawnChild = spawn, startupTimeoutMs = 30_000, shutdownTimeoutMs = 5_000,
} = {}) {
  const parsedUrl = new URL(url);
  // Vite still binds with strictPort: another process can win after this cheap probe closes.
  await requireAvailablePort(parsedUrl.hostname, parsedUrl.port);
  const readyPath = join(tmpdir(), `video-creater-vite-${process.pid}-${Date.now()}.ready`);
  const child = spawnChild(process.execPath, [
    resolve("scripts/vite-visual-qa-server.mjs"), parsedUrl.hostname, parsedUrl.port, readyPath,
  ], { cwd: process.cwd(), stdio: "inherit" });
  let spawnError;
  child.once("error", (error) => { spawnError = error; });
  const exited = new Promise((resolveExit) => {
    child.once("close", (code, signal) => resolveExit({ code, signal }));
  });
  const server = { child, exited, readyPath, shutdownTimeoutMs };
  const deadline = Date.now() + startupTimeoutMs;
  try {
    while (Date.now() < deadline) {
      if (spawnError) throw spawnError;
      if (child.exitCode !== null || child.signalCode !== null) {
        throw new Error(`Repository Vite server exited before ready (${child.exitCode ?? child.signalCode})`);
      }
      if (existsSync(readyPath)) return server;
      await Promise.race([exited, new Promise((resolveDelay) => setTimeout(resolveDelay, 25))]);
    }
    throw new Error(`Timed out starting repository Vite server at ${url}`);
  } catch (error) {
    await stopViteServer(server);
    throw error;
  }
}

export async function stopViteServer(server) {
  if (!server) return;
  if (existsSync(server.readyPath)) unlinkSync(server.readyPath);
  if (server.child.exitCode === null && server.child.signalCode === null) server.child.kill("SIGTERM");
  let timer;
  let closed = false;
  try {
    await Promise.race([
      server.exited.then(() => { closed = true; }),
      new Promise((resolveDelay) => { timer = setTimeout(resolveDelay, server.shutdownTimeoutMs ?? 5_000); }),
    ]);
  } finally {
    clearTimeout(timer);
  }
  if (!closed) {
    server.child.kill("SIGKILL");
    await server.exited;
  }
  // A starting child may have published readiness while termination was in flight.
  if (existsSync(server.readyPath)) unlinkSync(server.readyPath);
}

function resolvePwcli() {
  if (process.env.PWCLI) {
    return { command: process.env.PWCLI, args: [], local: false };
  }
  return {
    command: process.execPath,
    args: [resolve("scripts/playwright-cli.mjs")],
    local: true,
  };
}

function run(command, args, options = {}) {
  const result = spawnSync(command, args, {
    cwd: process.cwd(),
    encoding: "utf8",
    stdio: options.stdio || "pipe",
  });
  if (result.status !== 0) {
    const stdout = result.stdout ? `\nstdout:\n${result.stdout}` : "";
    const stderr = result.stderr ? `\nstderr:\n${result.stderr}` : "";
    throw new Error(`${command} ${args.join(" ")} failed.${stdout}${stderr}`);
  }
  return `${result.stdout || ""}${result.stderr || ""}`;
}

function createPwcli(pwcli, session) {
  return function runPwcli(args, options) {
    return run(
      pwcli.command,
      [...pwcli.args, "--session", session, ...args],
      options,
    );
  };
}

function runPageCode(runPwcli, code) {
  const output = runPwcli(["run-code", `async (page) => {\n${code}\n}`]);
  if (output.includes("### Error")) {
    throw new Error(output);
  }
  return output;
}

function pageCode(strings, ...values) {
  return strings.reduce((source, part, index) => {
    if (index === 0) {
      return part;
    }
    return `${source}${JSON.stringify(values[index - 1])}${part}`;
  }, "");
}

function assertNoHorizontalOverflow(surface) {
  return pageCode`
const result = await page.evaluate((surface) => {
  const root = document.getElementById("root");
  const documentElement = document.documentElement;
  const body = document.body;
  return {
    surface,
    textLength: (root?.innerText || "").trim().length,
    scrollWidth: Math.max(documentElement.scrollWidth, body.scrollWidth),
    clientWidth: documentElement.clientWidth,
  };
}, ${surface});
if (result.textLength < 20) {
  throw new Error(result.surface + " rendered as a blank app root");
}
if (result.scrollWidth > result.clientWidth + 8) {
  throw new Error(
    result.surface + " has horizontal overflow: " + result.scrollWidth + " > " + result.clientWidth
  );
}
`;
}

function assertRect(selector, expected, tolerance = 1) {
  return pageCode`
{
const rectNode = page.locator(${selector}).first();
await rectNode.waitFor({ state: "visible", timeout: 10000 });
const rect = await rectNode.boundingBox();
if (!rect) throw new Error("Missing geometry for " + ${selector});
const expectedRect = ${expected};
for (const [key, value] of Object.entries(expectedRect)) {
  if (Math.abs(rect[key] - value) > ${tolerance}) {
    throw new Error(${selector} + " " + key + " expected " + value + " got " + rect[key]);
  }
}
}
`;
}

function assertHomePaneAlignment() {
  return pageCode`
const homeSidebarRect = await page.getByTestId("project-home-sidebar").boundingBox();
const homeMainRect = await page.getByTestId("project-home-main").boundingBox();
const homeContentRect = await page.getByTestId("project-home-content").boundingBox();
if (!homeSidebarRect || !homeMainRect || !homeContentRect) {
  throw new Error("Missing project home alignment geometry");
}
const sidebarRight = homeSidebarRect.x + homeSidebarRect.width;
if (Math.abs(homeMainRect.x - sidebarRight) > 1) {
  throw new Error("project home main left edge must equal sidebar right edge");
}
const mainInset = homeContentRect.x - homeMainRect.x;
if (Math.abs(mainInset - 24) > 1) {
  throw new Error("project home main content inset expected 24px, got " + mainInset);
}
`;
}

export function palmierGeometryAssertions(scenario) {
  if (scenario.surface === "home" && scenario.state === "palmier") {
    return [
      assertRect("[data-testid='project-home-sidebar']", { width: 220 }, 1),
      assertRect("[data-testid='project-card']:first-of-type", { width: 150, height: 120 }, 1),
      assertHomePaneAlignment(),
    ].join("\n");
  }
  return "";
}

export function waitForSurface(surface) {
  const selectors = {
    home: "[aria-label='Project home']",
    editor: "main[aria-label='Video editor workspace']",
    "settings-state": "[role='tablist'][aria-label='Settings categories'], main[aria-label='Project Settings'], main[aria-label='System Health']",
  };
  return pageCode`
await page.waitForSelector(${selectors[surface]}, { timeout: 10000 });
`;
}

export function prepareSettingsVisualQaFixture(scenario) {
  if (scenario.surface !== "settings-state") return "";
  const installFixture = pageCode`
await page.evaluate((fixtureId) => {
  window.sessionStorage.setItem(${settingsVisualQaSessionKey}, fixtureId);
}, ${scenario.fixtureId});
await page.reload();
await page.getByRole("main", { name: "Project home", exact: true }).waitFor({ state: "visible", timeout: 10000 });
`;
  const openProject = scenario.projectContext === "active" ? `
await page.getByRole("button", { name: "Open sample project", exact: true }).click();
await page.getByRole("main", { name: "Video editor workspace", exact: true }).waitFor({ state: "visible", timeout: 10000 });
` : "";
  const openSurface = scenario.fixtureSurface === "appSettings" ? `
await page.getByRole("button", { name: "Model settings", exact: true }).click();
await page.getByRole("tablist", { name: "Settings categories", exact: true }).waitFor({ state: "visible", timeout: 10000 });
` : `
await page.evaluate(async ({ command }) => {
  window.__EDITOR_FIXTURE_DRIVER__.emit(
    "video-creater://native-menu-command",
    { sequence: 1, command },
  );
}, { command: ${JSON.stringify(scenario.fixtureSurface === "projectSettings" ? "openProjectSettings" : "openSystemHealth")} });
`;
  const disableMotion = `
await page.addStyleTag({ content: "*, *::before, *::after { animation: none !important; transition: none !important; caret-color: transparent !important; }" });
`;
  const selectSurfaceState = scenario.fixtureSurface === "appSettings" ? `
const selectedCategory = page.getByRole("tab", { name: ${JSON.stringify(scenario.categoryLabel)}, exact: true });
await selectedCategory.click();
await page.getByRole("tabpanel").filter({ visible: true }).waitFor({ state: "visible", timeout: 10000 });
` : scenario.fixtureSurface === "projectSettings" ? `
await page.getByRole("main", { name: "Project Settings", exact: true }).waitFor({ state: "visible", timeout: 10000 });
` : `
await page.getByRole("main", { name: "System Health", exact: true }).waitFor({ state: "visible", timeout: 10000 });
`;
  const prepareCaptureState = scenario.captureState === "generationModelSelectorOpen" ? `
await page.getByRole("combobox", { name: "Enabled generation models", exact: true }).click();
await page.getByRole("menu", { name: "Generation model choices", exact: true }).waitFor({ state: "visible", timeout: 10000 });
` : scenario.captureState === "renderArtifactCleanupOpen" ? `
await page.getByRole("button", { name: "Review cleanup", exact: true }).click();
await page.getByRole("dialog", { name: "Confirm render artifact cleanup", exact: true }).waitFor({ state: "visible", timeout: 10000 });
` : "";
  return [installFixture, openProject, openSurface, disableMotion, selectSurfaceState, prepareCaptureState].join("\n");
}

function settingsVisualQaReadiness(scenario) {
  if (scenario.surface !== "settings-state") return "";
  const visibleSurfaceLocator = scenario.fixtureSurface === "appSettings"
  ? 'page.getByRole("tabpanel").filter({ visible: true })'
  : scenario.fixtureSurface === "projectSettings"
    ? 'page.getByRole("main", { name: "Project Settings", exact: true }).filter({ visible: true })'
    : 'page.getByRole("main", { name: "System Health", exact: true }).filter({ visible: true })';
  const markerReadiness = `${`const visibleSettingsPanels = ${visibleSurfaceLocator};`}
${pageCode`
if (await visibleSettingsPanels.count() !== 1) {
  throw new Error(${scenario.id} + " must expose exactly one visible Settings tabpanel");
}
const visibleSettingsPanel = visibleSettingsPanels.first();
const expectedStateMarker = visibleSettingsPanel.getByText(${scenario.expectedMarker}, { exact: false }).first();
await expectedStateMarker.waitFor({ state: "visible", timeout: 10000 });
await expectedStateMarker.evaluate((marker) => {
  let panel = marker.closest('[role="tabpanel"]');
  if (!panel) panel = marker.closest('main[aria-label="Project Settings"], main[aria-label="System Health"]');
  if (!panel) throw new Error("Settings marker is not owned by the visible Settings surface");
  let candidate = marker.parentElement;
  let scrollOwner = panel;
  while (candidate && candidate !== panel) {
    const overflowY = window.getComputedStyle(candidate).overflowY;
    if ((overflowY === "auto" || overflowY === "scroll") && candidate.scrollHeight > candidate.clientHeight + 1) {
      scrollOwner = candidate;
      break;
    }
    candidate = candidate.parentElement;
  }
  const markerRect = marker.getBoundingClientRect();
  const ownerRect = scrollOwner.getBoundingClientRect();
  const viewportTop = Math.max(ownerRect.top, 0);
  const viewportBottom = Math.min(ownerRect.bottom, window.innerHeight);
  const viewportPadding = 24;
  if (markerRect.top < viewportTop + viewportPadding || markerRect.bottom > viewportBottom - viewportPadding) {
    const targetTop = viewportTop + Math.min(180, Math.max(72, (viewportBottom - viewportTop) * 0.28));
    scrollOwner.scrollTop += markerRect.top - targetTop;
  }
});
`}`;
  const captureContextReadiness = scenario.captureContext
    ? pageCode`
const captureContextMarker = visibleSettingsPanel.getByText(${scenario.captureContext.marker}, { exact: false }).first();
const expectedCaptureAction = visibleSettingsPanel.getByRole(${scenario.captureState === "generationModelSelectorOpen" ? "menuitemcheckbox" : "button"}, { name: ${scenario.captureContext.actionName}, exact: true }).first();
await captureContextMarker.waitFor({ state: "visible", timeout: 10000 });
await expectedCaptureAction.waitFor({ state: "visible", timeout: 10000 });
await expectedCaptureAction.evaluate((action, contextMarkerText) => {
  const panel = action.closest('[role="tabpanel"], main[aria-label="Project Settings"], main[aria-label="System Health"]');
  if (!panel) throw new Error("Settings capture action is not owned by the visible Settings surface");
  let operationalBlock = action.parentElement;
  while (operationalBlock && operationalBlock !== panel) {
    if ((operationalBlock.innerText || "").includes(contextMarkerText)) break;
    operationalBlock = operationalBlock.parentElement;
  }
  if (!operationalBlock) operationalBlock = panel;
  let candidate = operationalBlock.parentElement;
  let captureScrollOwner = panel;
  while (candidate && candidate !== panel) {
    const overflowY = window.getComputedStyle(candidate).overflowY;
    if ((overflowY === "auto" || overflowY === "scroll") && candidate.scrollHeight > candidate.clientHeight + 1) {
      captureScrollOwner = candidate;
      break;
    }
    candidate = candidate.parentElement;
  }
  const blockRect = operationalBlock.getBoundingClientRect();
  const ownerRect = captureScrollOwner.getBoundingClientRect();
  const safeTop = Math.max(ownerRect.top, 0) + 24;
  const safeBottom = Math.min(ownerRect.bottom, window.innerHeight) - 24;
  let scrollDelta = 0;
  if (blockRect.bottom > safeBottom) scrollDelta = blockRect.bottom - safeBottom;
  if (blockRect.top - scrollDelta < safeTop) scrollDelta = blockRect.top - safeTop;
  captureScrollOwner.scrollTop += scrollDelta;
}, ${scenario.captureContext.marker});
`
    : "";
  const stableLayoutReadiness = pageCode`
await page.evaluate(() => document.fonts.ready);
const waitForDoubleAnimationFrame = () => page.evaluate(() => new Promise((resolve) => {
  requestAnimationFrame(() => requestAnimationFrame(resolve));
}));
const readSettingsPaintSignature = () => visibleSettingsPanel.evaluate((panel) => {
  const panelRect = panel.getBoundingClientRect();
  const painted = Array.from(panel.querySelectorAll("h1, h2, h3, p, button, code, pre, [role='alert'], [role='status']"))
    .map((element) => {
      const rect = element.getBoundingClientRect();
      if (rect.bottom <= panelRect.top || rect.top >= panelRect.bottom || rect.right <= panelRect.left || rect.left >= panelRect.right) {
        return null;
      }
      return [
        (element.textContent || element.getAttribute("aria-label") || "").trim().slice(0, 160),
        Math.round(rect.x),
        Math.round(rect.y),
        Math.round(rect.width),
        Math.round(rect.height),
      ];
    })
    .filter(Boolean);
  return JSON.stringify({
    text: panel.innerText,
    panel: [Math.round(panelRect.x), Math.round(panelRect.y), Math.round(panelRect.width), Math.round(panelRect.height)],
    scroll: [panel.scrollLeft, panel.scrollTop, panel.scrollWidth, panel.scrollHeight],
    painted,
  });
});
let previousPaintSignature = null;
let stablePaintSamples = 0;
for (let sample = 0; sample < 12 && stablePaintSamples < 2; sample += 1) {
  await waitForDoubleAnimationFrame();
  const paintSignature = await readSettingsPaintSignature();
  if (paintSignature === previousPaintSignature) {
    stablePaintSamples += 1;
  } else {
    previousPaintSignature = paintSignature;
    stablePaintSamples = 0;
  }
}
if (stablePaintSamples < 2) {
  throw new Error(${scenario.id} + " did not reach stable text and bounds before capture");
}
const markerBounds = await expectedStateMarker.boundingBox();
const panelBounds = await visibleSettingsPanel.boundingBox();
const viewport = page.viewportSize();
const viewportPadding = 8;
if (!markerBounds || !panelBounds || !viewport) {
  throw new Error(${scenario.id} + " did not expose marker, panel, and viewport bounds");
}
if (
  markerBounds.y < viewportPadding ||
  markerBounds.y + markerBounds.height > viewport.height - viewportPadding ||
  markerBounds.x < panelBounds.x ||
  markerBounds.x + markerBounds.width > panelBounds.x + panelBounds.width
) {
  throw new Error(${scenario.id} + " marker is outside the captured Settings viewport");
}
`;
  const captureContextBounds = scenario.captureContext
    ? pageCode`
const captureContextBounds = await captureContextMarker.boundingBox();
const captureActionBounds = await expectedCaptureAction.boundingBox();
if (!captureContextBounds || !captureActionBounds) {
  throw new Error(${scenario.id} + " did not expose the required operational context and action bounds");
}
if (
  captureContextBounds.y < viewportPadding ||
  captureContextBounds.y + captureContextBounds.height > viewport.height - viewportPadding ||
  captureContextBounds.y < panelBounds.y + viewportPadding ||
  captureContextBounds.y + captureContextBounds.height > panelBounds.y + panelBounds.height - viewportPadding ||
  captureContextBounds.x < panelBounds.x ||
  captureContextBounds.x + captureContextBounds.width > panelBounds.x + panelBounds.width
) {
  throw new Error(${scenario.id} + " operational context is outside the captured Settings viewport");
}
if (
  captureActionBounds.y < viewportPadding ||
  captureActionBounds.y + captureActionBounds.height > viewport.height - viewportPadding ||
  captureActionBounds.y < panelBounds.y + viewportPadding ||
  captureActionBounds.y + captureActionBounds.height > panelBounds.y + panelBounds.height - viewportPadding ||
  captureActionBounds.x < panelBounds.x ||
  captureActionBounds.x + captureActionBounds.width > panelBounds.x + panelBounds.width
) {
  throw new Error(${scenario.id} + " operational action is outside the captured Settings viewport");
}
`
    : "";
  const layoutReadiness = pageCode`
const settingsLayout = await visibleSettingsPanel.evaluate((panel) => {
  const panelRect = panel.getBoundingClientRect();
  const overflowing = Array.from(panel.querySelectorAll("button, code, pre, [role='alert'], [role='group']"))
    .filter((element) => {
      const rect = element.getBoundingClientRect();
      return rect.right > panelRect.right + 2 || rect.left < panelRect.left - 2;
    })
    .map((element) => (element.textContent || element.getAttribute("aria-label") || element.tagName).trim().slice(0, 120));
  return { overflowing, scrollWidth: panel.scrollWidth, clientWidth: panel.clientWidth };
});
if (settingsLayout.scrollWidth > settingsLayout.clientWidth + 8) {
  throw new Error(${scenario.id} + " tabpanel overflows horizontally: " + settingsLayout.scrollWidth + " > " + settingsLayout.clientWidth);
}
if (settingsLayout.overflowing.length > 0) {
  throw new Error(${scenario.id} + " clips primary content: " + settingsLayout.overflowing.join(" | "));
}
`;
  const narrowRailReadiness =
    scenario.fixtureSurface === "appSettings" && scenario.viewport === "narrow"
      ? pageCode`
const settingsTabRail = page.getByRole("tablist", { name: "Settings categories", exact: true });
const settingsTabRailBounds = await settingsTabRail.boundingBox();
const activeSettingsTabBounds = await selectedCategory.boundingBox();
const settingsTabRailMetrics = await settingsTabRail.evaluate((rail) => ({
  clientWidth: rail.clientWidth,
  scrollWidth: rail.scrollWidth,
  affordance: rail.getAttribute("data-overflow-affordance"),
}));
if (
  !settingsTabRailBounds || !activeSettingsTabBounds ||
  activeSettingsTabBounds.x < settingsTabRailBounds.x ||
  activeSettingsTabBounds.x + activeSettingsTabBounds.width > settingsTabRailBounds.x + settingsTabRailBounds.width ||
  settingsTabRailMetrics.scrollWidth <= settingsTabRailMetrics.clientWidth ||
  settingsTabRailMetrics.affordance !== "horizontal-scroll"
) {
  throw new Error(${scenario.id} + " must keep the active tab visible and expose a horizontal-scroll affordance: " + JSON.stringify({ settingsTabRailBounds, activeSettingsTabBounds, settingsTabRailMetrics }));
}
`
      : "";
  return [
    markerReadiness,
    captureContextReadiness,
    stableLayoutReadiness,
    captureContextBounds,
    layoutReadiness,
    narrowRailReadiness,
  ].join("\n");
}

export function captureSettingsVisualQaScenario(scenario, screenshotPath) {
  if (!scenario || scenario.surface !== "settings-state") return "";
  const capture = pageCode`
await page.screenshot({
  path: ${screenshotPath},
  type: "png",
  animations: "disabled",
  caret: "hide",
});
`;
  const cleanup = pageCode`
await page.evaluate((settingsKey) => {
  window.sessionStorage.removeItem(settingsKey);
  delete window.__EDITOR_FIXTURE_RUNTIME__;
}, ${settingsVisualQaSessionKey}).catch(() => {});
`;
  return [
    "try {",
    prepareSettingsVisualQaFixture(scenario),
    waitForSurface(scenario.surface),
    "await page.evaluate(() => window.scrollTo(0, 0));",
    assertNoHorizontalOverflow(`${scenario.surface}:${scenario.viewport}`),
    settingsVisualQaReadiness(scenario),
    capture,
    "} finally {",
    cleanup,
    "}",
  ].join("\n");
}

export function scenarioScreenshotName(scenario) {
  if (scenario.id) {
    return `${scenario.id}.png`;
  }
  if (scenario.state) {
    return `${scenario.surface}-${scenario.state}-${scenario.viewport}.png`;
  }
  return `${scenario.surface}-${scenario.viewport}.png`;
}

function prepareHomeInteractionState(scenario) {
  if (scenario.surface !== "home" || scenario.state !== "missing-recent") {
    return "";
  }
  return pageCode`
await page.evaluate((storageKey) => {
  window.localStorage.setItem(
    storageKey,
    JSON.stringify([
      {
        id: "recent:missing-launch-cut",
        name: "Missing launch cut",
        projectDir: "/Volumes/archive/Missing launch cut",
        updatedAtLabel: "Opened last week",
        statusLabel: "Needs relink",
        warningLabel: "Folder missing. Choose the moved project folder to relink this recent item.",
      },
    ]),
  );
}, ${recentProjectsStorageKey});
await page.reload();
await page.waitForSelector("[aria-label='Project home']", { timeout: 10000 });
await page.waitForSelector("[aria-label='Recent project Missing launch cut']", { timeout: 10000 });
`;
}

function assertNoUnexpectedBrowserErrors(scenarioLabel) {
  return `
const unexpectedBrowserErrors = page.__videoCreaterUnexpectedBrowserErrors ?? [];
if (unexpectedBrowserErrors.length > 0) {
  throw new Error(
    ${JSON.stringify(scenarioLabel)} + " emitted unexpected browser errors: " +
      unexpectedBrowserErrors.join(" | "),
  );
}
unexpectedBrowserErrors.length = 0;
`;
}

function runScenario(runPwcli, scenario, outDir) {
  const screenshotPath = resolve(
    outDir,
    scenarioScreenshotName(scenario),
  );
  const scenarioLabel = `${scenario.surface}:${scenario.state ? `${scenario.state}:` : ""}${scenario.viewport}`;
  console.error(`browser visual QA: capturing ${scenarioLabel}`);
  mkdirSync(dirname(screenshotPath), { recursive: true });
  if (existsSync(screenshotPath)) {
    unlinkSync(screenshotPath);
  }
  runPwcli(["resize", String(scenario.width), String(scenario.height)]);
  if (scenario.surface === "settings-state") {
    runPageCode(
      runPwcli,
      captureSettingsVisualQaScenario(scenario, screenshotPath),
    );
  } else {
    runPageCode(
      runPwcli,
      [
        waitForSurface(scenario.surface),
        prepareHomeInteractionState(scenario),
        "await page.evaluate(() => window.scrollTo(0, 0));",
        assertNoHorizontalOverflow(`${scenario.surface}:${scenario.viewport}`),
        palmierGeometryAssertions(scenario),
      ].join("\n"),
    );
    runPwcli(["screenshot", "--filename", screenshotPath]);
  }
  assertScreenshotWritten(screenshotPath, scenario);
  console.error(`browser visual QA: wrote ${screenshotPath}`);
  runPageCode(runPwcli, assertNoUnexpectedBrowserErrors(scenarioLabel));
  return screenshotPath;
}

function assertScreenshotWritten(screenshotPath, scenario) {
  if (!existsSync(screenshotPath)) {
    throw new Error(
      `${scenario.surface}:${scenario.viewport} did not write screenshot ${screenshotPath}`,
    );
  }
  const size = statSync(screenshotPath).size;
  if (size <= 0) {
    throw new Error(
      `${scenario.surface}:${scenario.viewport} wrote an empty screenshot ${screenshotPath}`,
    );
  }
}

function runScreenshotBaselineComparison(screenshots, options, outDir) {
  if (!options.baselineDir) {
    return null;
  }

  const baselineDir = resolve(options.baselineDir);
  const diffDir = resolve(options.diffDir || join(outDir, "baseline-diffs"));
  const comparisonOut = resolve(
    options.comparisonOut || join(outDir, "browser-visual-baseline-comparison.json"),
  );
  const compareScript = resolve("scripts/compare-preview-render-frames.mjs");
  const args = [
    compareScript,
    "--threshold",
    String(options.threshold),
    "--channel-threshold",
    String(options.channelThreshold),
    "--out",
    comparisonOut,
    "--diff-dir",
    diffDir,
  ];
  if (options.failOnMismatch) {
    args.push("--fail-on-mismatch");
  }

  screenshots.forEach((screenshotPath, index) => {
    const baselinePath = join(baselineDir, basename(screenshotPath));
    if (!existsSync(baselinePath)) {
      throw new Error(`Missing browser visual QA baseline ${baselinePath}`);
    }
    args.push("--frame", `${index}:${baselinePath}:${screenshotPath}`);
  });

  run(process.execPath, args);
  return {
    baselineDir,
    diffDir,
    comparisonOut,
  };
}

async function main() {
  const options = parseArgs(process.argv.slice(2));
  const pwcli = resolvePwcli();
  const outDir = resolve(options.outDir);

  if (!existsSync(pwcli.command) || (pwcli.local && !existsSync(pwcli.args[0]))) {
    throw new Error(`Playwright CLI wrapper was not found at ${pwcli.args[0] ?? pwcli.command}`);
  }
  const viteServer = options.manageViteServer ? await startViteServer(options.url) : null;

  const runPwcli = createPwcli(pwcli, options.session);
  let localSessionOpen = false;
  const closeLocalSession = () => {
    if (!pwcli.local || !localSessionOpen) return;
    try {
      runPwcli(["close"]);
    } catch {
      // Preserve the original visual-QA failure while the server also observes disconnects.
    }
    localSessionOpen = false;
  };
  const cleanup = async () => {
    closeLocalSession();
    await stopViteServer(viteServer);
  };
  process.once("exit", () => {
    closeLocalSession();
    if (viteServer?.child.exitCode === null) viteServer.child.kill("SIGTERM");
    if (viteServer && existsSync(viteServer.readyPath)) unlinkSync(viteServer.readyPath);
  });
  process.once("SIGINT", () => {
    void cleanup().finally(() => process.exit(130));
  });
  process.once("SIGTERM", () => {
    void cleanup().finally(() => process.exit(143));
  });
  try {
    mkdirSync(outDir, { recursive: true });
    const screenshots = [];
    const selectedScenarios = options.only
      ? selectableVisualQaScenarios.filter((scenario) =>
          scenario.id === options.only ||
          `${scenario.surface}:${scenario.state ?? ""}:${scenario.viewport}` === options.only ||
          `${scenario.surface}:${scenario.state ?? ""}` === options.only ||
          scenario.surface === options.only,
        )
      : visualQaScenarios;

    if (options.only && selectedScenarios.length === 0) {
      throw new Error(`No visual QA scenario matches --only ${options.only}`);
    }
    const bootstrapSettingsFixtureId = selectedScenarios.length > 0 && selectedScenarios.every(
      (scenario) => scenario.surface === "settings-state",
    )
      ? "settings-general-no-project"
      : null;

    runPwcli(["open", "about:blank"]);
    localSessionOpen = pwcli.local;
    runPageCode(
      runPwcli,
      `
 const unexpectedBrowserErrors = [];
 page.on("pageerror", (error) => {
   unexpectedBrowserErrors.push(
     "pageerror: " + (error?.stack || error?.message || String(error)),
   );
 });
 page.on("console", (message) => {
   if (message.type() === "error") {
     unexpectedBrowserErrors.push("console.error: " + message.text());
   }
 });
 page.__videoCreaterUnexpectedBrowserErrors = unexpectedBrowserErrors;
 await page.addInitScript(([preferences, exportCapabilities, settingsKey, bootstrapFixtureId]) => {
   const fixtureId = window.sessionStorage.getItem(settingsKey) ?? bootstrapFixtureId;
   window.__EDITOR_FIXTURE_RUNTIME__ = {
     enabled: true,
     preferences,
     exportCapabilities,
     ...(fixtureId ? { settingsFixtureId: fixtureId } : {}),
   };
 }, ${JSON.stringify([
   browserVisualQaAppPreferences,
   visualExportCapabilities,
   settingsVisualQaSessionKey,
   bootstrapSettingsFixtureId,
 ])});
 await page.goto(${JSON.stringify(options.url)});
 await page.waitForLoadState("domcontentloaded");
 await page.evaluate(
   (storageKeys) => storageKeys.forEach((storageKey) => window.localStorage.removeItem(storageKey)),
   ${JSON.stringify([recentProjectsStorageKey])},
 );
 await page.reload();
 await page.waitForSelector("[aria-label='Project home']", { timeout: 10000 });
 `,
    );

    for (const scenario of selectedScenarios.filter((item) => item.surface === "home")) {
      screenshots.push(runScenario(runPwcli, scenario, outDir));
    }

    for (const scenario of selectedScenarios.filter((item) => item.surface !== "home")) {
      screenshots.push(runScenario(runPwcli, scenario, outDir));
    }

    const baselineComparison = runScreenshotBaselineComparison(
      screenshots,
      options,
      outDir,
    );

    console.log(
      JSON.stringify(
        {
          url: options.url,
          outDir,
          session: options.session,
          screenshots,
          baselineComparison,
        },
        null,
        2,
      ),
    );
    if (pwcli.local) {
      runPwcli(["close"]);
      localSessionOpen = false;
    }
  } finally {
    await cleanup();
  }
}

const isDirectRun =
  process.argv[1] && resolve(process.argv[1]) === fileURLToPath(import.meta.url);

if (isDirectRun) {
  main().catch((error) => {
    console.error(error instanceof Error ? error.message : String(error));
    process.exit(1);
  });
}
