import { applyProjectActionLocally } from "../../project";
import type {
  CodexConversationEditProposal,
  CodexConversationRange,
  CodexPreparedProposal,
  CodexProposalImpact,
  CodexProposalRisk,
  GeneratedAssetSettings,
  ProjectAction,
  ProjectValidationIssue,
  VideoProject,
} from "../../project";
import { defaultGenerationModels } from "../../generation/provider-rules";
import { buildFallbackGenerateMediaStartRequest } from "../../jobs/temporal-fallback";
import type { Timeline, TimelineItem } from "../../timeline";

/**
 * Deterministic conversation proposals for the DEV-only conversation fixture, prepared the way Rust
 * `prepare_codex_conversation_proposal` prepares a Codex proposal: the exact actions applied to a
 * copy (with the TypeScript mirror of the canonical action path), content-derived action ids, the
 * fail-closed risk allowlist from `codex/conversation/risk.rs`, and the impact rules from
 * `codex/conversation/impact.rs`. Nothing here mutates the supplied project.
 */

// ---- Risk (mirrors src-tauri/src/codex/conversation/risk.rs) ----

const safeActionTypes: ReadonlySet<string> = new Set([
  "addItems", "insertItems", "moveItems", "reorderItems", "resizeItems", "trimItems", "rippleTrimItem", "rippleDeleteRanges", "splitItems",
  "createTrack", "setTrackLocked", "reorderTrack", "setTrackSyncLocked", "setTrackEnabled",
  "editCaptionText", "editTextItem", "updateAudioFadeOut", "updateAudioFades", "updateAudioVolume", "updateAudioSync", "updateAudioClipSpeed", "detachAudio", "updateClipReverse",
  "updateVisualClipOpacity", "updateVisualClipTransform", "updateVisualClipCrop", "updateVisualClipFades", "updateVisualClipSpeed",
  "setItemKeyframes", "upsertItemKeyframe", "moveItemKeyframe", "deleteItemKeyframe",
  "upsertEffectParameterKeyframe", "moveEffectParameterKeyframe", "deleteEffectParameterKeyframe",
  "updateItemProperties", "updateItemEffects", "updateItemColorGrade",
  "linkItems", "unlinkItems", "applyCaptionRepair", "editTranscriptWords", "updateTemplateItems", "updateTextOverlayItems",
  "addTransition", "updateTransition", "removeTransition",
]);

type RiskReason = readonly [code: string, message: string];

const generatedAssets: RiskReason = ["changesGeneratedAssets", "Starts, changes, or places generated media."];
const jobs: RiskReason = ["changesJobs", "Starts or changes a background job or provider request."];
const renderMetadata: RiskReason = ["changesRenderMetadata", "Changes render or export records."];
const projectSettings: RiskReason = ["changesProjectSettings", "Changes project or render settings."];

const reviewReasons: Readonly<Record<string, RiskReason>> = {
  removeItems: ["deletesExistingItems", "Removes existing clips from the timeline."],
  removeTracks: ["removesTracks", "Removes whole tracks from the timeline."],
  deleteTimeline: ["deletesTimeline", "Deletes a timeline."],
  deleteMedia: ["deletesMedia", "Deletes media from the project library."],
  deleteMediaFolder: ["deletesMediaFolder", "Deletes a folder from the project library."],
  decomposeTimelineItem: ["decomposesItem", "Breaks a clip apart into separate layers."],
  recordGeneratedAsset: generatedAssets,
  updateGeneratedAssetStatus: generatedAssets,
  updateGeneratedAssetReferences: generatedAssets,
  completeGeneratedAsset: generatedAssets,
  replaceTimelineItemWithGeneratedOutput: generatedAssets,
  recordJob: jobs,
  updateJobStatus: jobs,
  updateJobProviderRequest: jobs,
  recordJobFailure: jobs,
  attachRenderReport: renderMetadata,
  recordExportArtifact: renderMetadata,
  updateProjectSettings: projectSettings,
  updateRenderSettings: projectSettings,
  updateTemplateOverride: ["changesTemplateOverride", "Changes a template for every place it is used."],
};

const unclassified: RiskReason = ["unclassifiedAction", "Includes a change that needs your review."];

