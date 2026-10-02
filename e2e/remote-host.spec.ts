import { expect, test } from "@playwright/test";
import { stat } from "node:fs/promises";
import {
  exportAndDownload,
  pairAndCreate,
  reconcileUnconfirmedEdit,
  uploadVideoAndApplyAgent,
  waitForHostRenderEvidence,
} from "./support/remote-host";

test("real remote host completes edit, reconnect, render, and download", async ({ page }, testInfo) => {
  await page.setViewportSize({ width: 1440, height: 900 });
  await pairAndCreate(page, "Desktop remote acceptance", "Desktop browser");
  await uploadVideoAndApplyAgent(page);

  const video = page.locator("video").first();
  await expect(video).toBeVisible();
  await video.evaluate((element) => {
    element.currentTime = 0.4;
  });

  await page.reload({ waitUntil: "networkidle" });
  await page
    .getByRole("article", { name: "Recent project Desktop remote acceptance" })
    .filter({ hasText: "Available" })
    .first()
    .getByRole("button", { name: "Open project" })
    .click();
  await expect(page.getByText(/00:00:00\.(?:799|8)/)).toBeVisible();
  await reconcileUnconfirmedEdit(page);

  const artifact = await exportAndDownload(page, "Desktop remote acceptance", {
    disconnectDuringRender: true,
  });
  const path = testInfo.outputPath("desktop-remote-acceptance.mp4");
  await artifact.saveAs(path);
  expect((await artifact.createReadStream())).not.toBeNull();
  const evidence = await waitForHostRenderEvidence("Desktop remote acceptance");
  expect((await stat(path)).size).toBe(evidence.outputBytes);
});
