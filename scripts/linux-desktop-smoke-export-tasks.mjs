// Export destination, task progress and render-time responsiveness steps of the Linux desktop smoke
// run (`--export-tasks`): an export to a folder and name the user chose, revealed from the Background
// tasks popover; the pill's progress fraction during a render; and editing, saving and importing while
// that render runs.

import { existsSync, mkdirSync, rmSync, statSync } from "node:fs";
import { spawnSync } from "node:child_process";
import { join } from "node:path";

import {
  backgroundTasks,
  backgroundTasksLabel,
  editorWorkspace,
  exportChoice,
  exportPopover,
  openExportPopover,
  openSampleEditor,
} from "./linux-desktop-smoke-selectors.mjs";
import { newExportArtifacts, resolveArtifactPath } from "./linux-desktop-smoke-retain.mjs";
import { skipped } from "./linux-desktop-smoke-steps.mjs";
import { extractedToolEnvironment, extractedToolPath, xScreenshotArgs } from "./linux-desktop-smoke-tools.mjs";

export const exportTaskStepNames = [
  "an export saves to a chosen folder under a chosen name, and Show in folder reveals it",
  "the Background tasks pill reports a progress fraction while a render runs",
  "the editor edits, saves and imports while a render runs",
];

const projectDir = "/tmp/video-creater-editor-project";
/** Where the chosen-folder export lands; outside the project, as a user's own folder is. */
const chosenFolder = "/tmp/vc-export-target";
const chosenName = "Linux Evidence Export";
/** A webview round trip slower than this while a render runs counts as a frozen UI. */
const responsiveBudgetMs = 1000;

/** The absolute path an export with a chosen folder and name writes. */
export function chosenExportOutputPath(directory, name, extension) {
  return `${directory.replace(/\/+$/, "")}/${name}.${extension}`;
}

/** The percentages the tasks pill reported, in the order they were seen ("Exporting · 42%"). */
export function progressPercents(labels) {
  return labels.flatMap((label) => {
    const match = typeof label === "string" ? label.match(/·\s*(\d+)%/) : null;
    return match ? [Number(match[1])] : [];
  });
}

/**
 * The GTK folder chooser has no accessible name a WebDriver can reach, so the path goes through
 * its location bar: Ctrl+L, the path, Return. The trailing slash makes GTK treat it as a folder.
 */
export function folderChooserKeystrokes(directory) {
  return [
    ["key", "ctrl+l"],
    ["type", `${directory.replace(/\/+$/, "")}/`],
    ["key", "Return"],
  ];
}

/** The keystrokes again, with the confirming Return that accepts the folder GTK navigated into. */
export function folderChooserConfirmKeystrokes(directory) {
  return [...folderChooserKeystrokes(directory), ["key", "Return"]];
}

/** The round trips that blew the budget, so a failure names the measurement. */
export function unresponsiveSamples(samples, budgetMs) {
  return samples.filter((sample) => sample > budgetMs);
}

