# Project-Native Editor Import And Timeline Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build a project-backed editor slice where imported media is registered in `VideoProject.media` and caption timing can be changed directly from the timeline.

**Architecture:** Rust owns canonical project import and timeline patch validation. React owns editor interaction state, calls narrow Tauri adapters, and renders Media Bin plus timeline drag/resize affordances from the same project state. Native file picking is isolated behind a frontend adapter so the existing repo can compile without adding the Tauri dialog plugin in this pass.

**Tech Stack:** Rust/Tauri 2, React 19, TypeScript, Vitest, Testing Library, Tailwind, lucide-react.

---

## File Structure

- Create `src-tauri/src/project/import.rs`: supported extension detection, destination filename construction, file copying, media asset creation, and project save.
- Modify `src-tauri/src/project/mod.rs`: export the import module.
- Modify `src-tauri/src/main.rs`: expose `import_media_to_project` Tauri command.
- Modify `src-tauri/tests/project_patch.rs`: add project import tests alongside existing project storage/patch tests.
- Create `src/lib/project.ts`: TypeScript project/media types and Tauri command adapters.
- Modify `src/lib/timeline.ts`: add patch creation helpers for resize and left-edge caption trim math.
- Modify `src/lib/timeline.test.ts`: add red-green tests for caption timing helpers.
- Modify `src/components/workspace/media-bin.tsx`: render real media, selection, import button state, and errors.
- Create `src/components/workspace/media-bin.test.tsx`: component coverage for empty/imported/error states.
- Modify `src/components/workspace/timeline-editor.tsx`: add pointer drag and resize handles for captions.
- Create `src/components/workspace/timeline-editor.test.tsx`: component coverage for move and resize callbacks.
- Modify `src/components/workspace/editor-workspace.tsx`: own project state, import state, selected media, and validated timeline patch callbacks.
- Create `src/components/workspace/editor-workspace.test.tsx`: integration coverage for project media and timeline wiring.

---

### Task 1: Rust Project Media Import

**Files:**
- Create: `src-tauri/src/project/import.rs`
- Modify: `src-tauri/src/project/mod.rs`
- Modify: `src-tauri/tests/project_patch.rs`

- [ ] **Step 1: Write failing Rust import tests**

Append these tests to `src-tauri/tests/project_patch.rs`:

```rust
use video_creater_lib::project::import::{import_media_files, ImportMediaError};

#[test]
fn imports_supported_media_file_into_project_media_folder() {
    let dir = tempfile::tempdir().expect("temp project dir");
    let source_dir = tempfile::tempdir().expect("temp source dir");
    let source = source_dir.path().join("camera clip.mp4");
    std::fs::write(&source, b"video bytes").expect("source file");
    let project = sample_project();

    let result = import_media_files(dir.path(), project, &[source.clone()]).expect("import media");

    assert_eq!(result.imported.len(), 1);
    assert_eq!(result.skipped.len(), 0);
    assert_eq!(result.project.media.len(), 2);
    let imported = &result.imported[0];
    assert_eq!(imported.kind, MediaKind::Video);
    assert!(imported.relative_path.starts_with("media/"));
    assert!(imported.relative_path.ends_with(".mp4"));
    assert!(dir.path().join(&imported.relative_path).is_file());
    assert_eq!(
        std::fs::read(dir.path().join(&imported.relative_path)).expect("copied file"),
        b"video bytes"
    );

    let loaded = load_project(dir.path()).expect("saved project should load");
    assert_eq!(loaded.media.len(), 2);
    assert_eq!(loaded.media.last(), Some(imported));
}

#[test]
fn import_media_uses_distinct_destinations_for_duplicate_names() {
    let dir = tempfile::tempdir().expect("temp project dir");
    let source_dir = tempfile::tempdir().expect("temp source dir");
    let first = source_dir.path().join("clip.mp4");
    let second_dir = tempfile::tempdir().expect("second source dir");
    let second = second_dir.path().join("clip.mp4");
    std::fs::write(&first, b"first").expect("first source");
    std::fs::write(&second, b"second").expect("second source");

    let result = import_media_files(dir.path(), sample_project(), &[first, second])
        .expect("import duplicate names");

    assert_eq!(result.imported.len(), 2);
    assert_ne!(result.imported[0].relative_path, result.imported[1].relative_path);
}

#[test]
fn import_media_skips_unsupported_files_but_imports_supported_files() {
    let dir = tempfile::tempdir().expect("temp project dir");
    let source_dir = tempfile::tempdir().expect("temp source dir");
    let supported = source_dir.path().join("voice.wav");
    let unsupported = source_dir.path().join("notes.txt");
    std::fs::write(&supported, b"audio").expect("audio source");
    std::fs::write(&unsupported, b"notes").expect("notes source");

    let result = import_media_files(dir.path(), sample_project(), &[supported, unsupported])
        .expect("mixed import");

    assert_eq!(result.imported.len(), 1);
    assert_eq!(result.imported[0].kind, MediaKind::Audio);
    assert_eq!(result.skipped.len(), 1);
    assert!(result.skipped[0].reason.contains("unsupported"));
}

#[test]
fn import_media_rejects_all_unsupported_files_without_changing_project() {
    let dir = tempfile::tempdir().expect("temp project dir");
    let source_dir = tempfile::tempdir().expect("temp source dir");
    let unsupported = source_dir.path().join("notes.txt");
    std::fs::write(&unsupported, b"notes").expect("notes source");
    let project = sample_project();

    let error = import_media_files(dir.path(), project.clone(), &[unsupported])
        .expect_err("all unsupported should fail");

    assert_eq!(error, ImportMediaError::NoImportableFiles);
    assert!(load_project(dir.path()).is_err());
}
```

