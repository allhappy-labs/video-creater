import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import { describe, expect, it } from "vitest";

const indexCss = readFileSync(resolve(process.cwd(), "src/index.css"), "utf8");
const tailwindConfig = readFileSync(resolve(process.cwd(), "tailwind.config.ts"), "utf8");

describe("captions workbench layout", () => {
  it("defines an opaque popover palette for caption agent actions", () => {
    expect(indexCss).toMatch(/--popover:\s*230 9% 13%/);
    expect(indexCss).toMatch(/--popover-foreground:\s*228 7% 92%/);
    expect(tailwindConfig).toContain('popover: {');
    expect(tailwindConfig).toContain('DEFAULT: "hsl(var(--popover))"');
    expect(tailwindConfig).toContain('foreground: "hsl(var(--popover-foreground))"');
  });
});
