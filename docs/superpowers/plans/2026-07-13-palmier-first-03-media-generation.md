# Palmier-First Media and Generation Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Restore Palmier's Media/Captions/Audio rail, compact library hierarchy, functional overflow actions, and attached generation drawer while preserving Video Creater's newer import, provider, folder, search, silence, and output-lifecycle work.

**Architecture:** Reconcile the deleted committed rail into the current `MediaBin` API rather than restoring an old whole file. Keep all provider and project workflow ownership in `EditorWorkspace`; extract the already-proven Rust matte creation logic into a shared project module for both Codex and direct UI use; render the composer as a child of the media pane.

**Tech Stack:** React 19, TypeScript, Tailwind CSS, Lucide, Vitest, Testing Library, Tauri 2, Rust image crate, existing split-project storage and provider catalog.

---

## File Structure

- Modify `src/components/workspace/media-bin.tsx`: rail, toolbar, overflow, direct grid, attached composer, and new typed callbacks.
- Modify `src/components/workspace/media-bin.test.tsx`: rail, overflow, organizer safety, matte, and composer geometry.
- Create `src/components/workspace/matte-sheet.tsx`: color/aspect/size form.
- Create `src/components/workspace/matte-sheet.test.tsx`: form and busy/error behavior.
- Create `src-tauri/src/project/matte.rs`: shared solid-color PNG creation and dimension validation.
- Modify `src-tauri/src/project/mod.rs`: export matte module types.
- Modify `src-tauri/src/codex/tools.rs`: call shared matte logic.
- Modify `src-tauri/src/lib.rs`: direct `create_matte_in_split_project_folder` Tauri command.
- Modify `src/lib/project.ts`: typed matte command wrapper.
- Modify `src/lib/project.test.ts`: wrapper payload test.
- Modify `src/components/workspace/editor-workspace.tsx`: wire matte reload, organizer prompt, captions rail content, and attached generation state.
- Modify `src/components/workspace/editor-workspace.test.tsx`: exact integration behavior.
- Modify `docs/parity.md`: reopen and then evidence the media/generation state.

### Task 1: Reopen stale media visual claims

**Files:**
- Modify: `docs/parity.md:518`

- [ ] **Step 1: Record the rail and composer regression**

Replace the media row with:

```markdown
| P1 | visual-regression | Media library chrome | Functional media, captions, audio, folders, generation, search, import, silence, and generated-output workflows remain, but the current tree removes the Media/Captions/Audio rail and detaches the generation composer from Palmier's pane hierarchy. Fresh references: `output/parity-audit-2026-07-13/reference/05-captions.png` through `07-media-overflow.png`. | Reopened 2026-07-13. |
```

- [ ] **Step 2: Verify and commit the truthful state**

Run: `rtk git diff --check -- docs/parity.md && rtk rg -n "Media library chrome|visual-regression|05-captions" docs/parity.md`

Expected: no whitespace errors and the exact new evidence paths are visible.

```bash
rtk git add docs/parity.md
rtk git commit -m "docs(parity): reopen media surface regression"
```

### Task 2: Restore the contextual source rail without losing dirty-tree APIs

**Files:**
- Modify: `src/components/workspace/media-bin.tsx`
- Modify: `src/components/workspace/media-bin.test.tsx`
- Modify: `src/components/workspace/editor-workspace.tsx`

- [ ] **Step 1: Add a failing rail test around the current props**

```tsx
it("switches Media, Captions, and Audio through one compact rail", () => {
  renderMediaBin({ captionsPanel: <div>Caption workbench</div> });

  const tabs = screen.getByRole("tablist", { name: "Source panel views" });
  expect(within(tabs).getByRole("tab", { name: "Media" })).toHaveAttribute("aria-selected", "true");
  fireEvent.click(within(tabs).getByRole("tab", { name: "Captions" }));
  expect(screen.getByText("Caption workbench")).toBeVisible();
  expect(screen.queryByRole("group", { name: "Project media grid" })).not.toBeInTheDocument();
  fireEvent.click(within(tabs).getByRole("tab", { name: "Audio" }));
  expect(screen.getByRole("tablist", { name: "Audio panel views" })).toBeVisible();
});
```

- [ ] **Step 2: Run the test to verify the regression**

Run: `rtk pnpm exec vitest run src/components/workspace/media-bin.test.tsx -t "compact rail"`

