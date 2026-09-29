# Tauri Video Editor Foundation Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build the first working foundation slice for the local-first Tauri video editor: React TypeScript UI, shadcn/ui setup, Rust project model, timeline patch validation, and a visible editor shell prepared for dnd-timeline interaction binding.

**Architecture:** Rust owns the canonical project model and timeline validation. React renders the editor workspace and calls Tauri commands for project creation, patch application, and persistence. dnd-timeline is introduced as the headless timeline interaction layer, while visible controls come from shadcn/ui primitives.

**Tech Stack:** Tauri 2, Rust, React 19, TypeScript, Vite, pnpm, shadcn/ui, Tailwind CSS, dnd-timeline, Vitest, cargo test.

---

## Scope Check

The approved design spec covers a full MVP with rendering, transcription, Codex app-server, and HyperFrames sidecars. That is too broad for one implementation plan. This plan implements the foundation vertical slice that future subsystem plans need:

- Project create/open/save shape.
- Canonical Rust timeline schema.
- Typed timeline patch validation.
- React editor workspace using shadcn/ui.
- Timeline adapter layer shaped for dnd-timeline interaction binding.
- TypeScript and Rust tests for the shared behaviors.

Rendering, Codex app-server, transcription, HyperFrames sidecar implementation, and full dnd-timeline drag/resize wiring each get their own follow-up plan after this foundation is passing.

## File Structure

Create or modify these files:

- `package.json`: pnpm scripts and frontend/dev dependencies.
- `pnpm-workspace.yaml`: workspace root declaration.
- `index.html`: Vite entry.
- `vite.config.ts`: React/Vite config for Tauri.
- `tsconfig.json`: TypeScript project settings.
- `tsconfig.node.json`: TypeScript settings for Vite config.
- `components.json`: shadcn/ui configuration.
- `postcss.config.js`: Tailwind PostCSS config.
- `tailwind.config.ts`: Tailwind content and theme config.
- `src/main.tsx`: React root.
- `src/App.tsx`: Workspace shell composition.
- `src/index.css`: Tailwind base plus editor-specific CSS.
- `src/lib/utils.ts`: shadcn class-name helper.
- `src/lib/timeline.ts`: TypeScript timeline types and adapter helpers.
- `src/lib/timeline.test.ts`: TypeScript unit tests for adapter helpers.
- `src/components/ui/button.tsx`: shadcn button component.
- `src/components/ui/card.tsx`: shadcn card component.
- `src/components/ui/separator.tsx`: shadcn separator component.
- `src/components/workspace/editor-workspace.tsx`: Main editor workspace layout.
- `src/components/workspace/timeline-editor.tsx`: timeline view using the adapter data shape that dnd-timeline will control in the next timeline interaction plan.
- `src/components/workspace/agent-panel.tsx`: Static foundation agent panel.
- `src/components/workspace/media-bin.tsx`: Static foundation media bin.
- `src/components/workspace/preview-panel.tsx`: Static foundation preview panel.
- `src-tauri/Cargo.toml`: Rust package and dependencies.
- `src-tauri/tauri.conf.json`: Tauri app configuration.
- `src-tauri/icons/icon.png`: Minimal placeholder icon required by Tauri context generation.
- `src-tauri/build.rs`: Tauri build script.
- `src-tauri/src/main.rs`: Tauri entrypoint and command registration.
- `src-tauri/src/project/mod.rs`: Project module exports.
- `src-tauri/src/project/model.rs`: Canonical project and timeline types.
- `src-tauri/src/project/patch.rs`: Timeline patch application and validation.
- `src-tauri/src/project/storage.rs`: Project folder creation and JSON persistence.
- `src-tauri/src/project/fixtures.rs`: Test fixture builders.
- `src-tauri/tests/project_patch.rs`: Rust integration tests.
- `.gitignore`: ignore generated dependencies/build output.

## Task 1: Scaffold Package And Tauri Config

**Files:**
- Create: `package.json`
- Create: `pnpm-workspace.yaml`
- Create: `index.html`
- Create: `vite.config.ts`
- Create: `tsconfig.json`
- Create: `tsconfig.node.json`
- Create: `src/main.tsx`
- Create: `src/App.tsx`
- Create: `src/index.css`
- Create: `src-tauri/Cargo.toml`
- Create: `src-tauri/Cargo.lock`
- Create: `src-tauri/tauri.conf.json`
- Create: `src-tauri/icons/icon.png`
- Create: `src-tauri/build.rs`
- Create: `src-tauri/src/lib.rs`
- Create: `src-tauri/src/main.rs`
- Modify: `.gitignore`

- [ ] **Step 1: Add workspace package files**

Create `package.json`:

```json
{
  "name": "video-creater",
  "version": "0.1.0",
  "private": true,
  "type": "module",
  "scripts": {
    "dev": "vite --host 127.0.0.1",
    "tauri": "tauri",
    "tauri:dev": "tauri dev",
    "build": "tsc && vite build",
    "test": "vitest run --passWithNoTests",
    "test:watch": "vitest",
    "lint": "tsc --noEmit && tsc -p tsconfig.node.json --noEmit",
    "rust:test": "cargo test --manifest-path src-tauri/Cargo.toml",
    "verify": "pnpm lint && pnpm test && pnpm rust:test"
  },
  "dependencies": {
    "@dnd-kit/core": "^6.3.1",
    "@radix-ui/react-slot": "^1.1.1",
    "@tauri-apps/api": "2.11.0",
    "class-variance-authority": "^0.7.1",
    "clsx": "^2.1.1",
    "dnd-timeline": "^3.1.0",
    "lucide-react": "^0.468.0",
    "react": "^19.0.0",
    "react-dom": "^19.0.0",
    "tailwind-merge": "^2.5.5"
  },
  "devDependencies": {
    "@tauri-apps/cli": "2.11.2",
    "@testing-library/jest-dom": "^6.6.3",
    "@testing-library/react": "^16.1.0",
    "@types/node": "^22.10.2",
    "@types/react": "^19.0.1",
    "@types/react-dom": "^19.0.2",
    "@vitejs/plugin-react": "^4.3.4",
    "autoprefixer": "^10.4.20",
    "jsdom": "^25.0.1",
    "tailwindcss": "^3.4.17",
    "typescript": "^5.7.2",
    "vite": "^6.0.5",
    "vitest": "^3.2.6"
  }
}
```

Create `pnpm-workspace.yaml`:

```yaml
packages:
  - "."
```

- [ ] **Step 2: Add Vite and TypeScript config**

Create `index.html`:

```html
<!doctype html>
<html lang="en">
  <head>
    <meta charset="UTF-8" />
    <meta name="viewport" content="width=device-width, initial-scale=1.0" />
    <title>Video Creater</title>
  </head>
  <body>
    <div id="root"></div>
    <script type="module" src="/src/main.tsx"></script>
  </body>
</html>
```

Create `vite.config.ts`:

```ts
import path from "node:path";
import react from "@vitejs/plugin-react";
import { defineConfig } from "vitest/config";

export default defineConfig({
  plugins: [react()],
  clearScreen: false,
  server: {
    port: 1420,
    strictPort: true,
    host: "127.0.0.1",
  },
  envPrefix: ["VITE_", "TAURI_"],
  resolve: {
    alias: {
      "@": path.resolve(__dirname, "./src"),
    },
  },
  test: {
    environment: "jsdom",
    globals: true,
    setupFiles: [],
  },
});
```

Create `tsconfig.json`:

```json
{
  "compilerOptions": {
    "target": "ES2020",
    "useDefineForClassFields": true,
    "lib": ["DOM", "DOM.Iterable", "ES2020"],
    "allowJs": false,
    "skipLibCheck": true,
    "esModuleInterop": true,
    "allowSyntheticDefaultImports": true,
    "strict": true,
    "forceConsistentCasingInFileNames": true,
    "module": "ESNext",
    "moduleResolution": "Node",
    "resolveJsonModule": true,
    "isolatedModules": true,
    "noEmit": true,
    "jsx": "react-jsx",
    "baseUrl": ".",
    "paths": {
      "@/*": ["src/*"]
    }
  },
  "include": ["src"],
  "references": [{ "path": "./tsconfig.node.json" }]
}
```

Create `tsconfig.node.json`:

```json
{
  "compilerOptions": {
    "composite": true,
    "target": "ES2020",
    "module": "ESNext",
    "moduleResolution": "Bundler",
    "allowSyntheticDefaultImports": true,
    "skipLibCheck": true,
    "strict": true
  },
  "include": ["vite.config.ts"]
}
```

- [ ] **Step 3: Add Tauri Rust config**

Create `src-tauri/Cargo.toml`:

```toml
[package]
name = "video-creater"
version = "0.1.0"
description = "Local-first agent-assisted video editor"
authors = ["olhapi"]
edition = "2021"
rust-version = "1.77"

[lib]
name = "video_creater_lib"
crate-type = ["staticlib", "cdylib", "rlib"]

[[bin]]
name = "video-creater"
path = "src/main.rs"

[build-dependencies]
tauri-build = { version = "2.6.2", features = [] }

[dependencies]
anyhow = "1.0.95"
chrono = { version = "0.4.39", features = ["serde"] }
serde = { version = "1.0.217", features = ["derive"] }
serde_json = "1.0.134"
tauri = { version = "2.11.2", features = [] }
thiserror = "2.0.9"
uuid = { version = "1.11.0", features = ["v4", "serde"] }

[dev-dependencies]
tempfile = "3.14.0"
```

Create `src-tauri/tauri.conf.json`:

```json
{
  "$schema": "https://schema.tauri.app/config/2",
  "productName": "Video Creater",
  "version": "0.1.0",
  "identifier": "com.olhapi.video-creater",
  "build": {
    "beforeDevCommand": "pnpm dev",
    "devUrl": "http://127.0.0.1:1420",
    "beforeBuildCommand": "pnpm build",
    "frontendDist": "../dist"
  },
  "app": {
    "windows": [
      {
        "title": "Video Creater",
        "width": 1440,
        "height": 920,
        "minWidth": 1100,
        "minHeight": 760
      }
    ],
    "security": {
      "csp": null
    }
  },
  "bundle": {
    "active": true,
    "targets": "all",
    "icon": []
  }
}
```

Create `src-tauri/build.rs`:

```rust
fn main() {
    tauri_build::build();
}
```

Create `src-tauri/icons/icon.png` as a minimal placeholder PNG. Tauri's `generate_context!()` macro reads this default icon path during Rust compilation, so `pnpm rust:test` can fail even when `bundle.icon` is empty if the file is missing.

- [ ] **Step 4: Add minimal placeholder entry files**

These files only make the scaffold compile. Later tasks replace or enrich them with the real project model, Tauri command registration, Tailwind, shadcn/ui, and editor workspace.

Create `src-tauri/src/lib.rs`:

```rust
pub fn run() {
    // Real Tauri runtime setup is added by later foundation tasks.
}
```

Create `src-tauri/src/main.rs`:

```rust
fn main() {
    video_creater_lib::run();
}
```

Create `src/App.tsx`:

```tsx
export function App() {
  return (
    <main className="app-shell">
      <h1>Video Creater</h1>
    </main>
  );
}

export default App;
```

Create `src/index.css`:

```css
:root {
  color: #202124;
  background: #f7f7f4;
  font-family:
    Inter, ui-sans-serif, system-ui, -apple-system, BlinkMacSystemFont, "Segoe UI",
    sans-serif;
}

body {
  margin: 0;
}

.app-shell {
  min-height: 100vh;
  display: grid;
  place-items: center;
}
```

Create `src/main.tsx`:

```tsx
import { StrictMode } from "react";
import { createRoot } from "react-dom/client";
import App from "./App";
import "./index.css";

const root = document.getElementById("root");

if (!root) {
  throw new Error("Root element not found");
}

createRoot(root).render(
  <StrictMode>
    <App />
  </StrictMode>,
);
```

- [ ] **Step 5: Update `.gitignore`**

Replace `.gitignore` with:

```gitignore
.superpowers/
.worktrees/
.playwright-mcp/
.pnpm-store/
node_modules/
dist/
target/
src-tauri/gen/
src-tauri/target/
*.tsbuildinfo
*.log
.DS_Store
```

- [ ] **Step 6: Install dependencies**

Run:

```bash
pnpm install
```

Expected: `pnpm-lock.yaml` is created or updated and dependencies install without errors.

- [ ] **Step 7: Verify the scaffold checks pass**

Run:

```bash
pnpm lint
pnpm test
pnpm rust:test
pnpm build
pnpm tauri info
```

Expected: PASS. The frontend and Rust entry files are placeholders only; later tasks replace or enrich them.

- [ ] **Step 8: Commit scaffold config**

Run:

```bash
git add .gitignore package.json pnpm-workspace.yaml pnpm-lock.yaml index.html vite.config.ts tsconfig.json tsconfig.node.json src src-tauri/Cargo.toml src-tauri/Cargo.lock src-tauri/tauri.conf.json src-tauri/icons/icon.png src-tauri/build.rs src-tauri/src
git commit -m "chore: scaffold Tauri workspace"
```

## Task 2: Add Rust Project Model

**Files:**
- Modify: `src-tauri/src/main.rs`
- Modify: `src-tauri/src/lib.rs`
- Create: `src-tauri/src/project/mod.rs`
- Create: `src-tauri/src/project/model.rs`
- Create: `src-tauri/src/project/fixtures.rs`

- [ ] **Step 1: Write the Rust project model**

Create `src-tauri/src/project/model.rs`:

```rust
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct VideoProject {
    pub schema_version: u32,
    pub id: String,
    pub name: String,
    pub created_at: String,
    pub updated_at: String,
    pub media: Vec<MediaAsset>,
    pub transcripts: Vec<Transcript>,
    pub timeline: Timeline,
    pub render_settings: RenderSettings,
    pub codex_thread_id: Option<String>,
    pub jobs: Vec<JobSummary>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct MediaAsset {
    pub id: String,
    pub relative_path: String,
    pub kind: MediaKind,
    pub duration_seconds: f64,
    pub width: Option<u32>,
    pub height: Option<u32>,
    pub fps: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum MediaKind {
    Video,
    Audio,
    Image,
    Generated,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Transcript {
    pub id: String,
    pub media_id: String,
    pub words: Vec<TranscriptWord>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct TranscriptWord {
    pub text: String,
    pub start_seconds: f64,
    pub end_seconds: f64,
    pub confidence: Option<f64>,
    pub speaker: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Timeline {
    pub duration_seconds: f64,
    pub tracks: Vec<TimelineTrack>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct TimelineTrack {
    pub id: String,
    pub name: String,
    pub kind: TrackKind,
    pub locked: bool,
    pub items: Vec<TimelineItem>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum TrackKind {
    Video,
    HyperframeScene,
    Overlay,
    Caption,
    Audio,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct TimelineItem {
    pub id: String,
    pub kind: TimelineItemKind,
    pub start_seconds: f64,
    pub duration_seconds: f64,
    pub source: TimelineSource,
    pub label: String,
    pub properties: BTreeMap<String, serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum TimelineItemKind {
    VideoClip,
    HyperframeScene,
    Overlay,
    Caption,
    AudioClip,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum TimelineSource {
    Media { media_id: String },
    Generated { artifact_id: String },
    Text { text: String },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct RenderSettings {
    pub width: u32,
    pub height: u32,
    pub fps: f64,
    pub loudness_lufs: f64,
    pub captions: CaptionRenderMode,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum CaptionRenderMode {
    BurnIn,
    Mux,
    Off,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct JobSummary {
    pub id: String,
    pub kind: String,
    pub status: JobStatus,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum JobStatus {
    Queued,
    Running,
    Progress,
    Blocked,
    Failed,
    Cancelled,
    Completed,
}

impl VideoProject {
    pub fn new_empty(id: String, name: String, now: String) -> Self {
        Self {
            schema_version: 1,
            id,
            name,
            created_at: now.clone(),
            updated_at: now,
            media: Vec::new(),
            transcripts: Vec::new(),
            timeline: Timeline {
                duration_seconds: 0.0,
                tracks: vec![
                    TimelineTrack::empty("track-video", "Video", TrackKind::Video),
                    TimelineTrack::empty("track-scenes", "HyperFrames", TrackKind::HyperframeScene),
                    TimelineTrack::empty("track-overlays", "Overlays", TrackKind::Overlay),
                    TimelineTrack::empty("track-captions", "Captions", TrackKind::Caption),
                    TimelineTrack::empty("track-audio", "Audio", TrackKind::Audio),
                ],
            },
            render_settings: RenderSettings {
                width: 1920,
                height: 1080,
                fps: 24.0,
                loudness_lufs: -14.0,
                captions: CaptionRenderMode::BurnIn,
            },
            codex_thread_id: None,
            jobs: Vec::new(),
        }
    }
}

impl TimelineTrack {
    pub fn empty(id: &str, name: &str, kind: TrackKind) -> Self {
        Self {
            id: id.to_string(),
            name: name.to_string(),
            kind,
            locked: false,
            items: Vec::new(),
        }
    }
}
```

- [ ] **Step 2: Add fixture helpers**

Create `src-tauri/src/project/fixtures.rs`:

```rust
use super::model::*;
use std::collections::BTreeMap;

pub fn sample_project() -> VideoProject {
    let mut project = VideoProject::new_empty(
        "project-test".to_string(),
        "Test Project".to_string(),
        "2026-06-11T00:00:00Z".to_string(),
    );

    project.media.push(MediaAsset {
        id: "media-1".to_string(),
        relative_path: "media/input.mp4".to_string(),
        kind: MediaKind::Video,
        duration_seconds: 12.0,
        width: Some(1920),
        height: Some(1080),
        fps: Some(24.0),
    });

    project.timeline.duration_seconds = 4.0;
    project.timeline.tracks[0].items.push(TimelineItem {
        id: "item-1".to_string(),
        kind: TimelineItemKind::VideoClip,
        start_seconds: 0.0,
        duration_seconds: 4.0,
        source: TimelineSource::Media {
            media_id: "media-1".to_string(),
        },
        label: "Opening clip".to_string(),
        properties: BTreeMap::new(),
    });

    project
}
```

- [ ] **Step 3: Add module exports and Tauri entry**

Create `src-tauri/src/project/mod.rs`:

```rust
pub mod fixtures;
pub mod model;
pub mod patch;
pub mod storage;
```

Replace `src-tauri/src/lib.rs`:

```rust
pub mod project;
```

Replace `src-tauri/src/main.rs`:

```rust
use video_creater_lib::project::model::VideoProject;
use tauri::Manager;

#[tauri::command]
fn create_empty_project(name: String) -> VideoProject {
    let now = chrono::Utc::now().to_rfc3339();
    VideoProject::new_empty(uuid::Uuid::new_v4().to_string(), name, now)
}

pub fn run() {
    tauri::Builder::default()
        .setup(|app| {
            app.manage("video-creater");
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![create_empty_project])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

fn main() {
    run();
}
```

- [ ] **Step 4: Add temporary empty modules for compile**

Create `src-tauri/src/project/patch.rs`:

```rust
use thiserror::Error;

#[derive(Debug, Error, PartialEq)]
pub enum TimelinePatchError {
    #[error("timeline patch support is not registered")]
    NotRegistered,
}
```

Create `src-tauri/src/project/storage.rs`:

```rust
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ProjectStorageError {
    #[error("project storage support is not registered")]
    NotRegistered,
}
```

- [ ] **Step 5: Run Rust tests**

Run:

```bash
pnpm rust:test
```

Expected: PASS. There are no assertions yet, but the Rust project compiles.

Note: this step depends on the Task 1 scaffold icon at `src-tauri/icons/icon.png` because `tauri::generate_context!()` reads the default icon path during compilation.

- [ ] **Step 6: Commit Rust model**

Run:

```bash
git add src-tauri/src/main.rs src-tauri/src/lib.rs src-tauri/src/project
git commit -m "feat: add canonical project model"
```

## Task 3: Add Timeline Patch Validation

**Files:**
- Modify: `src-tauri/src/project/patch.rs`
- Create: `src-tauri/tests/project_patch.rs`

- [ ] **Step 1: Write failing Rust patch tests**

Create `src-tauri/tests/project_patch.rs`:

```rust
use video_creater_lib::project::fixtures::sample_project;
use video_creater_lib::project::model::*;
use video_creater_lib::project::patch::{apply_timeline_patch, TimelinePatch, TimelinePatchError};

#[test]
fn moves_item_when_track_and_item_exist() {
    let mut project = sample_project();

    apply_timeline_patch(
        &mut project,
        TimelinePatch::MoveItem {
            item_id: "item-1".to_string(),
            target_track_id: "track-video".to_string(),
            start_seconds: 2.0,
        },
    )
    .expect("patch should apply");

    let item = &project.timeline.tracks[0].items[0];
    assert_eq!(item.start_seconds, 2.0);
    assert_eq!(project.timeline.duration_seconds, 6.0);
}

#[test]
fn rejects_negative_start() {
    let mut project = sample_project();

    let error = apply_timeline_patch(
        &mut project,
        TimelinePatch::MoveItem {
            item_id: "item-1".to_string(),
            target_track_id: "track-video".to_string(),
            start_seconds: -0.1,
        },
    )
    .expect_err("negative start must fail");

    assert_eq!(error, TimelinePatchError::NegativeTime);
}

#[test]
fn rejects_track_type_mismatch() {
    let mut project = sample_project();

    let error = apply_timeline_patch(
        &mut project,
        TimelinePatch::MoveItem {
            item_id: "item-1".to_string(),
            target_track_id: "track-captions".to_string(),
            start_seconds: 1.0,
        },
    )
    .expect_err("video item cannot move to caption track");

    assert_eq!(
        error,
        TimelinePatchError::TrackTypeMismatch {
            item_kind: TimelineItemKind::VideoClip,
            track_kind: TrackKind::Caption
        }
    );
}

#[test]
fn resizes_item_duration() {
    let mut project = sample_project();

    apply_timeline_patch(
        &mut project,
        TimelinePatch::ResizeItem {
            item_id: "item-1".to_string(),
            duration_seconds: 6.5,
        },
    )
    .expect("resize should apply");

    assert_eq!(project.timeline.tracks[0].items[0].duration_seconds, 6.5);
    assert_eq!(project.timeline.duration_seconds, 6.5);
}

#[test]
fn rejects_zero_duration() {
    let mut project = sample_project();

    let error = apply_timeline_patch(
        &mut project,
        TimelinePatch::ResizeItem {
            item_id: "item-1".to_string(),
            duration_seconds: 0.0,
        },
    )
    .expect_err("zero duration must fail");

    assert_eq!(error, TimelinePatchError::NonPositiveDuration);
}
```

- [ ] **Step 2: Run tests to verify failure**

Run:

```bash
pnpm exec cargo test --manifest-path src-tauri/Cargo.toml --test project_patch
```

Expected: FAIL with unresolved imports for `apply_timeline_patch` and `TimelinePatch`.

- [ ] **Step 3: Implement patch validation**

Replace `src-tauri/src/project/patch.rs` with:

```rust
use super::model::*;
use thiserror::Error;

#[derive(Debug, Clone, PartialEq)]
pub enum TimelinePatch {
    MoveItem {
        item_id: String,
        target_track_id: String,
        start_seconds: f64,
    },
    ResizeItem {
        item_id: String,
        duration_seconds: f64,
    },
}

#[derive(Debug, Error, PartialEq)]
pub enum TimelinePatchError {
    #[error("time values cannot be negative")]
    NegativeTime,
    #[error("duration must be greater than zero")]
    NonPositiveDuration,
    #[error("timeline item was not found: {0}")]
    ItemNotFound(String),
    #[error("timeline track was not found: {0}")]
    TrackNotFound(String),
    #[error("track is locked: {0}")]
    TrackLocked(String),
    #[error("item kind {item_kind:?} is incompatible with track kind {track_kind:?}")]
    TrackTypeMismatch {
        item_kind: TimelineItemKind,
        track_kind: TrackKind,
    },
}

pub fn apply_timeline_patch(
    project: &mut VideoProject,
    patch: TimelinePatch,
) -> Result<(), TimelinePatchError> {
    match patch {
        TimelinePatch::MoveItem {
            item_id,
            target_track_id,
            start_seconds,
        } => move_item(project, &item_id, &target_track_id, start_seconds),
        TimelinePatch::ResizeItem {
            item_id,
            duration_seconds,
        } => resize_item(project, &item_id, duration_seconds),
    }?;
    recalculate_duration(&mut project.timeline);
    Ok(())
}

fn move_item(
    project: &mut VideoProject,
    item_id: &str,
    target_track_id: &str,
    start_seconds: f64,
) -> Result<(), TimelinePatchError> {
    if start_seconds < 0.0 {
        return Err(TimelinePatchError::NegativeTime);
    }

    let (source_track_index, item_index) = find_item(&project.timeline, item_id)
        .ok_or_else(|| TimelinePatchError::ItemNotFound(item_id.to_string()))?;
    let target_track_index = project
        .timeline
        .tracks
        .iter()
        .position(|track| track.id == target_track_id)
        .ok_or_else(|| TimelinePatchError::TrackNotFound(target_track_id.to_string()))?;

    if project.timeline.tracks[target_track_index].locked {
        return Err(TimelinePatchError::TrackLocked(target_track_id.to_string()));
    }

    let mut item = project.timeline.tracks[source_track_index]
        .items
        .remove(item_index);
    let target_kind = project.timeline.tracks[target_track_index].kind.clone();

    if !item_allowed_on_track(&item.kind, &target_kind) {
        let original_track = &mut project.timeline.tracks[source_track_index];
        original_track.items.insert(item_index, item.clone());
        return Err(TimelinePatchError::TrackTypeMismatch {
            item_kind: item.kind,
            track_kind: target_kind,
        });
    }

    item.start_seconds = start_seconds;
    project.timeline.tracks[target_track_index].items.push(item);
    sort_track_items(&mut project.timeline.tracks[target_track_index]);
    Ok(())
}

fn resize_item(
    project: &mut VideoProject,
    item_id: &str,
    duration_seconds: f64,
) -> Result<(), TimelinePatchError> {
    if duration_seconds <= 0.0 {
        return Err(TimelinePatchError::NonPositiveDuration);
    }

    let (track_index, item_index) = find_item(&project.timeline, item_id)
        .ok_or_else(|| TimelinePatchError::ItemNotFound(item_id.to_string()))?;
    if project.timeline.tracks[track_index].locked {
        return Err(TimelinePatchError::TrackLocked(
            project.timeline.tracks[track_index].id.clone(),
        ));
    }

    project.timeline.tracks[track_index].items[item_index].duration_seconds = duration_seconds;
    sort_track_items(&mut project.timeline.tracks[track_index]);
    Ok(())
}

fn find_item(timeline: &Timeline, item_id: &str) -> Option<(usize, usize)> {
    timeline
        .tracks
        .iter()
        .enumerate()
        .find_map(|(track_index, track)| {
            track
                .items
                .iter()
                .position(|item| item.id == item_id)
                .map(|item_index| (track_index, item_index))
        })
}

fn item_allowed_on_track(item_kind: &TimelineItemKind, track_kind: &TrackKind) -> bool {
    matches!(
        (item_kind, track_kind),
        (TimelineItemKind::VideoClip, TrackKind::Video)
            | (TimelineItemKind::HyperframeScene, TrackKind::HyperframeScene)
            | (TimelineItemKind::Overlay, TrackKind::Overlay)
            | (TimelineItemKind::Caption, TrackKind::Caption)
            | (TimelineItemKind::AudioClip, TrackKind::Audio)
    )
}

fn sort_track_items(track: &mut TimelineTrack) {
    track.items.sort_by(|a, b| {
        a.start_seconds
            .partial_cmp(&b.start_seconds)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
}

fn recalculate_duration(timeline: &mut Timeline) {
    timeline.duration_seconds = timeline
        .tracks
        .iter()
        .flat_map(|track| track.items.iter())
        .map(|item| item.start_seconds + item.duration_seconds)
        .fold(0.0, f64::max);
}
```

