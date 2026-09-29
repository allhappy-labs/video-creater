# Linux License Exception Selection Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Let the existing declaration checker evaluate an explicit MIT choice from the real rustix/linux-raw-sys license expression without discarding or approving exception-bearing obligations.

**Architecture:** Extend the bounded parser with an atomic license-plus-exception term. Recognize only `LLVM-exception`, retaining that term intact for derivation checks; the existing six-license allowlist stays unchanged. This is a tooling prerequisite, not an artifact certification or the Linux port itself.

**Tech Stack:** Node 24 built-ins, existing JavaScript evaluator and TypeScript Node tests; no new dependencies.

**Spec:** `docs/superpowers/specs/2026-09-12-linux-compatibility-design.md`

## Global Constraints

- No LGPL dependencies, including application-linked or loaded system libraries.
- Independent OS services are permitted; app helpers are not exempt.
- Initial eligible licenses remain MIT, Apache-2.0, BSD-2-Clause, BSD-3-Clause, ISC, Zlib.
- An inventory result is declaration-only; native code and toolchain provenance still need independent audit.
- Preserve macOS behavior and unrelated work; work in `.worktrees/linux/permissive-runtime`.
- Sol implementation and independent review, TDD, Conventional Commits; no push or merge.

## Scope and source review

This implements only exception-aware branch selection. Packaging, inventory discovery,
toolchain certification, editor UI, codecs, models, and real desktop acceptance remain
separate work. Native Wayland and media feasibility investigations run independently.

[SPDX expression grammar](https://spdx.github.io/spdx-spec/v2.3/SPDX-license-expressions/)
defines `WITH` as binding a simple license expression to an exception before `AND`/`OR`.
[SPDX's LLVM entry](https://spdx.org/licenses/LLVM-exception.html) identifies the exception
used in the observed Rust dependency declarations. These sources establish parser
semantics, not eligibility of the exception-bearing term. Only the exact canonical
IDs in this bounded parser are supported; it is not a complete SPDX implementation.

### Task 1: Recognize exception-bearing alternatives without relaxing policy

**Files:**
- Modify: `scripts/linux-permissive-preflight.mjs` — bounded parser only.
- Modify: `scripts/linux-permissive-preflight.test.ts` — policy and CLI regression tests.
- Modify: `docs/development/linux-permissive-preflight.md` — supported syntax and limits.

**Interfaces:** Keep `evaluateLinuxInventory(input)` and the CLI schema/status/exit codes unchanged. A `WITH` leaf is represented as the whole canonical string, e.g. `Apache-2.0 WITH LLVM-exception`, never as bare `Apache-2.0`.

- [x] **Step 1:** Add failing tests using the existing component fixture: declaration
  `Apache-2.0 WITH LLVM-exception OR Apache-2.0 OR MIT`, selection `MIT`, must return
  `inventory-eligible`. Add a real CLI case with this declaration. Assert unknown
  exceptions fail even in unselected branches; missing/repeated `WITH`, compound
  `(MIT OR Apache-2.0) WITH LLVM-exception`, and standalone exception IDs fail.

  ```typescript
  const declaration = "Apache-2.0 WITH LLVM-exception OR Apache-2.0 OR MIT";
  const result = evaluateLinuxInventory(inventory([
    { ...app, license: declaration, selectedLicense: "MIT" },
    permissiveLibc,
  ]));
  assert.equal(result.status, "inventory-eligible");
  ```

- [x] **Step 2:** Run `node --test scripts/linux-permissive-preflight.test.ts` and
  retain the expected RED output before implementation in the ignored SDD report.
- [x] **Step 3:** In `parsePrimary`, after validating a license ID and before returning
  its leaf, consume at most one `WITH` and the exact recognized exception ID:

  ```javascript
  if (peek() === "WITH") {
    take();
    const exception = take();
    if (exception !== "LLVM-exception") throw new Error(`unknown exception ID ${exception ?? "<missing>"}`);
    return { type: "license", value: `${token} WITH ${exception}` };
  }
  ```

  Parenthesized compound nodes cannot take `WITH`; leftover tokens fail. Existing
  allowlist lookup denies every selected exception-bearing leaf. Keep existing
  limits and unknown-license checks, including for unselected OR branches.
- [x] **Step 4:** Add adversarial coverage: selecting bare Apache from only Apache
  WITH is not derivable; selecting WITH from bare Apache is not derivable; a selected
  WITH leaf is denied; AND retains exception obligations; nested OR can choose MIT;
  system LGPL still fails. Verify deterministic output and limits stay intact.
- [x] **Step 5:** Document recognition versus eligibility and the real MIT selection
  example. Run `node --test scripts/linux-permissive-preflight.test.ts scripts/release-runtime-policy.test.ts`
  and `git diff --check`. Commit only the three task files with
  `fix: evaluate Linux license alternatives with LLVM exceptions`.
- [x] **Step 6:** Obtain independent Sol review for spec compliance and code quality.
  Route findings through the implementer and rerun relevant tests after fixes.

## Execution record

Implementation and review evidence belongs in the plan-scoped ignored SDD directory.
The controller updates completion here only after reviewing fresh evidence.

Task completed in `a3ed32d8`. RED: 7 expected failures; GREEN: 31 tests passed
including the adjacent runtime-policy suite. Independent Sol task review approved
spec compliance and quality with no findings. Exception-bearing selected terms
remain denied; the eligible license set did not change. Whole-continuation review
will include this change with the native probe work.
