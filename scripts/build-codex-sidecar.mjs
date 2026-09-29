#!/usr/bin/env node

// Stages the pinned @openai/codex native binary as the Tauri `video-creater-codex`
// sidecar (src-tauri/binaries/video-creater-codex-<target>).
//
// Linux runtime layout (@openai/codex-linux-{x64,arm64} ships
// vendor/<arch>-unknown-linux-musl/{codex-package.json,bin/codex,codex-path/rg,
// codex-resources/bwrap,codex-resources/zsh/bin/zsh}):
// - codex only treats codex-path/ and codex-resources/ as bundled when the
//   executable lives at <root>/bin/<exe> next to <root>/codex-package.json. The
//   Tauri sidecar is installed flat beside the app executable, so that detection
//   does not apply; codex then resolves `rg` from PATH and `bwrap` from PATH
//   first, falling back to <exe dir>/codex-resources/bwrap.
// - codex-path/rg is staged to src-tauri/resources/codex-runtime/codex-path/rg.
//   Bundle it as "resources/codex-runtime/codex-path/rg" ->
//   "codex-runtime/codex-path/rg" and prepend <resource dir>/codex-runtime/codex-path
//   to PATH when spawning `codex app-server`.
// - codex-resources/bwrap is not staged: Ubuntu 24.04 restricts unprivileged user
//   namespaces through AppArmor, which only allows /usr/bin/bwrap (package
//   `bubblewrap`), so the sandbox must use the system bwrap found on PATH.
// - codex-resources/zsh is not staged: it only backs the disabled
//   shell_zsh_fork features.
import { chmodSync, copyFileSync, mkdirSync, readFileSync } from "node:fs";
import { createRequire } from "node:module";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { execFileSync } from "node:child_process";

const repoRoot = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const require = createRequire(import.meta.url);
const development = process.argv.slice(2).includes("--development");
const unknownArgs = process.argv.slice(2).filter((value) => value !== "--development");
if (unknownArgs.length > 0) {
  throw new Error(`unknown argument: ${unknownArgs[0]}`);
}
const hostLinuxTargetByArch = {
  x64: "x86_64-unknown-linux-gnu",
  arm64: "aarch64-unknown-linux-gnu",
};
const defaultTarget =
  (process.platform === "linux" && hostLinuxTargetByArch[process.arch]) || "aarch64-apple-darwin";
const target = process.env.TAURI_ENV_TARGET_TRIPLE?.trim() || defaultTarget;
const platformPackageByTarget = {
  "aarch64-apple-darwin": { name: "@openai/codex-darwin-arm64", vendor: "aarch64-apple-darwin" },
  "x86_64-apple-darwin": { name: "@openai/codex-darwin-x64", vendor: "x86_64-apple-darwin" },
  "x86_64-unknown-linux-gnu": {
    name: "@openai/codex-linux-x64",
    vendor: "x86_64-unknown-linux-musl",
    pathTools: ["rg"],
  },
  "aarch64-unknown-linux-gnu": {
    name: "@openai/codex-linux-arm64",
    vendor: "aarch64-unknown-linux-musl",
    pathTools: ["rg"],
  },
};
const platformPackage = platformPackageByTarget[target];
if (!platformPackage) {
  throw new Error(`unsupported bundled Codex target: ${target}`);
}

const codexPackagePath = require.resolve("@openai/codex/package.json");
const codexPackage = JSON.parse(readFileSync(codexPackagePath, "utf8"));
if (codexPackage.version !== "0.141.0") {
  throw new Error(`unexpected @openai/codex version: ${codexPackage.version}`);
}
const codexRequire = createRequire(codexPackagePath);
let platformPackagePath;
try {
  platformPackagePath = codexRequire.resolve(`${platformPackage.name}/package.json`);
} catch (error) {
  throw new Error(
    `${platformPackage.name} is not installed; run pnpm install on the ${target} host`,
    { cause: error },
  );
}
const vendorRoot = join(dirname(platformPackagePath), "vendor", platformPackage.vendor);
const source = join(vendorRoot, "bin", "codex");
const output = join(
  repoRoot,
  "src-tauri",
  "binaries",
  `video-creater-codex-${target}`,
);

mkdirSync(dirname(output), { recursive: true });
const outputs = [output];
if (development) {
  outputs.push(join(repoRoot, "src-tauri", "target", "debug", "video-creater-codex"));
}
for (const preparedOutput of outputs) {
  mkdirSync(dirname(preparedOutput), { recursive: true });
  copyFileSync(source, preparedOutput);
  chmodSync(preparedOutput, 0o755);
  if (process.platform === "darwin") {
    execFileSync("codesign", ["--force", "--sign", "-", preparedOutput], {
      cwd: repoRoot,
      stdio: "inherit",
    });
  }
}
const resources = [];
for (const tool of platformPackage.pathTools ?? []) {
  const resourceOutput = join(repoRoot, "src-tauri", "resources", "codex-runtime", "codex-path", tool);
  mkdirSync(dirname(resourceOutput), { recursive: true });
  copyFileSync(join(vendorRoot, "codex-path", tool), resourceOutput);
  chmodSync(resourceOutput, 0o755);
  resources.push(resourceOutput);
}
process.stdout.write(
  `${JSON.stringify({
    status: "prepared",
    target,
    version: codexPackage.version,
    outputs,
    ...(resources.length > 0 ? { resources } : {}),
  })}\n`,
);
