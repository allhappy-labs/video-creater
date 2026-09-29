import { describe, expect, it } from "vitest";
import { applyProjectActionLocally, type VideoProject } from "@/lib/project";
import {
  captionGroupItems,
  captionMotionPreset,
  captionPlacement,
  captionPresetActions,
  captionStyleActions,
  captionStylePreset,
  captionTextAction,
  captionWordAnimationPreset,
  captionWordAnimationPresetActions,
  captionWordAnimations,
  captionWordStaggerSeconds,
} from "@/lib/properties/caption-properties";
import { captionStyleProperties } from "@/lib/captions/caption-items";
import type { TimelineItem } from "@/lib/timeline";
import type { CommandResult } from "@/lib/timeline-ops/clip-commands";
import { fixtureItem, fixtureProject, fixtureTrack } from "@/test-utils/editor-fixtures";

function cue(id: string, properties: Record<string, unknown> = {}, text = "one two three"): TimelineItem {
  return {
    id,
    kind: "caption",
    startSeconds: 0,
    durationSeconds: 2,
    source: { type: "text", text },
    label: id,
    properties,
  };
}

function project(): VideoProject {
  const items = [
    cue("a1", { captionGroupId: "group-a" }),
    cue("a2", { captionGroupId: "group-a" }),
    cue("b1", { captionGroupId: "group-b" }),
    cue("loose"),
  ];
  return {
    ...fixtureProject(),
    timeline: {
      durationSeconds: 2,
      tracks: [{ id: "captions", name: "Captions", kind: "caption", locked: false, enabled: true, items }],
    },
  };
}

function actionsOf(result: CommandResult) {
  if ("blocked" in result) throw new Error(`Unexpectedly blocked: ${result.blocked}`);
  return result.actions;
}

function blockedOf(result: CommandResult) {
  if (!("blocked" in result)) throw new Error("Expected the edit to be blocked");
  return result.blocked;
}

describe("caption readers", () => {
  it("return defaults for missing or invalid values", () => {
    const item = cue("x", { captionPlacement: "left", stylePreset: 4, motionPresetId: "spin", captionWordStaggerSeconds: -1 });
    expect(captionPlacement(item)).toBe("lower");
    expect(captionStylePreset(item)).toBe("boldReadableLower");
    expect(captionMotionPreset(item)).toBe("snap-pop-v1");
    expect(captionWordAnimationPreset(item)).toBe("sequentialPop");
    expect(captionWordStaggerSeconds(item)).toBe(0.06);
  });

  it("read stored values", () => {
    const item = cue("x", {
      captionPlacement: "upper",
      stylePreset: "kineticFocus",
      motionPresetId: "soft-depth-card-v2",
      captionWordAnimationPreset: "karaokeFade",
      captionWordStaggerSeconds: 0.2,
    });
    expect(captionPlacement(item)).toBe("upper");
    expect(captionStylePreset(item)).toBe("kineticFocus");
    expect(captionMotionPreset(item)).toBe("soft-depth-card-v2");
    expect(captionWordAnimationPreset(item)).toBe("karaokeFade");
    expect(captionWordStaggerSeconds(item)).toBe(0.2);
  });
});

describe("captionGroupItems", () => {
  it("returns every caption sharing the group id", () => {
    expect(captionGroupItems(project(), "a2").map((item) => item.id)).toEqual(["a1", "a2"]);
  });

  it("returns just the cue without a group id, and nothing for unknown ids", () => {
    expect(captionGroupItems(project(), "loose").map((item) => item.id)).toEqual(["loose"]);
    expect(captionGroupItems(project(), "missing")).toEqual([]);
  });
});

describe("captionTextAction", () => {
  it("emits editCaptionText with the untrimmed text", () => {
    expect(actionsOf(captionTextAction(cue("a1"), " Fixed text ", project()))).toEqual([
      { type: "editCaptionText", itemId: "a1", text: " Fixed text " },
    ]);
  });

  it("edits a multi-word sample caption's own text instead of repairing its transcript word", () => {
    const sample = fixtureProject();
    const caption = fixtureItem(sample, "caption");
    const actions = actionsOf(captionTextAction(caption, "Restored voice", sample));
    expect(actions).toEqual([{ type: "editCaptionText", itemId: "caption-1", text: "Restored voice" }]);

    const next = actions.reduce(applyProjectActionLocally, sample);
    expect(fixtureItem(next, "caption")).toMatchObject({
      startSeconds: 0.65,
      durationSeconds: 1.35,
      source: { type: "text", text: "Restored voice" },
      properties: { textEdited: true, transcriptId: "transcript-media-1", wordIndex: 0 },
    });
    expect(next.transcripts).toEqual(sample.transcripts);
  });

  it("uses a caption repair when a single-word cue matches its transcript word", () => {
    const sample = fixtureProject();
    const caption = { ...fixtureItem(sample, "caption"), durationSeconds: 0.4, source: { type: "text" as const, text: "Original" } };
    fixtureTrack(sample, "caption").items[0] = caption;
    const actions = actionsOf(captionTextAction(caption, "Restored", sample));
    expect(actions).toMatchObject([
      {
        type: "applyCaptionRepair",
        repair: { captionItemId: "caption-1", transcriptId: "transcript-media-1", wordIndex: 0, text: "Restored", startSeconds: 0.65, endSeconds: 1.05 },
      },
    ]);

    const next = actions.reduce(applyProjectActionLocally, sample);
    expect(fixtureItem(next, "caption").source).toEqual({ type: "text", text: "Restored" });
    expect(next.transcripts[0]?.words[0]?.text).toBe("Restored");
  });

  it("rejects empty text and non-caption items", () => {
    expect(blockedOf(captionTextAction(cue("a1"), "  ", project()))).toMatch(/empty/i);
    expect(blockedOf(captionTextAction({ ...cue("a1"), kind: "overlay" }, "x", project()))).toMatch(/caption/i);
  });
});

