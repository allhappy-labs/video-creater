# Source Probe Cache Design

## Goal

Avoid re-running GStreamer source media discovery for repeated proposal renders when the source file has not changed.

## Design

Add a small source-probe cache under the proposal render output directory. The cache stores:

- schema version
- source path
- source file size
- source file modification timestamp
- serialized `MediaProbe`

`run_render_proposal_with_runner` checks the cache before calling `probe_media_with_gstreamer`. On a hit, it reuses the cached `MediaProbe` and synthesizes a successful `ProcessOutput` whose stdout is the cached probe JSON so render reports keep the same log shape. On a miss, it runs GStreamer discovery, writes the cache, and continues. Missing, stale, or malformed cache data is treated as a miss. Unexpected file IO errors become actionable pipeline errors.

## Non-Goals

- Do not cache final output probes.
- Do not make source validation optional.
- Do not introduce a shared global cache across projects.

## Tests

- Cache miss when metadata is absent.
- Cache hit when source path, file size, mtime, and schema match.
- Cache miss when the source file changes.
- Proposal source probe helper returns cached `ProcessOutput` without calling the probe closure on hit.
