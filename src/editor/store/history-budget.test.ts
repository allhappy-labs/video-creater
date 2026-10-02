import { describe, expect, it } from "vitest";
import { fixtureProject } from "@/test-utils/editor-fixtures";
import { boundHistory } from "./history-budget";

describe("history representation budget", () => {
  it("retains the nearest undo and redo states under a combined byte budget", () => {
    const snapshots = Array.from({ length: 8 }, (_, index) => ({ ...fixtureProject(), name: `state-${index}` }));
    const one = JSON.stringify(snapshots[0]).length * 2;
    const history = boundHistory(snapshots.slice(0, 5), snapshots.slice(5), one * 3);
    expect(history.past).toEqual([snapshots[4]]);
    expect(history.future).toEqual([snapshots[5], snapshots[6]]);
    expect(history.droppedPast).toBe(4);
  });

  it("keeps one oversized state undoable and preserves the existing count cap", () => {
    const snapshot = fixtureProject();
    expect(boundHistory([snapshot, snapshot], [], 1)).toMatchObject({ past: [snapshot], droppedPast: 1 });
    expect(boundHistory(Array.from({ length: 101 }, () => snapshot), [], Infinity).past).toHaveLength(100);
    expect(boundHistory([], [snapshot, snapshot], 1).future).toEqual([snapshot]);
  });
});
