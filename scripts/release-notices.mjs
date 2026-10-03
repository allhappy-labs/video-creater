import { copyFileSync, existsSync, mkdirSync, readFileSync } from "node:fs";
import { dirname, join } from "node:path";

/** Use the same component notice inventory for desktop and headless packages. */
export function releaseNoticeResources(repoRoot) {
  const config = JSON.parse(readFileSync(join(repoRoot, "src-tauri/tauri.conf.json"), "utf8"));
  return Object.entries(config.bundle.resources).filter(([, target]) =>
    target.startsWith("precompose-runtime/") || target.startsWith("audio-runtime/") || target.startsWith("glib-runtime/") || target === "codex-runtime/LICENSE-APACHE-2.0.txt" || target === "codex-runtime/provenance.json",
  );
}

export function stageReleaseNotices({ repoRoot, output }) {
  for (const [source, target] of releaseNoticeResources(repoRoot)) {
    const destination = join(output, "lib/Video Creater", target);
    mkdirSync(dirname(destination), { recursive: true });
    copyFileSync(join(repoRoot, "src-tauri", source), destination);
  }
}

export function missingReleaseNotices({ repoRoot, resourceRoot }) {
  return releaseNoticeResources(repoRoot).flatMap(([, target]) => existsSync(join(resourceRoot, target)) ? [] : [target]);
}
