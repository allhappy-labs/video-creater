//! Canonical frame math for clip transitions.
//!
//! The same window math as the editor preview
//! (`src/lib/preview/transition-frame.ts`) and GES rendering
//! (`render_pipeline/gstreamer_transitions.rs`): a transition of `d` seconds
//! spans `[start, start + d)` with progress `p = (t - start) / d`.
//!
//! - Crossfade: outgoing factor 1, incoming factor `p`.
//! - Dip to black or white: an opaque solid beneath both clips for the whole
//!   window; outgoing factor `max(0, 1 - 2p)`, incoming `max(0, 2p - 1)`.
//! - Wipe: both factors 1; the incoming clip is masked in output canvas
//!   coordinates so pixel column `x` shows when `x < p * W`.
//!
//! The factor dissolves a layer in encoded sRGB ([`dissolve_rgba8_srgb`]),
//! like CSS opacity in the DOM preview and the GES compositor's alpha. The
//! layer's own opacity and blend mode still composite in linear light.

use crate::project::model::TransitionKind;
use serde::Serialize;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum TransitionRole {
    Outgoing,
    Incoming,
}

/// A transition window in timeline seconds.
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TransitionWindow {
    pub kind: TransitionKind,
    pub start_seconds: f64,
    pub duration_seconds: f64,
}

impl TransitionWindow {
    pub fn end_seconds(&self) -> f64 {
        self.start_seconds + self.duration_seconds
    }

    /// Whether `seconds` is inside `[start, start + d)`.
    pub fn contains(&self, seconds: f64) -> bool {
        seconds >= self.start_seconds && seconds < self.end_seconds()
    }

    /// `p = (t - start) / d`, clamped to `[0, 1]`.
    pub fn progress(&self, seconds: f64) -> f64 {
        ((seconds - self.start_seconds) / self.duration_seconds).clamp(0.0, 1.0)
    }
}

/// How one clip takes part in a transition at a frame.
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TransitionFrameState {
    pub kind: TransitionKind,
    pub role: TransitionRole,
    pub progress: f64,
    /// The per-kind opacity factor.
    pub opacity: f64,
    /// Wipe incoming clips only: the fraction of the canvas width hidden from the right.
    pub wipe_inset_right: Option<f64>,
}

impl TransitionFrameState {
    pub fn new(kind: TransitionKind, role: TransitionRole, progress: f64) -> Self {
        let progress = progress.clamp(0.0, 1.0);
        let opacity = match (kind, role) {
            (TransitionKind::Crossfade, TransitionRole::Incoming) => progress,
            (TransitionKind::DipToBlack | TransitionKind::DipToWhite, TransitionRole::Outgoing) => {
                (1.0 - 2.0 * progress).max(0.0)
            }
            (TransitionKind::DipToBlack | TransitionKind::DipToWhite, TransitionRole::Incoming) => {
                (2.0 * progress - 1.0).max(0.0)
            }
            (TransitionKind::Crossfade | TransitionKind::Wipe, TransitionRole::Outgoing)
            | (TransitionKind::Wipe, TransitionRole::Incoming) => 1.0,
        };
        let wipe_inset_right = (kind == TransitionKind::Wipe && role == TransitionRole::Incoming)
            .then_some(1.0 - progress);
        Self {
            kind,
            role,
            progress,
            opacity,
            wipe_inset_right,
        }
    }

    /// The transition state of a window at `seconds`, or `None` outside it.
    pub fn at(window: &TransitionWindow, role: TransitionRole, seconds: f64) -> Option<Self> {
        window
            .contains(seconds)
            .then(|| Self::new(window.kind, role, window.progress(seconds)))
    }
}

/// The opaque solid a dip draws beneath both clips, or `None` for other kinds.
pub fn transition_solid_rgba8(kind: TransitionKind) -> Option<[u8; 4]> {
    match kind {
        TransitionKind::DipToBlack => Some([0, 0, 0, 255]),
        TransitionKind::DipToWhite => Some([255, 255, 255, 255]),
        TransitionKind::Crossfade | TransitionKind::Wipe => None,
    }
}

/// Whether canvas pixel column `x` shows through a wipe at `progress`: `x < p * W`.
pub fn wipe_column_visible(x: u32, width: u32, progress: f64) -> bool {
    f64::from(x) < progress * f64::from(width)
}

/// Hides every canvas column of a straight-alpha RGBA frame that the wipe has
/// not revealed yet.
pub fn apply_wipe_mask_rgba8(frame: &mut [u8], width: u32, height: u32, progress: f64) {
    let width_usize = width as usize;
    if width_usize == 0 || frame.len() != width_usize * height as usize * 4 {
        return;
    }
    let first_hidden = (0..width)
        .find(|x| !wipe_column_visible(*x, width, progress))
        .map_or(width_usize, |x| x as usize);
    for row in frame.chunks_exact_mut(width_usize * 4) {
        row[first_hidden * 4..].fill(0);
    }
}

