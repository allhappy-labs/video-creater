use super::captions::{
    build_caption_cues, CaptionGenerationError, CaptionGenerationOptions, CaptionSourceRange,
};
use super::edl::{
    build_multi_source_rough_cut_edl, build_rough_cut_edl_with_signals_and_silence,
    EdlCandidateRange, EdlError, MomentSignal, RoughCutEdl,
};
use super::preset::{EditJobRequest, EditRequestError};
use crate::project::action::{apply_project_action, ProjectAction, ProjectActionError};
use crate::project::model::{
    TimelineItem, TimelineItemKind, TimelineSource, TrackKind, TransitionKind, VideoProject,
};
use crate::search::semantic_visual::SemanticVisualHit;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::BTreeMap;
use thiserror::Error;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct GeneratedEditDraft {
    pub media_id: String,
    pub timeline_duration_seconds: f64,
    pub clip_count: usize,
    pub caption_count: usize,
    pub edl: RoughCutEdl,
}

#[derive(Debug, Error, PartialEq)]
pub enum GenerateEditError {
    #[error("edit request is invalid: {0}")]
    InvalidRequest(EditRequestError),
    #[error("media was not found: {0}")]
    MediaNotFound(String),
    #[error("transcript was not found for media: {0}")]
    TranscriptNotFound(String),
    #[error("timeline track was not found: {0:?}")]
    TrackNotFound(TrackKind),
    #[error("failed to build rough cut edl: {0}")]
    Edl(EdlError),
    #[error("failed to generate captions: {0}")]
    Captions(CaptionGenerationError),
    #[error("generated edit action failed validation: {0}")]
    ProjectAction(ProjectActionError),
}

