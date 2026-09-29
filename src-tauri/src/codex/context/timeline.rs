use super::media::{push_truncation_marker, timeline_item_count};
use super::{MAX_CONTEXT_TIMELINE_ITEMS, MAX_CONTEXT_TRACKS};
use crate::project::model::{
    TimelineItem, TimelineItemKind, TimelineSource, TimelineTrack, TrackKind, TransitionKind,
    VideoProject,
};

/// Tracks to render, each with the items selected from it.
pub(super) type TimelineSelection<'a> = Vec<(&'a TimelineTrack, Vec<&'a TimelineItem>)>;

pub(super) fn timeline_summary(project: &VideoProject) -> String {
    timeline_summary_for(project, &canonical_timeline_selection(project))
}

pub(super) fn canonical_timeline_selection(project: &VideoProject) -> TimelineSelection<'_> {
    let mut remaining = MAX_CONTEXT_TIMELINE_ITEMS;
    project
        .timeline
        .tracks
        .iter()
        .take(MAX_CONTEXT_TRACKS)
        .map(|track| {
            let items = track.items.iter().take(remaining).collect::<Vec<_>>();
            remaining -= items.len();
            (track, items)
        })
        .collect()
}

pub(super) fn timeline_summary_for(
    project: &VideoProject,
    selection: &TimelineSelection<'_>,
) -> String {
    let mut lines = vec![
        format!(
            "- durationSeconds: {:.3}",
            project.timeline.duration_seconds
        ),
        "Use timeline item ids and track ids when returning trimItems, splitItems, reorderItems, moveItems, resizeItems, insertItems, or removeItems projectActions.".to_string(),
        "Use track ids for createTrack, setTrackLocked, and setTrackEnabled; use audio item ids for updateAudioFades, updateAudioFadeOut, and updateAudioVolume; use visual item ids for updateVisualClipOpacity, updateVisualClipTransform, and updateVisualClipCrop.".to_string(),
        "Use a track id plus leftItemId and rightItemId of two adjacent clips on it for addTransition; the transition is centered on their cut and needs durationSeconds / 2 of unused source media on each side. Use transition ids for updateTransition and removeTransition.".to_string(),
    ];
    let mut item_count = 0usize;

    for (track, items) in selection {
        lines.push(format!(
            "- track {}: {} | {} | {} | {} | items: {}",
            track.id,
            track.name,
            track_kind_code(&track.kind),
            if track.locked { "locked" } else { "unlocked" },
            if track.enabled { "enabled" } else { "disabled" },
            track.items.len()
        ));

        for item in items {
            lines.push(format!("- item {}", timeline_item_summary(item)));
            item_count += 1;
        }
        // Only transitions between listed items, so truncation stays bounded.
        let listed = |item_id: &str| items.iter().any(|item| item.id == item_id);
        for transition in track.transitions.iter().filter(|transition| {
            listed(&transition.left_item_id) && listed(&transition.right_item_id)
        }) {
            lines.push(format!(
                "- transition {}: {} | {} -> {} | {:.3}s",
                transition.id,
                transition_kind_code(transition.kind),
                transition.left_item_id,
                transition.right_item_id,
                transition.duration_seconds
            ));
        }
    }

    push_truncation_marker(
        &mut lines,
        project.timeline.tracks.len(),
        selection.len(),
        "tracks",
    );
    push_truncation_marker(
        &mut lines,
        timeline_item_count(project),
        item_count,
        "timeline items",
    );

    lines.join("\n")
}

fn transition_kind_code(kind: TransitionKind) -> &'static str {
    match kind {
        TransitionKind::Crossfade => "crossfade",
        TransitionKind::DipToBlack => "dipToBlack",
        TransitionKind::DipToWhite => "dipToWhite",
        TransitionKind::Wipe => "wipe",
    }
}

fn timeline_item_summary(item: &TimelineItem) -> String {
    let end_seconds = item.start_seconds + item.duration_seconds;
    let mut parts = vec![
        format!("{}: {}", item.id, timeline_item_kind_code(&item.kind)),
        format!("{:.3}-{:.3}s", item.start_seconds, end_seconds),
        format!("source: {}", timeline_source_label(&item.source)),
        format!("label: {}", item.label),
    ];

    for key in [
        "sourceIn",
        "sourceOut",
        "reason",
        "transcriptId",
        "wordIndex",
        "text",
        "visualTreatment",
        "motion",
        "safeZone",
        "avoid",
        "textEdited",
        "fadeInSeconds",
        "fadeOutSeconds",
        "volumeDb",
        "templateId",
        "motionPresetId",
        "generatedAssetId",
        "generatedOutputMediaId",
    ] {
        if let Some(value) = item.properties.get(key) {
            parts.push(format!("{key}: {}", context_property_label(value)));
        }
    }

    parts.join(" | ")
}

fn timeline_source_label(source: &TimelineSource) -> String {
    match source {
        TimelineSource::Media { media_id } => format!("media {media_id}"),
        TimelineSource::Generated { artifact_id } => format!("generated {artifact_id}"),
        TimelineSource::Timeline { timeline_id } => format!("timeline {timeline_id}"),
        TimelineSource::Text { text } => format!("text \"{}\"", compact_context_text(text)),
    }
}

pub(super) fn context_property_label(value: &serde_json::Value) -> String {
    match value {
        serde_json::Value::Number(number) => number
            .as_f64()
            .map(|value| format!("{value:.3}"))
            .unwrap_or_else(|| number.to_string()),
        serde_json::Value::String(value) => compact_context_text(value),
        serde_json::Value::Bool(value) => value.to_string(),
        serde_json::Value::Null => "null".to_string(),
        other => other.to_string(),
    }
}

pub(super) fn compact_context_text(value: &str) -> String {
    const MAX_CONTEXT_TEXT_CHARS: usize = 80;
    let compact = value.split_whitespace().collect::<Vec<_>>().join(" ");
    if compact.chars().count() <= MAX_CONTEXT_TEXT_CHARS {
        return compact;
    }

    let truncated = compact
        .chars()
        .take(MAX_CONTEXT_TEXT_CHARS.saturating_sub(3))
        .collect::<String>();
    format!("{truncated}...")
}

fn track_kind_code(kind: &TrackKind) -> &'static str {
    match kind {
        TrackKind::Video => "video",
        TrackKind::HyperframeScene => "hyperframe_scene",
        TrackKind::Overlay => "overlay",
        TrackKind::Caption => "caption",
        TrackKind::Audio => "audio",
    }
}

fn timeline_item_kind_code(kind: &TimelineItemKind) -> &'static str {
    match kind {
        TimelineItemKind::VideoClip => "video_clip",
        TimelineItemKind::ImageClip => "image_clip",
        TimelineItemKind::LottieClip => "lottie_clip",
        TimelineItemKind::GeneratedClip => "generated_clip",
        TimelineItemKind::HyperframeScene => "hyperframe_scene",
        TimelineItemKind::Overlay => "overlay",
        TimelineItemKind::Caption => "caption",
        TimelineItemKind::AudioClip => "audio_clip",
    }
}
