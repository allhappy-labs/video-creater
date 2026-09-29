# Frontend fixes cross-review

## Task 2 follow-up — canonical frame recovery

The bounded review of `b9ec9e12` found that a failed prepared-frame image stayed suppressed when a successful rebuild returned the same deterministic frame URL. The local failure state cleared only after the URL disappeared. For a layer requiring canonical preparation, the failed URL was also removed from the coverage set, so the compositor reported a missing frame while `PreviewPanel` independently reported the load failure. That produced two overlapping recovery alerts.

`PreviewPanel` now treats a resolved frame URL as coverage even when that image reports a load error; its existing parent-level error surface remains the single owner of that failure. Retry clears the failed URL before requesting canonical preparation again, which remounts the image even when the rebuilt frame path is unchanged.

TDD evidence:

- Red: `pnpm exec vitest run src/components/workspace/preview-panel.test.tsx -t "retries a rich prepared frame at the same URL with one recovery surface"` failed because two `Preview issue` alerts were rendered instead of one.
- Green focused: seven canonical coverage, flattening, load-failure, retry, and toolbar tests passed.
- Green affected suite: `src/components/workspace/preview-panel.test.tsx` passed 71/71 tests.
- `git diff --check` passed for the changed component and test.

The review found no additional concrete correctness issue in transport ownership, Settings request supersession, tab semantics, or flattened single-composite coverage.
