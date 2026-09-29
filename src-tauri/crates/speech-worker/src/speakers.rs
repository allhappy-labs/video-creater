//! Per-segment speaker embedding and confidence helpers for the diarization output.

/// Speaker embedding extraction needs enough context; very short diarization turns are
/// widened symmetrically (within the recording) to this length before embedding.
pub const MIN_EMBEDDING_SECONDS: f64 = 1.5;

/// Names clusters `S1`, `S2`, ... in order of first appearance (sherpa-onnx cluster indices are not
/// guaranteed to be contiguous or chronological).
pub fn order_of_appearance_labels(clusters: &[i32]) -> Vec<String> {
    let mut seen = Vec::new();
    clusters
        .iter()
        .map(|cluster| {
            let index = match seen.iter().position(|known| known == cluster) {
                Some(index) => index,
                None => {
                    seen.push(*cluster);
                    seen.len() - 1
                }
            };
            format!("S{}", index + 1)
        })
        .collect()
}

/// Sample range used to embed a diarization turn `[start, end)` seconds.
pub fn embedding_window(
    start_seconds: f64,
    end_seconds: f64,
    total_samples: usize,
    sample_rate: u32,
) -> (usize, usize) {
    let rate = f64::from(sample_rate);
    let total_seconds = total_samples as f64 / rate;
    let mut start = start_seconds.clamp(0.0, total_seconds);
    let mut end = end_seconds.clamp(start, total_seconds);
    let missing = MIN_EMBEDDING_SECONDS - (end - start);
    if missing > 0.0 {
        start = (start - missing / 2.0).max(0.0);
        end = (start + MIN_EMBEDDING_SECONDS).min(total_seconds);
        start = (end - MIN_EMBEDDING_SECONDS).max(0.0);
    }
    let first = ((start * rate).floor() as usize).min(total_samples);
    let last = ((end * rate).ceil() as usize).clamp(first, total_samples);
    (first, last)
}

pub fn l2_normalized(values: &[f32]) -> Vec<f64> {
    let norm = values
        .iter()
        .map(|value| f64::from(*value) * f64::from(*value))
        .sum::<f64>()
        .sqrt();
    if !norm.is_finite() || norm <= 1e-12 {
        return values.iter().map(|value| f64::from(*value)).collect();
    }
    values
        .iter()
        .map(|value| f64::from(*value) / norm)
        .collect()
}

pub fn cosine_similarity(left: &[f64], right: &[f64]) -> f64 {
    if left.len() != right.len() || left.is_empty() {
        return 0.0;
    }
    let dot = left.iter().zip(right).map(|(a, b)| a * b).sum::<f64>();
    let left_norm = left.iter().map(|value| value * value).sum::<f64>().sqrt();
    let right_norm = right.iter().map(|value| value * value).sum::<f64>().sqrt();
    if left_norm <= 1e-12 || right_norm <= 1e-12 {
        return 0.0;
    }
    dot / (left_norm * right_norm)
}

pub fn mean_vector(vectors: &[&[f64]]) -> Vec<f64> {
    let Some(first) = vectors.first() else {
        return Vec::new();
    };
    let mut mean = vec![0.0; first.len()];
    for vector in vectors {
        for (sum, value) in mean.iter_mut().zip(vector.iter()) {
            *sum += value;
        }
    }
    for value in &mut mean {
        *value /= vectors.len() as f64;
    }
    mean
}

/// Maps sherpa-onnx's mean silhouette (`[-1, 1]`, or `-2` when unavailable) to a `[0, 1]`
/// quality score. When no silhouette exists (for example a single detected speaker), the cosine
/// similarity between the turn's embedding and its speaker centroid is used instead.
pub fn segment_confidence(silhouette: f32, embedding: &[f64], centroid: &[f64]) -> f64 {
    if silhouette.is_finite() && (-1.0..=1.0).contains(&silhouette) {
        return ((f64::from(silhouette) + 1.0) / 2.0).clamp(0.0, 1.0);
    }
    cosine_similarity(embedding, centroid).clamp(0.0, 1.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn labels_and_confidence_are_stable_and_bounded() {
        assert_eq!(
            order_of_appearance_labels(&[3, 0, 3, 7, 0]),
            ["S1", "S2", "S1", "S3", "S2"]
        );
        assert!(order_of_appearance_labels(&[]).is_empty());
        assert_eq!(segment_confidence(1.0, &[], &[]), 1.0);
        assert_eq!(segment_confidence(-1.0, &[], &[]), 0.0);
        assert!((segment_confidence(0.5, &[], &[]) - 0.75).abs() < 1e-12);
        assert!((segment_confidence(-2.0, &[1.0, 0.0], &[1.0, 0.0]) - 1.0).abs() < 1e-12);
        assert_eq!(segment_confidence(-2.0, &[1.0, 0.0], &[-1.0, 0.0]), 0.0);
    }

    #[test]
    fn short_turns_are_widened_within_the_recording() {
        let rate = 16_000;
        let total = rate as usize * 10;
        assert_eq!(embedding_window(2.0, 6.0, total, rate), (32_000, 96_000));
        let (start, end) = embedding_window(5.0, 5.5, total, rate);
        assert_eq!(end - start, 24_000);
        assert_eq!(start, 72_000);
        assert_eq!(embedding_window(0.1, 0.3, total, rate), (0, 24_000));
        assert_eq!(embedding_window(9.9, 10.5, total, rate), (136_000, total));
    }

    #[test]
    fn vectors_are_normalized_and_averaged() {
        let normalized = l2_normalized(&[3.0, 4.0]);
        assert!((normalized[0] - 0.6).abs() < 1e-12 && (normalized[1] - 0.8).abs() < 1e-12);
        assert_eq!(l2_normalized(&[0.0, 0.0]), vec![0.0, 0.0]);
        assert_eq!(mean_vector(&[&[1.0, 3.0], &[3.0, 5.0]]), vec![2.0, 4.0]);
        assert!(mean_vector(&[]).is_empty());
    }
}
