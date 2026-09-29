# Editor Redesign 07 — Export, Background Tasks, Menus, and Native Menu Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.
>
> **Detail level:** task-level. Expand each task into bite-sized TDD steps with exact code before executing it.

**Goal:** Build the single export entry point and a unified background tasks system, then finish the editor chrome.

- **Export popover:** video export, NLE XML, and project package.
- **Background tasks:**
  - top-bar indicator and popover,
  - polling, cancel, retry, log, and Show,
  - a details view covering workflow internals and render review.
- **Editor chrome:**
  - the full gear menu: project settings, app settings, keyboard shortcuts sheet, connect external agents (MCP), project skills;
  - toasts;
  - "Save range as media";
  - native menu rewiring, including an editor-aware Undo and Redo that works on macOS.

**Architecture:**
- **Jobs slice.** A `jobs` store slice derives task records from `project.jobs`, `generatedAssets`, `renderReports` and `exportArtifacts` using `buildActivityJobRecords`.
- **Polling.** Polling reloads the project folder while non-terminal jobs exist, every 1 s, backing off to 5 s after 60 s. `mergeJobState(current, loaded)` merges only job, generated-asset, render-report and export-artifact state into the current project. In-flight user edits are never clobbered.
- **Export service.** `export-service.ts` selects in-process or Temporal execution from the `generationExecutionBackend` preference, mirroring the pre-cut `exportInDesktopProcess` and `exportMediaProfile`.
- **Native menu.** The TS `NativeMenuState` and the Rust `native_menu.rs` are simplified to the redesign's commands.

**Tech Stack:** React 19, Zustand, Radix (popover, dialog, toast), Tauri 2.11, Rust, Vitest, Playwright.

**Spec sections:** Overlays (Export popover, Background tasks, Gear menu, Dialogs/toasts), Keyboard, Behavior Details, Acceptance flow 8.
**Depends on:** plans 01–06.

## Global Constraints

- **Workflow.** Prefix commands with `rtk`, use Conventional Commits, and stage only the named files.
- **File size and styling.** Keep files under 600 lines and use tokens only.
- **One export entry point.** The Export button (and ⌘E or the native menu Export command) opens the popover. No other export UI exists.
- **Background tasks replace three older surfaces.** They replace the Activity rail, the "Render status" button and the render inspector tab. There must be no second job list anywhere.
- **Failure copy.** Every failed task row states what failed in plain words and offers Retry when the operation is retryable, plus Log when a log path exists.
- **Cancel.** A cancel button appears only for kinds with a real cancel command:
  - in-process render: `cancelRenderJobInSplitProjectFolder`
  - in-process generation: `cancelGenerateMediaInProcess`, or the provider request cancel
  - agent turn: `cancelCodexConversationEditForProject`

  No generic Temporal cancel exists. Temporal-run rows show a disabled Cancel with the tooltip "This task runs in the workflow worker and can't be cancelled from the editor."
- **Show in folder.** Uses a new restricted command that reveals only export artifacts recorded in the project (Task 2). There is no generic path reveal.
- **Native menu.** Rust and TS command sets change together, with their tests, in one commit.
- **Reference.** Consult the pre-cut `export-sheet.tsx`, `activity-panel.tsx`, `render-report-panel.tsx`, `project-timeline-inspector.tsx`, and the `editor-workspace.tsx` export and job handlers via `rtk git show <pre-cut-sha>:<path>`.

---

## File Map

### Pure modules
- **`src/lib/jobs/merge-job-state.ts`:** `mergeJobState(current: VideoProject, loaded: VideoProject): VideoProject`. It takes `jobs` via `mergeProjectJobs`, updates `generatedAssets` by id with a status/outputs precedence rule, and unions `renderReports` and `exportArtifacts` by id. Everything else comes from `current`, and `contentRevision` becomes `max(current, loaded)` only when the loaded project is otherwise equal. See Task 1 for the conflict rule.
- **`src/lib/jobs/task-records.ts`:** `taskRecords(project, runtime): TaskRecord[]`. It builds on `buildActivityJobRecords` and adds generated assets without jobs, the agent turn (from the agent slice), and in-process renders.
  ```ts
  interface TaskRecord {
    id: string;
    kind: "transcription" | "analysis" | "generation" | "render" | "export" | "agent";
    label: string;
    status: "queued" | "running" | "completed" | "failed" | "cancelled" | "blocked";
    progress: number | null;
    detail: string | null;
    failureReason: string | null;
    artifactPath: string | null;
    logPath: string | null;
    cancel: { available: true } | { available: false; reason: string };
    retry: boolean;
    updatedAt: string;
    workflow: { runId?: string; workflowId?: string; backend: "temporal" | "inProcess" } | null;
  }
  ```
