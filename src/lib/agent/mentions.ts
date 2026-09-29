import { clockLabel } from "@/lib/agent/result-facts";
import { agentMentionTargets } from "@/lib/agent/selection-context";
import { mediaDisplayName } from "@/lib/media/names";
import type { CodexConversationFocus, VideoProject } from "@/lib/project";

/**
 * Composer mentions. The draft only ever shows `@<human name>` (quoted as `@"multi word"` when
 * needed); ids exist solely in the resolved request focus.
 */

export interface MentionTarget {
  readonly id: string;
  readonly kind: "media" | "timelineItem";
  /** Human name shown in the picker and the draft. Unique per target where possible. */
  readonly name: string;
}

export interface ParsedMention {
  readonly name: string;
  /** Index of the `@`. */
  readonly start: number;
  /** Index just past the mention token (including a closing quote). */
  readonly end: number;
  readonly quoted: boolean;
}

type MentionFocus = Pick<CodexConversationFocus, "mediaIds" | "timelineItemIds">;

export interface ResolvedMentions {
  readonly focus: MentionFocus;
  /** Mentioned names that match no target, first spelling kept, each once. */
  readonly unresolved: string[];
}

const mentionPattern = /(^|[\s([{])@(?:"([^"\n]*)"|([^\s"@]+))/g;
const activeMentionPattern = /(^|[\s([{])@("[^"\n]*|[^\s"@]*)$/;
const trailingPunctuation = /[.,;:!?)\]}]+$/;

function normalizedName(name: string): string {
  return name.replace(/"/g, "'").trim().toLowerCase();
}

export function parseMentions(draft: string): ParsedMention[] {
  const mentions: ParsedMention[] = [];
  for (const match of draft.matchAll(mentionPattern)) {
    const start = (match.index ?? 0) + (match[1] ?? "").length;
    const quotedName = match[2];
    if (quotedName !== undefined) {
      const name = quotedName.trim();
      if (name) mentions.push({ name, start, end: start + quotedName.length + 3, quoted: true });
      continue;
    }
    const name = (match[3] ?? "").replace(trailingPunctuation, "");
    if (name) mentions.push({ name, start, end: start + name.length + 1, quoted: false });
  }
  return mentions;
}

export function resolveMentions(draft: string, targets: readonly MentionTarget[]): ResolvedMentions {
  const byName = new Map<string, MentionTarget[]>();
  for (const target of targets) {
    const key = normalizedName(target.name);
    byName.set(key, [...(byName.get(key) ?? []), target]);
  }

  const mediaIds: string[] = [];
  const timelineItemIds: string[] = [];
  const unresolved: string[] = [];
  const unresolvedKeys = new Set<string>();
  const addUnique = (ids: string[], id: string) => {
    if (!ids.includes(id)) ids.push(id);
  };

  for (const mention of parseMentions(draft)) {
    const key = normalizedName(mention.name);
    const matches = byName.get(key);
    if (!matches) {
      if (!unresolvedKeys.has(key)) {
        unresolvedKeys.add(key);
        unresolved.push(mention.name);
      }
      continue;
    }
    for (const target of matches) addUnique(target.kind === "media" ? mediaIds : timelineItemIds, target.id);
  }

  return { focus: { mediaIds, timelineItemIds }, unresolved };
}

/** The `@query` being typed right before the caret, for opening and filtering the picker. */
export function activeMentionQuery(draft: string, caret: number): { start: number; query: string } | null {
  const match = activeMentionPattern.exec(draft.slice(0, caret));
  if (!match) return null;
  const token = match[2] ?? "";
  return {
    start: match.index + (match[1] ?? "").length,
    query: token.startsWith('"') ? token.slice(1) : token,
  };
}

/** Serializes a target as a draft token, quoting names that would not parse back bare. */
function mentionToken(name: string): string {
  const safeName = name.replace(/"/g, "'").trim();
  const bareRoundTrips = /^[^\s"@]+$/.test(safeName) && !trailingPunctuation.test(safeName);
  return bareRoundTrips ? `@${safeName}` : `@"${safeName}"`;
}

/**
 * Inserts `target` at the caret, replacing an active `@query` when one is being typed.
 * Returns the new draft and a caret placed after the mention and its trailing space.
 */
export function insertMention(
  draft: string,
  caret: number,
  target: MentionTarget,
): { draft: string; caret: number } {
  const boundedCaret = Math.max(0, Math.min(caret, draft.length));
  const active = activeMentionQuery(draft, boundedCaret);
  const before = draft.slice(0, active ? active.start : boundedCaret);
  const after = draft.slice(boundedCaret);
  const prefix = before.length === 0 || /\s$/.test(before) ? "" : " ";
  const token = `${prefix}${mentionToken(target.name)}`;
  const suffix = /^\s/.test(after) ? after : ` ${after}`;
  return { draft: `${before}${token}${suffix}`, caret: before.length + token.length + 1 };
}

/**
 * Mention targets for the picker: project media (generated outputs by their asset name) and the
 * active timeline's clips. Clip names that collide with another name get their start time.
 */
export function conversationMentionTargets(project: VideoProject): MentionTarget[] {
  const media: MentionTarget[] = agentMentionTargets(project)
    .filter((target) => target.label.trim())
    .map((target) => ({ id: target.mediaId, kind: "media", name: target.label.trim() }));

  const clips: { id: string; name: string; startSeconds: number }[] = [];
  for (const track of project.timeline.tracks) {
    for (const item of track.items) {
      const { source } = item;
      const mediaAsset = source.type === "media" ? project.media.find((asset) => asset.id === source.mediaId) : undefined;
      const name =
        item.label.trim() ||
        (mediaAsset ? mediaDisplayName(mediaAsset) : "") ||
        (source.type === "text" ? source.text.trim() : "");
      if (name) clips.push({ id: item.id, name, startSeconds: item.startSeconds });
    }
  }

  const nameCounts = new Map<string, number>();
  for (const { name } of [...media, ...clips]) {
    const key = normalizedName(name);
    nameCounts.set(key, (nameCounts.get(key) ?? 0) + 1);
  }

  return [
    ...media,
    ...clips.map(({ id, name, startSeconds }): MentionTarget => ({
      id,
      kind: "timelineItem",
      name: (nameCounts.get(normalizedName(name)) ?? 0) > 1 ? `${name} (${clockLabel(startSeconds)})` : name,
    })),
  ];
}
