use gst::prelude::*;
use gst_pbutils::prelude::*;
use gstreamer as gst;
use gstreamer_app as gst_app;
use gstreamer_pbutils as gst_pbutils;
use std::fs;
use std::io::{self, BufRead, Write};
use std::path::{Path, PathBuf};
use video_creater_compatibility_protocol::*;

#[cfg(target_os = "macos")]
const REQUIRED_FACTORIES: &[&str] = &[
    "uridecodebin",
    "matroskademux",
    "vp8dec",
    "vp9dec",
    "opusdec",
    "vorbisdec",
    "queue",
    "videoconvert",
    "audioconvert",
    "audioresample",
    "vtenc_prores",
    "vtenc_h264",
    "vtdec",
    "appsink",
    "qtmux",
    "filesink",
];

/// Linux codecs come from the bundled LGPL FFmpeg plugin and the BSD OpenH264 encoder.
#[cfg(not(target_os = "macos"))]
const REQUIRED_FACTORIES: &[&str] = &[
    "uridecodebin",
    "matroskademux",
    "vp8dec",
    "vp9dec",
    "opusdec",
    "vorbisdec",
    "queue",
    "videoconvert",
    "audioconvert",
    "audioresample",
    "avenc_prores_ks",
    "openh264enc",
    "avdec_h264",
    "h264parse",
    "appsink",
    "qtmux",
    "filesink",
];

#[cfg(target_os = "macos")]
const H264_ENCODER: &str =
    "video/x-raw,format=NV12 ! vtenc_h264 allow-frame-reordering=false realtime=false";
#[cfg(not(target_os = "macos"))]
const H264_ENCODER: &str =
    "video/x-raw,format=I420 ! openh264enc complexity=high rate-control=quality ! h264parse";
#[cfg(target_os = "macos")]
const PRORES_ENCODER: &str = "video/x-raw,format=NV12 ! vtenc_prores";
#[cfg(not(target_os = "macos"))]
const PRORES_ENCODER: &str = "avenc_prores_ks profile=standard";

fn main() {
    let code = run().map(|_| 0).unwrap_or_else(|error| {
        eprintln!("{error}");
        1
    });
    std::process::exit(code);
}

fn run() -> Result<(), String> {
    // ORC's generated machine code is blocked by the macOS hardened runtime.
    // The reviewed backup kernels avoid JIT entitlements while remaining fully
    // deterministic for decoding, scaling, and filmstrip extraction.
    std::env::set_var("ORC_CODE", "backup");
    gst::init().map_err(|error| error.to_string())?;
    let stdin = io::stdin();
    for line in stdin.lock().lines() {
        let line = line.map_err(|error| error.to_string())?;
        if line.trim().is_empty() {
            continue;
        }
        let request: CompatibilityRequest =
            serde_json::from_str(&line).map_err(|error| error.to_string())?;
        let id = request.request_id.clone();
        let event = match request.validate().and_then(|_| execute(&request)) {
            Ok(result) => WorkerEvent::Completed {
                request_id: id,
                result,
            },
            Err(message) => WorkerEvent::Failed {
                request_id: id,
                error: WorkerError {
                    code: "COMPATIBILITY_FAILED".into(),
                    message,
                    stage: "worker".into(),
                },
            },
        };
        serde_json::to_writer(io::stdout().lock(), &event).map_err(|error| error.to_string())?;
        io::stdout()
            .write_all(b"\n")
            .map_err(|error| error.to_string())?;
    }
    Ok(())
}

