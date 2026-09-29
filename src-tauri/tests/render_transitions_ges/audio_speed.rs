//! Retimed audio clips. GES 1.24.2 has no pitch-preserving audio time effect
//! (`scaletempo rate` is read-only, so GES does not register it), so render
//! preparation retimes the audio through `scaletempo` into a cached
//! intermediate that plays at speed 1 (`precompose/audio_retime.rs`).

use super::fixtures::*;
use super::media::{decode_audio, dominant_frequency, rms, write_tone, FPS, SAMPLE_RATE};
use super::structure::summary;
use ges::prelude::*;
use gstreamer_editing_services as ges;
use serde_json::json;
use std::path::Path;
use video_creater_lib::precompose::prepare_project_for_render;
use video_creater_lib::project::model::*;
use video_creater_lib::render_pipeline::gstreamer_backend::GstreamerGesRenderBackend;
use video_creater_lib::render_pipeline::process::SystemProcessRunner;

// `BaseEffectExt::is_time_effect` needs the crate's `v1_18` feature, which the
// app does not enable; the GES 1.24.2 runtime exports the C function.
#[link(name = "ges-1.0")]
unsafe extern "C" {
    fn ges_base_effect_is_time_effect(
        effect: *mut gstreamer_editing_services::ffi::GESBaseEffect,
    ) -> gstreamer::glib::ffi::gboolean;
}

/// Red video over `[0, duration)` and one retimed `tone-a` clip.
fn retimed_tone_project(
    duration: f64,
    source_in: f64,
    speed: f64,
    extra: serde_json::Value,
) -> VideoProject {
    let mut project = base_project();
    project.timeline.tracks[0].items = vec![clip(
        "red-clip",
        TimelineItemKind::VideoClip,
        "red",
        0.0,
        duration,
        0.0,
    )];
    let mut properties = json!({ "sourceOut": source_in + duration * speed, "speed": speed });
    if let (Some(target), Some(extra)) = (properties.as_object_mut(), extra.as_object()) {
        target.extend(extra.clone());
    }
    project.timeline.tracks[AUDIO_TRACK_INDEX].items = vec![with_properties(
        clip(
            "tone-a-clip",
            TimelineItemKind::AudioClip,
            "tone-a",
            0.0,
            duration,
            source_in,
        ),
        properties,
    )];
    project.timeline.duration_seconds = duration;
    project
}

/// Prepares `project` like project export, then renders it and decodes the audio.
fn render_audio(dir: &Path, project: &VideoProject, job: &str) -> Vec<f32> {
    let prepared = prepare_project_for_render(dir, project).expect("prepare retimed audio");
    decode_audio(&render(&plan(dir, &prepared.project, job)))
}

/// Records why retimed audio is prepared: GES does not register `scaletempo
/// rate` as a time effect on this runtime.
#[test]
fn scaletempo_rate_is_not_a_ges_time_effect() {
    let dir = project_dir();
    let tone = dir.path().join("media").join("tone-probe.webm");
    write_tone(&tone, 440.0, 1.0);
    let uri = gstreamer::glib::filename_to_uri(&tone, None).expect("tone uri");
    let asset = ges::UriClipAsset::request_sync(&uri).expect("tone asset");
    let timeline = ges::Timeline::new_audio_video();
    let layer = timeline.append_layer();
    let clip = layer
        .add_asset(&asset, None, None, None, ges::TrackType::AUDIO)
        .expect("tone clip");
    let effect = ges::Effect::new("scaletempo rate=2.0").expect("scaletempo effect");
    clip.add(&effect).expect("attach scaletempo");
    let base = effect.upcast_ref::<ges::BaseEffect>();
    let is_time_effect = unsafe {
        use gstreamer::glib::translate::ToGlibPtr;
        ges_base_effect_is_time_effect(base.to_glib_none().0)
    } != 0;
    assert!(
        !is_time_effect,
        "GES now treats scaletempo rate as a time effect; retimed audio could render without preparation"
    );
}

#[test]
fn retimed_audio_keeps_pitch_and_timeline_length() {
    let dir = project_dir();
    let project = retimed_tone_project(1.5, 0.5, 2.0, json!({}));
    let samples = render_audio(dir.path(), &project, "audio-speed");
    let seconds = samples.len() as f64 / SAMPLE_RATE as f64;
    let frequency = dominant_frequency(&samples, 0.3, 1.2);
    let tail = rms(&samples, 1.3, 1.45);
    eprintln!("retimed audio: {seconds:.4} s, {frequency:.1} Hz, tail rms {tail:.4}");
    assert!(
        (seconds - 1.5).abs() <= 1.0 / FPS,
        "output length {seconds}"
    );
    assert!(
        (frequency - 440.0).abs() <= 440.0 * 0.03,
        "pitch is kept at 440 Hz, measured {frequency}"
    );
    assert!(tail > 0.2, "the tone plays to the clip end: {tail}");
}

