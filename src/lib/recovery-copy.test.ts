import { describe, expect, it } from "vitest";
import { projectFolderRecoveryCopy } from "./recovery-copy";

describe("projectFolderRecoveryCopy", () => {
  it("turns missing manifest errors into relink or create guidance", () => {
    const copy = projectFolderRecoveryCopy(
      new Error("Create video-creater.project.json before opening."),
      "open",
    );

    expect(copy.title).toBe("Project folder is missing its manifest");
    expect(copy.message).toContain("Choose a split project folder");
    expect(copy.message).toContain("Create video-creater.project.json");
  });

  it("turns create permission failures into writable-folder guidance", () => {
    const copy = projectFolderRecoveryCopy("folder is not writable", "create");

    expect(copy.title).toBe("Project folder is not writable");
    expect(copy.message).toContain("Check folder permissions");
    expect(copy.message).toContain("folder is not writable");
  });

  it("keeps unknown failures actionable without hiding details", () => {
    const copy = projectFolderRecoveryCopy(null, "open");

    expect(copy.title).toBe("Project folder could not be opened");
    expect(copy.message).toContain("folder exists");
    expect(copy.message).toContain("No detailed error was returned");
  });
});