Expected: FAIL because the current dirty tree removed `captionsPanel`, `MediaPanelTab`, and the rail.

- [ ] **Step 3: Reintroduce the narrow API**

```tsx
export type MediaPanelTab = "media" | "captions" | "audio";

interface MediaBinProps {
  captionsPanel?: ReactNode;
  panelTab?: MediaPanelTab;
  onPanelTabChange?: (tab: MediaPanelTab) => void;
  // retain every current prop below these additions
}
```

Use controlled state when supplied and internal state otherwise:

```tsx
const [internalPanelTab, setInternalPanelTab] = useState<MediaPanelTab>("media");
const panelTab = controlledPanelTab ?? internalPanelTab;
function selectPanelTab(tab: MediaPanelTab) {
  if (controlledPanelTab === undefined) setInternalPanelTab(tab);
  onPanelTabChange?.(tab);
}
```

- [ ] **Step 4: Render the 42px icon rail and active panel**

Use `FolderOpen`, `Captions`, and `AudioLines` from Lucide with 18px icons, 32px hit targets, accessible labels, `role="tab"`, and a subtle raised active background. The content column renders the current media body, `captionsPanel`, or the existing audio-generation/silence body.

Do not remove `onImportPaths`, `externalDropActive`, current/all silence callbacks, semantic encoder status, provider cancellation capability, or current generated-output props added in the dirty tree.

- [ ] **Step 5: Wire controlled state in EditorWorkspace**

Add `const [mediaPanelTab, setMediaPanelTab] = useState<MediaPanelTab>("media")`. When the generation composer opens in audio mode, select Audio; when media is revealed, select Media; when the caption builder opens, select Captions.

- [ ] **Step 6: Run media and workspace tests**

Run: `rtk pnpm exec vitest run src/components/workspace/media-bin.test.tsx src/components/workspace/editor-workspace.test.tsx -t "rail|Media|Captions|Audio"`

Expected: rail tests and existing media/caption/audio routing tests PASS.

- [ ] **Step 7: Commit the reconciled rail**

```bash
rtk git add -p src/components/workspace/media-bin.tsx
rtk git add -p src/components/workspace/media-bin.test.tsx
rtk git add -p src/components/workspace/editor-workspace.tsx
rtk git diff --cached --name-only
rtk git commit -m "feat(media): restore contextual source rail"
```

### Task 3: Match Palmier's two-row media toolbar and overflow

**Files:**
- Modify: `src/components/workspace/media-bin.tsx`
- Modify: `src/components/workspace/media-bin.test.tsx`
- Modify: `src/components/workspace/editor-workspace.test.tsx`

- [ ] **Step 1: Add failing toolbar/menu tests**

```tsx
it("keeps primary media actions compact and secondary actions in overflow", () => {
  renderMediaBin();
  const toolbar = screen.getByRole("toolbar", { name: "Media actions" });
  expect(within(toolbar).getByRole("button", { name: "Import media" })).toBeVisible();
  expect(within(toolbar).getByRole("button", { name: "Generate media" })).toBeVisible();
  expect(within(toolbar).getByRole("button", { name: "More media actions" })).toBeVisible();
  expect(within(toolbar).getByRole("button", { name: "Smart search" })).toBeVisible();
  expect(within(toolbar).queryByRole("button", { name: "New folder" })).not.toBeInTheDocument();

  fireEvent.click(within(toolbar).getByRole("button", { name: "More media actions" }));
  const menu = screen.getByRole("menu", { name: "More media actions" });
  expect(within(menu).getByRole("menuitem", { name: "New Folder" })).toBeVisible();
  expect(within(menu).getByRole("menuitem", { name: "Create Matte" })).toBeVisible();
  expect(within(menu).getByRole("menuitem", { name: "Organize with Agent" })).toBeVisible();
});
```

- [ ] **Step 2: Run the test to verify failure**

Run: `rtk pnpm exec vitest run src/components/workspace/media-bin.test.tsx -t "primary media actions compact"`

Expected: FAIL because New Folder and silence/search controls are flattened into the wrapping toolbar and no overflow exists.

- [ ] **Step 3: Render the two-row hierarchy**

Row one is Import, selected Generate pill, overflow, flexible spacer, Smart Search. Row two is the search input, grid-size/view, sort, and filter controls. Use 32px controls and 8px gaps. Keep the Library/count row beneath them.

- [ ] **Step 4: Add a deterministic safe organizer prompt**

