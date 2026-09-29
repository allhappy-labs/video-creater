import { spawnSync } from "node:child_process";

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
