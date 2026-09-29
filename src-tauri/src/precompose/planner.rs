use crate::project::model::{
    MediaKind, TimelineItem, TimelineItemKind, TimelineSource, TrackKind, VideoProject,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PreparationScope {
    PreparedSource,
    FlattenedComposite,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PreparationReason {
    /// A reversed video clip plays a reversed intermediate (`precompose::reverse`).
    Reverse,
    Lottie,
    Lut,
    RichBlend,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PreparationTask {
    pub track_index: usize,
    pub item_index: usize,
    pub item_id: String,
    pub media_id: String,
    pub scope: PreparationScope,
    pub reasons: Vec<PreparationReason>,
}

pub fn plan_precomposition(project: &VideoProject) -> Vec<PreparationTask> {
    let mut tasks = Vec::new();
    for (track_index, track) in project.timeline.tracks.iter().enumerate() {
        if !track.enabled || track.kind != TrackKind::Video {
            continue;
        }
        for (item_index, item) in track.items.iter().enumerate() {
            let TimelineSource::Media { media_id } = &item.source else {
                continue;
            };
            let media_kind = project
                .media
                .iter()
                .find(|media| media.id == *media_id)
                .map(|media| &media.kind);
            let (scope, reasons) = classify_item(item, media_kind);
            if reasons.is_empty() {
                continue;
            }
            tasks.push(PreparationTask {
                track_index,
                item_index,
                item_id: item.id.clone(),
                media_id: media_id.clone(),
                scope,
                reasons,
            });
        }
    }
    tasks
}

fn classify_item(
    item: &TimelineItem,
    media_kind: Option<&MediaKind>,
) -> (PreparationScope, Vec<PreparationReason>) {
    let mut reasons = Vec::new();
    // Reverse first: later reasons prepare the reversed intermediate.
    if item.kind == TimelineItemKind::VideoClip
        && media_kind == Some(&MediaKind::Video)
        && crate::project::reverse::is_reversed(item)
    {
        reasons.push(PreparationReason::Reverse);
    }
    if media_kind == Some(&MediaKind::Lottie) {
        reasons.push(PreparationReason::Lottie);
    }
    if item
        .properties
        .get("colorGrade")
        .and_then(serde_json::Value::as_object)
        .and_then(|grade| grade.get("lut"))
        .is_some_and(|lut| !lut.is_null())
        || item
            .properties
            .get("effects")
            .and_then(serde_json::Value::as_array)
            .is_some_and(|effects| {
                effects.iter().any(|effect| {
                    effect.get("effectType").and_then(serde_json::Value::as_str)
                        == Some("color.lut")
                        && effect.get("enabled").and_then(serde_json::Value::as_bool) != Some(false)
                })
            })
    {
        reasons.push(PreparationReason::Lut);
    }
    let rich_blend = item
        .properties
        .get("blendMode")
        .and_then(serde_json::Value::as_str)
        .is_some_and(|blend| !matches!(blend, "over" | "normal"));
    if rich_blend {
        reasons.push(PreparationReason::RichBlend);
    }
    let scope = if rich_blend {
        PreparationScope::FlattenedComposite
    } else {
        PreparationScope::PreparedSource
    };
    (scope, reasons)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::collections::BTreeMap;

    fn item(properties: BTreeMap<String, serde_json::Value>) -> TimelineItem {
        TimelineItem {
            id: "item-1".to_string(),
            kind: TimelineItemKind::VideoClip,
            start_seconds: 0.0,
            duration_seconds: 1.0,
            source: TimelineSource::Media {
                media_id: "media-1".to_string(),
            },
            label: "Visual".to_string(),
            properties,
        }
    }

    #[test]
    fn routes_lottie_and_lut_to_prepared_source() {
        let mut properties = BTreeMap::new();
        properties.insert(
            "colorGrade".to_string(),
            json!({ "lut": { "path": "look.cube" } }),
        );
        assert_eq!(
            classify_item(&item(properties), Some(&MediaKind::Lottie)),
            (
                PreparationScope::PreparedSource,
                vec![PreparationReason::Lottie, PreparationReason::Lut]
            )
        );
    }

    #[test]
    fn routes_catalog_lut_effect_to_prepared_source() {
        let properties = BTreeMap::from([(
            "effects".to_string(),
            json!([{
                "effectInstanceId":"lut-a",
                "effectType":"color.lut",
                "enabled":true,
                "params":{"path":"looks/invert.cube","intensity":0.5}
            }]),
        )]);
        assert_eq!(
            classify_item(&item(properties), Some(&MediaKind::Video)),
            (
                PreparationScope::PreparedSource,
                vec![PreparationReason::Lut]
            )
        );
    }

    #[test]
    fn routes_reversed_video_clips_to_reverse_before_other_reasons() {
        let properties = BTreeMap::from([
            ("reverse".to_string(), json!(true)),
            (
                "colorGrade".to_string(),
                json!({ "lut": { "path": "look.cube" } }),
            ),
        ]);
        assert_eq!(
            classify_item(&item(properties.clone()), Some(&MediaKind::Video)),
            (
                PreparationScope::PreparedSource,
                vec![PreparationReason::Reverse, PreparationReason::Lut]
            )
        );
        // Only video media is reversed.
        assert_eq!(
            classify_item(&item(properties), Some(&MediaKind::Image)).1,
            vec![PreparationReason::Lut]
        );
    }

    #[test]
    fn routes_destination_dependent_blend_to_flattened_composite() {
        let mut properties = BTreeMap::new();
        properties.insert("blendMode".to_string(), json!("multiply"));
        assert_eq!(
            classify_item(&item(properties), Some(&MediaKind::Video)),
            (
                PreparationScope::FlattenedComposite,
                vec![PreparationReason::RichBlend]
            )
        );
    }
}