#[test]
fn retimed_audio_fades_land_in_timeline_time() {
    let dir = project_dir();
    // sourceIn 1 at speed 2 consumes source 1-4 s over 1.5 timeline seconds.
    let project = retimed_tone_project(1.5, 1.0, 2.0, json!({ "fadeInSeconds": 0.5 }));
    let samples = render_audio(dir.path(), &project, "audio-speed-fade");
    let steady = rms(&samples, 0.8, 1.3);
    let start = rms(&samples, 0.0, 0.02);
    let quarter = rms(&samples, 0.24, 0.26);
    let crossings = (1..=10)
        .map(|step| {
            let at = f64::from(step) * 0.05;
            format!(
                "{at:.2}s={:.3}",
                rms(&samples, at - 0.01, at + 0.01) / steady
            )
        })
        .collect::<Vec<_>>();
    eprintln!("retimed fade: start {start:.4}, 0.25 s {quarter:.4}, steady {steady:.4}; ramp {crossings:?}");
    assert!(steady > 0.2, "steady-state tone is audible: {steady}");
    assert!(start < 0.02, "fade-in starts silent: {start}");
    assert!(
        (quarter / steady - 0.5).abs() <= 0.125,
        "linear fade-in is halfway at 0.25 timeline seconds: {}",
        quarter / steady
    );
}

#[test]
fn retimed_audio_crossfade_uses_speed_scaled_handles() {
    let dir = project_dir();
    let mut project = base_project();
    project.timeline.tracks[0].items = vec![clip(
        "red-clip",
        TimelineItemKind::VideoClip,
        "red",
        0.0,
        3.25,
        0.0,
    )];
    project.timeline.tracks[AUDIO_TRACK_INDEX].items = vec![
        with_properties(
            clip(
                "tone-a-clip",
                TimelineItemKind::AudioClip,
                "tone-a",
                0.0,
                1.25,
                1.0,
            ),
            json!({ "sourceOut": 3.5, "speed": 2.0 }),
        ),
        clip(
            "tone-b-clip",
            TimelineItemKind::AudioClip,
            "tone-b",
            1.25,
            2.0,
            1.0,
        ),
    ];
    project.timeline.duration_seconds = 3.25;
    add_transition(
        &mut project,
        "track-audio",
        ("tone-a-clip", "tone-b-clip"),
        TransitionKind::Crossfade,
        0.5,
    );
    let samples = render_audio(dir.path(), &project, "audio-speed-crossfade");
    let steady = (rms(&samples, 0.2, 0.8) + rms(&samples, 2.0, 3.0)) / 2.0;
    let midpoint = rms(&samples, 1.225, 1.275);
    let decibels = 20.0 * (midpoint / steady).log10();
    let quietest = (0..50)
        .map(|step| 1.0 + f64::from(step) * 0.01)
        .map(|at| rms(&samples, at, at + 0.01))
        .fold(f64::INFINITY, f64::min);
    eprintln!("retimed crossfade: steady {steady:.4}, midpoint {midpoint:.4} ({decibels:+.2} dB), quietest 10 ms {quietest:.4}");
    assert!(steady > 0.2, "steady-state tone is audible: {steady}");
    assert!(
        decibels.abs() <= 0.5,
        "midpoint is {decibels:+.2} dB from steady state"
    );
    assert!(
        quietest > 0.1,
        "no silence gap through the window: {quietest}"
    );
}

#[test]
fn preparation_substitutes_a_cached_speed_one_intermediate_with_handles() {
    let dir = project_dir();
    let mut project = retimed_tone_project(1.5, 0.5, 2.0, json!({}));
    project.timeline.tracks[AUDIO_TRACK_INDEX].items[0].duration_seconds = 1.25;
    project.timeline.tracks[AUDIO_TRACK_INDEX].items[0]
        .properties
        .insert("sourceOut".to_string(), json!(3.0));
    project.timeline.tracks[AUDIO_TRACK_INDEX]
        .items
        .push(with_properties(
            clip(
                "tone-b-clip",
                TimelineItemKind::AudioClip,
                "tone-b",
                1.25,
                1.0,
                1.0,
            ),
            json!({}),
        ));
    add_transition(
        &mut project,
        "track-audio",
        ("tone-a-clip", "tone-b-clip"),
        TransitionKind::Crossfade,
        0.5,
    );

    let prepared = prepare_project_for_render(dir.path(), &project).expect("prepare");
    let item = &prepared.project.timeline.tracks[AUDIO_TRACK_INDEX].items[0];
    let TimelineSource::Media { media_id } = &item.source else {
        panic!("prepared retimed clip is media-backed");
    };
    let media = prepared
        .project
        .media
        .iter()
        .find(|media| media.id == *media_id)
        .expect("prepared media");
    eprintln!("prepared retimed clip: {item:?}\n{media:?}");
    assert!(media_id.starts_with("audio-retime-"));
    assert_eq!(item.properties.get("speed"), None);
    // 0.25 s tail handle; no head handle: the clip starts the track.
    assert_eq!(item.properties["sourceIn"], json!(0.0));
    assert_eq!(item.properties["sourceOut"], json!(1.25));
    assert!((media.duration_seconds - 1.5).abs() < 0.01, "{media:?}");
    let report = prepared
        .reports
        .iter()
        .find(|report| report.stage == "audioRetime")
        .expect("retime report");
    assert!(!report.cache_hit);
    let again = prepare_project_for_render(dir.path(), &project).expect("prepare again");
    assert!(again
        .reports
        .iter()
        .any(|report| report.stage == "audioRetime" && report.cache_hit));

    let summary = summary(
        dir.path(),
        &plan(dir.path(), &prepared.project, "audio-speed-summary"),
    );
    assert!(!summary.contains("scaletempo"), "{summary}");
}

