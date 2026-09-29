# Source Probe Cache Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Reuse validated source media probes for repeated proposal renders when the source file signature has not changed.

**Architecture:** Add a `source_probe_cache` module that validates a file signature before returning a cached `MediaProbe`. Integrate it into `run_render_proposal_with_runner` with a manual `sourceProbe` performance stage so cache hit/miss status is visible in render reports.

**Tech Stack:** Rust, serde JSON, filesystem metadata, Cargo render-pipeline tests.

---

## File Structure

- Create `src-tauri/src/render_pipeline/source_probe_cache.rs`: source signature, cache record, cache hit/miss validation, cache write.
- Modify `src-tauri/src/render_pipeline/mod.rs`: export the module.
- Modify `src-tauri/src/render_pipeline/proposal.rs`: check cache before GStreamer source discovery and report cache status in `sourceProbe` details.
- Modify `src-tauri/tests/render_pipeline.rs`: add cache and source-probe helper tests.

---

### Task 1: Cache Module

- [x] **Step 1: Write failing cache module tests**

Add tests for missing metadata, matching cache hit, and stale source size.

- [x] **Step 2: Run cache tests to verify they fail**

Run:

```bash
rtk cargo test --manifest-path src-tauri/Cargo.toml source_probe_cache --test render_pipeline -- --test-threads=1
```

Expected: compile failure because the module does not exist.

- [x] **Step 3: Implement `source_probe_cache.rs`**

Create the module with `SourceProbeCacheLookup`, `validate_source_probe_cache_hit`, and `write_source_probe_cache`.

- [x] **Step 4: Run cache tests**

Run the same cargo command. Expected: PASS.

### Task 2: Proposal Integration

- [x] **Step 1: Write failing helper test**

Add a test proving `probe_source_video_with_cache` returns a cached probe and does not call the fallback probe closure on a hit.

- [x] **Step 2: Run helper test to verify it fails**

Run:

```bash
rtk cargo test --manifest-path src-tauri/Cargo.toml probe_source_video_with_cache_uses_cached_probe_without_running_gstreamer --test render_pipeline -- --test-threads=1
```

Expected: compile failure because the helper does not exist.

- [x] **Step 3: Implement helper and wire `run_render_proposal_with_runner`**

Add `probe_source_video_with_cache` to `proposal.rs`, then replace the direct `probe_media_with_gstreamer` source probe call with the cache-aware helper. Record `cacheStatus` in the `sourceProbe` performance stage details.

- [x] **Step 4: Run helper test**

Run the same cargo command. Expected: PASS.

### Task 3: Verification and Commit

- [x] **Step 1: Run focused tests**

Run:

```bash
rtk cargo test --manifest-path src-tauri/Cargo.toml source_probe_cache --test render_pipeline -- --test-threads=1
rtk cargo test --manifest-path src-tauri/Cargo.toml probe_source_video_with_cache_uses_cached_probe_without_running_gstreamer --test render_pipeline -- --test-threads=1
```

- [x] **Step 2: Run full render-pipeline suite**

Run:

```bash
rtk cargo test --manifest-path src-tauri/Cargo.toml --test render_pipeline -- --test-threads=1
```

- [x] **Step 3: Review diff and commit**

Run:

```bash
rtk rustfmt --check src-tauri/src/render_pipeline/proposal.rs src-tauri/src/render_pipeline/source_probe_cache.rs src-tauri/tests/render_pipeline.rs
rtk git diff --check
rtk git diff --stat
rtk git status --short
```

Commit:

```bash
rtk git add docs/superpowers/specs/2026-06-19-source-probe-cache-design.md docs/superpowers/plans/2026-06-19-source-probe-cache.md src-tauri/src/render_pipeline/source_probe_cache.rs src-tauri/src/render_pipeline/mod.rs src-tauri/src/render_pipeline/proposal.rs src-tauri/tests/render_pipeline.rs
rtk git commit -m "perf: cache source media probes"
```
