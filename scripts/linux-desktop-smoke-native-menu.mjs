// Native menu steps of the Linux desktop smoke run (`--native-menu --xdotool-root <dir>`): real X key
// events from xdotool reach GTK, so Ctrl+Z and the GTK menu bar's Edit → Undo are checked below the
// WebDriver layer. X screenshots are taken with gst-launch-1.0 when `--gst-tools-root` is given.

import { spawnSync } from "node:child_process";
import { existsSync, readFileSync } from "node:fs";
import { join } from "node:path";

import { editorWorkspace, openSampleEditor } from "./linux-desktop-smoke-selectors.mjs";
import { extractedToolEnvironment, extractedToolPath, xScreenshotArgs } from "./linux-desktop-smoke-tools.mjs";

export const nativeMenuStepNames = [
  "native menu: a real Ctrl+Z key press undoes exactly one timeline edit",
  "native menu: Edit → Undo in the GTK menu bar undoes the next edit",
];

const projectDir = "/tmp/video-creater-editor-project";

export async function runNativeMenuSteps({ driver, step, options, outDir }) {
  const { execute, find, invoke, poll, pressKeys, screenshot, sleep } = driver;
  const display = options.display;

  const xdotool = (...args) => {
    const result = spawnSync(extractedToolPath(options.xdotoolRoot, "xdotool"), args, {
      env: extractedToolEnvironment(options.xdotoolRoot, process.env, { DISPLAY: display }),
      encoding: "utf8",
      timeout: 20_000,
    });
    if (result.error || result.status !== 0) {
      throw new Error(`xdotool ${args.join(" ")} failed: ${result.error?.message ?? result.stderr.trim()}`);
    }
    return result.stdout.trim();
  };

  const xShot = (name) => {
    if (!options.gstToolsRoot) return null;
    const path = join(outDir, `${name}.png`);
    const result = spawnSync(extractedToolPath(options.gstToolsRoot, "gst-launch-1.0"), ["-q", ...xScreenshotArgs(display, path)], {
      env: extractedToolEnvironment(options.gstToolsRoot, process.env, { DISPLAY: display }),
      encoding: "utf8",
      timeout: 30_000,
    });
    if (result.error || result.status !== 0 || !existsSync(path)) {
      throw new Error(`X screenshot ${name} failed: ${result.error?.message ?? result.stderr.trim()}`);
    }
    return path;
  };
  const xShotNote = options.gstToolsRoot ? undefined : "not captured (no --gst-tools-root)";

  const windowId = () => xdotool("search", "--sync", "--onlyvisible", "--name", "^Video Creater$").split("\n")[0];
  const savedItemIds = async () =>
    (await invoke("load_split_project_from_folder", { projectDir })).timeline.tracks.flatMap((track) => track.items.map((item) => item.id));
  const clipXPath = (itemId) => `${editorWorkspace}//*[@role='option' and @data-item-id='${itemId}']`;

  // Shared between the two steps: the first step leaves `first` deleted for the menu Undo to restore.
  const flow = { first: null, second: null, windowId: null };

  await step(nativeMenuStepNames[0], async () => {
    await openSampleEditor(driver);
    await sleep(2000);
    const ids = await execute(
      `return [...document.querySelectorAll("main[aria-label='Video editor workspace'] [role='option'][data-item-id]")].map((clip) => clip.getAttribute('data-item-id'));`,
    );
    const saved = await savedItemIds();
    const [first, second] = [...new Set(ids)].filter((id) => saved.includes(id));
    if (!first || !second) throw new Error(`the sample needs two saved timeline clips, found ${JSON.stringify(ids)}`);
    for (const itemId of [first, second]) {
      const clip = await find(clipXPath(itemId));
      await driver.clickElement(clip);
      await poll(() => driver.attribute(clip, "aria-selected"), (selected) => selected === "true", 10_000, 250);
      await pressKeys(driver.keys.delete);
      await poll(savedItemIds, (current) => !current.includes(itemId), 30_000, 500);
    }
    const webviewShots = [await screenshot("native-undo-01-two-clips-deleted")];
    const xShots = [xShot("ctrlz-01-before")];
    flow.windowId = windowId();
    xdotool("windowfocus", "--sync", flow.windowId);
    xdotool("key", "--clearmodifiers", "ctrl+z");
    await poll(savedItemIds, (current) => current.includes(second), 30_000, 500);
    const restoredAt = Date.now();
    await sleep(3000);
    const after = await savedItemIds();
    if (after.includes(first)) throw new Error(`one Ctrl+Z restored both clips ${first} and ${second}`);
    if (!after.includes(second)) throw new Error(`clip ${second} disappeared again after Ctrl+Z`);
    webviewShots.push(await screenshot("native-undo-02-ctrl-z"));
    xShots.push(xShot("ctrlz-02-after"));
    flow.first = first;
    flow.second = second;
    return {
      windowId: flow.windowId,
      restored: second,
      stillDeleted: first,
      stillDeletedCheckedMs: Date.now() - restoredAt,
      screenshots: webviewShots,
      xScreenshots: xShotNote ?? xShots,
    };
  });

  await step(nativeMenuStepNames[1], async () => {
    if (!flow.first) throw new Error("the Ctrl+Z step did not leave a deleted clip to restore");
    const restored = async (timeoutMs) => {
      try {
        await poll(savedItemIds, (current) => current.includes(flow.first), timeoutMs, 500);
        return true;
      } catch {
        return false;
      }
    };
    const identical = (left, right) => Boolean(left && right && readFileSync(left).equals(readFileSync(right)));
    const xShots = [];
    xdotool("windowfocus", "--sync", flow.windowId);
    // F10 opens the GTK menu bar's first menu (File); Right moves to Edit, whose first editor item is Undo.
    // A menu opened this way highlights no item (observed in output/gap-closure-06/native-menu), so Down
    // highlights Undo.
    const beforeMenu = xShot("menu-00-before");
    xdotool("key", "F10");
    await sleep(1000);
    const f10 = xShot("menu-01-f10");
    xShots.push(beforeMenu, f10);
    const f10Opened = beforeMenu && f10 ? !identical(beforeMenu, f10) : null;
    let method = "F10";
    if (f10Opened !== false) {
      xdotool("key", "Right");
      await sleep(700);
      xdotool("key", "Down");
      await sleep(700);
      xShots.push(xShot("menu-02-edit"));
      xdotool("key", "Return");
    }
    let changed = f10Opened !== false && (await restored(10_000));
    if (!changed) {
      // F10 did not open the menu (or the project did not change): close any menu and click the bar.
      xdotool("key", "Escape");
      xdotool("key", "Escape");
      await sleep(500);
      method = "menu bar click";
      // Without a window manager the window sits at the screen origin: Edit at (60, 12), Undo at (87, 37).
      xdotool("mousemove", "--window", flow.windowId, "60", "12", "click", "1");
      await sleep(1000);
      xShots.push(xShot("menu-03-edit-click"));
      xdotool("mousemove", "--window", flow.windowId, "87", "37", "click", "1");
      changed = await restored(10_000);
    }
    if (!changed) throw new Error(`Edit → Undo did not restore clip ${flow.first} (tried ${method})`);
    await find(clipXPath(flow.first), 15_000);
    return {
      method,
      f10OpenedMenu: f10Opened,
      keys: method === "F10" ? ["F10", "Right", "Down", "Return"] : ["click Edit at window (60, 12)", "click Undo at window (87, 37)"],
      restored: flow.first,
      screenshot: await screenshot("native-undo-03-menu-undo"),
      xScreenshots: xShotNote ?? xShots,
    };
  });
}
