//! Silero VAD windowing that reproduces FluidAudio's 256 ms chunk probabilities.
//!
//! FluidAudio's `silero-vad-unified-256ms` Core ML model takes 64 context samples plus 4096 new
//! samples, runs Silero on 8 consecutive 512-sample frames (each prefixed by the previous 64
//! samples) while carrying the recurrent state, and aggregates the 8 frame probabilities with a
//! noisy-OR. The Linux helper runs the upstream Silero ONNX model frame by frame with the same
//! framing and aggregation.

use crate::protocol::VadProbabilitySegment;

pub const CHUNK_SAMPLES: usize = 4096;
pub const FRAME_SAMPLES: usize = 512;
pub const CONTEXT_SAMPLES: usize = 64;
pub const FRAMES_PER_CHUNK: usize = CHUNK_SAMPLES / FRAME_SAMPLES;

/// Noisy-OR: `1 - Π(1 - p)`.
pub fn noisy_or(probabilities: &[f32]) -> f64 {
    let product = probabilities
        .iter()
        .map(|probability| 1.0 - f64::from(probability.clamp(0.0, 1.0)))
        .product::<f64>();
    (1.0 - product).clamp(0.0, 1.0)
}

/// Yields the 576-sample model inputs for one 4096-sample chunk. Short final chunks are zero padded
/// (as FluidAudio does) and the returned context is the tail of the padded chunk.
pub fn chunk_frame_inputs(
    chunk: &[f32],
    context: &[f32; CONTEXT_SAMPLES],
) -> (
    Vec<[f32; CONTEXT_SAMPLES + FRAME_SAMPLES]>,
    [f32; CONTEXT_SAMPLES],
) {
    let mut padded = [0.0_f32; CHUNK_SAMPLES];
    let used = chunk.len().min(CHUNK_SAMPLES);
    padded[..used].copy_from_slice(&chunk[..used]);
    let mut frames = Vec::with_capacity(FRAMES_PER_CHUNK);
    let mut frame_context = *context;
    for frame in padded.chunks_exact(FRAME_SAMPLES) {
        let mut input = [0.0_f32; CONTEXT_SAMPLES + FRAME_SAMPLES];
        input[..CONTEXT_SAMPLES].copy_from_slice(&frame_context);
        input[CONTEXT_SAMPLES..].copy_from_slice(frame);
        frames.push(input);
        frame_context.copy_from_slice(&frame[FRAME_SAMPLES - CONTEXT_SAMPLES..]);
    }
    (frames, frame_context)
}

/// Converts per-chunk probabilities into timed segments exactly like the Swift
/// `validatedSpeechAnalysisResponse`: chunk `i` spans `[i*0.256, min(duration, (i+1)*0.256)]`
/// and chunks starting at or after the media duration are dropped.
pub fn vad_segments(
    probabilities: &[f64],
    duration_seconds: f64,
    sample_rate: u32,
) -> Vec<VadProbabilitySegment> {
    let chunk_duration = CHUNK_SAMPLES as f64 / f64::from(sample_rate);
    probabilities
        .iter()
        .enumerate()
        .filter_map(|(index, probability)| {
            let start = index as f64 * chunk_duration;
            if start >= duration_seconds {
                return None;
            }
            let end = duration_seconds.min(start + chunk_duration);
            (end > start && probability.is_finite()).then(|| VadProbabilitySegment {
                end_seconds: end,
                speech_probability: probability.clamp(0.0, 1.0),
                start_seconds: start,
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn noisy_or_matches_the_unified_core_ml_aggregation() {
        assert_eq!(noisy_or(&[0.0; 8]), 0.0);
        assert!((noisy_or(&[0.5, 0.5]) - 0.75).abs() < 1e-12);
        assert!((noisy_or(&[1.0, 0.1]) - 1.0).abs() < 1e-12);
        assert!((noisy_or(&[0.1; 8]) - (1.0 - 0.9_f64.powi(8))).abs() < 1e-6);
    }

    #[test]
    fn frames_carry_a_64_sample_context_across_frames_and_chunks() {
        let chunk = (0..CHUNK_SAMPLES)
            .map(|index| index as f32)
            .collect::<Vec<_>>();
        let context = [-1.0_f32; CONTEXT_SAMPLES];
        let (frames, next_context) = chunk_frame_inputs(&chunk, &context);
        assert_eq!(frames.len(), FRAMES_PER_CHUNK);
        assert_eq!(frames[0][0], -1.0);
        assert_eq!(frames[0][CONTEXT_SAMPLES], 0.0);
        assert_eq!(frames[1][0], 448.0);
        assert_eq!(frames[1][CONTEXT_SAMPLES], 512.0);
        assert_eq!(next_context[0], 4032.0);
        assert_eq!(next_context[CONTEXT_SAMPLES - 1], 4095.0);

        let (short_frames, short_context) = chunk_frame_inputs(&chunk[..1000], &next_context);
        assert_eq!(short_frames.len(), FRAMES_PER_CHUNK);
        assert_eq!(short_frames[0][0], 4032.0);
        assert_eq!(short_frames[1][CONTEXT_SAMPLES + 487], 999.0);
        assert_eq!(short_frames[1][CONTEXT_SAMPLES + 488], 0.0);
        assert_eq!(short_context, [0.0; CONTEXT_SAMPLES]);
    }

    #[test]
    fn segments_follow_256ms_chunks_and_clamp_to_duration() {
        let segments = vad_segments(&[0.9, 0.2, 0.7], 0.6, 16_000);
        assert_eq!(segments.len(), 3);
        assert_eq!(segments[0].start_seconds, 0.0);
        assert!((segments[0].end_seconds - 0.256).abs() < 1e-12);
        assert!((segments[2].start_seconds - 0.512).abs() < 1e-12);
        assert!((segments[2].end_seconds - 0.6).abs() < 1e-12);
        assert_eq!(vad_segments(&[0.9, 0.2, 0.7], 0.5, 16_000).len(), 2);
    }
}
