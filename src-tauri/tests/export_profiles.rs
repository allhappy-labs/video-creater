use video_creater_lib::edit::render_plan::RenderQuality;
use video_creater_lib::project::export_options::ExportRenderOptions;
use video_creater_lib::project::export_profiles::{
    export_profile_availability, mp4_export_profile_availability_report, ExportProfile,
};
use video_creater_lib::project::fixtures::sample_project;
use video_creater_lib::render_pipeline::avfoundation_backend::{
    avfoundation_profile, AvFoundationExportProfile,
};
use video_creater_lib::render_pipeline::project_export::{
    build_project_media_render_plan_with_options, expected_media_for_render_plan,
};
use video_creater_lib::workflows::temporal_export_media_start_request_with_options;

const DELIVERY_PROFILES: [ExportProfile; 4] = [
    ExportProfile::Webm,
    ExportProfile::Mp4H264,
    ExportProfile::Mp4H265,
    ExportProfile::ProResMov,
];

#[cfg(target_os = "macos")]
#[test]
fn draft_prores_requires_proxy_capability() {
    let availability = export_profile_availability(ExportProfile::ProResMov);

    assert!(!availability.quality_available(RenderQuality::Draft));
    assert!(availability
        .draft_unavailable_reason()
        .expect("draft ProRes should explain why it is unavailable")
        .contains("ProRes Proxy"));
}

#[test]
fn avfoundation_uses_proxy_only_for_draft_prores() {
    assert_eq!(
        avfoundation_profile(ExportProfile::ProResMov, RenderQuality::Draft),
        AvFoundationExportProfile::ProResProxy
    );
    assert_eq!(
        avfoundation_profile(ExportProfile::ProResMov, RenderQuality::Final),
        AvFoundationExportProfile::ProRes422
    );
}

#[cfg(target_os = "macos")]
#[test]
fn mp4_profiles_report_one_complete_native_runtime() {
    let report = mp4_export_profile_availability_report();

    let h264 = report
        .iter()
        .find(|profile| profile.profile == ExportProfile::Mp4H264)
        .expect("h264 profile");

    assert_native_or_gstreamer_runtime(
        &h264.required_runtime,
        &[
            "gstreamer:mp4mux",
            "gstreamer:vtenc_h264",
            "gstreamer:atenc",
            "gstreamer:aacparse",
        ],
    );

    let h265 = report
        .iter()
        .find(|profile| profile.profile == ExportProfile::Mp4H265)
        .expect("h265 profile");

    assert_native_or_gstreamer_runtime(
        &h265.required_runtime,
        &[
            "gstreamer:mp4mux",
            "gstreamer:vtenc_h265",
            "gstreamer:atenc",
            "gstreamer:aacparse",
        ],
    );
}

#[cfg(target_os = "macos")]
#[test]
fn prores_profile_reports_quicktime_gstreamer_requirements() {
    let report = mp4_export_profile_availability_report();

    let prores = report
        .iter()
        .find(|profile| profile.profile == ExportProfile::ProResMov)
        .expect("prores profile");

    assert_eq!(prores.extension, "mov");
    assert_eq!(prores.mime_type, "video/quicktime");
    assert_eq!(prores.audio_codec.as_deref(), Some("pcm"));
    assert_native_or_gstreamer_runtime(
        &prores.required_runtime,
        &["gstreamer:qtmux", "gstreamer:vtenc_prores"],
    );
}

#[cfg(target_os = "macos")]
fn assert_native_or_gstreamer_runtime(actual: &[String], gstreamer: &[&str]) {
    let native = [
        "system:avfoundation".to_string(),
        "bundled:video-creater-avfoundation-exporter".to_string(),
    ];
    let gstreamer = gstreamer
        .iter()
        .map(|value| (*value).to_string())
        .collect::<Vec<_>>();
    assert!(
        actual == native || actual == gstreamer,
        "unexpected runtime requirements: {actual:?}"
    );
}

#[test]
fn export_profile_availability_report_does_not_expose_secrets() {
    let report = mp4_export_profile_availability_report();
    let serialized = serde_json::to_string(&report).expect("serialized report");

    assert!(!serialized.contains("FAL"));
    assert!(!serialized.contains("KEY"));
    assert!(!serialized.contains("token"));
    assert!(!serialized.contains("secret"));
    assert!(!serialized.contains("VIDEO_CREATER_APPROVED"));
}

#[cfg(target_os = "linux")]
#[test]
fn linux_delivery_profiles_report_linux_gstreamer_requirements() {
    let report = mp4_export_profile_availability_report();
    let requirements = |profile: ExportProfile| {
        report
            .iter()
            .find(|entry| entry.profile == profile)
            .expect("profile")
            .required_runtime
            .clone()
    };

    assert_eq!(
        requirements(ExportProfile::Mp4H264),
        [
            "gstreamer:mp4mux",
            "gstreamer:openh264enc",
            "gstreamer:avenc_aac",
            "gstreamer:h264parse",
            "gstreamer:aacparse"
        ]
    );
    assert_eq!(
        requirements(ExportProfile::ProResMov),
        ["gstreamer:qtmux", "gstreamer:avenc_prores_ks"]
    );
    let prores = export_profile_availability(ExportProfile::ProResMov);
    assert_eq!(
        prores.quality_available(RenderQuality::Draft),
        prores.quality_available(RenderQuality::Final)
    );
}

/// The render report copies the expected media into `summary`, and the Temporal
/// `ValidateRenderedMedia` activity compares that summary against the export
/// profile's validation payload field by field. The two tables are written by
/// hand in different modules, so any spelling drift between them rejects a
/// perfectly good export; every delivery profile has to describe the same file.
#[test]
fn every_delivery_profile_expects_the_media_its_validation_payload_requires() {
    for profile in DELIVERY_PROFILES {
        let availability = export_profile_availability(profile);
        let dir = tempfile::tempdir().expect("temp project dir");
        let mut project = sample_project();
        let item = &mut project.timeline.tracks[0].items[0];
        item.properties
            .insert("sourceIn".to_string(), serde_json::json!(0.0));
        item.properties
            .insert("sourceOut".to_string(), serde_json::json!(4.0));
        let options = ExportRenderOptions::new(profile, RenderQuality::Final, 1920, 1080)
            .expect("full-hd final options are valid");
        let plan = build_project_media_render_plan_with_options(
            dir.path(),
            &project,
            "export-validation-parity",
            options,
        )
        .unwrap_or_else(|errors| panic!("{profile:?} should build a render plan: {errors:?}"));
        let expected = expected_media_for_render_plan(&plan, true);

        let request = temporal_export_media_start_request_with_options(
            &project.id,
            &dir.path().to_string_lossy(),
            "export-validation-parity",
            options,
            &format!("exports/video.{}", availability.extension),
        );
        let validation = request
            .input
            .get("validation")
            .expect("export start request carries the validation payload");

        assert_eq!(
            expected.container.as_deref(),
            validation["container"].as_str(),
            "{profile:?} container"
        );
        assert_eq!(
            expected.video_codec.as_deref(),
            validation["videoCodec"].as_str(),
            "{profile:?} video codec"
        );
        assert_eq!(
            expected.audio_codec.as_deref(),
            validation["audioCodec"].as_str(),
            "{profile:?} audio codec"
        );
    }
}
