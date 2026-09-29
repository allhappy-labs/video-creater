#!/usr/bin/env node

import { spawnSync } from "node:child_process";

const metadataResult = spawnSync(
  "cargo",
  ["metadata", "--locked", "--format-version", "1", "--manifest-path", "src-tauri/Cargo.toml"],
  { cwd: process.cwd(), encoding: "utf8", maxBuffer: 64 * 1024 * 1024 },
);
if (metadataResult.status !== 0) {
  process.stderr.write(metadataResult.stderr);
  process.exit(metadataResult.status ?? 1);
}

const metadata = JSON.parse(metadataResult.stdout);
const packagesById = new Map(metadata.packages.map((pkg) => [pkg.id, pkg]));
const nodesById = new Map((metadata.resolve?.nodes || []).map((node) => [node.id, node]));
const roots = metadata.packages
  .filter((pkg) =>
    ["video-creater-precompose-protocol", "video-creater-precompose-worker"].includes(pkg.name),
  )
  .map((pkg) => pkg.id);
const reachable = new Set();
const queue = [...roots];
while (queue.length > 0) {
  const id = queue.shift();
  if (reachable.has(id)) continue;
  reachable.add(id);
  for (const dependency of nodesById.get(id)?.deps || []) {
    if (dependency.dep_kinds?.some(({ kind }) => kind !== "dev")) queue.push(dependency.pkg);
  }
}

const restrictive = /(^|[^A-Z])(AGPL|CDDL|EPL|GPL|LGPL|MPL|SSPL)(-|[^A-Z]|$)|Commons Clause/i;
const failures = [];
const licenses = [];
for (const id of [...reachable].sort()) {
  const pkg = packagesById.get(id);
  if (!pkg) continue;
  const license = pkg.license?.trim();
  licenses.push({ name: pkg.name, version: pkg.version, license: license || null });
  if (!license) failures.push(`${pkg.name} ${pkg.version} has no SPDX license expression`);
  else if (
    restrictive.test(license) &&
    !license
      .split(/\s+OR\s+/i)
      .some((alternative) => !restrictive.test(alternative))
  ) {
    failures.push(`${pkg.name} ${pkg.version} has a forbidden license expression: ${license}`);
  }
}

const result = {
  status: failures.length === 0 ? "passed" : "failed",
  packageCount: licenses.length,
  failures,
  licenses,
};
console.log(JSON.stringify(result, null, 2));
if (failures.length > 0) process.exitCode = 1;
