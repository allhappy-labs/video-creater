import { expect, type Page } from "@playwright/test";
import { access, readFile, readdir, stat } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";

const pairingCode = process.env.VIDEO_CREATER_REMOTE_E2E_PAIRING_CODE ?? "135790";

export async function pairAndCreate(page: Page, projectName: string, deviceName: string) {
  await page.goto("/");
  if (await page.getByLabel("Pairing code").isVisible()) {
    await page.getByLabel("Pairing code").fill(pairingCode);
    await page.getByLabel("Device name").fill(deviceName);
    await page.getByRole("button", { name: "Pair device" }).click();
  }
  await expect(page.getByText("Welcome to Video Creater")).toBeVisible();
  await page.getByRole("button", { name: "New project" }).click();
  await page.getByLabel("Project name").fill(projectName);
  await page.getByRole("button", { name: "Create project" }).click();
  await expect(page.getByText(projectName)).toBeVisible();
}

export async function uploadVideoAndApplyAgent(page: Page) {
  await page.getByText("Media", { exact: true }).click();
  await page.locator('input[type="file"]').first().setInputFiles("e2e/fixtures/preview.webm");
  await expect(page.getByText("preview.webm", { exact: true })).toBeVisible();
  const closeMedia = page.getByRole("button", { name: "Close Media" });
  if (await closeMedia.isVisible()) await closeMedia.click();

  await page.getByText("AI", { exact: true }).click();
  await page.getByPlaceholder("Describe an edit…").fill(
    "Create a concise video EDL rough cut from the imported media",
  );
  await page.getByRole("button", { name: /send/i }).click();
  await expect(page.getByText("Applied to Timeline 1")).toBeVisible({ timeout: 30_000 });
  await expect(page.getByText(/00:00:00\.(?:799|8)/)).toBeVisible();
  const closeAi = page.getByRole("button", { name: "Close AI" });
  if (await closeAi.isVisible()) await closeAi.click();
}

/** A reload can interrupt an acknowledged-on-host operation; resume only after a safe read. */
export async function reconcileUnconfirmedEdit(page: Page) {
  const notice = page.getByRole("alert", { name: "Unconfirmed edit" });
  if (await notice.isVisible()) {
    await notice.getByRole("button", { name: "Refresh project", exact: true }).click();
    await expect(notice).toHaveCount(0);
  }
}

export async function exportAndDownload(
  page: Page,
  name: string,
  options: { disconnectDuringRender?: boolean } = {},
) {
  await page.getByRole("button", { name: "Export" }).click();
  await page.getByLabel("Name").fill(name);
  await page.getByText("720p", { exact: true }).click();
  await page.getByText("Draft", { exact: true }).click();
  await page.getByRole("button", { name: "Export video" }).click();
  const backgroundTasks = page.getByRole("button", { name: "Background tasks" });
  if (options.disconnectDuringRender) {
    await expect(backgroundTasks).toContainText("Exporting", { timeout: 10_000 });
    await page.context().setOffline(true);
    await page.waitForTimeout(750);
    await page.context().setOffline(false);
    await waitForHostRenderEvidence(name);
    await page.reload({ waitUntil: "networkidle" });
    await page
      .getByRole("article", { name: `Recent project ${name}` })
      .filter({ hasText: "Available" })
      .first()
      .getByRole("button", { name: "Open project" })
      .click();
    await expect(page.getByRole("main", { name: "Video editor workspace" })).toBeVisible();
    await reconcileUnconfirmedEdit(page);
  }
  await expect(backgroundTasks).toContainText("Export complete", { timeout: 60_000 });

  const downloadButton = page.getByRole("button", { name: "Download" });
  if (!(await downloadButton.isVisible())) {
    await backgroundTasks.click();
  }
  const download = page.waitForEvent("download");
  await downloadButton.click();
  const artifact = await download;
  expect(artifact.suggestedFilename()).toMatch(/\.mp4$/i);
  return artifact;
}

interface HostRenderEvidence {
  readonly outputPath: string;
  readonly outputBytes: number;
  readonly durationSeconds: number;
  readonly video: boolean;
  readonly audio: boolean;
  readonly logPath: string;
}

export async function waitForHostRenderEvidence(projectName: string): Promise<HostRenderEvidence> {
  const dataRoot = process.env.VIDEO_CREATER_REMOTE_E2E_DATA_DIR
    ?? resolve(tmpdir(), "video-creater-remote-host-e2e");
  const deadline = Date.now() + 60_000;
  let lastError: unknown;
  while (Date.now() < deadline) {
    try {
      const projectsRoot = join(dataRoot, "projects");
      for (const entry of await readdir(projectsRoot, { withFileTypes: true })) {
        if (!entry.isDirectory() || !entry.name.endsWith(".palmier")) continue;
        const projectDir = join(projectsRoot, entry.name);
        const project = JSON.parse(await readFile(join(projectDir, "video-creater.project.json"), "utf8")) as {
          readonly name?: string;
        };
        if (project.name !== projectName) continue;
        const index = JSON.parse(await readFile(join(projectDir, "renders/index.json"), "utf8")) as {
          readonly reports?: Array<{
            readonly reportId: string;
            readonly status: string;
            readonly outputPath: string;
            readonly durationSeconds: number;
            readonly video: boolean;
            readonly audio: boolean;
            readonly logPath: string;
          }>;
        };
        const summary = index.reports?.find((report) =>
          report.status === "completed" && report.outputPath.toLowerCase().endsWith(".mp4"));
        if (!summary) continue;
        const report = JSON.parse(await readFile(join(projectDir, `renders/${summary.reportId}/report.json`), "utf8")) as {
          readonly artifacts?: string[];
          readonly checks?: Record<string, string>;
        };
        expect(summary.video).toBe(true);
        expect(summary.audio).toBe(true);
        expect(summary.durationSeconds).toBeGreaterThan(0.7);
        expect(summary.durationSeconds).toBeLessThan(1.0);
        expect(report.checks?.streams).toBe("passed");
        expect(report.checks?.duration).toBe("passed");
        expect(report.checks?.artifactPaths).toBe("passed");
        expect(report.checks?.logPath).toBe("passed");
        expect(report.artifacts).toContain(summary.outputPath);
        expect(report.artifacts).toContain(summary.logPath);
        const output = join(projectDir, summary.outputPath);
        await access(join(projectDir, summary.logPath));
        const outputBytes = (await stat(output)).size;
        expect(outputBytes).toBeGreaterThan(1_024);
        return { ...summary, outputBytes };
      }
    } catch (error) {
      lastError = error;
    }
    await new Promise((resolveDelay) => setTimeout(resolveDelay, 250));
  }
  throw new Error(`Host render evidence did not settle: ${String(lastError)}`);
}
