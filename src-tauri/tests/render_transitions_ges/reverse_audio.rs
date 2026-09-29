//! Reversed audio clips: render preparation decodes the clip's source span
//! (with its audio transition handles) forwards at 48 kHz, keeping the
//! source channel layout, and writes the samples back to front as a cached
//! WAV. The prepared clip reads that WAV forwards and keeps its speed, so
//! retiming runs on the reversed intermediate afterwards.

use super::fixtures::*;
use super::media::{decode_audio, dominant_frequency, rms, FPS, SAMPLE_RATE};
use serde_json::json;
use std::path::Path;
use video_creater_lib::precompose::{prepare_project_for_render, PreparedProject};
use video_creater_lib::project::model::*;
use video_creater_lib::render_pipeline::transition_plan::plan_clip_transitions;

const SWEEP_SECONDS: f64 = 4.0;
const SWEEP_START_HZ: f64 = 200.0;
const SWEEP_END_HZ: f64 = 2000.0;

/// The instantaneous frequency of the sweep at source second `seconds`.
fn sweep_hz(seconds: f64) -> f64 {
    SWEEP_START_HZ + (SWEEP_END_HZ - SWEEP_START_HZ) * seconds / SWEEP_SECONDS
}

/// Writes a 4 s stereo WAV sweep rising from 200 Hz to 2000 Hz: the left
/// channel at half amplitude and the right channel at a quarter, so the
/// channel layout is measurable.
fn write_stereo_sweep(path: &Path) {
    let spec = hound::WavSpec {
        channels: 2,
        sample_rate: SAMPLE_RATE as u32,
        bits_per_sample: 16,
        sample_format: hound::SampleFormat::Int,
    };
    let mut writer = hound::WavWriter::create(path, spec).expect("sweep wav");
    let rate = (SWEEP_END_HZ - SWEEP_START_HZ) / SWEEP_SECONDS;
    for index in 0..(SWEEP_SECONDS * SAMPLE_RATE as f64) as usize {
        let t = index as f64 / SAMPLE_RATE as f64;
        let value = (std::f64::consts::TAU * (SWEEP_START_HZ * t + rate * t * t / 2.0)).sin();
        for amplitude in [0.5, 0.25] {
            writer
                .write_sample((value * amplitude * f64::from(i16::MAX)) as i16)
                .expect("sweep sample");
        }
    }
    writer.finalize().expect("finish sweep");
}

/// The project folder of `project_dir` plus the stereo sweep as audio media.
fn sweep_project(dir: &Path) -> VideoProject {
    write_stereo_sweep(&dir.join("media").join("sweep.wav"));
    let mut project = base_project();
    project.media.push(MediaAsset {
        id: "sweep".to_string(),
        name: Some("sweep".to_string()),
        relative_path: "media/sweep.wav".to_string(),
        kind: MediaKind::Audio,
        duration_seconds: SWEEP_SECONDS,
        width: None,
        height: None,
        fps: None,
        folder_id: None,
    });
    project
}

fn reversed_sweep(start: f64, duration: f64, source_in: f64, speed: f64) -> TimelineItem {
    with_properties(
        clip(
            "sweep-clip",
            TimelineItemKind::AudioClip,
            "sweep",
            start,
            duration,
            source_in,
        ),
        json!({ "sourceOut": source_in + duration * speed, "speed": speed, "reverse": true }),
    )
}

fn prepared_sweep(prepared: &PreparedProject) -> (&TimelineItem, &MediaAsset) {
    let item = &prepared.project.timeline.tracks[AUDIO_TRACK_INDEX]
        .items
        .iter()
        .find(|item| item.id == "sweep-clip")
        .expect("prepared sweep clip");
    let TimelineSource::Media { media_id } = &item.source else {
        panic!("prepared sweep is media-backed");
    };
    let media = prepared
        .project
        .media
        .iter()
        .find(|media| media.id == *media_id)
        .expect("prepared sweep media");
    (item, media)
}

fn report_stages(prepared: &PreparedProject) -> Vec<(String, bool)> {
    prepared
        .reports
        .iter()
        .map(|report| (report.stage.clone(), report.cache_hit))
        .collect()
}

fn assert_hz(label: &str, measured: f64, expected: f64, tolerance: f64) {
    eprintln!("{label}: {measured:.1} Hz, expected {expected:.1} Hz");
    assert!(
        (measured - expected).abs() <= expected * tolerance,
        "{label}: measured {measured:.1} Hz, expected {expected:.1} Hz"
    );
}

