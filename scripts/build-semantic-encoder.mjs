import { copyFileSync, mkdirSync } from "node:fs";
import { execFileSync } from "node:child_process";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const root = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const packagePath = join(root, "src-tauri/native/semantic-encoder");
const target = "aarch64-apple-darwin";
const output = join(
  root,
  "src-tauri/binaries",
  `video-creater-semantic-encoder-${target}`,
);

execFileSync("swift", ["build", "-c", "release", "--package-path", packagePath], {
  cwd: root,
  stdio: "inherit",
});
mkdirSync(dirname(output), { recursive: true });
copyFileSync(
  join(packagePath, ".build/release/video-creater-semantic-encoder"),
  output,
);
execFileSync("codesign", ["--force", "--sign", "-", output], {
  cwd: root,
  stdio: "inherit",
});