pub fn generate_multi_source_edit_timeline(
    project: &mut VideoProject,
    request: EditJobRequest,
    candidates: &[EdlCandidateRange],
) -> Result<GeneratedEditDraft, GenerateEditError> {
    let edl =
        build_multi_source_rough_cut_edl(&request, candidates).map_err(GenerateEditError::Edl)?;
    let video_track_id = track_id(project, TrackKind::Video)?;
    let audio_track_id = track_id(project, TrackKind::Audio)?;
    let caption_track_id = track_id(project, TrackKind::Caption)?;
    let mut video_items = Vec::new();
    let mut audio_items = Vec::new();
    let mut caption_items = Vec::new();
    let mut output_cursor = 0.0;

    for (index, clip) in edl.clips.iter().enumerate() {
        let media = project
            .media
            .iter()
            .find(|media| media.id == clip.media_id)
            .cloned()
            .ok_or_else(|| GenerateEditError::MediaNotFound(clip.media_id.clone()))?;
        let transcript = project
            .transcripts
            .iter()
            .find(|transcript| transcript.media_id == clip.media_id)
            .cloned();
        let duration = clip.source_out - clip.source_in;
        let mut properties = BTreeMap::new();
        properties.insert("sourceIn".to_string(), json!(clip.source_in));
        properties.insert("sourceOut".to_string(), json!(clip.source_out));
        properties.insert("reason".to_string(), json!(clip.reason));
        properties.insert(
            "selectionReasons".to_string(),
            json!(clip.selection_reasons),
        );
        properties.insert("selectionScore".to_string(), json!(clip.score));
        properties.insert(
            "generatedBy".to_string(),
            json!("deterministic-multi-source-edl"),
        );

        let visual_kind = match media.kind {
            crate::project::model::MediaKind::Video => Some(TimelineItemKind::VideoClip),
            crate::project::model::MediaKind::Image => Some(TimelineItemKind::ImageClip),
            crate::project::model::MediaKind::Lottie => Some(TimelineItemKind::LottieClip),
            crate::project::model::MediaKind::Generated => Some(TimelineItemKind::GeneratedClip),
            crate::project::model::MediaKind::Audio => None,
        };
        if let Some(kind) = visual_kind {
            video_items.push(TimelineItem {
                id: format!("generated-multi-visual-{}", index + 1),
                kind,
                start_seconds: output_cursor,
                duration_seconds: duration,
                source: TimelineSource::Media {
                    media_id: clip.media_id.clone(),
                },
                label: format!("Generated visual {}", index + 1),
                properties: properties.clone(),
            });
        }
        if matches!(
            media.kind,
            crate::project::model::MediaKind::Video | crate::project::model::MediaKind::Audio
        ) {
            audio_items.push(TimelineItem {
                id: format!("generated-multi-audio-{}", index + 1),
                kind: TimelineItemKind::AudioClip,
                start_seconds: output_cursor,
                duration_seconds: duration,
                source: TimelineSource::Media {
                    media_id: clip.media_id.clone(),
                },
                label: format!("Generated audio {}", index + 1),
                properties: properties.clone(),
            });
        }

        let range = CaptionSourceRange {
            media_id: clip.media_id.clone(),
            source_in: clip.source_in,
            source_out: clip.source_out,
            output_start: output_cursor,
        };
        let cues = if let Some(transcript) = transcript {
            build_caption_cues(
                &transcript.words,
                &[range],
                CaptionGenerationOptions::default(),
            )
            .map_err(GenerateEditError::Captions)?
        } else {
            Vec::new()
        };
        for cue in cues {
            let cue_index = caption_items.len() + 1;
            let mut cue_properties = BTreeMap::new();
            cue_properties.insert("mediaId".to_string(), json!(cue.media_id));
            cue_properties.insert("sourceIn".to_string(), json!(cue.source_in));
            cue_properties.insert("sourceOut".to_string(), json!(cue.source_out));
            cue_properties.insert(
                "visualTreatment".to_string(),
                json!(cue.style.visual_treatment),
            );
            cue_properties.insert("motion".to_string(), json!(cue.style.motion));
            cue_properties.insert("safeZone".to_string(), json!(cue.style.safe_zone));
            cue_properties.insert("avoid".to_string(), json!(cue.style.avoid));
            cue_properties.insert(
                "generatedBy".to_string(),
                json!("deterministic-caption-stage"),
            );
            caption_items.push(TimelineItem {
                id: format!("generated-multi-caption-{cue_index}"),
                kind: TimelineItemKind::Caption,
                start_seconds: cue.start_seconds,
                duration_seconds: cue.duration_seconds,
                source: TimelineSource::Text { text: cue.text },
                label: format!("Caption {cue_index}"),
                properties: cue_properties,
            });
        }
        output_cursor += duration;
    }

    let mut actions = Vec::new();
    let existing_ids = project
        .timeline
        .tracks
        .iter()
        .filter(|track| {
            matches!(
                track.kind,
                TrackKind::Video | TrackKind::Audio | TrackKind::Caption
            )
        })
        .flat_map(|track| track.items.iter().map(|item| item.id.clone()))
        .collect::<Vec<_>>();
    if !existing_ids.is_empty() {
        actions.push(ProjectAction::RemoveItems {
            item_ids: existing_ids,
        });
    }
    if !video_items.is_empty() {
        actions.push(ProjectAction::AddItems {
            target_track_id: video_track_id,
            items: video_items,
        });
    }
    if !audio_items.is_empty() {
        actions.push(ProjectAction::AddItems {
            target_track_id: audio_track_id,
            items: audio_items,
        });
    }
    if !caption_items.is_empty() {
        actions.push(ProjectAction::AddItems {
            target_track_id: caption_track_id,
            items: caption_items.clone(),
        });
    }

    let mut validated = project.clone();
    for action in &actions {
        apply_project_action(&mut validated, action.clone())
            .map_err(GenerateEditError::ProjectAction)?;
    }
    for action in actions {
        apply_project_action(project, action).map_err(GenerateEditError::ProjectAction)?;
    }

    Ok(GeneratedEditDraft {
        media_id: request.media_id,
        timeline_duration_seconds: output_cursor,
        clip_count: edl.clips.len(),
        caption_count: caption_items.len(),
        edl,
    })
}

