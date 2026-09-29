// Audio denoise evidence for the Linux desktop smoke run, kept apart from the driver script so it
// can be unit tested.

import { existsSync, readFileSync } from "node:fs";
import { join } from "node:path";

const denoiseManifestPattern = /^cache\/audio-denoise\/v1\/sha256\/[0-9a-f]{2}\/[0-9a-f]{64}\/manifest\.json$/;

/**
 * The committed audio denoise cache entries a render's pipeline report lists as artifacts. The render
 * lists the manifest and output of every precompose stage it rendered from, so an entry here means
 * the rendered project used the denoised audio. Staging directories and cache folders that merely
 * exist never count, and an entry only counts when its manifest and output are on disk.
 */
export function audioDenoiseEvidence(projectDir, pipelineReport) {
  const artifacts = Array.isArray(pipelineReport?.artifacts) ? pipelineReport.artifacts : [];
  return artifacts
    .filter((path) => typeof path === "string" && denoiseManifestPattern.test(path))
    .flatMap((manifestPath) => {
      const outputPath = manifestPath.replace(/manifest\.json$/, "output.wav");
      if (!artifacts.includes(outputPath)) return [];
      if (!existsSync(join(projectDir, manifestPath)) || !existsSync(join(projectDir, outputPath))) return [];
      const manifest = JSON.parse(readFileSync(join(projectDir, manifestPath), "utf8"));
      return [
        {
          manifest: manifestPath,
          output: outputPath,
          algorithm: manifest.algorithm ?? null,
          noiseRmsBefore: manifest.noiseRmsBefore ?? null,
          noiseRmsAfter: manifest.noiseRmsAfter ?? null,
        },
      ];
    });
}
