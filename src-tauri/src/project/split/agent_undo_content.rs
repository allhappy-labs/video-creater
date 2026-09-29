//! What agent batch Undo compares and what it restores.
//!
//! Job bookkeeping (`jobs`, `renderReports`, `exportArtifacts`) records
//! background work: the result-frame capture, job status updates, render
//! reports and export artifacts. It is not a user-visible edit, so it neither
//! blocks Undo nor is rolled back by it. Undo restores the pre-apply snapshot's
//! content with the bookkeeping currently on disk, minus the records the
//! undone batch itself added.
//!
//! Generated assets stay part of the compared content, except for the
//! background progress of the generations the undone batch recorded itself
//! (rule version 3, see `agent_undo_comparable_content`): approving a
//! "Generate & place" bundle starts those generations, and their status,
//! outputs and placement over the batch's placeholder must not block its Undo.
//! Rule version 4 also ignores the progress of generations recorded before the
//! batch that the batch left alone (see `agent_undo_background`).

use super::agent_undo_background::{background_comparable_content, carry_background_progress};
use super::{SplitAgentEditHistoryEntry, SplitProjectError};
use crate::project::model::{
    GeneratedAsset, GeneratedAssetStatus, MediaAsset, MediaKind, Timeline, TimelineItem,
    TimelineItemKind, TimelineSource, VideoProject,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};

/// `afterContentHashVersion` of hashes computed by
/// `agent_project_content_hash_with_background` with the entry's
/// `addedGeneratedAssetIds` and `backgroundGeneratedAssetIds`. Version 3 hashes
/// used `agent_project_content_hash` with the added ids only; version 2 hashes
/// used it without generation scope; entries without a version carry the
/// original rule, which also hashed job bookkeeping.
pub(super) const AGENT_CONTENT_HASH_VERSION: u32 = 4;
const GENERATION_SCOPED_HASH_VERSION: u32 = 3;
const GENERATION_UNSCOPED_HASH_VERSION: u32 = 2;

/// Placement bookkeeping the editor and completion write on a generation's
/// placeholder or output clip.
const GENERATION_PLACEMENT_PROPERTIES: [&str; 8] = [
    "generatedAssetId",
    "generatedOutputMediaId",
    "generatedTimelinePlaceholder",
    "generatedPlaceholder",
    "pendingGeneratedAssetId",
    "reason",
    "sourceIn",
    "sourceOut",
];

/// IDs of the job bookkeeping records an agent batch added. Undo removes
/// exactly these and keeps every other current record.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SplitAgentBookkeepingIds {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub jobs: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub render_reports: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub export_artifacts: Vec<String>,
}

impl SplitAgentBookkeepingIds {
    pub fn is_empty(&self) -> bool {
        self.jobs.is_empty() && self.render_reports.is_empty() && self.export_artifacts.is_empty()
    }

    /// Records present in `after` but not in `before`.
    pub(super) fn added_between(before: &VideoProject, after: &VideoProject) -> Self {
        Self {
            jobs: added_ids(&before.jobs, &after.jobs, |job| &job.id),
            render_reports: added_ids(&before.render_reports, &after.render_reports, |report| {
                &report.id
            }),
            export_artifacts: added_ids(
                &before.export_artifacts,
                &after.export_artifacts,
                |artifact| &artifact.id,
            ),
        }
    }
}

/// Generated assets present in `after` but not in `before`: the generations a
/// batch recorded (`addedGeneratedAssetIds`).
pub(super) fn added_generated_asset_ids(
    before: &VideoProject,
    after: &VideoProject,
) -> Vec<String> {
    added_ids(&before.generated_assets, &after.generated_assets, |asset| {
        &asset.id
    })
}

fn added_ids<T>(before: &[T], after: &[T], id: fn(&T) -> &str) -> Vec<String> {
    after
        .iter()
        .map(id)
        .filter(|candidate| !before.iter().any(|record| id(record) == *candidate))
        .map(str::to_string)
        .collect()
}

