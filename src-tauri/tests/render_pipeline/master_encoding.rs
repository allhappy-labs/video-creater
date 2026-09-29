//! Master encode tier: encoder settings and the render command record.

use super::render_plan;
use video_creater_lib::edit::render_plan::{ExportEncodeTier, RenderQuality};
use video_creater_lib::render_pipeline::backend::RenderBackend;
#[cfg(feature = "ges-render")]
use video_creater_lib::render_pipeline::gstreamer_backend::webm_encoding_profile_summary_for_test;
use video_creater_lib::render_pipeline::gstreamer_backend::GstreamerGesRenderBackend;

#[cfg(all(feature = "ges-render", target_os = "linux"))]
#[test]
fn linux_master_encoder_properties_exist_in_the_curated_runtime() {
    use video_creater_lib::render_pipeline::gstreamer_backend::encoder_property_names_for_test;
    use video_creater_lib::render_runtime::start_render_process_runtime;

    start_render_process_runtime().expect("initialize curated render runtime");
    let required: [(&str, &[&str]); 3] = [
        (
            "openh264enc",
            &["bitrate", "max-bitrate", "complexity", "rate-control"],
        ),
        (
            "vp8enc",
            &[
                "target-bitrate",
                "deadline",
                "cpu-used",
                "auto-alt-ref",
                "lag-in-frames",
            ],
        ),
        ("vah265enc", &["bitrate", "target-usage", "rate-control"]),
    ];
    for (factory, properties) in required {
        let Some(names) = encoder_property_names_for_test(factory).expect("inspect encoder") else {
            assert_eq!(
                factory, "vah265enc",
                "{factory} must be in the curated runtime"
            );
            eprintln!("{factory} not present: not verified");
            continue;
        };
        for property in properties {
            assert!(
                names.iter().any(|name| name == property),
                "{factory} has no {property} property; it has {names:?}"
            );
        }
        eprintln!("{factory} verified: {properties:?}");
    }
}

#[cfg(feature = "ges-render")]
#[test]
fn gstreamer_ges_master_webm_profile_uses_the_master_ladder() {
    video_creater_lib::render_runtime::start_render_process_runtime()
        .expect("initialize curated render runtime");
    let mut plan = render_plan();
    plan.quality = RenderQuality::Final;
    plan.encode_tier = ExportEncodeTier::Master;
    plan.width = 1920;
    plan.height = 1080;
    plan.fps = 60.0;

    let summary =
        webm_encoding_profile_summary_for_test(&plan).expect("master WebM profile summary");

    assert_eq!(summary.encoder_factory, "vp8enc");
    // Final 1080p60 is 12 Mbps; Master doubles it.
    assert_eq!(summary.target_bitrate_bps, 24_000_000);
    assert_eq!(summary.deadline, 1_000_000);
    assert_eq!(summary.cpu_used, 0);
}

#[cfg(all(feature = "ges-render", not(target_os = "macos")))]
#[test]
fn linux_master_h264_profile_doubles_the_final_bitrate() {
    use video_creater_lib::project::export_profiles::ExportProfile;
    use video_creater_lib::render_pipeline::gstreamer_backend::encoding_profile_summary_for_test;

    let mut plan = render_plan();
    plan.quality = RenderQuality::Final;
    plan.width = 1920;
    plan.height = 1080;
    plan.fps = 30.0;
    let final_summary = encoding_profile_summary_for_test(&plan, ExportProfile::Mp4H264)
        .expect("final H.264 profile summary");
    plan.encode_tier = ExportEncodeTier::Master;
    let master_summary = encoding_profile_summary_for_test(&plan, ExportProfile::Mp4H264)
        .expect("master H.264 profile summary");

    assert_eq!(final_summary.video_factory, "openh264enc");
    assert_eq!(final_summary.video_bitrate_kbps, 6_000);
    assert_eq!(master_summary.video_factory, "openh264enc");
    assert_eq!(master_summary.video_bitrate_kbps, 12_000);
    assert!(!master_summary.realtime);
}

#[test]
fn gstreamer_ges_command_records_master_encode_tier_only_for_master() {
    let backend = GstreamerGesRenderBackend::new();
    let mut plan = render_plan();
    plan.quality = RenderQuality::Final;

    let standard = backend.build_command(&plan, &[]).expect("standard command");
    assert!(!standard
        .args
        .iter()
        .any(|arg| arg.starts_with("--encode-tier")));

    plan.encode_tier = ExportEncodeTier::Master;
    let master = backend.build_command(&plan, &[]).expect("master command");
    assert!(master.args.contains(&"--encode-tier=master".to_string()));
}
