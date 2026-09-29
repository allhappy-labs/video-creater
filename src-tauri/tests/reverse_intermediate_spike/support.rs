//! Media, decoding and reversal helpers for the reverse feasibility spike.

use super::fixtures::start_runtime;
use super::media::{write_segmented_video, FPS, HEIGHT, SAMPLE_RATE, WIDTH};
use gst::prelude::*;
use gstreamer as gst;
use gstreamer_app as gst_app;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};
use video_creater_lib::precompose::{
    decode_video_frames_rgba, package_png_frames_as_mov, FrameDecodeSpec, PngSequenceSpec,
};

pub(crate) const SEGMENTS: u32 = 14;
pub(crate) const SEGMENT_FRAMES: u32 = 7;
pub(crate) const FRAMES: u32 = SEGMENTS * SEGMENT_FRAMES;
pub(crate) const SWEEP_SECONDS: f64 = 4.0;

pub(crate) fn colours(warm: bool) -> Vec<u32> {
    (0..SEGMENTS)
        .map(|index| {
            let ramp = 16 * index;
            if warm {
                0xff00_0000 | 0xf0 << 16 | ramp << 8 | 0x20
            } else {
                0xff00_0000 | 0x20 << 16 | ramp << 8 | 0xf0
            }
        })
        .collect()
}

pub(crate) fn rgb(argb: u32) -> [u8; 3] {
    [(argb >> 16) as u8, (argb >> 8) as u8, argb as u8]
}

pub(crate) fn centre(frame: &[u8], width: usize, height: usize) -> [u8; 3] {
    let offset = (height / 2 * width + width / 2) * 4;
    [frame[offset], frame[offset + 1], frame[offset + 2]]
}

pub(crate) fn close(left: [u8; 3], right: [u8; 3], tolerance: u8) -> bool {
    left.iter()
        .zip(right)
        .all(|(a, b)| a.abs_diff(b) <= tolerance)
}

pub(crate) fn run_to_eos(description: &str, timeout: u64) {
    let pipeline = gst::parse::launch(description)
        .unwrap_or_else(|error| panic!("parse `{description}`: {error}"))
        .downcast::<gst::Pipeline>()
        .expect("pipeline");
    pipeline.set_state(gst::State::Playing).expect("play");
    let message = pipeline
        .bus()
        .expect("bus")
        .timed_pop_filtered(
            gst::ClockTime::from_seconds(timeout),
            &[gst::MessageType::Eos, gst::MessageType::Error],
        )
        .expect("pipeline finishes");
    pipeline.set_state(gst::State::Null).expect("stop");
    if let gst::MessageView::Error(error) = message.view() {
        panic!(
            "`{description}` failed: {} {:?}",
            error.error(),
            error.debug()
        );
    }
}

pub(crate) fn quoted(path: &Path) -> String {
    format!("\"{}\"", path.display())
}

/// (i) VP8 WebM and (ii) H.264 MP4 of the warm segments, plus (iv) the sweep.
pub(crate) fn small_media(dir: &Path) -> (PathBuf, PathBuf, PathBuf) {
    start_runtime();
    let webm = dir.join("segments.webm");
    write_segmented_video(&webm, &colours(true), SEGMENT_FRAMES);
    let mp4 = dir.join("segments.mp4");
    let branches = colours(true)
        .iter()
        .map(|argb| {
            format!(
                "videotestsrc pattern=solid-color foreground-color={argb} num-buffers={SEGMENT_FRAMES} \
                 ! video/x-raw,width={WIDTH},height={HEIGHT},framerate=24/1 ! queue ! concat. "
            )
        })
        .collect::<String>();
    run_to_eos(
        &format!(
            "concat name=concat ! videoconvert ! video/x-raw,format=I420 ! openh264enc bitrate=2000000 \
             ! h264parse ! mp4mux ! filesink location={} {branches}",
            quoted(&mp4)
        ),
        60,
    );
    let sweep = dir.join("sweep.webm");
    write_sweep(&dir.join("sweep.wav"));
    run_to_eos(
        &format!(
            "filesrc location={} ! wavparse ! audioconvert ! audioresample ! opusenc ! webmmux ! filesink location={}",
            quoted(&dir.join("sweep.wav")),
            quoted(&sweep)
        ),
        60,
    );
    (webm, mp4, sweep)
}

