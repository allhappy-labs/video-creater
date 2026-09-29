import assert from "node:assert/strict";
import { mkdir, mkdtemp, realpath, symlink, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
import test from "node:test";

import {
  REQUIRED_FREE_BYTES,
  VERIFY_CACHE_LIMIT_BYTES,
  decideVerificationCacheAction,
  directorySizeBytes,
  validateCargoTargetPath,
} from "./cargo-cache-policy.mjs";

test("verification cache policy enforces both size and free-space bounds", () => {
  assert.equal(VERIFY_CACHE_LIMIT_BYTES, 24 * 1024 ** 3);
  assert.equal(REQUIRED_FREE_BYTES, 20 * 1024 ** 3);
  assert.equal(decideVerificationCacheAction({
    verificationBytes: VERIFY_CACHE_LIMIT_BYTES,
    availableBytes: REQUIRED_FREE_BYTES,
  }), "retain");
  assert.equal(decideVerificationCacheAction({
    verificationBytes: VERIFY_CACHE_LIMIT_BYTES + 1,
    availableBytes: REQUIRED_FREE_BYTES,
  }), "clean");
  assert.equal(decideVerificationCacheAction({
    verificationBytes: 0,
    availableBytes: REQUIRED_FREE_BYTES - 1,
  }), "clean");
});

test("directory measurement treats a missing target as empty", async () => {
  const root = await mkdtemp(join(tmpdir(), "cargo-cache-size-"));
  assert.equal(await directorySizeBytes(join(root, "missing")), 0);
  await writeFile(join(root, "payload"), "12345");
  assert.equal(await directorySizeBytes(root), 5);
});

test("target validation accepts only exact canonical targets", async () => {
  const root = await mkdtemp(join(tmpdir(), "cargo-cache-path-"));
  const repoRoot = join(root, "repo");
  const target = join(repoRoot, "src-tauri", "target");
  const verify = join(target, "verify");
  await mkdir(verify, { recursive: true });

  const canonicalRepo = await realpath(repoRoot);
  assert.equal(await validateCargoTargetPath({
    repoRoot,
    targetPath: verify,
    kind: "verification",
  }), resolve(canonicalRepo, "src-tauri/target/verify"));
  assert.equal(await validateCargoTargetPath({
    repoRoot,
    targetPath: target,
    kind: "development",
  }), resolve(canonicalRepo, "src-tauri/target"));

  for (const rejected of [
    "",
    "/",
    repoRoot,
    join(repoRoot, ".."),
    join(target, "debug"),
    join(target, "..", "target", "other"),
    "$CARGO_TARGET_DIR",
  ]) {
    await assert.rejects(validateCargoTargetPath({
      repoRoot,
      targetPath: rejected,
      kind: "verification",
    }));
  }
});

test("verification target validation rejects a symlink", async () => {
  const root = await mkdtemp(join(tmpdir(), "cargo-cache-link-"));
  const repoRoot = join(root, "repo");
  const target = join(repoRoot, "src-tauri", "target");
  const elsewhere = join(root, "elsewhere");
  await mkdir(target, { recursive: true });
  await mkdir(elsewhere);
  await symlink(elsewhere, join(target, "verify"));

  await assert.rejects(validateCargoTargetPath({
    repoRoot,
    targetPath: join(target, "verify"),
    kind: "verification",
  }), /symbolic link/);
});

test("Cargo manifest keeps incremental development and non-incremental tests", async () => {
  const manifest = await import("node:fs/promises").then(({ readFile }) =>
    readFile(new URL("../src-tauri/Cargo.toml", import.meta.url), "utf8")
  );
  assert.match(manifest, /\[profile\.dev\]\ndebug = 1\nincremental = true/);
  assert.match(manifest, /\[profile\.test\]\ndebug = 1\nincremental = false/);
});
