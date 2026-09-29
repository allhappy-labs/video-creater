use std::fs;
use std::path::Path;

use image::{Rgba, RgbaImage};
use video_creater_lib::media_inspection::{
    derive_media_analysis_moments, derive_media_analysis_moments_with_sampled_frames,
    derive_media_analysis_moments_with_sampled_signals, derive_media_silence_ranges, inspect_image,
    inspect_render_directory, AudioWindowMetrics, ImageMetrics, VideoInspectionReport,
};
use video_creater_lib::render_pipeline::probe::{AudioProbe, MediaProbe, VideoProbe};

#[test]
fn image_inspection_reports_visual_metrics_for_png() {
    let temp = tempfile::tempdir().expect("create temp dir");
    let image_path = temp.path().join("preview.png");
    write_rgba_png(
        &image_path,
        4,
        1,
        &[
            [255, 255, 255, 255],
            [0, 255, 255, 255],
            [0, 0, 0, 255],
            [255, 0, 128, 16],
        ],
    );

    let report = inspect_image(&image_path).expect("inspect png");

    assert_eq!(report.kind, "image");
    assert_eq!(report.path, image_path);
    assert_eq!(report.size_bytes, fs::metadata(&image_path).unwrap().len());
    assert_eq!(report.image.width, 4);
    assert_eq!(report.image.height, 1);
    assert_eq!(report.image.total_pixels, 4);
    assert_eq!(report.image.visible_sample_count, 2);
    assert!((report.image.alpha_coverage - 0.75).abs() < 0.0001);
    assert!((report.image.average_saturation - 0.5).abs() < 0.0001);
    assert!(report.image.luminance_stddev > 20.0);
    assert!(report.warnings.is_empty());
}

#[test]
fn render_directory_inspection_discovers_preview_and_frame_sequence() {
    let temp = tempfile::tempdir().expect("create temp dir");
    let output_dir = temp.path().join("output");
    let frames_dir = output_dir.join("frames");
    fs::create_dir_all(&frames_dir).expect("create frames dir");
    write_rgba_png(
        &output_dir.join("preview.png"),
        2,
        1,
        &[[240, 244, 255, 255], [0, 190, 220, 255]],
    );
    write_rgba_png(
        &frames_dir.join("frame-000000.png"),
        2,
        1,
        &[[255, 255, 255, 255], [64, 96, 160, 255]],
    );
    write_rgba_png(
        &frames_dir.join("frame-000001.png"),
        2,
        1,
        &[[255, 255, 255, 255], [180, 200, 255, 255]],
    );

    let report = inspect_render_directory(&output_dir).expect("inspect render output");

    assert_eq!(report.kind, "renderDir");
    assert_eq!(report.path, output_dir);
    let render_dir = report.render_directory.expect("render dir details");
    assert_eq!(
        render_dir
            .preview
            .as_ref()
            .map(|preview| preview.path.as_path()),
        Some(output_dir.join("preview.png").as_path())
    );
    assert_eq!(render_dir.frame_sequence.as_ref().unwrap().frame_count, 2);
    assert_eq!(
        render_dir
            .frame_sequence
            .as_ref()
            .unwrap()
            .first_frame
            .as_path(),
        frames_dir.join("frame-000000.png").as_path()
    );
    assert_eq!(
        render_dir
            .frame_sequence
            .as_ref()
            .unwrap()
            .last_frame
            .as_path(),
        frames_dir.join("frame-000001.png").as_path()
    );
    assert!(render_dir.video.is_none());
    assert!(report.warnings.is_empty());
}

