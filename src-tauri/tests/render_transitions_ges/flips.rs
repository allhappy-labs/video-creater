//! GES clip flips: `videoflip video-direction` takes the
//! `GstVideoOrientationMethod` nicks `horiz`, `vert` and `180`.

use super::fixtures::{base_project, clip, render, start_runtime, with_properties};
use super::media::{decode_frames, HEIGHT, WIDTH};
use gstreamer as gst;
use gstreamer::glib::translate::IntoGlib;
use gstreamer::prelude::*;
use serde_json::json;
use video_creater_lib::edit::render_plan::RenderQualityProfile;
use video_creater_lib::project::model::{MediaAsset, MediaKind, TimelineItemKind};
use video_creater_lib::render_pipeline::project_export::build_project_webm_render_plan;

#[test]
fn videoflip_video_direction_accepts_orientation_nicks() {
    start_runtime();
    let flip = gst::ElementFactory::make("videoflip")
        .build()
        .expect("runtime provides videoflip");
    let pspec = flip
        .find_property("video-direction")
        .expect("video-direction property");
    let enum_class = gst::glib::EnumClass::with_type(pspec.value_type()).expect("enum property");
    for nick in ["horiz", "vert", "180"] {
        assert!(
            enum_class.value_by_nick(nick).is_some(),
            "`{nick}` is a video-direction nick"
        );
    }
    for invalid in ["horizontal-flip", "vertical-flip", "rotate-180"] {
        assert!(
            enum_class.value_by_nick(invalid).is_none(),
            "`{invalid}` is not a video-direction nick"
        );
    }
    assert_eq!(
        enum_class.value_by_nick("horiz").map(|value| value.value()),
        Some(gstreamer_video::VideoOrientationMethod::Horiz.into_glib())
    );
}

fn mirrored(frame: &[u8], horizontal: bool) -> Vec<u8> {
    let mut output = vec![0; frame.len()];
    for y in 0..HEIGHT {
        for x in 0..WIDTH {
            let (source_x, source_y) = if horizontal {
                (WIDTH - 1 - x, y)
            } else {
                (x, HEIGHT - 1 - y)
            };
            let from = (source_y * WIDTH + source_x) * 4;
            let to = (y * WIDTH + x) * 4;
            output[to..to + 4].copy_from_slice(&frame[from..from + 4]);
        }
    }
    output
}

fn mean_abs_difference(left: &[u8], right: &[u8]) -> f64 {
    let total = left
        .chunks_exact(4)
        .zip(right.chunks_exact(4))
        .map(|(left, right)| {
            (0..3)
                .map(|channel| f64::from(left[channel].abs_diff(right[channel])))
                .sum::<f64>()
        })
        .sum::<f64>();
    total / (left.len() / 4 * 3) as f64
}

#[test]
fn horizontal_and_vertical_flips_mirror_the_rendered_clip() {
    start_runtime();
    let dir = tempfile::tempdir().expect("project directory");
    std::fs::create_dir_all(dir.path().join("media")).expect("media directory");
    let pipeline = gst::parse::launch(&format!(
        "videotestsrc pattern=smpte num-buffers=96 \
         ! video/x-raw,width={WIDTH},height={HEIGHT},framerate=24/1 ! videoconvert \
         ! vp8enc deadline=1 ! webmmux ! filesink location=\"{}\"",
        dir.path().join("media/smpte.webm").display()
    ))
    .expect("SMPTE fixture pipeline");
    pipeline
        .set_state(gst::State::Playing)
        .expect("start fixture");
    let bus = pipeline.bus().expect("fixture bus");
    let message = bus
        .timed_pop_filtered(
            gst::ClockTime::from_seconds(60),
            &[gst::MessageType::Eos, gst::MessageType::Error],
        )
        .expect("fixture finishes");
    pipeline.set_state(gst::State::Null).expect("stop fixture");
    assert!(
        matches!(message.view(), gst::MessageView::Eos(_)),
        "SMPTE fixture failed"
    );

    let mut project = base_project();
    project.media.push(MediaAsset {
        id: "smpte".to_string(),
        name: Some("smpte".to_string()),
        relative_path: "media/smpte.webm".to_string(),
        kind: MediaKind::Video,
        duration_seconds: 4.0,
        width: Some(WIDTH as u32),
        height: Some(HEIGHT as u32),
        fps: Some(24.0),
        folder_id: None,
    });
    let smpte_clip = |id: &str, start: f64, transform: serde_json::Value| {
        with_properties(
            clip(id, TimelineItemKind::VideoClip, "smpte", start, 1.0, 0.0),
            json!({ "transform": transform }),
        )
    };
    project.timeline.tracks[0].items = vec![
        smpte_clip("plain", 0.0, json!({})),
        smpte_clip("horizontal", 1.0, json!({ "flipHorizontal": true })),
        smpte_clip("vertical", 2.0, json!({ "flipVertical": true })),
    ];
    project.timeline.duration_seconds = 3.0;

    let plan = build_project_webm_render_plan(
        dir.path(),
        &project,
        "flips",
        RenderQualityProfile::FinalWebm,
    )
    .expect("render plan");
    let frames = decode_frames(&render(&plan));
    let plain = &frames[&12];
    for (label, frame_index, horizontal) in [("horizontal", 36, true), ("vertical", 60, false)] {
        let flipped = &frames[&frame_index];
        let mirrored_difference = mean_abs_difference(flipped, &mirrored(plain, horizontal));
        let unflipped_difference = mean_abs_difference(flipped, plain);
        eprintln!(
            "{label} flip: mean difference {mirrored_difference:.2} from the mirrored clip, \
             {unflipped_difference:.2} from the unflipped clip"
        );
        assert!(
            mirrored_difference < 6.0,
            "{label} flip matches the mirrored clip"
        );
        assert!(
            unflipped_difference > 30.0,
            "{label} flip changes the frame"
        );
    }
}
