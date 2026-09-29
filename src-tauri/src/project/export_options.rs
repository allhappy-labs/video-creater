use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::edit::render_plan::{ExportEncodeTier, RenderQuality};
use crate::project::export_destination::ExportOutputRequest;
use crate::project::export_profiles::ExportProfile;

/// Export settings recorded on an export job, so Retry after a restart can
/// restore the format, quality, resolution, frame rate, name and folder.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct JobExportSettings {
    #[serde(flatten)]
    pub options: ExportRenderOptions,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub output: Option<ExportOutputRequest>,
}

impl JobExportSettings {
    /// The settings an in-process export job records for Retry: the editor's `requested` settings
    /// (its chosen resolution and frame rate, before Draft reduces them) when they are valid and
    /// describe the same profile, quality and tier, else the rendered `options`.
    pub fn for_export(
        options: ExportRenderOptions,
        requested: Option<JobExportSettings>,
        output: ExportOutputRequest,
    ) -> Self {
        let options = requested
            .and_then(|requested| requested.options.validated().ok())
            .filter(|requested| {
                requested.profile == options.profile
                    && requested.quality == options.quality
                    && requested.encode_tier == options.encode_tier
            })
            .unwrap_or(options);
        Self {
            options,
            output: Some(output),
        }
    }
}

const MAX_EXPORT_DIMENSION: u32 = 16_384;
const DRAFT_MAX_WIDTH: u32 = 1_280;
const DRAFT_MAX_HEIGHT: u32 = 720;
const MIN_EXPORT_FPS: f64 = 1.0;
const MAX_EXPORT_FPS: f64 = 120.0;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ExportRenderOptions {
    pub profile: ExportProfile,
    pub quality: RenderQuality,
    pub width: u32,
    pub height: u32,
    /// Output frame rate. `None` renders at the timeline rate.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fps: Option<f64>,
    #[serde(default, skip_serializing_if = "ExportEncodeTier::is_standard")]
    pub encode_tier: ExportEncodeTier,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum ExportRenderOptionsError {
    #[error("export dimensions must be positive")]
    ZeroDimension,
    #[error("export dimensions must be even")]
    OddDimension,
    #[error("export dimensions cannot exceed {MAX_EXPORT_DIMENSION} pixels")]
    DimensionTooLarge,
    #[error("project bundles are not renderable export options")]
    ProjectBundle,
    #[error("Choose a frame rate between 1 and 120 fps.")]
    InvalidFrameRate,
    #[error("Master quality needs Final quality.")]
    MasterNeedsFinalQuality,
    #[error(
        "Master quality is for MP4 and WebM exports; ProRes already exports at mastering quality."
    )]
    MasterUnsupportedProfile,
}

impl ExportRenderOptions {
    pub fn new(
        profile: ExportProfile,
        quality: RenderQuality,
        width: u32,
        height: u32,
    ) -> Result<Self, ExportRenderOptionsError> {
        if profile == ExportProfile::PalmierProject {
            return Err(ExportRenderOptionsError::ProjectBundle);
        }
        if width == 0 || height == 0 {
            return Err(ExportRenderOptionsError::ZeroDimension);
        }
        if !width.is_multiple_of(2) || !height.is_multiple_of(2) {
            return Err(ExportRenderOptionsError::OddDimension);
        }
        if width > MAX_EXPORT_DIMENSION || height > MAX_EXPORT_DIMENSION {
            return Err(ExportRenderOptionsError::DimensionTooLarge);
        }

        Ok(Self {
            profile,
            quality,
            width,
            height,
            fps: None,
            encode_tier: ExportEncodeTier::Standard,
        })
    }

    pub fn with_fps(self, fps: Option<f64>) -> Result<Self, ExportRenderOptionsError> {
        if let Some(fps) = fps {
            if !fps.is_finite() || !(MIN_EXPORT_FPS..=MAX_EXPORT_FPS).contains(&fps) {
                return Err(ExportRenderOptionsError::InvalidFrameRate);
            }
        }
        Ok(Self { fps, ..self })
    }

    pub fn with_encode_tier(
        self,
        encode_tier: ExportEncodeTier,
    ) -> Result<Self, ExportRenderOptionsError> {
        if encode_tier == ExportEncodeTier::Master {
            if self.quality != RenderQuality::Final {
                return Err(ExportRenderOptionsError::MasterNeedsFinalQuality);
            }
            if !matches!(
                self.profile,
                ExportProfile::Mp4H264 | ExportProfile::Mp4H265 | ExportProfile::Webm
            ) {
                return Err(ExportRenderOptionsError::MasterUnsupportedProfile);
            }
        }
        Ok(Self {
            encode_tier,
            ..self
        })
    }

    /// Re-validates options that arrived through serde, so a stored or sent
    /// value obeys the same rules as the builders.
    pub fn validated(self) -> Result<Self, ExportRenderOptionsError> {
        Self::new(self.profile, self.quality, self.width, self.height)?
            .with_fps(self.fps)?
            .with_encode_tier(self.encode_tier)
    }

    pub fn effective_fps(&self, project_fps: f64) -> f64 {
        self.fps.unwrap_or(project_fps)
    }

    pub fn for_project_defaults(
        profile: ExportProfile,
        quality: RenderQuality,
        source_width: u32,
        source_height: u32,
    ) -> Result<Self, ExportRenderOptionsError> {
        let (width, height) = match quality {
            RenderQuality::Draft => draft_dimensions(source_width, source_height),
            RenderQuality::Final => (source_width, source_height),
        };
        Self::new(profile, quality, width, height)
    }
}

