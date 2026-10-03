import assert from "node:assert/strict";
import test from "node:test";
import { assessJavaScriptAudit, bracesPatchSha256 } from "./dependency-security.mjs";

const finding = {
  github_advisory_id: "GHSA-vfj7-8cjw-p6xm", module_name: "braces",
  findings: [{ version: "3.0.3", paths: [".>tailwindcss>chokidar>braces"] }],
};
const report = (advisory = finding) => ({ advisories: { one: advisory }, metadata: { vulnerabilities: { high: 1 } }, muted: [] });

test("accepts only the hash-bound, tested mitigation without hiding the upstream finding", () => {
  assert.deepEqual(assessJavaScriptAudit(report(), bracesPatchSha256, true), { unresolved: 0, locallyPatched: 1 });
  assert.throws(() => assessJavaScriptAudit(report(), "changed", true));
  assert.throws(() => assessJavaScriptAudit(report(), bracesPatchSha256, false));
});

test("new advisories, changed dependency paths and incomplete reports fail closed", () => {
  assert.throws(() => assessJavaScriptAudit(report({ ...finding, github_advisory_id: "NEW" }), bracesPatchSha256, true));
  assert.throws(() => assessJavaScriptAudit(report({ ...finding, findings: [{ version: "3.0.3", paths: [".>runtime>braces"] }] }), bracesPatchSha256, true));
  assert.throws(() => assessJavaScriptAudit({ ...report(), metadata: { vulnerabilities: { high: 2 } } }, bracesPatchSha256, true));
  assert.throws(() => assessJavaScriptAudit({ error: "network" }, bracesPatchSha256, true));
});
