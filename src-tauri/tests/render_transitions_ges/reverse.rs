//! Reversed video clips: render preparation decodes the clip's source window
//! forwards (including its transition handles), encodes the frames with
//! `pngenc` in reverse order into a PNG-in-QuickTime intermediate, and the
//! prepared clip plays that intermediate forwards at speed 1 from
//! `sourceIn = head`. GES then renders it like canonical frames.
//!
//! Output frame `k` of the intermediate shows the source frame at
//! `source_seconds_at((k + 1) / fps - head)`: the exact reversal of the
//! frames a forward clip over the same source span shows.

use super::fixtures::render;
use super::media::{FPS, HEIGHT, WIDTH};
use super::parity::{
    assert_render_parity, fixture_dir, load_fixture, segment_colours, transition_sample_times,
};
use gstreamer as gst;
use gstreamer::prelude::*;
use serde_json::json;
use std::collections::BTreeMap;
use std::path::Path;
use std::time::{Duration, Instant};
use video_creater_lib::edit::render_plan::RenderQualityProfile;
use video_creater_lib::precompose::{
    decode_video_frames_rgba, prepare_project_for_render, FrameDecodeSpec, PreparedProject,
};
use video_creater_lib::project::action::{apply_project_action, ProjectAction};
use video_creater_lib::project::model::*;
use video_creater_lib::project::reverse::SourceWindow;
use video_creater_lib::render_pipeline::gstreamer_backend::GstreamerGesRenderBackend;
use video_creater_lib::render_pipeline::plugin_policy::{
    evaluate_gstreamer_factory, GstFactoryInfo, PluginPolicyVerdict,
};
use video_creater_lib::render_pipeline::process::SystemProcessRunner;
use video_creater_lib::render_pipeline::project_export::build_project_webm_render_plan;
use video_creater_lib::render_pipeline::transition_plan::plan_clip_transitions;

const SEGMENT_FRAMES: usize = 7;
const SEGMENTS: usize = 14;
const INVERT_LUT: &str = "LUT_3D_SIZE 2\n1 1 1\n0 1 1\n1 0 1\n0 0 1\n1 1 0\n0 1 0\n1 0 0\n0 0 0\n";

fn media_clip(
    id: &str,
    media: &str,
    start: f64,
    duration: f64,
    source_in: f64,
    speed: f64,
) -> TimelineItem {
    TimelineItem {
        id: id.to_string(),
        kind: TimelineItemKind::VideoClip,
        start_seconds: start,
        duration_seconds: duration,
        source: TimelineSource::Media {
            media_id: media.to_string(),
        },
        label: id.to_string(),
        properties: BTreeMap::from([
            ("sourceIn".to_string(), json!(source_in)),
            ("sourceOut".to_string(), json!(source_in + duration * speed)),
            ("speed".to_string(), json!(speed)),
        ]),
    }
}

fn crossfade(project: &mut VideoProject, (left, right): (&str, &str), seconds: f64) {
    apply_project_action(
        project,
        ProjectAction::AddTransition {
            track_id: "track-video".to_string(),
            transition: TimelineTransition {
                id: format!("{left}-{right}"),
                left_item_id: left.to_string(),
                right_item_id: right.to_string(),
                kind: TransitionKind::Crossfade,
                duration_seconds: seconds,
            },
        },
    )
    .unwrap_or_else(|error| panic!("{left}-{right} crossfade: {error:?}"));
}

/// Cool video, the reversed warm clip `rev` over `rev_duration` from source
/// `source_in` at `speed`, then cool video again, crossfading on both sides.
fn reversed_project(rev_duration: f64, source_in: f64, speed: f64, fade: f64) -> VideoProject {
    let mut project = load_fixture();
    let after = 2.0 + rev_duration;
    let track = &mut project.timeline.tracks[0];
    track.transitions.clear();
    let mut rev = media_clip("rev", "warm", 2.0, rev_duration, source_in, speed);
    rev.properties.insert("reverse".to_string(), json!(true));
    track.items = vec![
        media_clip("before", "cool", 0.0, 2.0, 1.0, 1.0),
        rev,
        media_clip("after", "cool", after, 2.0, 1.0, 1.0),
    ];
    project.timeline.duration_seconds = after + 2.0;
    crossfade(&mut project, ("before", "rev"), fade);
    crossfade(&mut project, ("rev", "after"), fade);
    project
}