#[test]
fn slowed_audio_keeps_pitch() {
    let dir = project_dir();
    let project = retimed_tone_project(3.0, 0.5, 0.5, json!({}));
    let samples = render_audio(dir.path(), &project, "audio-slow");
    let seconds = samples.len() as f64 / SAMPLE_RATE as f64;
    let frequency = dominant_frequency(&samples, 0.3, 2.7);
    eprintln!("slowed audio: {seconds:.4} s, {frequency:.1} Hz");
    assert!(
        (seconds - 3.0).abs() <= 1.0 / FPS,
        "output length {seconds}"
    );
    assert!(
        (frequency - 440.0).abs() <= 440.0 * 0.03,
        "pitch is kept at 440 Hz, measured {frequency}"
    );
}

#[test]
fn unprepared_retimed_audio_is_rejected_by_the_ges_backend() {
    let dir = project_dir();
    let project = retimed_tone_project(1.5, 0.5, 2.0, json!({}));
    let plan = plan(dir.path(), &project, "audio-speed-unprepared");
    std::fs::create_dir_all(
        Path::new(&plan.output_path)
            .parent()
            .expect("output parent"),
    )
    .expect("output dir");
    let errors = GstreamerGesRenderBackend::new()
        .render_cancellable(
            &SystemProcessRunner,
            &plan,
            &[],
            std::time::Duration::from_secs(60),
            None,
        )
        .expect_err("unprepared retimed audio must fail");
    assert_eq!(errors[0].path, "renderPlan.audioClips[0].properties.speed");
}

/// Writes a 4 s mono WAV whose frequency rises linearly from 200 Hz to 2000 Hz.
fn write_sweep(path: &Path) {
    let spec = hound::WavSpec {
        channels: 1,
        sample_rate: SAMPLE_RATE as u32,
        bits_per_sample: 16,
        sample_format: hound::SampleFormat::Int,
    };
    let mut writer = hound::WavWriter::create(path, spec).expect("sweep wav");
    for index in 0..4 * SAMPLE_RATE {
        let t = index as f64 / SAMPLE_RATE as f64;
        let phase = std::f64::consts::TAU * (200.0 * t + 225.0 * t * t);
        writer
            .write_sample((phase.sin() * 0.5 * f64::from(i16::MAX)) as i16)
            .expect("sweep sample");
    }
    writer.finalize().expect("finish sweep");
}

#[test]
fn retimed_audio_plays_its_source_range_at_the_clip_speed() {
    let dir = project_dir();
    write_sweep(&dir.path().join("media").join("sweep.wav"));
    let mut project = retimed_tone_project(1.5, 1.0, 2.0, json!({}));
    project.media.push(MediaAsset {
        id: "sweep".to_string(),
        name: Some("sweep".to_string()),
        relative_path: "media/sweep.wav".to_string(),
        kind: MediaKind::Audio,
        duration_seconds: 4.0,
        width: None,
        height: None,
        fps: None,
        folder_id: None,
    });
    project.timeline.tracks[AUDIO_TRACK_INDEX].items[0].source = TimelineSource::Media {
        media_id: "sweep".to_string(),
    };
    let samples = render_audio(dir.path(), &project, "audio-speed-sweep");
    // Output t reads source 1 + 2t, where the sweep is at 200 + 450 * source Hz.
    for (at, expected) in [(0.25, 875.0), (1.0, 1550.0)] {
        let measured = dominant_frequency(&samples, at - 0.05, at + 0.05);
        eprintln!("retimed sweep at {at} s: {measured:.0} Hz (source {expected} Hz)");
        assert!(
            (measured - expected).abs() <= expected * 0.05,
            "at {at} s the clip plays source {} s: expected {expected} Hz, measured {measured}",
            1.0 + 2.0 * at
        );
    }
}
