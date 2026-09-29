# Agent-Editable Project Files First Slice Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build the first usable split-project storage foundation: schema-v2 manifest files, split load/save, validation reports, migration from the current single-file project, and Tauri commands that expose those operations.

**Architecture:** Keep the existing `VideoProject` runtime model and add a new `project::split` module beside `project::storage`. `video-creater.project.json` becomes a schema-v2 manifest for split projects; `timeline.json`, `media/index.json`, and `transcripts/<media-id>.json` hold the first extracted runtime data. Validation returns field-specific issues and rejects path traversal before file reads or writes.

**Tech Stack:** Rust, serde JSON, tempfile, existing Tauri command bridge, existing `VideoProject` model, cargo tests.

---

## Scope Check

The approved spec covers several subsystems: split storage, migration, project actions, UI mutation routing, transcript/template/generated asset editing, render reports, and MCP wrappers. This plan intentionally implements only the first testable foundation:

- Split manifest structs and path-safe file resolution.
- Split load/save for the existing runtime fields: project metadata, render settings, Codex thread id, timeline, media, transcripts, and jobs.
- Validation reports for manifest paths, duplicate IDs, timeline references, source ranges, finite timing, and transcript monotonicity.
- Schema-v1 single-file to schema-v2 split migration.
- Tauri commands and TypeScript wrappers for load/save/validate/migrate.

Follow-up plans should cover:

- `ProjectAction` add/insert/move/trim/split/remove/template/transcript/render actions.
- UI routing from local-only mutation to Rust actions.
- MCP/app-server wrappers for the same action surface.
- Generated asset provenance and render report files once those runtime fields exist.

## File Structure

- Create `src-tauri/src/project/split.rs`: split manifest/data structs, path resolution, split load/save, validation, migration.
- Modify `src-tauri/src/project/mod.rs`: export the new `split` module.
- Create `src-tauri/tests/project_split.rs`: focused Rust tests for split project storage and validation.
- Modify `src-tauri/src/main.rs`: add Tauri commands for split load/save/validate/migrate and register them.
- Modify `src/lib/project.ts`: add TypeScript wrappers and types for split project commands.
- Create `src/lib/project-split.test.ts`: verify TypeScript wrappers call the expected command names and payloads.

## Data Contract For This Slice

`video-creater.project.json` schema v2:

```json
{
  "schemaVersion": 2,
  "id": "project-test",
  "name": "Test Project",
  "createdAt": "2026-06-11T00:00:00Z",
  "updatedAt": "2026-06-11T00:00:00Z",
  "layout": "split",
  "files": {
    "timeline": "timeline.json",
    "media": "media/index.json",
    "transcripts": "transcripts",
    "templates": "templates",
    "generated": "generated",
    "renders": "renders",
    "logs": "logs"
  },
  "renderSettings": {
    "width": 1920,
    "height": 1080,
    "fps": 24.0,
    "loudnessLufs": -14.0,
    "captions": "burn_in"
  },
  "codexThreadId": null,
  "jobs": []
}
```

`timeline.json`:

```json
{
  "schemaVersion": 1,
  "durationSeconds": 4.0,
  "tracks": []
}
```

`media/index.json`:

```json
{
  "schemaVersion": 1,
  "folders": [],
  "assets": []
}
```

`transcripts/<media-id>.json` uses the existing `Transcript` shape plus optional repair metadata ignored by the runtime in this first slice:

```json
{
  "schemaVersion": 1,
  "id": "transcript-media-1",
  "mediaId": "media-1",
  "engine": "nvidia/parakeet-tdt-0.6b-v3",
  "rawArtifactPath": null,
  "repairs": [],
  "segments": [],
  "words": []
}
```

---

### Task 1: Split Project Manifest And Path Safety

**Files:**
- Create: `src-tauri/src/project/split.rs`
- Modify: `src-tauri/src/project/mod.rs`
- Test: `src-tauri/tests/project_split.rs`

- [ ] **Step 1: Write failing manifest/path tests**

Create `src-tauri/tests/project_split.rs` with:

```rust
use std::fs;

use video_creater_lib::project::fixtures::sample_project;
use video_creater_lib::project::split::{
    default_split_manifest_for_project, resolve_project_relative_path, SplitProjectError,
    SplitProjectLayout,
};

#[test]
fn split_manifest_defaults_to_agent_editable_file_layout() {
    let project = sample_project();

    let manifest = default_split_manifest_for_project(&project);

    assert_eq!(manifest.schema_version, 2);
    assert_eq!(manifest.id, "project-test");
    assert_eq!(manifest.layout, SplitProjectLayout::Split);
    assert_eq!(manifest.files.timeline, "timeline.json");
    assert_eq!(manifest.files.media, "media/index.json");
    assert_eq!(manifest.files.transcripts, "transcripts");
    assert_eq!(manifest.files.templates, "templates");
    assert_eq!(manifest.files.generated, "generated");
    assert_eq!(manifest.files.renders, "renders");
    assert_eq!(manifest.files.logs, "logs");
    assert_eq!(manifest.render_settings, project.render_settings);
    assert_eq!(manifest.codex_thread_id, None);
    assert!(manifest.jobs.is_empty());
}

#[test]
fn split_path_resolution_rejects_absolute_and_parent_paths() {
    let dir = tempfile::tempdir().expect("project dir");

    let absolute = resolve_project_relative_path(dir.path(), "/tmp/outside.json")
        .expect_err("absolute path must be rejected");
    assert_eq!(
        absolute,
        SplitProjectError::UnsafeManifestPath {
            field: "path".to_string(),
            value: "/tmp/outside.json".to_string()
        }
    );

    let parent = resolve_project_relative_path(dir.path(), "../outside.json")
        .expect_err("parent path must be rejected");
    assert_eq!(
        parent,
        SplitProjectError::UnsafeManifestPath {
            field: "path".to_string(),
            value: "../outside.json".to_string()
        }
    );

    let resolved = resolve_project_relative_path(dir.path(), "timeline.json")
        .expect("relative path inside project");
    assert_eq!(resolved, dir.path().join("timeline.json"));
}

#[test]
fn split_manifest_serializes_with_camel_case_contract() {
    let project = sample_project();
    let manifest = default_split_manifest_for_project(&project);

    let json = serde_json::to_value(&manifest).expect("manifest json");

    assert_eq!(json["schemaVersion"], serde_json::json!(2));
    assert_eq!(json["layout"], serde_json::json!("split"));
    assert_eq!(json["codexThreadId"], serde_json::Value::Null);
    assert_eq!(json["files"]["timeline"], serde_json::json!("timeline.json"));
}

#[test]
fn split_module_does_not_create_files_during_manifest_build() {
    let dir = tempfile::tempdir().expect("project dir");
    let project = sample_project();

    let _manifest = default_split_manifest_for_project(&project);

    let entries = fs::read_dir(dir.path())
        .expect("read temp dir")
        .collect::<Result<Vec<_>, _>>()
        .expect("dir entries");
    assert!(entries.is_empty());
}
```