- [ ] **Step 4: Run Rust tests**

Run:

```bash
pnpm rust:test
```

Expected: PASS with the five `project_patch` tests passing.

- [ ] **Step 5: Commit patch validation**

Run:

```bash
git add src-tauri/src/project/patch.rs src-tauri/tests/project_patch.rs
git commit -m "feat: validate timeline patches"
```

## Task 4: Add shadcn/Tailwind Frontend Foundation

**Files:**
- Create: `components.json`
- Create: `postcss.config.js`
- Create: `tailwind.config.ts`
- Modify: `src/main.tsx`
- Modify: `src/App.tsx`
- Modify: `src/index.css`
- Create: `src/lib/utils.ts`
- Create: `src/components/ui/button.tsx`
- Create: `src/components/ui/card.tsx`
- Create: `src/components/ui/separator.tsx`

- [ ] **Step 1: Add shadcn and Tailwind config**

Create `components.json`:

```json
{
  "$schema": "https://ui.shadcn.com/schema.json",
  "style": "new-york",
  "rsc": false,
  "tsx": true,
  "tailwind": {
    "config": "tailwind.config.ts",
    "css": "src/index.css",
    "baseColor": "neutral",
    "cssVariables": true,
    "prefix": ""
  },
  "aliases": {
    "components": "@/components",
    "utils": "@/lib/utils",
    "ui": "@/components/ui",
    "lib": "@/lib",
    "hooks": "@/hooks"
  }
}
```

Create `postcss.config.js`:

```js
export default {
  plugins: {
    tailwindcss: {},
    autoprefixer: {},
  },
};
```

Create `tailwind.config.ts`:

```ts
import type { Config } from "tailwindcss";

export default {
  darkMode: ["class"],
  content: ["./index.html", "./src/**/*.{ts,tsx}"],
  theme: {
    extend: {
      colors: {
        border: "hsl(var(--border))",
        input: "hsl(var(--input))",
        ring: "hsl(var(--ring))",
        background: "hsl(var(--background))",
        foreground: "hsl(var(--foreground))",
        primary: {
          DEFAULT: "hsl(var(--primary))",
          foreground: "hsl(var(--primary-foreground))",
        },
        secondary: {
          DEFAULT: "hsl(var(--secondary))",
          foreground: "hsl(var(--secondary-foreground))",
        },
        muted: {
          DEFAULT: "hsl(var(--muted))",
          foreground: "hsl(var(--muted-foreground))",
        },
        accent: {
          DEFAULT: "hsl(var(--accent))",
          foreground: "hsl(var(--accent-foreground))",
        },
        card: {
          DEFAULT: "hsl(var(--card))",
          foreground: "hsl(var(--card-foreground))",
        },
      },
      borderRadius: {
        lg: "var(--radius)",
        md: "calc(var(--radius) - 2px)",
        sm: "calc(var(--radius) - 4px)",
      },
    },
  },
  plugins: [],
} satisfies Config;
```

- [ ] **Step 2: Add utility and shadcn components**

Create `src/lib/utils.ts`:

```ts
import { clsx, type ClassValue } from "clsx";
import { twMerge } from "tailwind-merge";

export function cn(...inputs: ClassValue[]) {
  return twMerge(clsx(inputs));
}
```

Create `src/components/ui/button.tsx`:

```tsx
import * as React from "react";
import { Slot } from "@radix-ui/react-slot";
import { cva, type VariantProps } from "class-variance-authority";
import { cn } from "@/lib/utils";

const buttonVariants = cva(
  "inline-flex items-center justify-center gap-2 whitespace-nowrap rounded-md text-sm font-medium transition-colors focus-visible:outline-none focus-visible:ring-1 focus-visible:ring-ring disabled:pointer-events-none disabled:opacity-50",
  {
    variants: {
      variant: {
        default: "bg-primary text-primary-foreground shadow hover:bg-primary/90",
        secondary: "bg-secondary text-secondary-foreground shadow-sm hover:bg-secondary/80",
        outline: "border border-input bg-background shadow-sm hover:bg-accent hover:text-accent-foreground",
        ghost: "hover:bg-accent hover:text-accent-foreground",
      },
      size: {
        default: "h-9 px-4 py-2",
        sm: "h-8 rounded-md px-3 text-xs",
        icon: "h-9 w-9",
      },
    },
    defaultVariants: {
      variant: "default",
      size: "default",
    },
  },
);

export interface ButtonProps
  extends React.ButtonHTMLAttributes<HTMLButtonElement>,
    VariantProps<typeof buttonVariants> {
  asChild?: boolean;
}

const Button = React.forwardRef<HTMLButtonElement, ButtonProps>(
  ({ className, variant, size, asChild = false, ...props }, ref) => {
    const Comp = asChild ? Slot : "button";
    return (
      <Comp
        className={cn(buttonVariants({ variant, size, className }))}
        ref={ref}
        {...props}
      />
    );
  },
);
Button.displayName = "Button";

export { Button, buttonVariants };
```

Create `src/components/ui/card.tsx`:

```tsx
import * as React from "react";
import { cn } from "@/lib/utils";

const Card = React.forwardRef<HTMLDivElement, React.HTMLAttributes<HTMLDivElement>>(
  ({ className, ...props }, ref) => (
    <div
      ref={ref}
      className={cn("rounded-lg border bg-card text-card-foreground shadow-sm", className)}
      {...props}
    />
  ),
);
Card.displayName = "Card";

const CardHeader = React.forwardRef<HTMLDivElement, React.HTMLAttributes<HTMLDivElement>>(
  ({ className, ...props }, ref) => (
    <div ref={ref} className={cn("flex flex-col space-y-1.5 p-4", className)} {...props} />
  ),
);
CardHeader.displayName = "CardHeader";

const CardTitle = React.forwardRef<HTMLHeadingElement, React.HTMLAttributes<HTMLHeadingElement>>(
  ({ className, ...props }, ref) => (
    <h3 ref={ref} className={cn("text-sm font-semibold leading-none tracking-normal", className)} {...props} />
  ),
);
CardTitle.displayName = "CardTitle";

const CardContent = React.forwardRef<HTMLDivElement, React.HTMLAttributes<HTMLDivElement>>(
  ({ className, ...props }, ref) => (
    <div ref={ref} className={cn("p-4 pt-0", className)} {...props} />
  ),
);
CardContent.displayName = "CardContent";

export { Card, CardContent, CardHeader, CardTitle };
```

