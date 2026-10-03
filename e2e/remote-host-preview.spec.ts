import { expect, test, type Locator, type Page } from "@playwright/test";
import { pairAndCreate } from "./support/remote-host";

const videoFixture = "e2e/fixtures/preview.webm";
const animationFixture = "src-tauri/tests/fixtures/transitions/lottie-colour-steps.json";

/** Bound actual GPU/render work while using the real creation, storage and RPC paths. */
async function openPreviewProject(page: Page, name: string) {
  await page.route("**/api/v1/rpc", async (route) => {
    const envelope = route.request().postDataJSON();
    if (envelope.operation === "remote_create_project") {
      envelope.payload.project.renderSettings = { ...envelope.payload.project.renderSettings, width: 128, height: 72, fps: 8 };
      await route.continue({ postData: JSON.stringify(envelope) });
    } else await route.continue();
  });
  await pairAndCreate(page, name, "Preview test browser");
}

async function importAsset(page: Page, file: string | { name: string; mimeType: string; buffer: Buffer }, name: string) {
  await openMedia(page);
  await page.locator('input[type="file"]').first().setInputFiles(file);
  await expect(page.getByRole("button", { name: `Preview ${name}`, exact: true })).toBeVisible();
}

/** Small deterministic PCM tone: real upload/probe/decoding without a binary fixture. */
function audioFixture() {
  const rate = 22050;
  const samples = rate * 2;
  const buffer = Buffer.alloc(44 + samples * 2);
  buffer.write("RIFF", 0); buffer.writeUInt32LE(buffer.length - 8, 4); buffer.write("WAVEfmt ", 8);
  buffer.writeUInt32LE(16, 16); buffer.writeUInt16LE(1, 20); buffer.writeUInt16LE(1, 22);
  buffer.writeUInt32LE(rate, 24); buffer.writeUInt32LE(rate * 2, 28);
  buffer.writeUInt16LE(2, 32); buffer.writeUInt16LE(16, 34);
  buffer.write("data", 36); buffer.writeUInt32LE(samples * 2, 40);
  for (let i = 0; i < samples; i++) buffer.writeInt16LE(Math.round(4000 * Math.sin(2 * Math.PI * 440 * i / rate)), 44 + i * 2);
  return { name: "preview-tone.wav", mimeType: "audio/wav", buffer };
}
async function openMedia(page: Page) {
  const tab = page.getByRole("tab", { name: "Media", exact: true });
  if (await tab.count()) await tab.click();
  else await page.getByRole("button", { name: "Media", exact: true }).click();
}
async function waitForVideo(video: Locator) {
  await expect.poll(() => video.evaluate((element: HTMLVideoElement) => ({
    ready: element.readyState >= 2, width: element.videoWidth > 0, error: element.error?.code ?? null,
  }))).toEqual({ ready: true, width: true, error: null });
}
async function pixel(frame: Locator): Promise<number[]> {
  return frame.evaluate(async (element: HTMLImageElement) => {
    await element.decode();
    const canvas = document.createElement("canvas");
    canvas.width = element.naturalWidth; canvas.height = element.naturalHeight;
    const ctx = canvas.getContext("2d")!;
    ctx.drawImage(element, 0, 0);
    return Array.from(ctx.getImageData(canvas.width / 2, canvas.height / 2, 1, 1).data);
  });
}

