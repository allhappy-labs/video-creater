import { describe, expect, it } from "vitest";

import { projectSessionKey } from "./project-session";

describe("projectSessionKey", () => {
  it("treats equivalent directory spellings as one project session", () => {
    expect(projectSessionKey("/projects/current///", "project-a")).toBe(
      projectSessionKey("/projects//current", "project-a"),
    );
  });

  it("changes when either canonical project identity field changes", () => {
    const current = projectSessionKey("/projects/current", "project-a");

    expect(projectSessionKey("/projects/next", "project-a")).not.toBe(current);
    expect(projectSessionKey("/projects/current", "project-b")).not.toBe(current);
  });

  it("does not conflate supported POSIX paths containing whitespace", () => {
    expect(projectSessionKey("/projects/current", "project-a")).not.toBe(
      projectSessionKey("/projects/current ", "project-a"),
    );
  });

  it("preserves a browser project URI scheme while normalizing its path", () => {
    const key = projectSessionKey("browser://bundled-sample///", "project-sample");

    expect(key).toContain("browser://bundled-sample");
    expect(key).not.toContain("browser:/bundled-sample");
  });
});