/** One review-level action makes the bundle `review`; unknown actions fail closed. */
export function classifyFixtureRisk(actions: readonly ProjectAction[]): CodexProposalRisk {
  const reasons: CodexProposalRisk["reasons"] = [];
  for (const action of actions) {
    if (safeActionTypes.has(action.type)) continue;
    const [code, message] = reviewReasons[action.type] ?? unclassified;
    if (reasons.every((reason) => reason.code !== code)) reasons.push({ code, message });
  }
  return { level: reasons.length === 0 ? "safe" : "review", reasons };
}

// ---- Action ids ----

/** JSON with sorted object keys and undefined members dropped, so equal content compares equal. */
export function canonicalJson(value: unknown): string {
  if (Array.isArray(value)) return `[${value.map((entry) => (entry === undefined ? "null" : canonicalJson(entry))).join(",")}]`;
  if (value !== null && typeof value === "object") {
    const entries = Object.entries(value as Record<string, unknown>)
      .filter(([, entry]) => entry !== undefined)
      .sort(([left], [right]) => (left < right ? -1 : left > right ? 1 : 0));
    return `{${entries.map(([key, entry]) => `${JSON.stringify(key)}:${canonicalJson(entry)}`).join(",")}}`;
  }
  return JSON.stringify(value) ?? "null";
}

function digest(text: string): string {
  let hash = 0x811c9dc5;
  for (let index = 0; index < text.length; index += 1) {
    hash ^= text.charCodeAt(index);
    hash = Math.imul(hash, 0x01000193) >>> 0;
  }
  return hash.toString(16).padStart(8, "0");
}

/** Position plus a digest of the action, so re-preparing the same proposal yields the same ids. */
function fixtureActionIds(actions: readonly ProjectAction[]): string[] {
  return actions.map((action, index) => `codex-action-${(index + 1).toString()}-${digest(canonicalJson(action))}`);
}

// ---- Impact (mirrors src-tauri/src/codex/conversation/impact.rs) ----

const tolerance = 1e-6;
type Span = readonly [number, number];
interface Placed {
  readonly trackId: string;
  readonly enabled: boolean;
  readonly item: TimelineItem;
}

const near = (left: number, right: number) => Math.abs(left - right) <= tolerance;
const span = (item: TimelineItem): Span => [item.startSeconds, item.startSeconds + item.durationSeconds];

function placedItems(timeline: Timeline): Placed[] {
  return timeline.tracks.flatMap((track) => track.items.map((item) => ({ trackId: track.id, enabled: track.enabled !== false, item })));
}

function sourceMappingAnchor(item: TimelineItem): number | null {
  if (item.source.type !== "media") return null;
  const speed = typeof item.properties.speed === "number" ? item.properties.speed : 1;
  const sourceIn = typeof item.properties.sourceIn === "number" ? item.properties.sourceIn : 0;
  if (item.properties.reverse !== true) return sourceIn - item.startSeconds * speed;
  // A reversed clip reads backwards from `sourceOut`.
  const sourceOut = typeof item.properties.sourceOut === "number" ? item.properties.sourceOut : sourceIn + item.durationSeconds * speed;
  return sourceOut + item.startSeconds * speed;
}

function sameContent(before: TimelineItem, after: TimelineItem): boolean {
  const content = ({ properties, kind, source, label }: TimelineItem) => {
    const rest = Object.entries(properties).filter(([key]) => key !== "sourceIn" && key !== "sourceOut");
    return canonicalJson({ kind, source, label, rest: Object.fromEntries(rest) });
  };
  return content(before) === content(after);
}

function changedSpans(before: Placed, after: Placed): Span[] {
  const [old, next] = [span(before.item), span(after.item)];
  if (before.trackId !== after.trackId || before.enabled !== after.enabled) return [old, next];
  if (canonicalJson(before.item) === canonicalJson(after.item)) return [];
  const beforeAnchor = sourceMappingAnchor(before.item);
  const afterAnchor = sourceMappingAnchor(after.item);
  const sameMapping =
    beforeAnchor === null || afterAnchor === null ? beforeAnchor === afterAnchor && near(old[0], next[0]) : near(beforeAnchor, afterAnchor);
  if (!sameContent(before.item, after.item) || !sameMapping) return [old, next];
  if (!(Math.max(old[0], next[0]) < Math.min(old[1], next[1]) - tolerance)) return [old, next];
  const spans: Span[] = [];
  if (!near(old[0], next[0])) spans.push([Math.min(old[0], next[0]), Math.max(old[0], next[0])]);
  if (!near(old[1], next[1])) spans.push([Math.min(old[1], next[1]), Math.max(old[1], next[1])]);
  return spans;
}