Create `src/components/ui/separator.tsx`:

```tsx
import * as React from "react";
import { cn } from "@/lib/utils";

type SeparatorProps = React.HTMLAttributes<HTMLDivElement> & {
  orientation?: "horizontal" | "vertical";
};

export function Separator({
  className,
  orientation = "horizontal",
  ...props
}: SeparatorProps) {
  return (
    <div
      role="separator"
      aria-orientation={orientation}
      className={cn(
        "shrink-0 bg-border",
        orientation === "horizontal" ? "h-px w-full" : "h-full w-px",
        className,
      )}
      {...props}
    />
  );
}
```

- [ ] **Step 3: Replace React entry and CSS placeholders**

Replace `src/main.tsx`:

```tsx
import React from "react";
import ReactDOM from "react-dom/client";
import App from "./App";
import "./index.css";

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>
    <App />
  </React.StrictMode>,
);
```

Replace `src/App.tsx`:

```tsx
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";

export default function App() {
  return (
    <main className="flex min-h-screen items-center justify-center p-6">
      <Card className="w-full max-w-md">
        <CardHeader>
          <CardTitle>Video Creater</CardTitle>
        </CardHeader>
        <CardContent className="space-y-4">
          <p className="text-sm text-muted-foreground">
            Local agent-assisted editor foundation.
          </p>
          <Button disabled>Workspace coming next</Button>
        </CardContent>
      </Card>
    </main>
  );
}
```

Replace `src/index.css`:

```css
@tailwind base;
@tailwind components;
@tailwind utilities;

@layer base {
  :root {
    --background: 40 18% 96%;
    --foreground: 0 0% 10%;
    --card: 0 0% 100%;
    --card-foreground: 0 0% 10%;
    --primary: 0 0% 10%;
    --primary-foreground: 0 0% 100%;
    --secondary: 35 12% 90%;
    --secondary-foreground: 0 0% 10%;
    --muted: 35 12% 92%;
    --muted-foreground: 0 0% 42%;
    --accent: 216 83% 53%;
    --accent-foreground: 0 0% 100%;
    --border: 35 10% 82%;
    --input: 35 10% 82%;
    --ring: 216 83% 53%;
    --radius: 0.5rem;
  }

  * {
    @apply border-border;
  }

  html,
  body,
  #root {
    height: 100%;
  }

  body {
    @apply bg-background text-foreground;
    margin: 0;
    font-family:
      Inter, ui-sans-serif, system-ui, -apple-system, BlinkMacSystemFont, "Segoe UI",
      sans-serif;
  }
}

.timeline-grid {
  background-image:
    linear-gradient(to right, hsl(var(--border)) 1px, transparent 1px),
    linear-gradient(to bottom, hsl(var(--border)) 1px, transparent 1px);
  background-size: 80px 100%, 100% 44px;
}
```

- [ ] **Step 4: Refresh dependency lockfile**

Run:

```bash
pnpm install
```

Expected: `pnpm-lock.yaml` is current for the dependencies declared in `package.json`.

- [ ] **Step 5: Run frontend compile**

Run:

```bash
pnpm lint
pnpm build
```

Expected: PASS. Task 6 replaces the temporary app body with the real editor workspace.

- [ ] **Step 6: Commit frontend foundation**

Run:

```bash
git add package.json pnpm-lock.yaml components.json postcss.config.js tailwind.config.ts src/main.tsx src/App.tsx src/index.css src/lib/utils.ts src/components/ui
git commit -m "feat: add React shadcn foundation"
```

## Task 5: Add Timeline Types And Adapter Tests

**Files:**
- Create: `src/lib/timeline.ts`
- Create: `src/lib/timeline.test.ts`

- [ ] **Step 1: Write failing TypeScript tests**

Create `src/lib/timeline.test.ts`:

```ts
import { describe, expect, it } from "vitest";
import {
  buildTimelineRows,
  createMovePatchFromDrag,
  sampleTimeline,
} from "./timeline";

describe("timeline adapter", () => {
  it("maps project tracks to timeline rows", () => {
    const rows = buildTimelineRows(sampleTimeline);

    expect(rows).toEqual([
      { id: "track-video", title: "Video" },
      { id: "track-scenes", title: "HyperFrames" },
      { id: "track-overlays", title: "Overlays" },
      { id: "track-captions", title: "Captions" },
      { id: "track-audio", title: "Audio" },
    ]);
  });

  it("creates a typed move patch from a drag result", () => {
    const patch = createMovePatchFromDrag({
      itemId: "item-1",
      targetTrackId: "track-video",
      startSeconds: 2.25,
    });

    expect(patch).toEqual({
      type: "moveItem",
      itemId: "item-1",
      targetTrackId: "track-video",
      startSeconds: 2.25,
    });
  });
});
```

- [ ] **Step 2: Run test to verify failure**

Run:

```bash
pnpm test src/lib/timeline.test.ts
```

Expected: FAIL with module `./timeline` not found.

- [ ] **Step 3: Implement timeline adapter**

Create `src/lib/timeline.ts`:

```ts
export type TrackKind = "video" | "hyperframe_scene" | "overlay" | "caption" | "audio";

export type TimelineItemKind =
  | "video_clip"
  | "hyperframe_scene"
  | "overlay"
  | "caption"
  | "audio_clip";

export type TimelineSource =
  | { type: "media"; mediaId: string }
  | { type: "generated"; artifactId: string }
  | { type: "text"; text: string };

export interface TimelineItem {
  id: string;
  kind: TimelineItemKind;
  startSeconds: number;
  durationSeconds: number;
  source: TimelineSource;
  label: string;
  properties: Record<string, unknown>;
}

export interface TimelineTrack {
  id: string;
  name: string;
  kind: TrackKind;
  locked: boolean;
  items: TimelineItem[];
}

export interface Timeline {
  durationSeconds: number;
  tracks: TimelineTrack[];
}

export interface TimelineRow {
  id: string;
  title: string;
}

export type TimelinePatch =
  | {
      type: "moveItem";
      itemId: string;
      targetTrackId: string;
      startSeconds: number;
    }
  | {
      type: "resizeItem";
      itemId: string;
      durationSeconds: number;
    };

export const sampleTimeline: Timeline = {
  durationSeconds: 4,
  tracks: [
    {
      id: "track-video",
      name: "Video",
      kind: "video",
      locked: false,
      items: [
        {
          id: "item-1",
          kind: "video_clip",
          startSeconds: 0,
          durationSeconds: 4,
          source: { type: "media", mediaId: "media-1" },
          label: "Opening clip",
          properties: {},
        },
      ],
    },
    { id: "track-scenes", name: "HyperFrames", kind: "hyperframe_scene", locked: false, items: [] },
    { id: "track-overlays", name: "Overlays", kind: "overlay", locked: false, items: [] },
    { id: "track-captions", name: "Captions", kind: "caption", locked: false, items: [] },
    { id: "track-audio", name: "Audio", kind: "audio", locked: false, items: [] },
  ],
};

export function buildTimelineRows(timeline: Timeline): TimelineRow[] {
  return timeline.tracks.map((track) => ({
    id: track.id,
    title: track.name,
  }));
}

export function createMovePatchFromDrag(input: {
  itemId: string;
  targetTrackId: string;
  startSeconds: number;
}): TimelinePatch {
  return {
    type: "moveItem",
    itemId: input.itemId,
    targetTrackId: input.targetTrackId,
    startSeconds: input.startSeconds,
  };
}
```

