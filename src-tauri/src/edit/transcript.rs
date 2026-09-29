use crate::project::model::{Transcript, TranscriptSegment, TranscriptWord};
use serde::Deserialize;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum TranscriptParseError {
    #[error("parakeet transcript tokens are missing")]
    MissingTokens,
    #[error("transcript artifact has no usable tokens after cleaning")]
    NoUsableTokens,
    #[error("failed to parse transcript artifact: {0}")]
    InvalidShape(serde_json::Error),
    #[error("unsupported transcript artifact schema version: {0}")]
    UnsupportedSchemaVersion(u32),
    #[error("transcript artifact metadata field is blank: {0}")]
    BlankMetadata(&'static str),
}

#[derive(Debug, Deserialize)]
struct ParakeetToken {
    token: String,
    start: f64,
    end: f64,
}

#[derive(Debug, Deserialize)]
struct ParakeetOutput {
    model: Option<String>,
    tokens: Option<Vec<ParakeetToken>>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct TranscriptArtifactToken {
    token: String,
    start: f64,
    end: f64,
    confidence: Option<f64>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct TranscriptArtifact {
    schema_version: u32,
    media_id: String,
    engine: String,
    model_id: String,
    runtime_id: String,
    language_mode: String,
    tokens: Vec<TranscriptArtifactToken>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TranscriptReplacement {
    pub originals: Vec<String>,
    pub replacement: String,
}

pub fn parse_parakeet_transcript(
    media_id: &str,
    value: &serde_json::Value,
) -> Result<Transcript, TranscriptParseError> {
    let output: ParakeetOutput =
        serde_json::from_value(value.clone()).map_err(TranscriptParseError::InvalidShape)?;
    let tokens = output.tokens.ok_or(TranscriptParseError::MissingTokens)?;
    let engine = output
        .model
        .clone()
        .unwrap_or_else(|| "legacy_parakeet".to_string());
    let model_id = output
        .model
        .clone()
        .unwrap_or_else(|| "unknown".to_string());
    let artifact = serde_json::json!({
        "schemaVersion": 1,
        "mediaId": media_id,
        "engine": engine,
        "modelId": model_id,
        "runtimeId": "legacy_parakeet",
        "languageMode": "unknown",
        "tokens": tokens.into_iter().map(|token| serde_json::json!({
            "token": token.token,
            "start": token.start,
            "end": token.end,
            "confidence": null
        })).collect::<Vec<_>>()
    });

    parse_transcript_artifact(&artifact)
}

pub fn parse_transcript_artifact(
    value: &serde_json::Value,
) -> Result<Transcript, TranscriptParseError> {
    if let Some(schema_version) = value
        .get("schemaVersion")
        .and_then(serde_json::Value::as_u64)
    {
        let schema_version = u32::try_from(schema_version).unwrap_or(u32::MAX);
        if schema_version != 1 {
            return Err(TranscriptParseError::UnsupportedSchemaVersion(
                schema_version,
            ));
        }
    }

    let output: TranscriptArtifact =
        serde_json::from_value(value.clone()).map_err(TranscriptParseError::InvalidShape)?;
    if output.schema_version != 1 {
        return Err(TranscriptParseError::UnsupportedSchemaVersion(
            output.schema_version,
        ));
    }
    validate_transcript_artifact_metadata(&output)?;
    if output.tokens.is_empty() {
        return Err(TranscriptParseError::MissingTokens);
    }

    let words = output
        .tokens
        .into_iter()
        .filter_map(|token| {
            let text = clean_transcript_text(&token.token);
            if text.is_empty() {
                return None;
            }

            Some(TranscriptWord {
                text,
                start_seconds: token.start,
                end_seconds: token.end,
                confidence: token.confidence,
                speaker: None,
            })
        })
        .collect::<Vec<_>>();
    if words.is_empty() {
        return Err(TranscriptParseError::NoUsableTokens);
    }

    let segments = group_words_into_caption_segments(&words);

    Ok(Transcript {
        id: format!("transcript-{}", output.media_id),
        media_id: output.media_id,
        engine: Some(output.engine),
        raw_artifact_path: None,
        repairs: Vec::new(),
        segments,
        words,
    })
}

fn validate_transcript_artifact_metadata(
    output: &TranscriptArtifact,
) -> Result<(), TranscriptParseError> {
    for (field_name, value) in [
        ("mediaId", output.media_id.as_str()),
        ("engine", output.engine.as_str()),
        ("modelId", output.model_id.as_str()),
        ("runtimeId", output.runtime_id.as_str()),
        ("languageMode", output.language_mode.as_str()),
    ] {
        if value.trim().is_empty() {
            return Err(TranscriptParseError::BlankMetadata(field_name));
        }
    }

    Ok(())
}

pub fn clean_transcript_text(text: &str) -> String {
    let mut cleaned = text.to_string();
    for (open, close) in [('[', ']'), ('(', ')'), ('{', '}')] {
        cleaned = strip_balanced_markers(&cleaned, open, close);
    }
    cleaned = strip_xmlish_blocks(&cleaned);
    cleaned = cleaned
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .replace(" ,", ",")
        .replace(" .", ".")
        .replace(" !", "!")
        .replace(" ?", "?")
        .replace(" ;", ";")
        .replace(" :", ":");
    cleaned.trim().to_string()
}

pub fn apply_transcript_replacements(text: &str, replacements: &[TranscriptReplacement]) -> String {
    let mut output = text.to_string();
    let mut variants = replacements
        .iter()
        .flat_map(|replacement| {
            replacement
                .originals
                .iter()
                .map(|original| (original.trim().to_string(), replacement.replacement.clone()))
                .collect::<Vec<_>>()
        })
        .filter(|(original, _)| !original.is_empty())
        .collect::<Vec<_>>();

    variants.sort_by_key(|variant| std::cmp::Reverse(variant.0.chars().count()));
    for (original, replacement) in variants {
        output = replace_case_insensitive(&output, &original, &replacement);
    }
    output
}

pub fn group_words_into_caption_segments(words: &[TranscriptWord]) -> Vec<TranscriptSegment> {
    let mut segments = Vec::new();
    let mut current: Vec<TranscriptWord> = Vec::new();

    for word in words {
        if let Some(previous) = current.last() {
            let gap = word.start_seconds - previous.end_seconds;
            if gap > 0.5 && !current.is_empty() {
                push_segment(&mut segments, &mut current);
            }
        }

        current.push(word.clone());

        if matches!(word.text.as_str(), "." | "!" | "?") {
            push_segment(&mut segments, &mut current);
        }
    }

    push_segment(&mut segments, &mut current);
    segments
}

fn push_segment(segments: &mut Vec<TranscriptSegment>, words: &mut Vec<TranscriptWord>) {
    if words.is_empty() {
        return;
    }

    let text = clean_transcript_text(&join_words(words));
    if text.is_empty() {
        words.clear();
        return;
    }

    let start_seconds = words
        .iter()
        .find(|word| !is_punctuation(&word.text))
        .map(|word| word.start_seconds)
        .unwrap_or(words[0].start_seconds);
    let end_seconds = words
        .iter()
        .rev()
        .find(|word| word.end_seconds > word.start_seconds)
        .map(|word| word.end_seconds)
        .unwrap_or_else(|| {
            words
                .last()
                .map(|word| word.end_seconds)
                .unwrap_or(start_seconds)
        });

    segments.push(TranscriptSegment {
        text,
        start_seconds,
        end_seconds: end_seconds.max(start_seconds + 0.1),
    });
    words.clear();
}

fn join_words(words: &[TranscriptWord]) -> String {
    let mut text = String::new();
    for word in words {
        if is_punctuation(&word.text) {
            text.push_str(&word.text);
        } else {
            if !text.is_empty() {
                text.push(' ');
            }
            text.push_str(&word.text);
        }
    }
    text.trim().to_string()
}

fn is_punctuation(text: &str) -> bool {
    matches!(text, "." | "," | ";" | ":" | "!" | "?")
}

fn strip_balanced_markers(text: &str, open: char, close: char) -> String {
    let mut output = String::new();
    let mut depth = 0usize;
    for character in text.chars() {
        if character == open {
            depth += 1;
            continue;
        }

        if character == close && depth > 0 {
            depth -= 1;
            continue;
        }

        if depth == 0 {
            output.push(character);
        }
    }
    output
}

fn strip_xmlish_blocks(text: &str) -> String {
    let mut output = String::new();
    let mut remaining = text;

    while let Some(open_index) = remaining.find('<') {
        output.push_str(&remaining[..open_index]);
        let tag_start = &remaining[open_index + 1..];
        let Some(tag_end_index) = tag_start.find('>') else {
            break;
        };

        let tag = tag_start[..tag_end_index].trim();
        let after_open_tag = &tag_start[tag_end_index + 1..];
        let tag_name = tag
            .trim_start_matches('/')
            .split_whitespace()
            .next()
            .unwrap_or("");

        if tag.starts_with('/') || tag_name.is_empty() {
            remaining = after_open_tag;
            continue;
        }

        let closing_tag = format!("</{tag_name}>");
        if let Some(close_index) = after_open_tag.find(&closing_tag) {
            remaining = &after_open_tag[close_index + closing_tag.len()..];
        } else {
            remaining = after_open_tag;
        }
    }

    output.push_str(remaining);
    output
}

fn replace_case_insensitive(text: &str, needle: &str, replacement: &str) -> String {
    let mut output = String::new();
    let text_lower = text.to_lowercase();
    let needle_lower = needle.to_lowercase();
    let mut cursor = 0usize;

    while let Some(relative_start) = text_lower[cursor..].find(&needle_lower) {
        let start = cursor + relative_start;
        let end = start + needle_lower.len();
        output.push_str(&text[cursor..start]);
        output.push_str(replacement);
        cursor = end;
    }

    output.push_str(&text[cursor..]);
    output
}