- **`src/lib/jobs/task-indicator.ts`:** `indicatorState(records, now)`. It returns the most important running task (running before queued, then the oldest) with its progress label. It returns hidden when there are no tasks or the last completion is more than 10 minutes old.
- **`src/lib/export/export-plan.ts`:** `exportPlan(input): ExportStartPlan`. It maps popover choices (format, resolution, quality, advanced codec and fps, name, directory) onto `renderMediaToSplitProjectFolder` input or a Temporal `ExportMediaStartInput`, using `lib/export/profiles` (`exportProfileById`, `exportProfileDisabledReason`, `dimensionsForResolution`, `mediaExportOutputPath`). `estimatedExportSize(plan, durationSeconds)` feeds the summary line.

### Services and store
- **`src/editor/store/jobs-slice.ts`:** records, polling lifecycle (`startPolling` / `stopPolling`), `cancelTask(id)`, `retryTask(id)`, `openTaskDetails(id)`.
- **`src/editor/services/export-service.ts`:**
  - `exportVideo(plan)`
  - `exportNleXml(format)` via `exportNleXmlToSplitProjectFolder`
  - `exportProjectPackage()` via `exportPalmierProjectPackageToSplitProjectFolder`
  - `saveRangeAsMedia(range)`: renders the range with `renderMediaToSplitProjectFolder` using `rangeStartSeconds` / `rangeEndSeconds`, then imports the output as media. Check whether the pre-cut flow imported via `importMediaToProject` or recorded the media directly.
- **`src/editor/services/reveal-service.ts`:** `revealExportArtifact(artifactPath)`.
- **`src/lib/runtime/adapters/tauri-dialog.ts`:** adds `chooseExportDirectory()`.

### Components
- `src/editor/overlays/export-popover.tsx`, `export-options.tsx`, `export-footer-links.tsx`
- `src/editor/overlays/tasks-indicator.tsx`, `tasks-popover.tsx`, `task-row.tsx`, `task-details-dialog.tsx`. The details dialog shows workflow metadata, Temporal preflight via `getTemporalWorkerEnvironmentReport`, and for renders the render report metrics (`lib/export/render-report`) plus "Compare preview and render" via `runPreviewRenderComparisonRequestInSplitProjectFolder`.
- `src/editor/overlays/gear-menu.tsx`, which replaces the plan 02 inline menu
- `src/editor/overlays/shortcuts-sheet.tsx`, grouped from `editorShortcuts`, formatted per platform
- `src/editor/overlays/connect-agents-dialog.tsx`, with `agentSetupSnippet` per client, copy buttons, and project folder label
- `src/editor/overlays/project-skills-dialog.tsx`. Its content is ported from the pre-cut `projectSkills` constant in `editor-workspace.tsx` and moved to `src/lib/agent/project-skills.ts`.
- `src/components/ui/popover.tsx`, `toast.tsx`, `toaster.tsx`, the latter holding a store-backed toast queue in `ui` slice `toasts`

### Rust
- **`src-tauri/src/main.rs`:** new command `reveal_export_artifact_in_split_project_folder(project_dir, artifact_path)`. It resolves the project dir, verifies that `artifact_path` equals a recorded `exportArtifacts[].path` (project-relative or absolute under the project dir), then calls `desktop_integration::reveal_path_in_file_manager`. Register it.
- **`src-tauri/src/native_menu.rs`:** simplified command set (Task 8) and tests.
- `src-tauri/tests/…`: tests for the reveal command's path restriction, colocated with existing `main.rs` unit tests or a new integration test following the storage reveal tests near `main.rs:4414`.

### Modified
- `src/lib/native-menu.ts` and its test
- `src/App.tsx` (native command forwarding)
- `src/editor/editor-root.tsx` (native menu state, request handling)
- `src/editor/shell/top-bar.tsx` (indicator, gear, export)
- the timeline context menu ("Save range as media" enabled)
- `knip.jsonc`

---

### Task 1: Job state merge and task records (pure)

- [ ] **Merge tests** (`merge-job-state.test.ts`):
  - A loaded job status update merges without touching a concurrent timeline edit in `current`.
  - A completed generated asset adds outputs.
  - Duplicate render reports and export artifacts are unioned by id.
  - **Conflict rule:** if `loaded.contentRevision > current.contentRevision` and loaded timeline content differs from current, the loaded project wins entirely. That means an external or agent edit happened. The merge then sets `externalChange: true` so the store can drop local redo history and clear stale selection.
