import { describe, expect, it } from "vitest";
import type { ProjectJobSummary, VideoProject } from "@/lib/project";
import { fixtureProject } from "@/test-utils/editor-fixtures";
import {
  allCaptionsPresetActions,
  captionBuildRange,
  captionSourceOptions,
  defaultCaptionSourceId,
  sharedCaptionPreset,
  sourceSecondsAtPlayhead,
  timelineSecondsForSource,
  transcriptCaptionItems,
  transcriptionState,
} from "./captions-model";

function job(overrides: Partial<ProjectJobSummary>): ProjectJobSummary {
  return { id: "transcribe-media-1-abc", kind: "transcribe_media", status: "running", updatedAt: "2026-09-14T10:00:00.000Z", ...overrides };
}

function withoutTranscripts(): VideoProject {
  return { ...fixtureProject(), transcripts: [] };
}

/** The sample project with the video clip trimmed to source 10–14 s and moved to 2 s on the timeline. */
function offsetClipProject(): VideoProject {
  const project = fixtureProject();
  project.timeline.tracks = project.timeline.tracks.map((track) =>
    track.kind === "video"
      ? {
          ...track,
          items: track.items
            .filter((item) => item.id === "item-1")
            .map((item) => ({ ...item, startSeconds: 2, durationSeconds: 4, properties: { ...item.properties, sourceIn: 10, sourceOut: 14 } })),
        }
      : track,
  );
  return project;
}