- [ ] **Step 2: Run Rust tests and verify they fail**

Run: `rtk cargo test --manifest-path src-tauri/Cargo.toml imports_supported_media_file_into_project_media_folder`

Expected: FAIL because `video_creater_lib::project::import` does not exist.

- [ ] **Step 3: Implement Rust import module**

Create `src-tauri/src/project/import.rs`:

```rust
use super::model::{MediaAsset, MediaKind, VideoProject};
use super::storage;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use thiserror::Error;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ImportSkippedFile {
    pub source_path: String,
    pub reason: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ImportMediaResult {
    pub project: VideoProject,
    pub imported: Vec<MediaAsset>,
    pub skipped: Vec<ImportSkippedFile>,
}

#[derive(Debug, Error, PartialEq)]
pub enum ImportMediaError {
    #[error("no source files were selected")]
    EmptySelection,
    #[error("no selected files can be imported")]
    NoImportableFiles,
    #[error("failed to save project after import: {0}")]
    SaveProject(String),
    #[error("failed to copy media from {source} to {destination}: {message}")]
    Copy {
        source: String,
        destination: String,
        message: String,
    },
}

pub fn import_media_files(
    project_dir: &Path,
    mut project: VideoProject,
    source_paths: &[PathBuf],
) -> Result<ImportMediaResult, ImportMediaError> {
    if source_paths.is_empty() {
        return Err(ImportMediaError::EmptySelection);
    }

    let mut imported = Vec::new();
    let mut skipped = Vec::new();

    for source_path in source_paths {
        match importable_media_kind(source_path) {
            Some(kind) if source_path.is_file() => {
                let asset = copy_media_asset(project_dir, source_path, kind)?;
                project.media.push(asset.clone());
                imported.push(asset);
            }
            Some(_) => skipped.push(ImportSkippedFile {
                source_path: source_path.display().to_string(),
                reason: "source path is not a file".to_string(),
            }),
            None => skipped.push(ImportSkippedFile {
                source_path: source_path.display().to_string(),
                reason: "unsupported media extension".to_string(),
            }),
        }
    }

    if imported.is_empty() {
        return Err(ImportMediaError::NoImportableFiles);
    }

    storage::save_project(project_dir, &project)
        .map_err(|error| ImportMediaError::SaveProject(error.to_string()))?;

    Ok(ImportMediaResult {
        project,
        imported,
        skipped,
    })
}

fn copy_media_asset(
    project_dir: &Path,
    source_path: &Path,
    kind: MediaKind,
) -> Result<MediaAsset, ImportMediaError> {
    let asset_id = format!("media-{}", uuid::Uuid::new_v4());
    let extension = source_path
        .extension()
        .and_then(|extension| extension.to_str())
        .unwrap_or("bin")
        .to_ascii_lowercase();
    let stem = source_path
        .file_stem()
        .and_then(|stem| stem.to_str())
        .map(sanitize_file_stem)
        .filter(|stem| !stem.is_empty())
        .unwrap_or_else(|| "media".to_string());
    let relative_path = format!("media/{asset_id}-{stem}.{extension}");
    let destination = project_dir.join(&relative_path);

    if let Some(parent) = destination.parent() {
        std::fs::create_dir_all(parent).map_err(|error| ImportMediaError::Copy {
            source: source_path.display().to_string(),
            destination: destination.display().to_string(),
            message: error.to_string(),
        })?;
    }

    std::fs::copy(source_path, &destination).map_err(|error| ImportMediaError::Copy {
        source: source_path.display().to_string(),
        destination: destination.display().to_string(),
        message: error.to_string(),
    })?;

    Ok(MediaAsset {
        id: asset_id,
        relative_path,
        kind,
        duration_seconds: 0.0,
        width: None,
        height: None,
        fps: None,
    })
}

fn sanitize_file_stem(stem: &str) -> String {
    stem.chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() || character == '-' || character == '_' {
                character.to_ascii_lowercase()
            } else {
                '-'
            }
        })
        .collect::<String>()
        .split('-')
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>()
        .join("-")
}

fn importable_media_kind(path: &Path) -> Option<MediaKind> {
    let extension = path.extension()?.to_str()?.to_ascii_lowercase();
    match extension.as_str() {
        "mp4" | "mov" | "m4v" | "webm" => Some(MediaKind::Video),
        "wav" | "mp3" | "m4a" | "aac" | "flac" => Some(MediaKind::Audio),
        "png" | "jpg" | "jpeg" | "webp" => Some(MediaKind::Image),
        _ => None,
    }
}
```