pub fn spoken_semantic_multi_source_candidates(
    project: &VideoProject,
    semantic_hits: &[SemanticVisualHit],
) -> Vec<EdlCandidateRange> {
    let mut candidates = Vec::new();
    for transcript in &project.transcripts {
        let Some(media) = project
            .media
            .iter()
            .find(|media| media.id == transcript.media_id)
        else {
            continue;
        };
        let spoken_ranges = if transcript.segments.is_empty() {
            transcript
                .words
                .chunks(6)
                .filter_map(|words| {
                    Some((
                        words.first()?.start_seconds,
                        words.last()?.end_seconds,
                        words
                            .iter()
                            .map(|word| word.text.as_str())
                            .collect::<Vec<_>>()
                            .join(" "),
                    ))
                })
                .collect::<Vec<_>>()
        } else {
            transcript
                .segments
                .iter()
                .map(|segment| {
                    (
                        segment.start_seconds,
                        segment.end_seconds,
                        segment.text.clone(),
                    )
                })
                .collect::<Vec<_>>()
        };
        for (range_start, range_end, text) in spoken_ranges {
            let source_in = range_start.max(0.0);
            let source_out = range_end.min(media.duration_seconds);
            if source_out <= source_in {
                continue;
            }
            candidates.push(EdlCandidateRange {
                media_id: media.id.clone(),
                source_duration_seconds: media.duration_seconds,
                source_in,
                source_out,
                hook_score: if source_in <= media.duration_seconds * 0.2 {
                    0.8
                } else {
                    0.35
                },
                payoff_score: if source_out >= media.duration_seconds * 0.65 {
                    0.75
                } else {
                    0.35
                },
                context_score: 0.65,
                quality_score: 0.8,
                spoken_score: 0.9,
                visual_score: 0.0,
                repetition_key: format!("spoken:{}:{source_in:.3}", media.id),
                reason: format!("spoken transcript: {}", text.trim()),
            });
        }
    }
    for hit in semantic_hits {
        let Some(media) = project.media.iter().find(|media| media.id == hit.media_id) else {
            continue;
        };
        let source_in = hit.shot_start_seconds.max(0.0);
        let source_out = hit.shot_end_seconds.min(media.duration_seconds);
        if source_out <= source_in {
            continue;
        }
        candidates.push(EdlCandidateRange {
            media_id: media.id.clone(),
            source_duration_seconds: media.duration_seconds,
            source_in,
            source_out,
            hook_score: if source_in <= media.duration_seconds * 0.2 {
                0.7
            } else {
                0.3
            },
            payoff_score: 0.55,
            context_score: 0.5,
            quality_score: hit.score.clamp(0.0, 1.0) as f64,
            spoken_score: 0.0,
            visual_score: hit.score.clamp(0.0, 1.0) as f64,
            repetition_key: format!("semantic:{}:{source_in:.3}", media.id),
            reason: format!(
                "local semantic visual match at {:.3}s ({:.3})",
                hit.time_seconds, hit.score
            ),
        });
    }
    candidates
}

pub fn generate_spoken_semantic_multi_source_edit_timeline(
    project: &mut VideoProject,
    request: EditJobRequest,
    semantic_hits: &[SemanticVisualHit],
) -> Result<GeneratedEditDraft, GenerateEditError> {
    let candidates = spoken_semantic_multi_source_candidates(project, semantic_hits);
    generate_multi_source_edit_timeline(project, request, &candidates)
}