- [ ] **Step 2: Run tests to verify red**

Run:

```bash
rtk cargo test --manifest-path src-tauri/Cargo.toml --test project_split -- --nocapture
```

Expected: FAIL because `project::split` does not exist.

- [ ] **Step 3: Add module export**

Modify `src-tauri/src/project/mod.rs` to include:

```rust
pub mod fixtures;
pub mod import;
pub mod model;
pub mod patch;
pub mod split;
pub mod storage;
```

- [ ] **Step 4: Create split manifest module**

Create `src-tauri/src/project/split.rs` with:

```rust
use super::model::{JobSummary, RenderSettings, Timeline, Transcript, VideoProject};
use serde::{Deserialize, Serialize};
use std::path::{Component, Path, PathBuf};
use thiserror::Error;

pub const SPLIT_PROJECT_SCHEMA_VERSION: u32 = 2;
pub const SPLIT_TIMELINE_SCHEMA_VERSION: u32 = 1;
pub const SPLIT_MEDIA_SCHEMA_VERSION: u32 = 1;
pub const SPLIT_TRANSCRIPT_SCHEMA_VERSION: u32 = 1;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SplitProjectLayout {
    Split,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct SplitProjectManifest {
    pub schema_version: u32,
    pub id: String,
    pub name: String,
    pub created_at: String,
    pub updated_at: String,
    pub layout: SplitProjectLayout,
    pub files: SplitProjectFiles,
    pub render_settings: RenderSettings,
    pub codex_thread_id: Option<String>,
    #[serde(default)]
    pub jobs: Vec<JobSummary>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SplitProjectFiles {
    pub timeline: String,
    pub media: String,
    pub transcripts: String,
    pub templates: String,
    pub generated: String,
    pub renders: String,
    pub logs: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct SplitTimelineFile {
    pub schema_version: u32,
    pub duration_seconds: f64,
    pub tracks: Vec<super::model::TimelineTrack>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct SplitMediaIndexFile {
    pub schema_version: u32,
    #[serde(default)]
    pub folders: Vec<SplitMediaFolder>,
    #[serde(default)]
    pub assets: Vec<super::model::MediaAsset>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SplitMediaFolder {
    pub id: String,
    pub name: String,
    pub parent_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct SplitTranscriptFile {
    pub schema_version: u32,
    pub id: String,
    pub media_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub engine: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub raw_artifact_path: Option<String>,
    #[serde(default)]
    pub repairs: Vec<serde_json::Value>,
    #[serde(default)]
    pub segments: Vec<super::model::TranscriptSegment>,
    #[serde(default)]
    pub words: Vec<super::model::TranscriptWord>,
}

#[derive(Debug, Error, PartialEq)]
pub enum SplitProjectError {
    #[error("manifest path {field} is unsafe: {value}")]
    UnsafeManifestPath { field: String, value: String },
    #[error("split project io error at {path}: {message}")]
    Io { path: String, message: String },
    #[error("split project json error at {path}: {message}")]
    Json { path: String, message: String },
    #[error("split project validation failed")]
    Validation { issues: Vec<ProjectValidationIssue> },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ProjectValidationIssue {
    pub path: String,
    pub message: String,
    pub fix: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ProjectValidationReport {
    pub ok: bool,
    pub issues: Vec<ProjectValidationIssue>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ProjectWriteReport {
    pub manifest_path: String,
    pub written_files: Vec<String>,
    pub removed_files: Vec<String>,
}

pub fn default_split_manifest_for_project(project: &VideoProject) -> SplitProjectManifest {
    SplitProjectManifest {
        schema_version: SPLIT_PROJECT_SCHEMA_VERSION,
        id: project.id.clone(),
        name: project.name.clone(),
        created_at: project.created_at.clone(),
        updated_at: project.updated_at.clone(),
        layout: SplitProjectLayout::Split,
        files: SplitProjectFiles {
            timeline: "timeline.json".to_string(),
            media: "media/index.json".to_string(),
            transcripts: "transcripts".to_string(),
            templates: "templates".to_string(),
            generated: "generated".to_string(),
            renders: "renders".to_string(),
            logs: "logs".to_string(),
        },
        render_settings: project.render_settings.clone(),
        codex_thread_id: project.codex_thread_id.clone(),
        jobs: project.jobs.clone(),
    }
}

pub fn split_timeline_from_project(project: &VideoProject) -> SplitTimelineFile {
    SplitTimelineFile {
        schema_version: SPLIT_TIMELINE_SCHEMA_VERSION,
        duration_seconds: project.timeline.duration_seconds,
        tracks: project.timeline.tracks.clone(),
    }
}

pub fn split_media_index_from_project(project: &VideoProject) -> SplitMediaIndexFile {
    SplitMediaIndexFile {
        schema_version: SPLIT_MEDIA_SCHEMA_VERSION,
        folders: Vec::new(),
        assets: project.media.clone(),
    }
}

pub fn split_transcript_from_runtime(transcript: &Transcript) -> SplitTranscriptFile {
    SplitTranscriptFile {
        schema_version: SPLIT_TRANSCRIPT_SCHEMA_VERSION,
        id: transcript.id.clone(),
        media_id: transcript.media_id.clone(),
        engine: transcript.engine.clone(),
        raw_artifact_path: transcript.raw_artifact_path.clone(),
        repairs: Vec::new(),
        segments: transcript.segments.clone(),
        words: transcript.words.clone(),
    }
}

pub fn runtime_transcript_from_split(file: SplitTranscriptFile) -> Transcript {
    Transcript {
        id: file.id,
        media_id: file.media_id,
        engine: file.engine,
        raw_artifact_path: file.raw_artifact_path,
        segments: file.segments,
        words: file.words,
    }
}

pub fn runtime_project_from_split_parts(
    manifest: SplitProjectManifest,
    timeline: SplitTimelineFile,
    media: SplitMediaIndexFile,
    transcripts: Vec<SplitTranscriptFile>,
) -> VideoProject {
    VideoProject {
        schema_version: manifest.schema_version,
        id: manifest.id,
        name: manifest.name,
        created_at: manifest.created_at,
        updated_at: manifest.updated_at,
        media: media.assets,
        transcripts: transcripts
            .into_iter()
            .map(runtime_transcript_from_split)
            .collect(),
        timeline: Timeline {
            duration_seconds: timeline.duration_seconds,
            tracks: timeline.tracks,
        },
        render_settings: manifest.render_settings,
        codex_thread_id: manifest.codex_thread_id,
        jobs: manifest.jobs,
    }
}

pub fn resolve_project_relative_path(
    project_dir: &Path,
    relative_path: &str,
) -> Result<PathBuf, SplitProjectError> {
    safe_join(project_dir, "path", relative_path)
}

fn safe_join(
    project_dir: &Path,
    field: &str,
    relative_path: &str,
) -> Result<PathBuf, SplitProjectError> {
    let path = Path::new(relative_path);
    let unsafe_component = path.components().any(|component| {
        matches!(
            component,
            Component::Prefix(_) | Component::RootDir | Component::ParentDir
        )
    });

    if relative_path.trim().is_empty() || unsafe_component {
        return Err(SplitProjectError::UnsafeManifestPath {
            field: field.to_string(),
            value: relative_path.to_string(),
        });
    }

    Ok(project_dir.join(path))
}
```

