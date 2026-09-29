import assert from "node:assert/strict";
import { mkdtempSync, mkdirSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join } from "node:path";
import { spawnSync } from "node:child_process";
import test from "node:test";

const checkerScript = new URL("./check-tooling-source.mjs", import.meta.url);

function write(root: string, path: string, contents: string) {
  const target = join(root, path);
  mkdirSync(dirname(target), { recursive: true });
  writeFileSync(target, contents);
}

function runChecker(root: string) {
  return spawnSync(process.execPath, [checkerScript.pathname, "--root", root], {
    encoding: "utf8",
  });
}

test("accepts syntactically valid repository tooling", () => {
  const root = mkdtempSync(join(tmpdir(), "video-creater-tooling-policy-"));
  write(root, "scripts/build.mjs", "export const build = () => 'ready';\n");
  write(root, "postcss.config.js", "export default { plugins: {} };\n");
  write(root, "src/ignored.js", "this is frontend policy territory, not tooling syntax");

  const result = runChecker(root);

  assert.equal(result.status, 0, result.stderr);
  assert.match(result.stdout, /tooling source check passed for 2 files/);
});

test("reports every invalid tooling file with its repository path", () => {
  const root = mkdtempSync(join(tmpdir(), "video-creater-tooling-policy-"));
  write(root, "scripts/first.mjs", "export const first = ;\n");
  write(root, "scripts/second.js", "const second = {;\n");

  const result = runChecker(root);

  assert.equal(result.status, 1);
  assert.match(result.stderr, /scripts\/first\.mjs: syntax check failed/);
  assert.match(result.stderr, /scripts\/second\.js: syntax check failed/);
});
