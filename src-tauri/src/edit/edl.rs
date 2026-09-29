use super::preset::{EditJobRequest, EditPreset, EditRequestError};
use crate::project::model::{MediaSilenceRange, TranscriptWord};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use thiserror::Error;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct RoughCutEdl {
    pub media_id: String,
    pub source_duration_seconds: f64,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub source_durations_seconds: BTreeMap<String, f64>,
    pub clips: Vec<EdlClip>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct EdlClip {
    #[serde(default)]
    pub media_id: String,
    pub source_in: f64,
    pub source_out: f64,
    pub reason: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub selection_reasons: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub score: Option<EdlSelectionScore>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct EdlSelectionScore {
    pub hook: f64,
    pub payoff: f64,
    pub context: f64,
    pub quality: f64,
    pub spoken: f64,
    pub visual: f64,
    pub diversity: f64,
    pub repetition: f64,
    pub target_duration: f64,
    pub total: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct EdlCandidateRange {
    pub media_id: String,
    pub source_duration_seconds: f64,
    pub source_in: f64,
    pub source_out: f64,
    #[serde(default)]
    pub hook_score: f64,
    #[serde(default)]
    pub payoff_score: f64,
    #[serde(default)]
    pub context_score: f64,
    #[serde(default)]
    pub quality_score: f64,
    #[serde(default)]
    pub spoken_score: f64,
    #[serde(default)]
    pub visual_score: f64,
    #[serde(default)]
    pub repetition_key: String,
    #[serde(default)]
    pub reason: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct MomentSignal {
    pub source_in: f64,
    pub source_out: f64,
    #[serde(default)]
    pub visual_action_score: f64,
    #[serde(default)]
    pub audio_energy_score: f64,
    #[serde(default)]
    pub label: String,
}

#[derive(Debug, Error, PartialEq)]
pub enum EdlError {
    #[error("edit request is invalid: {0}")]
    InvalidRequest(EditRequestError),
    #[error("source duration must be finite and greater than zero")]
    InvalidSourceDuration,
    #[error("edl must contain at least one clip")]
    Empty,
    #[error("clip source range is invalid")]
    InvalidClipRange,
    #[error("clip is outside source media duration")]
    ClipOutsideSource,
    #[error("one-click edit cannot pass through the full source as one clip")]
    FullSourcePassThrough,
    #[error("one-click edit must be shorter than source media")]
    NotShorterThanSource,
    #[error("edit duration exceeds preset maximum")]
    ExceedsPresetMaximum,
    #[error("candidate range is invalid")]
    InvalidCandidateRange,
    #[error("candidate media id is empty")]
    EmptyCandidateMediaId,
}

impl RoughCutEdl {
    pub fn duration_seconds(&self) -> f64 {
        self.clips
            .iter()
            .map(|clip| clip.source_out - clip.source_in)
            .sum()
    }
}

pub fn build_rough_cut_edl(
    request: &EditJobRequest,
    media_id: &str,
    source_duration_seconds: f64,
    words: &[TranscriptWord],
) -> Result<RoughCutEdl, EdlError> {
    build_rough_cut_edl_with_signals(request, media_id, source_duration_seconds, words, &[])
}

pub fn build_rough_cut_edl_with_signals(
    request: &EditJobRequest,
    media_id: &str,
    source_duration_seconds: f64,
    words: &[TranscriptWord],
    moment_signals: &[MomentSignal],
) -> Result<RoughCutEdl, EdlError> {
    build_rough_cut_edl_with_signals_and_silence(
        request,
        media_id,
        source_duration_seconds,
        words,
        moment_signals,
        &[],
    )
}

pub fn build_rough_cut_edl_with_signals_and_silence(
    request: &EditJobRequest,
    media_id: &str,
    source_duration_seconds: f64,
    words: &[TranscriptWord],
    moment_signals: &[MomentSignal],
    silence_ranges: &[MediaSilenceRange],
) -> Result<RoughCutEdl, EdlError> {
    request.validate().map_err(EdlError::InvalidRequest)?;
    if !source_duration_seconds.is_finite() || source_duration_seconds <= 0.0 {
        return Err(EdlError::InvalidSourceDuration);
    }

    let (_, preset_max) = request.preset.duration_range();
    let target = request
        .target_duration_seconds
        .unwrap_or_else(|| request.preset.default_target_duration_seconds())
        .min(preset_max)
        .min((source_duration_seconds * 0.8).max(1.0));
    let mut clips: Vec<EdlClip> = Vec::new();
    let mut accumulated = 0.0;
    let mut candidates = scored_moment_candidates(request, source_duration_seconds, words);
    let mut derived_signals =
        derive_timed_transcript_moment_signals(source_duration_seconds, words);
    derived_signals.extend_from_slice(moment_signals);
    candidates.extend(scored_signal_candidates(
        request,
        source_duration_seconds,
        &derived_signals,
    ));
    let silence_ranges =
        normalized_silence_ranges(media_id, source_duration_seconds, silence_ranges);
    candidates = candidates
        .into_iter()
        .flat_map(|candidate| split_candidate_around_silence(candidate, &silence_ranges))
        .collect();
    candidates.sort_by(|left, right| {
        right
            .score
            .total_cmp(&left.score)
            .then_with(|| left.source_in.total_cmp(&right.source_in))
    });

    for candidate in candidates {
        if accumulated >= target {
            break;
        }
        if clips.iter().any(|clip| {
            candidate.source_in < clip.source_out + 0.35
                && candidate.source_out > clip.source_in - 0.35
        }) {
            continue;
        }

        let remaining = target - accumulated;
        let clip_duration = candidate.source_out - candidate.source_in;
        let clipped_source_out = if clip_duration > remaining {
            candidate.source_in + remaining
        } else {
            candidate.source_out
        };
        if clipped_source_out > candidate.source_in {
            accumulated += clipped_source_out - candidate.source_in;
            clips.push(EdlClip {
                media_id: media_id.to_string(),
                source_in: candidate.source_in,
                source_out: clipped_source_out,
                selection_reasons: vec![candidate.reason.clone()],
                reason: candidate.reason,
                score: None,
            });
        }
    }

    if clips.is_empty() {
        let fallback_out = target.min(source_duration_seconds * 0.8).max(0.5);
        let fallback = MomentCandidate {
            source_in: 0.0,
            source_out: fallback_out,
            score: 0.0,
            reason: "fallback opening range".to_string(),
        };
        let fallback = split_candidate_around_silence(fallback, &silence_ranges)
            .into_iter()
            .next()
            .unwrap_or(MomentCandidate {
                source_in: 0.0,
                source_out: fallback_out,
                score: 0.0,
                reason: "fallback opening range".to_string(),
            });
        clips.push(EdlClip {
            media_id: media_id.to_string(),
            source_in: fallback.source_in,
            source_out: fallback.source_out,
            selection_reasons: vec![fallback.reason.clone()],
            reason: fallback.reason,
            score: None,
        });
    }

    let edl = RoughCutEdl {
        media_id: media_id.to_string(),
        source_duration_seconds,
        source_durations_seconds: BTreeMap::from([(media_id.to_string(), source_duration_seconds)]),
        clips,
    };
    validate_one_click_edl(&request.preset, &edl)?;
    Ok(edl)
}

pub fn build_multi_source_rough_cut_edl(
    request: &EditJobRequest,
    candidates: &[EdlCandidateRange],
) -> Result<RoughCutEdl, EdlError> {
    request.validate().map_err(EdlError::InvalidRequest)?;
    if candidates.is_empty() {
        return Err(EdlError::Empty);
    }

    let mut source_durations = BTreeMap::new();
    for candidate in candidates {
        if candidate.media_id.trim().is_empty() {
            return Err(EdlError::EmptyCandidateMediaId);
        }
        if !candidate.source_duration_seconds.is_finite()
            || candidate.source_duration_seconds <= 0.0
            || !candidate.source_in.is_finite()
            || !candidate.source_out.is_finite()
            || candidate.source_in < 0.0
            || candidate.source_out <= candidate.source_in
            || candidate.source_out > candidate.source_duration_seconds
        {
            return Err(EdlError::InvalidCandidateRange);
        }
        source_durations
            .entry(candidate.media_id.clone())
            .and_modify(|duration: &mut f64| {
                *duration = duration.max(candidate.source_duration_seconds)
            })
            .or_insert(candidate.source_duration_seconds);
    }

    let total_source_duration = source_durations.values().sum::<f64>();
    let (_, preset_max) = request.preset.duration_range();
    let target = request
        .target_duration_seconds
        .unwrap_or_else(|| request.preset.default_target_duration_seconds())
        .min(preset_max)
        .min((total_source_duration * 0.8).max(1.0));
    let enforce_diversity = source_durations.len() >= 2 && target >= 1.0;
    let mut selected = Vec::new();
    let mut selected_media = BTreeSet::new();
    let mut selected_repetition_keys = BTreeSet::new();
    let mut used_candidates = BTreeSet::new();
    let mut accumulated = 0.0;

    while accumulated + 0.000_001 < target {
        let remaining = target - accumulated;
        let require_new_source = enforce_diversity && selected_media.len() == 1;
        let mut ranked = candidates
            .iter()
            .enumerate()
            .filter(|(index, candidate)| {
                !used_candidates.contains(index)
                    && (!require_new_source || !selected_media.contains(&candidate.media_id))
                    && !selected.iter().any(|clip: &EdlClip| {
                        clip.media_id == candidate.media_id
                            && candidate.source_in < clip.source_out + 0.1
                            && candidate.source_out > clip.source_in - 0.1
                    })
            })
            .map(|(index, candidate)| {
                let score = score_candidate(
                    candidate,
                    remaining,
                    target,
                    &selected_media,
                    &selected_repetition_keys,
                );
                (index, candidate, score)
            })
            .collect::<Vec<_>>();
        ranked.sort_by(|left, right| {
            right
                .2
                .total
                .total_cmp(&left.2.total)
                .then_with(|| left.1.media_id.cmp(&right.1.media_id))
                .then_with(|| left.1.source_in.total_cmp(&right.1.source_in))
                .then_with(|| left.0.cmp(&right.0))
        });
        let Some((index, candidate, score)) = ranked.into_iter().next() else {
            break;
        };
        used_candidates.insert(index);

        let reserve_for_second_source = if enforce_diversity && selected.is_empty() {
            0.5
        } else {
            0.0
        };
        let allowed = (remaining - reserve_for_second_source).max(0.5);
        let source_out = candidate.source_out.min(candidate.source_in + allowed);
        if source_out <= candidate.source_in {
            continue;
        }
        let selection_reasons = selection_reasons(candidate, &score);
        let reason = selection_reasons.join("; ");
        accumulated += source_out - candidate.source_in;
        selected_media.insert(candidate.media_id.clone());
        if !candidate.repetition_key.trim().is_empty() {
            selected_repetition_keys.insert(candidate.repetition_key.trim().to_lowercase());
        }
        selected.push(EdlClip {
            media_id: candidate.media_id.clone(),
            source_in: candidate.source_in,
            source_out,
            reason,
            selection_reasons,
            score: Some(score),
        });
    }

    let edl = RoughCutEdl {
        media_id: request.media_id.clone(),
        source_duration_seconds: total_source_duration,
        source_durations_seconds: source_durations,
        clips: selected,
    };
    validate_one_click_edl(&request.preset, &edl)?;
    Ok(edl)
}

fn normalized_score(value: f64) -> f64 {
    if value.is_finite() {
        value.clamp(0.0, 1.0)
    } else {
        0.0
    }
}

fn score_candidate(
    candidate: &EdlCandidateRange,
    remaining: f64,
    target: f64,
    selected_media: &BTreeSet<String>,
    selected_repetition_keys: &BTreeSet<String>,
) -> EdlSelectionScore {
    let hook = normalized_score(candidate.hook_score);
    let payoff = normalized_score(candidate.payoff_score);
    let context = normalized_score(candidate.context_score);
    let quality = normalized_score(candidate.quality_score);
    let spoken = normalized_score(candidate.spoken_score);
    let visual = normalized_score(candidate.visual_score);
    let diversity = if selected_media.is_empty() || selected_media.contains(&candidate.media_id) {
        0.0
    } else {
        1.0
    };
    let repetition = if !candidate.repetition_key.trim().is_empty()
        && selected_repetition_keys.contains(&candidate.repetition_key.trim().to_lowercase())
    {
        1.0
    } else {
        0.0
    };
    let duration = candidate.source_out - candidate.source_in;
    let target_duration =
        (1.0 - (duration.min(remaining) - remaining).abs() / target.max(0.001)).clamp(0.0, 1.0);
    let total = hook * 1.25
        + payoff * 1.2
        + context * 0.75
        + quality
        + spoken * 0.6
        + visual * 0.6
        + diversity * 0.9
        - repetition * 1.25
        + target_duration * 0.35;
    EdlSelectionScore {
        hook,
        payoff,
        context,
        quality,
        spoken,
        visual,
        diversity,
        repetition,
        target_duration,
        total,
    }
}

fn selection_reasons(candidate: &EdlCandidateRange, score: &EdlSelectionScore) -> Vec<String> {
    let mut reasons = Vec::new();
    if !candidate.reason.trim().is_empty() {
        reasons.push(candidate.reason.trim().to_string());
    }
    for (label, value) in [
        ("hook", score.hook),
        ("payoff", score.payoff),
        ("context", score.context),
        ("quality", score.quality),
        ("spoken", score.spoken),
        ("visual", score.visual),
    ] {
        if value >= 0.65 {
            reasons.push(format!("{label} {value:.2}"));
        }
    }
    if score.diversity > 0.0 {
        reasons.push("source diversity".to_string());
    }
    if score.repetition > 0.0 {
        reasons.push("repetition penalty applied".to_string());
    }
    reasons.push(format!("target fit {:.2}", score.target_duration));
    reasons
}

#[derive(Debug, Clone)]
struct MomentCandidate {
    source_in: f64,
    source_out: f64,
    score: f64,
    reason: String,
}

#[derive(Debug, Clone, Copy)]
struct SilenceRange {
    source_in: f64,
    source_out: f64,
}

fn normalized_silence_ranges(
    media_id: &str,
    source_duration_seconds: f64,
    silence_ranges: &[MediaSilenceRange],
) -> Vec<SilenceRange> {
    let mut ranges = silence_ranges
        .iter()
        .filter(|range| range.media_id == media_id)
        .filter_map(|range| {
            if !range.source_in.is_finite()
                || !range.source_out.is_finite()
                || range.source_out <= range.source_in
            {
                return None;
            }
            let source_in = range.source_in.clamp(0.0, source_duration_seconds);
            let source_out = range.source_out.clamp(0.0, source_duration_seconds);
            (source_out > source_in).then_some(SilenceRange {
                source_in,
                source_out,
            })
        })
        .collect::<Vec<_>>();
    ranges.sort_by(|left, right| {
        left.source_in
            .total_cmp(&right.source_in)
            .then_with(|| left.source_out.total_cmp(&right.source_out))
    });

    let mut merged: Vec<SilenceRange> = Vec::new();
    for range in ranges {
        if let Some(last) = merged.last_mut() {
            if range.source_in <= last.source_out {
                last.source_out = last.source_out.max(range.source_out);
                continue;
            }
        }
        merged.push(range);
    }
    merged
}

fn split_candidate_around_silence(
    candidate: MomentCandidate,
    silence_ranges: &[SilenceRange],
) -> Vec<MomentCandidate> {
    if silence_ranges.is_empty() {
        return vec![candidate];
    }

    let mut segments = vec![(candidate.source_in, candidate.source_out, false)];
    for silence in silence_ranges {
        let mut next_segments = Vec::new();
        for (segment_in, segment_out, already_trimmed) in segments {
            if silence.source_out <= segment_in || silence.source_in >= segment_out {
                next_segments.push((segment_in, segment_out, already_trimmed));
                continue;
            }

            if silence.source_in > segment_in {
                next_segments.push((segment_in, silence.source_in.min(segment_out), true));
            }
            if silence.source_out < segment_out {
                next_segments.push((silence.source_out.max(segment_in), segment_out, true));
            }
        }
        segments = next_segments;
        if segments.is_empty() {
            break;
        }
    }

    segments
        .into_iter()
        .filter(|(source_in, source_out, _)| source_out - source_in >= 0.2)
        .map(|(source_in, source_out, trimmed)| MomentCandidate {
            source_in,
            source_out,
            score: candidate.score,
            reason: if trimmed {
                format!("{}; avoided stored dead air", candidate.reason)
            } else {
                candidate.reason.clone()
            },
        })
        .collect()
}

fn scored_moment_candidates(
    request: &EditJobRequest,
    source_duration_seconds: f64,
    words: &[TranscriptWord],
) -> Vec<MomentCandidate> {
    let prompt_tokens = meaningful_tokens(&request.prompt);
    let usable_words = words
        .iter()
        .filter(|word| {
            word.start_seconds.is_finite()
                && word.end_seconds.is_finite()
                && word.end_seconds >= word.start_seconds
        })
        .collect::<Vec<_>>();

    usable_words
        .iter()
        .enumerate()
        .filter_map(|(index, word)| {
            let source_in = (word.start_seconds - 0.8).max(0.0);
            let mut source_out = (word.end_seconds + 1.2).min(source_duration_seconds);
            if source_out <= source_in {
                source_out = (source_in + 0.5).min(source_duration_seconds);
            }
            if source_out <= source_in {
                return None;
            }

            let word_tokens = meaningful_tokens(&word.text);
            let prompt_matches = word_tokens
                .iter()
                .filter(|token| prompt_tokens.iter().any(|prompt| prompt == *token))
                .cloned()
                .collect::<Vec<_>>();
            let mut score = 1.0 + word.confidence.unwrap_or(0.8).clamp(0.0, 1.0);
            let mut reasons = Vec::new();

            if !prompt_matches.is_empty() {
                score += 12.0 + prompt_matches.len() as f64 * 3.0;
                reasons.push(format!("prompt relevance: {}", prompt_matches.join(", ")));
            }
            let lower = word.text.to_lowercase();
            if matches!(
                lower.as_str(),
                "hook" | "opening" | "start" | "first" | "intro"
            ) || index == 0
            {
                score += 2.0;
                reasons.push("hook candidate".to_string());
            }
            if matches!(
                lower.as_str(),
                "payoff" | "result" | "reveal" | "finally" | "win" | "launch"
            ) {
                score += 4.0;
                reasons.push("payoff keyword".to_string());
            }
            if matches!(
                lower.as_str(),
                "action" | "demo" | "show" | "watch" | "look" | "move"
            ) {
                score += 3.0;
                reasons.push("visual action keyword".to_string());
            }
            let nearby_tokens = nearby_transcript_tokens(&usable_words, index, 3);
            let has_setup_beat = nearby_tokens
                .iter()
                .any(|token| story_setup_terms().contains(&token.as_str()));
            let has_payoff_beat = nearby_tokens
                .iter()
                .any(|token| story_payoff_terms().contains(&token.as_str()));
            if has_setup_beat && has_payoff_beat {
                score += 9.0;
                reasons.push("story beat: setup-to-payoff".to_string());
            } else if has_setup_beat || has_payoff_beat {
                score += 3.0;
                reasons.push("story beat".to_string());
            }
            if index > 0 {
                let previous = usable_words[index - 1];
                let pause = word.start_seconds - previous.end_seconds;
                if pause >= 0.75 {
                    score += pause.min(3.0);
                    reasons.push("pause before moment".to_string());
                }
            }

            let reason = if reasons.is_empty() {
                format!("scored transcript moment `{}`", word.text)
            } else {
                format!("{} around `{}`", reasons.join("; "), word.text)
            };

            Some(MomentCandidate {
                source_in,
                source_out,
                score,
                reason,
            })
        })
        .collect()
}

fn scored_signal_candidates(
    request: &EditJobRequest,
    source_duration_seconds: f64,
    moment_signals: &[MomentSignal],
) -> Vec<MomentCandidate> {
    let prompt_tokens = meaningful_tokens(&request.prompt);
    moment_signals
        .iter()
        .filter_map(|signal| {
            if !signal.source_in.is_finite()
                || !signal.source_out.is_finite()
                || signal.source_in < 0.0
                || signal.source_out <= signal.source_in
            {
                return None;
            }
            let source_in = signal.source_in.min(source_duration_seconds);
            let source_out = signal.source_out.min(source_duration_seconds);
            if source_out <= source_in {
                return None;
            }

            let visual_action_score = signal.visual_action_score.clamp(0.0, 1.0);
            let audio_energy_score = signal.audio_energy_score.clamp(0.0, 1.0);
            if visual_action_score <= 0.0 && audio_energy_score <= 0.0 {
                return None;
            }

            let mut reasons = Vec::new();
            let mut score = 10.0 + visual_action_score * 14.0 + audio_energy_score * 10.0;
            if visual_action_score > 0.0 {
                reasons.push(format!("visual action signal {:.2}", visual_action_score));
            }
            if audio_energy_score > 0.0 {
                reasons.push(format!("audio energy signal {:.2}", audio_energy_score));
            }
            let label = signal.label.trim();
            if !label.is_empty() {
                let label_tokens = meaningful_tokens(label);
                let prompt_matches = label_tokens
                    .iter()
                    .filter(|token| prompt_tokens.iter().any(|prompt| prompt == *token))
                    .cloned()
                    .collect::<Vec<_>>();
                if !prompt_matches.is_empty() {
                    score += 8.0 + prompt_matches.len() as f64 * 2.0;
                    reasons.push(format!(
                        "media-analysis label relevance: {}",
                        prompt_matches.join(", ")
                    ));
                }
                let semantic_matches =
                    semantic_media_analysis_matches(&prompt_tokens, &label_tokens);
                if !semantic_matches.is_empty() {
                    score += 18.0 + semantic_matches.len() as f64 * 3.0;
                    reasons.push(format!(
                        "semantic media-analysis relevance: {}",
                        semantic_matches.join(", ")
                    ));
                }

                let has_setup_beat = label_tokens
                    .iter()
                    .any(|token| story_setup_terms().contains(&token.as_str()));
                let has_payoff_beat = label_tokens
                    .iter()
                    .any(|token| story_payoff_terms().contains(&token.as_str()));
                if has_setup_beat && has_payoff_beat {
                    score += 6.0;
                    reasons.push("story beat: setup-to-payoff".to_string());
                } else if has_setup_beat || has_payoff_beat {
                    score += 2.0;
                    reasons.push("story beat".to_string());
                }

                reasons.push(label.to_string());
            }

            Some(MomentCandidate {
                source_in,
                source_out,
                score,
                reason: reasons.join("; "),
            })
        })
        .collect()
}

fn derive_timed_transcript_moment_signals(
    source_duration_seconds: f64,
    words: &[TranscriptWord],
) -> Vec<MomentSignal> {
    if !source_duration_seconds.is_finite() || source_duration_seconds <= 0.0 {
        return Vec::new();
    }

    let usable_words = words
        .iter()
        .filter(|word| {
            word.start_seconds.is_finite()
                && word.end_seconds.is_finite()
                && word.end_seconds > word.start_seconds
        })
        .collect::<Vec<_>>();
    let mut signals = Vec::new();

    for window in usable_words.windows(6) {
        let first = window[0];
        let last = window[window.len() - 1];
        let window_duration = last.end_seconds - first.start_seconds;
        if !(0.45..=4.0).contains(&window_duration) {
            continue;
        }

        let spoken_duration = window
            .iter()
            .map(|word| word.end_seconds - word.start_seconds)
            .sum::<f64>();
        let spoken_coverage = (spoken_duration / window_duration).clamp(0.0, 1.0);
        let words_per_second = window.len() as f64 / window_duration;
        let average_confidence = window
            .iter()
            .map(|word| word.confidence.unwrap_or(0.75).clamp(0.0, 1.0))
            .sum::<f64>()
            / window.len() as f64;

        let cadence_score = ((words_per_second - 3.0) / 3.0).clamp(0.0, 1.0);
        let visual_action_score = (cadence_score * 0.7 + spoken_coverage * 0.3).clamp(0.0, 1.0);
        let audio_energy_score =
            (cadence_score * 0.45 + spoken_coverage * 0.25 + average_confidence * 0.3)
                .clamp(0.0, 1.0);

        if visual_action_score < 0.45 && audio_energy_score < 0.55 {
            continue;
        }

        signals.push(MomentSignal {
            source_in: (first.start_seconds - 0.4).max(0.0),
            source_out: (last.end_seconds + 0.9).min(source_duration_seconds),
            visual_action_score,
            audio_energy_score,
            label:
                "derived visual action signal; derived audio energy signal from timed transcript metadata"
                    .to_string(),
        });
    }

    signals
}

fn nearby_transcript_tokens(words: &[&TranscriptWord], index: usize, radius: usize) -> Vec<String> {
    let start = index.saturating_sub(radius);
    let end = (index + radius + 1).min(words.len());

    words[start..end]
        .iter()
        .flat_map(|word| meaningful_tokens(&word.text))
        .collect()
}

fn story_setup_terms() -> &'static [&'static str] {
    &[
        "problem",
        "challenge",
        "blocked",
        "pain",
        "mistake",
        "risk",
        "stakes",
        "because",
        "but",
        "before",
        "slow",
        "hard",
    ]
}

fn story_payoff_terms() -> &'static [&'static str] {
    &[
        "solution", "solved", "proof", "result", "changed", "fixed", "learned", "finally", "win",
        "after", "faster", "shipped",
    ]
}

fn semantic_media_analysis_matches(
    prompt_tokens: &[String],
    label_tokens: &[String],
) -> Vec<&'static str> {
    semantic_media_analysis_groups()
        .iter()
        .filter_map(|group| {
            let prompt_has_group = prompt_tokens
                .iter()
                .any(|token| group.terms.contains(&token.as_str()));
            let label_has_group = label_tokens
                .iter()
                .any(|token| group.terms.contains(&token.as_str()));
            (prompt_has_group && label_has_group).then_some(group.name)
        })
        .collect()
}

struct SemanticMediaAnalysisGroup {
    name: &'static str,
    terms: &'static [&'static str],
}

