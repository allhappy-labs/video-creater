//! Splits long recordings into bounded transcription windows.
//!
//! Parakeet's encoder uses full self-attention, so decoding a long file in one pass grows memory
//! quadratically. FluidAudio also processes long audio in windows. Here each window is at most
//! [`MAX_WINDOW_SECONDS`]; when a window must be cut, the cut is placed at the quietest 20 ms frame
//! within the final [`CUT_SEARCH_SECONDS`] so that words are rarely split.

pub const MAX_WINDOW_SECONDS: f64 = 30.0;
pub const CUT_SEARCH_SECONDS: f64 = 6.0;
const ENERGY_FRAME_SECONDS: f64 = 0.02;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Window {
    pub start: usize,
    pub end: usize,
}

pub fn transcription_windows(samples: &[f32], sample_rate: u32) -> Vec<Window> {
    let rate = f64::from(sample_rate);
    let max_len = (MAX_WINDOW_SECONDS * rate) as usize;
    let search_len = (CUT_SEARCH_SECONDS * rate) as usize;
    let frame_len = ((ENERGY_FRAME_SECONDS * rate) as usize).max(1);
    let mut windows = Vec::new();
    let mut start = 0;
    while start < samples.len() {
        let remaining = samples.len() - start;
        if remaining <= max_len {
            windows.push(Window {
                start,
                end: samples.len(),
            });
            break;
        }
        let hard_end = start + max_len;
        let search_start = hard_end.saturating_sub(search_len).max(start + frame_len);
        let mut best_end = hard_end;
        let mut best_energy = f64::INFINITY;
        let mut frame_start = search_start;
        while frame_start + frame_len <= hard_end {
            let energy = samples[frame_start..frame_start + frame_len]
                .iter()
                .map(|sample| f64::from(*sample) * f64::from(*sample))
                .sum::<f64>();
            // Prefer the latest frame among equally quiet ones to keep windows long.
            if energy <= best_energy {
                best_energy = energy;
                best_end = frame_start + frame_len / 2;
            }
            frame_start += frame_len;
        }
        windows.push(Window {
            start,
            end: best_end,
        });
        start = best_end;
    }
    windows
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn short_audio_is_a_single_window_and_empty_audio_has_none() {
        assert!(transcription_windows(&[], 16_000).is_empty());
        let samples = vec![0.1_f32; 16_000 * 12];
        assert_eq!(
            transcription_windows(&samples, 16_000),
            vec![Window {
                start: 0,
                end: samples.len()
            }]
        );
    }

    #[test]
    fn long_audio_is_cut_at_the_quiet_gap_and_covers_every_sample() {
        let rate = 16_000;
        let mut samples = vec![0.5_f32; rate * 70];
        // A silent gap at 27.0..27.2 s lies inside the 24..30 s search region.
        for sample in &mut samples[rate * 27..rate * 27 + rate / 5] {
            *sample = 0.0;
        }
        let windows = transcription_windows(&samples, rate as u32);
        assert!(windows.len() >= 3);
        assert_eq!(windows[0].start, 0);
        let first_cut = windows[0].end as f64 / rate as f64;
        assert!((27.0..27.2).contains(&first_cut), "cut at {first_cut}");
        for pair in windows.windows(2) {
            assert_eq!(pair[0].end, pair[1].start);
        }
        assert_eq!(windows.last().map(|window| window.end), Some(samples.len()));
        assert!(windows
            .iter()
            .all(|window| window.end - window.start <= rate * MAX_WINDOW_SECONDS as usize));
    }
}
