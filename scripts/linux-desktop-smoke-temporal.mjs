// Temporal steps of the Linux desktop smoke run (`--temporal`): model install, Temporal execution,
// an export and a transcription through the Temporal worker, and speech analysis.

import { existsSync, statSync } from "node:fs";
import { join } from "node:path";

import {
  backgroundTasksLabel,
  editorTab,
  editorWorkspace,
  exportChoice,
  exportPopover,
  openExportPopover,
  openSampleEditor,
  openSettings,
  projectHome,
  settingsTab,
  waitForEditor,
} from "./linux-desktop-smoke-selectors.mjs";
import { newExportArtifacts, renderPipelineReportPath, resolveArtifactPath, retainProjectFiles } from "./linux-desktop-smoke-retain.mjs";

export const temporalStepNames = [
  "install the transcription model from settings",
  "select Temporal execution in advanced settings",
  "export MP4 through the Temporal worker",
  "transcribe imported speech through the Temporal worker",
  "analyze speech in the desktop process",
];

export async function runTemporalSteps({ driver, step, projectPath, outDir }) {
  const { click, execute, find, invoke, poll, screenshot, sleep, type } = driver;

  await step(temporalStepNames[0], async () => {
    await openSettings(driver);
    await click(settingsTab("AI & Models"));
    const installed = (await invoke("list_transcription_models")).find((entry) => entry.isActive);
    if (installed?.installStatus !== "ready") await click("//button[normalize-space()='Download model']");
    const model = await poll(
      async () => (await invoke("list_transcription_models")).find((entry) => entry.isActive),
      (entry) => {
        if (entry?.installStatus === "failed") throw new Error(`model install failed: ${entry.lastErrorDetail}`);
        return entry?.installStatus === "ready";
      },
      1_800_000,
      5000,
    );
    if (!(await invoke("get_production_speech_model_status")).ready) {
      await click("//button[normalize-space()='Install']");
    }
    const speech = await poll(() => invoke("get_production_speech_model_status"), (status) => status.ready === true, 900_000, 5000);
    return {
      model: { modelId: model.modelId, runtimeId: model.runtimeId, installedBytes: model.installedBytes },
      runtime: await invoke("get_transcription_runtime_status"),
      speech,
      screenshot: await screenshot("08-models-installed"),
    };
  });

  await step(temporalStepNames[1], async () => {
    await openSettings(driver);
    await click(settingsTab("Advanced"));
    await click("//select[@aria-label='Generation execution backend']/option[@value='temporal']");
    // The later steps rely on Temporal execution; fail here unless the preference was actually saved.
    let saved;
    await poll(
      async () => (saved = (await invoke("get_app_preferences"))?.generationExecutionBackend),
      (backend) => backend === "temporal",
      15_000,
      500,
    ).catch((error) => {
      throw new Error(`the generation execution backend is still ${JSON.stringify(saved)}, not "temporal": ${error.message}`);
    });
    return { generationExecutionBackend: saved, screenshot: await screenshot("09-advanced-temporal") };
  });

  await step(temporalStepNames[2], async () => {
    await openSampleEditor(driver);
    const projectDir = "/tmp/video-creater-editor-project";
    const loadProject = () => invoke("load_split_project_from_folder", { projectDir });
    const before = (await loadProject()).exportArtifacts;
    await openExportPopover(driver);
    await click(exportChoice("MP4"));
    await click(exportChoice("720p"));
    await poll(
      () => execute(`return document.querySelector("[role='dialog'][aria-label='Export']").innerText.includes('H.264');`),
      Boolean,
      10_000,
      500,
    );
    await click(`${exportPopover}//button[normalize-space()='Export video']`);
    // The worker saves the export and records its artifact; the artifact names the job.
    const artifact = await poll(
      async () => {
        const [found] = newExportArtifacts(before, (await loadProject()).exportArtifacts, "mp4");
        const path = found && resolveArtifactPath(projectDir, found.path);
        return path && existsSync(path) && statSync(path).size > 0 ? found : undefined;
      },
      Boolean,
      600_000,
      3000,
    );
    const output = resolveArtifactPath(projectDir, artifact.path);
    const jobId = artifact.jobId;
    const project = await poll(
      loadProject,
      (latest) => latest.jobs.some((job) => job.id === jobId && job.status === "completed"),
      300_000,
      3000,
    );
    const job = project.jobs.find((entry) => entry.id === jobId);
    if (!job.workflow?.runId) throw new Error(`export job ${jobId} has no Temporal run id`);
    const tasksLabel = await poll(() => backgroundTasksLabel(driver), (label) => Boolean(label && / complete$/.test(label)), 60_000);
    return {
      artifact,
      output,
      bytes: statSync(output).size,
      tasksLabel,
      job: { id: job.id, status: job.status, runId: job.workflow?.runId },
      retained: retainProjectFiles(
        projectDir,
        [renderPipelineReportPath(projectDir, jobId), `renders/${jobId}/report.json`],
        join(outDir, "temporal-export-mp4"),
      ),
    };
  });

  await step(temporalStepNames[3], async () => {
    await execute("window.location.reload();");
    await find(projectHome, 60_000);
    await click("//button[@aria-label='Open Project']");
    await type("//input[@id='project-folder-path']", projectPath);
    await click("//form//button[@type='submit' and @aria-label='Open project folder']");
    await waitForEditor(driver);
    const project = await invoke("load_split_project_from_folder", { projectDir: projectPath });
    const media = project.media.find((asset) => asset.name.includes("edison"));
    if (!media) throw new Error(`imported media missing: ${JSON.stringify(project.media.map((asset) => asset.name))}`);
    // Media tab tiles are named "Preview <media name>" and preview the asset in the viewer.
    await click(editorTab("Media"));
    await click(`${editorWorkspace}//button[starts-with(@aria-label, 'Preview ') and contains(@aria-label, 'edison')]`);
    const preview = await poll(
      () =>
        execute(`return {
          failed: document.body.innerText.includes('Preview failed'),
          videos: [...document.querySelectorAll('video')].map((video) => ({ src: video.currentSrc, readyState: video.readyState, error: video.error && video.error.code })),
        };`),
      (state) => {
        if (state.failed) throw new Error(`user project preview failed: ${JSON.stringify(state.videos)}`);
        return state.videos.some((video) => video.readyState >= 2 && video.src.includes(encodeURIComponent("edison")));
      },
      60_000,
    );
    // Captions tab → "Generate captions" transcribes the source, then places timed captions.
    await click(editorTab("Captions"));
    await click(`${editorWorkspace}//button[normalize-space()='Generate captions' and not(@disabled)]`);
    const transcribed = await poll(
      () => invoke("load_split_project_from_folder", { projectDir: projectPath }),
      (latest) => latest.transcripts.some((transcript) => transcript.mediaId === media.id && transcript.words.length > 0),
      900_000,
      3000,
    );
    const transcript = transcribed.transcripts.find((entry) => entry.mediaId === media.id);
    const tasksLabel = await backgroundTasksLabel(driver);
    await sleep(2000);
    return {
      preview,
      engine: transcript.engine,
      words: transcript.words.length,
      text: transcript.words.slice(0, 25).map((word) => word.text).join(" "),
      firstWord: transcript.words[0],
      lastWord: transcript.words.at(-1),
      tasksLabel,
      screenshot: await screenshot("10-transcript"),
    };
  });

  await step(temporalStepNames[4], async () => {
    const project = await invoke("load_split_project_from_folder", { projectDir: projectPath });
    const media = project.media.find((asset) => asset.name.includes("edison"));
    const sidecar = await invoke("analyze_project_speech", {
      projectDir: projectPath,
      mediaId: media.id,
      preparedPcmPath: media.relativePath,
    });
    return {
      speechRanges: sidecar.speechRanges?.length,
      deadAirRanges: sidecar.deadAirRanges,
      firstSpeech: sidecar.speechRanges?.[0],
    };
  });
}
