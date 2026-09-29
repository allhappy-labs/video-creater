//! The GStreamer pipeline behind retimed audio intermediates:
//! `filesrc ! decodebin ! audioconvert ! audioresample ! scaletempo !
//! audioconvert ! audioresample ! S16LE 48 kHz ! appsink`, sought over the
//! source range at the clip speed. `scaletempo` turns the seek rate into a
//! tempo change at the original pitch; the samples are written as a WAV of
//! exactly the planned length. `scaletempo` holds back its last stride, so the
//! seek reads a little past the range and the output is cut to length; at the
//! very end of the media a short remainder is padded with silence.
//!
//! Only audio is decoded: a video stream in the source is demuxed and dropped.
//!
//! At speed 1 the tempo stage is left out, so the range is decoded unchanged
//! (reversed audio decodes its source span this way before reversing it).

use crate::render_pipeline::gstreamer_backend::require_allowed_factories;
use crate::render_runtime::render_runtime_environment;
use gstreamer as gst;
use gstreamer::prelude::*;
use gstreamer_app as gst_app;
use hound::{SampleFormat, WavSpec, WavWriter};
use std::path::Path;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

const OUTPUT_SAMPLE_RATE: i32 = 48_000;
const TIMEOUT: Duration = Duration::from_secs(30 * 60);
const PULL_INTERVAL: gst::ClockTime = gst::ClockTime::from_nseconds(100_000_000);
/// Output seconds read past the range so the tempo stage flushes the range end.
const TAIL_PAD_OUTPUT_SECONDS: f64 = 0.25;
/// The longest output shortfall padded with silence at the end of the media.
const MAX_SILENT_PAD_SECONDS: f64 = 0.15;

/// The source seconds to play and the playback speed.
#[derive(Debug, Clone, Copy)]
pub(super) struct RetimeRange {
    pub(super) start: f64,
    pub(super) stop: f64,
    pub(super) speed: f64,
    /// The source media duration, or 0 when unknown.
    pub(super) media_duration: f64,
}

/// Writes `range` of `source`'s first audio stream at `range.speed`, pitch
/// preserved, to `destination` at 48 kHz. Returns `(sample rate, channels, frames)`.
pub(super) fn retime_to_wav(
    source: &Path,
    destination: &Path,
    range: RetimeRange,
    is_cancelled: &dyn Fn() -> bool,
) -> Result<(u32, u16, u64), String> {
    let mut factories = vec![
        "filesrc",
        "decodebin",
        "audioconvert",
        "audioresample",
        "capsfilter",
        "appsink",
    ];
    if retimes(range) {
        factories.push("scaletempo");
    }
    require_allowed_factories(&factories).map_err(|errors| {
        errors
            .first()
            .map(|error| error.message.clone())
            .unwrap_or_else(|| {
                "The retime pipeline needs unreviewed GStreamer elements.".to_string()
            })
    })?;
    render_runtime_environment().map_err(|error| {
        format!(
            "Retimed audio requires the render runtime: {}",
            error.message
        )
    })?;
    let pipeline = gst::Pipeline::with_name("video-creater-audio-retime");
    let result = run(&pipeline, source, destination, range, is_cancelled);
    let _ = pipeline.set_state(gst::State::Null);
    if result.is_err() {
        let _ = std::fs::remove_file(destination);
    }
    result
}

fn element(factory: &str) -> Result<gst::Element, String> {
    gst::ElementFactory::make(factory)
        .build()
        .map_err(|error| format!("GStreamer element {factory} is unavailable: {error}"))
}

