import { createHash } from "node:crypto";
import { readFileSync } from "node:fs";
import { spawnSync } from "node:child_process";
import { fileURLToPath } from "node:url";

export const bracesPatchSha256 = "711acefd979058cb0a10af9ebfc2197d9796782cd5c945e6366bb001272b7d31";

export function assessJavaScriptAudit(audit, patchHash, regressionPassed) {
  if (!audit?.advisories || !audit?.metadata?.vulnerabilities || audit.error) {
    throw new Error("Dependency audit did not return a complete report.");
  }
  const findings = Object.values(audit.advisories);
  const count = Object.values(audit.metadata.vulnerabilities).reduce((total, value) => total + value, 0);
  if (count !== findings.length || audit.muted?.length) throw new Error("Incomplete or muted audit findings.");
  for (const finding of findings) {
    const patched = finding.github_advisory_id === "GHSA-vfj7-8cjw-p6xm"
      && finding.module_name === "braces"
      && finding.findings?.length > 0
      && finding.findings.every(item => item.version === "3.0.3"
        && item.paths?.length > 0
        && item.paths.every(path => path === ".>tailwindcss>chokidar>braces"))
      && patchHash === bracesPatchSha256 && regressionPassed;
    if (!patched) throw new Error(`Unresolved advisory: ${finding.github_advisory_id ?? finding.id}`);
  }
  return { unresolved: 0, locallyPatched: findings.length };
}

if (process.argv[1] === fileURLToPath(import.meta.url)) {
  const audit = spawnSync("pnpm", ["audit", "--json"], { encoding: "utf8", timeout: 120_000 });
  if (audit.error || ![0, 1].includes(audit.status)) throw new Error("Dependency audit failed to execute.");
  const regression = spawnSync(process.execPath, ["--test", "scripts/braces-depth-security.test.ts"], { encoding: "utf8", timeout: 30_000 });
  if (regression.status !== 0) process.stderr.write(regression.stdout + regression.stderr);
  const hash = createHash("sha256").update(readFileSync("patches/braces@3.0.3.patch")).digest("hex");
  const result = assessJavaScriptAudit(JSON.parse(audit.stdout), hash, regression.status === 0);
  console.log(`JavaScript security: ${result.unresolved} unresolved; ${result.locallyPatched} upstream advisory mitigated by the verified local braces depth patch.`);
}
