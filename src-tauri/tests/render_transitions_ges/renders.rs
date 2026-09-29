//! Rendered frames and audio at transition windows.

use super::fixtures::*;
use super::media::*;
use video_creater_lib::project::model::TransitionKind;

/// Frame 48 is output time 2.0 s, the midpoint of the `[1.5, 2.5]` window.
const MIDPOINT_FRAME: i64 = 48;
/// Frame 42 is output time 1.75 s, progress 0.25.
const QUARTER_FRAME: i64 = 42;
const TOLERANCE: f64 = 24.0;

fn render_frames(
    kind: Option<TransitionKind>,
    job: &str,
) -> std::collections::BTreeMap<i64, Vec<u8>> {
    let dir = project_dir();
    let output = render(&plan(dir.path(), &two_clip_project(kind), job));
    let frames = decode_frames(&output);
    assert_eq!(frames.len(), 96, "4 s at 24 fps");
    frames
}

fn assert_rgb(label: &str, actual: [f64; 3], expected: [f64; 3]) {
    eprintln!("{label}: rgb {actual:.1?} (expected {expected:.1?})");
    for (channel, (actual, expected)) in actual.iter().zip(expected).enumerate() {
        assert!(
            (actual - expected).abs() <= TOLERANCE,
            "{label}: channel {channel} is {actual:.1}, expected {expected:.1} ± {TOLERANCE}"
        );
    }
}

fn mix(incoming: f64) -> [f64; 3] {
    [255.0 * (1.0 - incoming), 0.0, 255.0 * incoming]
}

#[test]
fn crossfade_draws_the_incoming_clip_over_the_outgoing_clip_at_progress_alpha() {
    let frames = render_frames(Some(TransitionKind::Crossfade), "crossfade");
    assert_rgb("crossfade before", frame_mean(&frames[&12]), mix(0.0));
    assert_rgb(
        "crossfade quarter",
        frame_mean(&frames[&QUARTER_FRAME]),
        mix(0.25),
    );
    let midpoint = frame_mean(&frames[&MIDPOINT_FRAME]);
    assert_rgb("crossfade midpoint", midpoint, mix(0.5));
    assert!(midpoint[0] > 40.0 && midpoint[0] < 215.0 && midpoint[2] > 40.0 && midpoint[2] < 215.0);
    assert_rgb("crossfade after", frame_mean(&frames[&84]), mix(1.0));
}

#[test]
fn dips_pass_through_an_opaque_solid_at_the_cut() {
    let black = render_frames(Some(TransitionKind::DipToBlack), "dip-black");
    assert_rgb(
        "dip to black quarter",
        frame_mean(&black[&QUARTER_FRAME]),
        [127.5, 0.0, 0.0],
    );
    assert_rgb(
        "dip to black midpoint",
        frame_mean(&black[&MIDPOINT_FRAME]),
        [0.0; 3],
    );
    assert_rgb(
        "dip to black three quarters",
        frame_mean(&black[&54]),
        [0.0, 0.0, 127.5],
    );

    let white = render_frames(Some(TransitionKind::DipToWhite), "dip-white");
    assert_rgb(
        "dip to white quarter",
        frame_mean(&white[&QUARTER_FRAME]),
        [255.0, 127.5, 127.5],
    );
    assert_rgb(
        "dip to white midpoint",
        frame_mean(&white[&MIDPOINT_FRAME]),
        [255.0; 3],
    );
}

#[test]
fn wipe_reveals_the_incoming_clip_from_the_left_edge_of_the_canvas() {
    let frames = render_frames(Some(TransitionKind::Wipe), "wipe");
    let midpoint = &frames[&MIDPOINT_FRAME];
    assert_rgb(
        "wipe midpoint left",
        region_mean(midpoint, 4..56, 8..64),
        mix(1.0),
    );
    assert_rgb(
        "wipe midpoint right",
        region_mean(midpoint, 72..124, 8..64),
        mix(0.0),
    );
    let quarter = &frames[&QUARTER_FRAME];
    assert_rgb(
        "wipe quarter left",
        region_mean(quarter, 4..26, 8..64),
        mix(1.0),
    );
    assert_rgb(
        "wipe quarter right",
        region_mean(quarter, 40..124, 8..64),
        mix(0.0),
    );
    assert_rgb("wipe start", frame_mean(&frames[&35]), mix(0.0));
    assert_rgb("wipe end", frame_mean(&frames[&61]), mix(1.0));
}