/// The project as Undo compares it.
///
/// Always ignored: the CAS revision, the conversation bookkeeping
/// (`codexThreadId`, `updatedAt`) and job bookkeeping (`jobs`, `renderReports`,
/// `exportArtifacts`), which are written without editing the project.
///
/// For each generated asset in `batch_generated_asset_ids` (the generations
/// the batch recorded), background progress is ignored too:
///
/// 1. The asset's `status`, `outputs`, `references.providerInputUrls` and
///    `createdAt`.
/// 2. Its generation clips on every timeline, reduced to their track, start,
///    duration and remaining properties. A generation clip is the placeholder
///    (`generated` source naming the asset), an output clip (`media` source
///    naming one of the asset's outputs with `generatedAssetId` set to the
///    asset), or the clip a `replace:<itemId>` placement targets. Its `id`,
///    `kind`, `source`, `label`, the placement properties
///    (`GENERATION_PLACEMENT_PROPERTIES`) and a `linkGroupId` of
///    `link-<its id>` are cleared. An output audio clip linked to another
///    generation clip of the same asset (`linkGroupId` `link-<that clip id>`)
///    is dropped.
/// 3. Library media that completion added for one of the asset's outputs,
///    exactly as `completeGeneratedAsset` records it (generated kind, no name,
///    the asset's target folder, the output's path and dimensions), while no
///    timeline clip other than the asset's generation clips uses it.
///
/// Anything else, such as moving or restyling a generation clip, placing or
/// renaming its output media, or changing another asset, still differs.
pub fn agent_undo_comparable_content(
    project: &VideoProject,
    batch_generated_asset_ids: &[String],
) -> VideoProject {
    let mut content = project.clone();
    content.content_revision = 0;
    content.codex_thread_id = None;
    content.updated_at = String::new();
    content.jobs = Vec::new();
    content.render_reports = Vec::new();
    content.export_artifacts = Vec::new();

    let batch = project
        .generated_assets
        .iter()
        .filter(|asset| batch_generated_asset_ids.contains(&asset.id))
        .collect::<Vec<_>>();
    if batch.is_empty() {
        return content;
    }

    let background_media = background_output_media_ids(project, &batch);
    content
        .media
        .retain(|media| !background_media.contains(media.id.as_str()));
    reduce_generation_clips(&mut content.timeline, &batch);
    for entry in &mut content.timelines {
        reduce_generation_clips(&mut entry.timeline, &batch);
    }
    for asset in &mut content.generated_assets {
        if batch_generated_asset_ids.contains(&asset.id) {
            asset.status = GeneratedAssetStatus::Queued;
            asset.outputs = Vec::new();
            asset.references.provider_input_urls = Vec::new();
            asset.created_at = String::new();
        }
    }
    content
}

/// The batch asset whose generation clip `item` is (rule 2).
fn generation_clip_owner<'a>(item: &TimelineItem, batch: &[&'a GeneratedAsset]) -> Option<&'a str> {
    batch
        .iter()
        .find(|asset| match &item.source {
            TimelineSource::Generated { artifact_id } if *artifact_id == asset.id => true,
            TimelineSource::Media { media_id }
                if item.properties.get("generatedAssetId")
                    == Some(&serde_json::Value::String(asset.id.clone()))
                    && asset
                        .outputs
                        .iter()
                        .any(|output| output.media_id == *media_id) =>
            {
                true
            }
            _ => asset
                .placement_intent
                .as_deref()
                .and_then(|intent| intent.strip_prefix("replace:"))
                .is_some_and(|target| target == item.id),
        })
        .map(|asset| asset.id.as_str())
}

