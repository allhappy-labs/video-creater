// AI flows 1 and 2 of the Linux desktop smoke run (`--agent-flows`), sent through the AI tab to the app's
// bundled Codex app-server: a safe edit that auto-applies with result frames, Show changes and Undo; then
// a review edit that is dismissed without changing the timeline.

import { existsSync, readdirSync, statSync } from "node:fs";
import { join } from "node:path";

import { retainProjectFiles } from "./linux-desktop-smoke-retain.mjs";
import {
  agentUnavailableSection,
  aiPanel,
  appliedCard,
  autoApplySwitch,
  composer,
  conversationCard,
  editorTab,
  openSampleEditor,
  resultPreview,
  reviewCard,
  undoneCard,
} from "./linux-desktop-smoke-selectors.mjs";
import { skipped } from "./linux-desktop-smoke-steps.mjs";

export const agentFlowStepNames = [
  "agent flow 1: a safe edit auto-applies with result frames",
  "agent flow 1: Show changes highlights the changed clips",
  "agent flow 1: Undo restores the timeline",
  "agent flow 2: a review edit waits, and Dismiss leaves the timeline unchanged",
];

const projectDir = "/tmp/video-creater-editor-project";

/** The support rows every install needs, whichever agent answers a turn. */
export const requiredAgentComponentIds = ["agent.mcpServer", "agent.proposalValidator"];
/**
 * The interchangeable conversation-turn backends. Neither is required: Claude runs on the
 * user's own subscription with nothing bundled, Codex runs from the bundled sidecar, and an
 * install with one working backend is a working install (src-tauri/src/settings/health.rs's
 * `agent_category_health` makes the same call).
 */
export const agentBackendComponentIds = ["agent.codex", "agent.claude"];
/** Every row the agent self-test step exercises, so a run records all four diagnoses. */
export const agentSelfTestComponentIds = [...requiredAgentComponentIds, ...agentBackendComponentIds];

/**
 * Whether the recorded self-test results amount to an install that can take a turn.
 *
 * The support rows must pass. The backends are an either-or: the gate used to demand
 * `agent.codex` alone, which turned an install with a signed-in Claude and no staged sidecar
 * into a red run for an app the user could use perfectly well. Every row's state and diagnosis
 * is still recorded either way; only the verdict changed.
 *
 * @param {Record<string, { state?: string }>} results keyed by component id
 */
export function agentSelfTestVerdict(results) {
  const succeeded = (id) => results[id]?.state === "succeeded";
  const failedRequired = requiredAgentComponentIds.filter((id) => !succeeded(id));
  const readyBackends = agentBackendComponentIds.filter(succeeded);
  const unavailableBackends = agentBackendComponentIds.filter((id) => !succeeded(id));
  if (failedRequired.length > 0) {
    return { ok: false, readyBackends, unavailableBackends, reason: `required agent components failed: ${failedRequired.join(", ")}` };
  }
  if (readyBackends.length === 0) {
    return { ok: false, readyBackends, unavailableBackends, reason: `no conversation-turn backend is ready: ${unavailableBackends.join(", ")}` };
  }
  return { ok: true, readyBackends, unavailableBackends, reason: null };
}

/** The settings health row and the stored preference patch that pin one conversation-turn backend. */
const agentBackends = {
  codex: { componentId: "agent.codex", patch: { agentBackend: "codex" } },
  claude: { componentId: "agent.claude", patch: { agentBackend: "claude" } },
};

/**
 * How the flows pin a backend: the preference patch the run stores before its first prompt, the
 * settings health row that must be ready, and the patch that hands the choice back to the app.
 *
 * The flows go through the same stored preference the Agent settings page writes, so a run proves
 * the shipped resolution path rather than a smoke-only override.
 */
export function agentBackendPlan(backend, claudeModel) {
  const entry = agentBackends[backend];
  if (!entry) throw new Error(`unknown agent backend ${backend}`);
  return {
    componentId: entry.componentId,
    patch: backend === "claude" ? { ...entry.patch, claudeModel } : { ...entry.patch },
    restorePatch: { agentBackend: "automatic" },
  };
}
// A turn ends with an applied or review card, a failure card, or the "AI agent unavailable" section that
// replaces the card when the agent can't be reached (src/editor/panels/ai/missing-agent-state.tsx).
const failureLabels = ["Couldn't validate the edit", "Nothing was changed", "The edit didn't finish", agentUnavailableSection];

const conversationList = "main[aria-label='Video editor workspace'] ol[aria-label='Conversation']";
const turnCards = `${conversationList} article, ${conversationList} section[aria-label='AI agent unavailable']`;
/** A WebDriver script returning each turn card (result cards and agent-unavailable sections) in document order. */
export const conversationCardsScript = `return [...document.querySelectorAll(${JSON.stringify(turnCards)})]
    .map((card) => ({ label: card.getAttribute('aria-label'), text: card.innerText }));`;

/** Whether a conversation card ends the turn: applied, waiting for review, or failed. */
export function isFinishedTurnCard(card) {
  return Boolean(card?.label && (card.label.startsWith("Applied to ") || card.label === "Needs your review" || failureLabels.includes(card.label)));
}

