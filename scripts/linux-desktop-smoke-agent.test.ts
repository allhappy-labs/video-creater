import assert from "node:assert/strict";
import { mkdirSync, mkdtempSync, rmSync, utimesSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import test from "node:test";

import { JSDOM } from "jsdom";

import { agentBackendComponentIds, agentBackendPlan, agentFlowPrompts, agentSelfTestComponentIds, agentSelfTestVerdict, conversationCardsScript, isFinishedTurnCard, requiredAgentComponentIds, resultFramePngs, timelineFingerprint } from "./linux-desktop-smoke-agent.mjs";
import { conversationCard } from "./linux-desktop-smoke-selectors.mjs";

function fixtureProject() {
  return {
    contentRevision: 7,
    timeline: {
      tracks: [
        {
          id: "track-video",
          name: "Video",
          kind: "video",
          items: [
            { id: "clip-a", kind: "video_clip", label: "Opening clip", startSeconds: 0, durationSeconds: 4, source: { type: "media", mediaId: "m1" } },
            { id: "clip-b", kind: "video_clip", label: "Closing clip", startSeconds: 4, durationSeconds: 4, source: { type: "media", mediaId: "m2" } },
          ],
        },
        { id: "track-empty", name: "Overlays", kind: "overlay", items: [] },
        {
          id: "track-music",
          name: "Music",
          kind: "audio",
          items: [{ id: "bed", kind: "audio_clip", label: "Music bed", startSeconds: 0, durationSeconds: 8, source: { type: "media", mediaId: "m3" } }],
        },
      ],
    },
  };
}

test("timelineFingerprint ignores the content revision", () => {
  const changed = { ...fixtureProject(), contentRevision: 99 };
  assert.equal(timelineFingerprint(changed), timelineFingerprint(fixtureProject()));
});

test("timelineFingerprint changes with item timing, source, items and tracks", () => {
  const base = timelineFingerprint(fixtureProject());
  const mutations: ((project: ReturnType<typeof fixtureProject>) => void)[] = [
    (project) => (project.timeline.tracks[0].items[1].startSeconds = 5),
    (project) => (project.timeline.tracks[0].items[1].durationSeconds = 3),
    (project) => (project.timeline.tracks[0].items[1].source = { type: "media", mediaId: "other" }),
    (project) => project.timeline.tracks[0].items.pop(),
    (project) => project.timeline.tracks[2].items.push({ ...project.timeline.tracks[2].items[0], id: "bed-2" }),
    (project) => project.timeline.tracks.pop(),
    (project) => project.timeline.tracks.push({ id: "track-new", name: "New", kind: "audio", items: [] }),
  ];
  for (const [index, mutate] of mutations.entries()) {
    const project = fixtureProject();
    mutate(project);
    assert.notEqual(timelineFingerprint(project), base, `mutation ${index}`);
  }
});

test("agentFlowPrompts names the last visual clip for the safe edit and a non-empty track for review", () => {
  const prompts = agentFlowPrompts(fixtureProject());
  assert.match(prompts.safe, /"Closing clip"/);
  assert.match(prompts.safe, /one second/);
  assert.match(prompts.safe, /end/);
  assert.match(prompts.review, /"Music"/);
  assert.match(prompts.review, /remove/i);
  assert.match(prompts.review, /whole/);
  assert.equal(prompts.safeItemId, "clip-b");
  assert.equal(prompts.reviewTrackId, "track-music");
});

test("agentFlowPrompts refuses an empty timeline", () => {
  assert.throws(() => agentFlowPrompts({ timeline: { tracks: [{ id: "t", name: "Video", kind: "video", items: [] }] } }), /sample project has no clips to edit/);
});

test("resultFramePngs lists new preview frame PNGs only", () => {
  const projectDir = mkdtempSync(join(tmpdir(), "vc-smoke-agent-"));
  try {
    const write = (path: string, mtimeMs: number) => {
      mkdirSync(join(projectDir, path, ".."), { recursive: true });
      writeFileSync(join(projectDir, path), "png");
      utimesSync(join(projectDir, path), mtimeMs / 1000, mtimeMs / 1000);
    };
    const since = Date.now() - 60_000;
    write("renders/capture-b/preview-qa/preview-frames/preview-0001.png", since + 5000);
    write("renders/capture-a/preview-qa/preview-frames/preview-0001.png", since + 5000);
    write("renders/capture-old/preview-qa/preview-frames/preview-0001.png", since - 5000);
    write("renders/capture-a/preview-qa/other.png", since + 5000);
    write("renders/capture-a/output.png", since + 5000);
    assert.deepEqual(resultFramePngs(projectDir, since), [
      "renders/capture-a/preview-qa/preview-frames/preview-0001.png",
      "renders/capture-b/preview-qa/preview-frames/preview-0001.png",
    ]);
    assert.deepEqual(resultFramePngs(join(projectDir, "missing"), since), []);
  } finally {
    rmSync(projectDir, { recursive: true, force: true });
  }
});

/** The AI tab's conversation markup: result cards are articles; an unreachable agent renders a section instead. */
function conversationDocument() {
  return new JSDOM(`<main aria-label="Video editor workspace"><ol aria-label="Conversation">
    <li><p>Trim it</p></li>
    <li><article aria-label="Applied to 1 clip"><p>Trimmed</p></article></li>
    <li><p>Remove the track</p></li>
    <li><section aria-label="AI agent unavailable"><p>Agent unavailable</p></section></li>
  </ol></main>`).window.document;
}

test("the conversation cards script lists the agent-unavailable section as the latest turn card", () => {
  const document = conversationDocument();
  const cards = new Function("document", conversationCardsScript)(document) as { label: string }[];
  assert.deepEqual(
    cards.map((card) => card.label),
    ["Applied to 1 clip", "AI agent unavailable"],
  );
  assert.equal(isFinishedTurnCard(cards.at(-1)), true);
});

test("conversationCard counts agent-unavailable sections when locating a turn card", () => {
  const document = conversationDocument();
  const window = document.defaultView!;
  const card = document.evaluate(conversationCard(2), document, null, window.XPathResult.FIRST_ORDERED_NODE_TYPE, null).singleNodeValue as Element;
  assert.equal(card?.getAttribute("aria-label"), "AI agent unavailable");
});

test("isFinishedTurnCard accepts applied, review and failure cards but not in-progress ones", () => {
  for (const label of ["Applied to 2 clips", "Needs your review", "Couldn't validate the edit", "Nothing was changed", "The edit didn't finish", "AI agent unavailable"]) {
    assert.equal(isFinishedTurnCard({ label }), true, label);
  }
  for (const card of [undefined, { label: null }, { label: "Undone" }, { label: "Agent unavailable" }]) {
    assert.equal(isFinishedTurnCard(card), false, JSON.stringify(card));
  }
});

test("agentBackendPlan pins the Codex backend without naming a Claude model", () => {
  const plan = agentBackendPlan("codex", "haiku");
  assert.equal(plan.componentId, "agent.codex");
  assert.deepEqual(plan.patch, { agentBackend: "codex" });
  assert.deepEqual(plan.restorePatch, { agentBackend: "automatic" });
});

test("agentBackendPlan pins the Claude backend and its model", () => {
  const plan = agentBackendPlan("claude", "haiku");
  assert.equal(plan.componentId, "agent.claude");
  assert.deepEqual(plan.patch, { agentBackend: "claude", claudeModel: "haiku" });
  assert.deepEqual(plan.restorePatch, { agentBackend: "automatic" });
});

test("agentBackendPlan refuses a backend it cannot drive", () => {
  assert.throws(() => agentBackendPlan("gemini", "haiku"), /unknown agent backend gemini/);
});

function selfTestResults(states: Record<string, string>) {
  return Object.fromEntries(agentSelfTestComponentIds.map((id) => [id, { state: states[id] ?? "succeeded" }]));
}

test("the agent self-test step exercises both support rows and both backends", () => {
  assert.deepEqual(requiredAgentComponentIds, ["agent.mcpServer", "agent.proposalValidator"]);
  assert.deepEqual(agentBackendComponentIds, ["agent.codex", "agent.claude"]);
  assert.deepEqual(agentSelfTestComponentIds, ["agent.mcpServer", "agent.proposalValidator", "agent.codex", "agent.claude"]);
});

test("agentSelfTestVerdict passes when only Claude is ready", () => {
  const verdict = agentSelfTestVerdict(selfTestResults({ "agent.codex": "failed" }));
  assert.equal(verdict.ok, true);
  assert.equal(verdict.reason, null);
  assert.deepEqual(verdict.readyBackends, ["agent.claude"]);
  assert.deepEqual(verdict.unavailableBackends, ["agent.codex"]);
});

test("agentSelfTestVerdict passes when only Codex is ready", () => {
  const verdict = agentSelfTestVerdict(selfTestResults({ "agent.claude": "failed" }));
  assert.equal(verdict.ok, true);
  assert.deepEqual(verdict.readyBackends, ["agent.codex"]);
});

test("agentSelfTestVerdict fails when neither backend is ready", () => {
  const verdict = agentSelfTestVerdict(selfTestResults({ "agent.codex": "failed", "agent.claude": "failed" }));
  assert.equal(verdict.ok, false);
  assert.match(verdict.reason ?? "", /no conversation-turn backend is ready: agent.codex, agent.claude/);
});

test("agentSelfTestVerdict fails when a support row fails even with both backends ready", () => {
  const verdict = agentSelfTestVerdict(selfTestResults({ "agent.proposalValidator": "failed" }));
  assert.equal(verdict.ok, false);
  assert.match(verdict.reason ?? "", /required agent components failed: agent.proposalValidator/);
});

test("agentSelfTestVerdict treats a missing result as not ready", () => {
  const verdict = agentSelfTestVerdict({ "agent.mcpServer": { state: "succeeded" }, "agent.proposalValidator": { state: "succeeded" } });
  assert.equal(verdict.ok, false);
  assert.match(verdict.reason ?? "", /no conversation-turn backend is ready/);
});
