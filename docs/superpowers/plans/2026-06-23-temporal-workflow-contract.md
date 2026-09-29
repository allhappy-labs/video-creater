# Temporal Workflow Contract Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a Temporal-shaped Rust workflow contract for Video Creater job records before migrating render, transcription, media generation, and Codex edit execution into workers.

**Architecture:** Add a focused `src-tauri/src/workflows` module that owns task queue names, workflow type names, activity type names, deterministic workflow IDs, and `JobSummary` construction. Extend `JobSummary` with optional `TemporalWorkflowMetadata` so current project files remain backward compatible.

**Tech Stack:** Rust, serde, Cargo tests, existing project model.

---

### Task 1: Add Failing Workflow Contract Tests

**Files:**
- Create: `src-tauri/tests/temporal_workflows.rs`

- [ ] **Step 1: Add serialization and ID tests**

```rust
use serde_json::json;
use video_creater_lib::project::model::{JobStatus, JobSummary};
use video_creater_lib::workflows::{
    temporal_job_summary, temporal_workflow_spec, TemporalWorkflowKind,
    VIDEO_CREATER_TEMPORAL_TASK_QUEUE,
};

#[test]
fn temporal_job_summary_serializes_workflow_metadata() {
    let summary = temporal_job_summary(
        TemporalWorkflowKind::RenderDraft,
        "Project A",
        "Draft Render 1",
        JobStatus::Queued,
        "2026-06-23T12:00:00Z",
    );

    let value = serde_json::to_value(summary).expect("serialize summary");

    assert_eq!(
        value,
        json!({
            "id": "Draft Render 1",
            "kind": "render_draft",
            "status": "queued",
            "updatedAt": "2026-06-23T12:00:00Z",
            "workflow": {
                "workflowId": "video-creater/project-a/render-draft/draft-render-1",
                "workflowType": "VideoCreaterRenderDraftWorkflow",
                "taskQueue": "video-creater-workflows",
                "runId": null,
                "activityTypes": [
                    "BuildRenderPlan",
                    "RenderMedia",
                    "ValidateRenderedMedia",
                    "AttachRenderReport"
                ]
            }
        })
    );
}

#[test]
fn temporal_workflow_spec_maps_media_generation_to_registered_types() {
    let spec = temporal_workflow_spec(
        TemporalWorkflowKind::GenerateMedia,
        "project-1",
        "generated-shot-1",
    );

    assert_eq!(spec.task_queue, VIDEO_CREATER_TEMPORAL_TASK_QUEUE);
    assert_eq!(spec.workflow_type, "VideoCreaterGenerateMediaWorkflow");
    assert_eq!(
        spec.workflow_id,
        "video-creater/project-1/generate-media/generated-shot-1"
    );
    assert_eq!(
        spec.activity_types,
        vec![
            "RecordGenerationPrompt",
            "RunMediaProviderGeneration",
            "ImportGeneratedOutput",
            "AttachGeneratedAssetResult"
        ]
    );
}

#[test]
fn legacy_job_summaries_still_deserialize_without_workflow_metadata() {
    let summary: JobSummary = serde_json::from_value(json!({
        "id": "legacy-job",
        "kind": "render",
        "status": "completed",
        "updatedAt": "2026-06-20T00:00:00Z"
    }))
    .expect("deserialize legacy job summary");

    assert_eq!(summary.id, "legacy-job");
    assert_eq!(summary.workflow, None);
}
```

- [ ] **Step 2: Run red tests**

```bash
rtk cargo test --manifest-path src-tauri/Cargo.toml --test temporal_workflows
```

Expected: FAIL because `video_creater_lib::workflows` and `JobSummary.workflow` do not exist.

### Task 2: Implement Workflow Contract Module

**Files:**
- Create: `src-tauri/src/workflows/mod.rs`
- Modify: `src-tauri/src/lib.rs`
- Modify: `src-tauri/src/project/model.rs`

- [ ] **Step 1: Add the module export**

In `src-tauri/src/lib.rs`, add:

```rust
pub mod workflows;
```

- [ ] **Step 2: Extend `JobSummary`**

In `src-tauri/src/project/model.rs`, add:

```rust
pub workflow: Option<TemporalWorkflowMetadata>,
```

with:

```rust
#[serde(default, skip_serializing_if = "Option::is_none")]
```

Add:

```rust
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct TemporalWorkflowMetadata {
    pub workflow_id: String,
    pub workflow_type: String,
    pub task_queue: String,
    pub run_id: Option<String>,
    pub activity_types: Vec<String>,
}
```

- [ ] **Step 3: Add workflow contract implementation**

Create `src-tauri/src/workflows/mod.rs` with:

```rust
use crate::project::model::{JobStatus, JobSummary, TemporalWorkflowMetadata};

pub const VIDEO_CREATER_TEMPORAL_TASK_QUEUE: &str = "video-creater-workflows";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TemporalWorkflowKind {
    GenerateMedia,
    RenderDraft,
    TranscribeMedia,
    CodexEdit,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TemporalWorkflowSpec {
    pub workflow_id: String,
    pub workflow_type: String,
    pub task_queue: String,
    pub activity_types: Vec<String>,
}

pub fn temporal_workflow_spec(
    kind: TemporalWorkflowKind,
    project_id: &str,
    job_id: &str,
) -> TemporalWorkflowSpec {
    TemporalWorkflowSpec {
        workflow_id: format!(
            "video-creater/{}/{}/{}",
            safe_workflow_segment(project_id),
            kind.id_segment(),
            safe_workflow_segment(job_id)
        ),
        workflow_type: kind.workflow_type().to_string(),
        task_queue: VIDEO_CREATER_TEMPORAL_TASK_QUEUE.to_string(),
        activity_types: kind
            .activity_types()
            .iter()
            .map(|activity| (*activity).to_string())
            .collect(),
    }
}

pub fn temporal_job_summary(
    kind: TemporalWorkflowKind,
    project_id: &str,
    job_id: &str,
    status: JobStatus,
    updated_at: &str,
) -> JobSummary {
    let spec = temporal_workflow_spec(kind, project_id, job_id);
    JobSummary {
        id: job_id.to_string(),
        kind: kind.job_kind().to_string(),
        status,
        updated_at: updated_at.to_string(),
        workflow: Some(TemporalWorkflowMetadata {
            workflow_id: spec.workflow_id,
            workflow_type: spec.workflow_type,
            task_queue: spec.task_queue,
            run_id: None,
            activity_types: spec.activity_types,
        }),
    }
}
```

Add helper methods for `TemporalWorkflowKind` and `safe_workflow_segment()` matching the tests.

- [ ] **Step 4: Run green tests**

```bash
rtk cargo test --manifest-path src-tauri/Cargo.toml --test temporal_workflows
```

Expected: tests pass.

### Task 3: Verification and Commit

**Files:**
- Source: `src-tauri/src/workflows/mod.rs`, `src-tauri/src/lib.rs`, `src-tauri/src/project/model.rs`
- Tests: `src-tauri/tests/temporal_workflows.rs`
- Docs: `docs/superpowers/specs/2026-06-23-temporal-workflow-contract-design.md`, `docs/superpowers/plans/2026-06-23-temporal-workflow-contract.md`

- [ ] **Step 1: Run focused Rust checks**

```bash
rtk cargo test --manifest-path src-tauri/Cargo.toml --test temporal_workflows
rtk cargo test --manifest-path src-tauri/Cargo.toml --test project_split migrate_single_file_project_writes_split_files_and_preserves_runtime_data -- --exact
```

Expected: tests pass.

- [ ] **Step 2: Run frontend contract checks**

```bash
rtk pnpm test -- src/lib/project.test.ts
rtk pnpm test
```

Expected: tests pass.

- [ ] **Step 3: Commit**

```bash
rtk git add docs/superpowers/specs/2026-06-23-temporal-workflow-contract-design.md docs/superpowers/plans/2026-06-23-temporal-workflow-contract.md src-tauri/src/workflows/mod.rs src-tauri/src/lib.rs src-tauri/src/project/model.rs src-tauri/tests/temporal_workflows.rs
rtk git commit -m "feat: add temporal workflow contract"
```