fn run(
    pipeline: &gst::Pipeline,
    source: &Path,
    destination: &Path,
    range: RetimeRange,
    is_cancelled: &dyn Fn() -> bool,
) -> Result<(u32, u16, u64), String> {
    let file_source = gst::ElementFactory::make("filesrc")
        .property("location", source.to_string_lossy().as_ref())
        .build()
        .map_err(|error| format!("GStreamer element filesrc is unavailable: {error}"))?;
    // With `expose-all-streams` off, decodebin skips decoders whose output
    // can't reach `caps`, so the video stream of a clip is never decoded.
    let decoder = gst::ElementFactory::make("decodebin")
        .property("caps", gst::Caps::builder("audio/x-raw").build())
        .property("expose-all-streams", false)
        .build()
        .map_err(|error| format!("GStreamer element decodebin is unavailable: {error}"))?;
    let convert = element("audioconvert")?;
    let resample = element("audioresample")?;
    let tempo = retimes(range).then(|| element("scaletempo")).transpose()?;
    let output_convert = element("audioconvert")?;
    let output_resample = element("audioresample")?;
    let caps = gst::Caps::builder("audio/x-raw")
        .field("format", "S16LE")
        .field("layout", "interleaved")
        .field("rate", OUTPUT_SAMPLE_RATE)
        .build();
    let caps_filter = gst::ElementFactory::make("capsfilter")
        .property("caps", &caps)
        .build()
        .map_err(|error| format!("GStreamer element capsfilter is unavailable: {error}"))?;
    let sink = gst_app::AppSink::builder().sync(false).build();
    let chain = [&convert, &resample]
        .into_iter()
        .chain(tempo.as_ref())
        .chain([
            &output_convert,
            &output_resample,
            &caps_filter,
            sink.upcast_ref(),
        ])
        .collect::<Vec<_>>();
    pipeline
        .add_many(
            [&file_source, &decoder]
                .into_iter()
                .chain(chain.iter().copied()),
        )
        .map_err(|error| format!("Retime pipeline assembly failed: {error}"))?;
    file_source
        .link(&decoder)
        .map_err(|error| format!("Retime source link failed: {error}"))?;
    gst::Element::link_many(chain)
        .map_err(|error| format!("Retime conversion link failed: {error}"))?;

    let audio_sink_pad = convert
        .static_pad("sink")
        .ok_or("audioconvert has no sink pad")?;
    let link_error = Arc::new(Mutex::new(None::<String>));
    let link_error_for_pad = Arc::clone(&link_error);
    decoder.connect_pad_added(move |_, pad| {
        let is_audio = pad
            .current_caps()
            .and_then(|caps| {
                caps.structure(0)
                    .map(|structure| structure.name().starts_with("audio/x-raw"))
            })
            .unwrap_or(false);
        if !is_audio || audio_sink_pad.is_linked() {
            return;
        }
        if let Err(error) = pad.link(&audio_sink_pad) {
            *link_error_for_pad.lock().expect("retime link error lock") =
                Some(format!("Retime audio pad link failed: {error:?}"));
        }
    });

    let bus = pipeline.bus().ok_or("Retime pipeline has no bus")?;
    let started = Instant::now();
    pipeline
        .set_state(gst::State::Paused)
        .map_err(|error| format!("Retime pipeline could not start: {error}"))?;
    wait_for_preroll(pipeline, &bus, started, is_cancelled)?;
    if let Some(error) = link_error.lock().expect("retime link error lock").take() {
        return Err(error);
    }
    pipeline
        .seek(
            range.speed,
            gst::SeekFlags::FLUSH | gst::SeekFlags::ACCURATE,
            gst::SeekType::Set,
            clock_time(range.start),
            gst::SeekType::Set,
            clock_time(padded_stop(range)),
        )
        .map_err(|error| format!("Retime seek failed: {error}"))?;
    pipeline
        .set_state(gst::State::Playing)
        .map_err(|error| format!("Retime pipeline could not play: {error}"))?;

    let mut writer: Option<WavWriter<std::io::BufWriter<std::fs::File>>> = None;
    let mut format = (0_u32, 0_u16);
    let mut frames = 0_u64;
    let target_frames =
        ((range.stop - range.start) / range.speed * f64::from(OUTPUT_SAMPLE_RATE)).round() as u64;
    while frames < target_frames {
        if is_cancelled() {
            return Err("Render preparation was cancelled.".to_string());
        }
        if started.elapsed() > TIMEOUT {
            return Err("Retimed audio preparation timed out.".to_string());
        }
        check_bus(&bus)?;
        let Some(sample) = sink.try_pull_sample(PULL_INTERVAL) else {
            if sink.is_eos() {
                break;
            }
            continue;
        };
        if writer.is_none() {
            let structure = sample
                .caps()
                .and_then(|caps| caps.structure(0).map(ToOwned::to_owned))
                .ok_or("Retimed audio has no caps")?;
            let rate = structure
                .get::<i32>("rate")
                .map_err(|error| format!("Retimed audio rate: {error}"))?;
            let channels = structure
                .get::<i32>("channels")
                .map_err(|error| format!("Retimed audio channels: {error}"))?;
            format = (rate as u32, channels as u16);
            let spec = WavSpec {
                channels: format.1,
                sample_rate: format.0,
                bits_per_sample: 16,
                sample_format: SampleFormat::Int,
            };
            writer = Some(
                WavWriter::create(destination, spec)
                    .map_err(|error| format!("Retimed audio WAV could not be created: {error}"))?,
            );
        }
        let buffer = sample
            .buffer()
            .ok_or("Retimed audio sample has no buffer")?;
        let map = buffer
            .map_readable()
            .map_err(|error| format!("Retimed audio buffer could not be read: {error}"))?;
        let output = writer.as_mut().expect("writer created above");
        let channels = usize::from(format.1.max(1));
        for frame in map.as_slice().chunks_exact(2 * channels) {
            if frames >= target_frames {
                break;
            }
            for bytes in frame.chunks_exact(2) {
                output
                    .write_sample(i16::from_le_bytes([bytes[0], bytes[1]]))
                    .map_err(|error| format!("Retimed audio WAV write failed: {error}"))?;
            }
            frames += 1;
        }
    }
    check_bus(&bus)?;
    let mut writer = writer.ok_or("The source has no audio in the retimed range.")?;
    let shortfall = target_frames - frames.min(target_frames);
    if shortfall as f64 > MAX_SILENT_PAD_SECONDS * f64::from(OUTPUT_SAMPLE_RATE) {
        return Err(format!(
            "Retimed audio is {:.3} s long; {:.3} s were expected.",
            frames as f64 / f64::from(OUTPUT_SAMPLE_RATE),
            target_frames as f64 / f64::from(OUTPUT_SAMPLE_RATE)
        ));
    }
    for _ in 0..shortfall * u64::from(format.1.max(1)) {
        writer
            .write_sample(0_i16)
            .map_err(|error| format!("Retimed audio WAV write failed: {error}"))?;
    }
    frames += shortfall;
    writer
        .finalize()
        .map_err(|error| format!("Retimed audio WAV could not be finished: {error}"))?;
    Ok((format.0, format.1, frames))
}

