import { mkdtempSync, readFileSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { describe, expect, it } from "vitest";

// @ts-expect-error The repository does not generate declarations for scripts/*.mjs.
import { buildComparisonBoard } from "../scripts/palmier-visual-comparison.mjs";

describe("Palmier visual comparison board", () => {
  it("pairs the Palmier project home reference with the home capture", () => {
    const scriptSource = readFileSync(
      join(process.cwd(), "scripts/palmier-visual-comparison.mjs"),
      "utf8",
    );
    expect(scriptSource).toContain('["home", "Project home", "01-home.png", "home-palmier-desktop.png"]');
  });

  it("embeds reference and implementation images in one self-contained board", () => {
    const dir = mkdtempSync(join(tmpdir(), "palmier-board-"));
    const png = Buffer.from("89504e470d0a1a0a", "hex");
    writeFileSync(join(dir, "reference.png"), png);
    writeFileSync(join(dir, "implementation.png"), png);
    const result = buildComparisonBoard({ pairs: [{ id: "home", label: "Project home", reference: join(dir, "reference.png"), implementation: join(dir, "implementation.png") }], htmlOut: join(dir, "index.html"), manifestOut: join(dir, "manifest.json") });
    expect(result.pairs).toHaveLength(1);
    expect(readFileSync(join(dir, "index.html"), "utf8")).toContain("data:image/png;base64,");
    expect(JSON.parse(readFileSync(join(dir, "manifest.json"), "utf8"))).toMatchObject({ pairs: [{ id: "home" }] });
  });

  it("escapes untrusted labels and paths before rendering HTML", () => {
    const dir = mkdtempSync(join(tmpdir(), "palmier-board-"));
    const image = join(dir, "<capture>.png");
    writeFileSync(image, Buffer.from("89504e470d0a1a0a", "hex"));
    buildComparisonBoard({ pairs: [{ id: "unsafe", label: '<script>alert("no")</script>', reference: image, implementation: image }], htmlOut: join(dir, "index.html"), manifestOut: join(dir, "manifest.json") });
    const html = readFileSync(join(dir, "index.html"), "utf8");
    expect(html).not.toContain('<script>alert("no")</script>');
    expect(html).toContain("&lt;script&gt;");
    expect(html).toContain("&lt;capture&gt;.png");
  });

  it("fails closed when either side is missing", () => {
    expect(() => buildComparisonBoard({ pairs: [{ id: "missing", label: "Missing", reference: "/missing/reference.png", implementation: "/missing/implementation.png" }], htmlOut: "/tmp/board.html", manifestOut: "/tmp/manifest.json" })).toThrow(/missing/i);
  });
});