for (const viewport of [{ width: 1440, height: 900 }, { width: 402, height: 874 }]) {
  test(`remote source and timeline video decode, play, seek and replay at ${viewport.width}px`, async ({ page }, testInfo) => {
    await page.setViewportSize(viewport);
    await openPreviewProject(page, `Video preview ${viewport.width}`);
    await importAsset(page, videoFixture, "preview.webm");
    await page.getByRole("button", { name: "Preview preview.webm", exact: true }).click();
    if (viewport.width < 1024) await page.getByRole("button", { name: "Close Media", exact: true }).click();
    const source = page.getByLabel("Video preview preview.webm", { exact: true });
    await waitForVideo(source);
    await page.getByRole("button", { name: "Play preview", exact: true }).click();
    await expect.poll(() => source.evaluate((v: HTMLVideoElement) => v.currentTime)).toBeGreaterThan(0.1);
    await expect.poll(() => source.evaluate((v: HTMLVideoElement) => v.ended)).toBe(true);
    await page.getByRole("button", { name: "Play preview", exact: true }).click();
    await expect.poll(() => source.evaluate((v: HTMLVideoElement) => !v.paused && v.currentTime < 0.9)).toBe(true);
    await page.getByRole("button", { name: "Pause preview", exact: true }).click();
    const scrubber = page.getByRole("slider", { name: "Preview scrubber" });
    await scrubber.focus(); await scrubber.press("Home"); await scrubber.press("ArrowRight");
    await expect.poll(() => source.evaluate((v: HTMLVideoElement) => v.currentTime)).toBeGreaterThan(0);
    await page.getByRole("button", { name: "Back to timeline", exact: true }).click();
    await openMedia(page);
    await page.getByRole("button", { name: "Add preview.webm at the playhead", exact: true }).click();
    if (viewport.width < 1024) await page.getByRole("button", { name: "Close Media", exact: true }).click();
    const timeline = page.getByLabel("Timeline video preview.webm", { exact: true });
    await waitForVideo(timeline);
    await page.getByRole("button", { name: "Play preview", exact: true }).click();
    await expect.poll(() => timeline.evaluate((v: HTMLVideoElement) => v.currentTime)).toBeGreaterThan(0.1);
    // Force a decoded-current-frame stall while retaining actual native playback/remote media.
    await timeline.evaluate((v: HTMLVideoElement) => Object.defineProperty(v, "readyState", { configurable: true, get: () => 2 }));
    try {
      await expect.poll(() => timeline.evaluate((v: HTMLVideoElement) => v.paused)).toBe(true);
      const held = await timeline.evaluate((v: HTMLVideoElement) => v.currentTime);
      await page.waitForTimeout(200);
      expect(await timeline.evaluate((v: HTMLVideoElement) => v.currentTime)).toBeCloseTo(held, 2);
    } finally {
      await timeline.evaluate((v: HTMLVideoElement) => Reflect.deleteProperty(v, "readyState"));
    }
    await expect.poll(() => timeline.evaluate((v: HTMLVideoElement) => !v.paused)).toBe(true);
    await expect(page.getByRole("button", { name: "Play preview", exact: true })).toBeVisible();
    await page.screenshot({ path: testInfo.outputPath(`remote-video-${viewport.width}.png`) });
  });
}

test("remote Lottie sources and timeline render changing decoded frames with working transport", async ({ page }, testInfo) => {
  await page.setViewportSize({ width: 1440, height: 900 });
  await openPreviewProject(page, "Animated source preview");
  await importAsset(page, animationFixture, "lottie-colour-steps.json");
  await page.getByRole("button", { name: "Preview lottie-colour-steps.json", exact: true }).click();
  const frame = page.getByTestId("canonical-prepared-preview-frame");
  await expect(frame).toBeVisible({ timeout: 60_000 });
  const first = await pixel(frame);
  const firstUrl = await frame.getAttribute("src");
  await page.getByRole("button", { name: "Play preview", exact: true }).click();
  await expect.poll(() => frame.getAttribute("src")).not.toBe(firstUrl);
  await expect.poll(() => pixel(frame)).not.toEqual(first);
  await expect(page.getByRole("button", { name: "Play preview", exact: true })).toBeVisible();
  await page.getByRole("button", { name: "Back to timeline", exact: true }).click();
  await page.getByRole("button", { name: "Add lottie-colour-steps.json at the playhead", exact: true }).click();
  await expect(frame).toBeVisible({ timeout: 60_000 });
  await pixel(frame);
  const initialUrl = await frame.getAttribute("src");
  await page.getByRole("button", { name: "Play preview", exact: true }).click();
  await expect.poll(() => frame.getAttribute("src")).not.toBe(initialUrl);
  await page.screenshot({ path: testInfo.outputPath("remote-lottie.png") });
});