#[test]
fn derives_edl_media_analysis_moments_from_video_probe_and_image_metrics() {
    let report = VideoInspectionReport {
        kind: "video".to_string(),
        path: Path::new("media/action-cut.mp4").to_path_buf(),
        size_bytes: 8_000_000,
        media: MediaProbe {
            container_name: None,
            duration_seconds: Some(24.0),
            size_bytes: Some(8_000_000),
            video: Some(VideoProbe {
                codec_name: Some("h264".to_string()),
                width: Some(1920),
                height: Some(1080),
                fps: Some(60.0),
            }),
            audio: Some(AudioProbe {
                codec_name: Some("aac".to_string()),
            }),
        },
        warnings: Vec::new(),
    };
    let preview_metrics = ImageMetrics {
        width: 1920,
        height: 1080,
        total_pixels: 1920 * 1080,
        alpha_coverage: 1.0,
        visible_sample_count: 1024,
        average_saturation: 0.72,
        luminance_mean: 132.0,
        luminance_stddev: 58.0,
        dark_pixel_ratio: 0.18,
        bright_pixel_ratio: 0.14,
    };

    let moments = derive_media_analysis_moments("media-1", &report, Some(&preview_metrics));

    assert_eq!(moments.len(), 3);
    assert_eq!(moments[0].media_id, "media-1");
    assert_eq!(moments[0].source_in, 0.0);
    assert_eq!(moments[0].source_out, 4.0);
    assert_eq!(moments[1].source_in, 10.0);
    assert_eq!(moments[1].source_out, 14.0);
    assert_eq!(moments[2].source_in, 20.0);
    assert_eq!(moments[2].source_out, 24.0);
    assert!(moments[0].visual_action_score > 0.7);
    assert!(moments[0].audio_energy_score > 0.6);
    assert!(moments[0].label.contains("opening"));
    assert!(moments[1].label.contains("middle"));
    assert!(moments[2].label.contains("closing"));
}

#[test]
fn derives_stronger_media_analysis_from_sampled_frame_changes() {
    let report = VideoInspectionReport {
        kind: "video".to_string(),
        path: Path::new("media/static-probe-but-changing-frames.mp4").to_path_buf(),
        size_bytes: 8_000_000,
        media: MediaProbe {
            container_name: None,
            duration_seconds: Some(18.0),
            size_bytes: Some(8_000_000),
            video: Some(VideoProbe {
                codec_name: Some("h264".to_string()),
                width: Some(1920),
                height: Some(1080),
                fps: Some(24.0),
            }),
            audio: None,
        },
        warnings: Vec::new(),
    };
    let static_preview = ImageMetrics {
        width: 1920,
        height: 1080,
        total_pixels: 1920 * 1080,
        alpha_coverage: 1.0,
        visible_sample_count: 1024,
        average_saturation: 0.08,
        luminance_mean: 120.0,
        luminance_stddev: 12.0,
        dark_pixel_ratio: 0.04,
        bright_pixel_ratio: 0.03,
    };
    let sampled_frames = vec![
        ImageMetrics {
            luminance_mean: 24.0,
            luminance_stddev: 16.0,
            average_saturation: 0.08,
            dark_pixel_ratio: 0.88,
            bright_pixel_ratio: 0.0,
            ..static_preview.clone()
        },
        ImageMetrics {
            luminance_mean: 232.0,
            luminance_stddev: 66.0,
            average_saturation: 0.72,
            dark_pixel_ratio: 0.0,
            bright_pixel_ratio: 0.82,
            ..static_preview.clone()
        },
        ImageMetrics {
            luminance_mean: 78.0,
            luminance_stddev: 44.0,
            average_saturation: 0.48,
            dark_pixel_ratio: 0.38,
            bright_pixel_ratio: 0.05,
            ..static_preview
        },
    ];

    let preview_only = derive_media_analysis_moments("media-1", &report, None);
    let sampled = derive_media_analysis_moments_with_sampled_frames(
        "media-1",
        &report,
        None,
        &sampled_frames,
    );

    assert!(preview_only.is_empty());
    assert_eq!(sampled.len(), 3);
    assert!(sampled[0].visual_action_score > 0.35);
    assert!(sampled[0].label.contains("sampled-frame visual change"));
}

