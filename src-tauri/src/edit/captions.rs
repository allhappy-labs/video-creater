use crate::project::model::TranscriptWord;
use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct CaptionGenerationOptions {
    pub max_words_per_cue: usize,
    pub max_chars_per_cue: usize,
    pub max_chars_per_line: usize,
    pub min_duration_seconds: f64,
    pub max_duration_seconds: f64,
    pub pause_split_threshold_seconds: f64,
}

impl Default for CaptionGenerationOptions {
    fn default() -> Self {
        Self {
            max_words_per_cue: 7,
            max_chars_per_cue: 42,
            max_chars_per_line: 22,
            min_duration_seconds: 0.6,
            max_duration_seconds: 3.0,
            pause_split_threshold_seconds: 0.55,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct CaptionSourceRange {
    pub media_id: String,
    pub source_in: f64,
    pub source_out: f64,
    pub output_start: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct CaptionStyleMetadata {
    pub style_preset: String,
    pub visual_treatment: String,
    pub motion: String,
    pub safe_zone: String,
    pub avoid: String,
}

impl Default for CaptionStyleMetadata {
    fn default() -> Self {
        Self {
            style_preset: "boldReadableLower".to_string(),
            visual_treatment:
                "bold phone-readable lower-third caption with subtle translucent backing and accent emphasis"
                    .to_string(),
            motion: "quick pop-in, hold, and soft fade out".to_string(),
            safe_zone: "keep essential text inside 10% margins and above platform controls"
                .to_string(),
            avoid:
                "full-width opaque black slabs, centered text-only cards, faces, hands, product details, and main action"
                    .to_string(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct CaptionCue {
    pub media_id: String,
    pub text: String,
    pub line_breaks: Vec<String>,
    pub start_seconds: f64,
    pub duration_seconds: f64,
    pub source_in: f64,
    pub source_out: f64,
    pub style: CaptionStyleMetadata,
}

#[derive(Debug, Error, PartialEq)]
pub enum CaptionGenerationError {
    #[error("caption generation requires at least one source range")]
    EmptySourceRanges,
    #[error("caption source range is invalid")]
    InvalidSourceRange,
    #[error("caption generation options are invalid")]
    InvalidOptions,
}

pub fn build_caption_cues(
    words: &[TranscriptWord],
    source_ranges: &[CaptionSourceRange],
    options: CaptionGenerationOptions,
) -> Result<Vec<CaptionCue>, CaptionGenerationError> {
    validate_options(&options)?;
    if source_ranges.is_empty() {
        return Err(CaptionGenerationError::EmptySourceRanges);
    }

    let mut cues = Vec::new();
    for range in source_ranges {
        validate_range(range)?;
        let mut current_words = Vec::new();

        for word in words
            .iter()
            .filter(|word| {
                word.end_seconds > range.source_in && word.start_seconds < range.source_out
            })
            .filter(|word| word.start_seconds.is_finite() && word.end_seconds.is_finite())
            .filter(|word| !word.text.trim().is_empty())
        {
            if should_split_before(&current_words, word, &options) {
                push_cue(&mut cues, range, &mut current_words, &options);
            }

            current_words.push(word.clone());

            if is_sentence_ending(&word.text) {
                push_cue(&mut cues, range, &mut current_words, &options);
            }
        }

        push_cue(&mut cues, range, &mut current_words, &options);
    }

    Ok(cues)
}

fn validate_options(options: &CaptionGenerationOptions) -> Result<(), CaptionGenerationError> {
    if options.max_words_per_cue == 0
        || options.max_chars_per_cue == 0
        || options.max_chars_per_line == 0
        || !options.min_duration_seconds.is_finite()
        || !options.max_duration_seconds.is_finite()
        || !options.pause_split_threshold_seconds.is_finite()
        || options.min_duration_seconds <= 0.0
        || options.max_duration_seconds < options.min_duration_seconds
        || options.pause_split_threshold_seconds < 0.0
    {
        return Err(CaptionGenerationError::InvalidOptions);
    }

    Ok(())
}

fn validate_range(range: &CaptionSourceRange) -> Result<(), CaptionGenerationError> {
    if range.media_id.trim().is_empty()
        || !range.source_in.is_finite()
        || !range.source_out.is_finite()
        || !range.output_start.is_finite()
        || range.source_in < 0.0
        || range.source_out <= range.source_in
        || range.output_start < 0.0
    {
        return Err(CaptionGenerationError::InvalidSourceRange);
    }

    Ok(())
}

fn should_split_before(
    current_words: &[TranscriptWord],
    next_word: &TranscriptWord,
    options: &CaptionGenerationOptions,
) -> bool {
    let Some(first_word) = current_words.first() else {
        return false;
    };
    let Some(previous_word) = current_words.last() else {
        return false;
    };

    let gap = next_word.start_seconds - previous_word.end_seconds;
    if gap > options.pause_split_threshold_seconds {
        return true;
    }

    let word_count = current_words
        .iter()
        .filter(|word| !is_punctuation(&word.text))
        .count()
        + usize::from(!is_punctuation(&next_word.text));
    if word_count > options.max_words_per_cue {
        return true;
    }

    let mut candidate = current_words.to_vec();
    candidate.push(next_word.clone());
    if join_words(&candidate).chars().count() > options.max_chars_per_cue {
        return true;
    }

    next_word.end_seconds - first_word.start_seconds > options.max_duration_seconds
}

fn push_cue(
    cues: &mut Vec<CaptionCue>,
    range: &CaptionSourceRange,
    words: &mut Vec<TranscriptWord>,
    options: &CaptionGenerationOptions,
) {
    if words.is_empty() {
        return;
    }

    let text = join_words(words);
    if text.is_empty()
        || text
            .chars()
            .all(|character| character.is_ascii_punctuation())
    {
        words.clear();
        return;
    }

    let source_start = words
        .iter()
        .find(|word| !is_punctuation(&word.text))
        .map(|word| word.start_seconds.max(range.source_in))
        .unwrap_or(range.source_in);
    let source_end = words
        .iter()
        .rev()
        .find(|word| word.end_seconds > word.start_seconds)
        .map(|word| word.end_seconds.min(range.source_out))
        .unwrap_or(source_start);
    let clamped_source_end = source_end.max(source_start);
    let output_start = range.output_start + (source_start - range.source_in).max(0.0);
    let output_range_end = range.output_start + (range.source_out - range.source_in);
    let natural_duration = (clamped_source_end - source_start).max(0.1);
    let duration = natural_duration
        .max(options.min_duration_seconds)
        .min(options.max_duration_seconds)
        .min((output_range_end - output_start).max(0.1));

    cues.push(CaptionCue {
        media_id: range.media_id.clone(),
        text: text.clone(),
        line_breaks: break_lines(&text, options.max_chars_per_line),
        start_seconds: round_millis(output_start),
        duration_seconds: round_millis(duration),
        source_in: round_millis(source_start),
        source_out: round_millis(clamped_source_end),
        style: CaptionStyleMetadata::default(),
    });
    words.clear();
}

fn join_words(words: &[TranscriptWord]) -> String {
    let mut text = String::new();
    for word in words {
        let word_text = word.text.trim();
        if word_text.is_empty() {
            continue;
        }

        if is_punctuation(word_text) {
            text.push_str(word_text);
        } else {
            if !text.is_empty() {
                text.push(' ');
            }
            text.push_str(word_text);
        }
    }

    text.split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .replace(" ,", ",")
        .replace(" .", ".")
        .replace(" !", "!")
        .replace(" ?", "?")
        .replace(" ;", ";")
        .replace(" :", ":")
}

fn break_lines(text: &str, max_chars_per_line: usize) -> Vec<String> {
    if text.chars().count() <= max_chars_per_line {
        return vec![text.to_string()];
    }

    let mut lines = Vec::new();
    let mut current = String::new();
    for word in text.split_whitespace() {
        let next_len = if current.is_empty() {
            word.chars().count()
        } else {
            current.chars().count() + 1 + word.chars().count()
        };

        if next_len > max_chars_per_line && !current.is_empty() {
            lines.push(current);
            current = word.to_string();
        } else {
            if !current.is_empty() {
                current.push(' ');
            }
            current.push_str(word);
        }
    }

    if !current.is_empty() {
        lines.push(current);
    }

    if lines.len() <= 2 {
        lines
    } else {
        vec![lines[0].clone(), lines[1..].join(" ")]
    }
}

fn is_sentence_ending(text: &str) -> bool {
    matches!(text.trim(), "." | "!" | "?")
        || text.trim().ends_with('.')
        || text.trim().ends_with('!')
        || text.trim().ends_with('?')
}

fn is_punctuation(text: &str) -> bool {
    matches!(text.trim(), "." | "," | ";" | ":" | "!" | "?")
}

fn round_millis(value: f64) -> f64 {
    (value * 1000.0).round() / 1000.0
}
