import assert from "node:assert/strict";
import test from "node:test";

import {
  chosenExportOutputPath,
  folderChooserConfirmKeystrokes,
  folderChooserKeystrokes,
  progressPercents,
  unresponsiveSamples,
} from "./linux-desktop-smoke-export-tasks.mjs";

test("chosenExportOutputPath joins the chosen folder, name and extension", () => {
  assert.equal(chosenExportOutputPath("/tmp/vc-export-target", "Linux Evidence", "mp4"), "/tmp/vc-export-target/Linux Evidence.mp4");
  assert.equal(chosenExportOutputPath("/tmp/vc-export-target/", "Linux Evidence", "mp4"), "/tmp/vc-export-target/Linux Evidence.mp4");
});

test("progressPercents reads the fractions out of the pill's labels", () => {
  assert.deepEqual(progressPercents(["Exporting…", "Exporting · 0%", "Exporting · 42%", "Export complete"]), [0, 42]);
  assert.deepEqual(progressPercents(["Exporting…", "Export complete"]), []);
  assert.deepEqual(progressPercents([null, undefined, "Rendering · 7%"]), [7]);
});

test("folderChooserKeystrokes types the path into the GTK location bar and confirms it", () => {
  assert.deepEqual(folderChooserKeystrokes("/tmp/vc-export-target"), [
    ["key", "ctrl+l"],
    ["type", "/tmp/vc-export-target/"],
    ["key", "Return"],
  ]);
});

test("unresponsiveSamples names the round trips slower than the budget", () => {
  assert.deepEqual(unresponsiveSamples([12, 40, 2500], 1000), [2500]);
  assert.deepEqual(unresponsiveSamples([12, 40], 1000), []);
});

test("folderChooserConfirmKeystrokes accepts the folder GTK navigated into", () => {
  assert.deepEqual(folderChooserConfirmKeystrokes("/tmp/vc-export-target"), [
    ["key", "ctrl+l"],
    ["type", "/tmp/vc-export-target/"],
    ["key", "Return"],
    ["key", "Return"],
  ]);
});
