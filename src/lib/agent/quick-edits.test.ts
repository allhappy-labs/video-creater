import { describe, expect, it } from "vitest";
import { quickEdits } from "@/lib/agent/quick-edits";

describe("quickEdits", () => {
  it("offers the spec suggestions in order", () => {
    expect(quickEdits.map((edit) => edit.label)).toEqual([
      "Tighten the pacing",
      "Remove dead air",
      "Add clean captions",
      "Balance the audio",
      "Make a shorter cut",
    ]);
  });

  it("uses unique ids and visible prompts that start with the label", () => {
    expect(new Set(quickEdits.map((edit) => edit.id)).size).toBe(quickEdits.length);
    for (const edit of quickEdits) {
      expect(edit.prompt.startsWith(edit.label)).toBe(true);
      expect(edit.prompt.trim()).toBe(edit.prompt);
      expect(edit.prompt).not.toMatch(/media-|item-|\{|\}/);
    }
  });
});