- [ ] **Task record tests** (`task-records.test.ts`): each kind and status maps to a label, detail, failure reason and cancel availability. Temporal-backed rows get the disabled cancel reason. Generated assets without jobs still appear. Ordering is running, queued, failed, then completed by `updatedAt`.
- [ ] **Indicator tests** (`task-indicator.test.ts`): hidden when idle, the 10-minute window, and the label format "Transcribing · 62%" / "Exporting…".
- [ ] **Commit:** `feat(jobs): merge polled job state and derive background task records`

### Task 2: Restricted reveal command

- [ ] **Rust tests:**
  - Revealing a recorded export artifact calls the injected reveal function with its absolute path.
  - An unrecorded path returns `"This file isn't a recorded export of this project."`.
  - Paths that escape the project (`..`) are rejected.
  - Follow the `reveal_reported_storage_path_with` injection pattern.
- [ ] **Implement and register the command.** Add the TS wrapper `revealExportArtifactInSplitProjectFolder({ projectDir, artifactPath })` in `src/lib/project.ts` with a test.
- [ ] **Verify:**
  - `rtk cargo test --manifest-path src-tauri/Cargo.toml reveal_export_artifact -- --test-threads=1`
  - `rtk pnpm vitest run src/lib/project.test.ts`
- [ ] **Commit:** `feat(desktop): reveal recorded export artifacts from the editor`

### Task 3: Jobs slice, polling, cancel, retry

- [ ] **Polling:**
  - Starts when any record is non-terminal. Stops when none remain or the editor unmounts.
  - Reloads with `loadSplitProjectFromFolder({ projectDir })`, then applies `mergeJobState` via a `project.mergeExternalState(merged, { externalChange })` action. This is a new project slice method that does not record history.
  - Skipped when the project is not split-folder backed (sample or browser projects).
- [ ] **Cancel and retry** dispatch per kind using the commands listed in Global Constraints.
  - Retry for generation calls `generation-service.rerun`, or `retryDownload` when the failure is a download failure.
  - Retry for export re-runs `export-service.exportVideo` with the job's stored plan, kept in memory keyed by job id. Without a stored plan, the retry control opens the Export popover preset from the job.
  - Retry for transcription calls `speech-service.transcribe`.
- [ ] **Tests** with fake timers and a mocked backend:
  - Polling starts and backs off.
  - A merge preserves a concurrent local edit.
  - An external change drops redo history.
  - Cancel availability by backend.
  - Retry routing.
  - Polling stops on completion.
- [ ] **Commit:** `feat(jobs): add the jobs store slice with polling, cancel, and retry`

### Task 4: Tasks indicator, popover, and details dialog

- [ ] **Indicator.** A pill in the top bar showing a spinner plus the label from `indicatorState`, or a failed dot when the newest record failed. Clicking opens the popover.
- [ ] **Popover rows.**
  - Each row has a kind icon, label, progress bar or detail line, and actions: Cancel, Show (reveal), Retry, Log (reveals the log path via the same restricted command extended to recorded render log paths, or shows log text if the render report embeds it), and Details.
  - Failure text reads "<failureReason>".
- [ ] **Details dialog.** Shows the workflow backend, workflow and run ids, queue position (when present in workflow metadata), timestamps, Temporal worker preflight results, and, for render and export, the render report metrics with "Compare preview and render".
- [ ] **Mobile.** The indicator shows a compact spinner and percent, and the popover becomes a compact sheet.
- [ ] **Tests:**
  - Accessible names: button "Background tasks", dialog "Task details".
  - Failure copy, and disabled Cancel with the Temporal reason.
  - The Show button calls the reveal service.
  - The details dialog renders the preflight report fixture.
- [ ] **Commit:** `feat(jobs): add the background tasks indicator, popover, and details`

### Task 5: Export popover and export service

- [ ] **`export-plan.test.ts`:**
  - MP4/1080p/High maps to the H.264 profile at 1920×1080.
  - ProRes appears only when available.
  - WebM is labelled "WebM" (drop "Legacy").
  - Draft quality uses `draftExportDimensions`.
  - The Advanced codec (H.265) and fps overrides apply.
  - Output paths come from `mediaExportOutputPath`.
  - Unavailable profiles return a disabled reason.
- [ ] **Export service tests:**
  - The in-process preference records the job and awaits `render_media_to_split_project_folder`. On failure it loads the render report.
  - The Temporal preference builds the start request and starts the workflow.
  - NLE XML (both formats) and the project package call their commands and record a task.
