#!/usr/bin/env node
import {
  existsSync,
  mkdirSync,
  readdirSync,
  readFileSync,
  writeFileSync,
} from "node:fs";
import { homedir } from "node:os";
import { dirname, isAbsolute, join, relative, resolve } from "node:path";
import { spawnSync } from "node:child_process";

const defaultOutputDir = "output/playwright/preview-render-qa";
const defaultUrl = "http://127.0.0.1:1420";
const splitProjectRenderReportIndexPath = "renders/index.json";

function parseArgs(argv) {
  const options = {
    url: process.env.VISUAL_QA_BASE_URL || defaultUrl,
    outDir: defaultOutputDir,
    session: `video-creater-preview-render-qa-${Date.now()}`,
    threshold: 0.01,
    channelThreshold: 0,
    durationSeconds: null,
    openSampleProject: true,
    renderedFrames: [],
    renderedVideo: null,
    frameTimes: [],
    renderReportPath: null,
    projectDir: null,
    projectReportId: null,
    failOnMismatch: false,
  };

  for (let index = 0; index < argv.length; index += 1) {
    const value = argv[index];
    if (value === "--") {
      continue;
    } else if (value === "--url") {
      options.url = requireValue(value, argv[++index]);
    } else if (value === "--out") {
      options.outDir = requireValue(value, argv[++index]);
    } else if (value === "--session") {
      options.session = requireValue(value, argv[++index]);
    } else if (value === "--threshold") {
      options.threshold = parseNumber(value, argv[++index]);
    } else if (value === "--channel-threshold") {
      options.channelThreshold = parseNumber(value, argv[++index]);
    } else if (value === "--duration") {
      options.durationSeconds = parseNumber(value, argv[++index]);
    } else if (value === "--rendered-frame") {
      options.renderedFrames.push(parseRenderedFrame(requireValue(value, argv[++index])));
    } else if (value === "--rendered-video") {
      options.renderedVideo = requireValue(value, argv[++index]);
    } else if (value === "--frame-time") {
      options.frameTimes.push(parseFrameTime(requireValue(value, argv[++index])));
    } else if (value === "--render-report") {
      options.renderReportPath = requireValue(value, argv[++index]);
    } else if (value === "--project-dir") {
      options.projectDir = requireValue(value, argv[++index]);
    } else if (value === "--project-report-id") {
      options.projectReportId = requireValue(value, argv[++index]);
    } else if (value === "--fail-on-mismatch") {
      options.failOnMismatch = true;
    } else if (value === "--no-sample-project") {
      options.openSampleProject = false;
    } else if (value === "--help" || value === "-h") {
      printHelp();
      process.exit(0);
    } else {
      throw new Error(`Unknown argument: ${value}`);
    }
  }

  if (!Number.isFinite(options.durationSeconds) || options.durationSeconds <= 0) {
    throw new Error("--duration must be a positive timeline duration in seconds.");
  }
  if (options.renderedVideo && options.frameTimes.length === 0) {
    throw new Error("--rendered-video requires at least one --frame-time seconds value.");
  }
  if (!options.renderedVideo && options.renderedFrames.length === 0) {
    throw new Error("At least one --rendered-frame seconds:path entry or --rendered-video with --frame-time is required.");
  }
  if (options.projectDir && !options.projectReportId) {
    throw new Error("--project-dir requires --project-report-id so the split-project render report can be updated.");
  }
  if (!options.projectDir && options.projectReportId) {
    throw new Error("--project-report-id requires --project-dir.");
  }

  return options;
}

function requireValue(flag, value) {
  if (!value || value.startsWith("--")) {
    throw new Error(`Missing value for ${flag}.`);
  }
  return value;
}

function parseNumber(flag, value) {
  const parsed = Number(requireValue(flag, value));
  if (!Number.isFinite(parsed)) {
    throw new Error(`${flag} must be a finite number.`);
  }
  return parsed;
}

function parseRenderedFrame(spec) {
  const separator = spec.indexOf(":");
  if (separator <= 0) {
    throw new Error(`Invalid --rendered-frame value "${spec}". Expected seconds:path.`);
  }
  const timelineSeconds = Number(spec.slice(0, separator));
  if (!Number.isFinite(timelineSeconds) || timelineSeconds < 0) {
    throw new Error(`Invalid rendered frame timeline seconds in "${spec}".`);
  }
  return {
    timelineSeconds,
    renderedFrame: spec.slice(separator + 1),
  };
}