#[test]
fn reversed_audio_clip_plays_a_reversed_wav_with_its_channels() {
    let dir = project_dir();
    let mut project = sweep_project(dir.path());
    project.timeline.tracks[0].items = vec![clip(
        "red-clip",
        TimelineItemKind::VideoClip,
        "red",
        0.0,
        4.0,
        0.0,
    )];
    // Tone A (0-2 s) crossfades over 1 s into the sweep reversed over source
    // 1-3 s (2-4 s). Its head handle reads source 3-3.5 s.
    project.timeline.tracks[AUDIO_TRACK_INDEX].items = vec![
        clip(
            "tone-a-clip",
            TimelineItemKind::AudioClip,
            "tone-a",
            0.0,
            2.0,
            1.0,
        ),
        reversed_sweep(2.0, 2.0, 1.0, 1.0),
    ];
    project.timeline.duration_seconds = 4.0;
    add_transition(
        &mut project,
        "track-audio",
        ("tone-a-clip", "sweep-clip"),
        TransitionKind::Crossfade,
        1.0,
    );

    let prepared = prepare_project_for_render(dir.path(), &project).expect("prepare reverse");
    assert_eq!(
        report_stages(&prepared),
        [("audioReverse".to_string(), false)]
    );
    let (item, media) = prepared_sweep(&prepared);
    assert!(media.id.starts_with("audio-reverse-"), "{media:?}");
    assert_eq!(media.kind, MediaKind::Audio);
    assert!(!item.properties.contains_key("reverse"), "{item:?}");
    assert_eq!(item.properties["sourceIn"], json!(0.5));
    assert_eq!(item.properties["sourceOut"], json!(2.5));
    assert!((media.duration_seconds - 2.5).abs() < 1e-3, "{media:?}");
    let transitions = plan_clip_transitions(&prepared.project);
    assert_eq!(transitions.len(), 1);
    assert!(transitions[0].audio && (transitions[0].duration_seconds - 1.0).abs() < 1e-6);

    // The WAV reads source 3.5 s down to 1 s.
    let mut reader = hound::WavReader::open(dir.path().join(&media.relative_path)).expect("wav");
    let spec = reader.spec();
    assert_eq!((spec.channels, spec.sample_rate), (2, SAMPLE_RATE as u32));
    let samples = reader
        .samples::<i16>()
        .map(|sample| f32::from(sample.expect("sample")) / f32::from(i16::MAX))
        .collect::<Vec<_>>();
    let left = samples.iter().step_by(2).copied().collect::<Vec<_>>();
    let right = samples
        .iter()
        .skip(1)
        .step_by(2)
        .copied()
        .collect::<Vec<_>>();
    let seconds = left.len() as f64 / SAMPLE_RATE as f64;
    eprintln!(
        "reversed sweep wav: {seconds:.4} s, {} channels",
        spec.channels
    );
    assert!((seconds - 2.5).abs() < 0.002, "{seconds}");
    assert_hz(
        "wav head",
        dominant_frequency(&left, 0.05, 0.25),
        sweep_hz(3.35),
        0.05,
    );
    assert_hz(
        "wav tail",
        dominant_frequency(&left, 2.25, 2.45),
        sweep_hz(1.15),
        0.05,
    );
    let (left_rms, right_rms) = (rms(&left, 0.1, 2.4), rms(&right, 0.1, 2.4));
    eprintln!("channel rms left {left_rms:.4} right {right_rms:.4}");
    assert!(
        (left_rms / right_rms - 2.0).abs() < 0.1,
        "channels kept in place"
    );

    let again = prepare_project_for_render(dir.path(), &project).expect("prepare again");
    assert_eq!(report_stages(&again), [("audioReverse".to_string(), true)]);
    assert_eq!(again.project, prepared.project);

    // After the crossfade the sweep falls through GES: timeline 2.8 s is
    // source 2.2 s and 3.8 s is source 1.2 s.
    let rendered = decode_audio(&render(&plan(
        dir.path(),
        &prepared.project,
        "reverse-audio",
    )));
    assert_hz(
        "render 2.8 s",
        dominant_frequency(&rendered, 2.7, 2.9),
        sweep_hz(2.2),
        0.06,
    );
    assert_hz(
        "render 3.8 s",
        dominant_frequency(&rendered, 3.7, 3.9),
        sweep_hz(1.2),
        0.06,
    );
}

#[test]
fn reversed_audio_at_double_speed_keeps_pitch_and_length() {
    let dir = project_dir();
    let mut project = sweep_project(dir.path());
    project.timeline.tracks[0].items = vec![clip(
        "red-clip",
        TimelineItemKind::VideoClip,
        "red",
        0.0,
        1.5,
        0.0,
    )];
    // Source 0.5-3.5 s reversed at speed 2 over 1.5 s.
    project.timeline.tracks[AUDIO_TRACK_INDEX].items = vec![reversed_sweep(0.0, 1.5, 0.5, 2.0)];
    project.timeline.duration_seconds = 1.5;

    let prepared = prepare_project_for_render(dir.path(), &project).expect("prepare reverse");
    assert_eq!(
        report_stages(&prepared),
        [
            ("audioReverse".to_string(), false),
            ("audioRetime".to_string(), false)
        ],
        "retiming runs on the reversed intermediate"
    );
    let (item, media) = prepared_sweep(&prepared);
    assert!(media.id.starts_with("audio-retime-"), "{media:?}");
    assert!(!item.properties.contains_key("reverse"));
    assert!(!item.properties.contains_key("speed"));

    let rendered = decode_audio(&render(&plan(
        dir.path(),
        &prepared.project,
        "reverse-speed",
    )));
    let seconds = rendered.len() as f64 / SAMPLE_RATE as f64;
    eprintln!("reversed speed 2 render: {seconds:.4} s");
    assert!(
        (seconds - 1.5).abs() <= 1.0 / FPS,
        "output length {seconds}"
    );
    // Output 0.2 s reads source 3.5 - 0.4 and 1.3 s reads 3.5 - 2.6, at the
    // source pitch (varispeed would double it).
    assert_hz(
        "speed 2 at 0.2 s",
        dominant_frequency(&rendered, 0.1, 0.3),
        sweep_hz(3.1),
        0.08,
    );
    assert_hz(
        "speed 2 at 1.3 s",
        dominant_frequency(&rendered, 1.2, 1.4),
        sweep_hz(0.9),
        0.08,
    );
}