fn track_id(project: &VideoProject, kind: TrackKind) -> Result<String, GenerateEditError> {
    project
        .timeline
        .tracks
        .iter()
        .find(|track| track.kind == kind)
        .map(|track| track.id.clone())
        .ok_or(GenerateEditError::TrackNotFound(kind))
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
/// Legacy combined quality/container profile kept for wire compatibility during migration.
#[derive(Default)]
pub enum RenderQualityProfile {
    #[default]
    DraftWebm,
    FinalWebm,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
#[derive(Default)]
pub enum RenderQuality {
    #[default]
    Draft,
    Final,
}

/// Encoder effort for a Final export. `Master` is the highest-quality encode of
/// the chosen format; it is not a separate `RenderQuality`, because quality also
/// drives AVFoundation profile selection and availability.
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum ExportEncodeTier {
    #[default]
    Standard,
    Master,
}

impl ExportEncodeTier {
    pub fn is_standard(&self) -> bool {
        *self == Self::Standard
    }
}

pub fn render_quality_from_legacy(profile: RenderQualityProfile) -> RenderQuality {
    match profile {
        RenderQualityProfile::DraftWebm => RenderQuality::Draft,
        RenderQualityProfile::FinalWebm => RenderQuality::Final,
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
#[derive(Default)]
pub enum RenderOutputProfile {
    #[default]
    WebPreview,
    WebDelivery,
    Mp4Primary,
    Mp4Modern,
    QuicktimeInterchange,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct RenderPlan {
    pub input_path: String,
    pub output_path: String,
    pub width: u32,
    pub height: u32,
    pub fps: f64,
    #[serde(default)]
    pub quality: RenderQuality,
    #[serde(default)]
    pub output_profile: RenderOutputProfile,
    #[serde(default, skip_serializing_if = "ExportEncodeTier::is_standard")]
    pub encode_tier: ExportEncodeTier,
    pub clips: Vec<RenderClip>,
    #[serde(default)]
    pub audio_clips: Vec<RenderClip>,
    /// Transitions between two overlapping entries of `clips`.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub transitions: Vec<RenderTransition>,
    /// Transitions between two overlapping entries of `audio_clips`.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub audio_transitions: Vec<RenderTransition>,
}

/// A clip-to-clip transition in render output time.
///
/// The render plan extends both clips into their unused source media so they
/// overlap across the transition window `[start_seconds, start_seconds +
/// duration_seconds]`, which is centered on the canonical cut (the right clip's
/// timeline start). Progress at output time `t` is
/// `(t - start_seconds) / duration_seconds`. Range renders keep the full
/// window, so `start_seconds` may be negative or the window may end after the
/// output when the range cuts through it.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct RenderTransition {
    pub kind: TransitionKind,
    pub start_seconds: f64,
    pub duration_seconds: f64,
    /// Index of the outgoing clip in the plan's clip list.
    pub left_clip_index: usize,
    /// Index of the incoming clip in the plan's clip list.
    pub right_clip_index: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct TemplateRenderLayer {
    pub item_id: String,
    pub template_id: String,
    pub label: String,
    pub timeline_start_seconds: f64,
    pub duration_seconds: f64,
    pub fields: BTreeMap<String, String>,
    pub visual_treatment: String,
    pub motion: String,
    pub safe_zone: String,
    pub avoid: String,
    pub preview_variant: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct RenderClip {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_path: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub timeline_start_seconds: Option<f64>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub properties: BTreeMap<String, Value>,
    #[serde(default)]
    pub timeline_track_index: u32,
    pub source_in: f64,
    pub source_out: f64,
}

#[derive(Debug, Error, PartialEq)]
pub enum RenderPlanError {
    #[error("template render layer is invalid: {0}")]
    InvalidTemplateLayer(&'static str),
}

pub fn generate_one_click_edit_timeline(
    project: &mut VideoProject,
    request: EditJobRequest,
) -> Result<GeneratedEditDraft, GenerateEditError> {
    generate_one_click_edit_timeline_with_signals(project, request, &[])
}

pub fn generate_one_click_edit_timeline_with_signals(
    project: &mut VideoProject,
    request: EditJobRequest,
    moment_signals: &[MomentSignal],
) -> Result<GeneratedEditDraft, GenerateEditError> {
    request
        .validate()
        .map_err(GenerateEditError::InvalidRequest)?;

    let media = project
        .media
        .iter()
        .find(|media| media.id == request.media_id)
        .cloned()
        .ok_or_else(|| GenerateEditError::MediaNotFound(request.media_id.clone()))?;
    let transcript = project
        .transcripts
        .iter()
        .find(|transcript| transcript.media_id == request.media_id)
        .cloned()
        .ok_or_else(|| GenerateEditError::TranscriptNotFound(request.media_id.clone()))?;
    let mut merged_moment_signals = project
        .media_analysis
        .iter()
        .filter(|signal| signal.media_id == request.media_id)
        .map(|signal| MomentSignal {
            source_in: signal.source_in,
            source_out: signal.source_out,
            visual_action_score: signal.visual_action_score,
            audio_energy_score: signal.audio_energy_score,
            label: signal.label.clone(),
        })
        .collect::<Vec<_>>();
    merged_moment_signals.extend_from_slice(moment_signals);
    let edl = build_rough_cut_edl_with_signals_and_silence(
        &request,
        &media.id,
        media.duration_seconds,
        &transcript.words,
        &merged_moment_signals,
        &project.media_silence_ranges,
    )
    .map_err(GenerateEditError::Edl)?;

    let mut video_items = Vec::new();
    let mut audio_items = Vec::new();
    let mut caption_ranges = Vec::new();
    let mut output_cursor = 0.0;

    for (index, clip) in edl.clips.iter().enumerate() {
        let duration = clip.source_out - clip.source_in;
        caption_ranges.push(CaptionSourceRange {
            media_id: media.id.clone(),
            source_in: clip.source_in,
            source_out: clip.source_out,
            output_start: output_cursor,
        });

        let mut properties = BTreeMap::new();
        properties.insert("sourceIn".to_string(), json!(clip.source_in));
        properties.insert("sourceOut".to_string(), json!(clip.source_out));
        properties.insert("reason".to_string(), json!(clip.reason));

        video_items.push(TimelineItem {
            id: format!("generated-video-{}", index + 1),
            kind: TimelineItemKind::VideoClip,
            start_seconds: output_cursor,
            duration_seconds: duration,
            source: TimelineSource::Media {
                media_id: media.id.clone(),
            },
            label: format!("Generated clip {}", index + 1),
            properties: properties.clone(),
        });

        audio_items.push(TimelineItem {
            id: format!("generated-audio-{}", index + 1),
            kind: TimelineItemKind::AudioClip,
            start_seconds: output_cursor,
            duration_seconds: duration,
            source: TimelineSource::Media {
                media_id: media.id.clone(),
            },
            label: format!("Generated audio {}", index + 1),
            properties,
        });

        output_cursor += duration;
    }

    let caption_cues = build_caption_cues(
        &transcript.words,
        &caption_ranges,
        CaptionGenerationOptions::default(),
    )
    .map_err(GenerateEditError::Captions)?;
    let edited_caption_texts = collect_edited_caption_texts_by_source_range(project);
    let caption_items = caption_cues
        .into_iter()
        .enumerate()
        .map(|(index, cue)| {
            let mut properties = BTreeMap::new();
            properties.insert("sourceIn".to_string(), json!(cue.source_in));
            properties.insert("sourceOut".to_string(), json!(cue.source_out));
            properties.insert("cueIndex".to_string(), json!(index));
            properties.insert("lineBreaks".to_string(), json!(cue.line_breaks));
            properties.insert("stylePreset".to_string(), json!(cue.style.style_preset));
            properties.insert(
                "visualTreatment".to_string(),
                json!(cue.style.visual_treatment),
            );
            properties.insert("motion".to_string(), json!(cue.style.motion));
            properties.insert("safeZone".to_string(), json!(cue.style.safe_zone));
            properties.insert("avoid".to_string(), json!(cue.style.avoid));
            properties.insert(
                "generatedBy".to_string(),
                json!("deterministic-caption-stage"),
            );
            let source_range_key = caption_source_range_key(cue.source_in, cue.source_out);
            let caption_text =
                if let Some(edited_text) = edited_caption_texts.get(&source_range_key) {
                    properties.insert("textEdited".to_string(), json!(true));
                    edited_text.clone()
                } else {
                    properties.insert("textEdited".to_string(), json!(false));
                    cue.text
                };

            TimelineItem {
                id: format!("generated-caption-{}", index + 1),
                kind: TimelineItemKind::Caption,
                start_seconds: cue.start_seconds,
                duration_seconds: cue.duration_seconds,
                source: TimelineSource::Text { text: caption_text },
                label: format!("Caption {}", index + 1),
                properties,
            }
        })
        .collect::<Vec<_>>();

    replace_track_items(project, TrackKind::Video, video_items)?;
    replace_track_items(project, TrackKind::Caption, caption_items.clone())?;
    replace_track_items(project, TrackKind::Audio, audio_items)?;
    project.timeline.duration_seconds = output_cursor;

    Ok(GeneratedEditDraft {
        media_id: media.id,
        timeline_duration_seconds: output_cursor,
        clip_count: edl.clips.len(),
        caption_count: caption_items.len(),
        edl,
    })
}

pub fn collect_template_render_layers(
    project: &VideoProject,
) -> Result<Vec<TemplateRenderLayer>, RenderPlanError> {
    let mut layers = Vec::new();
    for track in &project.timeline.tracks {
        if track.kind != TrackKind::Overlay {
            continue;
        }
        for item in &track.items {
            let Some(template_id) = string_property(item, "templateId") else {
                continue;
            };
            if item.kind != TimelineItemKind::Overlay {
                return Err(RenderPlanError::InvalidTemplateLayer(
                    "template items must be overlay items",
                ));
            }
            if !item.start_seconds.is_finite()
                || item.start_seconds < 0.0
                || !item.duration_seconds.is_finite()
                || item.duration_seconds <= 0.0
            {
                return Err(RenderPlanError::InvalidTemplateLayer(
                    "template timing must be finite and positive",
                ));
            }

            layers.push(TemplateRenderLayer {
                item_id: item.id.clone(),
                template_id,
                label: item.label.clone(),
                timeline_start_seconds: item.start_seconds,
                duration_seconds: item.duration_seconds,
                fields: template_fields(item)?,
                visual_treatment: required_string_property(item, "visualTreatment")?,
                motion: required_string_property(item, "motion")?,
                safe_zone: required_string_property(item, "safeZone")?,
                avoid: required_string_property(item, "avoid")?,
                preview_variant: required_string_property(item, "previewVariant")?,
            });
        }
    }

    Ok(layers)
}

fn string_property(item: &TimelineItem, key: &str) -> Option<String> {
    item.properties
        .get(key)
        .and_then(serde_json::Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
}

fn required_string_property(item: &TimelineItem, key: &str) -> Result<String, RenderPlanError> {
    string_property(item, key).ok_or(RenderPlanError::InvalidTemplateLayer(
        "template metadata must include non-empty strings",
    ))
}

fn template_fields(item: &TimelineItem) -> Result<BTreeMap<String, String>, RenderPlanError> {
    let fields = item
        .properties
        .get("templateFields")
        .and_then(serde_json::Value::as_object)
        .ok_or(RenderPlanError::InvalidTemplateLayer(
            "template fields must be an object",
        ))?;
    let mut result = BTreeMap::new();
    for (key, value) in fields {
        let Some(value) = value.as_str().map(str::trim) else {
            return Err(RenderPlanError::InvalidTemplateLayer(
                "template fields must be strings",
            ));
        };
        result.insert(key.clone(), value.to_string());
    }
    Ok(result)
}

fn collect_edited_caption_texts_by_source_range(
    project: &VideoProject,
) -> BTreeMap<String, String> {
    let mut edited_texts = BTreeMap::new();
    let Some(track) = project
        .timeline
        .tracks
        .iter()
        .find(|track| track.kind == TrackKind::Caption)
    else {
        return edited_texts;
    };

    for item in &track.items {
        if item.kind != TimelineItemKind::Caption
            || item
                .properties
                .get("textEdited")
                .and_then(|value| value.as_bool())
                != Some(true)
        {
            continue;
        }
        let TimelineSource::Text { text } = &item.source else {
            continue;
        };
        let Some(source_in) = numeric_property(item, "sourceIn") else {
            continue;
        };
        let Some(source_out) = numeric_property(item, "sourceOut") else {
            continue;
        };
        edited_texts.insert(
            caption_source_range_key(source_in, source_out),
            text.clone(),
        );
    }

    edited_texts
}

fn numeric_property(item: &TimelineItem, key: &str) -> Option<f64> {
    item.properties
        .get(key)
        .and_then(serde_json::Value::as_f64)
        .filter(|value| value.is_finite())
}

fn caption_source_range_key(source_in: f64, source_out: f64) -> String {
    format!("{source_in:.3}:{source_out:.3}")
}

fn replace_track_items(
    project: &mut VideoProject,
    kind: TrackKind,
    items: Vec<TimelineItem>,
) -> Result<(), GenerateEditError> {
    let track = project
        .timeline
        .tracks
        .iter_mut()
        .find(|track| track.kind == kind)
        .ok_or(GenerateEditError::TrackNotFound(kind))?;
    track.items = items;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::edit::preset::{CaptionStyle, EditPreset, LanguageMode};
    use crate::project::model::{
        MediaAsset, MediaKind, MediaSilenceRange, Transcript, TranscriptWord,
    };
    use crate::render_pipeline::project_export::build_project_webm_render_plan;
    use std::collections::BTreeSet;
    use std::path::Path;

    #[test]
    fn multi_source_fixture_materializes_aligned_edl_captions_audio_and_render_plan() {
        let mut project = sample_project_with_media_and_transcript();
        project.media.push(MediaAsset {
            id: "media-2".to_string(),
            name: Some("Second source".to_string()),
            relative_path: "media/second.mp4".to_string(),
            kind: MediaKind::Image,
            duration_seconds: 120.0,
            width: Some(1080),
            height: Some(1920),
            fps: Some(30.0),
            folder_id: None,
        });
        let request = EditJobRequest {
            media_id: "media-1".to_string(),
            preset: EditPreset::TrailerCut,
            prompt: "Build a concise multi-source launch story".to_string(),
            target_duration_seconds: Some(30.0),
            language_mode: LanguageMode::English,
            caption_style: CaptionStyle::Bold,
            created_at: "2026-07-11T00:00:00Z".to_string(),
        };
        let candidate = |media_id: &str,
                         source_in: f64,
                         hook: f64,
                         payoff: f64,
                         context: f64,
                         visual: f64,
                         reason: &str| EdlCandidateRange {
            media_id: media_id.to_string(),
            source_duration_seconds: 120.0,
            source_in,
            source_out: source_in + 10.0,
            hook_score: hook,
            payoff_score: payoff,
            context_score: context,
            quality_score: 0.9,
            spoken_score: 0.85,
            visual_score: visual,
            repetition_key: reason.to_string(),
            reason: reason.to_string(),
        };
        let candidates = vec![
            candidate("media-1", 0.5, 1.0, 0.2, 0.4, 0.8, "opening hook"),
            candidate("media-1", 30.5, 0.3, 1.0, 0.5, 0.7, "product payoff"),
            candidate("media-2", 9.5, 0.7, 0.4, 1.0, 0.8, "needed context"),
            candidate("media-2", 39.5, 0.4, 0.9, 0.3, 1.0, "visual proof"),
        ];

        let draft = generate_multi_source_edit_timeline(&mut project, request, &candidates)
            .expect("generate deterministic multi-source edit");
        assert!((draft.timeline_duration_seconds - 30.0).abs() < 0.001);
        let selected_sources = draft
            .edl
            .clips
            .iter()
            .map(|clip| clip.media_id.as_str())
            .collect::<BTreeSet<_>>();
        assert!(
            selected_sources.len() >= 2,
            "fixture must select at least two canonical sources"
        );
        assert!(draft
            .edl
            .clips
            .iter()
            .all(|clip| clip.source_in > 0.1 || clip.source_out < 119.9));
        assert!(draft
            .edl
            .clips
            .iter()
            .all(|clip| !clip.selection_reasons.is_empty() && clip.score.is_some()));

        let video = project
            .timeline
            .tracks
            .iter()
            .find(|track| track.kind == TrackKind::Video)
            .expect("video track");
        let audio = project
            .timeline
            .tracks
            .iter()
            .find(|track| track.kind == TrackKind::Audio)
            .expect("audio track");
        assert!(
            video.items.len() > audio.items.len(),
            "image selection must not fabricate linked audio"
        );
        for audio_item in &audio.items {
            let video_item = video
                .items
                .iter()
                .find(|item| item.start_seconds == audio_item.start_seconds)
                .expect("audio must align to its source-backed video clip");
            assert_eq!(video_item.start_seconds, audio_item.start_seconds);
            assert_eq!(video_item.duration_seconds, audio_item.duration_seconds);
            assert_eq!(video_item.source, audio_item.source);
            assert_eq!(
                video_item.properties["sourceIn"],
                audio_item.properties["sourceIn"]
            );
            assert_eq!(
                video_item.properties["sourceOut"],
                audio_item.properties["sourceOut"]
            );
        }
        let captions = project
            .timeline
            .tracks
            .iter()
            .find(|track| track.kind == TrackKind::Caption)
            .expect("caption track");
        assert!(!captions.items.is_empty());
        assert!(captions
            .items
            .iter()
            .all(|caption| caption.start_seconds >= 0.0
                && caption.start_seconds + caption.duration_seconds
                    <= draft.timeline_duration_seconds + 0.001
                && caption.properties["mediaId"]
                    .as_str()
                    .is_some_and(|id| selected_sources.contains(id))));

        let plan = build_project_webm_render_plan(
            Path::new("/tmp/multi-source-fixture"),
            &project,
            "multi-source-fixture",
            RenderQualityProfile::DraftWebm,
        )
        .expect("build canonical render plan");
        assert_eq!(plan.clips.len(), draft.edl.clips.len());
        assert!(plan.audio_clips.len() < plan.clips.len());
        for audio_clip in &plan.audio_clips {
            let video_clip = plan
                .clips
                .iter()
                .find(|clip| clip.timeline_start_seconds == audio_clip.timeline_start_seconds)
                .expect("render audio must align to its source-backed visual clip");
            assert_eq!(
                video_clip.timeline_start_seconds,
                audio_clip.timeline_start_seconds
            );
            assert_eq!(video_clip.source_in, audio_clip.source_in);
            assert_eq!(video_clip.source_out, audio_clip.source_out);
            assert_eq!(video_clip.source_path, audio_clip.source_path);
        }
    }

    #[test]
    fn spoken_and_semantic_candidates_preserve_media_ids_and_reasons() {
        let mut project = sample_project_with_media_and_transcript();
        project.media.push(MediaAsset {
            id: "media-2".to_string(),
            name: Some("Semantic source".to_string()),
            relative_path: "media/second.mp4".to_string(),
            kind: MediaKind::Image,
            duration_seconds: 120.0,
            width: Some(1080),
            height: Some(1920),
            fps: Some(30.0),
            folder_id: None,
        });
        let semantic_hits = vec![SemanticVisualHit {
            media_id: "media-2".to_string(),
            time_seconds: 10.0,
            shot_start_seconds: 5.0,
            shot_end_seconds: 20.0,
            score: 1.0,
            thumbnail_relative_path: "search/visual/media-2/frame.png".to_string(),
        }];
        let request = EditJobRequest {
            media_id: "media-1".to_string(),
            preset: EditPreset::TrailerCut,
            prompt: "Use spoken hooks and local visual proof".to_string(),
            target_duration_seconds: Some(30.0),
            language_mode: LanguageMode::English,
            caption_style: CaptionStyle::Bold,
            created_at: "2026-07-11T00:00:00Z".to_string(),
        };
        let draft = generate_spoken_semantic_multi_source_edit_timeline(
            &mut project,
            request,
            &semantic_hits,
        )
        .expect("spoken plus semantic edit");
        let sources = draft
            .edl
            .clips
            .iter()
            .map(|clip| clip.media_id.as_str())
            .collect::<BTreeSet<_>>();
        assert_eq!(sources, BTreeSet::from(["media-1", "media-2"]));
        assert!(draft.edl.clips.iter().all(|clip| {
            !clip.reason.is_empty()
                && !clip.selection_reasons.is_empty()
                && (clip.reason.contains("spoken transcript")
                    || clip.reason.contains("local semantic visual"))
        }));
    }

    #[test]
    fn generated_timeline_avoids_stored_media_silence_ranges() {
        let mut project = sample_project_with_media_and_transcript();
        project.media_silence_ranges = vec![MediaSilenceRange {
            media_id: "media-1".to_string(),
            source_in: 41.0,
            source_out: 44.0,
            confidence: 0.91,
            label: "speech-free dead air".to_string(),
        }];
        let request = EditJobRequest {
            media_id: "media-1".to_string(),
            preset: EditPreset::TrailerCut,
            prompt: "Make a high-energy launch cut".to_string(),
            target_duration_seconds: Some(30.0),
            language_mode: LanguageMode::English,
            caption_style: CaptionStyle::Bold,
            created_at: "2026-06-12T00:00:00Z".to_string(),
        };
        let signals = vec![MomentSignal {
            source_in: 42.0,
            source_out: 45.5,
            visual_action_score: 0.95,
            audio_energy_score: 0.9,
            label: "fast product handling with music hit".to_string(),
        }];

        let result = generate_one_click_edit_timeline_with_signals(&mut project, request, &signals)
            .expect("generate edit with stored dead-air analysis");

        assert!(
            result
                .edl
                .clips
                .iter()
                .any(|clip| clip.reason.contains("stored dead air")),
            "expected EDL to record stored dead-air avoidance, got {:?}",
            result.edl
        );
        for clip in &result.edl.clips {
            assert!(
                clip.source_out <= 41.0 || clip.source_in >= 44.0,
                "EDL clip should not overlap stored dead air: {clip:?}"
            );
        }
        for item in project
            .timeline
            .tracks
            .iter()
            .find(|track| track.kind == TrackKind::Video)
            .expect("video track")
            .items
            .iter()
        {
            let source_in = item.properties["sourceIn"]
                .as_f64()
                .expect("generated item sourceIn");
            let source_out = item.properties["sourceOut"]
                .as_f64()
                .expect("generated item sourceOut");
            assert!(
                source_out <= 41.0 || source_in >= 44.0,
                "generated timeline item should not overlap stored dead air: {item:?}"
            );
        }
    }

    fn sample_project_with_media_and_transcript() -> VideoProject {
        let mut project = VideoProject::new_empty(
            "project-1".to_string(),
            "One Click Test".to_string(),
            "2026-06-12T00:00:00Z".to_string(),
        );
        project.media.push(MediaAsset {
            id: "media-1".to_string(),
            name: None,
            relative_path: "media/input.mp4".to_string(),
            kind: MediaKind::Video,
            duration_seconds: 120.0,
            width: Some(1080),
            height: Some(1920),
            fps: Some(30.0),
            folder_id: None,
        });
        project.transcripts.push(Transcript {
            id: "transcript-media-1".to_string(),
            media_id: "media-1".to_string(),
            engine: Some("nvidia/parakeet-tdt-0.6b-v3".to_string()),
            raw_artifact_path: None,
            repairs: Vec::new(),
            segments: Vec::new(),
            words: sample_words_across_two_minutes(),
        });

        project
    }

    fn sample_words_across_two_minutes() -> Vec<TranscriptWord> {
        (0..36)
            .map(|index| {
                let start_seconds = index as f64 * 3.0;
                TranscriptWord {
                    text: if index % 5 == 0 {
                        "мощно".to_string()
                    } else {
                        format!("word-{index}")
                    },
                    start_seconds,
                    end_seconds: start_seconds + 0.8,
                    confidence: Some(0.9),
                    speaker: None,
                }
            })
            .collect()
    }
}