function extentSeconds(timeline: Timeline): number {
  return timeline.tracks.flatMap((track) => track.items).reduce((end, item) => Math.max(end, item.startSeconds + item.durationSeconds), 0);
}

function normalizedRanges(spans: readonly Span[]): CodexConversationRange[] {
  const sorted = spans.filter(([start, end]) => Number.isFinite(start) && Number.isFinite(end) && end - start > 0).sort((left, right) => left[0] - right[0]);
  const merged: [number, number][] = [];
  for (const [start, end] of sorted) {
    const last = merged[merged.length - 1];
    if (last && start <= last[1] + tolerance) last[1] = Math.max(last[1], end);
    else merged.push([start, end]);
  }
  return merged.map(([startSeconds, endSeconds]) => ({ startSeconds, endSeconds }));
}

function impactSummary(added: number, changed: number, removed: number, before: number, after: number): string {
  const items = (count: number) => (count === 1 ? "1 item" : `${count.toString()} items`);
  const parts = ([["changes", changed], ["adds", added], ["removes", removed]] as const).filter(([, count]) => count > 0).map(([verb, count]) => `${verb} ${items(count)}`);
  const [first, ...rest] = parts;
  const edits = first ? [first.charAt(0).toUpperCase() + first.slice(1), ...rest].join(", ") : "No timeline items change";
  const duration = near(before, after) ? `the timeline stays ${before.toFixed(1)}s` : `the timeline goes from ${before.toFixed(1)}s to ${after.toFixed(1)}s`;
  return `${edits}; ${duration}.`;
}

function fixtureImpact(before: VideoProject, after: VideoProject): CodexProposalImpact {
  const beforeItems = placedItems(before.timeline);
  const afterItems = placedItems(after.timeline);
  const afterById = new Map(afterItems.map((placed) => [placed.item.id, placed]));
  const beforeIds = new Set(beforeItems.map((placed) => placed.item.id));
  const affectedItemIds: string[] = [];
  const spans: Span[] = [];
  let added = 0;
  let changed = 0;
  let removed = 0;
  for (const placed of beforeItems) {
    const next = afterById.get(placed.item.id);
    const itemSpans = next ? changedSpans(placed, next) : [span(placed.item)];
    if (!next) removed += 1;
    else if (itemSpans.length > 0) changed += 1;
    if (itemSpans.length === 0) continue;
    affectedItemIds.push(placed.item.id);
    spans.push(...itemSpans);
  }
  for (const placed of afterItems) {
    if (beforeIds.has(placed.item.id)) continue;
    added += 1;
    affectedItemIds.push(placed.item.id);
    spans.push(span(placed.item));
  }
  const beforeDurationSeconds = extentSeconds(before.timeline);
  const afterDurationSeconds = extentSeconds(after.timeline);
  const affectedRanges = normalizedRanges(spans);
  const fps = Number.isFinite(after.renderSettings.fps) && after.renderSettings.fps > 0 ? after.renderSettings.fps : 30;
  const latest = Math.max(0, afterDurationSeconds - 1 / fps);
  const first = affectedRanges[0]?.startSeconds ?? 0;
  return {
    summary: impactSummary(added, changed, removed, beforeDurationSeconds, afterDurationSeconds),
    beforeDurationSeconds,
    afterDurationSeconds,
    affectedItemIds,
    affectedRanges,
    previewTimestamp: Math.min(Math.max(first, 0), latest),
  };
}

// ---- Preparation ----

export class FixtureProposalError extends Error {}