fn prepared_item<'a>(
    prepared: &'a PreparedProject,
    id: &str,
) -> (&'a TimelineItem, &'a MediaAsset) {
    let item = prepared
        .project
        .timeline
        .tracks
        .iter()
        .flat_map(|track| &track.items)
        .find(|item| item.id == id)
        .expect("prepared item");
    let TimelineSource::Media { media_id } = &item.source else {
        panic!("prepared item {id} is media-backed");
    };
    let media = prepared
        .project
        .media
        .iter()
        .find(|media| media.id == *media_id)
        .expect("prepared media");
    (item, media)
}

fn centre(frame: &[u8]) -> [u8; 3] {
    let offset = (HEIGHT / 2 * WIDTH + WIDTH / 2) * 4;
    [frame[offset], frame[offset + 1], frame[offset + 2]]
}

fn rgb(argb: u32) -> [u8; 3] {
    [(argb >> 16) as u8, (argb >> 8) as u8, argb as u8]
}

/// Centre colours of every frame of a prepared intermediate.
fn intermediate_centres(dir: &Path, media: &MediaAsset) -> Vec<[u8; 3]> {
    let frame_count = (media.duration_seconds * FPS).round() as u32;
    let mut centres = Vec::new();
    decode_video_frames_rgba(
        &dir.join(&media.relative_path),
        FrameDecodeSpec {
            width: WIDTH as u32,
            height: HEIGHT as u32,
            fps_numerator: FPS as u32,
            fps_denominator: 1,
            source_start_micros: 0,
            source_stop_micros: (media.duration_seconds * 1_000_000.0).round() as u64,
            playback_rate_micros: 1_000_000,
            frame_count,
        },
        Duration::from_secs(60),
        |_, rgba| {
            centres.push(centre(rgba));
            Ok(())
        },
    )
    .expect("decode reversed intermediate");
    centres
}

/// The warm segment intermediate frame `k` must show, from R1's mapping.
fn expected_segment(window: SourceWindow, head: f64, frame: usize) -> usize {
    let source = window.source_seconds_at((frame + 1) as f64 / FPS - head);
    ((source * FPS + 1e-6).floor() as usize / SEGMENT_FRAMES).min(SEGMENTS - 1)
}

/// Asserts every intermediate frame shows the expected warm segment and
/// returns the observed segment order.
fn assert_reversed_frames(
    label: &str,
    centres: &[[u8; 3]],
    window: SourceWindow,
    head: f64,
    invert: bool,
) -> Vec<usize> {
    let colours = segment_colours(true);
    let mut order = Vec::new();
    for (frame, actual) in centres.iter().enumerate() {
        let segment = expected_segment(window, head, frame);
        let mut expected = rgb(colours[segment]);
        if invert {
            expected = expected.map(|channel| 255 - channel);
        }
        assert!(
            actual
                .iter()
                .zip(expected)
                .all(|(left, right)| left.abs_diff(right) <= 16),
            "{label}: frame {frame} shows {actual:?}, expected segment {segment} {expected:?}"
        );
        if order.last() != Some(&segment) {
            order.push(segment);
        }
    }
    order
}

fn report_stages(prepared: &PreparedProject) -> Vec<(String, bool)> {
    prepared
        .reports
        .iter()
        .map(|report| (report.stage.clone(), report.cache_hit))
        .collect()
}

fn assert_transitions_kept(project: &VideoProject, seconds: f64) {
    let planned = plan_clip_transitions(project);
    assert_eq!(planned.len(), 2, "the reversed clip keeps both transitions");
    assert!(planned
        .iter()
        .all(|transition| (transition.duration_seconds - seconds).abs() < 1e-6));
}

