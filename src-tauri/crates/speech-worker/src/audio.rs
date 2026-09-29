//! Media decoding to 16 kHz mono float PCM with the system GStreamer (dynamically linked LGPL).
//!
//! Like FluidAudio's `AudioConverter`, any container/codec the process can decode is accepted.
//! The helper inherits the app's `GST_PLUGIN_*` environment, so it only sees the plugin set the
//! app's render runtime policy allows.

use std::path::Path;
use std::sync::{Arc, Mutex};

use gstreamer as gst;
use gstreamer::prelude::*;
use gstreamer_app as gst_app;
use gstreamer_audio as gst_audio;

use crate::protocol::{HelperError, SAMPLE_RATE};

pub fn decode_mono_16k(path: &Path) -> Result<Vec<f32>, HelperError> {
    gst::init().map_err(|error| HelperError::Decode(format!("GStreamer init failed: {error}")))?;
    let decode_error = |detail: String| HelperError::Decode(detail);

    let pipeline = gst::Pipeline::new();
    let make = |factory: &str| {
        gst::ElementFactory::make(factory).build().map_err(|error| {
            decode_error(format!(
                "GStreamer element {factory} is unavailable: {error}"
            ))
        })
    };
    let source = make("filesrc")?;
    source.set_property("location", path.to_string_lossy().to_string());
    let decoder = make("decodebin")?;
    // Stop autoplugging at raw audio; video streams stay encoded and are left unlinked.
    decoder.set_property("caps", gst::Caps::builder("audio/x-raw").build());
    let convert = make("audioconvert")?;
    let resample = make("audioresample")?;
    let caps = gst_audio::AudioCapsBuilder::new_interleaved()
        .format(gst_audio::AudioFormat::F32le)
        .rate(SAMPLE_RATE as i32)
        .channels(1)
        .build();
    let sink = gst_app::AppSink::builder().caps(&caps).sync(false).build();

    pipeline
        .add_many([&source, &decoder, &convert, &resample, sink.upcast_ref()])
        .map_err(|error| decode_error(error.to_string()))?;
    source
        .link(&decoder)
        .map_err(|error| decode_error(error.to_string()))?;
    gst::Element::link_many([&convert, &resample, sink.upcast_ref()])
        .map_err(|error| decode_error(error.to_string()))?;

    let convert_weak = convert.downgrade();
    decoder.connect_pad_added(move |_, pad| {
        let Some(convert) = convert_weak.upgrade() else {
            return;
        };
        let is_audio = pad
            .current_caps()
            .and_then(|caps| {
                caps.structure(0)
                    .map(|structure| structure.name().starts_with("audio/x-raw"))
            })
            .unwrap_or(false);
        let Some(sink_pad) = convert.static_pad("sink") else {
            return;
        };
        if is_audio && !sink_pad.is_linked() {
            let _ = pad.link(&sink_pad);
        }
    });

    let samples = Arc::new(Mutex::new(Vec::<f32>::new()));
    let collected = samples.clone();
    sink.set_callbacks(
        gst_app::AppSinkCallbacks::builder()
            .new_sample(move |sink| {
                let sample = sink.pull_sample().map_err(|_| gst::FlowError::Eos)?;
                let buffer = sample.buffer().ok_or(gst::FlowError::Error)?;
                let map = buffer.map_readable().map_err(|_| gst::FlowError::Error)?;
                let bytes = map.as_slice();
                let mut samples = collected.lock().map_err(|_| gst::FlowError::Error)?;
                samples.extend(
                    bytes
                        .chunks_exact(4)
                        .map(|chunk| f32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]])),
                );
                Ok(gst::FlowSuccess::Ok)
            })
            .build(),
    );

    pipeline.set_state(gst::State::Playing).map_err(|error| {
        decode_error(format!(
            "could not start decoding {}: {error}",
            path.display()
        ))
    })?;
    let bus = pipeline
        .bus()
        .ok_or_else(|| decode_error("pipeline has no bus".to_string()))?;
    let mut failure = None;
    for message in bus.iter_timed(gst::ClockTime::NONE) {
        match message.view() {
            gst::MessageView::Eos(..) => break,
            gst::MessageView::Error(error) => {
                failure = Some(format!(
                    "{} ({})",
                    error.error(),
                    error
                        .debug()
                        .map(|debug| debug.to_string())
                        .unwrap_or_default()
                ));
                break;
            }
            _ => {}
        }
    }
    let _ = pipeline.set_state(gst::State::Null);
    if let Some(failure) = failure {
        return Err(decode_error(format!("{}: {failure}", path.display())));
    }
    let samples = std::mem::take(
        &mut *samples
            .lock()
            .map_err(|_| decode_error("sample buffer lock poisoned".to_string()))?,
    );
    if samples.is_empty() {
        return Err(decode_error(format!(
            "{} contains no decodable audio",
            path.display()
        )));
    }
    Ok(samples)
}