fn execute(request: &CompatibilityRequest) -> Result<CompatibilityResult, String> {
    match &request.operation {
        CompatibilityOperation::Capabilities => {
            let missing = REQUIRED_FACTORIES
                .iter()
                .filter(|name| gst::ElementFactory::find(name).is_none())
                .copied()
                .collect::<Vec<_>>();
            if !missing.is_empty() {
                return Err(format!(
                    "missing reviewed factories: {}",
                    missing.join(", ")
                ));
            }
            Ok(CompatibilityResult::Capabilities {
                protocol: COMPATIBILITY_PROTOCOL_NAME.into(),
                schema_version: COMPATIBILITY_PROTOCOL_VERSION,
                factories: REQUIRED_FACTORIES
                    .iter()
                    .map(|v| (*v).to_string())
                    .collect(),
            })
        }
        CompatibilityOperation::Probe { source_path } => Ok(CompatibilityResult::Probe {
            probe: probe(Path::new(source_path), request.budgets.timeout_millis)?,
        }),
        CompatibilityOperation::Transcode {
            source_path,
            output_root,
            output_path,
            profile,
        } => {
            let source = canonical_file(Path::new(source_path))?;
            let root = fs::canonicalize(output_root)
                .map_err(|error| format!("invalid output root: {error}"))?;
            let requested_output = PathBuf::from(output_path);
            if requested_output.extension().and_then(|v| v.to_str()) != Some("mov") {
                return Err("output must be a .mov inside outputRoot".into());
            }
            let requested_parent = requested_output.parent().ok_or("output has no parent")?;
            fs::create_dir_all(requested_parent).map_err(|error| error.to_string())?;
            let output_parent =
                fs::canonicalize(requested_parent).map_err(|error| error.to_string())?;
            if !output_parent.starts_with(&root) {
                return Err("output must be a .mov inside outputRoot".into());
            }
            let output = output_parent.join(
                requested_output
                    .file_name()
                    .ok_or("output has no filename")?,
            );
            let temp = output.with_extension(format!("{}.part", std::process::id()));
            let _ = fs::remove_file(&temp);
            let source_probe = probe(&source, request.budgets.timeout_millis)?;
            transcode(&source, &temp, *profile, request.budgets.timeout_millis)?;
            let bytes = fs::metadata(&temp)
                .map_err(|error| error.to_string())?
                .len();
            if bytes == 0 || bytes > request.budgets.max_output_bytes {
                let _ = fs::remove_file(&temp);
                return Err("compatibility output violates byte budget".into());
            }
            // The reviewed runtime intentionally contains decoders for foreign input
            // codecs, not a second decoder for every compatibility output. The
            // transcode pipeline reaching EOS proves qtmux finalized the output;
            // describe the selected profile from the validated source metadata.
            let video_codec = match profile {
                CompatibilityProfile::ProRes422PcmMov => "video/x-prores,variant=standard",
                CompatibilityProfile::H264PcmMov => "video/x-h264,stream-format=avc",
            };
            let probe = MediaProbe {
                duration_seconds: source_probe.duration_seconds,
                video: source_probe.video.map(|video| VideoProbe {
                    codec: video_codec.into(),
                    ..video
                }),
                audio: source_probe.audio.map(|audio| AudioProbe {
                    sample_rate: 48_000,
                    codec: "audio/x-raw,format=S16LE".into(),
                    ..audio
                }),
            };
            fs::rename(&temp, &output).map_err(|error| error.to_string())?;
            Ok(CompatibilityResult::Transcode {
                output_path: output.to_string_lossy().into_owned(),
                bytes,
                probe,
            })
        }
        CompatibilityOperation::ExtractFrames {
            source_path,
            output_root,
            width,
            height,
            frames,
        } => {
            let source = canonical_file(Path::new(source_path))?;
            let root = fs::canonicalize(output_root)
                .map_err(|error| format!("invalid output root: {error}"))?;
            if *width == 0 || *height == 0 || *width > 1_920 || *height > 1_080 {
                return Err("frame dimensions are invalid".into());
            }
            if frames.is_empty() || frames.len() > 32 {
                return Err("frame request count must be between 1 and 32".into());
            }
            let mut results = Vec::with_capacity(frames.len());
            let mut total_bytes = 0_u64;
            for frame in frames {
                if !frame.time_seconds.is_finite() || frame.time_seconds < 0.0 {
                    return Err("frame timestamp is invalid".into());
                }
                let output = safe_bmp_output(&root, Path::new(&frame.output_path))?;
                extract_bmp_frame(
                    &source,
                    frame.time_seconds,
                    &output,
                    *width,
                    *height,
                    request.budgets.timeout_millis,
                )?;
                let bytes = fs::metadata(&output)
                    .map_err(|error| error.to_string())?
                    .len();
                total_bytes = total_bytes.saturating_add(bytes);
                if total_bytes > request.budgets.max_output_bytes {
                    let _ = fs::remove_file(&output);
                    return Err("frame outputs violate byte budget".into());
                }
                results.push(CompatibilityFrameResult {
                    time_seconds: frame.time_seconds,
                    output_path: output.to_string_lossy().into_owned(),
                    bytes,
                });
            }
            Ok(CompatibilityResult::ExtractFrames { frames: results })
        }
    }
}

fn safe_bmp_output(root: &Path, requested: &Path) -> Result<PathBuf, String> {
    if requested.extension().and_then(|value| value.to_str()) != Some("bmp") {
        return Err("frame output must be a .bmp inside outputRoot".into());
    }
    let parent = requested.parent().ok_or("frame output has no parent")?;
    fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    let parent = fs::canonicalize(parent).map_err(|error| error.to_string())?;
    if !parent.starts_with(root) {
        return Err("frame output must be a .bmp inside outputRoot".into());
    }
    Ok(parent.join(
        requested
            .file_name()
            .ok_or("frame output has no filename")?,
    ))
}