#[test]
fn reversed_clip_intermediate_covers_its_handles_in_reverse_order() {
    let dir = fixture_dir();
    // Source 1-3 s reversed: head (4 - 3) / 1, tail 1 / 1; the 1 s crossfades
    // use 0.5 s of each.
    let project = reversed_project(2.0, 1.0, 1.0, 1.0);
    let started = Instant::now();
    let prepared = prepare_project_for_render(dir.path(), &project).expect("prepare reverse");
    let first_wall = started.elapsed();
    let (item, media) = prepared_item(&prepared, "rev");
    assert!(
        media.relative_path.ends_with("intermediate.mov"),
        "{media:?}"
    );
    assert!(!item.properties.contains_key("reverse"), "{item:?}");
    assert_eq!(item.properties["sourceIn"], json!(0.5));
    assert_eq!(item.properties["sourceOut"], json!(2.5));
    assert_eq!(item.properties["speed"], json!(1.0));
    assert_eq!(media.duration_seconds, 3.0, "0.5 s head + 2 s + 0.5 s tail");
    assert_transitions_kept(&prepared.project, 1.0);
    assert_eq!(report_stages(&prepared), [("reverse".to_string(), false)]);

    let window = SourceWindow {
        source_in: 1.0,
        source_out: 3.0,
        speed: 1.0,
        reverse: true,
    };
    let centres = intermediate_centres(dir.path(), media);
    assert_eq!(centres.len(), 72);
    let order = assert_reversed_frames("speed 1", &centres, window, 0.5, false);
    eprintln!("reversed intermediate segment order {order:?}, prepared in {first_wall:?}");
    assert_eq!(order, [11, 10, 9, 8, 7, 6, 5, 4, 3, 2, 1]);

    let started = Instant::now();
    let again = prepare_project_for_render(dir.path(), &project).expect("prepare again");
    eprintln!("second preparation {:?}", started.elapsed());
    assert_eq!(report_stages(&again), [("reverse".to_string(), true)]);
    assert_eq!(again.project, prepared.project);

    let comparison = assert_render_parity(
        "reverse-handles",
        dir.path(),
        &prepared.project,
        &prepared.project,
        &transition_sample_times(&[2.0, 4.0]),
    );
    eprintln!(
        "reverse-handles mismatch ratios: {:?}",
        comparison["comparedFrames"]
            .as_array()
            .expect("compared frames")
            .iter()
            .map(|frame| frame["mismatchRatio"].clone())
            .collect::<Vec<_>>()
    );
}

#[test]
fn reversed_sped_up_clip_bakes_speed_into_the_reversed_intermediate() {
    let dir = fixture_dir();
    // Source 1-3 s at speed 2 over 1 s: head (4 - 3) / 2 and tail 1 / 2; the
    // 0.5 s crossfades use 0.25 s of each.
    let project = reversed_project(1.0, 1.0, 2.0, 0.5);
    let prepared = prepare_project_for_render(dir.path(), &project).expect("prepare reverse");
    let (item, media) = prepared_item(&prepared, "rev");
    assert!(!item.properties.contains_key("reverse"));
    assert_eq!(item.properties["sourceIn"], json!(0.25));
    assert_eq!(item.properties["sourceOut"], json!(1.25));
    assert_eq!(item.properties["speed"], json!(1.0));
    assert!((media.duration_seconds - 1.5).abs() < 1e-9, "{media:?}");
    assert_transitions_kept(&prepared.project, 0.5);

    let window = SourceWindow {
        source_in: 1.0,
        source_out: 3.0,
        speed: 2.0,
        reverse: true,
    };
    let centres = intermediate_centres(dir.path(), media);
    assert_eq!(centres.len(), 36);
    let order = assert_reversed_frames("speed 2", &centres, window, 0.25, false);
    eprintln!("speed 2 reversed segment order {order:?}");
    assert_eq!(order, [11, 10, 9, 8, 7, 6, 5, 4, 3, 2, 1]);

    let mut times = transition_sample_times(&[2.0, 3.0]);
    times.sort_by(f64::total_cmp);
    assert_render_parity(
        "reverse-handles-speed",
        dir.path(),
        &prepared.project,
        &prepared.project,
        &times,
    );
}