#[test]
fn sampled_visual_and_audio_change_labels_transformation_reveal_moments() {
    let report = VideoInspectionReport {
        kind: "video".to_string(),
        path: Path::new("media/before-after-reveal.mp4").to_path_buf(),
        size_bytes: 8_000_000,
        media: MediaProbe {
            container_name: None,
            duration_seconds: Some(18.0),
            size_bytes: Some(8_000_000),
            video: Some(VideoProbe {
                codec_name: Some("h264".to_string()),
                width: Some(1920),
                height: Some(1080),
                fps: Some(24.0),
            }),
            audio: Some(AudioProbe {
                codec_name: Some("aac".to_string()),
            }),
        },
        warnings: Vec::new(),
    };
    let base_frame = ImageMetrics {
        width: 1920,
        height: 1080,
        total_pixels: 1920 * 1080,
        alpha_coverage: 1.0,
        visible_sample_count: 1024,
        average_saturation: 0.08,
        luminance_mean: 120.0,
        luminance_stddev: 12.0,
        dark_pixel_ratio: 0.04,
        bright_pixel_ratio: 0.03,
    };
    let sampled_frames = vec![
        ImageMetrics {
            luminance_mean: 18.0,
            luminance_stddev: 14.0,
            average_saturation: 0.05,
            dark_pixel_ratio: 0.9,
            bright_pixel_ratio: 0.0,
            ..base_frame.clone()
        },
        ImageMetrics {
            luminance_mean: 238.0,
            luminance_stddev: 72.0,
            average_saturation: 0.84,
            dark_pixel_ratio: 0.0,
            bright_pixel_ratio: 0.88,
            ..base_frame.clone()
        },
        ImageMetrics {
            luminance_mean: 76.0,
            luminance_stddev: 46.0,
            average_saturation: 0.46,
            dark_pixel_ratio: 0.36,
            bright_pixel_ratio: 0.05,
            ..base_frame
        },
    ];
    let sampled_audio = vec![
        AudioWindowMetrics {
            start_seconds: 0.0,
            end_seconds: 1.5,
            mean_volume_db: -50.0,
            max_volume_db: -36.0,
        },
        AudioWindowMetrics {
            start_seconds: 4.0,
            end_seconds: 5.5,
            mean_volume_db: -9.0,
            max_volume_db: -1.0,
        },
        AudioWindowMetrics {
            start_seconds: 8.0,
            end_seconds: 9.5,
            mean_volume_db: -44.0,
            max_volume_db: -30.0,
        },
    ];

    let sampled = derive_media_analysis_moments_with_sampled_signals(
        "media-1",
        &report,
        None,
        &sampled_frames,
        &sampled_audio,
    );

    assert_eq!(sampled.len(), 3);
    assert!(
        sampled.iter().any(|moment| {
            moment.label.contains("before after transformation")
                && moment.label.contains("payoff reveal")
        }),
        "expected sampled change labels to expose transformation/reveal semantics, got {sampled:?}"
    );
}

