import { describe, expect, it } from "vitest";
import { applyProjectActionLocally, type VideoProject } from "@/lib/project";
import {
  clip,
  cutProject,
  projectWithCrossfade,
  trackOf,
  transitionSampleProject,
  VIDEO_TRACK,
  withSourceRange,
  withTracks,
  withVideoItem,
} from "@/lib/timeline-ops/transition-fixtures";
import {
  cutAdjacentToItem,
  cutNearTime,
  locateTransition,
  planAddTransition,
  planRemoveTransition,
  planTransitionDuration,
  planTransitionKind,
  transitionAddTarget,
  transitionName,
} from "@/lib/timeline-ops/transition-commands";
import { cutsOnTrack } from "@/lib/timeline-ops/transitions";

function onlyCut(project: VideoProject) {
  const [cut] = cutsOnTrack(trackOf(project, VIDEO_TRACK), 1 / 24);
  if (!cut) throw new Error("Expected a cut");
  return cut;
}

const noSelection = { selectedTransitionId: null, selectedItemIds: [], selectedTrackId: null, playheadSeconds: 0 };

describe("planAddTransition", () => {
  it("adds a half-second transition with a derived id", () => {
    const project = cutProject();
    const plan = planAddTransition(project, onlyCut(project), "crossfade");
    expect(plan).toEqual({
      actions: [
        {
          type: "addTransition",
          trackId: VIDEO_TRACK,
          transition: { id: "transition-clip-a-clip-b", leftItemId: "clip-a", rightItemId: "clip-b", kind: "crossfade", durationSeconds: 0.5 },
        },
      ],
      transitionId: "transition-clip-a-clip-b",
      shortenedMessage: null,
    });
  });

  it("shortens to the maximum when handles run out and says why", () => {
    // clip-a uses 7.8–11.8 s of the 12 s source: 0.2 s of tail handle allows 0.4 s.
    const project = withVideoItem(cutProject(), 0, (item) => withSourceRange(item, 7.8, 11.8));
    const plan = planAddTransition(project, onlyCut(project), "dipToBlack");
    if ("blocked" in plan) throw new Error(plan.blocked);
    expect(plan.shortenedMessage).toBe("Shortened to 0.4s — not enough unused media");
    expect(plan.actions[0]).toMatchObject({ transition: { kind: "dipToBlack", durationSeconds: 0.4 } });
    expect(applyProjectActionLocally(project, plan.actions[0]!)).not.toBe(project);
  });

  it("refuses a cut without a frame of unused media with the validation message", () => {
    const project = withVideoItem(cutProject(), 0, (item) => withSourceRange(item, 8, 12));
    expect(planAddTransition(project, onlyCut(project), "wipe")).toEqual({
      blocked: "Not enough unused media after Opening shot for a 0.5s transition. Maximum is 0.0s.",
    });
  });

  it("changes the type of a cut that already has a transition", () => {
    const project = projectWithCrossfade(1);
    const cut = onlyCut(project);
    expect(planAddTransition(project, cut, "wipe")).toEqual({
      actions: [{ type: "updateTransition", trackId: VIDEO_TRACK, transitionId: "fade-1", kind: "wipe" }],
      transitionId: "fade-1",
      shortenedMessage: null,
    });
  });

  it("refuses locked tracks", () => {
    const project = withTracks(cutProject(), (tracks) => tracks.map((track) => ({ ...track, locked: true })));
    expect(planAddTransition(project, onlyCut(project), "crossfade")).toEqual({ blocked: "Unlock the track to add a transition." });
  });
});