function parseFrameTime(value) {
  const timelineSeconds = Number(value);
  if (!Number.isFinite(timelineSeconds) || timelineSeconds < 0) {
    throw new Error(`Invalid --frame-time value "${value}".`);
  }
  return timelineSeconds;
}

function printHelp() {
  console.log(`Usage: pnpm visual:qa:preview-render -- --duration 4 --rendered-video renders/final.mp4 --frame-time 1.25 --render-report renders/job/report.json --project-dir /path/to/project --project-report-id render-job
   or: pnpm visual:qa:preview-render -- --duration 4 --rendered-frame 1.25:renders/frame.png

Opens a running Video Creater app, captures Preview viewport PNGs at requested
timeline seconds, then writes previewComparison JSON by comparing those captures
against extracted or supplied rendered frame PNGs. Pass --render-report to attach
the comparison evidence to an existing render report JSON. Pass --project-dir and
--project-report-id to load that exact split project and update its render report.
Pass --fail-on-mismatch to return exit status 1 after evidence is written when
any compared frame exceeds the threshold.

Start the app first with: pnpm dev`);
}

function resolvePwcli() {
  if (process.env.PWCLI) {
    return process.env.PWCLI;
  }
  const codexHome = process.env.CODEX_HOME || join(homedir(), ".codex");
  return join(codexHome, "skills/playwright/scripts/playwright_cli.sh");
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
  return result.stdout || "";
}

function extractRenderedFrames(renderedVideo, frameTimes, renderedFrameDir) {
  const resolvedVideo = resolve(renderedVideo);
  if (!existsSync(resolvedVideo)) {
    throw new Error(`Rendered video was not found at ${resolvedVideo}`);
  }
  mkdirSync(renderedFrameDir, { recursive: true });

  return frameTimes.map((timelineSeconds, index) => {
    const renderedFrame = join(
      renderedFrameDir,
      `rendered-${String(index + 1).padStart(4, "0")}.png`,
    );
    run("ffmpeg", [
      "-y",
      "-hide_banner",
      "-loglevel",
      "error",
      "-ss",
      timelineSeconds.toFixed(3),
      "-i",
      resolvedVideo,
      "-frames:v",
      "1",
      renderedFrame,
    ]);
    return {
      timelineSeconds,
      renderedFrame,
    };
  });
}

function appendUniqueArtifact(artifacts, artifact) {
  if (artifact && !artifacts.includes(artifact)) {
    artifacts.push(artifact);
  }
}

function attachComparisonToRenderReport(renderReportPath, previewComparison, comparisonPath) {
  const resolvedReportPath = resolve(renderReportPath);
  if (!existsSync(resolvedReportPath)) {
    throw new Error(`Render report was not found at ${resolvedReportPath}`);
  }

  const report = JSON.parse(readFileSync(resolvedReportPath, "utf8"));
  const artifacts = Array.isArray(report.artifacts) ? [...report.artifacts] : [];
  appendUniqueArtifact(artifacts, comparisonPath);
  for (const frame of previewComparison.comparedFrames || []) {
    appendUniqueArtifact(artifacts, frame.previewFrame);
    appendUniqueArtifact(artifacts, frame.renderedFrame);
    appendUniqueArtifact(artifacts, frame.diffFrame);
  }

  writeFileSync(
    resolvedReportPath,
    `${JSON.stringify(
      {
        ...report,
        artifacts,
        previewComparison,
      },
      null,
      2,
    )}\n`,
  );
}

function renderReportStatusLabel(status) {
  if (typeof status === "string") {
    return status;
  }
  return "completed";
}

