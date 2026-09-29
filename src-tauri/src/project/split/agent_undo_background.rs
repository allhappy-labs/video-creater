//! Rule version 4 of agent batch Undo: the progress of generations a batch didn't touch.
//!
//! A generation recorded before a batch keeps running while later batches apply. Its
//! progress (status, outputs, provider input URLs, created-at, and the library media
//! completion adds for its outputs while no timeline clip uses them) is bookkeeping for that
//! generation, not an edit, so it doesn't block Undo of the later batch, and Undo keeps the
//! current progress instead of rewinding it. Placing its output on a timeline is an edit and
//! still conflicts. A generation whose progress the batch itself changed is not background:
//! Undo restores it.

use super::agent_undo_content::completion_added;
use crate::project::model::{GeneratedAsset, GeneratedAssetStatus, TimelineSource, VideoProject};

/// Generated assets present in both `before` and `after` of a batch whose progress fields are
/// equal in both: their later progress is background to the batch.
pub(super) fn background_generated_asset_ids(
    before: &VideoProject,
    after: &VideoProject,
) -> Vec<String> {
    after
        .generated_assets
        .iter()
        .filter(|asset| {
            before
                .generated_assets
                .iter()
                .find(|previous| previous.id == asset.id)
                .is_some_and(|previous| same_progress(previous, asset))
        })
        .map(|asset| asset.id.clone())
        .collect()
}

fn same_progress(left: &GeneratedAsset, right: &GeneratedAsset) -> bool {
    left.status == right.status
        && left.outputs == right.outputs
        && left.references.provider_input_urls == right.references.provider_input_urls
        && left.created_at == right.created_at
}

/// Clears the progress of the background assets `ids` in `content` (compared content derived
/// from `source`): their progress fields, and the library media completion added for their
/// outputs in `source` that no timeline clip of `source` uses.
pub(super) fn background_comparable_content(
    content: &mut VideoProject,
    source: &VideoProject,
    ids: &[String],
) {
    if ids.is_empty() {
        return;
    }
    let unused = unused_completion_media_ids(source, ids);
    content
        .media
        .retain(|media| !unused.contains(&media.id.as_str()));
    for asset in &mut content.generated_assets {
        if ids.contains(&asset.id) {
            asset.status = GeneratedAssetStatus::Queued;
            asset.outputs = Vec::new();
            asset.references.provider_input_urls = Vec::new();
            asset.created_at = String::new();
        }
    }
}

/// Copies the current progress of the background assets `ids` into the restored snapshot: drops
/// the snapshot's unused completion media that `current` no longer has, and appends the unused
/// completion media of `current` that the snapshot lacks, in current order.
pub(super) fn carry_background_progress(
    restored: &mut VideoProject,
    current: &VideoProject,
    ids: &[String],
) {
    if ids.is_empty() {
        return;
    }
    let removed = unused_completion_media_ids(restored, ids)
        .into_iter()
        .filter(|id| !current.media.iter().any(|media| media.id == *id))
        .map(str::to_string)
        .collect::<Vec<_>>();
    restored.media.retain(|media| !removed.contains(&media.id));
    for asset in &mut restored.generated_assets {
        if !ids.contains(&asset.id) {
            continue;
        }
        let Some(progress) = current
            .generated_assets
            .iter()
            .find(|candidate| candidate.id == asset.id)
        else {
            continue;
        };
        asset.status = progress.status.clone();
        asset.outputs = progress.outputs.clone();
        asset.references.provider_input_urls = progress.references.provider_input_urls.clone();
        asset.created_at = progress.created_at.clone();
    }
    let unused = unused_completion_media_ids(current, ids);
    for media in &current.media {
        if unused.contains(&media.id.as_str())
            && !restored
                .media
                .iter()
                .any(|existing| existing.id == media.id)
        {
            restored.media.push(media.clone());
        }
    }
}

