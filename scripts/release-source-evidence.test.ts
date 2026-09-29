import assert from "node:assert/strict";
import test from "node:test";

import {
  assertReleaseSourceStable,
  collectReleaseSourceEvidence,
} from "./release-source-evidence.mjs";

const COMMIT = "0123456789abcdef0123456789abcdef01234567";

function runner(status = "") {
  return (_command: string, args: string[]) => {
    const key = args.join(" ");
    if (key === "rev-parse HEAD") return success(`${COMMIT}\n`);
    if (key === "branch --show-current") return success("main\n");
    if (key === "status --porcelain=v1 --untracked-files=all") {
      return success(status);
    }
    return { status: 1, stdout: "", stderr: `unexpected command: ${key}` };
  };
}

function success(stdout: string) {
  return { status: 0, stdout, stderr: "" };
}

test("release source evidence records the exact clean main commit", () => {
  const evidence = collectReleaseSourceEvidence({
    repoRoot: "/repo",
    runCommand: runner(),
  });

  assert.deepEqual(evidence, {
    commit: COMMIT,
    shortCommit: COMMIT.slice(0, 12),
    branch: "main",
    dirtyStatePolicy:
      "Only .superpowers/sdd/progress.md may be dirty as controller-owned state; all task-owned source must be committed.",
    controllerOwnedDirtyPaths: [],
    taskOwnedDirtyPaths: [],
  });
});

test("release source evidence allows only controller-owned progress state", () => {
  const evidence = collectReleaseSourceEvidence({
    repoRoot: "/repo",
    runCommand: runner(" M .superpowers/sdd/progress.md\n"),
  });

  assert.deepEqual(evidence.controllerOwnedDirtyPaths, [
    ".superpowers/sdd/progress.md",
  ]);
});

test("release source evidence rejects task-owned changes", () => {
  assert.throws(
    () =>
      collectReleaseSourceEvidence({
        repoRoot: "/repo",
        runCommand: runner(" M src/App.tsx\n?? scripts/new-check.mjs\n"),
      }),
    /task-owned changes: src\/App\.tsx, scripts\/new-check\.mjs/,
  );
});

test("release packaging fails if the source commit changes", () => {
  const before = collectReleaseSourceEvidence({
    repoRoot: "/repo",
    runCommand: runner(),
  });
  const after = { ...before, commit: "f".repeat(40) };

  assert.throws(() => assertReleaseSourceStable(before, after), /commit changed/);
});
