use crate::edit::render_plan::{ExportEncodeTier, RenderQuality};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct RenderQualitySettings {
    pub output_width: u32,
    pub output_height: u32,
    pub output_fps: f64,
    pub video_bitrate_kbps: Option<u32>,
    pub speed_hint: String,
}

pub fn effective_quality_settings(
    quality: RenderQuality,
    width: u32,
    height: u32,
    fps: f64,
) -> RenderQualitySettings {
    effective_quality_settings_for_tier(quality, ExportEncodeTier::Standard, width, height, fps)
}

/// Quality settings including the encode tier. Master applies only to Final:
/// export options refuse Master for Draft before a render starts.
pub fn effective_quality_settings_for_tier(
    quality: RenderQuality,
    encode_tier: ExportEncodeTier,
    width: u32,
    height: u32,
    fps: f64,
) -> RenderQualitySettings {
    match (quality, encode_tier) {
        (RenderQuality::Final, ExportEncodeTier::Master) => RenderQualitySettings {
            output_width: width,
            output_height: height,
            output_fps: fps,
            video_bitrate_kbps: Some(master_video_bitrate_kbps(width, height, fps)),
            speed_hint: "master-quality".to_string(),
        },
        _ => standard_quality_settings(quality, width, height, fps),
    }
}

/// The Master bitrate ladder: twice the Final bitrate, within 12–60 Mbps.
pub fn master_video_bitrate_kbps(width: u32, height: u32, fps: f64) -> u32 {
    final_video_bitrate_kbps(width, height, fps)
        .saturating_mul(2)
        .clamp(12_000, 60_000)
}

fn standard_quality_settings(
    quality: RenderQuality,
    width: u32,
    height: u32,
    fps: f64,
) -> RenderQualitySettings {
    match quality {
        RenderQuality::Draft => draft_settings(width, height, fps),
        RenderQuality::Final => RenderQualitySettings {
            output_width: width,
            output_height: height,
            output_fps: fps,
            video_bitrate_kbps: Some(final_video_bitrate_kbps(width, height, fps)),
            speed_hint: "final-quality".to_string(),
        },
    }
}

fn final_video_bitrate_kbps(width: u32, height: u32, fps: f64) -> u32 {
    let pixels = u64::from(width.max(1)) * u64::from(height.max(1));
    let resolution_factor = pixels as f64 / (1920.0 * 1080.0);
    let fps_factor = if fps.is_finite() && fps > 0.0 {
        (fps / 30.0).clamp(1.0, 2.0)
    } else {
        1.0
    };
    (6_000.0 * resolution_factor * fps_factor)
        .round()
        .clamp(6_000.0, 24_000.0) as u32
}

fn draft_settings(width: u32, height: u32, fps: f64) -> RenderQualitySettings {
    let output_fps = if fps.is_finite() { fps.min(24.0) } else { 24.0 };
    RenderQualitySettings {
        output_width: width,
        output_height: height,
        output_fps,
        video_bitrate_kbps: Some(draft_video_bitrate_kbps(width, height, output_fps)),
        speed_hint: "draft-fast".to_string(),
    }
}

fn draft_video_bitrate_kbps(width: u32, height: u32, fps: f64) -> u32 {
    (final_video_bitrate_kbps(width, height, fps) as f64 * 0.35)
        .round()
        .clamp(1_200.0, 8_000.0) as u32
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::edit::render_plan::ExportEncodeTier;

    #[test]
    fn master_doubles_the_final_bitrate_with_a_floor() {
        let settings = effective_quality_settings_for_tier(
            RenderQuality::Final,
            ExportEncodeTier::Master,
            1920,
            1080,
            30.0,
        );
        assert_eq!(settings.video_bitrate_kbps, Some(12_000));
        assert_eq!(settings.speed_hint, "master-quality");
        assert_eq!(master_video_bitrate_kbps(1920, 1080, 30.0), 12_000);
    }

    #[test]
    fn master_ladder_follows_resolution_and_frame_rate() {
        let settings = effective_quality_settings_for_tier(
            RenderQuality::Final,
            ExportEncodeTier::Master,
            3840,
            2160,
            60.0,
        );
        assert_eq!(settings.video_bitrate_kbps, Some(48_000));
        assert_eq!(settings.output_fps, 60.0);
    }

    #[test]
    fn standard_tier_matches_the_existing_quality_settings() {
        for quality in [RenderQuality::Draft, RenderQuality::Final] {
            assert_eq!(
                effective_quality_settings_for_tier(
                    quality,
                    ExportEncodeTier::Standard,
                    1920,
                    1080,
                    60.0
                ),
                effective_quality_settings(quality, 1920, 1080, 60.0)
            );
        }
    }
}