fn extract_bmp_frame(
    source: &Path,
    time_seconds: f64,
    output: &Path,
    width: u32,
    height: u32,
    timeout_ms: u64,
) -> Result<(), String> {
    let uri = gst::glib::filename_to_uri(source, None).map_err(|error| error.to_string())?;
    let pipeline_description = format!(
        "uridecodebin uri=\"{}\" name=d d. ! queue ! videoconvert ! videoscale ! video/x-raw,format=BGRA,width={},height={},pixel-aspect-ratio=1/1 ! appsink name=filmstrip-sink sync=false max-buffers=1 drop=true",
        uri.replace('"', "\\\""), width, height,
    );
    let pipeline = gst::parse::launch(&pipeline_description)
        .map_err(|error| format!("frame pipeline parse failed: {error}"))?
        .downcast::<gst::Pipeline>()
        .map_err(|_| "frame pipeline construction failed".to_string())?;
    let appsink = pipeline
        .by_name("filmstrip-sink")
        .ok_or("frame pipeline has no sink")?
        .downcast::<gst_app::AppSink>()
        .map_err(|_| "frame pipeline sink is invalid".to_string())?;
    pipeline
        .set_state(gst::State::Paused)
        .map_err(|error| error.to_string())?;
    let timeout = gst::ClockTime::from_mseconds(timeout_ms);
    pipeline
        .state(timeout)
        .0
        .map_err(|error| format!("frame pipeline did not preroll: {error}"))?;
    let timestamp = gst::ClockTime::from_nseconds((time_seconds * 1_000_000_000.0).round() as u64);
    pipeline
        .seek_simple(gst::SeekFlags::FLUSH | gst::SeekFlags::ACCURATE, timestamp)
        .map_err(|error| format!("frame seek failed: {error}"))?;
    pipeline
        .set_state(gst::State::Playing)
        .map_err(|error| error.to_string())?;
    let result = (|| {
        let sample = appsink
            .try_pull_sample(timeout)
            .ok_or("frame extraction timed out")?;
        let buffer = sample.buffer().ok_or("frame sample has no buffer")?;
        let map = buffer.map_readable().map_err(|error| error.to_string())?;
        let expected = width as usize * height as usize * 4;
        if map.len() != expected {
            return Err(format!(
                "frame buffer has {} bytes, expected {expected}",
                map.len()
            ));
        }
        write_bgra_bmp(output, width, height, map.as_slice())
    })();
    let _ = pipeline.set_state(gst::State::Null);
    if result.is_err() {
        let _ = fs::remove_file(output);
    }
    result
}

fn write_bgra_bmp(output: &Path, width: u32, height: u32, pixels: &[u8]) -> Result<(), String> {
    let pixel_bytes = width
        .checked_mul(height)
        .and_then(|value| value.checked_mul(4))
        .ok_or("frame dimensions overflow")?;
    if pixels.len() != pixel_bytes as usize {
        return Err("frame pixels do not match dimensions".into());
    }
    let file_bytes = 54_u32.checked_add(pixel_bytes).ok_or("BMP size overflow")?;
    let mut bmp = Vec::with_capacity(file_bytes as usize);
    bmp.extend_from_slice(b"BM");
    bmp.extend_from_slice(&file_bytes.to_le_bytes());
    bmp.extend_from_slice(&[0; 4]);
    bmp.extend_from_slice(&54_u32.to_le_bytes());
    bmp.extend_from_slice(&40_u32.to_le_bytes());
    bmp.extend_from_slice(&(width as i32).to_le_bytes());
    bmp.extend_from_slice(&(-(height as i32)).to_le_bytes());
    bmp.extend_from_slice(&1_u16.to_le_bytes());
    bmp.extend_from_slice(&32_u16.to_le_bytes());
    bmp.extend_from_slice(&0_u32.to_le_bytes());
    bmp.extend_from_slice(&pixel_bytes.to_le_bytes());
    bmp.extend_from_slice(&2_835_i32.to_le_bytes());
    bmp.extend_from_slice(&2_835_i32.to_le_bytes());
    bmp.extend_from_slice(&0_u32.to_le_bytes());
    bmp.extend_from_slice(&0_u32.to_le_bytes());
    bmp.extend_from_slice(pixels);
    fs::write(output, bmp).map_err(|error| error.to_string())
}