#[test]
fn sampled_middle_visual_audio_reveal_scores_above_opening_proxy() {
    let report = VideoInspectionReport {
        kind: "video".to_string(),
        path: Path::new("media/middle-reveal.mp4").to_path_buf(),
        size_bytes: 8_000_000,
        media: MediaProbe {
            container_name: None,
            duration_seconds: Some(18.0),
            size_bytes: Some(8_000_000),
            video: Some(VideoProbe {
                codec_name: Some("h264".to_string()),
                width: Some(1920),
                height: Some(1080),
                fps: Some(24.0),
            }),
            audio: Some(AudioProbe {
                codec_name: Some("aac".to_string()),
            }),
        },
        warnings: Vec::new(),
    };
    let base_frame = ImageMetrics {
        width: 1920,
        height: 1080,
        total_pixels: 1920 * 1080,
        alpha_coverage: 1.0,
        visible_sample_count: 1024,
        average_saturation: 0.08,
        luminance_mean: 28.0,
        luminance_stddev: 12.0,
        dark_pixel_ratio: 0.86,
        bright_pixel_ratio: 0.01,
    };
    let sampled_frames = vec![
        base_frame.clone(),
        ImageMetrics {
            luminance_mean: 236.0,
            luminance_stddev: 72.0,
            average_saturation: 0.82,
            dark_pixel_ratio: 0.0,
            bright_pixel_ratio: 0.86,
            ..base_frame.clone()
        },
        ImageMetrics {
            luminance_mean: 42.0,
            luminance_stddev: 18.0,
            average_saturation: 0.12,
            dark_pixel_ratio: 0.78,
            bright_pixel_ratio: 0.02,
            ..base_frame
        },
    ];
    let sampled_audio = vec![
        AudioWindowMetrics {
            start_seconds: 0.0,
            end_seconds: 1.5,
            mean_volume_db: -48.0,
            max_volume_db: -34.0,
        },
        AudioWindowMetrics {
            start_seconds: 4.0,
            end_seconds: 5.5,
            mean_volume_db: -8.0,
            max_volume_db: -1.0,
        },
        AudioWindowMetrics {
            start_seconds: 8.0,
            end_seconds: 9.5,
            mean_volume_db: -45.0,
            max_volume_db: -32.0,
        },
    ];

    let moments = derive_media_analysis_moments_with_sampled_signals(
        "media-1",
        &report,
        None,
        &sampled_frames,
        &sampled_audio,
    );

    assert_eq!(moments.len(), 3);
    let opening = &moments[0];
    let middle = &moments[1];
    assert!(
        middle.visual_action_score > opening.visual_action_score,
        "expected sampled middle reveal visual score to beat opening proxy, got {moments:?}"
    );
    assert!(
        middle.audio_energy_score > opening.audio_energy_score,
        "expected sampled middle reveal audio score to beat opening proxy, got {moments:?}"
    );
    assert!(middle.label.contains("payoff reveal"));
}

#[test]
fn sampled_reveal_label_attaches_to_arriving_payoff_segment() {
    let report = VideoInspectionReport {
        kind: "video".to_string(),
        path: Path::new("media/setup-to-payoff-reveal.mp4").to_path_buf(),
        size_bytes: 8_000_000,
        media: MediaProbe {
            container_name: None,
            duration_seconds: Some(18.0),
            size_bytes: Some(8_000_000),
            video: Some(VideoProbe {
                codec_name: Some("h264".to_string()),
                width: Some(1920),
                height: Some(1080),
                fps: Some(24.0),
            }),
            audio: Some(AudioProbe {
                codec_name: Some("aac".to_string()),
            }),
        },
        warnings: Vec::new(),
    };
    let base_frame = ImageMetrics {
        width: 1920,
        height: 1080,
        total_pixels: 1920 * 1080,
        alpha_coverage: 1.0,
        visible_sample_count: 1024,
        average_saturation: 0.12,
        luminance_mean: 42.0,
        luminance_stddev: 16.0,
        dark_pixel_ratio: 0.78,
        bright_pixel_ratio: 0.01,
    };
    let sampled_frames = vec![
        base_frame.clone(),
        ImageMetrics {
            luminance_mean: 224.0,
            luminance_stddev: 70.0,
            average_saturation: 0.78,
            dark_pixel_ratio: 0.01,
            bright_pixel_ratio: 0.74,
            ..base_frame.clone()
        },
        ImageMetrics {
            luminance_mean: 220.0,
            luminance_stddev: 68.0,
            average_saturation: 0.74,
            dark_pixel_ratio: 0.01,
            bright_pixel_ratio: 0.7,
            ..base_frame
        },
    ];
    let sampled_audio = vec![
        AudioWindowMetrics {
            start_seconds: 0.0,
            end_seconds: 1.5,
            mean_volume_db: -48.0,
            max_volume_db: -34.0,
        },
        AudioWindowMetrics {
            start_seconds: 4.0,
            end_seconds: 5.5,
            mean_volume_db: -10.0,
            max_volume_db: -2.0,
        },
        AudioWindowMetrics {
            start_seconds: 8.0,
            end_seconds: 9.5,
            mean_volume_db: -12.0,
            max_volume_db: -3.0,
        },
    ];

    let moments = derive_media_analysis_moments_with_sampled_signals(
        "media-1",
        &report,
        None,
        &sampled_frames,
        &sampled_audio,
    );
    let opening = moments
        .iter()
        .find(|moment| moment.label.contains("opening"))
        .expect("opening moment");
    let middle = moments
        .iter()
        .find(|moment| moment.label.contains("middle"))
        .expect("middle moment");

    assert!(
        !opening.label.contains("payoff reveal"),
        "setup segment should not be labeled as the arriving payoff: {moments:?}"
    );
    assert!(
        middle
            .label
            .contains("before after transformation payoff reveal signal"),
        "middle segment should carry the arriving payoff label: {moments:?}"
    );
}

