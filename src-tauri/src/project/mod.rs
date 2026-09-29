pub mod action;
pub mod audio_edits;
pub mod command_queue;
pub mod export_destination;
pub mod export_options;
pub mod export_profiles;
pub mod export_reveal;
pub mod fixtures;
pub mod import;
pub mod job_progress;
pub mod matte;
pub mod model;
pub mod mutation;
pub mod nested_opacity;
pub mod nle_export;
pub mod patch;
pub mod reverse;
pub mod source_probe;
pub mod split;
pub mod storage;
pub mod transitions;

#[cfg(test)]
mod export_options_contract_tests {
    use crate::edit::render_plan::RenderQuality;
    use crate::project::export_options::{draft_dimensions, ExportRenderOptions};
    use crate::project::export_profiles::ExportProfile;
    use crate::render_pipeline::quality::effective_quality_settings;

    #[test]
    fn export_options_draft_defaults_a_4k_request_to_aspect_preserving_hd() {
        assert_eq!(draft_dimensions(3840, 2160), (1280, 720));
    }

    #[test]
    fn export_options_draft_default_never_upscales_smaller_source_media() {
        assert_eq!(draft_dimensions(640, 360), (640, 360));
    }

    #[test]
    fn export_options_draft_default_never_upscales_tiny_source_dimensions() {
        assert_eq!(draft_dimensions(1, 1), (1, 1));
        assert_eq!(draft_dimensions(1, 3), (1, 3));
    }

    #[test]
    fn export_options_project_defaults_apply_hd_only_for_draft() {
        let draft = ExportRenderOptions::for_project_defaults(
            ExportProfile::Mp4H264,
            RenderQuality::Draft,
            3840,
            2160,
        )
        .expect("4K draft defaults are valid");
        let final_options = ExportRenderOptions::for_project_defaults(
            ExportProfile::Mp4H264,
            RenderQuality::Final,
            3840,
            2160,
        )
        .expect("4K final dimensions are valid");

        assert_eq!((draft.width, draft.height), (1280, 720));
        assert_eq!((final_options.width, final_options.height), (3840, 2160));
    }

    #[test]
    fn export_options_explicit_resolution_is_not_changed_by_draft_quality() {
        let options =
            ExportRenderOptions::new(ExportProfile::Mp4H264, RenderQuality::Draft, 1920, 1080)
                .expect("explicit full-hd request is valid");
        assert_eq!((options.width, options.height), (1920, 1080));
    }

    #[test]
    fn export_options_reject_invalid_dimensions() {
        assert!(
            ExportRenderOptions::new(ExportProfile::Webm, RenderQuality::Draft, 0, 720).is_err()
        );
        assert!(
            ExportRenderOptions::new(ExportProfile::Webm, RenderQuality::Draft, 1281, 720).is_err()
        );
        assert!(
            ExportRenderOptions::new(ExportProfile::Webm, RenderQuality::Draft, 16_386, 720,)
                .is_err()
        );
    }

    #[test]
    fn export_options_draft_settings_retain_requested_dimensions_and_reduce_encoding_cost() {
        let draft = effective_quality_settings(RenderQuality::Draft, 1920, 1080, 60.0);
        let final_settings = effective_quality_settings(RenderQuality::Final, 1920, 1080, 60.0);

        assert_eq!((draft.output_width, draft.output_height), (1920, 1080));
        assert_eq!(draft.output_fps, 24.0);
        assert_eq!(draft.speed_hint, "draft-fast");
        assert!(draft.video_bitrate_kbps < final_settings.video_bitrate_kbps);
    }
}