Add prop `onOrganizeWithAgent?: (prompt: string) => void` and emit exactly:

```ts
const organizeMediaPrompt = [
  "Organize the current project media into clear folders using media metadata and content.",
  "Do not delete any media.",
  "Do not add, remove, move, trim, or otherwise change timeline items.",
  "Return reviewed project actions only; do not mutate canonical files directly.",
].join(" ");
```

Test the exact four safety sentences. `EditorWorkspace` opens Codex and populates the composer; it does not apply actions automatically.

- [ ] **Step 5: Preserve Video Creater-only actions below a separator**

Place Current Silence, All Silence, semantic analysis/rebuild, and richer view/filter controls after `role="separator"` in the same menu or their existing focused disclosure. Keep every disabled/busy reason.

- [ ] **Step 6: Run focused toolbar and safety tests**

Run: `rtk pnpm exec vitest run src/components/workspace/media-bin.test.tsx src/components/workspace/editor-workspace.test.tsx -t "media actions|Organize with Agent|silence|semantic"`

Expected: all focused tests PASS.

- [ ] **Step 7: Commit the toolbar**

```bash
rtk git add -p src/components/workspace/media-bin.tsx
rtk git add -p src/components/workspace/media-bin.test.tsx
rtk git add -p src/components/workspace/editor-workspace.test.tsx
rtk git commit -m "feat(media): add compact action overflow"
```

### Task 4: Extract and expose functional matte creation

**Files:**
- Create: `src-tauri/src/project/matte.rs`
- Modify: `src-tauri/src/project/mod.rs`
- Modify: `src-tauri/src/codex/tools.rs`
- Modify: `src-tauri/src/lib.rs`
- Modify: `src/lib/project.ts`
- Modify: `src/lib/project.test.ts`

- [ ] **Step 1: Write Rust dimension and persistence tests first**

```rust
#[test]
fn matte_dimensions_match_palmier_aspects_and_stay_even() {
    assert_eq!(matte_pixel_size("Project", 736, 400).unwrap(), (736, 400));
    assert_eq!(matte_pixel_size("16:9", 736, 400).unwrap(), (710, 400));
    assert_eq!(matte_pixel_size("9:16", 736, 400).unwrap(), (400, 710));
    assert_eq!(matte_pixel_size("1:1", 736, 400).unwrap(), (400, 400));
    assert!(matte_pixel_size("3:2", 736, 400).is_err());
}

#[test]
fn create_matte_writes_png_and_persists_media() {
    let fixture = split_project_fixture();
    let result = create_matte(&fixture.dir, MatteRequest {
        hex: "#112233".into(),
        aspect_ratio: "Project".into(),
        name: None,
        folder_id: None,
    }).unwrap();
    assert!(fixture.dir.join(&result.media.relative_path).is_file());
    assert_eq!(result.media.kind, MediaKind::Image);
    assert!(storage::load_project(&fixture.dir).unwrap().media.iter().any(|media| media.id == result.media.id));
}
```

- [ ] **Step 2: Run tests to verify the shared module is missing**

Run: `rtk cargo test --manifest-path src-tauri/Cargo.toml project::matte -- --test-threads=1`

Expected: FAIL because `project::matte` does not exist.

- [ ] **Step 3: Extract the existing proven logic**

Create public types:

```rust
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct MatteRequest {
    pub hex: String,
    pub aspect_ratio: String,
    pub name: Option<String>,
    pub folder_id: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MatteResult {
    pub project: VideoProject,
    pub media: MediaAsset,
}
```

Move `parse_matte_hex`, `matte_pixel_size`, even/fit helpers, default naming, PNG generation, folder validation, file write, project media append, and `storage::save_project` from `codex/tools.rs` into `project/matte.rs`. Preserve the aspect set `Project`, `16:9`, `9:16`, `1:1`, `4:3`, `9:14`, and `2.4:1`.

- [ ] **Step 4: Route the Codex tool through the shared module**

`create_matte_payload` converts `CreateMatteArgs` to `MatteRequest`, calls `project::matte::create_matte`, and returns the existing JSON keys unchanged so Codex tool contracts do not regress.

- [ ] **Step 5: Add the direct Tauri command and TypeScript wrapper**

```rust
#[tauri::command]
fn create_matte_in_split_project_folder(
    project_dir: String,
    request: project::matte::MatteRequest,
) -> Result<project::matte::MatteResult, String> {
    project::matte::create_matte(Path::new(&project_dir), request)
        .map_err(|error| error.to_string())
}
```