/** The timeline's tracks and item placement, ignoring bookkeeping such as `contentRevision`. */
export function timelineFingerprint(project) {
  const tracks = (project?.timeline?.tracks ?? []).map((track) => ({
    id: track.id,
    name: track.name,
    kind: track.kind,
    items: (track.items ?? []).map((item) => ({
      id: item.id,
      kind: item.kind,
      startSeconds: item.startSeconds,
      durationSeconds: item.durationSeconds,
      source: item.source ?? null,
      sourceIn: item.properties?.sourceIn ?? null,
      sourceOut: item.properties?.sourceOut ?? null,
    })),
  }));
  return JSON.stringify(tracks);
}

/** Prompts built from the sample: trim the last visual clip (safe), remove a whole track (review). */
export function agentFlowPrompts(project) {
  const tracks = project?.timeline?.tracks ?? [];
  const visualClips = tracks
    .flatMap((track) => (track.items ?? []).map((item) => ({ track, item })))
    .filter(({ item }) => item.kind === "video_clip" || item.kind === "image_clip");
  if (visualClips.length === 0) throw new Error("sample project has no clips to edit");
  const last = visualClips.reduce((latest, entry) => (entry.item.startSeconds >= latest.item.startSeconds ? entry : latest));
  const candidates = tracks.filter((track) => (track.items ?? []).length > 0 && track.id !== last.track.id);
  const reviewTrack = candidates.at(-1) ?? last.track;
  return {
    safe: `Trim one second off the end of the "${last.item.label}" clip. Change nothing else.`,
    safeItemId: last.item.id,
    review: `Remove the whole "${reviewTrack.name}" track from the timeline, including every clip on it.`,
    reviewTrackId: reviewTrack.id,
  };
}

/** Project-relative result-frame PNGs (`renders/<jobId>/preview-qa/preview-frames/*.png`) newer than `sinceMs`. */
export function resultFramePngs(dir, sinceMs) {
  const renders = join(dir, "renders");
  if (!existsSync(renders)) return [];
  return readdirSync(renders)
    .flatMap((jobId) => {
      const frames = join(renders, jobId, "preview-qa", "preview-frames");
      if (!existsSync(frames)) return [];
      return readdirSync(frames)
        .filter((name) => name.endsWith(".png") && statSync(join(frames, name)).mtimeMs >= sinceMs)
        .map((name) => `renders/${jobId}/preview-qa/preview-frames/${name}`);
    })
    .sort();
}

