import { expect, test } from "@playwright/test";
import { stat } from "node:fs/promises";
import {
  exportAndDownload,
  pairAndCreate,
  uploadVideoAndApplyAgent,
  waitForHostRenderEvidence,
} from "./support/remote-host";

test.use({ viewport: { width: 402, height: 874 } });

test("phone viewport completes the core remote workflow", async ({ page }, testInfo) => {
  await pairAndCreate(page, "Phone remote acceptance", "Phone browser");
  await uploadVideoAndApplyAgent(page);
  await expect(page.getByRole("button", { name: "Export" })).toBeVisible();
  const artifact = await exportAndDownload(page, "Phone remote acceptance");
  const path = testInfo.outputPath("phone-remote-acceptance.mp4");
  await artifact.saveAs(path);
  const evidence = await waitForHostRenderEvidence("Phone remote acceptance");
  expect((await stat(path)).size).toBe(evidence.outputBytes);
});