Register the command in the existing `generate_handler!` list. Add:

```ts
export async function createMatteInSplitProjectFolder(input: {
  projectDir: string;
  request: { hex: string; aspectRatio: string; name?: string; folderId?: string };
}): Promise<{ project: VideoProject; media: MediaAsset }> {
  return invoke("create_matte_in_split_project_folder", input);
}
```

- [ ] **Step 6: Add the wrapper payload test**

```ts
await createMatteInSplitProjectFolder({
  projectDir: "/tmp/project",
  request: { hex: "#112233", aspectRatio: "16:9", folderId: "folder-1" },
});
expect(invokeMock).toHaveBeenCalledWith("create_matte_in_split_project_folder", {
  projectDir: "/tmp/project",
  request: { hex: "#112233", aspectRatio: "16:9", folderId: "folder-1" },
});
```

- [ ] **Step 7: Run Rust and TypeScript tests**

Run:

```bash
rtk cargo test --manifest-path src-tauri/Cargo.toml project::matte -- --test-threads=1
rtk cargo test --manifest-path src-tauri/Cargo.toml codex::tools:: -- --test-threads=1
rtk pnpm exec vitest run src/lib/project.test.ts -t "matte"
```

Expected: shared matte, Codex compatibility, and wrapper tests PASS.

- [ ] **Step 8: Commit functional matte creation**

```bash
rtk git add src-tauri/src/project/matte.rs src-tauri/src/project/mod.rs src-tauri/src/codex/tools.rs src-tauri/src/lib.rs src/lib/project.ts src/lib/project.test.ts
rtk git commit -m "feat(media): expose local matte creation"
```

### Task 5: Add Palmier's matte sheet to the overflow

**Files:**
- Create: `src/components/workspace/matte-sheet.tsx`
- Create: `src/components/workspace/matte-sheet.test.tsx`
- Modify: `src/components/workspace/media-bin.tsx`
- Modify: `src/components/workspace/media-bin.test.tsx`
- Modify: `src/components/workspace/editor-workspace.tsx`
- Modify: `src/components/workspace/editor-workspace.test.tsx`

- [ ] **Step 1: Write the failing sheet test**

```tsx
it("creates a solid matte with Palmier aspect choices", () => {
  const onCreate = vi.fn();
  render(<MatteSheet open timelineWidth={736} timelineHeight={400} busy={false} error={null} onOpenChange={vi.fn()} onCreate={onCreate} />);
  expect(screen.getByRole("dialog", { name: "Create Matte" })).toHaveClass("w-[280px]");
  fireEvent.change(screen.getByLabelText("Matte color"), { target: { value: "#112233" } });
  fireEvent.change(screen.getByLabelText("Matte aspect"), { target: { value: "16:9" } });
  expect(screen.getByText("710 × 400")).toBeVisible();
  fireEvent.click(screen.getByRole("button", { name: "Create Matte" }));
  expect(onCreate).toHaveBeenCalledWith({ hex: "#112233", aspectRatio: "16:9" });
});
```

- [ ] **Step 2: Run the test to verify failure**

Run: `rtk pnpm exec vitest run src/components/workspace/matte-sheet.test.tsx`

Expected: FAIL because the sheet does not exist.

- [ ] **Step 3: Implement the 280px sheet**

Use the same aspect math as Rust for preview text, an HTML color input with an adjacent hex field, the seven aspect options, current computed size, inline error, and a full-width warm off-white submit button. Escape and Cancel restore focus to the overflow trigger.

- [ ] **Step 4: Wire creation and project replacement**

`MediaBin` opens the sheet from `Create Matte`. `EditorWorkspace` calls `createMatteInSplitProjectFolder`, replaces its project state with `result.project`, selects `result.media.id`, and keeps the active media folder as `folderId`. Error and busy state remain in the sheet.

- [ ] **Step 5: Run component and integration tests**

Run: `rtk pnpm exec vitest run src/components/workspace/matte-sheet.test.tsx src/components/workspace/media-bin.test.tsx src/components/workspace/editor-workspace.test.tsx -t "Matte|matte"`

Expected: form, overflow, command payload, reload, selection, folder, busy, and error tests PASS.

- [ ] **Step 6: Commit the UI**

