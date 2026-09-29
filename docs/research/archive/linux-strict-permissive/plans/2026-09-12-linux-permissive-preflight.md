# Linux Permissive Runtime Preflight Implementation Plan

> Historical completed preflight plan. The later approved option 1 boundary
> permits independent OS services; its former service-based blocker is superseded.
> The evaluator remains applicable unchanged to application/helper libraries.
> Follow the current specification for the authorized native-host feasibility spike.

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Implement the first dependency-policy gate and record whether the spec's desktop/runtime prerequisite has a proven path.

**Architecture:** A development-only Node tool validates an explicit runtime dependency inventory and rejects forbidden or unresolved entries. It never certifies a package: actual binary discovery, hashes, model quality, and desktop acceptance remain separate required gates. A parallel read-only feasibility investigation determines whether downstream implementation can start without guessing.

**Tech Stack:** Node 24 built-ins, existing `.mjs` tooling and `.test.ts` Node test convention. No new dependencies.

**Spec:** `docs/superpowers/specs/2026-09-12-linux-compatibility-design.md`

## Global Constraints

- **no LGPL dependencies, including system libraries**.
- Every application utility and required runtime is bundled; no user-installed tool fallback.
- No release-readiness or artifact-compliance claim from a declared inventory alone.
- Existing macOS paths and release gates remain unchanged.
- Work only in `/home/olhapi/projects/video-creater/.worktrees/linux/permissive-runtime`.
- Use Sol implementers and reviewers; do not spawn additional agents from workers.
- No runtime implementation after the desktop/runtime feasibility gate fails.

## Scope and remaining spec coverage

This is the first subproject, covering license-policy evaluation and recording
desktop/runtime feasibility. It does not implement the Linux host, media stack,
model workers, secure desktop services, or release package. Their implementation
plans require a proven runtime selection under the spec. The historical Linux
port plan is explicitly not executable under the confirmed policy.

### Task 1: Fail-closed declared dependency inventory preflight

**Files:**
- Create: `scripts/linux-permissive-preflight.mjs` — pure inventory evaluator and CLI.
- Create: `scripts/linux-permissive-preflight.test.ts` — focused Node behavior tests.
- Create: `docs/development/linux-permissive-preflight.md` — schema, invocation, limits.

**Interfaces:**
- Consumes JSON `{ schemaVersion: 1, roots: string[], components: Component[] }`.
- Component fields: `id`, `name`, `version`, `sourceUrl`, `license`, `selectedLicense`,
  `classification`, `dependencies`. All strings are nonempty; URLs must be HTTPS;
  classification is `bundled` or `system`; dependencies is an array of IDs.
- Produces `evaluateLinuxInventory(input)` returning
  `{ status: "blocked" | "inventory-eligible", failures: string[], componentCount: number,
  limitations: string[] }`. `inventory-eligible` never means Linux compatible or
  package audited. Limitations must always state declaration-only, no artifact
  verification, no runtime/desktop acceptance.
- CLI: `node scripts/linux-permissive-preflight.mjs inventory.json`, JSON stdout;
  exit 0 only for inventory-eligible, 1 for policy violations, 2 for invalid CLI/I/O/JSON.

- [x] **Step 1: Write failing behavior tests.** Start with an eligible real object
  and a transitive LGPL system library. Use the evaluator directly, no mocks:

```typescript
const app = { id: "app", name: "app", version: "1", sourceUrl: "https://example.org/app",
  license: "MIT", selectedLicense: "MIT", classification: "bundled", dependencies: ["libc"] };
const libc = { id: "libc", name: "glibc", version: "2.39", sourceUrl: "https://www.gnu.org/s/libc/",
  license: "LGPL-2.1-or-later", selectedLicense: "LGPL-2.1-or-later",
  classification: "system", dependencies: [] };
assert.equal(evaluateLinuxInventory({ schemaVersion: 1, roots: ["app"], components: [app, libc] }).status, "blocked");
```

  Cover missing/empty inventory, invalid schema/types, duplicate IDs, unknown root
  or dependency, disconnected forbidden entries, dependency cycles, unknown license,
  forbidden system and bundled components, malformed expressions, AND obligations,
  valid OR choice, false selected license, CLI malformed JSON/nonexistent file,
  and real CLI success/blocked exits using temporary inventory files.