/// A linear chirp from 200 Hz to 2000 Hz over four seconds at half amplitude.
pub(crate) fn write_sweep(path: &Path) {
    let spec = hound::WavSpec {
        channels: 1,
        sample_rate: SAMPLE_RATE as u32,
        bits_per_sample: 16,
        sample_format: hound::SampleFormat::Int,
    };
    let mut writer = hound::WavWriter::create(path, spec).expect("sweep wav");
    let total = (SWEEP_SECONDS * SAMPLE_RATE as f64) as usize;
    let rate = 1800.0 / SWEEP_SECONDS;
    for index in 0..total {
        let t = index as f64 / SAMPLE_RATE as f64;
        let phase = std::f64::consts::TAU * (200.0 * t + rate * t * t / 2.0);
        writer
            .write_sample((phase.sin() * 0.5 * f64::from(i16::MAX)) as i16)
            .expect("sweep sample");
    }
    writer.finalize().expect("finish sweep");
}

pub(crate) fn write_wav(path: &Path, samples: &[f32], channels: u16) {
    let spec = hound::WavSpec {
        channels,
        sample_rate: SAMPLE_RATE as u32,
        bits_per_sample: 16,
        sample_format: hound::SampleFormat::Int,
    };
    let mut writer = hound::WavWriter::create(path, spec).expect("reversed wav");
    for sample in samples {
        writer
            .write_sample((sample.clamp(-1.0, 1.0) * f32::from(i16::MAX)) as i16)
            .expect("wav sample");
    }
    writer.finalize().expect("finish wav");
}

/// Zero-crossing frequency of `samples` (mono, 48 kHz) over `[start, end)` seconds.
pub(crate) fn zero_crossing_hz(samples: &[f32], start: f64, end: f64) -> f64 {
    let from = (start * SAMPLE_RATE as f64) as usize;
    let to = ((end * SAMPLE_RATE as f64) as usize).min(samples.len());
    let crossings = samples[from..to]
        .windows(2)
        .filter(|pair| (pair[0] < 0.0) != (pair[1] < 0.0))
        .count();
    crossings as f64 / 2.0 / ((to - from) as f64 / SAMPLE_RATE as f64)
}

pub(crate) struct Delivered {
    pub(crate) pts: Vec<u64>,
    pub(crate) payloads: Vec<Vec<u8>>,
    pub(crate) wall: Duration,
    pub(crate) eos: bool,
}

/// Plays `description` (ending in `appsink name=sink`) at rate -1 over `[0, stop]`.
pub(crate) fn negative_rate_playback(description: &str, stop: gst::ClockTime) -> Delivered {
    let started = Instant::now();
    let pipeline = gst::parse::launch(description)
        .unwrap_or_else(|error| panic!("parse `{description}`: {error}"))
        .downcast::<gst::Pipeline>()
        .expect("pipeline");
    let sink = pipeline
        .by_name("sink")
        .expect("sink")
        .downcast::<gst_app::AppSink>()
        .expect("appsink");
    sink.set_property("sync", false);
    pipeline.set_state(gst::State::Paused).expect("pause");
    let (state, _, _) = pipeline.state(gst::ClockTime::from_seconds(30));
    state.expect("preroll");
    pipeline
        .seek(
            -1.0,
            gst::SeekFlags::FLUSH | gst::SeekFlags::ACCURATE,
            gst::SeekType::Set,
            gst::ClockTime::ZERO,
            gst::SeekType::Set,
            stop,
        )
        .expect("negative-rate seek");
    let _ = pipeline.state(gst::ClockTime::from_seconds(30));
    pipeline.set_state(gst::State::Playing).expect("play");
    let mut delivered = Delivered {
        pts: Vec::new(),
        payloads: Vec::new(),
        wall: Duration::ZERO,
        eos: false,
    };
    while let Some(sample) = sink.try_pull_sample(gst::ClockTime::from_seconds(10)) {
        let buffer = sample.buffer().expect("buffer");
        delivered
            .pts
            .push(buffer.pts().map(|pts| pts.nseconds()).unwrap_or(u64::MAX));
        delivered
            .payloads
            .push(buffer.map_readable().expect("map").as_slice().to_vec());
    }
    delivered.eos = sink.is_eos();
    if let Some(message) = pipeline
        .bus()
        .expect("bus")
        .pop_filtered(&[gst::MessageType::Error])
    {
        if let gst::MessageView::Error(error) = message.view() {
            eprintln!("SPIKE A error: {} {:?}", error.error(), error.debug());
        }
    }
    pipeline.set_state(gst::State::Null).expect("stop");
    delivered.wall = started.elapsed();
    delivered
}