- [ ] **Step 5: Verify manifest tests pass**

Run:

```bash
rtk cargo test --manifest-path src-tauri/Cargo.toml --test project_split -- --nocapture
```

Expected: PASS for the manifest tests.

- [ ] **Step 6: Commit manifest foundation**

Run:

```bash
rtk git add src-tauri/src/project/mod.rs src-tauri/src/project/split.rs src-tauri/tests/project_split.rs
rtk git commit -m "feat: add split project manifest contract"
```

Expected: commit succeeds.

---

### Task 2: Split Project Save And Load

**Files:**
- Modify: `src-tauri/src/project/split.rs`
- Modify: `src-tauri/tests/project_split.rs`

- [ ] **Step 1: Add failing save/load round-trip tests**

Append to `src-tauri/tests/project_split.rs`:

```rust
use video_creater_lib::project::split::{
    load_split_project, save_split_project, split_project_manifest_path,
};

#[test]
fn save_split_project_writes_manifest_timeline_media_and_transcript_files() {
    let dir = tempfile::tempdir().expect("project dir");
    let mut project = sample_project();
    project.transcripts.push(video_creater_lib::project::model::Transcript {
        id: "transcript-media-1".to_string(),
        media_id: "media-1".to_string(),
        engine: Some("nvidia/parakeet-tdt-0.6b-v3".to_string()),
        raw_artifact_path: None,
        segments: Vec::new(),
        words: vec![video_creater_lib::project::model::TranscriptWord {
            text: "Hello".to_string(),
            start_seconds: 0.0,
            end_seconds: 0.4,
            confidence: Some(0.95),
            speaker: None,
        }],
    });

    let report = save_split_project(dir.path(), &project).expect("save split project");

    assert_eq!(
        report.manifest_path,
        dir.path().join("video-creater.project.json").display().to_string()
    );
    assert!(dir.path().join("video-creater.project.json").exists());
    assert!(dir.path().join("timeline.json").exists());
    assert!(dir.path().join("media/index.json").exists());
    assert!(dir.path().join("transcripts/media-1.json").exists());
    assert!(dir.path().join("templates").is_dir());
    assert!(dir.path().join("generated").is_dir());
    assert!(dir.path().join("renders").is_dir());
    assert!(dir.path().join("logs").is_dir());
    assert!(report
        .written_files
        .iter()
        .any(|path| path.ends_with("timeline.json")));
}

#[test]
fn load_split_project_reconstructs_runtime_video_project() {
    let dir = tempfile::tempdir().expect("project dir");
    let mut project = sample_project();
    project.transcripts.push(video_creater_lib::project::model::Transcript {
        id: "transcript-media-1".to_string(),
        media_id: "media-1".to_string(),
        engine: Some("nvidia/parakeet-tdt-0.6b-v3".to_string()),
        raw_artifact_path: Some("transcripts/media-1.raw.json".to_string()),
        segments: Vec::new(),
        words: vec![video_creater_lib::project::model::TranscriptWord {
            text: "Hello".to_string(),
            start_seconds: 0.0,
            end_seconds: 0.4,
            confidence: Some(0.95),
            speaker: None,
        }],
    });

    save_split_project(dir.path(), &project).expect("save split project");

    let loaded = load_split_project(dir.path()).expect("load split project");

    assert_eq!(loaded.schema_version, 2);
    assert_eq!(loaded.id, project.id);
    assert_eq!(loaded.media, project.media);
    assert_eq!(loaded.timeline, project.timeline);
    assert_eq!(loaded.transcripts, project.transcripts);
    assert_eq!(loaded.render_settings, project.render_settings);
}

#[test]
fn split_project_manifest_path_uses_existing_project_file_name() {
    let dir = tempfile::tempdir().expect("project dir");

    assert_eq!(
        split_project_manifest_path(dir.path()),
        dir.path().join("video-creater.project.json")
    );
}
```

- [ ] **Step 2: Run tests to verify red**

Run:

```bash
rtk cargo test --manifest-path src-tauri/Cargo.toml --test project_split -- --nocapture
```

Expected: FAIL because split save/load functions do not exist.

- [ ] **Step 3: Implement save/load helpers**

Append this code to `src-tauri/src/project/split.rs`:

```rust
pub fn split_project_manifest_path(project_dir: &Path) -> PathBuf {
    project_dir.join(super::storage::PROJECT_FILE_NAME)
}

pub fn save_split_project(
    project_dir: &Path,
    project: &VideoProject,
) -> Result<ProjectWriteReport, SplitProjectError> {
    let manifest = default_split_manifest_for_project(project);
    let timeline = split_timeline_from_project(project);
    let media = split_media_index_from_project(project);
    let transcript_files = project
        .transcripts
        .iter()
        .map(split_transcript_from_runtime)
        .collect::<Vec<_>>();

    create_split_dirs(project_dir, &manifest)?;

    let manifest_path = split_project_manifest_path(project_dir);
    let timeline_path = safe_join(project_dir, "files.timeline", &manifest.files.timeline)?;
    let media_path = safe_join(project_dir, "files.media", &manifest.files.media)?;
    let transcripts_dir = safe_join(project_dir, "files.transcripts", &manifest.files.transcripts)?;

    let mut written_files = Vec::new();
    write_json_file(&manifest_path, &manifest)?;
    written_files.push(manifest_path.display().to_string());
    write_json_file(&timeline_path, &timeline)?;
    written_files.push(timeline_path.display().to_string());
    write_json_file(&media_path, &media)?;
    written_files.push(media_path.display().to_string());

    for transcript in transcript_files {
        let path = transcripts_dir.join(format!("{}.json", transcript.media_id));
        write_json_file(&path, &transcript)?;
        written_files.push(path.display().to_string());
    }

    Ok(ProjectWriteReport {
        manifest_path: split_project_manifest_path(project_dir).display().to_string(),
        written_files,
        removed_files: Vec::new(),
    })
}

pub fn load_split_project(project_dir: &Path) -> Result<VideoProject, SplitProjectError> {
    let manifest_path = split_project_manifest_path(project_dir);
    let manifest: SplitProjectManifest = read_json_file(&manifest_path)?;
    let timeline_path = safe_join(project_dir, "files.timeline", &manifest.files.timeline)?;
    let media_path = safe_join(project_dir, "files.media", &manifest.files.media)?;
    let transcripts_dir = safe_join(project_dir, "files.transcripts", &manifest.files.transcripts)?;

    let timeline: SplitTimelineFile = read_json_file(&timeline_path)?;
    let media: SplitMediaIndexFile = read_json_file(&media_path)?;
    let mut transcripts = Vec::new();
    if transcripts_dir.exists() {
        let mut transcript_paths = std::fs::read_dir(&transcripts_dir)
            .map_err(|error| SplitProjectError::Io {
                path: transcripts_dir.display().to_string(),
                message: error.to_string(),
            })?
            .map(|entry| entry.map(|entry| entry.path()))
            .collect::<Result<Vec<_>, _>>()
            .map_err(|error| SplitProjectError::Io {
                path: transcripts_dir.display().to_string(),
                message: error.to_string(),
            })?;
        transcript_paths.sort();
        for path in transcript_paths {
            if path.extension().and_then(|value| value.to_str()) == Some("json") {
                transcripts.push(read_json_file::<SplitTranscriptFile>(&path)?);
            }
        }
    }

    Ok(runtime_project_from_split_parts(
        manifest,
        timeline,
        media,
        transcripts,
    ))
}

fn create_split_dirs(
    project_dir: &Path,
    manifest: &SplitProjectManifest,
) -> Result<(), SplitProjectError> {
    create_dir(project_dir)?;
    for path in [
        "media",
        &manifest.files.transcripts,
        &manifest.files.templates,
        &manifest.files.generated,
        &manifest.files.renders,
        &manifest.files.logs,
    ] {
        let dir = safe_join(project_dir, "files.directory", path)?;
        create_dir(&dir)?;
    }
    if let Some(parent) = safe_join(project_dir, "files.media", &manifest.files.media)?.parent() {
        create_dir(parent)?;
    }
    Ok(())
}

fn create_dir(path: &Path) -> Result<(), SplitProjectError> {
    std::fs::create_dir_all(path).map_err(|error| SplitProjectError::Io {
        path: path.display().to_string(),
        message: error.to_string(),
    })
}

fn write_json_file<T: Serialize>(path: &Path, value: &T) -> Result<(), SplitProjectError> {
    if let Some(parent) = path.parent() {
        create_dir(parent)?;
    }
    let json = serde_json::to_string_pretty(value).map_err(|error| SplitProjectError::Json {
        path: path.display().to_string(),
        message: error.to_string(),
    })?;
    let mut temp_file = tempfile::NamedTempFile::with_prefix_in(
        format!(
            ".{}.tmp.",
            path.file_name()
                .and_then(|value| value.to_str())
                .unwrap_or("split-project")
        ),
        path.parent().unwrap_or_else(|| Path::new(".")),
    )
    .map_err(|error| SplitProjectError::Io {
        path: path.display().to_string(),
        message: error.to_string(),
    })?;
    use std::io::Write;
    temp_file
        .write_all(json.as_bytes())
        .map_err(|error| SplitProjectError::Io {
            path: path.display().to_string(),
            message: error.to_string(),
        })?;
    temp_file
        .as_file()
        .sync_all()
        .map_err(|error| SplitProjectError::Io {
            path: path.display().to_string(),
            message: error.to_string(),
        })?;
    temp_file.persist(path).map_err(|error| SplitProjectError::Io {
        path: path.display().to_string(),
        message: error.error.to_string(),
    })?;
    Ok(())
}

fn read_json_file<T: for<'de> Deserialize<'de>>(path: &Path) -> Result<T, SplitProjectError> {
    let json = std::fs::read_to_string(path).map_err(|error| SplitProjectError::Io {
        path: path.display().to_string(),
        message: error.to_string(),
    })?;
    serde_json::from_str(&json).map_err(|error| SplitProjectError::Json {
        path: path.display().to_string(),
        message: error.to_string(),
    })
}
```

- [ ] **Step 4: Verify save/load tests pass**

Run:

```bash
rtk cargo test --manifest-path src-tauri/Cargo.toml --test project_split -- --nocapture
```

Expected: PASS.

- [ ] **Step 5: Commit split save/load**

Run:

```bash
rtk git add src-tauri/src/project/split.rs src-tauri/tests/project_split.rs
rtk git commit -m "feat: load and save split project files"
```

Expected: commit succeeds.

---

### Task 3: Split Project Validation Reports

**Files:**
- Modify: `src-tauri/src/project/split.rs`
- Modify: `src-tauri/tests/project_split.rs`

- [ ] **Step 1: Add failing validation tests**

Append to `src-tauri/tests/project_split.rs`:

```rust
use video_creater_lib::project::split::validate_split_project;

#[test]
fn validate_split_project_reports_missing_manifest_references() {
    let dir = tempfile::tempdir().expect("project dir");
    let project = sample_project();
    save_split_project(dir.path(), &project).expect("save split project");
    std::fs::remove_file(dir.path().join("timeline.json")).expect("remove timeline");

    let report = validate_split_project(dir.path()).expect("validation report");

    assert!(!report.ok);
    assert!(report.issues.iter().any(|issue| {
        issue.path == "timeline.json" && issue.message.contains("missing")
    }));
}

#[test]
fn validate_split_project_reports_duplicate_media_ids() {
    let dir = tempfile::tempdir().expect("project dir");
    let mut project = sample_project();
    let duplicate = project.media[0].clone();
    project.media.push(duplicate);
    save_split_project(dir.path(), &project).expect("save split project");

    let report = validate_split_project(dir.path()).expect("validation report");

    assert!(!report.ok);
    assert!(report
        .issues
        .iter()
        .any(|issue| issue.path == "media/index.json assets" && issue.message.contains("duplicate")));
}

#[test]
fn validate_split_project_reports_missing_media_references() {
    let dir = tempfile::tempdir().expect("project dir");
    let mut project = sample_project();
    project.timeline.tracks[0].items[0].source =
        video_creater_lib::project::model::TimelineSource::Media {
            media_id: "missing-media".to_string(),
        };
    save_split_project(dir.path(), &project).expect("save split project");

    let report = validate_split_project(dir.path()).expect("validation report");

    assert!(!report.ok);
    assert!(report.issues.iter().any(|issue| {
        issue.path == "timeline.json tracks[0].items[0].source.mediaId"
            && issue.message.contains("missing-media")
    }));
}

#[test]
fn validate_split_project_reports_source_ranges_outside_media_duration() {
    let dir = tempfile::tempdir().expect("project dir");
    let mut project = sample_project();
    project.timeline.tracks[0].items[0]
        .properties
        .insert("sourceIn".to_string(), serde_json::json!(0.2));
    project.timeline.tracks[0].items[0]
        .properties
        .insert("sourceOut".to_string(), serde_json::json!(999.0));
    save_split_project(dir.path(), &project).expect("save split project");

    let report = validate_split_project(dir.path()).expect("validation report");

    assert!(!report.ok);
    assert!(report.issues.iter().any(|issue| {
        issue.path == "timeline.json tracks[0].items[0].properties.sourceOut"
            && issue.message.contains("inside media duration")
    }));
}

#[test]
fn validate_split_project_reports_transcript_words_out_of_order() {
    let dir = tempfile::tempdir().expect("project dir");
    let mut project = sample_project();
    project.transcripts.push(video_creater_lib::project::model::Transcript {
        id: "transcript-media-1".to_string(),
        media_id: "media-1".to_string(),
        engine: None,
        raw_artifact_path: None,
        segments: Vec::new(),
        words: vec![
            video_creater_lib::project::model::TranscriptWord {
                text: "later".to_string(),
                start_seconds: 1.0,
                end_seconds: 1.2,
                confidence: None,
                speaker: None,
            },
            video_creater_lib::project::model::TranscriptWord {
                text: "earlier".to_string(),
                start_seconds: 0.5,
                end_seconds: 0.7,
                confidence: None,
                speaker: None,
            },
        ],
    });
    save_split_project(dir.path(), &project).expect("save split project");

    let report = validate_split_project(dir.path()).expect("validation report");

    assert!(!report.ok);
    assert!(report.issues.iter().any(|issue| {
        issue.path == "transcripts/media-1.json words[1].startSeconds"
            && issue.message.contains("monotonic")
    }));
}

#[test]
fn validate_split_project_accepts_valid_split_project() {
    let dir = tempfile::tempdir().expect("project dir");
    let project = sample_project();
    save_split_project(dir.path(), &project).expect("save split project");

    let report = validate_split_project(dir.path()).expect("validation report");

    assert!(report.ok);
    assert!(report.issues.is_empty());
}
```

- [ ] **Step 2: Run tests to verify red**

Run:

```bash
rtk cargo test --manifest-path src-tauri/Cargo.toml --test project_split validate_split_project_ -- --nocapture
```

Expected: FAIL because `validate_split_project` does not exist.

- [ ] **Step 3: Implement validation report**

Append to `src-tauri/src/project/split.rs`:

```rust
pub fn validate_split_project(project_dir: &Path) -> Result<ProjectValidationReport, SplitProjectError> {
    let mut issues = Vec::new();
    let manifest_path = split_project_manifest_path(project_dir);
    if !manifest_path.exists() {
        issues.push(issue(
            super::storage::PROJECT_FILE_NAME,
            "manifest file is missing",
            "Create video-creater.project.json before opening the split project.",
        ));
        return Ok(ProjectValidationReport { ok: false, issues });
    }

    let manifest: SplitProjectManifest = match read_json_file(&manifest_path) {
        Ok(manifest) => manifest,
        Err(error) => {
            issues.push(issue(
                super::storage::PROJECT_FILE_NAME,
                error.to_string(),
                "Repair the project manifest JSON.",
            ));
            return Ok(ProjectValidationReport { ok: false, issues });
        }
    };

    let timeline_path = match safe_join(project_dir, "files.timeline", &manifest.files.timeline) {
        Ok(path) => path,
        Err(error) => {
            issues.push(issue("video-creater.project.json files.timeline", error.to_string(), "Use a project-relative timeline path."));
            return Ok(ProjectValidationReport { ok: false, issues });
        }
    };
    let media_path = match safe_join(project_dir, "files.media", &manifest.files.media) {
        Ok(path) => path,
        Err(error) => {
            issues.push(issue("video-creater.project.json files.media", error.to_string(), "Use a project-relative media index path."));
            return Ok(ProjectValidationReport { ok: false, issues });
        }
    };

    if !timeline_path.exists() {
        issues.push(issue(
            &manifest.files.timeline,
            "timeline file is missing",
            "Create timeline.json or update the manifest files.timeline path.",
        ));
    }
    if !media_path.exists() {
        issues.push(issue(
            &manifest.files.media,
            "media index file is missing",
            "Create media/index.json or update the manifest files.media path.",
        ));
    }
    if !issues.is_empty() {
        return Ok(ProjectValidationReport { ok: false, issues });
    }

    let timeline: SplitTimelineFile = match read_json_file(&timeline_path) {
        Ok(timeline) => timeline,
        Err(error) => {
            issues.push(issue(&manifest.files.timeline, error.to_string(), "Repair timeline JSON."));
            return Ok(ProjectValidationReport { ok: false, issues });
        }
    };
    let media: SplitMediaIndexFile = match read_json_file(&media_path) {
        Ok(media) => media,
        Err(error) => {
            issues.push(issue(&manifest.files.media, error.to_string(), "Repair media index JSON."));
            return Ok(ProjectValidationReport { ok: false, issues });
        }
    };

    validate_media_index(&media, &mut issues);
    validate_timeline(&timeline, &media, &mut issues);
    validate_transcript_files(project_dir, &manifest, &media, &mut issues)?;

    Ok(ProjectValidationReport {
        ok: issues.is_empty(),
        issues,
    })
}

fn validate_media_index(media: &SplitMediaIndexFile, issues: &mut Vec<ProjectValidationIssue>) {
    let mut ids = std::collections::BTreeSet::new();
    for asset in &media.assets {
        if !ids.insert(asset.id.clone()) {
            issues.push(issue(
                "media/index.json assets",
                format!("duplicate media id `{}`", asset.id),
                "Use unique media asset ids.",
            ));
        }
        if !asset.duration_seconds.is_finite() || asset.duration_seconds < 0.0 {
            issues.push(issue(
                format!("media/index.json assets[{}].durationSeconds", asset.id),
                "media duration must be finite and non-negative",
                "Probe the media again or repair the duration.",
            ));
        }
    }
}

fn validate_timeline(
    timeline: &SplitTimelineFile,
    media: &SplitMediaIndexFile,
    issues: &mut Vec<ProjectValidationIssue>,
) {
    let media_ids = media
        .assets
        .iter()
        .map(|asset| asset.id.as_str())
        .collect::<std::collections::BTreeSet<_>>();
    let media_by_id = media
        .assets
        .iter()
        .map(|asset| (asset.id.as_str(), asset))
        .collect::<std::collections::BTreeMap<_, _>>();

    if !timeline.duration_seconds.is_finite() || timeline.duration_seconds < 0.0 {
        issues.push(issue(
            "timeline.json durationSeconds",
            "timeline duration must be finite and non-negative",
            "Recalculate timeline duration from item end times.",
        ));
    }

    let mut item_ids = std::collections::BTreeSet::new();
    for (track_index, track) in timeline.tracks.iter().enumerate() {
        for (item_index, item) in track.items.iter().enumerate() {
            let item_path = format!("timeline.json tracks[{track_index}].items[{item_index}]");
            if !item_ids.insert(item.id.clone()) {
                issues.push(issue(
                    format!("{item_path}.id"),
                    format!("duplicate timeline item id `{}`", item.id),
                    "Use unique timeline item ids.",
                ));
            }
            if !item.start_seconds.is_finite() || item.start_seconds < 0.0 {
                issues.push(issue(
                    format!("{item_path}.startSeconds"),
                    "item start must be finite and non-negative",
                    "Move the item to a valid timeline time.",
                ));
            }
            if !item.duration_seconds.is_finite() || item.duration_seconds <= 0.0 {
                issues.push(issue(
                    format!("{item_path}.durationSeconds"),
                    "item duration must be finite and greater than zero",
                    "Set a positive item duration.",
                ));
            }
            if let super::model::TimelineSource::Media { media_id } = &item.source {
                if !media_ids.contains(media_id.as_str()) {
                    issues.push(issue(
                        format!("{item_path}.source.mediaId"),
                        format!("timeline item references missing media `{media_id}`"),
                        "Add the media asset or update the timeline item source.",
                    ));
                } else if let Some(asset) = media_by_id.get(media_id.as_str()) {
                    validate_source_range(&item_path, item, asset.duration_seconds, issues);
                }
            }
        }
    }
}

fn validate_source_range(
    item_path: &str,
    item: &super::model::TimelineItem,
    media_duration: f64,
    issues: &mut Vec<ProjectValidationIssue>,
) {
    let source_in = item.properties.get("sourceIn").and_then(serde_json::Value::as_f64);
    let source_out = item.properties.get("sourceOut").and_then(serde_json::Value::as_f64);
    if let Some(value) = source_in {
        if !value.is_finite() || value < 0.0 {
            issues.push(issue(
                format!("{item_path}.properties.sourceIn"),
                "sourceIn must be finite and non-negative",
                "Set sourceIn inside the source media duration.",
            ));
        }
    }
    if let Some(value) = source_out {
        if !value.is_finite() || value < 0.0 || value > media_duration {
            issues.push(issue(
                format!("{item_path}.properties.sourceOut"),
                "sourceOut must be finite, non-negative, and inside media duration",
                "Set sourceOut inside the source media duration.",
            ));
        }
    }
    if let (Some(source_in), Some(source_out)) = (source_in, source_out) {
        if source_out <= source_in {
            issues.push(issue(
                format!("{item_path}.properties.sourceOut"),
                "sourceOut must be greater than sourceIn",
                "Increase sourceOut or decrease sourceIn.",
            ));
        }
    }
}

fn validate_transcript_files(
    project_dir: &Path,
    manifest: &SplitProjectManifest,
    media: &SplitMediaIndexFile,
    issues: &mut Vec<ProjectValidationIssue>,
) -> Result<(), SplitProjectError> {
    let transcripts_dir = safe_join(project_dir, "files.transcripts", &manifest.files.transcripts)?;
    if !transcripts_dir.exists() {
        return Ok(());
    }
    let media_ids = media
        .assets
        .iter()
        .map(|asset| asset.id.as_str())
        .collect::<std::collections::BTreeSet<_>>();
    let mut transcript_paths = std::fs::read_dir(&transcripts_dir)
        .map_err(|error| SplitProjectError::Io {
            path: transcripts_dir.display().to_string(),
            message: error.to_string(),
        })?
        .map(|entry| entry.map(|entry| entry.path()))
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| SplitProjectError::Io {
            path: transcripts_dir.display().to_string(),
            message: error.to_string(),
        })?;
    transcript_paths.sort();
    for path in transcript_paths {
        if path.extension().and_then(|value| value.to_str()) != Some("json") {
            continue;
        }
        let transcript: SplitTranscriptFile = match read_json_file(&path) {
            Ok(transcript) => transcript,
            Err(error) => {
                issues.push(issue(path.display().to_string(), error.to_string(), "Repair transcript JSON."));
                continue;
            }
        };
        let display_path = path
            .strip_prefix(project_dir)
            .unwrap_or(&path)
            .display()
            .to_string();
        if !media_ids.contains(transcript.media_id.as_str()) {
            issues.push(issue(
                format!("{display_path} mediaId"),
                format!("transcript references missing media `{}`", transcript.media_id),
                "Add the media asset or remove the transcript file.",
            ));
        }
        let mut previous_start = 0.0;
        for (index, word) in transcript.words.iter().enumerate() {
            if !word.start_seconds.is_finite()
                || !word.end_seconds.is_finite()
                || word.start_seconds < previous_start
                || word.end_seconds < word.start_seconds
            {
                issues.push(issue(
                    format!("{display_path} words[{index}].startSeconds"),
                    "transcript word timestamps must be finite and monotonic",
                    "Repair word start/end timestamps so they increase over source time.",
                ));
            }
            previous_start = word.start_seconds;
        }
    }
    Ok(())
}

fn issue(
    path: impl Into<String>,
    message: impl Into<String>,
    fix: impl Into<String>,
) -> ProjectValidationIssue {
    ProjectValidationIssue {
        path: path.into(),
        message: message.into(),
        fix: fix.into(),
    }
}
```

- [ ] **Step 4: Verify validation tests pass**

Run:

```bash
rtk cargo test --manifest-path src-tauri/Cargo.toml --test project_split validate_split_project_ -- --nocapture
```

Expected: PASS.

- [ ] **Step 5: Commit split validation**

Run:

```bash
rtk git add src-tauri/src/project/split.rs src-tauri/tests/project_split.rs
rtk git commit -m "feat: validate split project files"
```

Expected: commit succeeds.

---

### Task 4: Schema V1 To Split Project Migration

**Files:**
- Modify: `src-tauri/src/project/split.rs`
- Modify: `src-tauri/tests/project_split.rs`

- [ ] **Step 1: Add failing migration tests**

Append to `src-tauri/tests/project_split.rs`:

```rust
use video_creater_lib::project::split::migrate_single_file_project_to_split;
use video_creater_lib::project::storage::save_project;

#[test]
fn migrate_single_file_project_to_split_preserves_runtime_state_and_writes_report() {
    let dir = tempfile::tempdir().expect("project dir");
    let project = sample_project();
    save_project(dir.path(), &project).expect("save schema v1 project");

    let report = migrate_single_file_project_to_split(dir.path()).expect("migrate project");

    assert!(dir.path().join("video-creater.project.json").exists());
    assert!(dir.path().join("timeline.json").exists());
    assert!(dir.path().join("media/index.json").exists());
    assert!(report
        .written_files
        .iter()
        .any(|path| path.contains("project-migration-")));

    let loaded = load_split_project(dir.path()).expect("load migrated project");
    assert_eq!(loaded.schema_version, 2);
    assert_eq!(loaded.id, project.id);
    assert_eq!(loaded.media, project.media);
    assert_eq!(loaded.timeline, project.timeline);
}

#[test]
fn migrate_single_file_project_to_split_writes_log_with_source_and_output_files() {
    let dir = tempfile::tempdir().expect("project dir");
    let project = sample_project();
    save_project(dir.path(), &project).expect("save schema v1 project");

    let report = migrate_single_file_project_to_split(dir.path()).expect("migrate project");
    let migration_report_path = report
        .written_files
        .iter()
        .find(|path| path.contains("project-migration-"))
        .expect("migration report");
    let report_json = std::fs::read_to_string(migration_report_path).expect("read migration report");
    let report_value: serde_json::Value =
        serde_json::from_str(&report_json).expect("migration report json");

    assert_eq!(
        report_value["sourceSchemaVersion"],
        serde_json::json!(1)
    );
    assert_eq!(
        report_value["targetSchemaVersion"],
        serde_json::json!(2)
    );
    assert!(report_value["writtenFiles"]
        .as_array()
        .expect("written files")
        .iter()
        .any(|value| value.as_str().unwrap_or("").ends_with("timeline.json")));
}
```

- [ ] **Step 2: Run tests to verify red**

Run:

```bash
rtk cargo test --manifest-path src-tauri/Cargo.toml --test project_split migrate_single_file_project_to_split_ -- --nocapture
```

Expected: FAIL because migration function does not exist.

- [ ] **Step 3: Implement migration**

Append to `src-tauri/src/project/split.rs`:

```rust
pub fn migrate_single_file_project_to_split(
    project_dir: &Path,
) -> Result<ProjectWriteReport, SplitProjectError> {
    let source_project = super::storage::load_project(project_dir).map_err(|error| SplitProjectError::Io {
        path: super::storage::project_file_path(project_dir).display().to_string(),
        message: error.to_string(),
    })?;
    let mut split_project = source_project;
    split_project.schema_version = SPLIT_PROJECT_SCHEMA_VERSION;
    let mut report = save_split_project(project_dir, &split_project)?;

    let migration_log_path = project_dir
        .join("logs")
        .join(format!("project-migration-{}.json", chrono::Utc::now().format("%Y%m%dT%H%M%SZ")));
    let migration_report = serde_json::json!({
        "sourceSchemaVersion": 1,
        "targetSchemaVersion": SPLIT_PROJECT_SCHEMA_VERSION,
        "manifestPath": report.manifest_path,
        "writtenFiles": report.written_files,
        "createdAt": chrono::Utc::now().to_rfc3339(),
    });
    write_json_file(&migration_log_path, &migration_report)?;
    report.written_files.push(migration_log_path.display().to_string());
    Ok(report)
}
```

- [ ] **Step 4: Verify migration tests pass**

Run:

```bash
rtk cargo test --manifest-path src-tauri/Cargo.toml --test project_split migrate_single_file_project_to_split_ -- --nocapture
```

Expected: PASS.

- [ ] **Step 5: Commit migration**

Run:

```bash
rtk git add src-tauri/src/project/split.rs src-tauri/tests/project_split.rs
rtk git commit -m "feat: migrate projects to split files"
```

Expected: commit succeeds.

---

### Task 5: Tauri Commands For Split Project Operations

**Files:**
- Modify: `src-tauri/src/main.rs`
- Test: `src-tauri/tests/project_split.rs`

- [ ] **Step 1: Add command-level library test through functions where possible**

Append to `src-tauri/tests/project_split.rs`:

```rust
#[test]
fn split_project_error_messages_are_user_facing() {
    let dir = tempfile::tempdir().expect("project dir");
    let error = load_split_project(dir.path()).expect_err("missing manifest");

    let message = error.to_string();

    assert!(message.contains("split project"));
    assert!(message.contains("video-creater.project.json") || message.contains("No such file"));
}
```

- [ ] **Step 2: Run focused tests**

Run:

```bash
rtk cargo test --manifest-path src-tauri/Cargo.toml --test project_split split_project_error_messages_are_user_facing -- --nocapture
```

Expected: PASS after prior split error implementation. If it fails because the message is too generic, adjust `SplitProjectError::Io` display context to include the path.

- [ ] **Step 3: Import split functions in `src-tauri/src/main.rs`**

Modify the project imports in `src-tauri/src/main.rs`:

```rust
use video_creater_lib::project::split::{
    load_split_project, migrate_single_file_project_to_split, save_split_project,
    validate_split_project, ProjectValidationReport, ProjectWriteReport,
};
```

- [ ] **Step 4: Add Tauri command functions**

Add near the existing project commands in `src-tauri/src/main.rs`:

```rust
#[tauri::command]
fn load_split_project_from_folder(project_dir: String) -> Result<VideoProject, String> {
    let project_dir = resolve_project_dir(&project_dir)?;
    load_split_project(&project_dir).map_err(|error| error.to_string())
}

#[tauri::command]
fn save_split_project_to_folder(
    project_dir: String,
    project: VideoProject,
) -> Result<ProjectWriteReport, String> {
    let project_dir = resolve_project_dir(&project_dir)?;
    save_split_project(&project_dir, &project).map_err(|error| error.to_string())
}

#[tauri::command]
fn validate_split_project_folder(project_dir: String) -> Result<ProjectValidationReport, String> {
    let project_dir = resolve_project_dir(&project_dir)?;
    validate_split_project(&project_dir).map_err(|error| error.to_string())
}

#[tauri::command]
fn migrate_project_folder_to_split(project_dir: String) -> Result<ProjectWriteReport, String> {
    let project_dir = resolve_project_dir(&project_dir)?;
    migrate_single_file_project_to_split(&project_dir).map_err(|error| error.to_string())
}
```

- [ ] **Step 5: Register Tauri commands**

In the `tauri::generate_handler!` list in `src-tauri/src/main.rs`, add:

```rust
load_split_project_from_folder,
save_split_project_to_folder,
validate_split_project_folder,
migrate_project_folder_to_split,
```

- [ ] **Step 6: Verify main crate compiles**

Run:

```bash
rtk cargo check --manifest-path src-tauri/Cargo.toml
```

Expected: PASS.

- [ ] **Step 7: Commit Tauri split commands**

Run:

```bash
rtk git add src-tauri/src/main.rs src-tauri/tests/project_split.rs
rtk git commit -m "feat: expose split project commands"
```

Expected: commit succeeds.

---

### Task 6: TypeScript Project API Wrappers

**Files:**
- Modify: `src/lib/project.ts`
- Create: `src/lib/project-split.test.ts`

