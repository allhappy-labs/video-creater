//! Reverse feasibility spike (plan 04 Task 12, design decision 14).
//!
//! Measures two ways of producing reversed clip intermediates on the curated
//! runtime and prints the numbers recorded in
//! `docs/research/2026-09-16-reverse-intermediate-spike.md`:
//!
//! - Strategy A: negative-rate seeks on the curated decoders.
//! - Strategy B: forward decode, then reversed reassembly (PNG-MOV or ProRes
//!   for video, reversed samples in a WAV for audio).
//!
//! Needs `VIDEO_CREATER_RENDER_RUNTIME_ROOT` on Linux development machines.
#![cfg(feature = "ges-render")]

#[path = "render_transitions_ges/fixtures.rs"]
#[allow(dead_code)]
mod fixtures;
#[path = "render_transitions_ges/media.rs"]
#[allow(dead_code)]
mod media;
#[path = "render_transitions_ges/parity.rs"]
#[allow(dead_code)]
mod parity;
#[path = "reverse_intermediate_spike/support.rs"]
mod support;

use fixtures::{add_transition, base_project, plan, render, start_runtime, AUDIO_TRACK_INDEX};
use gst::prelude::*;
use gstreamer as gst;
use media::{decode_audio, FPS, HEIGHT, SAMPLE_RATE, WIDTH};
use serde_json::json;
use std::collections::BTreeMap;
use std::time::{Duration, Instant};
use support::*;
use video_creater_lib::precompose::{
    decode_video_frames_rgba, package_png_frames_as_mov, FrameDecodeSpec, PngSequenceSpec,
};
use video_creater_lib::project::model::*;
use video_creater_lib::render_pipeline::plugin_policy::{
    evaluate_gstreamer_factory, GstFactoryInfo, PluginPolicyVerdict,
};

#[test]
fn spike_a_negative_rate_seeks() {
    let dir = tempfile::tempdir().expect("spike dir");
    let (webm, mp4, sweep) = small_media(dir.path());
    let stop = gst::ClockTime::from_nseconds(FRAMES as u64 * 1_000_000_000 / 24);
    let expected = colours(true);
    let video_sink = format!(
        "videoconvert ! video/x-raw,format=RGBA,width={WIDTH},height={HEIGHT} ! appsink name=sink"
    );
    for (decoder, description) in [
        (
            "vp8dec",
            format!(
                "filesrc location={} ! matroskademux ! vp8dec ! {video_sink}",
                quoted(&webm)
            ),
        ),
        (
            "avdec_h264",
            format!(
                "filesrc location={} ! qtdemux ! h264parse ! avdec_h264 ! {video_sink}",
                quoted(&mp4)
            ),
        ),
        (
            "openh264dec",
            format!(
                "filesrc location={} ! qtdemux ! h264parse ! openh264dec ! {video_sink}",
                quoted(&mp4)
            ),
        ),
    ] {
        let delivered = negative_rate_playback(&description, stop);
        // Frame k of a correct reversal shows the colour of source frame FRAMES-1-k.
        let reversed_matches = delivered
            .payloads
            .iter()
            .enumerate()
            .filter(|(index, frame)| {
                let source = (FRAMES as usize).saturating_sub(1 + index);
                close(
                    centre(frame, WIDTH, HEIGHT),
                    rgb(expected[source / SEGMENT_FRAMES as usize]),
                    16,
                )
            })
            .count();
        eprintln!(
            "SPIKE A video {decoder}: frames {}/{FRAMES}, strictly descending {}, duplicate pts {}, reversed colour matches {}/{FRAMES}, eos {}, wall {:.3}s, first pts {:?}, last pts {:?}",
            delivered.pts.len(),
            strictly_descending(&delivered.pts),
            duplicates(&delivered.pts),
            reversed_matches,
            delivered.eos,
            delivered.wall.as_secs_f64(),
            delivered.pts.first(),
            delivered.pts.last(),
        );
    }
    let audio_sink = "audioconvert ! audioresample ! audio/x-raw,format=F32LE,channels=1,rate=48000 ! appsink name=sink";
    let delivered = negative_rate_playback(
        &format!(
            "filesrc location={} ! matroskademux ! opusdec ! {audio_sink}",
            quoted(&sweep)
        ),
        gst::ClockTime::from_seconds(4),
    );
    report_reverse_audio("opusdec", &delivered, 4.0);
}

