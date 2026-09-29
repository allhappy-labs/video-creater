import { existsSync, readdirSync, statSync } from "node:fs";
import { spawnSync } from "node:child_process";
import { dirname, join, relative, resolve, sep } from "node:path";
import { fileURLToPath } from "node:url";

const excludedDirectories = new Set([
  ".git",
  ".pnpm-store",
  "node_modules",
  "output",
  "resources",
  "src",
  "target",
  "target-current-release",
  "target-final-release",
  "vendor",
]);

function toPosix(path) {
  return path.split(sep).join("/");
}

function toolingFiles(root) {
  const files = [];
  const visit = (directory) => {
    const entries = readdirSync(directory, { withFileTypes: true }).sort((left, right) =>
      left.name.localeCompare(right.name),
    );
    for (const entry of entries) {
      const path = join(directory, entry.name);
      if (entry.isDirectory()) {
        if (!excludedDirectories.has(entry.name)) visit(path);
        continue;
      }
      if (entry.isFile() && (entry.name.endsWith(".mjs") || entry.name.endsWith(".js"))) {
        files.push(path);
      }
    }
  };
  visit(root);
  return files;
}

export function checkToolingSource(root) {
  const resolvedRoot = resolve(root);
  const files = toolingFiles(resolvedRoot);
  const failures = [];
  for (const path of files) {
    const result = spawnSync(process.execPath, ["--check", path], { encoding: "utf8" });
    if (result.status !== 0) {
      const displayPath = toPosix(relative(resolvedRoot, path));
      const detail = result.stderr.trim();
      failures.push(`${displayPath}: syntax check failed${detail ? `\n${detail}` : ""}`);
    }
  }
  return { failures, fileCount: files.length };
}

function parseRoot(argv) {
  const rootIndex = argv.indexOf("--root");
  if (rootIndex === -1) return resolve(dirname(fileURLToPath(import.meta.url)), "..");
  const root = argv[rootIndex + 1];
  if (!root) throw new Error("--root requires a directory path");
  const resolvedRoot = resolve(root);
  if (!existsSync(resolvedRoot) || !statSync(resolvedRoot).isDirectory()) {
    throw new Error(`tooling root is not a directory: ${root}`);
  }
  return resolvedRoot;
}

function main() {
  try {
    const root = parseRoot(process.argv.slice(2));
    const result = checkToolingSource(root);
    if (result.failures.length > 0) {
      process.stderr.write(`${result.failures.join("\n")}\n`);
      process.exitCode = 1;
      return;
    }
    process.stdout.write(`tooling source check passed for ${result.fileCount} files\n`);
  } catch (error) {
    process.stderr.write(`${error instanceof Error ? error.message : String(error)}\n`);
    process.exitCode = 2;
  }
}

if (process.argv[1] && resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  main();
}
