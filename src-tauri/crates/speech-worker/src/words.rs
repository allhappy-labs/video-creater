//! Token timing normalization and word assembly for Parakeet TDT output.

use serde::Deserialize;

use crate::protocol::Word;

/// Parakeet TDT encoder frames are 10 ms feature hops with 8x subsampling.
pub const PARAKEET_FRAME_SECONDS: f64 = 0.08;

#[derive(Debug, Clone, PartialEq)]
pub struct TokenTiming {
    pub token: String,
    pub start_seconds: f64,
    pub end_seconds: f64,
    pub confidence: f64,
}

/// The subset of sherpa-onnx's `SherpaOnnxGetOfflineStreamResultAsJson` payload used here.
/// `timestamps` and `durations` are seconds; `ys_log_probs` are natural-log token probabilities.
#[derive(Debug, Clone, Default, PartialEq, Deserialize)]
pub struct OfflineRecognizerJson {
    #[serde(default)]
    pub text: String,
    #[serde(default)]
    pub tokens: Vec<String>,
    #[serde(default)]
    pub timestamps: Vec<f64>,
    #[serde(default)]
    pub durations: Vec<f64>,
    #[serde(default)]
    pub ys_log_probs: Vec<f64>,
}

/// Converts one decoded chunk into absolute token timings.
///
/// Start times come from the TDT frame index. The end is the start plus the predicted TDT
/// duration (at least one encoder frame), clipped to the next token's start so tokens never
/// overlap; a token followed by another token on the same frame therefore has zero length. Confidence is the softmax
/// probability of the selected token (`exp(log_prob)`), matching FluidAudio's token confidence.
pub fn token_timings_from_result(
    result: &OfflineRecognizerJson,
    offset_seconds: f64,
) -> Vec<TokenTiming> {
    let count = result.tokens.len().min(result.timestamps.len());
    let mut timings = Vec::with_capacity(count);
    for index in 0..count {
        let start = offset_seconds + result.timestamps[index];
        let duration = result
            .durations
            .get(index)
            .copied()
            .filter(|duration| duration.is_finite())
            .unwrap_or(0.0)
            .max(PARAKEET_FRAME_SECONDS);
        let mut end = start + duration;
        if let Some(next_start) = result
            .timestamps
            .get(index + 1)
            .map(|next| offset_seconds + next)
        {
            if next_start >= start {
                end = end.min(next_start);
            }
        }
        let confidence = result
            .ys_log_probs
            .get(index)
            .copied()
            .filter(|log_prob| log_prob.is_finite())
            .map(f64::exp)
            .unwrap_or(0.0)
            .clamp(0.0, 1.0);
        if !start.is_finite() || !end.is_finite() {
            continue;
        }
        timings.push(TokenTiming {
            token: result.tokens[index].clone(),
            start_seconds: start,
            end_seconds: end.max(start),
            confidence,
        });
    }
    timings
}

/// Groups SentencePiece tokens into words exactly like the Swift helper's `words(from:)`:
/// a token containing `▁` or starting with a space begins a new word, text is the concatenation
/// of normalized pieces, timing spans the first and last token, and confidence is the mean token
/// confidence clamped to `0...1`.
pub fn words_from_tokens(tokens: &[TokenTiming]) -> Vec<Word> {
    let mut words = Vec::new();
    let mut current_text = String::new();
    let mut current_start = 0.0;
    let mut current_end = 0.0;
    let mut confidence_sum = 0.0;
    let mut confidence_count = 0_u32;

    let finish = |words: &mut Vec<Word>, text: &str, start: f64, end: f64, sum: f64, count: u32| {
        let text = text.trim();
        if text.is_empty() {
            return;
        }
        let confidence = if count > 0 {
            sum / f64::from(count)
        } else {
            0.0
        };
        words.push(Word {
            confidence: confidence.clamp(0.0, 1.0),
            end_seconds: end,
            start_seconds: start,
            text: text.to_string(),
        });
    };

    for timing in tokens {
        let raw = timing.token.as_str();
        let starts_new_word = raw.contains('\u{2581}') || raw.starts_with(' ');
        let normalized = raw.replace('\u{2581}', " ");
        let normalized = normalized.trim();
        if normalized.is_empty() {
            continue;
        }
        if starts_new_word && !current_text.trim().is_empty() {
            finish(
                &mut words,
                &current_text,
                current_start,
                current_end,
                confidence_sum,
                confidence_count,
            );
            current_text.clear();
            confidence_sum = 0.0;
            confidence_count = 0;
        }
        if current_text.is_empty() {
            current_start = timing.start_seconds;
        }
        current_text.push_str(normalized);
        current_end = timing.end_seconds;
        confidence_sum += timing.confidence;
        confidence_count += 1;
    }
    finish(
        &mut words,
        &current_text,
        current_start,
        current_end,
        confidence_sum,
        confidence_count,
    );
    words
}