#[test]
fn wipe_masks_in_canvas_space_when_the_incoming_clip_is_smaller_than_the_canvas() {
    let dir = project_dir();
    let mut project = two_clip_project(Some(TransitionKind::Wipe));
    project.timeline.tracks[0].items[1].properties.insert(
        "transform".to_string(),
        serde_json::json!({ "centerX": 0.5, "centerY": 0.5, "width": 0.5, "height": 0.5 }),
    );
    let frames = decode_frames(&render(&plan(dir.path(), &project, "wipe-box")));
    // The blue box spans x 32..96, y 18..54. At the midpoint the reveal edge
    // is canvas x 64, halfway across the box.
    let midpoint = &frames[&MIDPOINT_FRAME];
    assert_rgb(
        "boxed wipe midpoint revealed",
        region_mean(midpoint, 36..60, 22..50),
        mix(1.0),
    );
    assert_rgb(
        "boxed wipe midpoint masked",
        region_mean(midpoint, 68..92, 22..50),
        mix(0.0),
    );
    assert_rgb(
        "boxed wipe midpoint outside",
        region_mean(midpoint, 4..28, 22..50),
        mix(0.0),
    );
    // At progress 0.25 the edge (x 32) has only reached the box's left side.
    assert_rgb(
        "boxed wipe quarter",
        region_mean(&frames[&QUARTER_FRAME], 36..92, 22..50),
        mix(0.0),
    );
    let end = &frames[&66];
    assert_rgb(
        "boxed wipe after",
        region_mean(end, 36..92, 22..50),
        mix(1.0),
    );
    assert_rgb(
        "boxed wipe after outside",
        region_mean(end, 4..28, 22..50),
        [0.0; 3],
    );
}

#[test]
fn range_renders_through_a_transition_window_keep_partial_progress() {
    let dir = project_dir();
    let project = two_clip_project(Some(TransitionKind::Crossfade));
    let range = range_plan(dir.path(), &project, "crossfade-range", 2.0, 3.0);
    assert_eq!(range.transitions[0].start_seconds, -0.5);
    let frames = decode_frames(&render(&range));
    // Output 0 s is timeline 2.0 s (progress 0.5); output 0.25 s is progress 0.75.
    assert_rgb("range start", frame_mean(&frames[&0]), mix(0.5));
    assert_rgb("range quarter", frame_mean(&frames[&6]), mix(0.75));
    assert_rgb("range after window", frame_mean(&frames[&18]), mix(1.0));

    let wipe = two_clip_project(Some(TransitionKind::Wipe));
    let frames = decode_frames(&render(&range_plan(
        dir.path(),
        &wipe,
        "wipe-range",
        1.75,
        2.75,
    )));
    // Output 0.25 s is timeline 2.0 s: the wipe has revealed the left half.
    let midpoint = &frames[&6];
    assert_rgb(
        "wipe range left",
        region_mean(midpoint, 4..56, 8..64),
        mix(1.0),
    );
    assert_rgb(
        "wipe range right",
        region_mean(midpoint, 72..124, 8..64),
        mix(0.0),
    );
}

#[test]
fn audio_crossfades_keep_equal_power_through_the_window() {
    let dir = project_dir();
    let samples = decode_audio(&render(&plan(
        dir.path(),
        &audio_crossfade_project(),
        "audio",
    )));
    let steady = (rms(&samples, 0.5, 1.0) + rms(&samples, 3.0, 3.5)) / 2.0;
    let midpoint = rms(&samples, 1.975, 2.025);
    let decibels = 20.0 * (midpoint / steady).log10();
    eprintln!(
        "audio crossfade: steady rms {steady:.4}, midpoint rms {midpoint:.4} ({decibels:+.2} dB)"
    );
    assert!(steady > 0.2, "steady-state tone is audible: {steady}");
    assert!(
        decibels.abs() <= 3.0,
        "midpoint is {decibels:+.2} dB from steady state"
    );
}

#[test]
fn audio_fade_ins_start_silent() {
    let dir = project_dir();
    let samples = decode_audio(&render(&plan(dir.path(), &audio_fade_project(), "fade")));
    let steady = rms(&samples, 1.0, 1.5);
    let start = rms(&samples, 0.0, 0.02);
    let halfway = rms(&samples, 0.24, 0.26);

    eprintln!(
        "audio fade-in: start rms {start:.4}, 0.25 s rms {halfway:.4}, steady rms {steady:.4}"
    );
    assert!(steady > 0.2, "steady-state tone is audible: {steady}");
    assert!(
        start < steady * 0.1,
        "fade-in starts silent: {start} vs {steady}"
    );
    assert!(
        (halfway / steady - 0.5).abs() < 0.15,
        "linear fade-in is halfway at 0.25 s"
    );
}
