use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::Command as StdCommand;

#[cfg(feature = "ges-render")]
use video_creater_lib::project::model::CaptionRenderMode;
use video_creater_lib::project::model::VideoProject;
use video_creater_lib::project::storage::save_project;
use video_creater_lib::render_pipeline::error::PipelineErrorCode;
#[cfg(feature = "ges-render")]
use video_creater_lib::render_pipeline::template_project::TemplateRenderReport;
use video_creater_lib::render_pipeline::template_project::{
    parse_render_template_args, project_relative_report_path, run_render_template_project,
    RenderTemplateProjectConfig, TemplateRenderConfig, TemplateRenderFormat,
};

#[test]
fn render_template_args_default_to_project_render_template_config() {
    let config = parse_render_template_args([
        "video-creater-render-template",
        "--project-root",
        "/tmp/video-creater/projects/gradient-background-loop",
    ])
    .expect("project-root args should parse");

    assert_eq!(
        config.project_root,
        Path::new("/tmp/video-creater/projects/gradient-background-loop")
    );
    assert_eq!(
        config.template_config_path,
        Path::new("/tmp/video-creater/projects/gradient-background-loop/render-template.json")
    );
}

#[test]
fn render_template_args_accept_explicit_config_path() {
    let config = parse_render_template_args([
        "video-creater-render-template",
        "--project-root",
        "/tmp/video-creater/projects/gradient-background-loop",
        "--config",
        "/tmp/video-creater/projects/gradient-background-loop/configs/full-hd.json",
    ])
    .expect("explicit config args should parse");

    assert_eq!(
        config.template_config_path,
        Path::new("/tmp/video-creater/projects/gradient-background-loop/configs/full-hd.json")
    );
}

#[test]
fn render_template_args_accept_package_manager_separator() {
    let config = parse_render_template_args([
        "video-creater-render-template",
        "--",
        "--project-root",
        "/tmp/video-creater/projects/gradient-background-loop",
    ])
    .expect("package-manager separator should be ignored");

    assert_eq!(
        config.project_root,
        Path::new("/tmp/video-creater/projects/gradient-background-loop")
    );
}

#[test]
fn render_template_args_reject_unknown_flag() {
    let errors = parse_render_template_args([
        "video-creater-render-template",
        "--project-root",
        "/tmp/video-creater/projects/gradient-background-loop",
        "--surprise",
        "value",
    ])
    .expect_err("unknown flags should be rejected");

    assert_eq!(errors.len(), 1);
    assert_eq!(errors[0].code, PipelineErrorCode::PipelineInputInvalid);
    assert_eq!(errors[0].path, "args.surprise");
}

#[test]
fn template_render_config_deserializes_project_manifest_shape() {
    let config: TemplateRenderConfig = serde_json::from_str(
        r##"{
          "schemaVersion": 1,
          "templateId": "gradient-background-loop-v1",
          "outputName": "gradient-background-loop-test",
          "fields": {
            "headline": "Love\nwins."
          },
          "renderSettings": {
            "width": 320,
            "height": 180,
            "fps": 6,
            "durationSeconds": 0.5,
            "format": "webm"
          },
          "visualTreatment": "full-frame animated gradient background with floating translucent panels",
          "motion": "slow seamless loop with subtle card drift and headline shimmer",
          "safeZone": "keep headline inside the central 70% frame area",
          "avoid": "opaque caption slabs and static text-only cards"
        }"##,
    )
    .expect("render-template.json should deserialize");

    assert_eq!(config.schema_version, 1);
    assert_eq!(config.template_id, "gradient-background-loop-v1");
    assert_eq!(config.output_name, "gradient-background-loop-test");
    assert_eq!(config.fields["headline"], "Love\nwins.");
    assert_eq!(config.render_settings.width, 320);
    assert_eq!(config.render_settings.height, 180);
    assert_eq!(config.render_settings.fps, 6.0);
    assert_eq!(config.render_settings.duration_seconds, 0.5);
    assert_eq!(config.render_settings.format, TemplateRenderFormat::Webm);
}

#[test]
fn template_report_artifact_paths_are_project_relative() {
    let project_root = Path::new("/tmp/video-creater/projects/gradient-background-loop");

    let relative = project_relative_report_path(
        project_root,
        &project_root.join("renders/gradient-background-loop-full-hd.webm"),
    )
    .expect("project artifact path should become relative");

    assert_eq!(relative, "renders/gradient-background-loop-full-hd.webm");
}