pub fn draft_dimensions(source_width: u32, source_height: u32) -> (u32, u32) {
    if source_width == 0 || source_height == 0 {
        return (source_width, source_height);
    }
    if source_width < 2 || source_height < 2 {
        return (source_width, source_height);
    }
    if source_width <= DRAFT_MAX_WIDTH && source_height <= DRAFT_MAX_HEIGHT {
        return (even_dimension(source_width), even_dimension(source_height));
    }

    let scale = (DRAFT_MAX_WIDTH as f64 / source_width as f64)
        .min(DRAFT_MAX_HEIGHT as f64 / source_height as f64)
        .min(1.0);
    let width = (source_width as f64 * scale).round() as u32;
    let height = (source_height as f64 * scale).round() as u32;

    (even_dimension(width.max(2)), even_dimension(height.max(2)))
}

fn even_dimension(value: u32) -> u32 {
    value - value % 2
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::edit::render_plan::ExportEncodeTier;

    fn final_mp4() -> ExportRenderOptions {
        ExportRenderOptions::new(ExportProfile::Mp4H264, RenderQuality::Final, 1920, 1080)
            .expect("valid options")
    }

    #[test]
    fn frame_rate_override_is_kept_when_in_range() {
        let options = final_mp4()
            .with_fps(Some(29.97))
            .expect("29.97 fps is valid");
        assert_eq!(options.fps, Some(29.97));
        assert_eq!(options.effective_fps(30.0), 29.97);
        assert_eq!(final_mp4().effective_fps(30.0), 30.0);
        assert_eq!(final_mp4().with_fps(None).expect("no override").fps, None);
        assert!(final_mp4().with_fps(Some(1.0)).is_ok());
        assert!(final_mp4().with_fps(Some(120.0)).is_ok());
    }

    #[test]
    fn frame_rate_override_outside_the_range_is_refused() {
        for fps in [0.0, f64::NAN, 240.5, f64::INFINITY, -24.0] {
            let error = final_mp4().with_fps(Some(fps)).expect_err("invalid fps");
            assert_eq!(error, ExportRenderOptionsError::InvalidFrameRate);
            assert_eq!(
                error.to_string(),
                "Choose a frame rate between 1 and 120 fps."
            );
        }
    }

    #[test]
    fn master_needs_final_quality_and_a_master_capable_profile() {
        let draft =
            ExportRenderOptions::new(ExportProfile::Mp4H264, RenderQuality::Draft, 1280, 720)
                .expect("draft");
        let error = draft
            .with_encode_tier(ExportEncodeTier::Master)
            .expect_err("draft master");
        assert_eq!(error, ExportRenderOptionsError::MasterNeedsFinalQuality);
        assert_eq!(error.to_string(), "Master quality needs Final quality.");

        let prores =
            ExportRenderOptions::new(ExportProfile::ProResMov, RenderQuality::Final, 1920, 1080)
                .expect("prores");
        let error = prores
            .with_encode_tier(ExportEncodeTier::Master)
            .expect_err("prores master");
        assert_eq!(error, ExportRenderOptionsError::MasterUnsupportedProfile);
        assert_eq!(
            error.to_string(),
            "Master quality is for MP4 and WebM exports; ProRes already exports at mastering quality."
        );

        let master = final_mp4()
            .with_encode_tier(ExportEncodeTier::Master)
            .expect("final mp4 master");
        assert_eq!(master.encode_tier, ExportEncodeTier::Master);
        assert!(draft.with_encode_tier(ExportEncodeTier::Standard).is_ok());
    }

    #[test]
    fn serde_defaults_fps_and_encode_tier_and_omits_them_when_default() {
        let options: ExportRenderOptions = serde_json::from_value(serde_json::json!({
            "profile": "mp4H264",
            "quality": "final",
            "width": 2,
            "height": 2
        }))
        .expect("legacy options deserialize");
        assert_eq!(options.fps, None);
        assert_eq!(options.encode_tier, ExportEncodeTier::Standard);
        let value = serde_json::to_value(options).expect("serialize");
        assert!(value.get("fps").is_none());
        assert!(value.get("encodeTier").is_none());

        let master = final_mp4()
            .with_fps(Some(25.0))
            .and_then(|options| options.with_encode_tier(ExportEncodeTier::Master))
            .expect("master");
        let value = serde_json::to_value(master).expect("serialize");
        assert_eq!(value["fps"], 25.0);
        assert_eq!(value["encodeTier"], "master");
    }

    #[test]
    fn an_export_job_records_the_requested_draft_settings_for_retry() {
        let output = ExportOutputRequest {
            file_name: "Edison intro".to_string(),
            directory: None,
        };
        let rendered =
            ExportRenderOptions::new(ExportProfile::Mp4H264, RenderQuality::Draft, 1280, 720)
                .expect("draft options")
                .with_fps(Some(24.0))
                .expect("draft fps");
        let requested = JobExportSettings {
            options: ExportRenderOptions::new(
                ExportProfile::Mp4H264,
                RenderQuality::Draft,
                1920,
                1080,
            )
            .expect("requested options"),
            output: None,
        };

        let recorded =
            JobExportSettings::for_export(rendered, Some(requested.clone()), output.clone());
        assert_eq!(recorded.options, requested.options);
        assert_eq!(recorded.output, Some(output.clone()));

        for mismatched in [
            JobExportSettings {
                options: final_mp4(),
                output: None,
            },
            JobExportSettings {
                options: ExportRenderOptions {
                    width: 1921,
                    ..requested.options
                },
                output: None,
            },
        ] {
            let recorded =
                JobExportSettings::for_export(rendered, Some(mismatched), output.clone());
            assert_eq!(recorded.options, rendered);
        }
        assert_eq!(
            JobExportSettings::for_export(rendered, None, output).options,
            rendered
        );
    }
}