fn wait_for_preroll(
    pipeline: &gst::Pipeline,
    bus: &gst::Bus,
    started: Instant,
    is_cancelled: &dyn Fn() -> bool,
) -> Result<(), String> {
    loop {
        check_bus(bus)?;
        if is_cancelled() {
            return Err("Render preparation was cancelled.".to_string());
        }
        let (result, current, _) = pipeline.state(PULL_INTERVAL);
        result.map_err(|_| "Retime pipeline could not preroll the source audio.".to_string())?;
        if current == gst::State::Paused {
            return Ok(());
        }
        if started.elapsed() > TIMEOUT {
            return Err(
                "Retimed audio preparation timed out while opening the source.".to_string(),
            );
        }
    }
}

fn check_bus(bus: &gst::Bus) -> Result<(), String> {
    if let Some(message) = bus.pop_filtered(&[gst::MessageType::Error]) {
        if let gst::MessageView::Error(error) = message.view() {
            return Err(format!("Retimed audio decoding failed: {}", error.error()));
        }
    }
    Ok(())
}

/// Whether the range plays at a speed other than 1 and needs the tempo stage.
fn retimes(range: RetimeRange) -> bool {
    (range.speed - 1.0).abs() > f64::EPSILON
}

fn padded_stop(range: RetimeRange) -> f64 {
    let padded = range.stop + TAIL_PAD_OUTPUT_SECONDS * range.speed;
    if range.media_duration > 0.0 {
        padded.min(range.media_duration).max(range.stop)
    } else {
        padded
    }
}