describe("captionStyleActions", () => {
  it("updates every cue in the group for scope all, in one action", () => {
    expect(actionsOf(captionStyleActions(project(), "a1", "all", { captionPlacement: "center" }))).toEqual([
      {
        type: "updateItemProperties",
        updates: [
          { itemId: "a1", set: { captionPlacement: "center" }, remove: [] },
          { itemId: "a2", set: { captionPlacement: "center" }, remove: [] },
        ],
      },
    ]);
  });

  it("updates only the selected cue for scope this", () => {
    expect(actionsOf(captionStyleActions(project(), "a2", "this", { highlightColor: "#ffcf5a" }))).toEqual([
      { type: "updateItemProperties", updates: [{ itemId: "a2", set: { highlightColor: "#ffcf5a" }, remove: [] }] },
    ]);
  });

  it("turns null values into removals", () => {
    expect(actionsOf(captionStyleActions(project(), "loose", "all", { fontName: null }))).toEqual([
      { type: "updateItemProperties", updates: [{ itemId: "loose", set: {}, remove: ["fontName"] }] },
    ]);
  });

  it("applies style presets through captionStyleProperties", () => {
    expect(actionsOf(captionPresetActions(project(), "b1", "all", "centeredMinimal"))).toEqual([
      {
        type: "updateItemProperties",
        updates: [{ itemId: "b1", set: captionStyleProperties("centeredMinimal"), remove: [] }],
      },
    ]);
  });

  it("blocks unknown cues and empty edits", () => {
    expect(blockedOf(captionStyleActions(project(), "missing", "all", { captionPlacement: "upper" }))).toMatch(/caption/i);
    expect(blockedOf(captionStyleActions(project(), "a1", "all", {}))).toMatch(/change/i);
  });
});

describe("caption word animation presets", () => {
  const timed = cue("w", {
    emphasizedWordIndices: [0, 2, 9, "x"],
    captionWordTimings: [
      { wordIndex: 0, startSeconds: 0, endSeconds: 0.5 },
      { wordIndex: 2, startSeconds: 1, endSeconds: 2 },
    ],
  });

  it("builds sequentialPop animations with the legacy stagger math", () => {
    expect(captionWordAnimations(timed, "sequentialPop", 0.1)).toEqual([
      {
        wordIndex: 0,
        enterStartSeconds: 0,
        enterEndSeconds: 0.1,
        holdEndSeconds: 0.4,
        exitEndSeconds: 0.5,
        emphasisScale: 1.14,
        emphasisColor: "#ffcf5a",
        emphasisOpacity: 1,
        easing: "outBack",
      },
      {
        wordIndex: 2,
        enterStartSeconds: expect.closeTo(1.1, 9),
        enterEndSeconds: expect.closeTo(1.28, 9),
        holdEndSeconds: expect.closeTo(1.82, 9),
        exitEndSeconds: 2,
        emphasisScale: 1.14,
        emphasisColor: "#ffcf5a",
        emphasisOpacity: 1,
        easing: "outBack",
      },
    ]);
  });

  it("uses no offset for groupPulse and karaoke styling for karaokeFade", () => {
    const pulse = captionWordAnimations(timed, "groupPulse", 0.4);
    expect(pulse[1]).toMatchObject({ enterStartSeconds: 1, emphasisScale: 1.08, easing: "outQuad" });
    const karaoke = captionWordAnimations(timed, "karaokeFade", 0.4);
    expect(karaoke[1]).toMatchObject({
      enterStartSeconds: 1.25,
      emphasisScale: 1,
      emphasisColor: "#ffffff",
      emphasisOpacity: 0.82,
      easing: "outQuad",
    });
  });

  it("clamps the stagger and stores the preset per cue in scope", () => {
    const withTimed: VideoProject = {
      ...project(),
      timeline: {
        durationSeconds: 2,
        tracks: [
          {
            id: "captions",
            name: "Captions",
            kind: "caption",
            locked: false,
            enabled: true,
            items: [{ ...timed, properties: { ...timed.properties, captionGroupId: "g" } }, cue("other", { captionGroupId: "g" })],
          },
        ],
      },
    };
    const [action] = actionsOf(captionWordAnimationPresetActions(withTimed, "w", "all", "sequentialPop", 3));
    expect(action).toEqual({
      type: "updateItemProperties",
      updates: [
        {
          itemId: "w",
          set: {
            captionWordAnimations: captionWordAnimations(timed, "sequentialPop", 0.5),
            captionWordAnimationPreset: "sequentialPop",
            captionWordStaggerSeconds: 0.5,
          },
          remove: [],
        },
        {
          itemId: "other",
          set: { captionWordAnimations: [], captionWordAnimationPreset: "sequentialPop", captionWordStaggerSeconds: 0.5 },
          remove: [],
        },
      ],
    });
  });
});