```bash
rtk git add src/components/workspace/matte-sheet.tsx src/components/workspace/matte-sheet.test.tsx
rtk git add -p src/components/workspace/media-bin.tsx
rtk git add -p src/components/workspace/media-bin.test.tsx
rtk git add -p src/components/workspace/editor-workspace.tsx
rtk git add -p src/components/workspace/editor-workspace.test.tsx
rtk git commit -m "feat(media): add matte creation sheet"
```

### Task 6: Attach the generation composer to the media pane

**Files:**
- Modify: `src/components/workspace/media-bin.tsx`
- Modify: `src/components/workspace/media-bin.test.tsx`

- [ ] **Step 1: Replace the fixed-sheet expectation with a failing drawer test**

```tsx
it("keeps generation attached to the media pane", () => {
  renderMediaBin();
  fireEvent.click(screen.getByRole("button", { name: "Generate media" }));
  const drawer = screen.getByTestId("media-generation-drawer");
  expect(drawer).toHaveClass("absolute", "inset-x-2", "bottom-2");
  expect(drawer).not.toHaveClass("fixed");
  expect(drawer).toHaveAttribute("data-pane-child", "true");
});
```

- [ ] **Step 2: Run the test to verify failure**

Run: `rtk pnpm exec vitest run src/components/workspace/media-bin.test.tsx -t "generation attached"`

Expected: FAIL because the desktop composer uses a viewport-fixed `xl` sheet.

- [ ] **Step 3: Make the media content column the positioning context**

Use `relative min-h-0 overflow-hidden`. The scrollable library receives bottom padding equal to the current drawer height plus 16px. The drawer uses `absolute inset-x-2 bottom-2 z-30` and exposes a keyboard/pointer resize separator. Clamp height so at least 120px of library remains visible.

- [ ] **Step 4: Match the compact composer hierarchy**

Header: image/video/audio icon segments left, history/close right. Body: First/Last and Reference tabs, clean click/drop slots, prompt. Footer: model, resolution, duration, aspect, cost, and submit in one compact row. Move optional Name and provider-specific controls under `Advanced` while retaining their values and validation.

- [ ] **Step 5: Keep lifecycle and provider behavior unchanged**

Update tests to open `Advanced` before querying optional fields. Preserve typed references, provider/model capability enforcement, queued/running/completed/failed/cancelled states, retry, history, multi-output selection, insert/replace, local cancellation, and provider cancellation.

- [ ] **Step 6: Run the complete media test file**

Run: `rtk pnpm exec vitest run src/components/workspace/media-bin.test.tsx`

Expected: every media, folder, search, generation, history, cancellation, output, import, and drawer test PASS.

- [ ] **Step 7: Commit the attached drawer**

```bash
rtk git add -p src/components/workspace/media-bin.tsx
rtk git add -p src/components/workspace/media-bin.test.tsx
rtk git commit -m "feat(generation): attach composer to media pane"
```

### Task 7: Make the direct thumbnail grid the default

**Files:**
- Modify: `src/components/workspace/media-bin.tsx`
- Modify: `src/components/workspace/media-bin.test.tsx`

- [ ] **Step 1: Add a failing default-view test**

```tsx
it("opens the library as a compact direct asset grid", () => {
  renderMediaBin({ media: fixtureMediaWithFolders });
  expect(screen.getByRole("group", { name: "Project media grid" })).toBeVisible();
  expect(screen.queryByRole("group", { name: "Project folder grid" })).not.toBeInTheDocument();
  expect(screen.getByRole("button", { name: "Folders" })).toHaveAttribute("aria-pressed", "false");
});
```

- [ ] **Step 2: Run the test to verify failure**

Run: `rtk pnpm exec vitest run src/components/workspace/media-bin.test.tsx -t "direct asset grid"`

Expected: FAIL because `libraryViewMode` defaults to `folder`.

- [ ] **Step 3: Default to flat while preserving folders**

Initialize `libraryViewMode` to `flat`. Keep Folders, Flat, and Grouped choices in the view control, preserve active folder navigation/drop targets, and remember the explicit user choice through the existing persisted media view state.

Use compact responsive columns with 8px gaps, image-dominant thumbnails, one-line names, small duration/AI badges, subtle selection outline, and no large centered kind icon when a real preview exists.

- [ ] **Step 4: Run folder, drag, sort, filter, and grid tests**

Run: `rtk pnpm exec vitest run src/components/workspace/media-bin.test.tsx -t "folder|Folder|grid|sort|filter|drag|drop"`

