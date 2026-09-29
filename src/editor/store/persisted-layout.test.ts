import { beforeEach, describe, expect, it } from "vitest";
import { loadEditorLayout, loadTimelineView, saveEditorLayout, saveTimelineView } from "./persisted-layout";

describe("persisted layout", () => {
  beforeEach(() => window.localStorage.clear());

  it("returns defaults and clamps stored values", () => {
    expect(loadEditorLayout()).toEqual({ activeTab: "ai", leftWidth: 360 });
    window.localStorage.setItem("video-creater.editor.v2.layout", JSON.stringify({ activeTab: "nope", leftWidth: 9999 }));
    expect(loadEditorLayout()).toEqual({ activeTab: "ai", leftWidth: 440 });
  });

  it("round-trips layout and per-project timeline view", () => {
    saveEditorLayout({ activeTab: "captions", leftWidth: 320 });
    expect(loadEditorLayout()).toEqual({ activeTab: "captions", leftWidth: 320 });
    const view = {
      splitHeight: 280,
      zoomPercent: 150,
      snapEnabled: false,
      keyframesVisible: true,
      tool: "blade",
      laneProperty: "volumeDb",
    } as const;
    saveTimelineView("/p", view);
    expect(loadTimelineView("/p")).toEqual(view);
    expect(loadTimelineView("/other")).toEqual({
      splitHeight: 300,
      zoomPercent: 100,
      snapEnabled: true,
      keyframesVisible: false,
      tool: "select",
      laneProperty: "opacity",
    });
  });

  it("falls back for an unknown tool or lane property", () => {
    window.localStorage.setItem(
      "video-creater.editor.v2.timeline-view:/p",
      JSON.stringify({ tool: "lasso", laneProperty: "brightness" }),
    );
    expect(loadTimelineView("/p")).toMatchObject({ tool: "select", laneProperty: "opacity" });
  });

  it("ignores corrupt storage", () => {
    window.localStorage.setItem("video-creater.editor.v2.layout", "{");
    expect(loadEditorLayout()).toEqual({ activeTab: "ai", leftWidth: 360 });
  });
});
