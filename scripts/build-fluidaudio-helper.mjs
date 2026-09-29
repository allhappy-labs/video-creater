import { chmodSync, copyFileSync, mkdirSync } from "node:fs";
import { spawnSync } from "node:child_process";
import { dirname, join, resolve } from "node:path";

import { prepareFluidAudioCheckout } from "./prepare-fluidaudio-checkout.mjs";

const root = resolve(import.meta.dirname, "..");
const packagePath = join(root, "src-tauri/native/fluidaudio-parakeet");
const release = process.argv.includes("--release");
const configuration = release ? "release" : "debug";
const target = process.env.TAURI_ENV_TARGET_TRIPLE || "aarch64-apple-darwin";
if (target !== "aarch64-apple-darwin") {
  throw new Error(`FluidAudio speech helper currently supports aarch64-apple-darwin; found ${target}`);
}
prepareFluidAudioCheckout(join(packagePath, ".build", "checkouts", "FluidAudio"));
const build = spawnSync(
  "swift",
  ["build", "--package-path", packagePath, "-c", configuration, "--product", "video-creater-fluidaudio-transcribe"],
  { cwd: root, encoding: "utf8", stdio: "inherit" },
);
if (build.status !== 0) {
  throw new Error(`FluidAudio speech helper build failed with status ${build.status ?? "unknown"}`);
}
const source = join(packagePath, ".build", configuration, "video-creater-fluidaudio-transcribe");
const destination = join(root, "src-tauri/binaries", `video-creater-fluidaudio-transcribe-${target}`);
mkdirSync(dirname(destination), { recursive: true });
copyFileSync(source, destination);
chmodSync(destination, 0o755);
console.log(JSON.stringify({ status: "passed", configuration, target, destination }, null, 2));