/// Audio buffers of a reverse playback arrive in descending order with forward
/// content; appsink does not reverse inside buffers, so the harness does.
#[test]
fn spike_b_forward_decode_and_reversed_reassembly() {
    let dir = tempfile::tempdir().expect("spike dir");
    let (webm, mp4, sweep) = small_media(dir.path());
    for (label, source) in [("vp8-webm", &webm), ("h264-mp4", &mp4)] {
        let (mov, wall) = reverse_to_png_mov(source, dir.path(), label);
        let exact = verify_reversed_video(label, &mov);
        eprintln!(
            "SPIKE B video {label}: exact reversal {exact}, wall {:.3}s",
            wall.as_secs_f64()
        );
    }
    let started = Instant::now();
    let mut samples = decode_audio_forward(&sweep, "matroskademux ! opusdec");
    samples.reverse();
    let reversed = dir.path().join("sweep-reversed.wav");
    write_wav(&reversed, &samples, 1);
    let check = decode_audio_forward(&reversed, "wavparse");
    let seconds = check.len() as f64 / SAMPLE_RATE as f64;
    eprintln!(
        "SPIKE B audio opus sweep: seconds {seconds:.3}, head {:.0} Hz, tail {:.0} Hz (forward head {:.0} Hz, tail {:.0} Hz), wall {:.3}s",
        zero_crossing_hz(&check, 0.05, 0.3),
        zero_crossing_hz(&check, seconds - 0.3, seconds - 0.05),
        zero_crossing_hz(&decode_audio_forward(&sweep, "matroskademux ! opusdec"), 0.05, 0.3),
        zero_crossing_hz(&decode_audio_forward(&sweep, "matroskademux ! opusdec"), seconds - 0.3, seconds - 0.05),
        started.elapsed().as_secs_f64()
    );
}

#[test]
fn spike_b_reversed_intermediate_renders_through_transitions() {
    let dir = parity::fixture_dir();
    let work = dir.path().join("media");
    let (mov, _) = reverse_to_png_mov(&work.join("warm.webm"), &work, "warm");
    assert!(verify_reversed_video("warm", &mov));
    write_sweep(&work.join("sweep.wav"));
    let mut sweep = decode_audio_forward(&work.join("sweep.wav"), "wavparse");
    sweep.reverse();
    write_wav(&work.join("sweep-reversed.wav"), &sweep, 1);

    let mut project = base_project();
    let seconds = f64::from(FRAMES) / FPS;
    let asset = |id: &str, file: &str, kind: MediaKind| {
        let visual = kind == MediaKind::Video;
        MediaAsset {
            id: id.to_string(),
            name: Some(id.to_string()),
            relative_path: format!("media/{file}"),
            kind,
            duration_seconds: if visual { seconds } else { SWEEP_SECONDS },
            width: visual.then_some(WIDTH as u32),
            height: visual.then_some(HEIGHT as u32),
            fps: visual.then_some(FPS),
            folder_id: None,
        }
    };
    project.media = vec![
        asset("cool", "cool.webm", MediaKind::Video),
        asset("reversed", "warm-reversed.mov", MediaKind::Video),
        asset("sweep", "sweep-reversed.wav", MediaKind::Audio),
    ];
    let clip =
        |id: &str, kind: TimelineItemKind, media: &str, start: f64, source_in: f64| TimelineItem {
            id: id.to_string(),
            kind,
            start_seconds: start,
            duration_seconds: 2.0,
            source: TimelineSource::Media {
                media_id: media.to_string(),
            },
            label: id.to_string(),
            properties: BTreeMap::from([
                ("sourceIn".to_string(), json!(source_in)),
                ("sourceOut".to_string(), json!(source_in + 2.0)),
            ]),
        };
    project.timeline.tracks[0].items = vec![
        clip("cool-a", TimelineItemKind::VideoClip, "cool", 0.0, 1.0),
        clip(
            "reversed",
            TimelineItemKind::VideoClip,
            "reversed",
            2.0,
            1.0,
        ),
        clip("cool-b", TimelineItemKind::VideoClip, "cool", 4.0, 1.0),
    ];
    project.timeline.duration_seconds = 6.0;
    add_transition(
        &mut project,
        "track-video",
        ("cool-a", "reversed"),
        TransitionKind::Crossfade,
        0.5,
    );
    add_transition(
        &mut project,
        "track-video",
        ("reversed", "cool-b"),
        TransitionKind::Crossfade,
        0.5,
    );
    let started = Instant::now();
    let comparison = parity::assert_render_parity(
        "reverse-spike",
        dir.path(),
        &project,
        &project,
        &[1.8, 2.0, 2.2, 3.0, 3.8, 4.0, 4.2],
    );
    let worst = comparison["comparedFrames"]
        .as_array()
        .expect("frames")
        .iter()
        .filter_map(|frame| frame["mismatchRatio"].as_f64())
        .fold(0.0_f64, f64::max);
    eprintln!(
        "SPIKE B GES parity: status {}, worst mismatch ratio {worst}, wall {:.3}s",
        comparison["status"],
        started.elapsed().as_secs_f64()
    );

    let mut audio = project.clone();
    audio.timeline.tracks[AUDIO_TRACK_INDEX].items = vec![TimelineItem {
        duration_seconds: 4.0,
        properties: BTreeMap::from([
            ("sourceIn".to_string(), json!(0.0)),
            ("sourceOut".to_string(), json!(4.0)),
        ]),
        ..clip("sweep", TimelineItemKind::AudioClip, "sweep", 1.0, 0.0)
    }];
    let output = decode_audio(&render(&plan(dir.path(), &audio, "reverse-spike-audio")));
    eprintln!(
        "SPIKE B GES reversed sweep: at 1.1-1.35 s {:.0} Hz, at 4.65-4.9 s {:.0} Hz",
        zero_crossing_hz(&output, 1.1, 1.35),
        zero_crossing_hz(&output, 4.65, 4.9)
    );
}