Modify `src-tauri/src/project/mod.rs`:

```rust
pub mod fixtures;
pub mod import;
pub mod model;
pub mod patch;
pub mod storage;
```

- [ ] **Step 4: Run Rust import tests and verify they pass**

Run: `rtk cargo test --manifest-path src-tauri/Cargo.toml import_media`

Expected: PASS for the new import tests.

---

### Task 2: Tauri Import Command And Frontend Project Adapter

**Files:**
- Modify: `src-tauri/src/main.rs`
- Create: `src/lib/project.ts`

- [ ] **Step 1: Write failing TypeScript adapter tests**

Create `src/lib/project.test.ts`:

```ts
import { describe, expect, it, vi } from "vitest";
import { importMediaToProject, type VideoProject } from "./project";

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(),
}));

const project: VideoProject = {
  schemaVersion: 1,
  id: "project-1",
  name: "Project",
  createdAt: "2026-06-13T00:00:00Z",
  updatedAt: "2026-06-13T00:00:00Z",
  media: [],
  transcripts: [],
  timeline: { durationSeconds: 0, tracks: [] },
  renderSettings: {
    width: 1920,
    height: 1080,
    fps: 24,
    loudnessLufs: -14,
    captions: "burn_in",
  },
  codexThreadId: null,
  jobs: [],
};

describe("project command adapters", () => {
  it("calls the Rust media import command with project dir, project, and source paths", async () => {
    const { invoke } = await import("@tauri-apps/api/core");
    vi.mocked(invoke).mockResolvedValue({
      project,
      imported: [],
      skipped: [],
    });

    await importMediaToProject({
      projectDir: "/tmp/project",
      project,
      sourcePaths: ["/tmp/source.mp4"],
    });

    expect(invoke).toHaveBeenCalledWith("import_media_to_project", {
      projectDir: "/tmp/project",
      project,
      sourcePaths: ["/tmp/source.mp4"],
    });
  });
});
```

- [ ] **Step 2: Run adapter test and verify it fails**

Run: `rtk pnpm test src/lib/project.test.ts`

Expected: FAIL because `src/lib/project.ts` does not exist.

- [ ] **Step 3: Add Tauri command**

Modify `src-tauri/src/main.rs`:

```rust
use video_creater_lib::project::import::{import_media_files, ImportMediaResult};
```

Add command:

```rust
#[tauri::command]
fn import_media_to_project(
    project_dir: String,
    project: VideoProject,
    source_paths: Vec<String>,
) -> Result<ImportMediaResult, String> {
    let project_dir = resolve_project_dir(&project_dir)?;
    let source_paths = source_paths.into_iter().map(PathBuf::from).collect::<Vec<_>>();
    import_media_files(&project_dir, project, &source_paths).map_err(|error| error.to_string())
}
```

Add `import_media_to_project` to `tauri::generate_handler![...]`.

- [ ] **Step 4: Add TypeScript project adapter**

Create `src/lib/project.ts`:

```ts
import { invoke } from "@tauri-apps/api/core";
import type { Timeline, TimelinePatch } from "./timeline";

export type MediaKind = "video" | "audio" | "image" | "generated";

export interface MediaAsset {
  id: string;
  relativePath: string;
  kind: MediaKind;
  durationSeconds: number;
  width: number | null;
  height: number | null;
  fps: number | null;
}

export interface VideoProject {
  schemaVersion: number;
  id: string;
  name: string;
  createdAt: string;
  updatedAt: string;
  media: MediaAsset[];
  transcripts: unknown[];
  timeline: Timeline;
  renderSettings: {
    width: number;
    height: number;
    fps: number;
    loudnessLufs: number;
    captions: "burn_in" | "mux" | "off";
  };
  codexThreadId: string | null;
  jobs: unknown[];
}

export interface ImportSkippedFile {
  sourcePath: string;
  reason: string;
}

export interface ImportMediaResult {
  project: VideoProject;
  imported: MediaAsset[];
  skipped: ImportSkippedFile[];
}

export async function importMediaToProject(input: {
  projectDir: string;
  project: VideoProject;
  sourcePaths: string[];
}): Promise<ImportMediaResult> {
  return invoke("import_media_to_project", input);
}

export async function applyTimelinePatchToProject(input: {
  project: VideoProject;
  patch: TimelinePatch;
}): Promise<VideoProject> {
  return invoke("apply_timeline_patch_to_project", input);
}
```

- [ ] **Step 5: Run adapter and Rust command tests**

Run: `rtk pnpm test src/lib/project.test.ts`

Expected: PASS.

Run: `rtk cargo test --manifest-path src-tauri/Cargo.toml import_media`

Expected: PASS.

---

### Task 3: Timeline Patch Helpers

**Files:**
- Modify: `src/lib/timeline.ts`
- Modify: `src/lib/timeline.test.ts`

- [ ] **Step 1: Write failing timeline helper tests**

Append to `src/lib/timeline.test.ts`:

```ts
import {
  createResizePatchFromDrag,
  createCaptionLeftResizePatches,
} from "./timeline";

it("creates a typed resize patch from a resize result", () => {
  const patch = createResizePatchFromDrag({
    itemId: "caption-1",
    durationSeconds: 1.8,
  });

  expect(patch).toEqual({
    type: "resizeItem",
    itemId: "caption-1",
    durationSeconds: 1.8,
  });
});

it("creates move and resize patches for left-edge caption resizing", () => {
  const patches = createCaptionLeftResizePatches({
    itemId: "caption-1",
    targetTrackId: "track-captions",
    originalStartSeconds: 1,
    originalDurationSeconds: 2,
    nextStartSeconds: 1.4,
    minimumDurationSeconds: 0.1,
  });

  expect(patches).toEqual([
    {
      type: "moveItem",
      itemId: "caption-1",
      targetTrackId: "track-captions",
      startSeconds: 1.4,
    },
    {
      type: "resizeItem",
      itemId: "caption-1",
      durationSeconds: 1.6,
    },
  ]);
});

it("clamps left-edge resize to keep caption duration above the minimum", () => {
  const patches = createCaptionLeftResizePatches({
    itemId: "caption-1",
    targetTrackId: "track-captions",
    originalStartSeconds: 1,
    originalDurationSeconds: 2,
    nextStartSeconds: 2.98,
    minimumDurationSeconds: 0.25,
  });

  expect(patches[0]).toMatchObject({ startSeconds: 2.75 });
  expect(patches[1]).toMatchObject({ durationSeconds: 0.25 });
});
```

- [ ] **Step 2: Run helper tests and verify they fail**

Run: `rtk pnpm test src/lib/timeline.test.ts`

Expected: FAIL because helper functions do not exist.

- [ ] **Step 3: Implement timeline helpers**

Add to `src/lib/timeline.ts`:

```ts
export function createResizePatchFromDrag(input: {
  itemId: string;
  durationSeconds: number;
}): TimelinePatch {
  return {
    type: "resizeItem",
    itemId: input.itemId,
    durationSeconds: input.durationSeconds,
  };
}

export function createCaptionLeftResizePatches(input: {
  itemId: string;
  targetTrackId: string;
  originalStartSeconds: number;
  originalDurationSeconds: number;
  nextStartSeconds: number;
  minimumDurationSeconds: number;
}): TimelinePatch[] {
  const originalEndSeconds = input.originalStartSeconds + input.originalDurationSeconds;
  const clampedStartSeconds = Math.max(
    0,
    Math.min(input.nextStartSeconds, originalEndSeconds - input.minimumDurationSeconds),
  );
  const durationSeconds = originalEndSeconds - clampedStartSeconds;

  return [
    createMovePatchFromDrag({
      itemId: input.itemId,
      targetTrackId: input.targetTrackId,
      startSeconds: Number(clampedStartSeconds.toFixed(3)),
    }),
    createResizePatchFromDrag({
      itemId: input.itemId,
      durationSeconds: Number(durationSeconds.toFixed(3)),
    }),
  ];
}
```

- [ ] **Step 4: Run helper tests and verify they pass**

Run: `rtk pnpm test src/lib/timeline.test.ts`

Expected: PASS.

---

### Task 4: Media Bin UI

**Files:**
- Modify: `src/components/workspace/media-bin.tsx`
- Create: `src/components/workspace/media-bin.test.tsx`

- [ ] **Step 1: Write failing Media Bin tests**

Create `src/components/workspace/media-bin.test.tsx`:

```tsx
import "@testing-library/jest-dom/vitest";
import { fireEvent, render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { MediaBin } from "./media-bin";
import type { MediaAsset } from "@/lib/project";

const media: MediaAsset[] = [
  {
    id: "media-2",
    relativePath: "media/media-2-camera.mp4",
    kind: "video",
    durationSeconds: 0,
    width: null,
    height: null,
    fps: null,
  },
];

describe("MediaBin", () => {
  it("renders an empty project media state", () => {
    render(<MediaBin media={[]} />);

    expect(screen.getByText("No media imported")).toBeInTheDocument();
  });

  it("renders imported media and supports selection", () => {
    const onSelectMedia = vi.fn();

    render(<MediaBin media={media} selectedMediaId={null} onSelectMedia={onSelectMedia} />);

    fireEvent.click(screen.getByRole("button", { name: /camera.mp4/i }));

    expect(onSelectMedia).toHaveBeenCalledWith("media-2");
  });

  it("shows import error and disables import while importing", () => {
    render(
      <MediaBin
        media={media}
        importStatus="importing"
        importError="Import failed"
        onImport={vi.fn()}
      />,
    );

    expect(screen.getByRole("button", { name: /import media/i })).toBeDisabled();
    expect(screen.getByText("Import failed")).toBeInTheDocument();
  });
});
```

- [ ] **Step 2: Run Media Bin tests and verify they fail**

Run: `rtk pnpm test src/components/workspace/media-bin.test.tsx`

Expected: FAIL because `MediaBin` props are not implemented.

- [ ] **Step 3: Implement Media Bin props and states**

Replace `src/components/workspace/media-bin.tsx` with a component that:

- accepts `media`, `selectedMediaId`, `importStatus`, `importError`, `onImport`, and `onSelectMedia`
- renders `Import media` button with an `Upload` icon
- renders empty state text `No media imported`
- renders media rows as buttons labelled by filename
- renders selected row state
- renders error copy below the import button

- [ ] **Step 4: Run Media Bin tests and verify they pass**

Run: `rtk pnpm test src/components/workspace/media-bin.test.tsx`

Expected: PASS.

---

### Task 5: Timeline Editor Caption Drag And Resize

**Files:**
- Modify: `src/components/workspace/timeline-editor.tsx`
- Create: `src/components/workspace/timeline-editor.test.tsx`

- [ ] **Step 1: Write failing Timeline Editor tests**

Create `src/components/workspace/timeline-editor.test.tsx`:

```tsx
import "@testing-library/jest-dom/vitest";
import { fireEvent, render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { sampleTimeline } from "@/lib/timeline";
import { TimelineEditor } from "./timeline-editor";

describe("TimelineEditor", () => {
  it("creates a move patch when a caption body is dragged horizontally", () => {
    const onPatch = vi.fn();
    render(<TimelineEditor timeline={sampleTimeline} selectedItemId="caption-1" onTimelinePatch={onPatch} />);

    const caption = screen.getByRole("button", { name: /caption 1/i });
    fireEvent.pointerDown(caption, { clientX: 100, pointerId: 1 });
    fireEvent.pointerMove(caption, { clientX: 180, pointerId: 1 });
    fireEvent.pointerUp(caption, { clientX: 180, pointerId: 1 });

    expect(onPatch).toHaveBeenCalledWith({
      type: "moveItem",
      itemId: "caption-1",
      targetTrackId: "track-captions",
      startSeconds: 1.65,
    });
  });

  it("creates a resize patch when a caption right handle is dragged", () => {
    const onPatch = vi.fn();
    render(<TimelineEditor timeline={sampleTimeline} selectedItemId="caption-1" onTimelinePatch={onPatch} />);

    const handle = screen.getByLabelText("Resize Caption 1 right edge");
    fireEvent.pointerDown(handle, { clientX: 100, pointerId: 1 });
    fireEvent.pointerMove(handle, { clientX: 140, pointerId: 1 });
    fireEvent.pointerUp(handle, { clientX: 140, pointerId: 1 });

    expect(onPatch).toHaveBeenCalledWith({
      type: "resizeItem",
      itemId: "caption-1",
      durationSeconds: 1.85,
    });
  });
});
```

- [ ] **Step 2: Run Timeline Editor tests and verify they fail**

Run: `rtk pnpm test src/components/workspace/timeline-editor.test.tsx`

Expected: FAIL because `onTimelinePatch` and handles are not implemented.

- [ ] **Step 3: Implement pointer drag and resize**

Modify `src/components/workspace/timeline-editor.tsx` to:

- accept `onTimelinePatch?: (patch: TimelinePatch) => void`
- keep active pointer interaction in component state
- convert horizontal pixel delta to seconds using `pixelsPerSecond`
- call `createMovePatchFromDrag` on body drag pointer up
- call `createResizePatchFromDrag` on right handle pointer up
- render left and right handles only for selected captions
- prevent handle pointer events from also selecting/dragging the body
- clamp start at `0`
- clamp duration to at least `0.1`

- [ ] **Step 4: Run Timeline Editor tests and verify they pass**

Run: `rtk pnpm test src/components/workspace/timeline-editor.test.tsx`

Expected: PASS.

---

### Task 6: Editor Workspace Integration

**Files:**
- Modify: `src/components/workspace/editor-workspace.tsx`
- Modify: `src/App.tsx` if prop names require cleanup
- Create: `src/components/workspace/editor-workspace.test.tsx`

- [ ] **Step 1: Write failing workspace integration test**

Create `src/components/workspace/editor-workspace.test.tsx`:

```tsx
import "@testing-library/jest-dom/vitest";
import { render, screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import { EditorWorkspace } from "./editor-workspace";

describe("EditorWorkspace", () => {
  it("renders project media through the media bin", () => {
    render(<EditorWorkspace />);

    expect(screen.getByText(/media/i)).toBeInTheDocument();
    expect(screen.getByText(/input.mp4|no media imported/i)).toBeInTheDocument();
  });
});
```

- [ ] **Step 2: Implement project-backed workspace state**

Modify `EditorWorkspace` to:

- create a sample `VideoProject` from the existing sample timeline and media
- pass `project.media` to `MediaBin`
- pass `project.timeline` to `TimelineEditor`
- apply caption text edits by updating `project.timeline`
- apply timeline patches by calling a workspace-local helper that mirrors the Rust patch contract for current in-memory state; keep the Tauri `applyTimelinePatchToProject` adapter available for persisted project flows
- wire `onTimelinePatch` to update selected caption timing
- wire `onImport` to set import state and call `importMediaToProject` when source paths are available

- [ ] **Step 3: Run workspace or existing component tests**

Run: `rtk pnpm test src/components/workspace`

Expected: PASS.

---

### Task 7: Verification And Commit

**Files:**
- All changed files

- [ ] **Step 1: Run frontend tests**

Run: `rtk pnpm test`

Expected: PASS.

- [ ] **Step 2: Run Rust tests**

Run: `rtk cargo test --manifest-path src-tauri/Cargo.toml`

Expected: PASS.

- [ ] **Step 3: Run lint/typecheck**

Run: `rtk pnpm lint`

Expected: PASS.

- [ ] **Step 4: Inspect changed files**

Run: `rtk git status --short`

Expected: only intentional source, tests, and plan changes.

- [ ] **Step 5: Commit implementation**

Run:

```bash
rtk git add docs/superpowers/plans/2026-06-13-project-native-editor-import-timeline.md src src-tauri
rtk git commit -m "feat: add project media import editor slice"
```

Expected: commit succeeds on branch `codex/project-native-editor-import`.