fn semantic_media_analysis_groups() -> &'static [SemanticMediaAnalysisGroup] {
    &[
        SemanticMediaAnalysisGroup {
            name: "interview",
            terms: &[
                "interview",
                "founder",
                "customer",
                "speaker",
                "talking",
                "head",
                "quote",
                "testimonial",
                "insight",
                "conversation",
            ],
        },
        SemanticMediaAnalysisGroup {
            name: "product-detail",
            terms: &[
                "product",
                "feature",
                "detail",
                "spec",
                "closeup",
                "close",
                "hands",
                "demo",
                "interface",
                "screen",
            ],
        },
        SemanticMediaAnalysisGroup {
            name: "location",
            terms: &[
                "location",
                "place",
                "route",
                "map",
                "arrival",
                "venue",
                "city",
                "travel",
                "walkthrough",
            ],
        },
        SemanticMediaAnalysisGroup {
            name: "reaction",
            terms: &[
                "reaction",
                "emotional",
                "emotion",
                "audience",
                "smile",
                "smiles",
                "laugh",
                "laughter",
                "applause",
                "cheer",
                "excited",
                "delight",
            ],
        },
        SemanticMediaAnalysisGroup {
            name: "process",
            terms: &[
                "process",
                "workflow",
                "tutorial",
                "guide",
                "setup",
                "step",
                "steps",
                "walkthrough",
                "sequence",
                "roadmap",
                "method",
            ],
        },
        SemanticMediaAnalysisGroup {
            name: "chapter",
            terms: &[
                "chapter",
                "section",
                "phase",
                "transition",
                "act",
                "reset",
                "story",
                "narrative",
                "beat",
                "break",
                "title",
                "turning",
                "point",
            ],
        },
        SemanticMediaAnalysisGroup {
            name: "pricing",
            terms: &[
                "pricing", "price", "cost", "savings", "budget", "roi", "revenue", "margin",
                "spend", "proof",
            ],
        },
        SemanticMediaAnalysisGroup {
            name: "metric",
            terms: &[
                "metric",
                "metrics",
                "kpi",
                "data",
                "dashboard",
                "analytics",
                "chart",
                "graph",
                "retention",
                "growth",
                "report",
                "performance",
            ],
        },
        SemanticMediaAnalysisGroup {
            name: "comparison",
            terms: &[
                "comparison",
                "compare",
                "contrast",
                "delta",
                "difference",
                "different",
                "side",
                "split",
                "versus",
                "before",
                "after",
            ],
        },
        SemanticMediaAnalysisGroup {
            name: "tracking",
            terms: &[
                "tracking",
                "track",
                "callout",
                "highlight",
                "pointer",
                "spotlight",
                "annotation",
                "annotate",
                "detail",
                "focus",
                "important",
            ],
        },
        SemanticMediaAnalysisGroup {
            name: "schedule",
            terms: &[
                "schedule",
                "scheduled",
                "calendar",
                "deadline",
                "milestone",
                "announcement",
                "announce",
                "timing",
                "date",
                "reminder",
                "upcoming",
            ],
        },
        SemanticMediaAnalysisGroup {
            name: "risk",
            terms: &[
                "risk",
                "warning",
                "alert",
                "security",
                "compliance",
                "failure",
                "blocked",
                "urgent",
                "remediation",
                "reviewers",
            ],
        },
        SemanticMediaAnalysisGroup {
            name: "event",
            terms: &[
                "event",
                "recap",
                "conference",
                "keynote",
                "stage",
                "crowd",
                "booth",
                "expo",
                "summit",
                "demo",
                "demos",
                "launch",
            ],
        },
        SemanticMediaAnalysisGroup {
            name: "brand",
            terms: &[
                "brand",
                "logo",
                "identity",
                "wordmark",
                "signage",
                "product",
                "launch",
                "reveal",
                "packaging",
                "mark",
            ],
        },
    ]
}