#[cfg(feature = "ges-render")]
#[test]
fn render_template_project_writes_project_scoped_video_preview_and_report() {
    let tempdir = tempfile::tempdir().expect("temp render template project dir");
    let mut project = VideoProject::new_empty(
        "gradient-background-loop-test".to_string(),
        "Gradient Background Loop Test".to_string(),
        "2026-06-21T00:00:00Z".to_string(),
    );
    project.timeline.duration_seconds = 0.5;
    project.render_settings.width = 320;
    project.render_settings.height = 180;
    project.render_settings.fps = 6.0;
    project.render_settings.captions = CaptionRenderMode::Off;
    save_project(tempdir.path(), &project).expect("save project");

    let template_config_path = tempdir.path().join("render-template.json");
    std::fs::write(
        &template_config_path,
        serde_json::to_string_pretty(&sample_template_config())
            .expect("template config should serialize"),
    )
    .expect("write render-template.json");

    let config = RenderTemplateProjectConfig {
        project_root: tempdir.path().to_path_buf(),
        template_config_path,
    };

    let result = run_render_template_project(&config);

    match result {
        Ok(result) => {
            assert_eq!(
                result.video_path,
                tempdir
                    .path()
                    .join("renders/gradient-background-loop-test.webm")
            );
            assert_eq!(
                result.graphics_dir,
                tempdir
                    .path()
                    .join("generated/graphics/gradient-background-loop-test")
            );
            assert_eq!(
                result.preview_path,
                tempdir
                    .path()
                    .join("generated/graphics/gradient-background-loop-test/preview.png")
            );
            assert!(result.video_path.exists(), "template WebM should exist");
            assert!(result.preview_path.exists(), "preview PNG should exist");
            assert!(
                result.graphics_dir.join("frames/frame-000000.png").exists(),
                "rendered frames should exist"
            );
            assert!(
                result.report_path.exists(),
                "render report JSON should exist before success is returned"
            );

            let report_json =
                std::fs::read_to_string(&result.report_path).expect("render report should read");
            let report: TemplateRenderReport =
                serde_json::from_str(&report_json).expect("render report should deserialize");
            assert_eq!(report.status, "succeeded");
            assert_eq!(report.template_id, "gradient-background-loop-v1");
            assert_eq!(report.output_name, "gradient-background-loop-test");
            assert_eq!(
                report.video_path,
                "renders/gradient-background-loop-test.webm"
            );
            assert_eq!(
                report.graphics_dir,
                "generated/graphics/gradient-background-loop-test"
            );
            assert_eq!(
                report.preview_path,
                "generated/graphics/gradient-background-loop-test/preview.png"
            );
            assert_eq!(
                report.manifest_path,
                "generated/graphics/gradient-background-loop-test/manifest.json"
            );
            assert_eq!(report.width, 320);
            assert_eq!(report.height, 180);
            assert_eq!(report.fps, 6.0);
            assert_eq!(report.duration_seconds, 0.5);
            assert_eq!(report.format, TemplateRenderFormat::Webm);
            assert!(report.frame_count >= 1);
        }
        Err(errors)
            if errors
                .iter()
                .any(|error| error.code == PipelineErrorCode::RenderBackendUnavailable) =>
        {
            eprintln!("Skipping template render project e2e because GStreamer is unavailable");
        }
        Err(errors) => panic!("unexpected template render project errors: {errors:?}"),
    }
}

#[test]
fn render_template_project_rejects_path_escape_output_name() {
    let tempdir = tempfile::tempdir().expect("temp render template project dir");
    let project = VideoProject::new_empty(
        "gradient-background-loop-test".to_string(),
        "Gradient Background Loop Test".to_string(),
        "2026-06-21T00:00:00Z".to_string(),
    );
    save_project(tempdir.path(), &project).expect("save project");

    let mut config_json = sample_template_config();
    config_json.output_name = "../escape".to_string();
    let template_config_path = tempdir.path().join("render-template.json");
    std::fs::write(
        &template_config_path,
        serde_json::to_string_pretty(&config_json).expect("template config should serialize"),
    )
    .expect("write render-template.json");

    let errors = run_render_template_project(&RenderTemplateProjectConfig {
        project_root: tempdir.path().to_path_buf(),
        template_config_path,
    })
    .expect_err("path escaping output names should fail");

    assert_eq!(errors.len(), 1);
    assert_eq!(errors[0].code, PipelineErrorCode::PipelineInputInvalid);
    assert_eq!(errors[0].path, "renderTemplate.outputName");
}

#[test]
fn render_template_cli_does_not_succeed_after_parse_only() {
    let tempdir = tempfile::tempdir().expect("temp render template project dir");
    let output = StdCommand::new(render_template_bin_path())
        .args([
            "--project-root",
            tempdir.path().to_str().expect("temp path should be utf8"),
        ])
        .output()
        .expect("render template binary should run");

    assert!(
        !output.status.success(),
        "binary must fail instead of returning ok:true after argument parsing; stdout: {} stderr: {}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        !tempdir.path().join("renders/render-report.json").exists(),
        "failed render template run must not write a success report"
    );
}

fn sample_template_config() -> TemplateRenderConfig {
    TemplateRenderConfig {
        schema_version: 1,
        template_id: "gradient-background-loop-v1".to_string(),
        output_name: "gradient-background-loop-test".to_string(),
        fields: BTreeMap::from([("headline".to_string(), "Love\nwins.".to_string())]),
        render_settings:
            video_creater_lib::render_pipeline::template_project::TemplateRenderSettings {
                width: 320,
                height: 180,
                fps: 6.0,
                duration_seconds: 0.5,
                format: TemplateRenderFormat::Webm,
            },
        visual_treatment: Some(
            "full-frame animated gradient background with floating translucent panels".to_string(),
        ),
        motion: Some("slow seamless loop with subtle card drift and headline shimmer".to_string()),
        safe_zone: Some("keep headline inside the central 70% frame area".to_string()),
        avoid: Some("opaque caption slabs and static text-only cards".to_string()),
    }
}

fn render_template_bin_path() -> PathBuf {
    std::env::var("CARGO_BIN_EXE_video-creater-render-template")
        .map(PathBuf::from)
        .unwrap_or_else(|_| {
            Path::new(env!("CARGO_MANIFEST_DIR")).join("target/debug/video-creater-render-template")
        })
}
