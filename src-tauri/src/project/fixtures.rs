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
        name: None,
        relative_path: "media/input.mp4".to_string(),
        kind: MediaKind::Video,
        duration_seconds: 12.0,
        width: Some(1920),
        height: Some(1080),
        fps: Some(24.0),
        folder_id: None,
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
