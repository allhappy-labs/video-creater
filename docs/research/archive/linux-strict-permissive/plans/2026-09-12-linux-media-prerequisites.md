# Linux Media Prerequisites Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Status:** Complete. Task 1 was recorded in `35a4d2ed` and qualified in
`07ca2b01`; Task 2 was recorded in `b737c208`. Scoped independent evidence
reviews accepted both tasks; the final codec report had zero findings. These are
prerequisite proofs, not production media-backend or release qualification.

**Goal:** Resolve the concrete C/C++ runtime and MP4 timing/seek failures found in the first media feasibility report before choosing the production backend.

**Architecture:** Two independent development probes investigate the existing OpenH264/libxaac candidates with a permissive target runtime, and a replacement Rust MP4 container implementation. Both retain pinned sources, build commands, and actual output evidence. A combined CPU MP4 proof was deferred until both gates had concrete results; those prerequisite results are now recorded, while the combined proof remains separate.

**Tech Stack:** OpenH264 2.6.0, libxaac 0.1.13, static x86_64 musl, a source-reviewed LLVM C++ runtime candidate, and a pinned Rust MP4 candidate. No production application dependency changes.

**Spec:** `docs/superpowers/specs/2026-09-12-linux-compatibility-design.md`

## Global Constraints

- App helpers and every linked/loaded library must be permissively licensed; no GNU C++/GCC runtime exception exemption.
- Compiler executables may be development prerequisites; generated runtime objects must be audited separately.
- No system package installation, privileged operations, host mounts, shared network listeners, or changes outside private worktree output.
- No install-time codec download, host-codec fallback, or reduction of the required common H.264/AAC MP4 functionality.
- Preserve earlier proofs and binaries; use separate output directories.
- Sol workers, independent review, exact source pins, actual build/runtime evidence, Conventional Commits.
- Source terms, functional execution, and complete artifact qualification are distinct results. Do not conflate them.

## Scope

Read `docs/research/2026-09-12-linux-media-feasibility.md`. The first stack had
three concrete failures: GNU startup objects in AAC testbench executables,
OpenH264's default GNU C++ runtime linkage, and minimp4's missing composition
offset/sync-sample support. These tasks investigate fixes rather than repeat
the same failed build or treat the initial six-ID allowlist as a user ban on
other explicitly reviewed permissive terms.

### Task 1: Target codec runtime proof without known GNU runtime objects

**Files:**
- Create ignored: `output/linux-codec-runtime-proof/` — pinned source/builds and evidence.
- Create: `docs/research/2026-09-12-linux-codec-runtime-proof.md` — exact outcomes and remaining gaps.

**Interfaces:** Input is the existing pinned OpenH264/libxaac source and original
link-map findings. Output is a source/build recipe and measured codec test results
with final link inputs, or a concrete failed build and diagnosis. No application API.

- [x] **Step 1:** Verify the retained source pins. Read official LLVM runtime license
  texts and target-build documentation before selecting an exact libc++/libc++abi/
  unwind candidate. Record hashes and which runtime components are actually needed.
- [x] **Step 2:** Attempt private target builds with extracted development tools if
  necessary. Use the existing musl sysroot; do not reuse glibc-built C++ runtime
  archives as target evidence. Preserve all exact compiler/linker invocations.
- [x] **Step 3:** Link the AAC archives through Rust/musl or another inspected target
  recipe that removes the demonstrated GCC startup objects. Exercise the distinct
  encoder and decoder using the retained AAC-LC fixture. Record status, stream shape,
  duration/delay limits, ELF properties, hash and final link map.
- [x] **Step 4:** Build OpenH264 against the selected target C++ runtime and exercise
  CPU I420 encode then decode, using a tiny generated deterministic frame sequence.
  Check decoded dimensions/frame count and nonconstant content. Record whether this
  is same-library roundtrip only; it is not independent conformance validation.
- [x] **Step 5:** Audit final linked objects/archives for GNU CRT, libstdc++, libgcc,
  glibc, unresolved symbols, and dynamic imports. Identify exact provenance gaps in
  rustup-supplied objects instead of inferring licenses from their directory names.
- [x] **Step 6:** Commit only the report; obtain independent Sol evidence review.
  If a build fails, record the exact error and plausible next repair. No speculative
  full-backend implementation or unmeasured eligibility claim.

### Task 2: MP4 presentation timing and sync-sample proof

**Files:**
- Create ignored: `output/linux-mp4-timing-proof/` — standalone pinned crate and fixtures.
- Create: `docs/research/2026-09-12-linux-mp4-timing-proof.md` — source selection and measured behavior.

**Interfaces:** Investigate `alfg/mp4-rust` as an unqualified candidate. Required
sample semantics are decode time, signed composition offset, presentation time,
duration, byte offset/length, and sync flag. A seeking adapter must choose the
preceding sync sample then decode forward; selecting a sample by timestamp alone
does not establish usable media seeking.

- [x] **Step 1:** Verify an exact released crate/source revision, full license texts,
  dependency graph and APIs from primary sources. Reject any required nonpermissive
  library. Record source evidence for composition offsets, sync tables and edit lists.
- [x] **Step 2:** Create an actual standalone musl test using a synthetic MP4 with
  known decode/presentation ordering and sync samples, plus an existing valid H.264/
  AAC MP4 fixture if one is available with clear provenance. Identify synthetic
  encoded payloads as container-only tests; never claim decoder acceptance for them.
- [x] **Step 3:** Assert expected table values, signed offsets where supported, and
  preceding-sync selection at multiple requested times. Test malformed/truncated
  input with bounded resources and report errors/panics rather than hide failures.
  Write failing tests before any local adapter or upstream repair.
- [x] **Step 4:** Build/run `--locked --target x86_64-unknown-linux-musl`; record
  artifact hash, ELF evidence, Cargo graph/licenses, and actual assertion outcomes.
  Clarify whether edits, fragmented MP4, rotation, VFR, large offsets and decoder
  reordering were covered. Missing format coverage is an open gate, not a pass.
- [x] **Step 5:** Commit only the report; obtain independent Sol evidence review.
  Report whether this candidate resolves the two minimp4 failures and exactly what
  a combined codec/container proof must still establish.