export async function runExportTaskSteps({ driver, step, options, fixtureMedia, outDir }) {
  const { click, execute, find, invoke, poll, pressKeys, replaceText, screenshot, sleep } = driver;
  const loadProject = () => invoke("load_split_project_from_folder", { projectDir });
  const savedItemIds = async () => (await loadProject()).timeline.tracks.flatMap((track) => track.items.map((item) => item.id));

  /** The visible X windows, so a chooser that never opens is reported instead of timing out blindly. */
  const visibleWindowIds = () => {
    const result = spawnSync(extractedToolPath(options.xdotoolRoot, "xdotool"), ["search", "--onlyvisible", "--name", ""], {
      env: extractedToolEnvironment(options.xdotoolRoot, process.env, { DISPLAY: options.display }),
      encoding: "utf8",
      timeout: 20_000,
    });
    return result.stdout.split("\n").map((line) => line.trim()).filter(Boolean);
  };

  /** An X screenshot of the whole display, which is the only way to see a GTK dialog. */
  const xShot = (name) => {
    if (!options.gstToolsRoot) return null;
    const path = join(outDir, `${name}.png`);
    const result = spawnSync(extractedToolPath(options.gstToolsRoot, "gst-launch-1.0"), ["-q", ...xScreenshotArgs(options.display, path)], {
      env: extractedToolEnvironment(options.gstToolsRoot, process.env, { DISPLAY: options.display }),
      encoding: "utf8",
      timeout: 30_000,
    });
    return result.status === 0 && existsSync(path) ? path : null;
  };

  const xdotool = (...args) => {
    const result = spawnSync(extractedToolPath(options.xdotoolRoot, "xdotool"), args, {
      env: extractedToolEnvironment(options.xdotoolRoot, process.env, { DISPLAY: options.display }),
      encoding: "utf8",
      timeout: 20_000,
    });
    if (result.error || result.status !== 0) {
      throw new Error(`xdotool ${args.join(" ")} failed: ${result.error?.message ?? result.stderr.trim()}`);
    }
    return result.stdout.trim();
  };

  // Starts an export from the popover with the given segment choices and returns the artifacts
  // recorded before it, so the new one can be told apart.
  const startExport = async (choices) => {
    const before = (await loadProject()).exportArtifacts;
    await openExportPopover(driver);
    for (const choice of choices) await click(exportChoice(choice));
    await sleep(500);
    await click(`${exportPopover}//button[normalize-space()='Export video']`);
    return before;
  };

  const waitForArtifact = async (before, timeoutMs) =>
    poll(
      async () => {
        const [found] = newExportArtifacts(before, (await loadProject()).exportArtifacts, "mp4");
        const path = found && resolveArtifactPath(projectDir, found.path);
        return path && existsSync(path) && statSync(path).size > 0 ? found : undefined;
      },
      Boolean,
      timeoutMs,
      1000,
    );

  await step(exportTaskStepNames[0], async () => {
    if (!options.xdotoolRoot) return skipped("run with --xdotool-root <dir>: the GTK folder chooser needs real key events");
    rmSync(chosenFolder, { recursive: true, force: true });
    mkdirSync(chosenFolder, { recursive: true });
    await openSampleEditor(driver);
    await openExportPopover(driver);
    await click(exportChoice("MP4"));
    await click(exportChoice("720p"));
    await replaceText(`${exportPopover}//input[contains(@aria-describedby, '-name-hint')]`, chosenName);
    await poll(
      () => execute(`const input = document.querySelector("[role='dialog'][aria-label='Export'] input[aria-describedby$='-name-hint']"); return input ? input.value : null;`),
      (value) => value === chosenName,
      10_000,
      250,
    );
    const saveLocation = () =>
      execute(`const popover = document.querySelector("[role='dialog'][aria-label='Export']");
        const group = popover && [...popover.querySelectorAll("[role='group']")].find((node) => node.innerText.startsWith('Save to'));
        return group ? group.innerText.split('\\n').map((line) => line.trim()).filter(Boolean)[1] ?? null : null;`);
    const before = await saveLocation();
    const windowsBefore = visibleWindowIds();
    await click(`${exportPopover}//button[@aria-label='Choose export folder']`);
    // The chooser is a separate X window; it only appears when the host can open one, and a
    // WebDriver screenshot cannot see it, so the evidence is an X screenshot of the display.
    const chooser = await poll(
      () => Promise.resolve(visibleWindowIds().filter((id) => !windowsBefore.includes(id))),
      (ids) => ids.length > 0,
      15_000,
      500,
    ).catch(() => []);
    if (chooser.length === 0) {
      return skipped("no folder chooser window opened: this host has no working file-chooser portal");
    }
    await sleep(1500);
    const chooserShot = xShot("export-dest-01-chooser");
    // Keys go through XTEST to the focused window. Xvfb runs no window manager, so focus is set
    // directly with windowfocus (windowactivate needs an EWMH window manager).
    const focused = chooser.filter((id) => {
      try {
        xdotool("windowfocus", "--sync", id);
        return true;
      } catch {
        return false;
      }
    });
    if (focused.length === 0) throw new Error(`no chooser window took keyboard focus: ${JSON.stringify(chooser)}`);
    for (const [action, value] of folderChooserConfirmKeystrokes(chosenFolder)) {
      xdotool(...(action === "key" ? ["key", value] : ["type", "--delay", "40", value]));
      await sleep(800);
    }
    const typedShot = xShot("export-dest-02-chooser-typed");
    const chosen = await poll(saveLocation, (location) => location === chosenFolder, 25_000, 500).catch((error) => {
      throw new Error(
        `the export folder did not change from ${before}: ${error}; windows ${JSON.stringify(chooser)}, focused ${JSON.stringify(focused)}, screenshots ${JSON.stringify([chooserShot, typedShot])}`,
      );
    });
    const artifactsBefore = (await loadProject()).exportArtifacts;
    await click(`${exportPopover}//button[normalize-space()='Export video']`);
    const artifact = await waitForArtifact(artifactsBefore, 300_000);
    const output = chosenExportOutputPath(chosenFolder, chosenName, "mp4");
    if (!existsSync(output)) throw new Error(`the export did not land at ${output}; artifact path ${artifact.path}`);
    // "Show in folder" lives on the task row inside the Background tasks popover.
    await click(backgroundTasks);
    await find("//*[@role='dialog' and @aria-label='Background tasks']");
    await sleep(1000);
    const revealButton = `//*[@role='dialog' and @aria-label='Background tasks']//button[@aria-label='Show']`;
    await find(revealButton, 20_000);
    await click(revealButton);
    await sleep(1500);
    const shot = await screenshot("export-dest-03-tasks");
    // The click cannot be observed on a headless host, so the same command is invoked directly and
    // its outcome recorded: the app either accepted the recorded export path or said why not.
    const reveal = await invoke("reveal_export_artifact_in_split_project_folder", { projectDir, artifactPath: artifact.path })
      .then(() => ({ state: "ok" }))
      .catch((error) => ({ state: "error", error: String(error) }));
    await pressKeys(driver.keys.escape);
    return {
      chosenFolder: chosen,
      chosenName,
      artifact,
      output,
      bytes: statSync(output).size,
      reveal,
      screenshots: [chooserShot, typedShot, shot],
    };
  });

  await step(exportTaskStepNames[1], async () => {
    await openSampleEditor(driver);
    // A 1080p High render lasts long enough for the pill to be sampled while it runs.
    const before = await startExport(["MP4", "1080p", "High"]);
    const labels = [];
    const sample = async () => {
      const label = await backgroundTasksLabel(driver);
      if (label && !labels.includes(label)) labels.push(label);
      return label;
    };
    const bar = () =>
      execute(`const row = document.querySelector("[role='progressbar'][aria-label$='progress']");
        return row ? { label: row.getAttribute('aria-label'), now: Number(row.getAttribute('aria-valuenow')) } : null;`);
    let popoverOpened = false;
    const bars = [];
    const seen = await poll(
      async () => {
        await sample();
        if (!popoverOpened) {
          await click(backgroundTasks).catch(() => {});
          popoverOpened = true;
        }
        const reading = await bar();
        if (reading && !bars.some((entry) => entry.now === reading.now)) bars.push(reading);
        return progressPercents(labels);
      },
      (percents) => percents.length > 0,
      240_000,
      500,
    ).catch(() => progressPercents(labels));
    const shot = await screenshot("export-progress-01-pill");
    await pressKeys(driver.keys.escape).catch(() => {});
    const artifact = await waitForArtifact(before, 300_000);
    if (seen.length === 0) {
      throw new Error(`the pill never reported a progress fraction; labels ${JSON.stringify(labels)}, progressbars ${JSON.stringify(bars)}`);
    }
    return { percents: seen, labels, progressbars: bars, artifact, screenshot: shot };
  });

  await step(exportTaskStepNames[2], async () => {
    await openSampleEditor(driver);
    await sleep(2000);
    const clip = await find(`${editorWorkspace}//*[@role='option' and @data-item-id]`);
    const itemId = await driver.attribute(clip, "data-item-id");
    const clipSelector = `main[aria-label='Video editor workspace'] [role='option'][data-item-id='${itemId}']`;
    const clipGone = () => execute(`return document.querySelector(${JSON.stringify(clipSelector)}) === null;`);
    const clipBack = () => execute(`return document.querySelector(${JSON.stringify(clipSelector)}) !== null;`);

    // Deletes the clip through the timeline and times how long the editor and the saved project
    // take to agree, so the same edit can be compared idle and during a render.
    const deleteClip = async () => {
      const target = await find(`${editorWorkspace}//*[@role='option' and @data-item-id='${itemId}']`);
      await driver.clickElement(target);
      await poll(() => driver.attribute(target, "aria-selected"), (selected) => selected === "true", 15_000, 250);
      const startedAt = Date.now();
      await pressKeys(driver.keys.delete);
      await poll(clipGone, Boolean, 120_000, 250);
      const uiMs = Date.now() - startedAt;
      await poll(savedItemIds, (ids) => !ids.includes(itemId), 120_000, 250);
      return { uiMs, savedMs: Date.now() - startedAt };
    };
    const undoDelete = async () => {
      await pressKeys(driver.keys.control, "z");
      await poll(clipBack, Boolean, 60_000, 250);
      await poll(savedItemIds, (ids) => ids.includes(itemId), 60_000, 250);
    };

    // The control: the same delete with nothing rendering.
    const idle = await deleteClip();
    await undoDelete();

    const before = await startExport(["MP4", "1080p", "High"]);
    // The popover covers the timeline and takes the key events, so it is closed before editing.
    await pressKeys(driver.keys.escape);
    await poll(() => execute(`return document.querySelector("[role='dialog'][aria-label='Export']") === null;`), Boolean, 15_000, 250);
    // The render is running from here on; every measurement below happens against it.
    const roundTrips = [];
    const roundTrip = async () => {
      const startedAt = Date.now();
      await execute("return document.title;");
      roundTrips.push(Date.now() - startedAt);
    };
    await poll(
      async () => {
        await roundTrip();
        return backgroundTasksLabel(driver);
      },
      (label) => Boolean(label && /^(Exporting|Rendering)/.test(label)),
      60_000,
      500,
    );
    const rendering = await deleteClip();
    await roundTrip();
    const imported = await invoke("import_media_to_project", {
      projectDir,
      project: await loadProject(),
      sourcePaths: [fixtureMedia],
    }).catch((error) => ({ error: String(error) }));
    await roundTrip();
    const shot = await screenshot("render-responsive-01-editing");
    const artifact = await waitForArtifact(before, 300_000);
    // Restoring the clip is housekeeping, not the measurement: every later step reopens the sample.
    const restored = await undoDelete()
      .then(() => true)
      .catch(() => false);
    const slow = unresponsiveSamples(roundTrips, responsiveBudgetMs);
    if (slow.length > 0) throw new Error(`the webview stalled while rendering: ${JSON.stringify(roundTrips)} ms`);
    if (imported?.error) throw new Error(`the import failed while rendering: ${imported.error}`);
    return {
      itemId,
      idleDelete: idle,
      deleteWhileRendering: rendering,
      roundTrips,
      imported: imported?.error ? imported : { media: imported?.project?.media?.length ?? null },
      restoredAfterRender: restored,
      artifact,
      screenshot: shot,
    };
  });
  void outDir;
}
