import assert from "node:assert/strict";
import { createRequire } from "node:module";
import test from "node:test";

const require = createRequire(import.meta.url);
const tailwindRequire = createRequire(require.resolve("tailwindcss/package.json"));
const chokidarRequire = createRequire(tailwindRequire.resolve("chokidar/package.json"));
const braces = chokidarRequire("braces");

test("the installed watcher dependency rejects deeply nested brace patterns before recursive traversal", () => {
  const pattern = "{".repeat(4_000) + "x,y" + "}".repeat(4_000);
  for (const operation of [braces.parse, braces.compile, braces.expand]) {
    assert.throws(() => operation(pattern), { name: "SyntaxError", code: "BRACES_MAX_DEPTH" });
  }
});

test("direct AST traversal rejects excessive depth and cycles", () => {
  for (const operation of [braces.compile, braces.expand, braces.stringify]) {
    const ast = { type: "root", nodes: [] as unknown[] };
    let current = ast;
    for (let index = 0; index < 15_000; index++) {
      const child = { type: "brace", nodes: [] as unknown[] };
      current.nodes.push(child); current = child;
    }
    assert.throws(() => operation(ast), { name: "SyntaxError", code: "BRACES_MAX_DEPTH" });
    const cyclic = { type: "root", nodes: [] as unknown[] };
    cyclic.nodes.push(cyclic);
    assert.throws(() => operation(cyclic), { name: "SyntaxError", code: "BRACES_MAX_DEPTH" });
  }
});

test("ordinary watcher globs retain expansion, escaping and compile semantics", () => {
  assert.deepEqual(braces.expand("src/{lib,editor}/**/*.{ts,tsx}"), ["src/lib/**/*.ts", "src/lib/**/*.tsx", "src/editor/**/*.ts", "src/editor/**/*.tsx"]);
  assert.deepEqual(braces.expand("frame-{01..03}.png"), ["frame-01.png", "frame-02.png", "frame-03.png"]);
  assert.deepEqual(braces.expand("literal-\\{brace\\}"), ["literal-{brace}"]);
  const regex = new RegExp(`^${braces.compile("file-{a,b}.ts")}$`);
  assert.equal(regex.test("file-a.ts"), true);
  assert.equal(regex.test("file-c.ts"), false);
});