fn meaningful_tokens(text: &str) -> Vec<String> {
    text.split(|character: char| !character.is_alphanumeric())
        .map(|token| token.trim().to_lowercase())
        .filter(|token| token.chars().count() >= 3)
        .filter(|token| {
            !matches!(
                token.as_str(),
                "the" | "and" | "for" | "with" | "that" | "this" | "make" | "create"
            )
        })
        .collect()
}

pub fn validate_one_click_edl(preset: &EditPreset, edl: &RoughCutEdl) -> Result<(), EdlError> {
    if !edl.source_duration_seconds.is_finite() || edl.source_duration_seconds <= 0.0 {
        return Err(EdlError::InvalidSourceDuration);
    }
    if edl.clips.is_empty() {
        return Err(EdlError::Empty);
    }

    for clip in &edl.clips {
        if !clip.source_in.is_finite()
            || !clip.source_out.is_finite()
            || clip.source_in < 0.0
            || clip.source_out <= clip.source_in
        {
            return Err(EdlError::InvalidClipRange);
        }
        let clip_media_id = if clip.media_id.is_empty() {
            &edl.media_id
        } else {
            &clip.media_id
        };
        let source_duration = edl
            .source_durations_seconds
            .get(clip_media_id)
            .copied()
            .or_else(|| (clip_media_id == &edl.media_id).then_some(edl.source_duration_seconds))
            .ok_or(EdlError::ClipOutsideSource)?;
        if clip.source_out > source_duration {
            return Err(EdlError::ClipOutsideSource);
        }
    }

    if edl.clips.len() == 1 {
        let clip = &edl.clips[0];
        let clip_media_id = if clip.media_id.is_empty() {
            &edl.media_id
        } else {
            &clip.media_id
        };
        let source_duration = edl
            .source_durations_seconds
            .get(clip_media_id)
            .copied()
            .unwrap_or(edl.source_duration_seconds);
        let covers_full_source = clip.source_in <= 0.1 && clip.source_out >= source_duration - 0.1;
        if covers_full_source {
            return Err(EdlError::FullSourcePassThrough);
        }
    }

    let duration = edl.duration_seconds();
    let total_source_duration = if edl.source_durations_seconds.is_empty() {
        edl.source_duration_seconds
    } else {
        edl.source_durations_seconds.values().sum()
    };
    if duration >= total_source_duration {
        return Err(EdlError::NotShorterThanSource);
    }

    let (_, max_duration) = preset.duration_range();
    if duration > max_duration {
        return Err(EdlError::ExceedsPresetMaximum);
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::edit::preset::{CaptionStyle, LanguageMode};

    #[test]
    fn legacy_edl_clip_without_media_id_decodes_and_uses_root_source() {
        let edl: RoughCutEdl = serde_json::from_value(serde_json::json!({
            "mediaId": "legacy-media",
            "sourceDurationSeconds": 20.0,
            "clips": [{ "sourceIn": 2.0, "sourceOut": 8.0, "reason": "legacy hook" }]
        }))
        .expect("decode legacy EDL");
        assert_eq!(edl.clips[0].media_id, "");
        assert_eq!(
            validate_one_click_edl(&EditPreset::TrailerCut, &edl),
            Ok(())
        );
    }

    #[test]
    fn visual_action_and_audio_energy_signals_can_win_moment_scoring() {
        let words = (0..20)
            .map(|index| {
                let start_seconds = 1.0 + index as f64 * 1.4;
                TranscriptWord {
                    text: "welcome".to_string(),
                    start_seconds,
                    end_seconds: start_seconds + 0.4,
                    confidence: Some(0.95),
                    speaker: None,
                }
            })
            .collect::<Vec<_>>();
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

        let edl = build_rough_cut_edl_with_signals(&request, "media-1", 60.0, &words, &signals)
            .expect("build edl with signals");

        assert!(
            edl.clips.first().is_some_and(|clip| clip.source_in >= 41.0),
            "expected visual/audio signal to win scoring, got {edl:?}"
        );
        assert!(edl
            .clips
            .iter()
            .any(|clip| clip.reason.contains("visual action signal")));
        assert!(edl
            .clips
            .iter()
            .any(|clip| clip.reason.contains("audio energy signal")));
    }

    #[test]
    fn metric_media_analysis_labels_can_win_moment_scoring() {
        let words = (0..20)
            .map(|index| {
                let start_seconds = 1.0 + index as f64 * 1.4;
                TranscriptWord {
                    text: "welcome".to_string(),
                    start_seconds,
                    end_seconds: start_seconds + 0.4,
                    confidence: Some(0.95),
                    speaker: None,
                }
            })
            .collect::<Vec<_>>();
        let request = EditJobRequest {
            media_id: "media-1".to_string(),
            preset: EditPreset::TrailerCut,
            prompt: "Make a KPI dashboard analytics cut about growth".to_string(),
            target_duration_seconds: Some(30.0),
            language_mode: LanguageMode::English,
            caption_style: CaptionStyle::Bold,
            created_at: "2026-06-12T00:00:00Z".to_string(),
        };
        let signals = vec![
            MomentSignal {
                source_in: 8.0,
                source_out: 11.5,
                visual_action_score: 0.96,
                audio_energy_score: 0.92,
                label: "generic fast montage with music hit".to_string(),
            },
            MomentSignal {
                source_in: 42.0,
                source_out: 45.5,
                visual_action_score: 0.58,
                audio_energy_score: 0.56,
                label: "metric chart graph retention line".to_string(),
            },
        ];

        let edl = build_rough_cut_edl_with_signals(&request, "media-1", 60.0, &words, &signals)
            .expect("build edl with metric signal");

        assert!(
            edl.clips.first().is_some_and(|clip| clip.source_in >= 41.0),
            "expected metric semantic signal to win scoring, got {edl:?}"
        );
        assert!(edl.clips.iter().any(|clip| clip
            .reason
            .contains("semantic media-analysis relevance: metric")));
    }

    #[test]
    fn schedule_media_analysis_labels_can_win_moment_scoring() {
        let words = (0..20)
            .map(|index| {
                let start_seconds = 1.0 + index as f64 * 1.4;
                TranscriptWord {
                    text: "welcome".to_string(),
                    start_seconds,
                    end_seconds: start_seconds + 0.4,
                    confidence: Some(0.95),
                    speaker: None,
                }
            })
            .collect::<Vec<_>>();
        let request = EditJobRequest {
            media_id: "media-1".to_string(),
            preset: EditPreset::TrailerCut,
            prompt: "Make a launch announcement timing cut".to_string(),
            target_duration_seconds: Some(30.0),
            language_mode: LanguageMode::English,
            caption_style: CaptionStyle::Bold,
            created_at: "2026-06-12T00:00:00Z".to_string(),
        };
        let signals = vec![
            MomentSignal {
                source_in: 8.0,
                source_out: 11.5,
                visual_action_score: 0.96,
                audio_energy_score: 0.92,
                label: "generic fast montage with music hit".to_string(),
            },
            MomentSignal {
                source_in: 42.0,
                source_out: 45.5,
                visual_action_score: 0.58,
                audio_energy_score: 0.56,
                label: "calendar milestone schedule deadline reminder".to_string(),
            },
        ];

        let edl = build_rough_cut_edl_with_signals(&request, "media-1", 60.0, &words, &signals)
            .expect("build edl with schedule signal");

        assert!(
            edl.clips.first().is_some_and(|clip| clip.source_in >= 41.0),
            "expected schedule semantic signal to win scoring, got {edl:?}"
        );
        assert!(edl.clips.iter().any(|clip| clip
            .reason
            .contains("semantic media-analysis relevance: schedule")));
    }

    #[test]
    fn comparison_media_analysis_labels_can_win_moment_scoring() {
        let words = (0..20)
            .map(|index| {
                let start_seconds = 1.0 + index as f64 * 1.4;
                TranscriptWord {
                    text: "welcome".to_string(),
                    start_seconds,
                    end_seconds: start_seconds + 0.4,
                    confidence: Some(0.95),
                    speaker: None,
                }
            })
            .collect::<Vec<_>>();
        let request = EditJobRequest {
            media_id: "media-1".to_string(),
            preset: EditPreset::TrailerCut,
            prompt: "Make a contrast comparison cut showing the delta".to_string(),
            target_duration_seconds: Some(30.0),
            language_mode: LanguageMode::English,
            caption_style: CaptionStyle::Bold,
            created_at: "2026-06-12T00:00:00Z".to_string(),
        };
        let signals = vec![
            MomentSignal {
                source_in: 8.0,
                source_out: 11.5,
                visual_action_score: 0.96,
                audio_energy_score: 0.92,
                label: "generic fast montage with music hit".to_string(),
            },
            MomentSignal {
                source_in: 42.0,
                source_out: 45.5,
                visual_action_score: 0.58,
                audio_energy_score: 0.56,
                label: "side by side split screen difference".to_string(),
            },
        ];

        let edl = build_rough_cut_edl_with_signals(&request, "media-1", 60.0, &words, &signals)
            .expect("build edl with comparison signal");

        assert!(
            edl.clips.first().is_some_and(|clip| clip.source_in >= 41.0),
            "expected comparison semantic signal to win scoring, got {edl:?}"
        );
        assert!(edl.clips.iter().any(|clip| clip
            .reason
            .contains("semantic media-analysis relevance: comparison")));
    }

    #[test]
    fn tracking_media_analysis_labels_can_win_moment_scoring() {
        let words = (0..20)
            .map(|index| {
                let start_seconds = 1.0 + index as f64 * 1.4;
                TranscriptWord {
                    text: "welcome".to_string(),
                    start_seconds,
                    end_seconds: start_seconds + 0.4,
                    confidence: Some(0.95),
                    speaker: None,
                }
            })
            .collect::<Vec<_>>();
        let request = EditJobRequest {
            media_id: "media-1".to_string(),
            preset: EditPreset::TrailerCut,
            prompt: "Make a spotlight annotation cut for the important detail".to_string(),
            target_duration_seconds: Some(30.0),
            language_mode: LanguageMode::English,
            caption_style: CaptionStyle::Bold,
            created_at: "2026-06-12T00:00:00Z".to_string(),
        };
        let signals = vec![
            MomentSignal {
                source_in: 8.0,
                source_out: 11.5,
                visual_action_score: 0.96,
                audio_energy_score: 0.92,
                label: "generic fast montage with music hit".to_string(),
            },
            MomentSignal {
                source_in: 42.0,
                source_out: 45.5,
                visual_action_score: 0.58,
                audio_energy_score: 0.56,
                label: "callout tracking highlight pointer".to_string(),
            },
        ];

        let edl = build_rough_cut_edl_with_signals(&request, "media-1", 60.0, &words, &signals)
            .expect("build edl with tracking signal");

        assert!(
            edl.clips.first().is_some_and(|clip| clip.source_in >= 41.0),
            "expected tracking semantic signal to win scoring, got {edl:?}"
        );
        assert!(edl.clips.iter().any(|clip| clip
            .reason
            .contains("semantic media-analysis relevance: tracking")));
    }

    #[test]
    fn chapter_media_analysis_labels_can_win_moment_scoring() {
        let words = (0..20)
            .map(|index| {
                let start_seconds = 1.0 + index as f64 * 1.4;
                TranscriptWord {
                    text: "welcome".to_string(),
                    start_seconds,
                    end_seconds: start_seconds + 0.4,
                    confidence: Some(0.95),
                    speaker: None,
                }
            })
            .collect::<Vec<_>>();
        let request = EditJobRequest {
            media_id: "media-1".to_string(),
            preset: EditPreset::TrailerCut,
            prompt: "Make a narrative turning-point cut for the next act".to_string(),
            target_duration_seconds: Some(30.0),
            language_mode: LanguageMode::English,
            caption_style: CaptionStyle::Bold,
            created_at: "2026-06-12T00:00:00Z".to_string(),
        };
        let signals = vec![
            MomentSignal {
                source_in: 8.0,
                source_out: 11.5,
                visual_action_score: 0.96,
                audio_energy_score: 0.92,
                label: "generic fast montage with music hit".to_string(),
            },
            MomentSignal {
                source_in: 42.0,
                source_out: 45.5,
                visual_action_score: 0.58,
                audio_energy_score: 0.56,
                label: "chapter reset story break section title".to_string(),
            },
        ];

        let edl = build_rough_cut_edl_with_signals(&request, "media-1", 60.0, &words, &signals)
            .expect("build edl with chapter signal");

        assert!(
            edl.clips.first().is_some_and(|clip| clip.source_in >= 41.0),
            "expected chapter semantic signal to win scoring, got {edl:?}"
        );
        assert!(edl.clips.iter().any(|clip| clip
            .reason
            .contains("semantic media-analysis relevance: chapter")));
    }
}