/// Mixes `target` over `backdrop` by `amount` in encoded sRGB with
/// premultiplied alpha: `backdrop + (target - backdrop) * amount`. `target`
/// is the backdrop with the layer already composited at full strength, so an
/// amount of 1 returns it unchanged and 0 returns the backdrop.
pub fn dissolve_rgba8_srgb(backdrop: &[u8], target: &[u8], amount: f64) -> Vec<u8> {
    let amount = amount.clamp(0.0, 1.0);
    if amount >= 1.0 || backdrop.len() != target.len() {
        return target.to_vec();
    }
    if amount <= 0.0 {
        return backdrop.to_vec();
    }
    let mut output = vec![0_u8; target.len()];
    for ((out, back), over) in output
        .chunks_exact_mut(4)
        .zip(backdrop.chunks_exact(4))
        .zip(target.chunks_exact(4))
    {
        let back_alpha = f64::from(back[3]) / 255.0;
        let over_alpha = f64::from(over[3]) / 255.0;
        let alpha = back_alpha + (over_alpha - back_alpha) * amount;
        if alpha * 255.0 < 0.5 {
            continue;
        }
        for channel in 0..3 {
            let back_premultiplied = f64::from(back[channel]) * back_alpha;
            let over_premultiplied = f64::from(over[channel]) * over_alpha;
            let mixed = back_premultiplied + (over_premultiplied - back_premultiplied) * amount;
            out[channel] = (mixed / alpha).round().clamp(0.0, 255.0) as u8;
        }
        out[3] = (alpha * 255.0).round().clamp(0.0, 255.0) as u8;
    }
    output
}

#[cfg(test)]
mod tests {
    use super::*;

    fn window(kind: TransitionKind) -> TransitionWindow {
        // A 1 s transition at the 4 s cut spans [3.5, 4.5).
        TransitionWindow {
            kind,
            start_seconds: 3.5,
            duration_seconds: 1.0,
        }
    }

    #[test]
    fn window_contains_its_start_but_not_its_end() {
        let window = window(TransitionKind::Crossfade);
        assert!(window.contains(3.5));
        assert!(window.contains(4.49));
        assert!(!window.contains(4.5));
        assert!(!window.contains(3.4));
        assert_eq!(window.progress(3.5), 0.0);
        assert_eq!(window.progress(4.0), 0.5);
        assert_eq!(window.progress(3.75), 0.25);
    }

    #[test]
    fn crossfade_keeps_outgoing_opaque_and_fades_incoming_in() {
        let at = |role, seconds| {
            TransitionFrameState::at(&window(TransitionKind::Crossfade), role, seconds)
                .expect("inside window")
        };
        assert_eq!(at(TransitionRole::Outgoing, 4.0).opacity, 1.0);
        assert_eq!(at(TransitionRole::Incoming, 4.0).opacity, 0.5);
        assert_eq!(at(TransitionRole::Incoming, 3.75).opacity, 0.25);
        assert_eq!(at(TransitionRole::Outgoing, 4.25).opacity, 1.0);
        assert_eq!(at(TransitionRole::Incoming, 3.5).opacity, 0.0);
        assert_eq!(at(TransitionRole::Outgoing, 4.0).wipe_inset_right, None);
    }

    #[test]
    fn dips_hide_both_clips_at_the_cut_over_a_solid() {
        let dip = window(TransitionKind::DipToBlack);
        let opacity = |role, seconds| {
            TransitionFrameState::at(&dip, role, seconds)
                .expect("inside window")
                .opacity
        };
        assert_eq!(opacity(TransitionRole::Outgoing, 4.0), 0.0);
        assert_eq!(opacity(TransitionRole::Incoming, 4.0), 0.0);
        assert_eq!(opacity(TransitionRole::Outgoing, 3.75), 0.5);
        assert_eq!(opacity(TransitionRole::Incoming, 3.75), 0.0);
        assert_eq!(opacity(TransitionRole::Incoming, 4.25), 0.5);
        assert_eq!(
            transition_solid_rgba8(TransitionKind::DipToBlack),
            Some([0, 0, 0, 255])
        );
        assert_eq!(
            transition_solid_rgba8(TransitionKind::DipToWhite),
            Some([255, 255, 255, 255])
        );
        assert_eq!(transition_solid_rgba8(TransitionKind::Wipe), None);
    }

    #[test]
    fn wipe_masks_the_incoming_clip_in_canvas_columns() {
        let wipe = window(TransitionKind::Wipe);
        let incoming =
            TransitionFrameState::at(&wipe, TransitionRole::Incoming, 4.0).expect("inside");
        assert_eq!(incoming.opacity, 1.0);
        assert_eq!(incoming.wipe_inset_right, Some(0.5));
        let outgoing =
            TransitionFrameState::at(&wipe, TransitionRole::Outgoing, 4.0).expect("inside");
        assert_eq!(outgoing.wipe_inset_right, None);
        assert_eq!(
            TransitionFrameState::at(&wipe, TransitionRole::Incoming, 3.75)
                .and_then(|state| state.wipe_inset_right),
            Some(0.75)
        );

        let mut frame = vec![200_u8; 4 * 4 * 2];
        apply_wipe_mask_rgba8(&mut frame, 4, 2, 0.5);
        for row in frame.chunks_exact(16) {
            assert_eq!(&row[..8], &[200; 8]);
            assert_eq!(&row[8..], &[0; 8]);
        }
        assert!(wipe_column_visible(63, 128, 0.5));
        assert!(!wipe_column_visible(64, 128, 0.5));
    }

    #[test]
    fn dissolve_mixes_in_encoded_srgb() {
        let red = [250, 0, 0, 255];
        let blue = [0, 0, 248, 255];
        assert_eq!(
            dissolve_rgba8_srgb(&red, &blue, 0.5),
            vec![125, 0, 124, 255]
        );
        assert_eq!(dissolve_rgba8_srgb(&red, &blue, 1.0), blue.to_vec());
        assert_eq!(dissolve_rgba8_srgb(&red, &blue, 0.0), red.to_vec());
        // Over a transparent canvas the layer keeps its colour and takes the amount as alpha.
        assert_eq!(
            dissolve_rgba8_srgb(&[0, 0, 0, 0], &blue, 0.25),
            vec![0, 0, 248, 64]
        );
        assert_eq!(dissolve_rgba8_srgb(&[0, 0, 0, 0], &blue, 0.0), vec![0; 4]);
    }
}