pub(crate) fn strictly_descending(pts: &[u64]) -> bool {
    pts.windows(2).all(|pair| pair[0] > pair[1])
}

pub(crate) fn duplicates(pts: &[u64]) -> usize {
    let mut sorted = pts.to_vec();
    sorted.sort_unstable();
    sorted.windows(2).filter(|pair| pair[0] == pair[1]).count()
}

pub(crate) fn report_reverse_audio(decoder: &str, delivered: &Delivered, seconds: f64) -> Vec<f32> {
    let mut samples = Vec::new();
    for payload in &delivered.payloads {
        let mut chunk = payload
            .chunks_exact(4)
            .map(|bytes| f32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]))
            .collect::<Vec<_>>();
        chunk.reverse();
        samples.extend(chunk);
    }
    let delivered_seconds = samples.len() as f64 / SAMPLE_RATE as f64;
    let (head, tail) = if delivered_seconds > 0.6 {
        (
            zero_crossing_hz(&samples, 0.05, 0.3),
            zero_crossing_hz(&samples, delivered_seconds - 0.3, delivered_seconds - 0.05),
        )
    } else {
        (0.0, 0.0)
    };
    eprintln!(
        "SPIKE A audio {decoder}: buffers {}, strictly descending {}, duplicate pts {}, seconds {delivered_seconds:.3}/{seconds:.3}, head {head:.0} Hz, tail {tail:.0} Hz, eos {}, wall {:.3}s",
        delivered.pts.len(),
        strictly_descending(&delivered.pts),
        duplicates(&delivered.pts),
        delivered.eos,
        delivered.wall.as_secs_f64(),
    );
    samples
}

/// Strategy B video: forward RGBA decode, PNGs written in reverse order, PNG-MOV packaging.
pub(crate) fn reverse_to_png_mov(source: &Path, work: &Path, label: &str) -> (PathBuf, Duration) {
    let started = Instant::now();
    let frames = work.join(format!("{label}-frames"));
    std::fs::create_dir_all(&frames).expect("frames dir");
    let spec = FrameDecodeSpec {
        width: WIDTH as u32,
        height: HEIGHT as u32,
        fps_numerator: 24,
        fps_denominator: 1,
        source_start_micros: 0,
        source_stop_micros: u64::from(FRAMES) * 1_000_000 / 24,
        playback_rate_micros: 1_000_000,
        frame_count: FRAMES,
    };
    decode_video_frames_rgba(source, spec, Duration::from_secs(60), |index, rgba| {
        image::RgbaImage::from_raw(WIDTH as u32, HEIGHT as u32, rgba.to_vec())
            .expect("frame size")
            .save(frames.join(format!("frame-{:06}.png", FRAMES - 1 - index)))
            .expect("write png");
        Ok(())
    })
    .unwrap_or_else(|errors| panic!("{label}: forward decode {errors:?}"));
    let output = work.join(format!("{label}-reversed.mov"));
    package_png_frames_as_mov(
        &frames,
        FRAMES,
        PngSequenceSpec {
            width: WIDTH as u32,
            height: HEIGHT as u32,
            fps_numerator: 24,
            fps_denominator: 1,
        },
        &output,
        Duration::from_secs(60),
    )
    .unwrap_or_else(|errors| panic!("{label}: packaging {errors:?}"));
    (output, started.elapsed())
}