Expected: all focused tests PASS and folders remain fully reachable.

- [ ] **Step 5: Commit the default hierarchy**

```bash
rtk git add -p src/components/workspace/media-bin.tsx
rtk git add -p src/components/workspace/media-bin.test.tsx
rtk git commit -m "style(media): prioritize direct asset thumbnails"
```

### Task 8: Verify media/generation visually and update the tracker

**Files:**
- Modify: `scripts/browser-visual-qa.mjs`
- Modify: `docs/parity.md`
- Evidence: `output/parity-audit-2026-07-13/media-generation/`

- [ ] **Step 1: Add deterministic states and geometry assertions**

Add these deterministic states to `scripts/browser-visual-qa.mjs`:

```js
{ surface: "media", viewport: "desktop", width: 1440, height: 960, state: "default" },
{ surface: "media", viewport: "desktop", width: 1440, height: 960, state: "overflow" },
{ surface: "generation", viewport: "desktop", width: 1440, height: 960, state: "attached" },
{ surface: "captions", viewport: "desktop", width: 1440, height: 960, state: "workbench" },
{ surface: "audio", viewport: "desktop", width: 1440, height: 960, state: "speech" },
```

Assert rail width 42px; drawer bottom equals media pane bottom±2px; drawer width equals media content minus 16±2px; visible library height is at least 120px; no drawer rect crosses the media pane or viewport.

- [ ] **Step 2: Run focused functional gates**

Run:

```bash
rtk pnpm exec vitest run src/components/workspace/media-bin.test.tsx src/components/workspace/matte-sheet.test.tsx src/components/workspace/editor-workspace.test.tsx src/lib/project.test.ts
rtk cargo test --manifest-path src-tauri/Cargo.toml project::matte -- --test-threads=1
rtk pnpm lint
rtk pnpm build
rtk git diff --check
```

Expected: all commands exit 0.

- [ ] **Step 3: Capture and compare**

With Vite retained at `127.0.0.1:1420`, run:

```bash
rtk pnpm visual:qa:browser -- --url http://127.0.0.1:1420 --out output/parity-audit-2026-07-13/media-generation
rtk ffmpeg -y \
  -i output/parity-audit-2026-07-13/reference/06-generation.png \
  -i output/parity-audit-2026-07-13/media-generation/generation-attached-desktop.png \
  -filter_complex '[0:v]scale=700:960:force_original_aspect_ratio=decrease,pad=700:960:(ow-iw)/2:(oh-ih)/2:color=black[left];[1:v]scale=700:960:force_original_aspect_ratio=decrease,pad=700:960:(ow-iw)/2:(oh-ih)/2:color=black[right];[left][right]hstack=inputs=2' \
  -frames:v 1 output/parity-audit-2026-07-13/media-generation/palmier-video-creater-generation-side-by-side.png
rtk ffmpeg -y \
  -i output/parity-audit-2026-07-13/reference/07-media-overflow.png \
  -i output/parity-audit-2026-07-13/media-generation/media-overflow-desktop.png \
  -filter_complex '[0:v]scale=700:960:force_original_aspect_ratio=decrease,pad=700:960:(ow-iw)/2:(oh-ih)/2:color=black[left];[1:v]scale=700:960:force_original_aspect_ratio=decrease,pad=700:960:(ow-iw)/2:(oh-ih)/2:color=black[right];[left][right]hstack=inputs=2' \
  -frames:v 1 output/parity-audit-2026-07-13/media-generation/palmier-video-creater-media-overflow-side-by-side.png
```

Expected: the harness exits 0 and both 1400×960 comparison PNGs are non-empty. Inspect them and reject the slice for wrong rail width, toolbar wrapping, folder-first default, detached composer, form-heavy footer, incorrect radii/borders, or overflow clipping. Plan 05 later incorporates the passed captures and the Captions state into the complete comparison board.

- [ ] **Step 4: Update tracker truth**

Record Create Matte as implemented through the shared local Rust path, Organize with Agent as reviewed-prompt-only, and Video Creater-only provider lifecycle, semantic search, silence tools, nested folders, multi-output selection, and cancellation as preserved. Close media/generation visual rows only after comparison `review.json` records pass.

- [ ] **Step 5: Commit exact evidence wording**

```bash
rtk git add scripts/browser-visual-qa.mjs docs/parity.md
rtk git commit -m "docs(parity): verify Palmier media generation"
```
