//! Solid-colour and tone fixture media, and decoding of rendered WebM output.

use gst::prelude::*;
use gstreamer as gst;
use gstreamer_app as gst_app;
use std::collections::BTreeMap;
use std::path::Path;

pub(crate) const WIDTH: usize = 128;
pub(crate) const HEIGHT: usize = 72;
pub(crate) const FPS: f64 = 24.0;
pub(crate) const SAMPLE_RATE: usize = 48_000;

pub(crate) const RED: u32 = 0xffff_0000;
pub(crate) const BLUE: u32 = 0xff00_00ff;

fn run_to_eos(description: &str) {
    let pipeline = gst::parse::launch(description)
        .unwrap_or_else(|error| panic!("parse `{description}`: {error}"))
        .downcast::<gst::Pipeline>()
        .expect("fixture pipeline");
    pipeline
        .set_state(gst::State::Playing)
        .expect("start fixture");
    let bus = pipeline.bus().expect("fixture bus");
    let message = bus
        .timed_pop_filtered(
            gst::ClockTime::from_seconds(60),
            &[gst::MessageType::Eos, gst::MessageType::Error],
        )
        .expect("fixture pipeline finishes");
    pipeline.set_state(gst::State::Null).expect("stop fixture");
    if let gst::MessageView::Error(error) = message.view() {
        panic!("fixture `{description}` failed: {}", error.error());
    }
}

fn location(path: &Path) -> String {
    format!("\"{}\"", path.display())
}

/// Writes a VP8 WebM of `seconds` of a solid ARGB colour at the fixture size.
pub(crate) fn write_solid_video(path: &Path, argb: u32, seconds: f64) {
    let frames = (seconds * FPS).round() as u32;
    run_to_eos(&format!(
        "videotestsrc pattern=solid-color foreground-color={argb} num-buffers={frames} \
         ! video/x-raw,width={WIDTH},height={HEIGHT},framerate=24/1 ! videoconvert \
         ! vp8enc deadline=1 ! webmmux ! filesink location={}",
        location(path)
    ));
}

/// Writes a VP8 WebM of `segments` solid colours, `segment_frames` frames
/// each, so a frame's colour identifies its source time.
pub(crate) fn write_segmented_video(path: &Path, segments: &[u32], segment_frames: u32) {
    let branches = segments
        .iter()
        .map(|argb| {
            format!(
                "videotestsrc pattern=solid-color foreground-color={argb} num-buffers={segment_frames} \
                 ! video/x-raw,width={WIDTH},height={HEIGHT},framerate=24/1 ! queue ! concat. "
            )
        })
        .collect::<String>();
    run_to_eos(&format!(
        "concat name=concat ! videoconvert ! vp8enc deadline=1 ! webmmux ! filesink location={} {branches}",
        location(path)
    ));
}

/// Writes an Opus WebM of `seconds` of a mono sine tone at half amplitude.
pub(crate) fn write_tone(path: &Path, frequency: f64, seconds: f64) {
    let buffers = (seconds * SAMPLE_RATE as f64 / 960.0).round() as u32;
    run_to_eos(&format!(
        "audiotestsrc wave=sine freq={frequency} volume=0.5 samplesperbuffer=960 num-buffers={buffers} \
         ! audio/x-raw,rate=48000,channels=1 ! audioconvert ! audioresample ! opusenc \
         ! webmmux ! filesink location={}",
        location(path)
    ));
}

/// Writes a WebM of `seconds` of a solid ARGB colour (VP8) with a mono sine
/// tone at half amplitude (Opus), like a camera clip with its own sound.
pub(crate) fn write_video_with_tone(path: &Path, argb: u32, frequency: f64, seconds: f64) {
    let frames = (seconds * FPS).round() as u32;
    let buffers = (seconds * SAMPLE_RATE as f64 / 960.0).round() as u32;
    run_to_eos(&format!(
        "webmmux name=mux ! filesink location={} \
         videotestsrc pattern=solid-color foreground-color={argb} num-buffers={frames} \
         ! video/x-raw,width={WIDTH},height={HEIGHT},framerate=24/1 ! videoconvert \
         ! vp8enc deadline=1 ! queue ! mux. \
         audiotestsrc wave=sine freq={frequency} volume=0.5 samplesperbuffer=960 num-buffers={buffers} \
         ! audio/x-raw,rate=48000,channels=1 ! audioconvert ! audioresample ! opusenc \
         ! queue ! mux.",
        location(path)
    ));
}

fn pull_all(pipeline: &gst::Pipeline, sink: &gst_app::AppSink) -> Vec<gst::Sample> {
    pipeline
        .set_state(gst::State::Playing)
        .expect("start decode");
    let mut samples = Vec::new();
    while let Some(sample) = sink.try_pull_sample(gst::ClockTime::from_seconds(10)) {
        samples.push(sample);
    }
    let bus = pipeline.bus().expect("decode bus");
    let error = bus.pop_filtered(&[gst::MessageType::Error]);
    pipeline.set_state(gst::State::Null).expect("stop decode");
    if let Some(message) = error {
        if let gst::MessageView::Error(error) = message.view() {
            panic!("decode failed: {}", error.error());
        }
    }
    samples
}

