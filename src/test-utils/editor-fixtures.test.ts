import { describe, expect, it } from "vitest";
import {
  fixtureGeneratedAsset,
  fixtureItem,
  fixtureJobs,
  fixtureMedia,
  fixtureProject,
  fixtureTrack,
} from "./editor-fixtures";

describe("editor fixtures", () => {
  it("returns independent sample project copies", () => {
    const first = fixtureProject();
    const second = fixtureProject();
    first.name = "changed";
    expect(second.name).not.toBe("changed");
  });

  it("finds representative media, items, and generated assets", () => {
    const project = fixtureProject();
    expect(fixtureMedia(project, "video").id).toBeTruthy();
    expect(fixtureItem(project, "video").id).toBeTruthy();
    expect(fixtureGeneratedAsset(project).id).toBeTruthy();
  });

  it("finds representative tracks and jobs", () => {
    const project = fixtureProject();
    expect(fixtureTrack(project, "caption").items.length).toBeGreaterThan(0);
    expect(fixtureJobs(project).length).toBeGreaterThan(0);
  });
});
