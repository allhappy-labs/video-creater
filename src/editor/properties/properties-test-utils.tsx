import { act, fireEvent, screen } from "@testing-library/react";
import { vi } from "vitest";
import type { ProjectAction, VideoProject } from "@/lib/project";
import type { TimelineItem, TimelineItemKind, TimelineTrack, TrackKind } from "@/lib/timeline";
import { renderWithEditorStore } from "@/test-utils/editor-render";
import { fixtureProject } from "@/test-utils/editor-fixtures";
import { PropertiesPanel } from "./properties-panel";

export function testItem(
  id: string,
  kind: TimelineItemKind,
  properties: Record<string, unknown> = {},
  mediaId = kind === "audio_clip" ? "media-voiceover" : "media-1",
): TimelineItem {
  return { id, kind, startSeconds: 0, durationSeconds: 4, source: { type: "media", mediaId }, label: id, properties };
}

export function track(id: string, kind: TrackKind, items: TimelineItem[], locked = false): TimelineTrack {
  return { id, name: id, kind, locked, enabled: true, items };
}

/** A project with one video track and one audio track holding the given items. */
export function testProject(visual: TimelineItem[], audio: TimelineItem[] = [], patch: Partial<VideoProject> = {}): VideoProject {
  return {
    ...fixtureProject(),
    timeline: { durationSeconds: 4, tracks: [track("video", "video", visual), track("audio", "audio", audio)] },
    ...patch,
  };
}

/** A project holding exactly the given tracks. */
export function projectWithTracks(tracks: TimelineTrack[], patch: Partial<VideoProject> = {}): VideoProject {
  return { ...fixtureProject(), timeline: { durationSeconds: 20, tracks }, ...patch };
}

interface RenderOptions {
  readonly projectDir?: string;
  /** Replace applyActions with a spy (default); false keeps the real store action for undo checks. */
  readonly spyApply?: boolean;
  readonly playheadSeconds?: number;
}

/** Renders the Properties panel with `selected` items and opens `tab` when given. */
export async function renderProperties(project: VideoProject, selected: readonly string[], tab?: string, options: RenderOptions = {}) {
  const rendered = renderWithEditorStore(<PropertiesPanel />, {
    project,
    ...(options.projectDir === undefined ? {} : { projectDir: options.projectDir }),
  });
  const applyActions = vi.fn(async (_actions: readonly ProjectAction[]): Promise<VideoProject | null> => null);
  const setLastError = vi.fn();
  act(() => {
    if (options.spyApply !== false) rendered.store.setState({ applyActions });
    rendered.store.setState({ setLastError, playheadSeconds: options.playheadSeconds ?? 0 });
    rendered.store.getState().selectItems(selected);
  });
  if (tab) fireEvent.mouseDown(screen.getByRole("tab", { name: tab }), { button: 0 });
  // Let the session effect catalog settle.
  await act(async () => undefined);
  return { ...rendered, applyActions, setLastError };
}

/** Types into a slider field's numeric input, presses Enter and lets the commit settle. */
export async function enterValue(label: string, value: string): Promise<void> {
  const input = screen.getByRole("textbox", { name: label });
  await act(async () => {
    fireEvent.change(input, { target: { value } });
    fireEvent.keyDown(input, { key: "Enter" });
  });
}
