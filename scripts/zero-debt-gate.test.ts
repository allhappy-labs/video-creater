import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import test from "node:test";

const packageJson = JSON.parse(
  readFileSync(new URL("../package.json", import.meta.url), "utf8"),
) as { scripts?: Record<string, string> };

test("frontend verification contains no native work", () => {
  const expected = [
    "pnpm check:source-quality",
    "pnpm check:tooling-source",
    "pnpm test:source-quality",
    "pnpm lint",
    "pnpm test",
    "pnpm build",
    "pnpm check:unused",
    "pnpm test:browser",
    "pnpm visual:qa:browser-release",
  ].join(" && ");

  const frontend = packageJson.scripts?.["verify:frontend"] ?? "";
  assert.equal(frontend, expected);
  assert.doesNotMatch(
    frontend,
    /\bcargo\b|\btauri\b|\brust:|run-native|build:(?:gstreamer|compatibility|precompose|avfoundation|audio|semantic|fluidaudio|codex)/,
  );
});

test("canonical release verification delegates bounded native work", () => {
  assert.equal(
    packageJson.scripts?.["verify:release"],
    "pnpm verify:frontend && pnpm test:gstreamer-release-policy && pnpm test:release-runtime-policy && pnpm check:release-runtime-policy && pnpm test:verification-policy && pnpm verify:native:release",
  );
  assert.equal(packageJson.scripts?.["verify:zero-debt"], "pnpm verify:release");
  assert.equal(packageJson.scripts?.verify, "pnpm verify:release");
});

test("Clippy covers the exact release binary feature profiles", () => {
  const clippy = packageJson.scripts?.["rust:clippy"] ?? "";

  assert.match(
    clippy,
    /--no-default-features --features mcp-server --bin video-creater-mcp-server -- -D warnings/,
  );
  assert.match(
    clippy,
    /--no-default-features --features app-runtime,custom-protocol,coreml-inspect,ges-render,gpu-render,graphics-render --bin video-creater -- -D warnings/,
  );
});