- [ ] **Step 1: Write failing TypeScript wrapper tests**

Create `src/lib/project-split.test.ts`:

```ts
import { beforeEach, describe, expect, it, vi } from "vitest";
import { invoke } from "@tauri-apps/api/core";
import {
  loadSplitProjectFromFolder,
  migrateProjectFolderToSplit,
  saveSplitProjectToFolder,
  validateSplitProjectFolder,
  type ProjectValidationReport,
  type ProjectWriteReport,
  type VideoProject,
} from "./project";

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(),
}));

const invokeMock = vi.mocked(invoke);

function sampleProject(): VideoProject {
  return {
    schemaVersion: 2,
    id: "project-test",
    name: "Test Project",
    createdAt: "2026-06-11T00:00:00Z",
    updatedAt: "2026-06-11T00:00:00Z",
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
}

describe("split project API", () => {
  beforeEach(() => {
    invokeMock.mockReset();
  });

  it("loads a split project from a folder", async () => {
    const project = sampleProject();
    invokeMock.mockResolvedValue(project);

    await expect(loadSplitProjectFromFolder("/tmp/project")).resolves.toEqual(project);

    expect(invokeMock).toHaveBeenCalledWith("load_split_project_from_folder", {
      projectDir: "/tmp/project",
    });
  });

  it("saves a split project to a folder", async () => {
    const project = sampleProject();
    const report: ProjectWriteReport = {
      manifestPath: "/tmp/project/video-creater.project.json",
      writtenFiles: ["/tmp/project/timeline.json"],
      removedFiles: [],
    };
    invokeMock.mockResolvedValue(report);

    await expect(saveSplitProjectToFolder({ projectDir: "/tmp/project", project })).resolves.toEqual(
      report,
    );

    expect(invokeMock).toHaveBeenCalledWith("save_split_project_to_folder", {
      projectDir: "/tmp/project",
      project,
    });
  });

  it("validates a split project folder", async () => {
    const report: ProjectValidationReport = { ok: true, issues: [] };
    invokeMock.mockResolvedValue(report);

    await expect(validateSplitProjectFolder("/tmp/project")).resolves.toEqual(report);

    expect(invokeMock).toHaveBeenCalledWith("validate_split_project_folder", {
      projectDir: "/tmp/project",
    });
  });

  it("migrates a project folder to split files", async () => {
    const report: ProjectWriteReport = {
      manifestPath: "/tmp/project/video-creater.project.json",
      writtenFiles: ["/tmp/project/timeline.json"],
      removedFiles: [],
    };
    invokeMock.mockResolvedValue(report);

    await expect(migrateProjectFolderToSplit("/tmp/project")).resolves.toEqual(report);

    expect(invokeMock).toHaveBeenCalledWith("migrate_project_folder_to_split", {
      projectDir: "/tmp/project",
    });
  });
});
```

- [ ] **Step 2: Run tests to verify red**

Run:

```bash
rtk pnpm test -- src/lib/project-split.test.ts
```

Expected: FAIL because wrapper functions and types do not exist.

- [ ] **Step 3: Add TypeScript split project types and wrappers**

Append to `src/lib/project.ts`:

```ts
export interface ProjectValidationIssue {
  path: string;
  message: string;
  fix: string;
}

export interface ProjectValidationReport {
  ok: boolean;
  issues: ProjectValidationIssue[];
}

export interface ProjectWriteReport {
  manifestPath: string;
  writtenFiles: string[];
  removedFiles: string[];
}

export async function loadSplitProjectFromFolder(projectDir: string): Promise<VideoProject> {
  return invoke("load_split_project_from_folder", { projectDir });
}

export async function saveSplitProjectToFolder(input: {
  projectDir: string;
  project: VideoProject;
}): Promise<ProjectWriteReport> {
  return invoke("save_split_project_to_folder", input);
}

export async function validateSplitProjectFolder(
  projectDir: string,
): Promise<ProjectValidationReport> {
  return invoke("validate_split_project_folder", { projectDir });
}

export async function migrateProjectFolderToSplit(
  projectDir: string,
): Promise<ProjectWriteReport> {
  return invoke("migrate_project_folder_to_split", { projectDir });
}
```

- [ ] **Step 4: Verify TypeScript tests pass**

Run:

```bash
rtk pnpm test -- src/lib/project-split.test.ts
```

Expected: PASS.

- [ ] **Step 5: Commit TypeScript wrappers**

Run:

```bash
rtk git add src/lib/project.ts src/lib/project-split.test.ts
rtk git commit -m "feat: add split project frontend API"
```

Expected: commit succeeds.

---

### Task 7: Full Verification

**Files:**
- No new files.

- [ ] **Step 1: Run Rust split project tests**

Run:

```bash
rtk cargo test --manifest-path src-tauri/Cargo.toml --test project_split -- --nocapture
```

Expected: PASS.

- [ ] **Step 2: Run existing project patch tests**

Run:

```bash
rtk cargo test --manifest-path src-tauri/Cargo.toml --test project_patch -- --nocapture
```

Expected: PASS.

- [ ] **Step 3: Run TypeScript split project tests**

Run:

```bash
rtk pnpm test -- src/lib/project-split.test.ts
```

Expected: PASS.

- [ ] **Step 4: Run type checks**

Run:

```bash
rtk pnpm lint
```

Expected: PASS.

- [ ] **Step 5: Run Rust format check for touched files**

Run:

```bash
rtk rustfmt --edition 2021 --check src-tauri/src/project/split.rs src-tauri/tests/project_split.rs src-tauri/src/main.rs
```

Expected: PASS.

- [ ] **Step 6: Inspect final status**

Run:

```bash
rtk git status --short
```

Expected: only pre-existing unrelated dirty files remain, plus no unstaged changes from this plan.

---

## Self-Review

Spec coverage:

- Covered split manifest, timeline, media index, transcripts, split load/save, validation, schema-v1 migration, validation reports, Tauri bridge, and TypeScript API.
- Deferred `ProjectAction`, UI mutation routing, generated asset provenance, template override files, render report files, and MCP wrappers to follow-up plans because they are separate implementation slices from the approved spec.

Placeholder scan:

- No incomplete markers or open-ended implementation steps are intentionally left in this plan.

Type consistency:

- Rust uses `SplitProjectManifest`, `SplitProjectFiles`, `SplitTimelineFile`, `SplitMediaIndexFile`, `SplitTranscriptFile`, `ProjectValidationReport`, `ProjectValidationIssue`, and `ProjectWriteReport`.
- TypeScript mirrors `ProjectValidationReport`, `ProjectValidationIssue`, and `ProjectWriteReport`.
- Commands are consistently named `load_split_project_from_folder`, `save_split_project_to_folder`, `validate_split_project_folder`, and `migrate_project_folder_to_split`.
