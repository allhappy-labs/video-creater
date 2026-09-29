# Reviewed LLVM License Term Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Status:** Complete in `4431322c`. Independent source-policy, specification,
and code review accepted the exact-term change with zero findings. Verification
recorded 31 focused and 40 combined passing tests; artifact and package
qualification remain separate gates.

**Goal:** Reflect the explicit permissive-license review of Apache-2.0 WITH LLVM-exception in the declaration checker without approving arbitrary exceptions or artifacts.

**Architecture:** Add exactly one combined atomic term to the existing eligible set. Keep parser, derivation rules, schema, CLI, and declaration-only result semantics unchanged.

**Tech Stack:** Existing Node built-in evaluator and tests, no new dependencies.

**Spec:** `docs/superpowers/specs/2026-09-12-linux-compatibility-design.md`, with the explicit source review `docs/research/2026-09-12-linux-llvm-license-policy.md`.

## Global Constraints

- No LGPL/GPL dependency exemptions, including app-linked system libraries and GNU runtime exceptions.
- Approve only `Apache-2.0 WITH LLVM-exception` in addition to the existing six IDs.
- Full source/binary provenance, third-party files, notices and packaging remain independent obligations.
- No input/config allowlist overrides; retain bounded parsing and all AND obligations.
- Sol implementation and independent review; TDD and Conventional Commits; preserve unrelated files.

### Task 1: Add the explicitly reviewed atomic term

**Files:**
- Modify: `scripts/linux-permissive-preflight.mjs`, `scripts/linux-permissive-preflight.test.ts`.
- Modify: `docs/development/linux-permissive-preflight.md` and current-policy paragraphs in the Linux spec.
- Modify: completion sentence only in `docs/research/2026-09-12-linux-llvm-license-policy.md` after implementation/review evidence.

**Interfaces:** `evaluateLinuxInventory` and CLI contracts stay unchanged. A combined
term remains one leaf; `RECOGNIZED_LICENSES` must continue to contain only simple IDs,
not accidentally treat an eligible combined string as a simple identifier.

- [x] **Step 1:** Read the pinned source-review report and inspect its source texts.
  Raise a concrete contradiction before code changes if the claimed permissive
  classification is unsupported. Do not broaden approval to all LLVM files.
- [x] **Step 2:** Add RED coverage for a selected `Apache-2.0 WITH LLVM-exception`
  leaf becoming eligible and for an AND expression retaining that exact leaf:

  ```typescript
  const term = "Apache-2.0 WITH LLVM-exception";
  const result = evaluateLinuxInventory(inventory([
    { ...app, license: term, selectedLicense: term }, permissiveLibc,
  ]));
  assert.equal(result.status, "inventory-eligible");
  ```

  Replace the former selected-term-denial expectation with its newly approved
  outcome. Add explicit denied `GPL-3.0-only WITH LLVM-exception`, denied
  `MIT WITH LLVM-exception`, unknown-exception, bare/combined derivation mismatch,
  and transitive system-LGPL regression cases. Retain the MIT alternative tests.
- [x] **Step 3:** Add only the combined approved leaf to eligible policy. Keep
  recognized simple IDs separate, for example add the combined leaf to the eligible
  Set after the recognized-ID Set has been constructed. Do not alter the parser to
  discard exception suffixes or grant eligibility based on the base ID alone.
- [x] **Step 4:** Update current docs to distinguish the original six-ID policy,
  the prior recognition-only stage, and the newly reviewed term. Link the source
  review; historical spike reports still accurately state their original result.
- [x] **Step 5:** Run focused tests RED/GREEN, then the combined preflight,
  release-runtime-policy, and host-runner Node gate; run `git diff --check`. Commit
  only task files and obtain independent Sol source-policy/spec/code review.