#[test]
fn reversed_lut_clip_bakes_the_look_over_reversed_frames() {
    let dir = fixture_dir();
    std::fs::create_dir_all(dir.path().join("looks")).expect("looks directory");
    std::fs::write(dir.path().join("looks/invert.cube"), INVERT_LUT).expect("LUT");
    let mut project = load_fixture();
    let mut rev = media_clip("rev", "warm", 0.0, 2.0, 1.0, 1.0);
    rev.properties.insert("reverse".to_string(), json!(true));
    rev.properties.insert(
        "colorGrade".to_string(),
        json!({ "lut": { "path": "looks/invert.cube", "strength": 1.0 } }),
    );
    project.timeline.tracks[0].transitions.clear();
    project.timeline.tracks[0].items = vec![rev];
    project.timeline.duration_seconds = 2.0;

    let prepared = prepare_project_for_render(dir.path(), &project).expect("prepare reverse LUT");
    assert_eq!(
        report_stages(&prepared),
        [("reverse".to_string(), false), ("lut".to_string(), false)],
        "the LUT is baked over the reversed intermediate"
    );
    let (item, media) = prepared_item(&prepared, "rev");
    assert_eq!(prepared.reports[1].prepared_media_id, media.id);
    assert!(!item.properties.contains_key("reverse"));
    assert!(!item.properties.contains_key("colorGrade"));
    let window = SourceWindow {
        source_in: 1.0,
        source_out: 3.0,
        speed: 1.0,
        reverse: true,
    };
    let centres = intermediate_centres(dir.path(), media);
    let order = assert_reversed_frames("reverse + LUT", &centres, window, 0.0, true);
    assert_eq!(order, [10, 9, 8, 7, 6, 5, 4, 3]);
}

#[test]
fn ges_rejects_reversed_clips_rendered_without_preparation() {
    let dir = fixture_dir();
    let project = reversed_project(2.0, 1.0, 1.0, 1.0);
    let plan = build_project_webm_render_plan(
        dir.path(),
        &project,
        "reverse-unprepared",
        RenderQualityProfile::FinalWebm,
    )
    .expect("render plan");
    let output = std::path::PathBuf::from(&plan.output_path);
    std::fs::create_dir_all(output.parent().expect("output parent")).expect("output dir");
    let errors = GstreamerGesRenderBackend::new()
        .render_cancellable(
            &SystemProcessRunner,
            &plan,
            &[],
            Duration::from_secs(60),
            None,
        )
        .expect_err("GES fails closed on unprepared reversed clips");
    assert!(
        errors[0].path.ends_with("].properties.reverse"),
        "{errors:?}"
    );
    assert!(!output.exists(), "nothing was rendered");
    // The prepared project renders.
    let prepared = prepare_project_for_render(dir.path(), &project).expect("prepare reverse");
    let plan = build_project_webm_render_plan(
        dir.path(),
        &prepared.project,
        "reverse-prepared",
        RenderQualityProfile::FinalWebm,
    )
    .expect("prepared render plan");
    assert!(render(&plan).is_file());
}