#[test]
fn derives_media_analysis_from_sampled_audio_loudness_changes() {
    let report = VideoInspectionReport {
        kind: "video".to_string(),
        path: Path::new("media/quiet-loud-quiet-audio.mp4").to_path_buf(),
        size_bytes: 8_000_000,
        media: MediaProbe {
            container_name: None,
            duration_seconds: Some(18.0),
            size_bytes: Some(8_000_000),
            video: Some(VideoProbe {
                codec_name: Some("h264".to_string()),
                width: Some(1920),
                height: Some(1080),
                fps: Some(24.0),
            }),
            audio: Some(AudioProbe {
                codec_name: Some("aac".to_string()),
            }),
        },
        warnings: Vec::new(),
    };
    let sampled_audio = vec![
        AudioWindowMetrics {
            start_seconds: 0.0,
            end_seconds: 1.5,
            mean_volume_db: -48.0,
            max_volume_db: -34.0,
        },
        AudioWindowMetrics {
            start_seconds: 4.0,
            end_seconds: 5.5,
            mean_volume_db: -11.0,
            max_volume_db: -2.0,
        },
        AudioWindowMetrics {
            start_seconds: 8.0,
            end_seconds: 9.5,
            mean_volume_db: -42.0,
            max_volume_db: -28.0,
        },
    ];

    let probed_only = derive_media_analysis_moments("media-1", &report, None);
    let sampled = derive_media_analysis_moments_with_sampled_signals(
        "media-1",
        &report,
        None,
        &[],
        &sampled_audio,
    );

    assert_eq!(sampled.len(), 3);
    assert!(sampled[0].audio_energy_score > probed_only[0].audio_energy_score);
    assert!(sampled[0]
        .label
        .contains("sampled-audio loudness/change analysis"));
}

#[test]
fn derives_media_silence_ranges_from_quiet_sampled_audio_windows() {
    let sampled_audio = vec![
        AudioWindowMetrics {
            start_seconds: 0.0,
            end_seconds: 1.5,
            mean_volume_db: -12.0,
            max_volume_db: -3.0,
        },
        AudioWindowMetrics {
            start_seconds: 4.0,
            end_seconds: 6.0,
            mean_volume_db: -49.0,
            max_volume_db: -42.0,
        },
        AudioWindowMetrics {
            start_seconds: 8.0,
            end_seconds: 9.5,
            mean_volume_db: -13.0,
            max_volume_db: -4.0,
        },
    ];

    let ranges = derive_media_silence_ranges("media-1", &sampled_audio);

    assert_eq!(ranges.len(), 1);
    assert_eq!(ranges[0].media_id, "media-1");
    assert_eq!(ranges[0].source_in, 4.0);
    assert_eq!(ranges[0].source_out, 6.0);
    assert!(ranges[0].confidence > 0.8);
    assert!(ranges[0].label.contains("speech-free"));
}

fn write_rgba_png(path: &Path, width: u32, height: u32, pixels: &[[u8; 4]]) {
    assert_eq!(pixels.len(), (width * height) as usize);
    let mut image = RgbaImage::new(width, height);
    for (index, pixel) in pixels.iter().enumerate() {
        let x = index as u32 % width;
        let y = index as u32 / width;
        image.put_pixel(x, y, Rgba(*pixel));
    }
    image.save(path).expect("write png");
}
