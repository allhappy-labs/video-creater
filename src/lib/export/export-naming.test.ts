import { describe, expect, it } from "vitest";
import {
  artifactFileName,
  exportFileLabel,
  exportFileNameProblem,
  saveRangeMediaName,
} from "@/lib/export/export-naming";

describe("exportFileNameProblem", () => {
  it("accepts a plain name", () => {
    expect(exportFileNameProblem("Edison intro")).toBeNull();
    expect(exportFileNameProblem("Edison intro.mp4")).toBeNull();
    expect(exportFileNameProblem("a".repeat(180))).toBeNull();
  });

  it("refuses names the backend refuses, with the same words", () => {
    expect(exportFileNameProblem("")).toBe("Give the export a name.");
    expect(exportFileNameProblem("  ")).toBe("Give the export a name.");
    expect(exportFileNameProblem("a/b")).toBe("Export names can't contain slashes.");
    expect(exportFileNameProblem("a\\b")).toBe("Export names can't contain slashes.");
    expect(exportFileNameProblem(".")).toBe("Export names can't start with a dot.");
    expect(exportFileNameProblem("..")).toBe("Export names can't start with a dot.");
    expect(exportFileNameProblem(".hidden")).toBe("Export names can't start with a dot.");
    expect(exportFileNameProblem("a\u0000b")).toBe(
      "Export names can't contain control characters.",
    );
    expect(exportFileNameProblem("a\nb")).toBe("Export names can't contain control characters.");
    expect(exportFileNameProblem("a".repeat(181))).toBe(
      "Export names can't be longer than 180 bytes.",
    );
    // 90 two-byte characters plus one byte is 181 UTF-8 bytes.
    expect(exportFileNameProblem(`${"é".repeat(90)}a`)).toBe(
      "Export names can't be longer than 180 bytes.",
    );
  });
});

describe("exportFileLabel", () => {
  it("adds the extension once", () => {
    expect(exportFileLabel("Edison intro.mp4", "mp4")).toBe("Edison intro.mp4");
    expect(exportFileLabel("Edison intro.MP4", "mp4")).toBe("Edison intro.mp4");
    expect(exportFileLabel("Edison intro", "mp4")).toBe("Edison intro.mp4");
    expect(exportFileLabel(" Edison intro ", "webm")).toBe("Edison intro.webm");
  });
});

describe("artifactFileName", () => {
  it("returns the last path segment", () => {
    expect(artifactFileName("/home/me/Movies/Edison intro (2).mp4")).toBe("Edison intro (2).mp4");
    expect(artifactFileName("exports/a.mp4")).toBe("a.mp4");
    expect(artifactFileName("C:\\Users\\me\\a.mp4")).toBe("a.mp4");
    expect(artifactFileName("a.mp4")).toBe("a.mp4");
  });
});

describe("saveRangeMediaName", () => {
  const range = { startSeconds: 4, endSeconds: 9 };

  it("uses the project name when there is at most one timeline", () => {
    expect(saveRangeMediaName({ name: "Edison Restoration Demo" }, range)).toBe(
      "Edison Restoration Demo 00:04–00:09",
    );
    expect(
      saveRangeMediaName(
        {
          name: "Edison Restoration Demo",
          timelines: [{ id: "main", name: "Timeline 1" }],
        },
        range,
      ),
    ).toBe("Edison Restoration Demo 00:04–00:09");
  });

  it("uses the active timeline name when the project has several timelines", () => {
    const project = {
      name: "Edison Restoration Demo",
      activeTimelineId: "b",
      timelines: [
        { id: "main", name: "Timeline 1" },
        { id: "b", name: "Trailer cut" },
      ],
    };
    expect(saveRangeMediaName(project, range)).toBe("Trailer cut 00:04–00:09");
    expect(saveRangeMediaName({ ...project, activeTimelineId: null }, range)).toBe(
      "Timeline 1 00:04–00:09",
    );
  });

  it("falls back to a generic name when the name is blank", () => {
    expect(saveRangeMediaName({ name: "  " }, { startSeconds: 0, endSeconds: 65 })).toBe(
      "Timeline range 00:00–01:05",
    );
  });
});