test("remote audio sources and timeline decode, play and recover from buffering", async ({ page }) => {
  await openPreviewProject(page, "Audio preview");
  await importAsset(page, audioFixture(), "preview-tone.wav");
  await page.getByRole("button", { name: "Preview preview-tone.wav", exact: true }).click();
  const source = page.getByLabel("Audio preview preview-tone.wav", { exact: true });
  await expect.poll(() => source.evaluate((a: HTMLAudioElement) => a.duration)).toBeCloseTo(2, 2);
  expect(await source.evaluate((a: HTMLAudioElement) => a.muted)).toBe(false);
  await page.getByRole("button", { name: "Play preview", exact: true }).click();
  await expect.poll(() => source.evaluate((a: HTMLAudioElement) => !a.paused && a.currentTime > 0.1)).toBe(true);
  await page.getByRole("button", { name: "Pause preview", exact: true }).click();
  await expect.poll(() => source.evaluate((a: HTMLAudioElement) => a.paused)).toBe(true);
  await page.getByRole("button", { name: "Back to timeline", exact: true }).click();
  await page.getByRole("button", { name: "Add preview-tone.wav at the playhead", exact: true }).click();
  const timeline = page.getByLabel("Timeline audio preview-tone.wav", { exact: true });
  await page.getByRole("button", { name: "Play preview", exact: true }).click();
  await expect.poll(() => timeline.evaluate((a: HTMLAudioElement) => !a.paused && a.currentTime > 0.1)).toBe(true);
  await timeline.evaluate((a: HTMLAudioElement) => Object.defineProperty(a, "readyState", { configurable: true, get: () => 2 }));
  try {
    await expect.poll(() => timeline.evaluate((a: HTMLAudioElement) => a.paused)).toBe(true);
    const held = await timeline.evaluate((a: HTMLAudioElement) => a.currentTime);
    await page.waitForTimeout(200);
    expect(await timeline.evaluate((a: HTMLAudioElement) => a.currentTime)).toBeCloseTo(held, 2);
  } finally {
    await timeline.evaluate((a: HTMLAudioElement) => Reflect.deleteProperty(a, "readyState"));
  }
  await expect.poll(() => timeline.evaluate((a: HTMLAudioElement) => !a.paused)).toBe(true);
  expect(await timeline.evaluate((a: HTMLAudioElement) => a.error)).toBeNull();
  await expect(page.getByRole("button", { name: "Play preview", exact: true })).toBeVisible();
});

test("remote shaders and motion templates display real animated frames", async ({ page }, testInfo) => {
  await page.setViewportSize({ width: 1440, height: 900 });
  await openPreviewProject(page, "Shader and motion preview");
  await page.getByRole("tab", { name: "Effects", exact: true }).click();
  await page.getByRole("button", { name: "Backgrounds", exact: true }).click();
  await page.getByRole("button", { name: "Add Octagrams to the timeline", exact: true }).click();
  const frame = page.getByTestId("canonical-prepared-preview-frame");
  await expect(frame).toBeVisible({ timeout: 90_000 });
  const first = await pixel(frame);
  await page.getByRole("slider", { name: "Preview scrubber" }).focus();
  await page.getByRole("slider", { name: "Preview scrubber" }).press("ArrowRight");
  await page.getByRole("slider", { name: "Preview scrubber" }).press("ArrowRight");
  await expect.poll(() => pixel(frame)).not.toEqual(first);
  await page.screenshot({ path: testInfo.outputPath("remote-shader.png") });
  await page.getByRole("tab", { name: "Text", exact: true }).click();
  await page.getByRole("button", { name: "Add Punchy Caption", exact: true }).click();
  await expect(page.getByTestId("canonical-prepared-preview-frame")).toHaveCount(2, { timeout: 90_000 });
  await expect(page.getByRole("region", { name: "Preview panel", exact: true }).getByLabel("Punchy Caption preview", { exact: true })).toHaveCount(0);
  const frames = page.getByTestId("canonical-prepared-preview-frame");
  const urls = await frames.evaluateAll((elements) => elements.map((e) => e.getAttribute("src")));
  await page.getByRole("button", { name: "Play preview", exact: true }).click();
  await expect.poll(() => frames.evaluateAll((elements) => elements.map((e) => e.getAttribute("src")))).not.toEqual(urls);
  await page.screenshot({ path: testInfo.outputPath("remote-shader-motion.png") });
});