#[cfg(test)]
mod tests {
    use super::*;

    fn timing(token: &str, start: f64, end: f64, confidence: f64) -> TokenTiming {
        TokenTiming {
            token: token.to_string(),
            start_seconds: start,
            end_seconds: end,
            confidence,
        }
    }

    #[test]
    fn merges_sentencepiece_pieces_into_words_like_the_swift_helper() {
        let words = words_from_tokens(&[
            timing("\u{2581}Hel", 0.0, 0.1, 0.8),
            timing("lo", 0.1, 0.2, 0.6),
            timing(",", 0.2, 0.24, 1.0),
            timing(" wor", 0.4, 0.5, 0.5),
            timing("ld", 0.5, 0.6, 0.7),
            timing("\u{2581}", 0.6, 0.64, 0.9),
            timing("\u{2581}again", 0.7, 0.9, 1.4),
        ]);
        assert_eq!(
            words,
            vec![
                Word {
                    confidence: (0.8 + 0.6 + 1.0) / 3.0,
                    end_seconds: 0.24,
                    start_seconds: 0.0,
                    text: "Hello,".to_string(),
                },
                Word {
                    confidence: 0.6,
                    end_seconds: 0.6,
                    start_seconds: 0.4,
                    text: "world".to_string(),
                },
                Word {
                    confidence: 1.0,
                    end_seconds: 0.9,
                    start_seconds: 0.7,
                    text: "again".to_string(),
                },
            ]
        );
    }

    #[test]
    fn leading_continuation_pieces_form_a_word_and_empty_input_yields_nothing() {
        assert!(words_from_tokens(&[]).is_empty());
        let words = words_from_tokens(&[
            timing("ing", 1.0, 1.1, 0.5),
            timing("\u{2581}ok", 1.2, 1.3, 0.5),
        ]);
        assert_eq!(words.len(), 2);
        assert_eq!(words[0].text, "ing");
        assert_eq!(words[1].text, "ok");
    }

    #[test]
    fn sherpa_results_become_offset_non_overlapping_token_timings() {
        let result: OfflineRecognizerJson = serde_json::from_str(
            r#"{"lang":"","emotion":"","event":"","text":" Hi there","timestamps":[0.32, 0.40, 0.40, 0.96],"durations":[0.08, 0.00, 0.40, 0.16],"tokens":["▁H","i","▁the","re"],"ys_log_probs":[-0.1, 0.0, -2.0, -0.5],"words":[]}"#,
        )
        .expect("sherpa result json");
        let timings = token_timings_from_result(&result, 30.0);
        assert_eq!(timings.len(), 4);
        assert!((timings[0].start_seconds - 30.32).abs() < 1e-9);
        assert!((timings[0].end_seconds - 30.40).abs() < 1e-9);
        // Tokens sharing a frame are clipped to zero length; the last token of a frame spans it.
        assert!((timings[1].end_seconds - 30.40).abs() < 1e-9);
        assert!((timings[2].end_seconds - 30.80).abs() < 1e-9);
        assert!((timings[3].end_seconds - 31.12).abs() < 1e-9);
        assert!((timings[0].confidence - (-0.1_f64).exp()).abs() < 1e-9);
        assert_eq!(timings[1].confidence, 1.0);
        let words = words_from_tokens(&timings);
        assert_eq!(
            words
                .iter()
                .map(|word| word.text.as_str())
                .collect::<Vec<_>>(),
            ["Hi", "there"]
        );
        assert!(words
            .windows(2)
            .all(|pair| pair[0].end_seconds <= pair[1].start_seconds));
    }
}
