use super::{MAX_CONTEXT_FOLDERS, MAX_CONTEXT_MEDIA};
use crate::project::model::{
    GeneratedAssetStatus, MediaAsset, MediaFolder, MediaKind, TranscriptWord, VideoProject,
};
use std::collections::{BTreeMap, BTreeSet};

pub(super) fn media_library_summary(project: &VideoProject) -> String {
    media_library_summary_for(
        project,
        &canonical_prefix(&project.media_folders, MAX_CONTEXT_FOLDERS),
        &canonical_prefix(&project.media, MAX_CONTEXT_MEDIA),
    )
}

pub(super) fn media_library_summary_for(
    project: &VideoProject,
    folders: &[&MediaFolder],
    media_assets: &[&MediaAsset],
) -> String {
    let folder_paths = media_folder_paths(&project.media_folders);
    let mut lines = vec![
        "Folders:".to_string(),
        "Use folder ids, not display paths, in projectActions.".to_string(),
    ];

    if project.media_folders.is_empty() {
        lines.push("- none".to_string());
    } else {
        for folder in folders {
            let path = folder_paths
                .get(&folder.id)
                .cloned()
                .unwrap_or_else(|| folder.name.clone());
            lines.push(format!("- folder {}: {}", folder.id, path));
        }
        push_truncation_marker(
            &mut lines,
            project.media_folders.len(),
            folders.len(),
            "folders",
        );
    }

    lines.push("Media assets:".to_string());
    if project.media.is_empty() {
        lines.push("- none".to_string());
    } else {
        for media in media_assets {
            let folder = media
                .folder_id
                .as_ref()
                .map(|folder_id| {
                    folder_paths
                        .get(folder_id)
                        .cloned()
                        .unwrap_or_else(|| format!("unknown folder {folder_id}"))
                })
                .unwrap_or_else(|| "Unfiled".to_string());
            lines.push(format!(
                "- media {}: {} | {} | {:.3}s | folder: {}",
                media.id,
                media.relative_path,
                media_kind_code(&media.kind),
                media.duration_seconds,
                folder
            ));
        }
        push_truncation_marker(
            &mut lines,
            project.media.len(),
            media_assets.len(),
            "media assets",
        );
    }

    lines.join("\n")
}

fn media_folder_paths(folders: &[MediaFolder]) -> BTreeMap<String, String> {
    let folders_by_id = folders
        .iter()
        .map(|folder| (folder.id.clone(), folder))
        .collect::<BTreeMap<_, _>>();
    let mut paths = BTreeMap::new();
    for folder in folders {
        let mut visiting = BTreeSet::new();
        let path = media_folder_path(folder, &folders_by_id, &mut paths, &mut visiting);
        paths.insert(folder.id.clone(), path);
    }
    paths
}

fn media_folder_path(
    folder: &MediaFolder,
    folders_by_id: &BTreeMap<String, &MediaFolder>,
    paths: &mut BTreeMap<String, String>,
    visiting: &mut BTreeSet<String>,
) -> String {
    if let Some(path) = paths.get(&folder.id) {
        return path.clone();
    }
    if !visiting.insert(folder.id.clone()) {
        return folder.name.clone();
    }

    let path = folder
        .parent_id
        .as_ref()
        .and_then(|parent_id| folders_by_id.get(parent_id))
        .map(|parent| {
            let parent_path = media_folder_path(parent, folders_by_id, paths, visiting);
            format!("{parent_path} / {}", folder.name)
        })
        .unwrap_or_else(|| folder.name.clone());

    visiting.remove(&folder.id);
    paths.insert(folder.id.clone(), path.clone());
    path
}

pub(super) fn media_kind_code(kind: &MediaKind) -> &'static str {
    match kind {
        MediaKind::Video => "video",
        MediaKind::Audio => "audio",
        MediaKind::Image => "image",
        MediaKind::Lottie => "lottie",
        MediaKind::Generated => "generated",
    }
}

pub(super) fn generated_asset_status_code(status: &GeneratedAssetStatus) -> &'static str {
    match status {
        GeneratedAssetStatus::Queued => "queued",
        GeneratedAssetStatus::Running => "running",
        GeneratedAssetStatus::Cancelled => "cancelled",
        GeneratedAssetStatus::Failed => "failed",
        GeneratedAssetStatus::Completed => "completed",
    }
}

pub(super) fn canonical_prefix<T>(entries: &[T], cap: usize) -> Vec<&T> {
    entries.iter().take(cap).collect()
}

pub(super) fn timeline_item_count(project: &VideoProject) -> usize {
    project
        .timeline
        .tracks
        .iter()
        .map(|track| track.items.len())
        .sum()
}

pub(super) fn push_truncation_marker(
    lines: &mut Vec<String>,
    total: usize,
    shown: usize,
    noun: &str,
) {
    if total > shown {
        lines.push(format!("- [{} {noun} truncated]", total - shown));
    }
}

pub(super) fn transcript_excerpt(media: &MediaAsset, words: &[TranscriptWord]) -> String {
    let mut excerpt = words
        .iter()
        .take(80)
        .map(|word| {
            format!(
                "[{start:.2}-{end:.2}] {text}",
                start = word.start_seconds,
                end = word.end_seconds,
                text = word.text
            )
        })
        .collect::<Vec<_>>()
        .join("\n");

    if words.len() > 80 {
        excerpt.push_str("\n[transcript truncated]");
    }
    if excerpt.is_empty() {
        excerpt = format!(
            "[no words available for media duration {:.2}s]",
            media.duration_seconds
        );
    }
    excerpt
}