- [ ] **Popover.**
  - Fields and controls:
    - Name
    - Save to: default location shown, with a folder button using `chooseExportDirectory`
    - Format segmented control, filtered by `getExportProfileAvailabilityReport`, with unavailable options disabled and a tooltip reason
    - Resolution and Quality segmented controls
    - A summary line: codec · fps · ≈ size
    - An Advanced disclosure for codec and fps
  - Buttons and links:
    - "Export video" button
    - Footer links: Premiere XML · DaVinci XML · Project package
  - Behavior:
    - Starting an export closes the popover.
    - Completion shows a toast "Exported <name>" with "Show in folder".
    - The popover opens from the Export button, ⌘E, or native Export.
- [ ] **Tests:**
  - Disabled reasons render.
  - The summary updates.
  - Export starts one task and closes the popover.
  - A footer link starts the NLE export.
  - The completion toast's Show calls reveal.
- [ ] **Commit:** `feat(export): add the export popover with video, XML, and package exports`

### Task 6: Save range as media

- [ ] **Context menu.** Enable "Save range as media" for a selected clip, which uses its span, or for the I/O range when set.
- [ ] **Flow.** It runs `export-service.saveRangeAsMedia`, which shows a task. On completion the new media appears in the Media grid and is revealed there.
- [ ] **Tests:**
  - The range comes from the selection or from I/O marks.
  - The task completes and the media is added.
  - With no range, the item is disabled with the reason "Select a clip or mark in and out points".
- [ ] **Commit:** `feat(export): save a timeline range as new media`

### Task 7: Gear menu, shortcuts sheet, connect agents, project skills, toasts

- [ ] **Gear menu items:**
  - Project settings
  - App settings
  - separator
  - Keyboard shortcuts (⌘/)
  - Connect external agents…
  - Project skills…
- [ ] **Shortcuts sheet.** Groups from `editorShortcuts` by `group`, with bindings formatted by `formatShortcut` for the current platform. It is searchable.
- [ ] **Connect agents dialog.**
  - Client tabs: Codex, Claude Code, Claude Desktop, Cursor. Take the client list from `AgentSetupClient`.
  - Each tab shows the snippet from `agentSetupSnippet` in a read-only code block with a "Copy" button that uses the Clipboard API and falls back to select-all.
- [ ] **Project skills dialog.** Shows the three required skills with descriptions and a link to App settings → Skills.
- [ ] **Toasts.**
  - Radix toast plus a `ui.toasts` queue.
  - Used only for completions and undoable background outcomes.
  - At most 3 visible. Auto-dismiss after 6 s, which pauses on hover or focus.
- [ ] **Tests:**
  - Menu items open the right overlays.
  - The shortcut sheet lists `editor.undo` as "⌘Z" on macOS and "Ctrl+Z" on Linux.
  - The copy button writes the snippet.
  - The toast queue caps at 3 and auto-dismisses.
- [ ] **Commit:** `feat(editor): add the gear menu, shortcuts sheet, agent connection, and toasts`

### Task 8: Native menu rewiring and macOS Undo/Redo

- [ ] **TS `NativeMenuState`** (`src/lib/native-menu.ts`) becomes:

  ```ts
  export interface NativeMenuState {
    view: "home" | "editor" | "settings";
    canImport: boolean;
    canExport: boolean;
    canUndo: boolean;
    canRedo: boolean;
    canSplit: boolean;
    canTrimStart: boolean;
    canTrimEnd: boolean;
    canDelete: boolean;
    canRippleDelete: boolean;
    canSelectForward: boolean;
  }
  ```

  **Commands:**
  - **Keep:** `openSettings`, `openProjectSettings`, `openAdvancedSettings`, `openSystemHealth`, `newProject`, `openProject`, `importMedia`, `exportProject`, `selectForwardTrack`, `selectForwardAll`, `split`, `trimStart`, `trimEnd`, `delete`, `rippleDelete`, `openShortcuts`, `sendFeedback`
  - **Add:** `undo`, `redo`, `showTab:ai|media|audio|text|captions|effects` (as six commands), `openConnectAgents`
  - **Remove:** `toggleMedia`, `toggleInspector`, `toggleCodex`, `toggleMaximize`, `layoutDefault`, `layoutMedia`, `layoutVertical`, `openTour`

  **Also:**
  - Delete the `EditorOverlayCommand` type.
  - Update `inactiveNativeMenuState` and `native-menu.test.ts`.