export async function runAgentFlowSteps({ driver, step, outDir, backend = "claude", claudeModel = "sonnet" }) {
  const plan = agentBackendPlan(backend, claudeModel);
  const { click, execute, find, invoke, poll, pressKeys, screenshot, sleep, type } = driver;
  const loadProject = () => invoke("load_split_project_from_folder", { projectDir });
  const cards = () => execute(conversationCardsScript);
  const highlights = () => execute(`return document.querySelectorAll("[data-testid='clip-highlight']").length;`);
  const flow = { applied: null, restoreAutoApply: null, backendPinned: false };

  // Sends a prompt and waits for the turn's card: applied, review or a failure card.
  const send = async (prompt, timeoutMs) => {
    const baseline = (await cards()).length;
    await type(composer, prompt);
    await pressKeys(driver.keys.enter);
    const all = await poll(
      cards,
      (current) => current.length > baseline && isFinishedTurnCard(current.at(-1)),
      timeoutMs,
      2000,
    );
    return { position: all.length, card: all.at(-1) };
  };

  try {
    await step(agentFlowStepNames[0], async () => {
      await invoke("update_app_preferences", { patch: plan.patch });
      flow.backendPinned = true;
      const health = await poll(
        () => invoke("get_agent_settings_health"),
        (entry) => entry.items?.some((item) => item.id === plan.componentId && item.state !== "checking"),
        60_000,
        2000,
      );
      const agent = health.items.find((item) => item.id === plan.componentId);
      if (agent.state !== "ready") throw new Error(`${plan.componentId} is ${agent.state}: ${agent.diagnosticCode} ${agent.summary}`);
      await openSampleEditor(driver);
      await click(editorTab("AI"));
      await find(aiPanel);
      const switchElement = await find(autoApplySwitch);
      if ((await driver.attribute(switchElement, "aria-checked")) !== "true") {
        await driver.clickElement(switchElement);
        flow.restoreAutoApply = false;
        await poll(() => driver.attribute(switchElement, "aria-checked"), (checked) => checked === "true", 10_000, 250);
      }
      const project = await loadProject();
      const prompts = agentFlowPrompts(project);
      flow.prompts = prompts;
      flow.before = timelineFingerprint(project);
      const startedAt = Date.now();
      const { position, card } = await send(prompts.safe, 240_000);
      if (!card.label.startsWith("Applied to ")) throw new Error(`the safe edit did not apply: ${card.label}: ${card.text}`);
      const cardXPath = conversationCard(position);
      await find(appliedCard);
      const frame = await poll(
        () =>
          execute(
            `const card = document.evaluate(arguments[0], document, null, XPathResult.FIRST_ORDERED_NODE_TYPE, null).singleNodeValue;
            const preview = card && card.querySelector("[role='group'][aria-label='Result preview']");
            const image = preview && preview.querySelector('img');
            return { unavailable: Boolean(preview && preview.innerText.includes("Preview frames aren't available.")), loaded: Boolean(image && image.complete && image.naturalWidth > 0), width: image ? image.naturalWidth : 0, images: preview ? preview.querySelectorAll('img').length : 0 };`,
            [cardXPath],
          ),
        (state) => {
          if (state.unavailable) throw new Error("the applied card shows \"Preview frames aren't available.\"");
          return state.loaded;
        },
        120_000,
        2000,
      );
      await find(`${cardXPath}${resultPreview}//img`);
      const pngs = resultFramePngs(projectDir, startedAt);
      if (pngs.length === 0) throw new Error("no result-frame PNGs were written under renders/*/preview-qa/preview-frames/");
      const retained = retainProjectFiles(projectDir, pngs, join(outDir, "agent-flow-1", "frames"));
      const after = await loadProject();
      if (timelineFingerprint(after) === flow.before) throw new Error("the applied edit did not change the saved timeline");
      flow.applied = { position, cardXPath };
      return {
        backend,
        model: backend === "claude" ? claudeModel : undefined,
        agentHealth: { id: agent.id, state: agent.state, summary: agent.summary, provenance: agent.provenance },
        prompt: prompts.safe,
        status: card.label,
        facts: await execute(
          `const card = document.evaluate(arguments[0], document, null, XPathResult.FIRST_ORDERED_NODE_TYPE, null).singleNodeValue;
          const facts = card && card.querySelector("ul[aria-label='Facts']");
          return facts ? facts.innerText : null;`,
          [cardXPath],
        ),
        cardText: card.text,
        frame,
        frames: retained,
        screenshot: await screenshot("agent-flow-1-applied"),
      };
    });

    await step(agentFlowStepNames[1], async () => {
      if (!flow.applied) return skipped("flow 1 did not apply");
      await click(`${flow.applied.cardXPath}//button[normalize-space()='Show changes']`);
      const count = await poll(highlights, (value) => value > 0, 15_000, 250);
      return { highlights: count, screenshot: await screenshot("agent-flow-1-show-changes") };
    });

    await step(agentFlowStepNames[2], async () => {
      if (!flow.applied) return skipped("flow 1 did not apply");
      await click(`${flow.applied.cardXPath}//button[normalize-space()='Undo']`);
      await find(`${conversationCard(flow.applied.position)}[@aria-label='Undone']`, 60_000);
      await find(undoneCard);
      await poll(async () => timelineFingerprint(await loadProject()), (value) => value === flow.before, 30_000, 500);
      const remaining = await poll(highlights, (value) => value === 0, 10_000, 250);
      return { highlightsAfterUndo: remaining, screenshot: await screenshot("agent-flow-1-undone") };
    });

    await step(agentFlowStepNames[3], async () => {
      await click(editorTab("AI"));
      await find(composer);
      const project = await loadProject();
      const prompts = flow.prompts ?? agentFlowPrompts(project);
      const before = timelineFingerprint(project);
      const { position, card } = await send(prompts.review, 240_000);
      const cardXPath = conversationCard(position);
      if (card.label.startsWith("Applied to ")) {
        await click(`${cardXPath}//button[normalize-space()='Undo']`);
        await find(`${cardXPath}[@aria-label='Undone']`, 60_000);
        throw new Error("the agent applied a request expected to need review");
      }
      if (card.label !== "Needs your review") throw new Error(`the review edit failed: ${card.label}: ${card.text}`);
      await find(reviewCard);
      const plannedChanges = await execute(
        `const card = document.evaluate(arguments[0], document, null, XPathResult.FIRST_ORDERED_NODE_TYPE, null).singleNodeValue;
        const list = card && card.querySelector("ol[aria-label='Planned changes']");
        return list ? list.innerText : null;`,
        [cardXPath],
      );
      const shot = await screenshot("agent-flow-2-review");
      if (timelineFingerprint(await loadProject()) !== before) throw new Error("the timeline changed before the review edit was approved");
      await click(`${cardXPath}//button[normalize-space()='Dismiss']`);
      await poll(() => execute("return document.body.innerText.includes('Dismissed');"), Boolean, 15_000, 250);
      await sleep(3000);
      if (timelineFingerprint(await loadProject()) !== before) throw new Error("the timeline changed after Dismiss");
      return {
        prompt: prompts.review,
        status: card.label,
        plannedChanges,
        cardText: card.text,
        screenshots: [shot, await screenshot("agent-flow-2-dismissed")],
      };
    });
  } finally {
    if (flow.restoreAutoApply === false) {
      await click(autoApplySwitch).catch(() => {});
    }
    if (flow.backendPinned) {
      await invoke("update_app_preferences", { patch: plan.restorePatch }).catch(() => {});
    }
  }
}
