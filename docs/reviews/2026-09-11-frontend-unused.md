# Frontend unused-code review

Date: 2026-09-11  
Scope: React/TypeScript frontend and its npm dependencies  
Tool: Knip 6.35.1 via `npx`

Raw command output is preserved in [evidence/2026-09-11-knip.txt](evidence/2026-09-11-knip.txt). Both commands exited `1` because Knip found issues; the process itself completed normally.

## Commands and interpretation

The baseline was:

```sh
rtk npx knip --no-progress
```

A second run constrained the source graph to the browser entry and omitted test files:

```sh
rtk npx knip --config .knip-frontend-review.mjs --no-progress
```

The temporary configuration was:

```js
export default {
  entry: ["src/main.tsx"],
  project: ["src/**/*.{ts,tsx}", "!src/**/*.{test,spec}.{ts,tsx}"],
  ignoreDependencies: ["autoprefixer"],
};
```

It was removed after the run. The baseline raw output includes the temporary configuration itself as a fourth unused file; exclude that review artifact to obtain three repository file findings (two genuine unused files and the convention-loaded PostCSS config). The scoped run is supporting evidence for distinguishing production reachability from test-only reachability; Knip still auto-discovered package scripts, so its system-binary findings are outside this frontend cleanup.

## Verified production cleanup

These findings were confirmed with repository-wide reference searches excluding `node_modules` and `.worktrees`.

| Priority | Candidate | Evidence | Recommended action |
| --- | --- | --- | --- |
| Low | `src/lib/provider-account.ts` | No import or reference outside its own declarations. The current generation credit UI has separate implementation in `media-bin.tsx`. | Delete the unused frontend module in a cleanup change; retain the backend command unless separately reviewed. |
| Low | `src/components/ui/card.tsx` | No source import or JSX usage; references exist only in historical plans. | Delete the unused shadcn primitive. |
| Low | `@dnd-kit/core` | Present only in `package.json`, the lockfile, and historical design documents. | Remove the dependency and refresh the lockfile. |
| Low | `dnd-timeline` | Present only in `package.json`, the lockfile, and historical design documents. Current timeline interactions are implemented in app code. | Remove the dependency and refresh the lockfile. |
| Medium | `cancelModelDownload` | Declared in `src/lib/transcription-models.ts`; no production caller. A settings integration test merely mocks the name. | Remove the function and remove the stale mock member. |
| Medium | `captureCanonicalPreviewFrameInSplitProjectFolder` | Declared in `src/lib/project.ts`; no caller found in source or tests. | Delete after confirming no downstream consumer treats this private app source file as an API. |
| Medium | `renderWebmToSplitProjectFolder` | Declared in `src/lib/project.ts`; no caller found in source or tests. | Delete under the same private-module assumption. |

The two dependency removals are independent of the three dead functions. Each should be a small change with `pnpm lint`, the relevant focused tests, and `pnpm build` after the batch.

## Test-only components that need a product decision

The production-only graph exposed two substantial components that the baseline did not report because their tests import them:

| Component | Current state | Recommendation |
| --- | --- | --- |
| `src/components/settings/skills-settings.tsx` | Imported by its own tests and accessibility tests, but not rendered by the production settings composition. The live settings path uses project settings and the shared skills library directly. | Treat as an architecture issue before cleanup. Either restore it as an intentional settings surface or delete the component and its component-specific tests. Keeping a well-tested component that users cannot reach creates false confidence. |
| `src/components/workspace/render-quality-control.tsx` | Imported only by `render-quality-control.test.tsx`; no production render site remains. | Confirm that export/render controls supersede it, then delete component and test together. |

`src/test-utils/required.ts` is also omitted from the production graph, as expected for a test helper. It is not a cleanup candidate while tests use it.

## Safe export-surface cleanup

Knip reported 21 unused exported runtime values in the frontend-only run. Most are live implementation details used inside their declaring module. The safe change is to remove only the `export` modifier, not the value:

- `generationModelSummary`, `formatStorageBytes`, and `buttonVariants`;
- `captionStyleDetails`, `matteAspectOptions`, `mattePreviewSize`, and `visualBlendModes`;
- `supportedMediaFileExtensions`;
- `punchyCaptionTemplate`, `metricCalloutTemplate`, `chapterCardTemplate`, and `trackingHighlightTemplate`;
- `nativeMenuCommands`;
- `canonicalProjectActionKeyframeEasing` and `canonicalizeProjectActionKeyframes`;
- `systemHealthOverall`;
- `timelinePreviewLayerGeometry` and `isTimelinePreviewItemActiveAt`.

The three fully dead functions listed in the previous section account for the remaining reported frontend exports and should be removed rather than merely made private.

The baseline also reported two script constants, `manifestSchemaVersion` and `CONTROLLER_OWNED_DIRTY_PATHS`. Both are used within their own script modules. Their `export` modifiers can be removed if those scripts are confirmed to have no external programmatic consumers.

Knip reported 112 exported types. This is mainly an oversized module-surface problem, especially in `src/lib/project.ts`, rather than proof that all 112 declarations are dead. Many are used locally. Handle these in a mechanical follow-up: remove unnecessary `export` modifiers first, rerun TypeScript, then separately evaluate declarations that become demonstrably unreferenced. Avoid a broad deletion pass over project and render contract types.

## Findings that are not unused code

- `postcss.config.js` is convention-loaded by the Vite/PostCSS toolchain. Its Tailwind and Autoprefixer plugin declarations make the file operational even though JavaScript imports do not point to it.
- `autoprefixer` is referenced by `postcss.config.js`; Knip does not infer this string-key plugin usage. Keep it.
- The 13 “unlisted binaries” are host tools invoked by build, release, and evidence scripts (`ffmpeg`, `rustc`, `swift`, `pkg-config`, `cc`, `brew`, and `gst-inspect-1.0`). They are external runtime prerequisites, not npm dependencies to add during frontend cleanup.
- `defaultAppPreferences` and `defaultAppSettingsPreferences` are aliases, but both names have current consumers. Consolidating them is reasonable naming cleanup, not a direct dead-code deletion.
- The settings visual-QA fixture is dynamically imported from `src/main.tsx` and `settings.tsx`. Knip followed that import; it did not produce a dynamic-loading false positive in the baseline.

## Proposed cleanup batches

1. Remove the two unused npm dependencies and the two verified unused files (`card.tsx`, `provider-account.ts`).
2. Remove the three dead exported functions and their stale test mock where applicable.
3. Decide whether Skills Settings and Render Quality Control should be restored to production or removed with their tests.
4. Remove unnecessary export modifiers in small module groups, beginning with UI components and utilities, then settings, timeline, and the large project contract module.
5. Add a checked-in Knip configuration and a `check:unused` script only after agreeing on intentional exclusions for convention-loaded config, external binaries, and contract types. This prevents a noisy CI gate from normalizing ignored failures.

No application source, package manifest, or lockfile was changed by this review.