fn decode_pipeline(path: &Path, branch: &str) -> (gst::Pipeline, gst_app::AppSink) {
    let pipeline = gst::parse::launch(&format!(
        "filesrc location={} ! matroskademux name=demux {branch} ! appsink name=sink sync=false",
        location(path)
    ))
    .expect("decode pipeline")
    .downcast::<gst::Pipeline>()
    .expect("decode pipeline bin");
    let sink = pipeline
        .by_name("sink")
        .expect("appsink")
        .downcast::<gst_app::AppSink>()
        .expect("appsink type");
    (pipeline, sink)
}

/// Decodes every video frame of a rendered WebM as RGBA, keyed by frame index.
pub(crate) fn decode_frames(path: &Path) -> BTreeMap<i64, Vec<u8>> {
    let (pipeline, sink) = decode_pipeline(
        path,
        &format!(
            "demux.video_0 ! queue ! vp8dec ! videoconvert ! videoscale \
             ! video/x-raw,format=RGBA,width={WIDTH},height={HEIGHT}"
        ),
    );
    pull_all(&pipeline, &sink)
        .into_iter()
        .map(|sample| {
            let buffer = sample.buffer().expect("frame buffer");
            let pts = buffer.pts().expect("frame pts");
            let index = (pts.nseconds() as f64 / 1e9 * FPS).round() as i64;
            let map = buffer.map_readable().expect("map frame");
            (index, map.as_slice().to_vec())
        })
        .collect()
}

/// Decodes the audio of a rendered WebM as 48 kHz mono samples from time zero.
pub(crate) fn decode_audio(path: &Path) -> Vec<f32> {
    let (pipeline, sink) = decode_pipeline(
        path,
        "demux.audio_0 ! queue ! opusdec ! audioconvert ! audioresample \
         ! audio/x-raw,format=F32LE,channels=1,rate=48000",
    );
    let mut samples = Vec::new();
    for sample in pull_all(&pipeline, &sink) {
        let buffer = sample.buffer().expect("audio buffer");
        let offset = buffer
            .pts()
            .map(|pts| (pts.nseconds() as f64 / 1e9 * SAMPLE_RATE as f64).round() as usize)
            .unwrap_or(samples.len());
        let map = buffer.map_readable().expect("map audio");
        let chunk = map
            .as_slice()
            .chunks_exact(4)
            .map(|bytes| f32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]));
        if samples.len() < offset {
            samples.resize(offset, 0.0);
        }
        samples.truncate(offset);
        samples.extend(chunk);
    }
    samples
}

/// Mean RGB of `frame` over the given pixel rectangle.
pub(crate) fn region_mean(
    frame: &[u8],
    xs: std::ops::Range<usize>,
    ys: std::ops::Range<usize>,
) -> [f64; 3] {
    let mut sum = [0.0; 3];
    let mut count = 0.0;
    for y in ys {
        for x in xs.clone() {
            let offset = (y * WIDTH + x) * 4;
            for (channel, total) in sum.iter_mut().enumerate() {
                *total += f64::from(frame[offset + channel]);
            }
            count += 1.0;
        }
    }
    sum.map(|total| total / count)
}

/// Mean RGB of the frame's interior, away from encoder edge artefacts.
pub(crate) fn frame_mean(frame: &[u8]) -> [f64; 3] {
    region_mean(frame, 8..WIDTH - 8, 8..HEIGHT - 8)
}

/// RMS of `samples` over `[start, end)` seconds.
pub(crate) fn rms(samples: &[f32], start: f64, end: f64) -> f64 {
    let from = (start * SAMPLE_RATE as f64) as usize;
    let to = ((end * SAMPLE_RATE as f64) as usize).min(samples.len());
    assert!(
        to > from,
        "audio window {start}..{end} is outside the render"
    );
    let energy = samples[from..to]
        .iter()
        .map(|sample| f64::from(*sample).powi(2))
        .sum::<f64>();
    (energy / (to - from) as f64).sqrt()
}

/// Dominant frequency of a steady tone in `samples` over `[start, end)`
/// seconds: zero crossings / 2 / seconds.
pub(crate) fn dominant_frequency(samples: &[f32], start: f64, end: f64) -> f64 {
    let from = (start * SAMPLE_RATE as f64) as usize;
    let to = ((end * SAMPLE_RATE as f64) as usize).min(samples.len());
    assert!(
        to > from,
        "audio window {start}..{end} is outside the render"
    );
    let crossings = samples[from..to]
        .windows(2)
        .filter(|pair| (pair[0] < 0.0) != (pair[1] < 0.0))
        .count();
    crossings as f64 / 2.0 / ((to - from) as f64 / SAMPLE_RATE as f64)
}
