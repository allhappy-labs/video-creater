import { expect, test } from "@playwright/test";
import { readFile, stat } from "node:fs/promises";
import { exportAndDownload, pairAndCreate } from "./support/remote-host";

test("remote shader and motion previews produce a playable animated export", async ({ page }, testInfo) => {
  test.setTimeout(180_000);
  await page.setViewportSize({ width: 1440, height: 900 });
  await page.route("**/api/v1/rpc", async (route) => {
    const envelope = route.request().postDataJSON();
    if (envelope.operation === "remote_create_project") {
      envelope.payload.project.renderSettings = { ...envelope.payload.project.renderSettings, width: 128, height: 72, fps: 8 };
      await route.continue({ postData: JSON.stringify(envelope) });
    } else await route.continue();
  });
  await pairAndCreate(page, "Animated export acceptance", "Graphics browser");
  await page.getByRole("tab", { name: "Effects", exact: true }).click();
  await page.getByRole("button", { name: "Backgrounds", exact: true }).click();
  await page.getByRole("button", { name: "Add Octagrams to the timeline", exact: true }).click();
  await expect(page.getByTestId("canonical-prepared-preview-frame")).toBeVisible({ timeout: 90_000 });
  await page.getByRole("tab", { name: "Text", exact: true }).click();
  await page.getByRole("button", { name: "Add Punchy Caption", exact: true }).click();
  await expect(page.getByTestId("canonical-prepared-preview-frame")).toHaveCount(2, { timeout: 90_000 });
  const artifact = await exportAndDownload(page, "Animated export acceptance");
  const path = testInfo.outputPath("shader-motion-export.mp4");
  await artifact.saveAs(path);
  expect((await stat(path)).size).toBeGreaterThan(1024);
  const encoded = (await readFile(path)).toString("base64");
  const evidence = await page.evaluate(async (data) => {
    const video = document.createElement("video");
    video.muted = true;
    const bytes = Uint8Array.from(atob(data), (value) => value.charCodeAt(0));
    const url = URL.createObjectURL(new Blob([bytes], { type: "video/mp4" }));
    try {
      const loaded = new Promise<void>((resolve, reject) => { video.onloadeddata = () => resolve(); video.onerror = () => reject(new Error("Export video failed to decode")); });
      video.src = url;
      await loaded;
      const sample = async (time: number) => {
        const sought = new Promise<void>((resolve) => { video.onseeked = () => resolve(); });
        video.currentTime = time;
        await sought;
        const canvas = document.createElement("canvas"); canvas.width = 32; canvas.height = 18;
        const context = canvas.getContext("2d")!;
        context.drawImage(video, 0, 0, 32, 18);
        return Array.from(context.getImageData(0, 0, 32, 18).data);
      };
      return { width: video.videoWidth, height: video.videoHeight, duration: video.duration, first: await sample(0.5), second: await sample(2) };
    } finally { video.pause(); video.removeAttribute("src"); video.load(); URL.revokeObjectURL(url); }
  }, encoded);
  expect(evidence).toMatchObject({ width: 1280, height: 720 });
  expect(evidence.duration).toBeGreaterThan(2);
  expect(evidence.first).not.toEqual(evidence.second);
});