fn clock_time(seconds: f64) -> gst::ClockTime {
    gst::ClockTime::from_nseconds((seconds.max(0.0) * 1_000_000_000.0).round() as u64)
}

#[cfg(all(test, feature = "ges-render"))]
mod tests {
    use super::*;

    /// Writes a WebM with a VP8 video stream and an Opus tone of `seconds`.
    fn write_video_with_tone(path: &Path, seconds: u32) {
        let description = format!(
            "webmmux name=mux ! filesink location=\"{}\" \
             videotestsrc num-buffers={} ! video/x-raw,width=64,height=36,framerate=24/1 \
             ! videoconvert ! vp8enc deadline=1 ! queue ! mux. \
             audiotestsrc wave=sine freq=440 samplesperbuffer=960 num-buffers={} \
             ! audio/x-raw,rate=48000,channels=1 ! audioconvert ! opusenc ! queue ! mux.",
            path.display(),
            seconds * 24,
            seconds * 50,
        );
        let fixture = gst::parse::launch(&description)
            .expect("fixture pipeline")
            .downcast::<gst::Pipeline>()
            .expect("fixture is a pipeline");
        fixture
            .set_state(gst::State::Playing)
            .expect("fixture plays");
        let message = fixture
            .bus()
            .expect("fixture bus")
            .timed_pop_filtered(
                gst::ClockTime::from_seconds(60),
                &[gst::MessageType::Eos, gst::MessageType::Error],
            )
            .expect("fixture finishes");
        fixture.set_state(gst::State::Null).expect("fixture stops");
        assert!(
            matches!(message.view(), gst::MessageView::Eos(_)),
            "fixture failed: {message:?}"
        );
    }

    #[test]
    fn retiming_audio_never_decodes_the_video_stream() {
        crate::render_runtime::start_render_process_runtime()
            .expect("GES render runtime starts (set VIDEO_CREATER_RENDER_RUNTIME_ROOT)");
        let dir = tempfile::tempdir().expect("temporary directory");
        let source = dir.path().join("camera.webm");
        let destination = dir.path().join("retimed.wav");
        write_video_with_tone(&source, 2);

        let pipeline = gst::Pipeline::with_name("audio-retime-decoder-probe");
        let decoders = Arc::new(Mutex::new(Vec::<String>::new()));
        let seen = Arc::clone(&decoders);
        pipeline.connect_deep_element_added(move |_, _, element| {
            if let Some(factory) = element.factory() {
                let klass = factory.klass().to_string();
                if klass.contains("Decoder") {
                    seen.lock().expect("decoder list").push(klass);
                }
            }
        });
        let range = RetimeRange {
            start: 0.25,
            stop: 1.25,
            speed: 2.0,
            media_duration: 2.0,
        };
        let result = run(&pipeline, &source, &destination, range, &|| false);
        let _ = pipeline.set_state(gst::State::Null);

        let (_, _, frames) = result.expect("retime the tone");
        assert_eq!(frames, 24_000);
        let decoders = decoders.lock().expect("decoder list").clone();
        assert!(
            decoders.iter().any(|klass| klass.contains("Audio")),
            "the audio stream is decoded: {decoders:?}"
        );
        assert!(
            !decoders.iter().any(|klass| klass.contains("Video")),
            "no video decoder is plugged: {decoders:?}"
        );
    }
}
