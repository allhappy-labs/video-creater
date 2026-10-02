import { expect, test, type Locator } from "@playwright/test";
import { timelineClips } from "./support/editor";
import { pairAndCreate, uploadVideoAndApplyAgent } from "./support/remote-host";

interface RpcEnvelope {
  requestId: string;
  operation: string;
  projectId: string;
  expectedRevision: number;
  payload: { actions?: Array<{ type: string; splits?: Array<{ newItemId: string }> }> };
}

interface CanonicalProject {
  id: string;
  contentRevision: number;
  timeline: { tracks: Array<{ items: Array<{ id: string }> }> };
}

interface RecoveryMarker {
  hostOrigin: string;
  requestId: string;
  operation: string;
  projectId: string;
  canonicalProjectId?: string;
  phase: string;
  outcome: string;
}

const storageKey = "video-creater.remotePendingOutcomes.v1";

async function selectMidpoint(clip: Locator) {
  const box = await clip.boundingBox();
  if (!box) throw new Error("The fixture clip is not laid out.");
  await clip.click({ position: { x: box.width / 2, y: box.height / 2 } });
  await expect(clip).toHaveAttribute("aria-selected", "true");
}

for (const recovery of ["reload", "new tab with cleared storage"] as const) {
test(`remote edit uncertainty survives ${recovery} and canonical refresh without replay`, async ({ page: initialPage, context }) => {
  let page = initialPage;
  await page.setViewportSize({ width: 1440, height: 900 });
  const projectName = `Remote ${recovery === "reload" ? "reload" : "fresh-tab"} recovery`;
  await pairAndCreate(page, projectName, "Recovery browser");
  await uploadVideoAndApplyAgent(page);

  let clips = timelineClips(page);
  const initialCount = await clips.count();
  expect(initialCount).toBeGreaterThan(0);
  let splitButton = page.getByRole("toolbar", { name: "Timeline tools" }).getByRole("button", { name: "Split", exact: true });
  const splitRequests: Array<{ requestId: string; newItemIds: string[] }> = [];
  let lostRequest: RpcEnvelope | undefined;
  let committedProject: CanonicalProject | undefined;

  await context.route("**/api/v1/rpc", async (route) => {
    const envelope = route.request().postDataJSON() as RpcEnvelope;
    const newItemIds = envelope.payload.actions?.flatMap((action) => action.type === "splitItems" ? (action.splits ?? []).map((split) => split.newItemId) : []) ?? [];
    if (envelope.operation !== "apply_project_actions_to_split_project_folder" || newItemIds.length === 0) {
      await route.continue();
      return;
    }
    splitRequests.push({ requestId: envelope.requestId, newItemIds });
    if (lostRequest) {
      await route.continue();
      return;
    }
    lostRequest = envelope;
    // Forward the authenticated request to the real host and lose only its committed ACK.
    const response = await route.fetch();
    expect(response.ok()).toBe(true);
    const body = await response.json() as { requestId: string; ok: boolean; result: { project: CanonicalProject } };
    expect(body).toMatchObject({ requestId: envelope.requestId, ok: true });
    committedProject = body.result.project;
    expect(committedProject.contentRevision).toBe(envelope.expectedRevision + 1);
    await route.abort("failed");
  });

  await selectMidpoint(clips.first());
  await expect(splitButton).not.toHaveAttribute("aria-disabled", "true");
  await splitButton.click();
  await expect.poll(() => committedProject !== undefined, { timeout: 30_000 }).toBe(true);
  let notice = page.getByRole("alert", { name: "Unconfirmed edit" });
  await expect(notice).toBeVisible();
  if (!lostRequest || !committedProject) throw new Error("The fixture edit did not reach the host.");
  const dropped = lostRequest;
  const canonical = committedProject;
  const committedIds = canonical.timeline.tracks.flatMap((track) => track.items.map((item) => item.id)).sort();
  expect(committedIds).toHaveLength(initialCount + splitRequests[0]!.newItemIds.length);
  expect(new Set(committedIds).size).toBe(committedIds.length);

  const retained = await page.evaluate((key) => JSON.parse(sessionStorage.getItem(key) ?? "[]") as RecoveryMarker[], storageKey);
  expect(retained).toHaveLength(1);
  expect(retained[0]).toMatchObject({ requestId: dropped.requestId, projectId: dropped.projectId, operation: dropped.operation, outcome: "unknown" });
  // An unrelated well-formed marker proves refresh clears only this project's request.
  // This sentinel contains no credentials or payload and never invokes a host mutation.
  const unrelated = { ...retained[0]!, projectId: "unrelated-recovery-project", requestId: "browser-unrelated-recovery" };
  await page.evaluate(({ key, markers }) => sessionStorage.setItem(key, JSON.stringify(markers)), { key: storageKey, markers: [...retained, unrelated] });

  const expectedMarkers = recovery === "reload" ? [unrelated] : [];
  if (recovery === "reload") {
    await page.reload({ waitUntil: "networkidle" });
  } else {
    await page.evaluate((key) => sessionStorage.removeItem(key), storageKey);
    page = await context.newPage();
    await page.setViewportSize({ width:1440, height:900 });
    await initialPage.close();
    await page.goto("/", { waitUntil:"networkidle" });
    clips = timelineClips(page);
    splitButton = page.getByRole("toolbar", { name:"Timeline tools" }).getByRole("button", { name:"Split", exact:true });
    notice = page.getByRole("alert", { name:"Unconfirmed edit" });
    const restored = await page.evaluate((key) => JSON.parse(sessionStorage.getItem(key) ?? "[]") as RecoveryMarker[], storageKey);
    expect(restored).toContainEqual(expect.objectContaining({ requestId:dropped.requestId, projectId:dropped.projectId }));
  }
  await page.getByRole("article", { name: `Recent project ${projectName}` }).filter({ hasText: "Available" }).first().getByRole("button", { name: "Open project" }).click();
  await expect(notice).toBeVisible();
  await expect(clips).toHaveCount(committedIds.length);
  expect((await clips.evaluateAll((elements) => elements.map((element) => element.getAttribute("data-item-id")))).sort()).toEqual(committedIds);
  expect(splitRequests).toHaveLength(1);

  // A new edit is held locally until the restored unknown request is reconciled.
  await selectMidpoint(clips.first());
  await splitButton.click();
  await expect(notice).toBeVisible();
  await expect(clips).toHaveCount(committedIds.length);
  expect(splitRequests).toHaveLength(1);

  const refreshResponse = page.waitForResponse((response) => {
    if (!response.url().endsWith("/api/v1/rpc") || response.request().method() !== "POST") return false;
    const envelope = response.request().postDataJSON() as RpcEnvelope;
    return envelope.operation === "read_project_snapshot_from_split_project_folder" && envelope.projectId === dropped.projectId;
  });
  await notice.getByRole("button", { name: "Refresh project", exact: true }).click();
  const refreshed = await refreshResponse;
  expect(refreshed.ok()).toBe(true);
  const refreshEnvelope = refreshed.request().postDataJSON() as RpcEnvelope;
  expect(refreshEnvelope.requestId).not.toBe(dropped.requestId);
  const refreshBody = await refreshed.json() as { requestId: string; ok: boolean; result: CanonicalProject };
  expect(refreshBody).toMatchObject({ requestId: refreshEnvelope.requestId, ok: true, result: { id: canonical.id, contentRevision: canonical.contentRevision } });
  await expect(notice).toHaveCount(0);
  await expect(clips).toHaveCount(committedIds.length);
  await expect.poll(async () => page.evaluate((key) => JSON.parse(sessionStorage.getItem(key) ?? "[]"), storageKey), { timeout: 30_000 }).toEqual(expectedMarkers);

  // Resume through the UI using a fresh operation, without replaying the original split.
  await selectMidpoint(clips.first());
  const nextResponse = page.waitForResponse((response) => response.url().endsWith("/api/v1/rpc") && response.request().method() === "POST" && (response.request().postDataJSON() as RpcEnvelope).operation === "apply_project_actions_to_split_project_folder");
  await splitButton.click();
  const next = await nextResponse;
  expect(next.ok()).toBe(true);
  const nextBody = await next.json() as { ok: boolean; result: { project: CanonicalProject } };
  expect(nextBody).toMatchObject({ ok: true, result: { project: { contentRevision: canonical.contentRevision + 1 } } });
  expect(splitRequests).toHaveLength(2);
  expect(splitRequests[1]!.requestId).not.toBe(dropped.requestId);
  for (const originalId of splitRequests[0]!.newItemIds) expect(splitRequests[1]!.newItemIds).not.toContain(originalId);
  await expect(clips).toHaveCount(committedIds.length + splitRequests[1]!.newItemIds.length);
  await expect(notice).toHaveCount(0);
  await expect.poll(async () => page.evaluate((key) => JSON.parse(sessionStorage.getItem(key) ?? "[]"), storageKey), { timeout: 30_000 }).toEqual(expectedMarkers);
});

}