- [ ] **Step 4: Run tests**

Run:

```bash
pnpm test src/lib/timeline.test.ts
```

Expected: PASS with two tests.

- [ ] **Step 5: Commit timeline adapter**

Run:

```bash
git add src/lib/timeline.ts src/lib/timeline.test.ts
git commit -m "feat: add timeline adapter types"
```

## Task 6: Add Editor Workspace Components

**Files:**
- Create: `src/components/workspace/editor-workspace.tsx`
- Create: `src/components/workspace/timeline-editor.tsx`
- Create: `src/components/workspace/agent-panel.tsx`
- Create: `src/components/workspace/media-bin.tsx`
- Create: `src/components/workspace/preview-panel.tsx`

- [ ] **Step 1: Add static workspace panels**

Create `src/components/workspace/media-bin.tsx`:

```tsx
import { Film } from "lucide-react";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";

export function MediaBin() {
  return (
    <Card className="h-full rounded-md">
      <CardHeader>
        <CardTitle className="flex items-center gap-2">
          <Film className="h-4 w-4" />
          Media
        </CardTitle>
      </CardHeader>
      <CardContent className="space-y-2 text-sm text-muted-foreground">
        <div className="rounded-md border bg-muted/40 p-3">input.mp4</div>
        <div className="rounded-md border bg-muted/40 p-3">voiceover.wav</div>
      </CardContent>
    </Card>
  );
}
```

Create `src/components/workspace/preview-panel.tsx`:

```tsx
import { Play } from "lucide-react";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";

export function PreviewPanel() {
  return (
    <Card className="h-full rounded-md">
      <CardHeader>
        <CardTitle>Preview</CardTitle>
      </CardHeader>
      <CardContent>
        <div className="flex aspect-video items-center justify-center rounded-md border bg-neutral-950 text-white">
          <Button variant="secondary" size="icon" aria-label="Play preview">
            <Play className="h-4 w-4" />
          </Button>
        </div>
      </CardContent>
    </Card>
  );
}
```

Create `src/components/workspace/agent-panel.tsx`:

```tsx
import { Sparkles } from "lucide-react";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";

export function AgentPanel() {
  return (
    <Card className="h-full rounded-md">
      <CardHeader>
        <CardTitle className="flex items-center gap-2">
          <Sparkles className="h-4 w-4" />
          Codex
        </CardTitle>
      </CardHeader>
      <CardContent className="space-y-3 text-sm">
        <p className="text-muted-foreground">
          Foundation panel for rough cuts, timeline patches, and render diagnostics.
        </p>
        <Button className="w-full" disabled>
          Generate rough cut
        </Button>
      </CardContent>
    </Card>
  );
}
```

- [ ] **Step 2: Add timeline editor**

Create `src/components/workspace/timeline-editor.tsx`:

```tsx
import { useMemo } from "react";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { buildTimelineRows, sampleTimeline } from "@/lib/timeline";

export function TimelineEditor() {
  const rows = useMemo(() => buildTimelineRows(sampleTimeline), []);
  const items = sampleTimeline.tracks.flatMap((track) =>
    track.items.map((item) => ({
      ...item,
      trackId: track.id,
    })),
  );

  return (
    <Card className="rounded-md">
      <CardHeader className="pb-3">
        <CardTitle>Timeline</CardTitle>
      </CardHeader>
      <CardContent>
        <div className="overflow-hidden rounded-md border">
          <div className="grid grid-cols-[180px_1fr] bg-muted text-xs font-medium text-muted-foreground">
            <div className="border-r p-2">Tracks</div>
            <div className="p-2">00:00 - 00:12</div>
          </div>
          <div className="grid grid-cols-[180px_1fr]">
            <div>
              {rows.map((row) => (
                <div key={row.id} className="h-11 border-b border-r px-3 py-3 text-xs">
                  {row.title}
                </div>
              ))}
            </div>
            <div className="timeline-grid relative">
              {rows.map((row) => (
                <div key={row.id} className="h-11 border-b" />
              ))}
              {items.map((item) => (
                <div
                  key={item.id}
                  className="absolute top-1 h-9 rounded-md border border-blue-300 bg-blue-50 px-3 py-2 text-xs text-blue-950"
                  style={{
                    left: `${item.startSeconds * 80}px`,
                    width: `${item.durationSeconds * 80}px`,
                  }}
                >
                  {item.label}
                </div>
              ))}
            </div>
          </div>
        </div>
      </CardContent>
    </Card>
  );
}
```

- [ ] **Step 3: Add workspace composition**

Create `src/components/workspace/editor-workspace.tsx`:

```tsx
import { Button } from "@/components/ui/button";
import { Separator } from "@/components/ui/separator";
import { AgentPanel } from "./agent-panel";
import { MediaBin } from "./media-bin";
import { PreviewPanel } from "./preview-panel";
import { TimelineEditor } from "./timeline-editor";

export function EditorWorkspace() {
  return (
    <main className="flex h-full min-h-0 flex-col">
      <header className="flex h-14 shrink-0 items-center justify-between border-b bg-card px-4">
        <div>
          <h1 className="text-sm font-semibold tracking-normal">Video Creater</h1>
          <p className="text-xs text-muted-foreground">Local agent-assisted editor</p>
        </div>
        <div className="flex items-center gap-2">
          <Button variant="outline" size="sm">
            Import
          </Button>
          <Button size="sm">Render draft</Button>
        </div>
      </header>
      <section className="grid min-h-0 flex-1 grid-cols-[260px_1fr_300px] gap-3 p-3">
        <MediaBin />
        <div className="flex min-h-0 flex-col gap-3">
          <PreviewPanel />
          <Separator />
          <TimelineEditor />
        </div>
        <AgentPanel />
      </section>
    </main>
  );
}
```

