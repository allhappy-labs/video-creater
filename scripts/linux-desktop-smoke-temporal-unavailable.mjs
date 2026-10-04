// Temporal-unavailable steps of the Linux desktop smoke run (`--temporal-unavailable`), for builds
// without the `temporal-worker` feature such as the release package: with Temporal execution selected,
// an export must be refused with its reason and must never render in the desktop process instead.

import { existsSync, readdirSync } from "node:fs";
import { join } from "node:path";

import { backgroundTasksLabel, exportChoice, exportPopover, openExportPopover, openSampleEditor, openSettings, settingsTab } from "./linux-desktop-smoke-selectors.mjs";
import { newExportArtifacts } from "./linux-desktop-smoke-retain.mjs";

export const temporalUnavailableStepNames = [
  "Temporal unavailable: the build reports no Temporal worker feature",
  "Temporal unavailable: an export is refused and never renders in the desktop process",
];

const unavailableMessage = "Temporal runtime is unavailable in this build";

/** `settleMs` is how long a silent desktop-process render is given to show itself after the refusal. */
export async function runTemporalUnavailableSteps({ driver, step, projectDir = "/tmp/video-creater-editor-project", settleMs = 20_000 }) {
  const { click, execute, invoke, poll, screenshot, sleep } = driver;

  await step(temporalUnavailableStepNames[0], async () => {
    const report = await invoke("get_temporal_worker_environment_report");
    if (report.featureEnabled !== false || report.ready !== false) {
      throw new Error(`this build can run Temporal workflows (use --temporal instead): ${JSON.stringify(report)}`);
    }
    return { featureEnabled: report.featureEnabled, ready: report.ready, featureName: report.featureName };
  });

  await step(temporalUnavailableStepNames[1], async () => {
    try {
      await openSettings(driver);
      await click(settingsTab("Advanced"));
      await click("//select[@aria-label='Generation execution backend']/option[@value='temporal']");
      await poll(async () => (await invoke("get_app_preferences"))?.generationExecutionBackend, (backend) => backend === "temporal", 15_000, 500);
      await openSampleEditor(driver);
      await sleep(3000);
      const loadProject = () => invoke("load_split_project_from_folder", { projectDir });
      const before = await loadProject();
      const knownJobs = new Set(before.jobs.map((job) => job.id));
      const newJobs = async () => (await loadProject()).jobs.filter((job) => !knownJobs.has(job.id));
      const exportFiles = () => {
        const dir = join(projectDir, "exports");
        return existsSync(dir) ? readdirSync(dir) : [];
      };
      const filesBefore = exportFiles();
      await openExportPopover(driver);
      await click(exportChoice("MP4"));
      await click(exportChoice("720p"));
      await sleep(500);
      await click(`${exportPopover}//button[normalize-space()='Export video']`);
      await poll(newJobs, (jobs) => jobs.some((job) => job.status === "failed"), 60_000, 1000);
      const reasonShown = await poll(() => execute(`return document.body.innerText.includes(${JSON.stringify(unavailableMessage)});`), Boolean, 15_000, 500).then(
        () => true,
        () => false,
      );
      const shot = await screenshot("temporal-unavailable-export-refused");
      await sleep(settleMs);
      const after = await loadProject();
      const jobs = after.jobs.filter((job) => !knownJobs.has(job.id));
      // The backend's own answer for the job the editor recorded.
      const startResult = jobs[0] ? await invoke("start_temporal_workflow", { job: jobs[0] }) : null;
      const detail = {
        startResult,
        jobs: jobs.map(({ id, kind, status, workflow }) => ({ id, kind, status, runId: workflow?.runId ?? null })),
        newArtifacts: newExportArtifacts(before.exportArtifacts, after.exportArtifacts, "mp4"),
        newFiles: exportFiles().filter((name) => !filesBefore.includes(name)),
        renderDirs: jobs.filter((job) => existsSync(join(projectDir, "renders", job.id))).map((job) => `renders/${job.id}`),
        reasonShown,
        tasksLabel: await backgroundTasksLabel(driver),
        screenshot: shot,
      };
      const problems = [];
      if (startResult?.status !== "unavailable" || startResult?.runId) problems.push("the workflow start was not answered as unavailable");
      if (jobs.length !== 1 || jobs[0].status !== "failed") problems.push("the export did not leave exactly one failed job");
      if (detail.newArtifacts.length > 0 || detail.newFiles.length > 0 || detail.renderDirs.length > 0) problems.push("the export produced output without a Temporal worker");
      if (!reasonShown) problems.push("the unavailable reason is not shown in the editor");
      if (problems.length > 0) throw new Error(`${problems.join("; ")}: ${JSON.stringify(detail)}`);
      return detail;
    } finally {
      // Later steps and later runs expect the default desktop execution backend.
      await invoke("update_app_preferences", { patch: { generationExecutionBackend: "inProcess" } }).catch(() => undefined);
    }
  });
}
