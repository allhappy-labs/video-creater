import type { ProjectActionKeyframeProperty } from "@/lib/project";

const editorTabIds = ["ai", "media", "audio", "text", "captions", "effects"] as const;
export type EditorTabId = (typeof editorTabIds)[number];

interface EditorLayout {
  readonly activeTab: EditorTabId;
  readonly leftWidth: number;
}

export type TimelineTool = "select" | "blade";

const timelineTools: readonly TimelineTool[] = ["select", "blade"];

const laneProperties: readonly ProjectActionKeyframeProperty[] = [
  "opacity",
  "volumeDb",
  "positionX",
  "positionY",
  "scale",
  "scaleX",
  "scaleY",
  "rotationDegrees",
  "cropTop",
  "cropRight",
  "cropBottom",
  "cropLeft",
];

interface TimelineViewPreferences {
  readonly splitHeight: number;
  readonly zoomPercent: number;
  readonly snapEnabled: boolean;
  readonly keyframesVisible: boolean;
  readonly tool: TimelineTool;
  readonly laneProperty: ProjectActionKeyframeProperty;
}

export const leftWidthBounds = { min: 300, max: 440, default: 360 } as const;
export const splitHeightBounds = { min: 160, default: 300 } as const;
export const zoomBounds = { min: 10, max: 1000, default: 100 } as const;

const layoutKey = "video-creater.editor.v2.layout";
const timelineViewKey = (projectDir: string) => `video-creater.editor.v2.timeline-view:${projectDir}`;

export function clamp(value: number, min: number, max: number): number {
  if (!Number.isFinite(value)) return min;
  return Math.min(max, Math.max(min, value));
}

function readJson(key: string): Record<string, unknown> | null {
  try {
    const raw = window.localStorage.getItem(key);
    if (!raw) return null;
    const parsed: unknown = JSON.parse(raw);
    return parsed && typeof parsed === "object" ? (parsed as Record<string, unknown>) : null;
  } catch {
    return null;
  }
}

function writeJson(key: string, value: unknown): void {
  try {
    window.localStorage.setItem(key, JSON.stringify(value));
  } catch {
    // Storage full or unavailable: layout persistence is best-effort.
  }
}

function isOneOf<T extends string>(value: unknown, options: readonly T[]): value is T {
  return typeof value === "string" && (options as readonly string[]).includes(value);
}

function isTabId(value: unknown): value is EditorTabId {
  return isOneOf(value, editorTabIds);
}

export function loadEditorLayout(): EditorLayout {
  const stored = readJson(layoutKey);
  return {
    activeTab: isTabId(stored?.activeTab) ? stored.activeTab : "ai",
    leftWidth:
      typeof stored?.leftWidth === "number"
        ? clamp(stored.leftWidth, leftWidthBounds.min, leftWidthBounds.max)
        : leftWidthBounds.default,
  };
}

export function saveEditorLayout(layout: EditorLayout): void {
  writeJson(layoutKey, layout);
}

export function loadTimelineView(projectDir: string): TimelineViewPreferences {
  const stored = readJson(timelineViewKey(projectDir));
  return {
    splitHeight:
      typeof stored?.splitHeight === "number" && Number.isFinite(stored.splitHeight)
        ? Math.max(splitHeightBounds.min, stored.splitHeight)
        : splitHeightBounds.default,
    zoomPercent:
      typeof stored?.zoomPercent === "number" ? clamp(stored.zoomPercent, zoomBounds.min, zoomBounds.max) : zoomBounds.default,
    snapEnabled: typeof stored?.snapEnabled === "boolean" ? stored.snapEnabled : true,
    keyframesVisible: typeof stored?.keyframesVisible === "boolean" ? stored.keyframesVisible : false,
    tool: isOneOf(stored?.tool, timelineTools) ? stored.tool : "select",
    laneProperty: isOneOf(stored?.laneProperty, laneProperties) ? stored.laneProperty : "opacity",
  };
}

export function saveTimelineView(projectDir: string, view: TimelineViewPreferences): void {
  writeJson(timelineViewKey(projectDir), view);
}

const autoApplySafeKey = "video-creater.editor.v2.agent.autoApplySafe";

/** The AI tab "Auto-apply safe edits" preference; on unless the user turned it off. */
export function loadAgentAutoApplySafe(): boolean {
  try {
    return window.localStorage.getItem(autoApplySafeKey) !== "false";
  } catch {
    return true;
  }
}

export function saveAgentAutoApplySafe(value: boolean): void {
  try {
    window.localStorage.setItem(autoApplySafeKey, value ? "true" : "false");
  } catch {
    // Storage full or unavailable: the preference lasts for this session only.
  }
}
