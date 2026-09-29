import assert from "node:assert/strict";
import { mkdtempSync, mkdirSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join } from "node:path";
import { spawnSync } from "node:child_process";
import test from "node:test";

const policyScript = new URL("./source-quality-policy.mjs", import.meta.url);

function write(root: string, path: string, contents: string) {
  const target = join(root, path);
  mkdirSync(dirname(target), { recursive: true });
  writeFileSync(target, contents);
}

function validTsconfig() {
  return JSON.stringify({
    compilerOptions: {
      strict: true,
      noUncheckedIndexedAccess: true,
      exactOptionalPropertyTypes: true,
      noImplicitOverride: true,
    },
  });
}

function createValidFixture() {
  const root = mkdtempSync(join(tmpdir(), "video-creater-source-policy-"));
  write(root, "tsconfig.json", validTsconfig());
  write(root, "tsconfig.node.json", validTsconfig());
  write(root, "src/index.ts", "export const ready = true;\n");
  write(root, "src-tauri/src/lib.rs", "pub fn ready() -> bool { true }\n");
  write(root, "scripts/build-release.mjs", "export const command = 'pnpm build';\n");
  return root;
}

function runPolicy(root: string) {
  return spawnSync(process.execPath, [policyScript.pathname, "--root", root], {
    encoding: "utf8",
  });
}

test("rejects JavaScript inside the frontend source boundary", () => {
  const root = createValidFixture();
  write(root, "src/legacy.js", "export default true;\n");

  const result = runPolicy(root);

  assert.equal(result.status, 1);
  assert.match(result.stderr, /src\/legacy\.js: frontend source must use \.ts or \.tsx/);
});

test("requires every strict TypeScript compiler option", () => {
  const root = createValidFixture();
  write(
    root,
    "tsconfig.json",
    JSON.stringify({ compilerOptions: { strict: true } }),
  );

  const result = runPolicy(root);

  assert.equal(result.status, 1);
  assert.match(result.stderr, /tsconfig\.json: noUncheckedIndexedAccess must be true/);
  assert.match(result.stderr, /tsconfig\.json: exactOptionalPropertyTypes must be true/);
  assert.match(result.stderr, /tsconfig\.json: noImplicitOverride must be true/);
});

test("rejects broad Rust Clippy allow attributes", () => {
  const root = createValidFixture();
  write(
    root,
    "src-tauri/src/lib.rs",
    "#[allow(clippy::too_many_arguments)]\npub fn legacy() {}\n",
  );

  const result = runPolicy(root);

  assert.equal(result.status, 1);
  assert.match(result.stderr, /src-tauri\/src\/lib\.rs:1: broad Clippy allow is prohibited/);
});

test("requires a reason on every Clippy expect attribute", () => {
  const root = createValidFixture();
  write(
    root,
    "src-tauri/src/lib.rs",
    "#[expect(clippy::too_many_arguments)]\npub fn protocol_shape() {}\n",
  );

  const result = runPolicy(root);

  assert.equal(result.status, 1);
  assert.match(
    result.stderr,
    /src-tauri\/src\/lib\.rs:1: Clippy expect requires a non-empty reason/,
  );
});

test("rejects forbidden bare commands in release Rust targets", () => {
  const root = createValidFixture();
  write(
    root,
    "src-tauri/src/main.rs",
    'fn preview() { let _ = std::process::Command::new("pnpm"); }\n',
  );

  const result = runPolicy(root);

  assert.equal(result.status, 1);
  assert.match(
    result.stderr,
    /src-tauri\/src\/main\.rs:1: release runtime resolves forbidden bare command "pnpm"/,
  );
});

test("rejects forbidden bare commands hidden behind release command specs", () => {
  const root = createValidFixture();
  write(
    root,
    "src-tauri/src/lib.rs",
    'fn preview() { let _ = CommandSpec::new("ffmpeg"); }\n',
  );

  const result = runPolicy(root);

  assert.equal(result.status, 1);
  assert.match(
    result.stderr,
    /src-tauri\/src\/lib\.rs:1: release runtime resolves forbidden bare command "ffmpeg"/,
  );
});

test("rejects developer-only GStreamer recovery copy in release errors", () => {
  const root = createValidFixture();
  write(
    root,
    "src-tauri/src/lib.rs",
    'const RECOVERY: &str = "Install the GStreamer pbutils runtime and retry.";\n',
  );

  const result = runPolicy(root);

  assert.equal(result.status, 1);
  assert.match(
    result.stderr,
    /src-tauri\/src\/lib\.rs:1: release-facing recovery exposes developer instruction "Install the GStreamer pbutils runtime and retry\."/,
  );
});

test("permits package-manager commands in classified build tooling", () => {
  const root = createValidFixture();

  const result = runPolicy(root);

  assert.equal(result.status, 0, result.stderr);
  assert.match(result.stdout, /source quality policy passed/);
});

test("rejects dotLottie module declarations whose curated source is absent", () => {
  const root = createValidFixture();
  write(
    root,
    "src-tauri/vendor/dotlottie-rs/src/lib.rs",
    [
      '#[cfg(feature = "audio")]',
      "mod audio;",
      '#[cfg(feature = "audio")] as compile_error_guard',
      'compile_error!("unsupported");',
      "",
    ].join("\n"),
  );
  const result = runPolicy(root);

  assert.equal(result.status, 1);
  assert.match(
    result.stderr,
    /src-tauri\/vendor\/dotlottie-rs\/src\/lib\.rs:2: module audio has no traversable source file/,
  );
});

test("requires every forbidden dotLottie feature to fail closed", () => {
  const root = createValidFixture();
  const forbiddenFeatures = [
    "audio",
    "c_api",
    "dev",
    "state-machines",
    "theming",
    "tracking_allocator",
    "tvg-gl",
    "tvg-log",
    "tvg-otf",
    "tvg-simd",
    "tvg-threads",
    "tvg-ttf",
    "tvg-wg",
    "wasm-bindgen-api",
    "webgl",
    "webgpu",
  ];
  write(
    root,
    "src-tauri/vendor/dotlottie-rs/Cargo.toml",
    `[features]\n${forbiddenFeatures.map((feature) => `${feature} = []`).join("\n")}\n`,
  );
  write(
    root,
    "src-tauri/vendor/dotlottie-rs/src/lib.rs",
    [
      "#[cfg(any(",
      ...forbiddenFeatures
        .filter((feature) => feature !== "audio")
        .map((feature) => `    feature = "${feature}",`),
      "))]",
      'compile_error!("this vendored dotLottie runtime supports only the reviewed native CPU feature set");',
      "",
    ].join("\n"),
  );
  write(
    root,
    "src-tauri/vendor/dotlottie-rs/build.rs",
    forbiddenFeatures
      .filter((feature) => feature !== "audio")
      .map((feature) => `cfg!(feature = "${feature}");`)
      .join("\n"),
  );
  write(
    root,
    "src-tauri/crates/precompose-worker/Cargo.toml",
    'dotlottie-rs = { path = "../../vendor/dotlottie-rs", features = ["dotlottie", "audio"] }\n',
  );

  const result = runPolicy(root);

  assert.equal(result.status, 1);
  assert.match(result.stderr, /dotLottie feature audio is missing from the fail-closed compile guard/);
  assert.match(result.stderr, /dotLottie feature audio is missing from the pre-compilation build guard/);
  assert.match(result.stderr, /precompose-worker enables forbidden dotLottie feature audio/);
});