function refreshProjectRenderReportIndex(projectDir) {
  const rendersDir = join(projectDir, "renders");
  const indexPath = join(projectDir, splitProjectRenderReportIndexPath);
  if (!existsSync(rendersDir)) {
    throw new Error(`Split-project renders directory was not found at ${rendersDir}`);
  }

  const reports = [];
  for (const entry of readdirSorted(rendersDir)) {
    const reportPath = join(rendersDir, entry, "report.json");
    if (!existsSync(reportPath)) {
      continue;
    }
    const report = JSON.parse(readFileSync(reportPath, "utf8"));
    const failedCheckCount = Object.values(report.checks || {}).filter(
      (status) => status === "failed",
    ).length;
    reports.push({
      reportId: report.id,
      path: `renders/${entry}/report.json`,
      status: renderReportStatusLabel(report.status),
      outputPath: report.outputPath,
      durationSeconds: report.durationSeconds,
      video: Boolean(report.streams?.video),
      audio: Boolean(report.streams?.audio),
      checkCount: Object.keys(report.checks || {}).length,
      failedCheckCount,
      artifactCount: Array.isArray(report.artifacts) ? report.artifacts.length : 0,
      logPath: report.logPath,
      createdAt: report.createdAt,
    });
  }
  reports.sort((left, right) => left.reportId.localeCompare(right.reportId));

  writeFileSync(
    indexPath,
    `${JSON.stringify({ schemaVersion: 1, reports }, null, 2)}\n`,
  );
}

function readdirSorted(path) {
  return readdirSync(path).sort();
}

function attachComparisonToProjectReport(projectDir, projectReportId, previewComparison, comparisonPath) {
  const resolvedProjectDir = resolve(projectDir);
  const reportPath = join(
    resolvedProjectDir,
    "renders",
    projectReportId,
    "report.json",
  );
  attachComparisonToRenderReport(
    reportPath,
    relativizePreviewComparison(resolvedProjectDir, previewComparison),
    projectRelativePath(resolvedProjectDir, comparisonPath),
  );
  refreshProjectRenderReportIndex(resolvedProjectDir);
  return reportPath;
}

function relativizePreviewComparison(projectDir, previewComparison) {
  return {
    ...previewComparison,
    comparedFrames: (previewComparison.comparedFrames || []).map((frame) => ({
      ...frame,
      previewFrame: projectRelativePath(projectDir, frame.previewFrame),
      renderedFrame: projectRelativePath(projectDir, frame.renderedFrame),
      diffFrame: frame.diffFrame ? projectRelativePath(projectDir, frame.diffFrame) : frame.diffFrame,
    })),
  };
}

function projectRelativePath(projectDir, path) {
  if (typeof path !== "string" || path.trim().length === 0) {
    return path;
  }
  const resolvedPath = resolve(path);
  const relativePath = relative(projectDir, resolvedPath).replaceAll("\\", "/");
  if (
    relativePath.length > 0 &&
    !relativePath.startsWith("../") &&
    relativePath !== ".." &&
    !isAbsolute(relativePath)
  ) {
    return relativePath;
  }
  return path;
}

function createPwcli(pwcli, session) {
  return function runPwcli(args) {
    return run(pwcli, ["--session", session, ...args]);
  };
}

function pageCode(strings, ...values) {
  return strings.reduce((source, part, index) => {
    if (index === 0) {
      return part;
    }
    return `${source}${JSON.stringify(values[index - 1])}${part}`;
  }, "");
}

function runPageCode(runPwcli, code) {
  runPwcli(["run-code", `async (page) => {\n${code}\n}`]);
}

function seekAndCaptureCode({ timelineSeconds, durationSeconds, previewPath }) {
  return pageCode`
await page.waitForSelector("[aria-label='Timeline canvas']", { timeout: 10000 });
await page.waitForSelector("[aria-label='Preview viewport']", { timeout: 10000 });
// The scrubber spans the whole timeline duration, so a click at the time ratio seeks without
// selecting a clip (a timeline click would draw canvas selection handles into the capture).
const scrubberThumb = page.getByRole("slider", { name: "Preview scrubber" });
const scrubber = scrubberThumb.locator("xpath=../..");
const scrubberBox = await scrubber.boundingBox();
if (!scrubberBox) {
  throw new Error("Preview scrubber is not visible.");
}
await scrubberThumb.focus();
await page.keyboard.press("Home");
const ratio = Math.max(0, Math.min(1, ${timelineSeconds} / ${durationSeconds}));
await page.mouse.click(scrubberBox.x + scrubberBox.width * ratio, scrubberBox.y + scrubberBox.height * 0.5);
await page.waitForTimeout(250);
const preview = page.locator("[aria-label='Preview viewport']");
await preview.screenshot({ path: ${previewPath} });
`;
}