/** Applies the exact actions to a copy; an empty proposal or an action that changes nothing is invalid. */
export function prepareFixtureProposal(project: VideoProject, proposal: CodexConversationEditProposal): { prepared: CodexPreparedProposal; after: VideoProject } {
  if (!proposal.summary.trim()) throw new FixtureProposalError("the proposal summary is empty");
  const actions = proposal.projectActions;
  if (actions.length === 0) throw new FixtureProposalError("the proposal has no project actions");
  let after = project;
  actions.forEach((action, index) => {
    const next = applyProjectActionLocally(after, action);
    if (canonicalJson(next) === canonicalJson(after)) throw new FixtureProposalError(`action ${(index + 1).toString()} doesn't change the project`);
    after = next;
  });
  return {
    prepared: { actions: [...actions], actionIds: fixtureActionIds(actions), risk: classifyFixtureRisk(actions), impact: fixtureImpact(project, after) },
    after,
  };
}

// ---- Keyword routing ----

export type FixtureTurnKind = "tighten" | "generate" | "fail" | "caption";

/** "fail" wins over "generate", which wins over "tighten"; anything else is a caption edit. */
export function fixtureTurnKind(prompt: string): FixtureTurnKind {
  const text = prompt.toLowerCase();
  if (text.includes("fail")) return "fail";
  if (text.includes("generate")) return "generate";
  if (text.includes("tighten")) return "tighten";
  return "caption";
}

const noRenderReview = null;

function videoTrack(project: VideoProject) {
  return project.timeline.tracks.find((track) => track.kind === "video" && track.items.length > 0) ?? null;
}

/** Splits the last video clip one second before its end and ripple-deletes the two seconds before that. */
function tightenProposal(project: VideoProject): CodexConversationEditProposal | null {
  const track = videoTrack(project);
  const clip = track?.items.reduce((last, item) => (item.startSeconds > last.startSeconds ? item : last));
  if (!track || !clip || clip.durationSeconds < 3) return null;
  const end = clip.startSeconds + clip.durationSeconds;
  const splitSeconds = Math.round((end - 1) * 1000) / 1000;
  const cutStart = Math.max(clip.startSeconds + 0.5, splitSeconds - 2);
  // Every unlocked track with clips closes the gap, so captions and audio stay in sync.
  const trackIds = project.timeline.tracks.filter((candidate) => !candidate.locked && candidate.items.length > 0).map((candidate) => candidate.id);
  return {
    summary: "This tightens the pacing by cutting a slow stretch near the end and closing the gap.",
    edl: [],
    projectActions: [
      { type: "splitItems", splits: [{ itemId: clip.id, newItemId: `${clip.id}-tail`, splitSeconds }] },
      { type: "rippleDeleteRanges", ranges: [{ startSeconds: cutStart, endSeconds: splitSeconds, trackIds }] },
    ],
    renderReview: noRenderReview,
  };
}

const labShots = ["Lab bench wide shot", "Beaker pour close-up", "Microscope focus pull"] as const;

function labShotSettings(timelineStartSeconds: number | null): GeneratedAssetSettings {
  return { width: 1920, height: 1080, durationSeconds: 4, fps: 24, aspectRatio: "16:9", ...(timelineStartSeconds === null ? {} : { timelineStartSeconds }) };
}

/**
 * Three queued video generations, each recorded as Rust `generation_record_actions` records them: the
 * asset, then a queued `job-<assetId>` job carrying its start request. The first is placed on the
 * timeline through a placeholder clip.
 */