describe("transition targets", () => {
  const threeClips = () =>
    transitionSampleProject([clip("a", "A", 0, 2, 1), clip("b", "B", 2, 2, 3), clip("c", "C", 4, 2, 5)]);

  it("picks the selected transition's cut before the playhead", () => {
    const project = applyProjectActionLocally(threeClips(), {
      type: "addTransition",
      trackId: VIDEO_TRACK,
      transition: { id: "t", leftItemId: "b", rightItemId: "c", kind: "crossfade", durationSeconds: 0.5 },
    });
    expect(transitionAddTarget(project, { ...noSelection, selectedTransitionId: "t" })).toMatchObject({ leftItemId: "b", transitionId: "t" });
    expect(transitionAddTarget(project, { ...noSelection, playheadSeconds: 1.8 })).toMatchObject({ leftItemId: "a", transitionId: null });
  });

  it("uses the selected clip's track and returns null when it has no cut", () => {
    const project = withTracks(threeClips(), (tracks) =>
      tracks.map((track) => (track.kind === "audio" ? { ...track, items: [{ ...clip("m", "M", 0, 2, 0), kind: "audio_clip" }] } : track)),
    );
    expect(transitionAddTarget(project, { ...noSelection, selectedItemIds: ["m"] })).toBeNull();
    expect(transitionAddTarget(project, { ...noSelection, selectedItemIds: ["c"], playheadSeconds: 5 })).toMatchObject({ leftItemId: "b" });
  });

  it("finds a dropped tile's cut within 12 px at the current zoom", () => {
    const project = threeClips();
    const track = trackOf(project, VIDEO_TRACK);
    // 80 px/s: 12 px is 0.15 s.
    expect(cutNearTime(project, track, 2.14, 80)).toMatchObject({ leftItemId: "a", rightItemId: "b" });
    expect(cutNearTime(project, track, 2.2, 80)).toBeNull();
    expect(cutNearTime(project, track, 3.9, 80)).toMatchObject({ leftItemId: "b", rightItemId: "c" });
  });

  it("resolves the context-menu cut at the clicked clip edge", () => {
    const project = threeClips();
    expect(cutAdjacentToItem(project, "b", 2.3)).toMatchObject({ rightItemId: "b" });
    expect(cutAdjacentToItem(project, "b", 3.7)).toMatchObject({ leftItemId: "b" });
    expect(cutAdjacentToItem(project, "a", 0.1)).toMatchObject({ leftItemId: "a", rightItemId: "b" });
    const lonely = transitionSampleProject([clip("solo", "Solo", 0, 2, 1)]);
    expect(cutAdjacentToItem(lonely, "solo", 1)).toBeNull();
  });
});

describe("transition edits", () => {
  it("clamps durations to the cut's maximum and skips unchanged values", () => {
    const project = projectWithCrossfade(1);
    expect(planTransitionDuration(project, "fade-1", 9)).toEqual({
      actions: [{ type: "updateTransition", trackId: VIDEO_TRACK, transitionId: "fade-1", durationSeconds: 4 }],
    });
    expect(planTransitionDuration(project, "fade-1", 1)).toEqual({ actions: [] });
    expect(planTransitionDuration(project, "fade-1", 0)).toMatchObject({ actions: [{ durationSeconds: 1 / 24 }] });
  });

  it("changes the kind, removes, and names transitions", () => {
    const project = projectWithCrossfade(1);
    expect(planTransitionKind(project, "fade-1", "crossfade")).toEqual({ actions: [] });
    expect(planTransitionKind(project, "fade-1", "dipToWhite")).toMatchObject({ actions: [{ kind: "dipToWhite" }] });
    expect(planRemoveTransition(project, "fade-1")).toEqual({
      actions: [{ type: "removeTransition", trackId: VIDEO_TRACK, transitionId: "fade-1" }],
    });
    expect(planRemoveTransition(project, "missing")).toEqual({ blocked: "That transition no longer exists." });
    const located = locateTransition(project.timeline, "fade-1");
    expect(located && transitionName(located.transition)).toBe("Crossfade transition, 1.0s");
    expect(transitionName({ kind: "dipToBlack", durationSeconds: 0.25 })).toBe("Dip to black transition, 0.25s");
  });
});