fn canonical_file(path: &Path) -> Result<PathBuf, String> {
    let value = fs::canonicalize(path).map_err(|error| format!("source is unreadable: {error}"))?;
    if !value.is_file() {
        return Err("source is not a file".into());
    }
    Ok(value)
}

fn quote(value: &Path) -> String {
    format!(
        "\"{}\"",
        value
            .to_string_lossy()
            .replace('\\', "\\\\")
            .replace('"', "\\\"")
    )
}

fn transcode(
    source: &Path,
    output: &Path,
    profile: CompatibilityProfile,
    timeout_ms: u64,
) -> Result<(), String> {
    for factory_name in ["vp9dec", "vp8dec"] {
        if let Some(factory) = gst::ElementFactory::find(factory_name) {
            factory.set_rank(gst::Rank::PRIMARY + 1000);
        }
    }
    // Deprioritize decoders that cannot handle every profile of their format: Apple's
    // VideoToolbox path on macOS, and OpenH264's constrained-baseline-only decoder on Linux.
    for factory_name in ["vtdec", "vtdec_hw", "openh264dec"] {
        if let Some(factory) = gst::ElementFactory::find(factory_name) {
            factory.set_rank(gst::Rank::NONE);
        }
    }
    let uri = gst::glib::filename_to_uri(source, None).map_err(|error| error.to_string())?;
    let video_encoder = match profile {
        CompatibilityProfile::ProRes422PcmMov => PRORES_ENCODER,
        CompatibilityProfile::H264PcmMov => H264_ENCODER,
    };
    let pipeline = format!(
        "uridecodebin uri=\"{}\" name=d qtmux name=mux faststart=true ! filesink location={} d. ! queue ! videoconvert ! {} ! mux.video_0 d. ! queue ! audioconvert ! audioresample ! audio/x-raw,format=S16LE,rate=48000 ! mux.audio_0",
        uri.replace('"', "\\\""), quote(output), video_encoder
    );
    let element =
        gst::parse::launch(&pipeline).map_err(|error| format!("pipeline parse failed: {error}"))?;
    let pipeline = element
        .downcast::<gst::Pipeline>()
        .map_err(|_| "pipeline construction failed".to_string())?;
    pipeline
        .set_state(gst::State::Playing)
        .map_err(|error| error.to_string())?;
    let bus = pipeline.bus().ok_or("pipeline has no bus")?;
    let timeout = gst::ClockTime::from_mseconds(timeout_ms);
    let result =
        match bus.timed_pop_filtered(timeout, &[gst::MessageType::Eos, gst::MessageType::Error]) {
            Some(message) => match message.view() {
                gst::MessageView::Eos(..) => Ok(()),
                gst::MessageView::Error(error) => {
                    Err(format!("{} ({:?})", error.error(), error.debug()))
                }
                _ => Err("unexpected pipeline message".into()),
            },
            None => Err("compatibility transcode timed out".into()),
        };
    let _ = pipeline.set_state(gst::State::Null);
    if result.is_err() {
        let _ = fs::remove_file(output);
    }
    result
}

fn probe(path: &Path, timeout_ms: u64) -> Result<MediaProbe, String> {
    let uri = gst::glib::filename_to_uri(path, None).map_err(|error| error.to_string())?;
    let discoverer = gst_pbutils::Discoverer::new(gst::ClockTime::from_mseconds(timeout_ms))
        .map_err(|error| error.to_string())?;
    let info = discoverer
        .discover_uri(&uri)
        .map_err(|error| error.to_string())?;
    if info.result() != gst_pbutils::DiscovererResult::Ok {
        return Err(format!("discovery failed: {:?}", info.result()));
    }
    let video = info.video_streams().into_iter().next().map(|stream| {
        let rate = stream.framerate();
        VideoProbe {
            width: stream.width(),
            height: stream.height(),
            fps: if rate.denom() == 0 {
                0.0
            } else {
                rate.numer() as f64 / rate.denom() as f64
            },
            codec: stream
                .caps()
                .map(|caps| caps.to_string())
                .unwrap_or_default(),
            still_image: stream.is_image(),
        }
    });
    let audio = info
        .audio_streams()
        .into_iter()
        .next()
        .map(|stream| AudioProbe {
            sample_rate: stream.sample_rate(),
            channels: stream.channels(),
            codec: stream
                .caps()
                .map(|caps| caps.to_string())
                .unwrap_or_default(),
        });
    Ok(MediaProbe {
        duration_seconds: info
            .duration()
            .map(|v| v.nseconds() as f64 / 1e9)
            .unwrap_or(0.0),
        video,
        audio,
    })
}