function generateProposal(project: VideoProject, projectDir: string): CodexConversationEditProposal {
  const track = videoTrack(project) ?? project.timeline.tracks.find((candidate) => candidate.kind === "video");
  const start = track ? track.items.reduce((end, item) => Math.max(end, item.startSeconds + item.durationSeconds), 0) : 0;
  const createdAt = "2026-09-15T10:00:00.000Z";
  const actions: ProjectAction[] = labShots.flatMap((name, index): ProjectAction[] => {
    const assetId = `agent-lab-shot-${(index + 1).toString()}`;
    const jobId = `job-${assetId}`;
    const brief = {
      name,
      targetFolderId: null,
      placementIntent: index === 0 && track ? ("timeline" as const) : ("library" as const),
      prompt: `${name} in a bright modern laboratory, handheld documentary look`,
      model: { ...defaultGenerationModels.video },
      references: { mediaIds: [] as string[], firstFrameMediaId: null, lastFrameMediaId: null },
      settings: labShotSettings(index === 0 && track ? start : null),
    };
    const startRequest = buildFallbackGenerateMediaStartRequest({ projectId: project.id, projectDir, assetId, jobId, mockMode: false, ...brief });
    return [
      { type: "recordGeneratedAsset", asset: { id: assetId, kind: "video", status: "queued", ...brief, outputs: [], createdAt, parentAssetId: null, retryOfAssetId: null } },
      { type: "recordJob", job: { id: jobId, kind: "generate_media", status: "queued", updatedAt: createdAt, startRequest } },
    ];
  });
  const [firstName] = labShots;
  if (track) {
    const assetId = "agent-lab-shot-1";
    actions.push({
      type: "addItems",
      targetTrackId: track.id,
      items: [
        {
          id: `generated-placeholder-${assetId}`,
          kind: "video_clip",
          startSeconds: start,
          durationSeconds: 4,
          source: { type: "generated", artifactId: assetId },
          label: firstName,
          properties: { generatedAssetId: assetId, generatedTimelinePlaceholder: true, sourceIn: 0, sourceOut: 4 },
        },
      ],
    });
  }
  return { summary: "This generates three lab shots and places the first one at the end of the timeline.", edl: [], projectActions: actions, renderReview: noRenderReview };
}

/** Fixes the first caption's text, or fades in the first visual clip when the project has no captions. */
function captionProposal(project: VideoProject): CodexConversationEditProposal | null {
  const caption = project.timeline.tracks.flatMap((track) => track.items).find((item) => item.kind === "caption" && item.source.type === "text");
  if (caption && caption.source.type === "text") {
    const words = caption.source.text.trim().replace(/[.!?]+$/, "");
    const text = `${words.charAt(0).toUpperCase()}${words.slice(1)}.`;
    return {
      summary: "This cleans up the first caption so it reads as a full sentence.",
      edl: [],
      projectActions: [{ type: "editCaptionText", itemId: caption.id, text: text === caption.source.text ? `${words}!` : text }],
      renderReview: noRenderReview,
    };
  }
  const clip = videoTrack(project)?.items[0];
  if (!clip) return null;
  return {
    summary: "This adds a short fade-in to the opening clip.",
    edl: [],
    projectActions: [{ type: "updateVisualClipFades", itemId: clip.id, fadeInSeconds: 0.5, fadeOutSeconds: 0 }],
    renderReview: noRenderReview,
  };
}

const failIssues: ProjectValidationIssue[] = [
  {
    path: "proposal.projectActions[0].ranges[0].endSeconds",
    message: "The proposed cut ends after the end of the timeline.",
    fix: "Ask for a cut that stays inside the timeline.",
  },
];

export interface FixtureTurn {
  readonly proposal: CodexConversationEditProposal | null;
  readonly prepared: CodexPreparedProposal | null;
  readonly issues: ProjectValidationIssue[] | null;
}

/** The fixture agent's answer to a prompt, prepared against `project` in `projectDir`. */
export function fixtureTurn(project: VideoProject, prompt: string, projectDir = ""): FixtureTurn {
  const kind = fixtureTurnKind(prompt);
  if (kind === "fail") {
    const proposal: CodexConversationEditProposal = {
      summary: "This cuts the long pause after the last line.",
      edl: [],
      projectActions: [{ type: "rippleDeleteRanges", ranges: [{ startSeconds: project.timeline.durationSeconds + 2, endSeconds: project.timeline.durationSeconds + 6, trackIds: [] }] }],
      renderReview: noRenderReview,
    };
    return { proposal, prepared: null, issues: failIssues.map((issue) => ({ ...issue })) };
  }
  const proposal = kind === "tighten" ? tightenProposal(project) : kind === "generate" ? generateProposal(project, projectDir) : captionProposal(project);
  if (!proposal) return { proposal: null, prepared: null, issues: null };
  try {
    return { proposal, prepared: prepareFixtureProposal(project, proposal).prepared, issues: null };
  } catch (error) {
    const message = error instanceof Error ? error.message : String(error);
    return { proposal, prepared: null, issues: [{ path: "proposal.projectActions", message: `The edit doesn't fit this project: ${message}.`, fix: "Ask for the edit again." }] };
  }
}
