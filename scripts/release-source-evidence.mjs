import { spawnSync } from "node:child_process";
import { createHash } from "node:crypto";
import { existsSync, readFileSync, readdirSync, statSync, writeFileSync } from "node:fs";
import { join, resolve } from "node:path";

const CONTROLLER_OWNED_DIRTY_PATHS = Object.freeze([
  ".superpowers/sdd/progress.md",
]);

export function collectReleaseSourceEvidence({ repoRoot, runCommand = run }) {
  const commit = checkedOutput(runCommand, repoRoot, ["rev-parse", "HEAD"], "source commit");
  if (!/^[0-9a-f]{40}$/.test(commit)) {
    throw new Error(`release source commit is invalid: ${commit || "<missing>"}`);
  }
  const branch = checkedOutput(
    runCommand,
    repoRoot,
    ["branch", "--show-current"],
    "source branch",
    { allowEmpty: true },
  );
  const status = checkedOutput(
    runCommand,
    repoRoot,
    ["status", "--porcelain=v1", "--untracked-files=all"],
    "source status",
    { allowEmpty: true, preserveWhitespace: true },
  );
  const dirtyPaths = status
    .split("\n")
    .map((line) => line.trimEnd())
    .filter(Boolean)
    .map(statusPath);
  const controllerOwnedDirtyPaths = dirtyPaths.filter((path) =>
    CONTROLLER_OWNED_DIRTY_PATHS.includes(path),
  );
  const taskOwnedDirtyPaths = dirtyPaths.filter(
    (path) => !CONTROLLER_OWNED_DIRTY_PATHS.includes(path),
  );
  if (taskOwnedDirtyPaths.length > 0) {
    throw new Error(
      `release source contains task-owned changes: ${taskOwnedDirtyPaths.join(", ")}`,
    );
  }
  return {
    commit,
    shortCommit: commit.slice(0, 12),
    branch: branch || null,
    dirtyStatePolicy:
      "Only .superpowers/sdd/progress.md may be dirty as controller-owned state; all task-owned source must be committed.",
    controllerOwnedDirtyPaths,
    taskOwnedDirtyPaths,
  };
}

export function assertReleaseSourceStable(before, after) {
  if (before.commit !== after.commit) {
    throw new Error(
      `release source commit changed during packaging: ${before.commit} -> ${after.commit}`,
    );
  }
  if (JSON.stringify(before.taskOwnedDirtyPaths) !== JSON.stringify(after.taskOwnedDirtyPaths)) {
    throw new Error("release source dirty state changed during packaging");
  }
}

function artifactHash(path) {
  return createHash("sha256").update(readFileSync(path)).digest("hex");
}

function inputSnapshot(inputPaths) {
  const visit = (path) => statSync(path).isDirectory()
    ? readdirSync(path).sort().flatMap((name) => visit(join(path, name)))
    : [{ path, sha256: artifactHash(path) }];
  return inputPaths.map((path) => resolve(path)).sort().flatMap(visit);
}

export function writeArtifactSourceEvidence({ artifactPath, source, inputPaths = [] }) {
  if (!/^[0-9a-f]{40}$/.test(source.commit) || source.taskOwnedDirtyPaths?.length) {
    throw new Error("artifact source evidence requires committed clean source");
  }
  const evidence = { schemaVersion: 1, source, sha256: artifactHash(artifactPath), inputs: inputSnapshot(inputPaths) };
  writeFileSync(`${artifactPath}.source.json`, `${JSON.stringify(evidence, null, 2)}\n`);
  return evidence;
}

export function verifyArtifactSourceEvidence({ artifactPath, source, inputPaths = [] }) {
  const path = `${artifactPath}.source.json`;
  if (!existsSync(path)) throw new Error(`artifact source evidence is missing: ${path}; rebuild from committed source`);
  const evidence = JSON.parse(readFileSync(path, "utf8"));
  if (evidence.schemaVersion !== 1 || evidence.source?.commit !== source.commit || evidence.source?.taskOwnedDirtyPaths?.length) {
    throw new Error(`artifact source commit does not match clean current source: ${artifactPath}`);
  }
  if (evidence.sha256 !== artifactHash(artifactPath)) throw new Error(`artifact hash differs from build source evidence: ${artifactPath}`);
  if (JSON.stringify(evidence.inputs) !== JSON.stringify(inputSnapshot(inputPaths))) {
    throw new Error(`package payload hashes or inputs differ from build source evidence: ${artifactPath}`);
  }
  return evidence;
}

function statusPath(line) {
  const path = line.slice(3).trim();
  const renamed = path.includes(" -> ") ? path.split(" -> ").at(-1) : path;
  return unquoteGitPath(renamed ?? path);
}

function unquoteGitPath(path) {
  if (!(path.startsWith('"') && path.endsWith('"'))) return path;
  try {
    return JSON.parse(path);
  } catch {
    return path;
  }
}

function checkedOutput(
  runCommand,
  repoRoot,
  args,
  label,
  { allowEmpty = false, preserveWhitespace = false } = {},
) {
  const result = runCommand("git", args, { cwd: repoRoot });
  if (result.error || result.status !== 0) {
    throw new Error(
      `could not inspect release ${label}: ${result.error?.message || result.stderr || "git failed"}`,
    );
  }
  const rawOutput = String(result.stdout ?? "");
  const output = preserveWhitespace ? rawOutput : rawOutput.trim();
  if (!allowEmpty && !output) {
    throw new Error(`release ${label} is empty`);
  }
  return output;
}

function run(command, args, options) {
  return spawnSync(command, args, {
    cwd: options.cwd,
    encoding: "utf8",
    stdio: "pipe",
  });
}