/// Library media of `project` that completion added for an output of an asset in `ids` and
/// that no clip on any timeline uses.
fn unused_completion_media_ids<'a>(project: &'a VideoProject, ids: &[String]) -> Vec<&'a str> {
    let assets = project
        .generated_assets
        .iter()
        .filter(|asset| ids.contains(&asset.id))
        .collect::<Vec<_>>();
    let used = |media_id: &str| {
        std::iter::once(&project.timeline)
            .chain(project.timelines.iter().map(|entry| &entry.timeline))
            .flat_map(|timeline| timeline.tracks.iter())
            .flat_map(|track| track.items.iter())
            .any(|item| {
                matches!(&item.source, TimelineSource::Media { media_id: used } if used == media_id)
            })
    };
    project
        .media
        .iter()
        .filter(|media| {
            assets.iter().any(|asset| completion_added(asset, media)) && !used(&media.id)
        })
        .map(|media| media.id.as_str())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::project::fixtures::sample_project;
    use crate::project::model::{
        GeneratedAssetOutput, GenerationModel, MediaAsset, MediaKind, TimelineItem,
        TimelineItemKind,
    };
    use std::collections::BTreeMap;

    fn asset(id: &str, status: GeneratedAssetStatus) -> GeneratedAsset {
        GeneratedAsset {
            schema_version: 1,
            id: id.to_string(),
            kind: MediaKind::Generated,
            status,
            name: None,
            target_folder_id: None,
            placement_intent: None,
            prompt: "A sunrise".to_string(),
            model: GenerationModel {
                provider: "fal.ai".to_string(),
                id: "fal-ai/test".to_string(),
            },
            references: Default::default(),
            settings: Default::default(),
            outputs: Vec::new(),
            created_at: "2026-09-16T10:00:00Z".to_string(),
            parent_asset_id: None,
            retry_of_asset_id: None,
        }
    }

    fn output() -> GeneratedAssetOutput {
        GeneratedAssetOutput {
            media_id: "sunrise-output".to_string(),
            relative_path: "generated/sunrise/output.mp4".to_string(),
            source_url: None,
            width: 1280,
            height: 720,
            duration_seconds: 4.0,
            fps: 24.0,
        }
    }

    /// The asset completed, with the library media completion adds for its output.
    fn completed(mut project: VideoProject) -> VideoProject {
        let generation = project
            .generated_assets
            .iter_mut()
            .find(|asset| asset.id == "sunrise")
            .expect("sunrise");
        generation.status = GeneratedAssetStatus::Completed;
        generation.outputs = vec![output()];
        project.media.push(MediaAsset {
            id: "sunrise-output".to_string(),
            name: None,
            relative_path: "generated/sunrise/output.mp4".to_string(),
            kind: MediaKind::Generated,
            duration_seconds: 4.0,
            width: Some(1280),
            height: Some(720),
            fps: Some(24.0),
            folder_id: None,
        });
        project
    }

    fn with_running_generation() -> VideoProject {
        let mut project = sample_project();
        project
            .generated_assets
            .push(asset("sunrise", GeneratedAssetStatus::Running));
        project
            .generated_assets
            .push(asset("sunset", GeneratedAssetStatus::Queued));
        project
    }

    #[test]
    fn background_assets_are_the_ones_whose_progress_the_batch_left_alone() {
        let before = with_running_generation();
        let mut after = before.clone();
        after.generated_assets[1].status = GeneratedAssetStatus::Cancelled;
        after
            .generated_assets
            .push(asset("new", GeneratedAssetStatus::Queued));

        assert_eq!(
            background_generated_asset_ids(&before, &after),
            vec!["sunrise".to_string()]
        );
    }

    #[test]
    fn comparable_content_clears_progress_and_unused_completion_media() {
        let running = with_running_generation();
        let done = completed(running.clone());
        let ids = ["sunrise".to_string()];

        let mut running_content = running.clone();
        background_comparable_content(&mut running_content, &running, &ids);
        let mut done_content = done.clone();
        background_comparable_content(&mut done_content, &done, &ids);
        assert_eq!(running_content, done_content);

        let mut placed = done.clone();
        placed.timeline.tracks[0].items.push(TimelineItem {
            id: "sunrise-clip".to_string(),
            kind: TimelineItemKind::VideoClip,
            start_seconds: 20.0,
            duration_seconds: 4.0,
            source: TimelineSource::Media {
                media_id: "sunrise-output".to_string(),
            },
            label: "Sunrise".to_string(),
            properties: BTreeMap::new(),
        });
        let mut placed_content = placed.clone();
        background_comparable_content(&mut placed_content, &placed, &ids);
        assert!(placed_content
            .media
            .iter()
            .any(|media| media.id == "sunrise-output"));
    }

    #[test]
    fn carrying_progress_keeps_the_current_status_outputs_and_unused_media() {
        let mut restored = with_running_generation();
        restored.name = "Before the batch".to_string();
        let current = completed(with_running_generation());

        carry_background_progress(&mut restored, &current, &["sunrise".to_string()]);

        assert_eq!(restored.name, "Before the batch");
        assert_eq!(
            restored.generated_assets[0].status,
            GeneratedAssetStatus::Completed
        );
        assert_eq!(restored.generated_assets[0].outputs, vec![output()]);
        assert_eq!(
            restored.generated_assets[1].status,
            GeneratedAssetStatus::Queued
        );
        assert_eq!(
            restored.media.last().map(|media| media.id.as_str()),
            Some("sunrise-output")
        );
    }

    #[test]
    fn carrying_progress_keeps_a_later_removal_of_unused_completion_media() {
        let mut restored = completed(with_running_generation());
        let mut current = restored.clone();
        current.media.retain(|media| media.id != "sunrise-output");
        let ids = ["sunrise".to_string()];

        carry_background_progress(&mut restored, &current, &ids);

        assert!(!restored
            .media
            .iter()
            .any(|media| media.id == "sunrise-output"));
        assert_eq!(restored.media, current.media);
    }
}