describe("captions model", () => {
  it("offers video and audio sources by display name", () => {
    const project = fixtureProject();
    const options = captionSourceOptions(project);
    expect(options.map((option) => option.value)).toEqual(expect.arrayContaining(["media-1", "media-voiceover"]));
    expect(options.every((option) => !option.label.startsWith("media-"))).toBe(true);
  });

  it("defaults the source to the selected caption's transcript, then the selected clip, then a transcribed source", () => {
    const project = fixtureProject();
    expect(defaultCaptionSourceId(project, ["caption-2"])).toBe("media-1");
    expect(defaultCaptionSourceId(project, ["music-bed"])).toBe("media-voiceover");
    expect(defaultCaptionSourceId(project, [])).toBe("media-1");
    expect(defaultCaptionSourceId({ ...project, media: [] }, [])).toBeNull();
  });

  it("reads the latest transcription job for the source", () => {
    const project = withoutTranscripts();
    expect(transcriptionState(project, "media-1")).toBe("idle");
    project.jobs = [
      job({ status: "failed", updatedAt: "2026-09-14T09:00:00.000Z" }),
      job({ id: "transcribe-media-1-def", status: "running", startRequest: null }),
      job({ id: "transcribe-media-voiceover-1", status: "failed" }),
    ];
    expect(transcriptionState(project, "media-1")).toBe("transcribing");
    expect(transcriptionState(project, "media-voiceover")).toBe("failed");
    project.jobs = [job({ status: "completed" })];
    expect(transcriptionState(project, "media-1")).toBe("idle");
  });

  it("matches jobs by the start request's media id before the job id prefix", () => {
    const project = withoutTranscripts();
    const startRequest = { workflowId: "wf", workflowType: "t", taskQueue: "q", input: { mediaId: "media-1-extra" }, searchAttributes: {}, activityTypes: [], idReusePolicy: "x" };
    project.jobs = [job({ id: "transcribe-media-1-extra-abc", startRequest })];
    expect(transcriptionState(project, "media-1")).toBe("idle");
    expect(transcriptionState(project, "media-1-extra")).toBe("transcribing");
  });

  it("builds from the selected or first clip of the source, else the whole transcript", () => {
    const project = offsetClipProject();
    const transcript = project.transcripts[0];
    if (!transcript) throw new Error("fixture transcript");
    expect(captionBuildRange(project, "media-1", transcript, [])).toEqual({
      startSeconds: 10,
      endSeconds: 14,
      timelineStartSeconds: 2,
      timelineSecondsPerSourceSecond: 1,
    });
    const unplaced = { ...project, timeline: { ...project.timeline, tracks: project.timeline.tracks.map((track) => ({ ...track, items: [] })) } };
    expect(captionBuildRange(unplaced, "media-1", transcript, [])).toEqual({ startSeconds: 0.65, endSeconds: 3.35 });
    expect(captionBuildRange(unplaced, "media-1", { ...transcript, words: [] }, [])).toBeNull();
  });

  it("maps transcript time through reversed clips and never builds captions from them", () => {
    const project = offsetClipProject();
    const transcript = project.transcripts[0];
    if (!transcript) throw new Error("fixture transcript");
    const tracks = project.timeline.tracks.map((track) =>
      track.kind === "video" ? { ...track, items: track.items.map((item) => ({ ...item, properties: { ...item.properties, reverse: true } })) } : track,
    );
    const reversed = { ...project, timeline: { ...project.timeline, tracks } };
    // Source 10-14 plays backwards over timeline 2-6.
    expect(sourceSecondsAtPlayhead(reversed, "media-1", 3)).toBe(13);
    expect(timelineSecondsForSource(reversed, "media-1", 12.5)).toBe(3.5);
    // Its words would play backwards, so a reversed clip is not a caption build range.
    expect(captionBuildRange(reversed, "media-1", transcript, ["item-1"])).toBeNull();

    const withForward = {
      ...reversed,
      timeline: {
        ...reversed.timeline,
        tracks: tracks.map((track) =>
          track.kind === "video"
            ? { ...track, items: [...track.items, ...track.items.map((item) => ({ ...item, id: "item-forward", startSeconds: 8, properties: { sourceIn: 10, sourceOut: 14 } }))] }
            : track,
        ),
      },
    };
    expect(captionBuildRange(withForward, "media-1", transcript, ["item-1"])).toEqual({
      startSeconds: 10,
      endSeconds: 14,
      timelineStartSeconds: 8,
      timelineSecondsPerSourceSecond: 1,
    });
  });

  it("maps the playhead to source time through the clip and back", () => {
    const project = offsetClipProject();
    expect(sourceSecondsAtPlayhead(project, "media-1", 3)).toBe(11);
    expect(sourceSecondsAtPlayhead(project, "media-1", 1)).toBeNull();
    expect(timelineSecondsForSource(project, "media-1", 12.5)).toBe(4.5);
    // Unplaced sources use transcript time directly (legacy transcript seek).
    expect(sourceSecondsAtPlayhead(project, "media-voiceover-missing", 3)).toBe(3);
    expect(timelineSecondsForSource(project, "media-1", 20)).toBe(20);
  });

  it("lists the caption cues built from a transcript", () => {
    const project = fixtureProject();
    expect(transcriptCaptionItems(project, "transcript-media-1").map((item) => item.id)).toEqual(["caption-1", "caption-2"]);
    expect(transcriptCaptionItems(project, "other")).toEqual([]);
  });

  it("applies a style preset to every unlocked caption in one update", () => {
    const project = fixtureProject();
    expect(sharedCaptionPreset(project)).toBe("boldReadableLower");
    const result = allCaptionsPresetActions(project, "kineticFocus");
    if ("blocked" in result) throw new Error(result.blocked);
    expect(result.actions).toHaveLength(1);
    const [action] = result.actions;
    expect(action?.type).toBe("updateItemProperties");
    if (action?.type !== "updateItemProperties") return;
    expect(action.updates.map((update) => update.itemId)).toEqual(["caption-1", "caption-2"]);
    expect(action.updates[0]?.set).toMatchObject({ stylePreset: "kineticFocus", motionPresetId: "pulse-emphasis-v2" });

    const locked = fixtureProject();
    locked.timeline.tracks = locked.timeline.tracks.map((track) => (track.kind === "caption" ? { ...track, locked: true } : track));
    expect(allCaptionsPresetActions(locked, "kineticFocus")).toEqual({ blocked: "There are no unlocked captions to style." });
  });
});