fn reduce_generation_clips(timeline: &mut Timeline, batch: &[&GeneratedAsset]) {
    let owners = timeline
        .tracks
        .iter()
        .flat_map(|track| track.items.iter())
        .filter_map(|item| generation_clip_owner(item, batch).map(|owner| (item.id.clone(), owner)))
        .collect::<BTreeMap<_, _>>();
    if owners.is_empty() {
        return;
    }
    let linked_companion = |item: &TimelineItem, owner: &str| {
        item.kind == TimelineItemKind::AudioClip
            && item
                .properties
                .get("linkGroupId")
                .and_then(serde_json::Value::as_str)
                .and_then(|group| group.strip_prefix("link-"))
                .is_some_and(|linked| {
                    linked != item.id && owners.get(linked).is_some_and(|other| *other == owner)
                })
    };
    for track in &mut timeline.tracks {
        track.items.retain(|item| {
            owners
                .get(&item.id)
                .is_none_or(|owner| !linked_companion(item, owner))
        });
        for item in &mut track.items {
            let Some(owner) = owners.get(&item.id) else {
                continue;
            };
            let own_link = format!("link-{}", item.id);
            if item
                .properties
                .get("linkGroupId")
                .and_then(serde_json::Value::as_str)
                == Some(own_link.as_str())
            {
                item.properties.remove("linkGroupId");
            }
            for key in GENERATION_PLACEMENT_PROPERTIES {
                item.properties.remove(key);
            }
            item.id = format!("generation-clip:{owner}");
            item.kind = TimelineItemKind::GeneratedClip;
            item.source = TimelineSource::Generated {
                artifact_id: (*owner).to_string(),
            };
            item.label = String::new();
        }
    }
}

/// Output media completion added for the batch's assets that no clip other
/// than their generation clips uses (rule 3).
fn background_output_media_ids<'a>(
    project: &'a VideoProject,
    batch: &[&'a GeneratedAsset],
) -> BTreeSet<&'a str> {
    let timelines = std::iter::once(&project.timeline)
        .chain(project.timelines.iter().map(|entry| &entry.timeline))
        .collect::<Vec<_>>();
    project
        .media
        .iter()
        .filter(|media| {
            batch.iter().any(|asset| {
                completion_added(asset, media)
                    && timelines
                        .iter()
                        .flat_map(|timeline| timeline.tracks.iter())
                        .flat_map(|track| track.items.iter())
                        .filter(|item| {
                            matches!(&item.source, TimelineSource::Media { media_id } if *media_id == media.id)
                        })
                        .all(|item| generation_clip_owner(item, batch) == Some(asset.id.as_str()))
            })
        })
        .map(|media| media.id.as_str())
        .collect()
}

/// Whether `media` is exactly the library record `completeGeneratedAsset`
/// adds for one of `asset`'s outputs.
pub(super) fn completion_added(asset: &GeneratedAsset, media: &MediaAsset) -> bool {
    asset.outputs.iter().any(|output| {
        *media
            == MediaAsset {
                id: output.media_id.clone(),
                name: None,
                relative_path: output.relative_path.clone(),
                kind: MediaKind::Generated,
                duration_seconds: output.duration_seconds,
                width: Some(output.width),
                height: Some(output.height),
                fps: (output.fps > 0.0).then_some(output.fps),
                folder_id: asset.target_folder_id.clone(),
            }
    })
}

/// Stable content identity for Undo conflict detection
/// (`AGENT_CONTENT_HASH_VERSION`): the hash of `agent_undo_comparable_content`.
pub fn agent_project_content_hash(
    project: &VideoProject,
    batch_generated_asset_ids: &[String],
) -> Result<String, SplitProjectError> {
    sha256_json(&agent_undo_comparable_content(
        project,
        batch_generated_asset_ids,
    ))
}

/// The version 4 content identity: `agent_project_content_hash` that also
/// ignores the progress of the background generations `background_ids`.
pub(super) fn agent_project_content_hash_with_background(
    project: &VideoProject,
    batch_generated_asset_ids: &[String],
    background_generated_asset_ids: &[String],
) -> Result<String, SplitProjectError> {
    let mut content = agent_undo_comparable_content(project, batch_generated_asset_ids);
    background_comparable_content(&mut content, project, background_generated_asset_ids);
    sha256_json(&content)
}