- [x] **Step 2: Run `node --test scripts/linux-permissive-preflight.test.ts` and
  record the expected RED failure before implementation.**
- [x] **Step 3: Implement the evaluator and CLI.** Follow this algorithm:

```text
Validate the entire object and every field before graph traversal.
Reject empty roots/components, duplicate IDs, and unresolved graph references.
Validate every declared component, including disconnected ones; cycles may exist
in shared-library graphs and must terminate traversal without failing solely for a cycle.
Allow only MIT, Apache-2.0, BSD-2-Clause, BSD-3-Clause, ISC, Zlib.
Parse license expressions using IDs, AND, OR, parentheses (AND binds tighter).
Fail closed for unknown tokens, WITH exceptions, or malformed expressions.
Treat selectedLicense as the chosen expression: it may resolve OR alternatives
but must retain all AND obligations and must derive from the declared expression.
Every selected license leaf must be on the fixed allowlist.
Do not accept CLI/config supplied allowlist overrides or system exemptions.
Return deterministic actionable failure messages identifying component IDs.
Always include declaration-only limitations; never report status passed/compatible.
```

  Keep the parser/evaluator compact; if selection verification becomes unwieldy,
  use enumerated permitted license sets with bounded expression size/complexity
  and fail closed on excess. Do not evaluate user text as code. CLI reads only the
  explicitly provided file and never installs, executes, or probes a component.
- [x] **Step 4: Run focused tests and the existing adjacent baseline:**
  `node --test scripts/linux-permissive-preflight.test.ts scripts/release-runtime-policy.test.ts`.
  Record command/output and run `git diff --check`. Explain test-only fixtures
  are declarations rather than actual redistributable dependency evidence.
- [x] **Step 5: Document the schema, examples, exact statuses, exit codes, and
  limitations; commit only the three task files with `feat: add Linux permissive inventory preflight`.**

### Task 2: Record the desktop feasibility decision and review the complete slice

**Files:**
- Create: `docs/research/2026-09-12-linux-runtime-feasibility.md`.
- Modify: this plan's task checkboxes and spec evidence status only as supported.

**Interfaces:** Consumes Sol's primary-source/runtime findings and Task 1's tested
CLI; produces a reproducible pass/blocked report, never an unsupported host choice.

- [x] **Step 1:** Record exact sources, current checkout, current host evidence,
  and any dependency path that invalidates a proposed host. Separate source
  conclusions from actual binary/runtime tests. Do not execute untrusted binaries.
- [x] **Step 2:** If a complete viable host path is established, write its concrete
  proof implementation plan before changing app behavior. Otherwise mark downstream
  work blocked by the spec's prerequisite and list the precise decision needed.
- [x] **Step 3:** Obtain Sol task review (spec compliance and quality), then Sol
  whole-branch review. Resolve findings through the implementer with scoped tests.
- [x] **Step 4:** Commit reviewed evidence; retain the isolated branch and report
  implemented versus blocked scope. No automatic main merge or release.

## Execution result

Preflight implementation and evidence review completed on `feat/linux-permissive-runtime`.
Sol task reviews and final whole-branch review found no remaining critical or
important findings. The focused preflight and adjacent runtime-policy suites
passed 24 tests. One editorial minor is retained: feasibility-report metadata
renders as one paragraph. No Linux application, package, or release was built.
Downstream work remains blocked by the required desktop-service dependency closure.
The branch and ignored execution ledger are preserved; nothing was merged or pushed.