#[test]
#[ignore = "30 s 1080p cost measurement for the reverse spike; run with --ignored"]
fn spike_b_large_media_cost() {
    start_runtime();
    for factory in ["avenc_prores_ks", "pngenc"] {
        let found = gst::ElementFactory::find(factory);
        let decision = found.as_ref().map(|found| {
            let plugin = found.plugin();
            evaluate_gstreamer_factory(
                &GstFactoryInfo::new(factory)
                    .plugin_name(
                        found
                            .plugin_name()
                            .map(|name| name.to_string())
                            .unwrap_or_default(),
                    )
                    .package(
                        plugin
                            .as_ref()
                            .map(|plugin| plugin.package().to_string())
                            .unwrap_or_default(),
                    )
                    .license(
                        plugin
                            .as_ref()
                            .map(|plugin| plugin.license().to_string())
                            .unwrap_or_default(),
                    ),
            )
        });
        eprintln!(
            "SPIKE policy {factory}: present {}, allowed {:?}",
            found.is_some(),
            decision.map(|decision| (
                decision.verdict == PluginPolicyVerdict::Allowed,
                decision.reason
            ))
        );
    }
    let dir = tempfile::tempdir().expect("large spike dir");
    let source = dir.path().join("large.mp4");
    let started = Instant::now();
    run_to_eos(
        &format!(
            "videotestsrc pattern=smpte horizontal-speed=8 num-buffers=720 ! video/x-raw,width=1920,height=1080,framerate=24/1 \
             ! videoconvert ! video/x-raw,format=I420 ! openh264enc bitrate=8000000 ! h264parse ! mp4mux name=mux ! filesink location={} \
             audiotestsrc wave=sine freq=440 samplesperbuffer=1024 num-buffers=1407 ! audio/x-raw,rate=48000,channels=2 \
             ! audioconvert ! avenc_aac ! aacparse ! mux.",
            quoted(&source)
        ),
        600,
    );
    eprintln!(
        "SPIKE (iii) fixture encode wall {:.1}s, {} bytes",
        started.elapsed().as_secs_f64(),
        std::fs::metadata(&source).expect("large").len()
    );

    // PNG-MOV: forward decode to PNG with pngenc, reverse by renaming, package.
    let frames = dir.path().join("frames");
    std::fs::create_dir_all(&frames).expect("frames");
    let started = Instant::now();
    run_to_eos(
        &format!(
            "filesrc location={} ! qtdemux name=d d.video_0 ! queue ! h264parse ! avdec_h264 ! videoconvert ! video/x-raw,format=RGBA \
             ! pngenc ! multifilesink location={}",
            quoted(&source),
            quoted(&frames.join("forward-%06d.png"))
        ),
        900,
    );
    let decoded = std::fs::read_dir(&frames).expect("frames").count() as u32;
    let decode_wall = started.elapsed();
    for index in 0..decoded {
        std::fs::rename(
            frames.join(format!("forward-{index:06}.png")),
            frames.join(format!("frame-{:06}.png", decoded - 1 - index)),
        )
        .expect("rename");
    }
    let png_bytes: u64 = std::fs::read_dir(&frames)
        .expect("frames")
        .map(|entry| entry.expect("entry").metadata().expect("meta").len())
        .sum();
    let mov = dir.path().join("large-reversed.mov");
    package_png_frames_as_mov(
        &frames,
        decoded,
        PngSequenceSpec {
            width: 1920,
            height: 1080,
            fps_numerator: 24,
            fps_denominator: 1,
        },
        &mov,
        Duration::from_secs(900),
    )
    .unwrap_or_else(|errors| panic!("package large: {errors:?}"));
    eprintln!(
        "SPIKE (iii) PNG-MOV: frames {decoded}/720, decode+pngenc {:.1}s, total {:.1}s, png bytes {png_bytes}, mov bytes {}",
        decode_wall.as_secs_f64(),
        started.elapsed().as_secs_f64(),
        std::fs::metadata(&mov).expect("mov").len()
    );

    // ProRes: the reversed PNGs through pngdec into avenc_prores_ks.
    let prores = dir.path().join("large-reversed-prores.mov");
    let started = Instant::now();
    run_to_eos(
        &format!(
            "multifilesrc location={} index=0 stop-index={} caps=image/png,framerate=24/1 ! pngdec ! videoconvert \
             ! avenc_prores_ks ! qtmux ! filesink location={}",
            quoted(&frames.join("frame-%06d.png")),
            decoded - 1,
            quoted(&prores)
        ),
        900,
    );
    eprintln!(
        "SPIKE (iii) ProRes from reversed PNGs: encode {:.1}s (plus the decode+pngenc above), bytes {}",
        started.elapsed().as_secs_f64(),
        std::fs::metadata(&prores).expect("prores").len()
    );
    std::fs::remove_dir_all(&frames).expect("remove frames");

    let started = Instant::now();
    let mut samples = decode_audio_forward(
        &source,
        "qtdemux name=d d.audio_0 ! queue ! aacparse ! avdec_aac",
    );
    samples.reverse();
    write_wav(&dir.path().join("large-reversed.wav"), &samples, 1);
    eprintln!(
        "SPIKE (iii) audio: {} samples reversed, wall {:.1}s",
        samples.len(),
        started.elapsed().as_secs_f64()
    );

    // Worst-case size: two seconds of full-frame noise.
    run_to_eos(
        &format!(
            "videotestsrc pattern=snow num-buffers=48 ! video/x-raw,width=1920,height=1080,framerate=24/1 ! videoconvert \
             ! video/x-raw,format=RGBA ! pngenc ! multifilesink location={}",
            quoted(&dir.path().join("snow-%06d.png"))
        ),
        300,
    );
    let snow: u64 = (0..48)
        .map(|index| {
            std::fs::metadata(dir.path().join(format!("snow-{index:06}.png")))
                .expect("snow")
                .len()
        })
        .sum();
    eprintln!(
        "SPIKE (iii) worst-case noise PNG: {} bytes per frame, {} bytes for 720 frames",
        snow / 48,
        snow / 48 * 720
    );
}

