import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import test from "node:test";

const knipConfig = readFileSync(new URL("../knip.jsonc", import.meta.url), "utf8");

test("knip has no temporary editor redesign ignore block", () => {
  assert.doesNotMatch(knipConfig, /Editor redesign/i);
  assert.doesNotMatch(
    knipConfig,
    /"ignoreIssues"\s*:/,
    "Remove dead exports or give them a consumer instead of ignoring knip issues.",
  );
});
