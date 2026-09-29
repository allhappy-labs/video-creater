import { describe, expect, it } from "vitest";

import type { ProjectActionRippleDeleteRange } from "@/lib/project";

import { silenceSummary } from "./cleanup-summary";

describe("silenceSummary", () => {
  it("is empty for no ranges", () => {
    expect(silenceSummary([])).toEqual({ count: 0, secondsSaved: 0 });
  });

  it("counts ranges and rounds the time saved to 0.1 s", () => {
    expect(
      silenceSummary([
        { startSeconds: 1, endSeconds: 1.84 },
        { startSeconds: 3.2, endSeconds: 4.31 },
      ]),
    ).toEqual({ count: 2, secondsSaved: 2 });
    expect(silenceSummary([{ startSeconds: 0, endSeconds: 0.76 }])).toEqual({ count: 1, secondsSaved: 0.8 });
    expect(silenceSummary([{ startSeconds: 0, endSeconds: 0.74 }])).toEqual({ count: 1, secondsSaved: 0.7 });
  });

  it("counts timeline time once when linked tracks share a range", () => {
    const rippleRanges: ProjectActionRippleDeleteRange[] = [
      { startSeconds: 2, endSeconds: 3, trackIds: ["v1"] },
      { startSeconds: 2.5, endSeconds: 3.5, trackIds: ["a1"] },
    ];

    expect(silenceSummary(rippleRanges)).toEqual({ count: 2, secondsSaved: 1.5 });
  });

  it("ignores empty or invalid ranges in the time saved", () => {
    expect(
      silenceSummary([
        { startSeconds: 4, endSeconds: 4 },
        { startSeconds: 5, endSeconds: Number.NaN },
        { startSeconds: 1, endSeconds: 2 },
      ]),
    ).toEqual({ count: 3, secondsSaved: 1 });
  });
});