/// `pngenc` comes from the reviewed `png` plugin (GStreamer Good, LGPL) and is
/// allowed by the render plugin policy; reversed intermediates rely on its
/// `compression-level` and `snapshot` properties.
#[test]
fn pngenc_is_a_reviewed_png_plugin_encoder() {
    super::fixtures::start_runtime();
    let factory = gst::ElementFactory::find("pngenc").expect("pngenc is registered");
    let plugin = factory.plugin().expect("pngenc plugin");
    eprintln!(
        "pngenc: plugin {} package {:?} license {} version {}",
        plugin.plugin_name(),
        plugin.package(),
        plugin.license(),
        plugin.version()
    );
    assert_eq!(plugin.plugin_name(), "png");
    let decision = evaluate_gstreamer_factory(
        &GstFactoryInfo::new("pngenc")
            .plugin_name(plugin.plugin_name().to_string())
            .package(plugin.package().to_string())
            .license(plugin.license().to_string()),
    );
    assert_eq!(
        decision.verdict,
        PluginPolicyVerdict::Allowed,
        "{decision:?}"
    );
    let element = factory.create().build().expect("pngenc element");
    assert_eq!(element.property::<u32>("compression-level"), 6);
    assert!(!element.property::<bool>("snapshot"));
}

/// The spike's cost criterion through production preparation: a 30 s
/// 1920x1080 24 fps H.264 clip reversed in full. Run with `--ignored`.
#[test]
#[ignore = "encodes and reverses 30 s of 1080p video"]
fn reversing_thirty_seconds_of_1080p_fits_the_spike_budget() {
    super::fixtures::start_runtime();
    let dir = tempfile::tempdir().expect("cost project");
    std::fs::create_dir_all(dir.path().join("media")).expect("media dir");
    let source = dir.path().join("media/large.mp4");
    let fixture = gst::parse::launch(&format!(
        "videotestsrc pattern=smpte horizontal-speed=8 num-buffers=720 \
         ! video/x-raw,width=1920,height=1080,framerate=24/1 ! videoconvert \
         ! video/x-raw,format=I420 ! openh264enc bitrate=8000000 ! h264parse ! mp4mux \
         ! filesink location=\"{}\"",
        source.display()
    ))
    .expect("fixture pipeline");
    fixture
        .set_state(gst::State::Playing)
        .expect("encode fixture");
    let message = fixture
        .bus()
        .expect("bus")
        .timed_pop_filtered(
            gst::ClockTime::from_seconds(900),
            &[gst::MessageType::Eos, gst::MessageType::Error],
        )
        .expect("fixture finishes");
    fixture.set_state(gst::State::Null).expect("stop fixture");
    assert!(matches!(message.view(), gst::MessageView::Eos(..)));

    let mut project = load_fixture();
    project.render_settings.width = 1920;
    project.render_settings.height = 1080;
    project.media = vec![MediaAsset {
        id: "large".to_string(),
        name: Some("large".to_string()),
        relative_path: "media/large.mp4".to_string(),
        kind: MediaKind::Video,
        duration_seconds: 30.0,
        width: Some(1920),
        height: Some(1080),
        fps: Some(FPS),
        folder_id: None,
    }];
    let mut rev = media_clip("rev", "large", 0.0, 30.0, 0.0, 1.0);
    rev.properties.insert("reverse".to_string(), json!(true));
    project.timeline.tracks[0].transitions.clear();
    project.timeline.tracks[0].items = vec![rev];
    project.timeline.duration_seconds = 30.0;

    let started = Instant::now();
    let prepared = prepare_project_for_render(dir.path(), &project).expect("prepare 1080p");
    let first = started.elapsed();
    let (_, media) = prepared_item(&prepared, "rev");
    let size = std::fs::metadata(dir.path().join(&media.relative_path))
        .expect("intermediate")
        .len();
    let started = Instant::now();
    let again = prepare_project_for_render(dir.path(), &project).expect("prepare again");
    let second = started.elapsed();
    eprintln!(
        "1080p reverse: first preparation {:.1} s, intermediate {size} bytes, cache hit {} in {:.1} s",
        first.as_secs_f64(),
        again.reports[0].cache_hit,
        second.as_secs_f64()
    );
    assert!(again.reports[0].cache_hit);
    assert!(first <= Duration::from_secs(120), "{first:?}");
    assert!(size <= 4 * 1024 * 1024 * 1024);
}