- [ ] **Step 4: Run frontend checks**

Run:

```bash
pnpm lint
pnpm test
```

Expected: both PASS.

- [ ] **Step 5: Commit workspace UI**

Run:

```bash
git add src/components/workspace src/App.tsx
git commit -m "feat: add editor workspace shell"
```

## Task 7: Add Tauri Project Storage Commands

**Files:**
- Modify: `src-tauri/src/project/storage.rs`
- Modify: `src-tauri/src/main.rs`

- [ ] **Step 1: Implement project storage**

Replace `src-tauri/src/project/storage.rs` with:

```rust
use super::model::VideoProject;
use std::fs;
use std::path::{Path, PathBuf};
use thiserror::Error;

pub const PROJECT_FILE_NAME: &str = "video-creater.project.json";

#[derive(Debug, Error)]
pub enum ProjectStorageError {
    #[error("failed to create project directory {path}: {source}")]
    CreateDir {
        path: String,
        source: std::io::Error,
    },
    #[error("failed to serialize project: {0}")]
    Serialize(serde_json::Error),
    #[error("failed to write project file {path}: {source}")]
    Write {
        path: String,
        source: std::io::Error,
    },
    #[error("failed to read project file {path}: {source}")]
    Read {
        path: String,
        source: std::io::Error,
    },
    #[error("failed to parse project file {path}: {source}")]
    Parse {
        path: String,
        source: serde_json::Error,
    },
}

pub fn project_file_path(project_dir: &Path) -> PathBuf {
    project_dir.join(PROJECT_FILE_NAME)
}

pub fn save_project(project_dir: &Path, project: &VideoProject) -> Result<PathBuf, ProjectStorageError> {
    fs::create_dir_all(project_dir).map_err(|source| ProjectStorageError::CreateDir {
        path: project_dir.display().to_string(),
        source,
    })?;

    for child in ["media", "transcripts", "generated/hyperframes", "generated/previews", "renders", "logs"] {
        fs::create_dir_all(project_dir.join(child)).map_err(|source| ProjectStorageError::CreateDir {
            path: project_dir.join(child).display().to_string(),
            source,
        })?;
    }

    let json = serde_json::to_string_pretty(project).map_err(ProjectStorageError::Serialize)?;
    let path = project_file_path(project_dir);
    fs::write(&path, json).map_err(|source| ProjectStorageError::Write {
        path: path.display().to_string(),
        source,
    })?;
    Ok(path)
}

pub fn load_project(project_dir: &Path) -> Result<VideoProject, ProjectStorageError> {
    let path = project_file_path(project_dir);
    let json = fs::read_to_string(&path).map_err(|source| ProjectStorageError::Read {
        path: path.display().to_string(),
        source,
    })?;
    serde_json::from_str(&json).map_err(|source| ProjectStorageError::Parse {
        path: path.display().to_string(),
        source,
    })
}
```

- [ ] **Step 2: Add Tauri commands**

Replace `src-tauri/src/main.rs` with:

```rust
mod project;

use project::model::VideoProject;
use project::storage;
use std::path::PathBuf;
use tauri::Manager;

#[tauri::command]
fn create_empty_project(name: String) -> VideoProject {
    let now = chrono::Utc::now().to_rfc3339();
    VideoProject::new_empty(uuid::Uuid::new_v4().to_string(), name, now)
}

#[tauri::command]
fn save_project_to_folder(project_dir: String, project: VideoProject) -> Result<String, String> {
    storage::save_project(&PathBuf::from(project_dir), &project)
        .map(|path| path.display().to_string())
        .map_err(|error| error.to_string())
}

#[tauri::command]
fn load_project_from_folder(project_dir: String) -> Result<VideoProject, String> {
    storage::load_project(&PathBuf::from(project_dir)).map_err(|error| error.to_string())
}

pub fn run() {
    tauri::Builder::default()
        .setup(|app| {
            app.manage("video-creater");
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            create_empty_project,
            save_project_to_folder,
            load_project_from_folder
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

fn main() {
    run();
}
```

- [ ] **Step 3: Add storage integration test**

Append to `src-tauri/tests/project_patch.rs`:

```rust
use video_creater_lib::project::storage::{load_project, save_project, PROJECT_FILE_NAME};

#[test]
fn saves_and_loads_project_file() {
    let dir = tempfile::tempdir().expect("temp dir");
    let project = sample_project();

    let saved_path = save_project(dir.path(), &project).expect("save project");

    assert_eq!(saved_path.file_name().unwrap(), PROJECT_FILE_NAME);
    assert!(dir.path().join("media").exists());
    assert!(dir.path().join("generated/hyperframes").exists());

    let loaded = load_project(dir.path()).expect("load project");
    assert_eq!(loaded, project);
}
```

- [ ] **Step 4: Run Rust tests**

Run:

```bash
pnpm rust:test
```

Expected: PASS.

- [ ] **Step 5: Commit storage commands**

Run:

```bash
git add src-tauri/src/main.rs src-tauri/src/project/storage.rs src-tauri/tests/project_patch.rs
git commit -m "feat: add project storage commands"
```

## Task 8: Verify Full Foundation

**Files:**
- Modify only if verification exposes a compile issue in files from earlier tasks.

- [ ] **Step 1: Run full verification**

Run:

```bash
pnpm verify
```

Expected: PASS for TypeScript compile, Vitest tests, and Rust tests.

- [ ] **Step 2: Run Tauri dev smoke**

Run:

```bash
pnpm tauri:dev
```

Expected: The Tauri window opens to the editor workspace with media bin, preview, timeline, and Codex panel. Stop the dev server after confirming the UI renders.

- [ ] **Step 3: Inspect git state**

Run:

```bash
git status --short
```

Expected: no unstaged files except build artifacts already ignored by `.gitignore`.

- [ ] **Step 4: Commit any verification-only fixes**

If Step 1 or Step 2 required a code correction, commit only those changed files:

```bash
git add <changed-files>
git commit -m "fix: stabilize foundation verification"
```

If no corrections were required, do not create an empty commit.

## Self-Review Notes

Spec coverage in this foundation plan:

- Tauri shell: Task 1 and Task 8.
- React TypeScript frontend: Tasks 1, 4, and 6.
- shadcn/ui-only visible controls: Task 4 and Task 6.
- Rust-owned canonical model: Task 2.
- Timeline validation and patches: Task 3.
- dnd-timeline interaction binding preparation: Task 5 and Task 6.
- Project file-backed storage: Task 7.

Spec requirements intentionally left for follow-up plans:

- Native ffmpeg render pipeline.
- ffprobe media import metadata.
- Codex app-server lifecycle and structured proposal flow.
- HyperFrames Node sidecar.
- Transcription engine.
- End-to-end render queue and final `mp4` smoke fixture.
- Full dnd-timeline drag/resize wiring.