- [ ] **Rust `native_menu.rs`** must stay consistent with the TS side:
  - **IDs and commands.** Update the command ids, the `NativeMenuCommand` enum, `build_native_menu`, `sync_native_menu_state`, `command_for_menu_id`, `menu_item_ids_for_test`, and the tests at L546–617.
  - **View menu.** Replace it with "AI", "Media", "Audio", "Text", "Captions" and "Effects" items using accelerators `CmdOrCtrl+1`…`6`, enabled only in the editor view.
  - **Help menu.** "Keyboard Shortcuts" (`CmdOrCtrl+/`), "Connect External Agents…", "Project Guidance", and "Send Feedback". Remove "Editor Tour".
  - **Undo and Redo.**
    - In the editor view, the Edit menu uses custom "Undo" (`CmdOrCtrl+Z`) and "Redo" (`Shift+CmdOrCtrl+Z`) items. They emit `undo` / `redo` and are enabled from `canUndo` / `canRedo`.
    - Outside the editor view (Home, Settings), keep the predefined `.undo()` / `.redo()` items so text fields keep native undo.
    - If swapping items at runtime is not supported, rebuild the menu when the view changes, following how `sync_native_menu_state` is called.
    - Keep the GTK comment (L266–268) behavior on Linux.
  - **Before implementing,** verify in `rtk pnpm dev` on macOS that the predefined undo item swallows ⌘Z before the webview `keydown`. Record the observation in the commit body. On Linux, verify that Ctrl+Z reaches the webview.
- [ ] **Command routing.**
  - `App.tsx` forwards editor commands to `EditorRoot` as today.
  - `editor-root.tsx` handles each command through the store and services: import opens the dialog adapter, export opens the popover, undo and redo route through the global undo logic from plan 06, and so on.
  - Native menu state is reported from the store: `canUndo` includes the agent-undo availability, `canSplit` comes from the timeline planners' non-blocked results, and so on.
- [ ] **Verify:**
  - `rtk cargo test --manifest-path src-tauri/Cargo.toml native_menu -- --test-threads=1`
  - `rtk pnpm vitest run src/lib/native-menu.test.ts src/App.test.tsx src/editor`
  - `rtk pnpm lint`
- [ ] **Commit:** `feat(desktop): rewire the native menu for the redesigned editor`

### Task 9: Integration, fixtures, knip, e2e

- [ ] **Fixture handlers** in `src/lib/runtime/fixtures/export-fixtures.ts`, registered when `marker.exportFixture === true`:
  - `render_media_to_split_project_folder`: resolves after 3 polls with a recorded export artifact.
  - `get_export_profile_availability_report`: returns MP4, WebM and ProRes-unavailable with a reason.
  - `load_split_project_from_folder`: returns the fixture project with advancing job status.
  - `reveal_export_artifact_in_split_project_folder`: records the call on `window.__EDITOR_FIXTURE_DRIVER__.calls`.
  - NLE export and package commands.
  - Handler tests.
- [ ] **knip.** Remove the consumed entries from the temporary `knip.jsonc` block.
- [ ] **`e2e/editor-export-tasks.spec.ts`**, run at desktop 1440×900 and phone 402×874:
  1. Press Export and choose MP4 · 1080p · High. The summary line shows H.264.
  2. Press Export video. The popover closes and the "Background tasks" indicator shows "Exporting".
  3. Completion shows a toast. "Show in folder" records a reveal call.
  4. Open Background tasks and confirm the completed row. Open Details and confirm the dialog shows the render report.
  5. ProRes is disabled with its reason in a tooltip.
  6. Desktop only: ⌘/ (or Ctrl+/) opens the shortcuts sheet, and Esc closes it.
  7. Gear → Connect external agents → Copy writes to the clipboard. Grant permission via the Playwright context.
- [ ] **Visual check.** Capture the export popover, tasks popover and details dialog at 1440×900 and 402×874. Read the PNGs.
- [ ] **Gate.** `rtk pnpm verify:frontend`.
- [ ] **Commit:** `test(editor): cover export, background tasks, and editor menus`

## Acceptance

- Export has exactly one UI. Background tasks is the only job surface, and every spec overlay bullet is implemented.
- Polling never clobbers local edits. External changes are detected and redo is dropped.
- Cancel appears only where a real cancel exists, and Show reveals only recorded export artifacts.
- The native menu matches the redesign on macOS and Linux, and ⌘Z / Ctrl+Z undo editor edits in the Tauri app.
- Spec acceptance flow 8 passes at both viewports.
