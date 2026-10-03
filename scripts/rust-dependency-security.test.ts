import assert from "node:assert/strict";
import test from "node:test";
import { assessRustAudit, verifyGlibBackport } from "./rust-dependency-security.mjs";

const clean = { database: { "last-commit": "fresh" }, settings: { ignore: [] }, vulnerabilities: { list: [], count: 0, found: false }, warnings: { unmaintained: [{}] } };
test("Rust security gate retains maintenance warnings and rejects vulnerabilities, unsoundness and audit ignores", () => {
  assert.deepEqual(assessRustAudit(clean), { vulnerabilities: 0, unmaintained: 1 });
  assert.throws(() => assessRustAudit({ ...clean, vulnerabilities: { list: [{}], count: 1, found: true } }));
  assert.throws(() => assessRustAudit({ ...clean, warnings: { unsound: [{}] } }));
  assert.throws(() => assessRustAudit({ ...clean, settings: { ignore: ["ANY"] } }));
  assert.throws(() => assessRustAudit({ error: "network" }));
});

test("reviewed GTK3 compatibility backport matches its complete source inventory", () => {
  assert.equal(verifyGlibBackport(), true);
});