/// The unversioned rule written by the first batch entries: job bookkeeping
/// was still hashed.
fn unversioned_agent_project_content_hash(
    project: &VideoProject,
) -> Result<String, SplitProjectError> {
    let mut content = project.clone();
    content.content_revision = 0;
    content.codex_thread_id = None;
    content.updated_at = String::new();
    sha256_json(&content)
}

fn sha256_json(project: &VideoProject) -> Result<String, SplitProjectError> {
    let bytes = serde_json::to_vec(project).map_err(|error| SplitProjectError::Json {
        path: "agent project content hash".to_string(),
        message: error.to_string(),
    })?;
    let hex = Sha256::digest(&bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();
    Ok(format!("sha256:{hex}"))
}

/// Whether `current` still holds what `entry` committed. Version 4 hashes also
/// ignore the progress of the generations the batch left alone; version 3
/// hashes ignore the background progress of the batch's own generations; version 2
/// hashes and legacy full snapshots only ignore job bookkeeping; unversioned
/// hashes can only be checked with the rule they were written with.
pub(super) fn entry_matches_project(
    entry: &SplitAgentEditHistoryEntry,
    current: &VideoProject,
) -> Result<bool, SplitProjectError> {
    if let Some(hash) = &entry.after_content_hash {
        let current_hash = match entry.after_content_hash_version {
            None => unversioned_agent_project_content_hash(current)?,
            Some(GENERATION_UNSCOPED_HASH_VERSION) => agent_project_content_hash(current, &[])?,
            Some(AGENT_CONTENT_HASH_VERSION) => agent_project_content_hash_with_background(
                current,
                &entry.added_generated_asset_ids,
                &entry.background_generated_asset_ids,
            )?,
            Some(GENERATION_SCOPED_HASH_VERSION) => {
                agent_project_content_hash(current, &entry.added_generated_asset_ids)?
            }
            Some(_) => return Ok(false),
        };
        return Ok(*hash == current_hash);
    }
    let Some(after) = &entry.after else {
        return Ok(false);
    };
    Ok(agent_undo_comparable_content(&after.project()?, &[])
        == agent_undo_comparable_content(current, &[]))
}

/// The project Undo writes for a matching `entry`: the pre-apply snapshot with
/// the current Codex thread and job bookkeeping, minus the bookkeeping records
/// the batch added. Records the batch only updated keep their current state.
/// The batch's generated assets and the output media completion added for
/// them are not in the snapshot, so they are removed. Version 4 entries keep
/// the current progress of the generations the batch left alone.
pub(super) fn restored_agent_snapshot(
    entry: &SplitAgentEditHistoryEntry,
    current: &VideoProject,
) -> Result<VideoProject, SplitProjectError> {
    let mut restored = entry.before.project()?;
    let added = if entry.after_content_hash_version.is_some() {
        entry.added_bookkeeping_ids.clone()
    } else {
        // Unversioned hash and full-snapshot entries: the match guarantees the
        // bookkeeping the batch committed is still present (in `after`, or in
        // `current` for an unversioned hash, which hashed bookkeeping too).
        let after = entry
            .after
            .as_ref()
            .map(|after| after.project())
            .transpose()?;
        SplitAgentBookkeepingIds::added_between(&restored, after.as_ref().unwrap_or(current))
    };
    restored.codex_thread_id = current.codex_thread_id.clone();
    restored.jobs = kept(&current.jobs, &added.jobs, |job| &job.id);
    restored.render_reports = kept(&current.render_reports, &added.render_reports, |report| {
        &report.id
    });
    restored.export_artifacts = kept(
        &current.export_artifacts,
        &added.export_artifacts,
        |artifact| &artifact.id,
    );
    if entry.after_content_hash_version == Some(AGENT_CONTENT_HASH_VERSION) {
        carry_background_progress(
            &mut restored,
            current,
            &entry.background_generated_asset_ids,
        );
    }
    Ok(restored)
}

fn kept<T: Clone>(records: &[T], removed: &[String], id: fn(&T) -> &str) -> Vec<T> {
    records
        .iter()
        .filter(|record| !removed.iter().any(|removed| removed == id(record)))
        .cloned()
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::project::fixtures::sample_project;
    use crate::project::model::{
        GeneratedAsset, GeneratedAssetStatus, GenerationModel, JobStatus, JobSummary, MediaKind,
    };

    fn job(id: &str) -> JobSummary {
        JobSummary {
            id: id.to_string(),
            kind: "captureCanonicalPreviewFrame".to_string(),
            status: JobStatus::Completed,
            updated_at: "2026-09-15T10:00:00Z".to_string(),
            workflow: None,
            start_request: None,
            provider_request: None,
            failure_reason: None,
            export_settings: None,
        }
    }

    #[test]
    fn content_hash_ignores_revision_thread_and_job_bookkeeping_only() {
        let project = sample_project();
        let mut bookkeeping = project.clone();
        bookkeeping.content_revision = 9;
        bookkeeping.codex_thread_id = Some("thread-9".to_string());
        bookkeeping.updated_at = "2027-01-01T00:00:00Z".to_string();
        bookkeeping.jobs.push(job("capture-1"));
        bookkeeping.render_reports.clear();
        bookkeeping.export_artifacts.clear();
        let mut edited = project.clone();
        edited.name = "Edited".to_string();
        let mut generated = project.clone();
        generated.generated_assets.push(GeneratedAsset {
            schema_version: 1,
            id: "generated-1".to_string(),
            kind: MediaKind::Generated,
            status: GeneratedAssetStatus::Queued,
            name: None,
            target_folder_id: None,
            placement_intent: None,
            prompt: "A sunrise".to_string(),
            model: GenerationModel {
                provider: "mock".to_string(),
                id: "mock-image".to_string(),
            },
            references: Default::default(),
            settings: Default::default(),
            outputs: Vec::new(),
            created_at: "2026-09-15T10:00:00Z".to_string(),
            parent_asset_id: None,
            retry_of_asset_id: None,
        });
        let mut generated_status = generated.clone();
        generated_status.generated_assets[0].status = GeneratedAssetStatus::Running;

        let hash = agent_project_content_hash(&project, &[]).expect("hash");
        assert_eq!(
            hash,
            agent_project_content_hash(&bookkeeping, &[]).expect("hash")
        );
        assert_ne!(
            hash,
            agent_project_content_hash(&edited, &[]).expect("hash")
        );
        let generated_hash = agent_project_content_hash(&generated, &[]).expect("hash");
        assert_ne!(hash, generated_hash);
        assert_ne!(
            generated_hash,
            agent_project_content_hash(&generated_status, &[]).expect("hash")
        );
        let scope = ["generated-1".to_string()];
        assert_eq!(
            agent_project_content_hash(&generated, &scope).expect("hash"),
            agent_project_content_hash(&generated_status, &scope).expect("hash"),
            "a batch's own generation status is background progress"
        );
    }

    #[test]
    fn unversioned_hash_still_covers_job_bookkeeping() {
        let project = sample_project();
        let mut bookkeeping = project.clone();
        bookkeeping.jobs.push(job("capture-1"));

        assert_ne!(
            unversioned_agent_project_content_hash(&project).expect("hash"),
            unversioned_agent_project_content_hash(&bookkeeping).expect("hash")
        );
    }

    #[test]
    fn added_bookkeeping_ids_list_only_new_records() {
        let mut before = sample_project();
        before.jobs = vec![job("existing")];
        let mut after = before.clone();
        after.jobs.push(job("added"));

        let added = SplitAgentBookkeepingIds::added_between(&before, &after);

        assert_eq!(added.jobs, vec!["added"]);
        assert!(added.render_reports.is_empty() && added.export_artifacts.is_empty());
    }
}
