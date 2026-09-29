//! Clip speed through GES: sped-up clips move from their first frame, and
//! retimed clips render like canonical frames through transitions.
//!
//! Before the `gstreamer_video_speed` correction, on the staged GStreamer/GES
//! 1.24.2 runtime:
//! - Source 1.0-4.0 s at speed 2: GES output frames 0-7 all showed the source
//!   segment around 1.0 s (centre [235, 40, 29]) while canonical frames
//!   advance every 3.5 frames; frames 2-11 differed from canonical frames.
//! - Speed 1.5/0.5 clips through a 0.5 s crossfade and dip to black: every
//!   sampled frame failed parity (mismatch ratio 1).
//!
//! GES applies a clip's alpha envelope per source frame, so a slowed clip
//! steps its transition alpha once per source frame. At speed 0.5 an output
//! frame whose source time falls between two source frames shows the alpha
//! of a neighbouring source frame, one output frame of progress away.

use super::fixtures::{add_transition, plan, render};
use super::media::{decode_frames, FPS, HEIGHT, WIDTH};
use super::parity::{assert_render_parity, fixture_dir, load_fixture, transition_sample_times};
use serde_json::json;
use std::collections::BTreeMap;
use video_creater_lib::precompose::render_canonical_frame_rgba;
use video_creater_lib::project::model::*;

fn retimed_clip(
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

fn project_with(items: Vec<TimelineItem>, duration: f64) -> VideoProject {
    let mut project = load_fixture();
    project.timeline.tracks[0].items = items;
    project.timeline.tracks[0].transitions.clear();
    project.timeline.duration_seconds = duration;
    project
}

fn centre(frame: &[u8]) -> [u8; 3] {
    let offset = (HEIGHT / 2 * WIDTH + WIDTH / 2) * 4;
    [frame[offset], frame[offset + 1], frame[offset + 2]]
}

#[test]
fn sped_up_clip_starts_moving_on_its_first_frame() {
    let dir = fixture_dir();
    // Source 1.0-4.0 s at speed 2 over 1.5 timeline seconds: a new 7-frame
    // colour segment every 3.5 output frames.
    let project = project_with(vec![retimed_clip("fast", "warm", 0.0, 1.5, 1.0, 2.0)], 1.5);
    let rendered = decode_frames(&render(&plan(dir.path(), &project, "speed-first-frames")));
    let mut observed = Vec::new();
    let mut mismatches = Vec::new();
    for index in 0..12_i64 {
        let seconds = index as f64 / FPS;
        let canonical = render_canonical_frame_rgba(dir.path(), &project, seconds, None)
            .unwrap_or_else(|errors| panic!("canonical frame {index}: {errors:?}"));
        let ges = centre(
            rendered
                .get(&index)
                .unwrap_or_else(|| panic!("rendered frame {index}")),
        );
        let expected = centre(&canonical);
        observed.push((index, ges, expected));
        if ges
            .iter()
            .zip(expected)
            .any(|(left, right)| left.abs_diff(right) > 16)
        {
            mismatches.push(index);
        }
    }
    let longest_run = observed
        .windows(2)
        .fold((1, 1), |(longest, current), pair| {
            let same = pair[0]
                .1
                .iter()
                .zip(pair[1].1)
                .all(|(left, right)| left.abs_diff(right) <= 2);
            let current = if same { current + 1 } else { 1 };
            (longest.max(current), current)
        })
        .0;
    eprintln!("sped-up first frames (index, ges, canonical): {observed:?}; longest identical run {longest_run}; mismatches {mismatches:?}");
    assert!(
        mismatches.is_empty(),
        "frames {mismatches:?} differ from canonical frames"
    );
    assert!(
        longest_run <= 4,
        "a run of {longest_run} identical frames while source time advances"
    );
}

#[test]
fn speed_changes_render_like_canonical_frames_through_transitions() {
    let dir = fixture_dir();
    let mut project = project_with(
        vec![
            retimed_clip("a", "warm", 0.0, 2.0, 0.5, 1.5),
            retimed_clip("b", "cool", 2.0, 2.0, 1.0, 0.5),
            retimed_clip("c", "warm", 4.0, 2.0, 0.5, 1.5),
            retimed_clip("d", "cool", 6.0, 2.0, 1.0, 0.5),
        ],
        8.0,
    );
    add_transition(
        &mut project,
        "track-video",
        ("a", "b"),
        TransitionKind::Crossfade,
        0.5,
    );
    add_transition(
        &mut project,
        "track-video",
        ("c", "d"),
        TransitionKind::DipToBlack,
        0.5,
    );
    // Two frames into each clip: source time lands on a source frame at both
    // speeds (3 frames at 1.5, 1 frame at 0.5).
    let mut times = transition_sample_times(&[2.0, 6.0]);
    times.extend([0.0, 2.0, 4.0, 6.0].map(|start| start + 2.0 / FPS));
    let comparison =
        assert_render_parity("speed-transitions", dir.path(), &project, &project, &times);
    let worst = comparison["comparedFrames"]
        .as_array()
        .expect("compared frames")
        .iter()
        .filter_map(|frame| frame["mismatchRatio"].as_f64())
        .fold(0.0_f64, f64::max);
    // Three frames into the slowed clips, source time falls between source
    // frames: the rendered alpha is that of a neighbouring output frame.
    let rendered = decode_frames(&render(&plan(dir.path(), &project, "speed-between-frames")));
    for seconds in [2.0 + 3.0 / FPS, 6.0 + 3.0 / FPS] {
        let index = (seconds * FPS).round() as i64;
        let ges = centre(rendered.get(&index).expect("rendered frame"));
        let neighbours = [seconds - 1.0 / FPS, seconds + 1.0 / FPS].map(|at| {
            centre(
                &render_canonical_frame_rgba(dir.path(), &project, at, None)
                    .unwrap_or_else(|errors| panic!("canonical frame at {at}: {errors:?}")),
            )
        });
        eprintln!("slowed clip at {seconds:.4}s: ges {ges:?}, canonical neighbours {neighbours:?}");
        for channel in 0..3 {
            let low = neighbours[0][channel].min(neighbours[1][channel]);
            let high = neighbours[0][channel].max(neighbours[1][channel]);
            assert!(
                ges[channel] + 16 >= low && ges[channel] <= high.saturating_add(16),
                "{seconds}s channel {channel}: ges {ges:?} outside canonical neighbours {neighbours:?}"
            );
        }
    }
    eprintln!("speed-transitions worst mismatch ratio {worst}");
}
