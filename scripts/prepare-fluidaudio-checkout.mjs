import {
  chmodSync,
  existsSync,
  readFileSync,
  statSync,
  writeFileSync,
} from "node:fs";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const benchmarkRelativePath = "ASR/Parakeet/Unified/benchmark.md";
const targetPath = "Sources/FluidAudio";

export function prepareFluidAudioCheckout(checkoutPath) {
  const resolvedCheckout = resolve(checkoutPath);
  const manifestPath = join(resolvedCheckout, "Package.swift");
  const benchmarkPath = join(resolvedCheckout, targetPath, benchmarkRelativePath);

  if (!existsSync(benchmarkPath)) {
    return { status: "not-needed", checkoutPath: resolvedCheckout };
  }

  const manifest = readFileSync(manifestPath, "utf8");
  const exclusion = `exclude: ["${benchmarkRelativePath}"]`;
  if (manifest.includes(exclusion)) {
    return { status: "already-prepared", checkoutPath: resolvedCheckout };
  }

  const pathDeclaration = `path: "${targetPath}"`;
  const matches = manifest.match(new RegExp(pathDeclaration, "g")) ?? [];
  if (matches.length !== 1) {
    throw new Error(
      `Expected one FluidAudio target path declaration in ${manifestPath}; found ${matches.length}`,
    );
  }

  const preparedManifest = manifest.replace(
    pathDeclaration,
    `${pathDeclaration},\n            ${exclusion}`,
  );
  const originalMode = statSync(manifestPath).mode & 0o777;
  try {
    chmodSync(manifestPath, originalMode | 0o200);
    writeFileSync(manifestPath, preparedManifest, "utf8");
  } finally {
    chmodSync(manifestPath, originalMode);
  }
  return { status: "prepared", checkoutPath: resolvedCheckout };
}

function checkoutArgument(argv) {
  const index = argv.indexOf("--checkout");
  if (index < 0 || !argv[index + 1]) {
    throw new Error("Usage: prepare-fluidaudio-checkout.mjs --checkout <path>");
  }
  return argv[index + 1];
}

const invokedPath = process.argv[1] ? resolve(process.argv[1]) : null;
if (invokedPath === fileURLToPath(import.meta.url)) {
  const result = prepareFluidAudioCheckout(checkoutArgument(process.argv.slice(2)));
  console.log(JSON.stringify(result));
}