pub(crate) fn decode_mov_frames(path: &Path) -> BTreeMap<i64, Vec<u8>> {
    let pipeline = gst::parse::launch(&format!(
        "filesrc location={} ! qtdemux ! pngdec ! videoconvert ! video/x-raw,format=RGBA ! appsink name=sink sync=false",
        quoted(path)
    ))
    .expect("mov decode")
    .downcast::<gst::Pipeline>()
    .expect("pipeline");
    let sink = pipeline
        .by_name("sink")
        .expect("sink")
        .downcast::<gst_app::AppSink>()
        .expect("appsink");
    pipeline.set_state(gst::State::Playing).expect("play");
    let mut frames = BTreeMap::new();
    while let Some(sample) = sink.try_pull_sample(gst::ClockTime::from_seconds(10)) {
        let buffer = sample.buffer().expect("buffer");
        let index = (buffer.pts().expect("pts").nseconds() as f64 / 1e9 * FPS).round() as i64;
        frames.insert(
            index,
            buffer.map_readable().expect("map").as_slice().to_vec(),
        );
    }
    pipeline.set_state(gst::State::Null).expect("stop");
    frames
}

pub(crate) fn decode_audio_forward(path: &Path, demux: &str) -> Vec<f32> {
    let pipeline = gst::parse::launch(&format!(
        "filesrc location={} ! {demux} ! audioconvert ! audioresample ! audio/x-raw,format=F32LE,channels=1,rate=48000 ! appsink name=sink sync=false",
        quoted(path)
    ))
    .expect("audio decode")
    .downcast::<gst::Pipeline>()
    .expect("pipeline");
    let sink = pipeline
        .by_name("sink")
        .expect("sink")
        .downcast::<gst_app::AppSink>()
        .expect("appsink");
    pipeline.set_state(gst::State::Playing).expect("play");
    let mut samples = Vec::new();
    while let Some(sample) = sink.try_pull_sample(gst::ClockTime::from_seconds(10)) {
        let buffer = sample.buffer().expect("buffer");
        let map = buffer.map_readable().expect("map");
        samples.extend(
            map.as_slice()
                .chunks_exact(4)
                .map(|b| f32::from_le_bytes([b[0], b[1], b[2], b[3]])),
        );
    }
    pipeline.set_state(gst::State::Null).expect("stop");
    samples
}

/// Checks the reversed MOV frame by frame: frame k shows source segment of frame FRAMES-1-k.
pub(crate) fn verify_reversed_video(label: &str, mov: &Path) -> bool {
    let expected = colours(true);
    let frames = decode_mov_frames(mov);
    let mismatches = (0..FRAMES as i64)
        .filter(|index| {
            let source = (FRAMES as i64 - 1 - index) as usize / SEGMENT_FRAMES as usize;
            frames
                .get(index)
                .is_none_or(|frame| !close(centre(frame, WIDTH, HEIGHT), rgb(expected[source]), 16))
        })
        .collect::<Vec<_>>();
    eprintln!(
        "SPIKE B video {label}: decoded frames {}/{FRAMES}, extra frames {}, mismatched frames {:?}, size {} bytes",
        frames.len(),
        frames.keys().filter(|index| **index >= FRAMES as i64 || **index < 0).count(),
        mismatches,
        std::fs::metadata(mov).map(|meta| meta.len()).unwrap_or(0)
    );
    frames.len() == FRAMES as usize && mismatches.is_empty()
}