function openEditorCode(options) {
  if (options.projectDir) {
    return pageCode`
await page.waitForSelector("[aria-label='Project home']", { timeout: 10000 });
const projectFolder = page.locator("#project-folder-path");
await projectFolder.fill(${resolve(options.projectDir)});
await page.getByRole("button", { name: "Open project folder" }).click();
await page.waitForSelector("[aria-label='Video editor workspace']", { timeout: 10000 });
`;
  }

  const openSampleProject = options.openSampleProject;
  if (!openSampleProject) {
    return `
await page.waitForSelector("[aria-label='Video editor workspace']", { timeout: 10000 });
`;
  }

  return `
await page.waitForSelector("[aria-label='Project home']", { timeout: 10000 });
await page.getByRole("button", { name: "Open sample project" }).click();
await page.waitForSelector("[aria-label='Video editor workspace']", { timeout: 10000 });
`;
}

function main() {
  const options = parseArgs(process.argv.slice(2));
  const pwcli = resolvePwcli();
  if (!existsSync(pwcli)) {
    throw new Error(`Playwright CLI wrapper was not found at ${pwcli}`);
  }

  const outDir = resolve(options.outDir);
  const previewDir = join(outDir, "preview-frames");
  const renderedFrameDir = join(outDir, "rendered-frames");
  const diffDir = join(outDir, "diffs");
  const comparisonPath = join(outDir, "preview-comparison.json");
  mkdirSync(previewDir, { recursive: true });
  mkdirSync(diffDir, { recursive: true });
  mkdirSync(dirname(comparisonPath), { recursive: true });

  const runPwcli = createPwcli(pwcli, options.session);
  runPwcli(["open", options.url]);
  runPageCode(
    runPwcli,
    `
await page.waitForLoadState("domcontentloaded");
${openEditorCode(options)}
`,
  );

  const renderedFrames = [
    ...options.renderedFrames,
    ...(options.renderedVideo
      ? extractRenderedFrames(options.renderedVideo, options.frameTimes, renderedFrameDir)
      : []),
  ];

  const frames = renderedFrames.map((frame, index) => {
    const previewFrame = join(previewDir, `preview-${String(index + 1).padStart(4, "0")}.png`);
    runPageCode(
      runPwcli,
      seekAndCaptureCode({
        timelineSeconds: frame.timelineSeconds,
        durationSeconds: options.durationSeconds,
        previewPath: previewFrame,
      }),
    );
    if (!existsSync(previewFrame)) {
      throw new Error(
        `Preview screenshot was not written at ${previewFrame}. ` +
          "Ensure the editor opened the requested project and the browser session has access to the native project bridge.",
      );
    }
    return {
      timelineSeconds: frame.timelineSeconds,
      previewFrame,
      renderedFrame: frame.renderedFrame,
    };
  });

  const compareArgs = [
    "scripts/compare-preview-render-frames.mjs",
    "--threshold",
    String(options.threshold),
    "--channel-threshold",
    String(options.channelThreshold),
    "--out",
    comparisonPath,
    "--diff-dir",
    diffDir,
  ];
  for (const frame of frames) {
    compareArgs.push(
      "--frame",
      `${frame.timelineSeconds}:${frame.previewFrame}:${frame.renderedFrame}`,
    );
  }

  const previewComparison = JSON.parse(run("node", compareArgs));
  if (options.renderReportPath) {
    attachComparisonToRenderReport(
      options.renderReportPath,
      previewComparison,
      comparisonPath,
    );
  }
  const projectReportPath =
    options.projectDir && options.projectReportId
      ? attachComparisonToProjectReport(
          options.projectDir,
          options.projectReportId,
          previewComparison,
          comparisonPath,
        )
      : null;

  console.log(
    JSON.stringify(
      {
        previewComparison,
        outDir,
        previewFrames: frames.map((frame) => frame.previewFrame),
        renderedFrames: frames.map((frame) => frame.renderedFrame),
        comparisonPath,
        renderReportPath: options.renderReportPath,
        projectReportPath,
      },
      null,
      2,
    ),
  );
  if (options.failOnMismatch && previewComparison.status !== "passed") {
    process.exitCode = 1;
  }
}

try {
  main();
} catch (error) {
  console.error(error instanceof Error ? error.message : String(error));
  process.exit(1);
}