#[test]
#[ignore = "30 s 1080p cost measurement for the reverse spike; run with --ignored"]
fn spike_b_reviewed_decode_path_cost() {
    start_runtime();
    let dir = tempfile::tempdir().expect("decode spike dir");
    let source = dir.path().join("large.mp4");
    run_to_eos(
        &format!(
            "videotestsrc pattern=smpte horizontal-speed=8 num-buffers=720 ! video/x-raw,width=1920,height=1080,framerate=24/1 \
             ! videoconvert ! video/x-raw,format=I420 ! openh264enc bitrate=8000000 ! h264parse ! mp4mux ! filesink location={}",
            quoted(&source)
        ),
        600,
    );
    let spec = FrameDecodeSpec {
        width: 1920,
        height: 1080,
        fps_numerator: 24,
        fps_denominator: 1,
        source_start_micros: 0,
        source_stop_micros: 30_000_000,
        playback_rate_micros: 1_000_000,
        frame_count: 720,
    };
    let started = Instant::now();
    let mut received = 0_u32;
    let mut encode = Duration::ZERO;
    decode_video_frames_rgba(&source, spec, Duration::from_secs(900), |index, rgba| {
        received += 1;
        if index < 24 {
            let encode_started = Instant::now();
            image::RgbaImage::from_raw(1920, 1080, rgba.to_vec())
                .expect("frame size")
                .save(dir.path().join(format!("frame-{index:06}.png")))
                .expect("write png");
            encode += encode_started.elapsed();
        }
        Ok(())
    })
    .unwrap_or_else(|errors| panic!("reviewed decode: {errors:?}"));
    eprintln!(
        "SPIKE (iii) reviewed decode_video_frames_rgba: frames {received}/720, wall {:.1}s including {:.1}s of image-crate PNG encoding for 24 frames ({:.0} ms/frame, debug build)",
        started.elapsed().as_secs_f64(),
        encode.as_secs_f64(),
        encode.as_secs_f64() * 1000.0 / 24.0
    );
}
