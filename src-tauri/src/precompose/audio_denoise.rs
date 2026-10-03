use super::{ensure_not_cancelled, project_relative_path, seconds_to_micros, PrecomposeReport};
use crate::project::model::{
    MediaAsset, MediaKind, TimelineItemKind, TimelineSource, VideoProject,
};
use crate::render_pipeline::cancel::RenderCancellationToken;
use crate::render_pipeline::error::{PipelineError, PipelineErrorCode, PipelineResult};
use hound::{SampleFormat, WavReader, WavSpec, WavWriter};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::fs;
use std::path::Path;
use std::path::PathBuf;
use std::process::Command;
use uuid::Uuid;

#[cfg(target_os = "macos")]
const ALGORITHM: &str = "deepfilternet3-coreml-wet-dry-v1";
#[cfg(not(target_os = "macos"))]
const ALGORITHM: &str = "deepfilternet3-onnx-tract-wet-dry-v1";
#[cfg(target_os = "macos")]
const COMPOSITOR_BACKEND: &str = "coreml-deepfilternet3";
#[cfg(not(target_os = "macos"))]
const COMPOSITOR_BACKEND: &str = "tract-deepfilternet3";
const CACHE_VERSION: u32 = 3;
const MODEL_SAMPLE_RATE: u32 = 48_000;

#[derive(Debug, Clone)]
struct Task {
    track_index: usize,
    item_index: usize,
    item_id: String,
    media_id: String,
    amount: f64,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct Fingerprint<'a> {
    schema_version: u32,
    source_sha256: &'a str,
    source_in_micros: u64,
    source_out_micros: u64,
    speed_micros: u32,
    strength_micros: u32,
    algorithm: &'static str,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Manifest {
    schema_version: u32,
    fingerprint: String,
    algorithm: String,
    source_sha256: String,
    output_sha256: String,
    sample_rate: u32,
    channels: u16,
    input_frames: u64,
    output_frames: u64,
    duration_seconds: f64,
    strength: f64,
    noise_rms_before: f64,
    noise_rms_after: f64,
    source_conversion: String,
}

pub(super) fn prepare_audio_denoise(
    project_dir: &Path,
    project: &mut VideoProject,
    cancellation: Option<&RenderCancellationToken>,
) -> PipelineResult<Vec<PrecomposeReport>> {
    let tasks = collect_tasks(project)?;
    let mut reports = Vec::with_capacity(tasks.len());
    for task in tasks {
        ensure_not_cancelled(cancellation)?;
        reports.push(prepare_task(project_dir, project, &task, cancellation)?);
    }
    Ok(reports)
}

fn collect_tasks(project: &VideoProject) -> PipelineResult<Vec<Task>> {
    let mut tasks = Vec::new();
    for (track_index, track) in project.timeline.tracks.iter().enumerate() {
        if !track.enabled {
            continue;
        }
        for (item_index, item) in track.items.iter().enumerate() {
            if item.kind != TimelineItemKind::AudioClip {
                continue;
            }
            let Some(amount) = denoise_amount(item.properties.get("effects"))? else {
                continue;
            };
            let TimelineSource::Media { media_id } = &item.source else {
                return Err(vec![audio_error(
                    &item.id,
                    "Denoise requires a media-backed audio clip.",
                )]);
            };
            tasks.push(Task {
                track_index,
                item_index,
                item_id: item.id.clone(),
                media_id: media_id.clone(),
                amount,
            });
        }
    }
    Ok(tasks)
}

fn denoise_amount(effects: Option<&Value>) -> PipelineResult<Option<f64>> {
    let Some(effects) = effects else {
        return Ok(None);
    };
    let effects = effects
        .as_array()
        .ok_or_else(|| vec![audio_error("unknown", "Audio effects must be an array.")])?;
    let mut amount = None;
    for effect in effects {
        let kind = effect
            .get("effectType")
            .or_else(|| effect.get("type"))
            .and_then(Value::as_str);
        if kind != Some("audio.denoise")
            || effect.get("enabled").and_then(Value::as_bool) == Some(false)
        {
            continue;
        }
        if amount.is_some() {
            return Err(vec![audio_error(
                "unknown",
                "Only one enabled audio.denoise effect is allowed per clip.",
            )]);
        }
        let value = effect
            .get("params")
            .and_then(Value::as_object)
            .and_then(|params| params.get("amount"))
            .and_then(Value::as_f64)
            .unwrap_or(0.6);
        if !value.is_finite() || !(0.0..=1.0).contains(&value) {
            return Err(vec![audio_error(
                "unknown",
                "Denoise amount must be between zero and one.",
            )]);
        }
        amount = Some(value);
    }
    Ok(amount)
}

fn prepare_task(
    project_dir: &Path,
    project: &mut VideoProject,
    task: &Task,
    cancellation: Option<&RenderCancellationToken>,
) -> PipelineResult<PrecomposeReport> {
    let item = project.timeline.tracks[task.track_index].items[task.item_index].clone();
    let media = project
        .media
        .iter()
        .find(|media| media.id == task.media_id)
        .cloned()
        .ok_or_else(|| {
            vec![audio_error(
                &task.item_id,
                "Denoise source media was not found.",
            )]
        })?;
    if !media_supports_audio_denoise(&media) {
        return Err(vec![audio_error(
            &task.item_id,
            "Denoise source must be audio media, generated audio, or a video container with an audio stream.",
        )]);
    }
    let source_in = number(&item, "sourceIn").unwrap_or(0.0);
    let source_out = number(&item, "sourceOut").unwrap_or(media.duration_seconds);
    let speed = number(&item, "speed").unwrap_or(1.0);
    if !source_in.is_finite()
        || !source_out.is_finite()
        || source_in < 0.0
        || source_out <= source_in
        || source_out > media.duration_seconds
        || !speed.is_finite()
        || !(0.1..=8.0).contains(&speed)
    {
        return Err(vec![audio_error(
            &task.item_id,
            "Denoise trim or playback speed is invalid.",
        )]);
    }
    let source_path = project_dir.join(&media.relative_path);
    let source_sha256 = super::cache::sha256_file_cancellable(&source_path, || {
        cancellation.is_some_and(RenderCancellationToken::is_cancelled)
    })
    .map_err(|error| {
        vec![audio_error(
            &task.item_id,
            &format!("Denoise source could not be hashed: {error}"),
        )]
    })?;
    let fingerprint = super::cache::fingerprint(&Fingerprint {
        schema_version: CACHE_VERSION,
        source_sha256: &source_sha256,
        source_in_micros: seconds_to_micros(source_in)?,
        source_out_micros: seconds_to_micros(source_out)?,
        speed_micros: (speed * 1_000_000.0).round() as u32,
        strength_micros: (task.amount * 1_000_000.0).round() as u32,
        algorithm: ALGORITHM,
    })?;
    let cache_dir = project_dir
        .join("cache/audio-denoise/v1/sha256")
        .join(&fingerprint[..2])
        .join(&fingerprint);
    fs::create_dir_all(cache_dir.parent().unwrap_or(project_dir)).map_err(|error| {
        vec![audio_error(
            &task.item_id,
            &format!("Denoise cache could not be created: {error}"),
        )]
    })?;
    let output_path = cache_dir.join("output.wav");
    let manifest_path = cache_dir.join("manifest.json");
    let mut cache_hit = validate_cache(&manifest_path, &output_path, &fingerprint);
    if !cache_hit {
        if cache_dir.exists() {
            fs::remove_dir_all(&cache_dir).map_err(|error| {
                vec![audio_error(
                    &task.item_id,
                    &format!("Stale denoise cache could not be removed: {error}"),
                )]
            })?;
        }
        let staging =
            cache_dir.with_file_name(format!(".{}.staging-{}", fingerprint, Uuid::new_v4()));
        fs::create_dir_all(&staging).map_err(|error| {
            vec![audio_error(
                &task.item_id,
                &format!("Denoise staging could not be created: {error}"),
            )]
        })?;
        let decoded = staging.join("decoded.wav");
        let conversion =
            decode_to_wav_with_cancellation(&source_path, &decoded, &task.item_id, cancellation)?;
        ensure_not_cancelled(cancellation)?;
        let staged_output = staging.join("output.wav");
        let manifest = process_wav(
            &decoded,
            &staged_output,
            AudioDenoiseRequest {
                source_in,
                source_out,
                strength: task.amount,
                fingerprint: &fingerprint,
                source_sha256: &source_sha256,
                conversion,
                item_id: &task.item_id,
            },
            cancellation,
        )?;
        fs::write(
            staging.join("manifest.json"),
            serde_json::to_vec_pretty(&manifest).map_err(|error| {
                vec![audio_error(
                    &task.item_id,
                    &format!("Denoise manifest serialization failed: {error}"),
                )]
            })?,
        )
        .map_err(|error| {
            vec![audio_error(
                &task.item_id,
                &format!("Denoise manifest write failed: {error}"),
            )]
        })?;
        let _ = fs::remove_file(&decoded);
        fs::rename(&staging, &cache_dir).map_err(|error| {
            vec![audio_error(
                &task.item_id,
                &format!("Denoise cache publish failed: {error}"),
            )]
        })?;
        cache_hit = false;
    }
    let manifest: Manifest =
        serde_json::from_slice(&fs::read(&manifest_path).map_err(|error| {
            vec![audio_error(
                &task.item_id,
                &format!("Denoise manifest read failed: {error}"),
            )]
        })?)
        .map_err(|error| {
            vec![audio_error(
                &task.item_id,
                &format!("Denoise manifest is invalid: {error}"),
            )]
        })?;
    let relative_output = project_relative_path(project_dir, &output_path)?;
    let prepared_media_id = format!("audio-denoise-{fingerprint}");
    if !project
        .media
        .iter()
        .any(|candidate| candidate.id == prepared_media_id)
    {
        project.media.push(MediaAsset {
            id: prepared_media_id.clone(),
            name: Some(format!("Denoised {}", item.label)),
            relative_path: relative_output.clone(),
            kind: MediaKind::Audio,
            duration_seconds: manifest.duration_seconds,
            width: None,
            height: None,
            fps: None,
            folder_id: media.folder_id.clone(),
        });
    }
    let prepared = &mut project.timeline.tracks[task.track_index].items[task.item_index];
    prepared.source = TimelineSource::Media {
        media_id: prepared_media_id.clone(),
    };
    prepared
        .properties
        .insert("sourceIn".to_string(), json!(0.0));
    prepared
        .properties
        .insert("sourceOut".to_string(), json!(manifest.duration_seconds));
    prepared.properties.insert("audioDenoisePreparation".to_string(), json!({ "status":"completed", "progress":1.0, "retryable":true, "algorithm":ALGORITHM, "fingerprint":fingerprint, "artifact":relative_output }));
    remove_denoise_effect(prepared.properties.get_mut("effects"));
    Ok(PrecomposeReport {
        stage: "audioDenoise".to_string(),
        item_id: task.item_id.clone(),
        media_id: task.media_id.clone(),
        prepared_media_id,
        fingerprint,
        cache_hit,
        expressions_enabled: false,
        compositor_backend: Some(COMPOSITOR_BACKEND.to_string()),
        compositor_fallback: None,
        worker_manifest: project_relative_path(project_dir, &manifest_path)?,
        intermediate: relative_output,
    })
}

pub(crate) fn decode_to_wav(
    source: &Path,
    destination: &Path,
    item_id: &str,
) -> PipelineResult<String> {
    decode_to_wav_with_cancellation(source, destination, item_id, None)
}

fn decode_to_wav_with_cancellation(
    source: &Path,
    destination: &Path,
    item_id: &str,
    cancellation: Option<&RenderCancellationToken>,
) -> PipelineResult<String> {
    if source
        .extension()
        .and_then(|value| value.to_str())
        .is_some_and(|value| value.eq_ignore_ascii_case("wav"))
    {
        fs::copy(source, destination).map_err(|error| {
            vec![audio_error(
                item_id,
                &format!("PCM/WAV staging failed: {error}"),
            )]
        })?;
        return Ok("pcm-wav-local".to_string());
    }
    let extension = source
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    if !is_supported_compressed_extension(&extension) {
        return Err(vec![audio_error(
            item_id,
            "Compressed denoise input format is unsupported.",
        )]);
    }
    #[cfg(target_os = "macos")]
    {
        let _ = cancellation;
        let output = Command::new("/usr/bin/afconvert")
            .args([
                "-f",
                "WAVE",
                "-d",
                "LEI16@48000",
                "-c",
                "1",
                &source.display().to_string(),
                &destination.display().to_string(),
            ])
            .output()
            .map_err(|error| {
                vec![audio_error(
                    item_id,
                    &format!("System afconvert audio conversion is unavailable: {error}"),
                )]
            })?;
        output
            .status
            .success()
            .then(|| "macos-afconvert-wave-pcm16-v1".to_string())
            .ok_or_else(|| {
                vec![audio_error(
                    item_id,
                    &format!(
                        "System afconvert audio conversion failed: {}",
                        String::from_utf8_lossy(&output.stderr)
                    ),
                )]
            })
    }
    #[cfg(all(not(target_os = "macos"), feature = "ges-render"))]
    {
        crate::render_runtime::render_runtime_environment().map_err(|error| {
            vec![audio_error(
                item_id,
                &format!(
                    "GStreamer audio conversion requires the render runtime: {}",
                    error.message
                ),
            )]
        })?;
        gstreamer_decode::decode_to_pcm16_mono_wav(
            source,
            destination,
            &|| cancellation.is_some_and(RenderCancellationToken::is_cancelled),
            gstreamer_decode::DECODE_TIMEOUT,
        )
        .map(|()| gstreamer_decode::CONVERSION_BACKEND.to_string())
        .map_err(|message| vec![audio_error(item_id, &message)])
    }
    #[cfg(all(not(target_os = "macos"), not(feature = "ges-render")))]
    {
        let _ = (destination, cancellation);
        Err(vec![audio_error(
            item_id,
            "Compressed audio conversion requires the GStreamer render runtime.",
        )])
    }
}

/// Decodes the first audio stream of a container to 48 kHz mono PCM16 WAV, matching the macOS
/// `afconvert -f WAVE -d LEI16@48000 -c 1` contract.
#[cfg(all(not(target_os = "macos"), feature = "ges-render"))]
mod gstreamer_decode {
    use gstreamer as gst;
    use gstreamer::glib;
    use gstreamer::prelude::*;
    use std::path::Path;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::{Arc, Mutex};
    use std::time::{Duration, Instant};

    pub(super) const CONVERSION_BACKEND: &str = "gstreamer-wave-pcm16-mono-48k-v1";
    pub(super) const DECODE_TIMEOUT: Duration = Duration::from_secs(30 * 60);
    const POLL_INTERVAL: Duration = Duration::from_millis(100);
    /// `GstAutoplugSelectResult` values from decodebin.
    const AUTOPLUG_TRY: i32 = 0;
    const AUTOPLUG_EXPOSE: i32 = 1;

    #[derive(Default)]
    struct DecodeState {
        audio_linked: AtomicBool,
        no_more_pads: AtomicBool,
        unlinked_caps: Mutex<Vec<String>>,
    }

    fn element(factory: &str) -> Result<gst::Element, String> {
        gst::ElementFactory::make(factory)
            .build()
            .map_err(|error| format!("GStreamer element {factory} is unavailable: {error}"))
    }

    pub(super) fn decode_to_pcm16_mono_wav(
        source: &Path,
        destination: &Path,
        is_cancelled: &dyn Fn() -> bool,
        timeout: Duration,
    ) -> Result<(), String> {
        let pipeline = gst::Pipeline::with_name("video-creater-audio-decode");
        let result = run_pipeline(&pipeline, source, destination, is_cancelled, timeout);
        let _ = pipeline.set_state(gst::State::Null);
        if result.is_err() {
            let _ = std::fs::remove_file(destination);
        }
        result
    }

    fn run_pipeline(
        pipeline: &gst::Pipeline,
        source: &Path,
        destination: &Path,
        is_cancelled: &dyn Fn() -> bool,
        timeout: Duration,
    ) -> Result<(), String> {
        let file_source = gst::ElementFactory::make("filesrc")
            .property("location", source.to_string_lossy().as_ref())
            .build()
            .map_err(|error| format!("GStreamer element filesrc is unavailable: {error}"))?;
        let decoder = element("decodebin")?;
        let convert = element("audioconvert")?;
        let resample = element("audioresample")?;
        let caps = gst::Caps::builder("audio/x-raw")
            .field("format", "S16LE")
            .field("layout", "interleaved")
            .field("rate", 48_000_i32)
            .field("channels", 1_i32)
            .build();
        let caps_filter = gst::ElementFactory::make("capsfilter")
            .property("caps", &caps)
            .build()
            .map_err(|error| format!("GStreamer element capsfilter is unavailable: {error}"))?;
        let encoder = element("wavenc")?;
        let sink = gst::ElementFactory::make("filesink")
            .property("location", destination.to_string_lossy().as_ref())
            .build()
            .map_err(|error| format!("GStreamer element filesink is unavailable: {error}"))?;
        pipeline
            .add_many([
                &file_source,
                &decoder,
                &convert,
                &resample,
                &caps_filter,
                &encoder,
                &sink,
            ])
            .map_err(|error| format!("GStreamer audio decode pipeline assembly failed: {error}"))?;
        file_source
            .link(&decoder)
            .map_err(|error| format!("GStreamer source link failed: {error}"))?;
        gst::Element::link_many([&convert, &resample, &caps_filter, &encoder, &sink])
            .map_err(|error| format!("GStreamer PCM conversion link failed: {error}"))?;

        // Expose video, image, and subtitle streams undecoded; only audio needs decoding.
        let select_result =
            glib::Type::from_name("GstAutoplugSelectResult").and_then(glib::EnumClass::with_type);
        decoder.connect("autoplug-select", false, move |values| {
            let factory = values.get(3)?.get::<gst::ElementFactory>().ok()?;
            let klass = factory
                .metadata(gst::ELEMENT_METADATA_KLASS)
                .unwrap_or_default();
            let skip_decode = klass.contains("Decoder")
                && ["Video", "Image", "Subtitle"]
                    .iter()
                    .any(|kind| klass.contains(kind));
            let result = if skip_decode {
                AUTOPLUG_EXPOSE
            } else {
                AUTOPLUG_TRY
            };
            select_result
                .as_ref()
                .and_then(|class| class.to_value(result))
        });

        let state = Arc::new(DecodeState::default());
        let pad_state = Arc::clone(&state);
        let weak_pipeline = pipeline.downgrade();
        let audio_sink_pad = convert
            .static_pad("sink")
            .ok_or_else(|| "audioconvert has no sink pad".to_string())?;
        decoder.connect_pad_added(move |_, pad| {
            let caps = pad.current_caps().unwrap_or_else(|| pad.query_caps(None));
            let name = caps
                .structure(0)
                .map(|structure| structure.name().to_string())
                .unwrap_or_default();
            if name == "audio/x-raw"
                && !pad_state.audio_linked.load(Ordering::Acquire)
                && pad.link(&audio_sink_pad).is_ok()
            {
                pad_state.audio_linked.store(true, Ordering::Release);
                return;
            }
            if let Ok(mut unlinked) = pad_state.unlinked_caps.lock() {
                unlinked.push(name);
            }
            let Some(pipeline) = weak_pipeline.upgrade() else {
                return;
            };
            let Ok(discard) = gst::ElementFactory::make("fakesink")
                .property("sync", false)
                .property("async", false)
                .build()
            else {
                return;
            };
            if pipeline.add(&discard).is_ok() {
                let _ = discard.sync_state_with_parent();
                if let Some(discard_pad) = discard.static_pad("sink") {
                    let _ = pad.link(&discard_pad);
                }
            }
        });
        let no_more_pads_state = Arc::clone(&state);
        decoder.connect_no_more_pads(move |_| {
            no_more_pads_state
                .no_more_pads
                .store(true, Ordering::Release);
        });

        pipeline
            .set_state(gst::State::Playing)
            .map_err(|error| format!("GStreamer audio decode could not start: {error}"))?;
        let bus = pipeline
            .bus()
            .ok_or_else(|| "GStreamer audio decode pipeline has no bus".to_string())?;
        let deadline = Instant::now() + timeout;
        loop {
            if is_cancelled() {
                return Err("Audio conversion was cancelled.".to_string());
            }
            if Instant::now() >= deadline {
                return Err(format!(
                    "GStreamer audio conversion timed out after {} seconds.",
                    timeout.as_secs()
                ));
            }
            if state.no_more_pads.load(Ordering::Acquire)
                && !state.audio_linked.load(Ordering::Acquire)
            {
                let unlinked = state
                    .unlinked_caps
                    .lock()
                    .map(|caps| caps.join(", "))
                    .unwrap_or_default();
                return Err(format!(
                    "Source has no decodable audio stream (streams: {}).",
                    if unlinked.is_empty() {
                        "none"
                    } else {
                        &unlinked
                    }
                ));
            }
            let Some(message) = bus.timed_pop(gst::ClockTime::from_nseconds(
                POLL_INTERVAL.as_nanos() as u64,
            )) else {
                continue;
            };
            match message.view() {
                gst::MessageView::Eos(..) => {
                    return if state.audio_linked.load(Ordering::Acquire) {
                        Ok(())
                    } else {
                        Err("Source has no decodable audio stream.".to_string())
                    };
                }
                gst::MessageView::Error(error) => {
                    return Err(format!(
                        "GStreamer audio conversion failed: {} ({})",
                        error.error(),
                        error.debug().unwrap_or_default()
                    ));
                }
                _ => {}
            }
        }
    }
}

fn is_supported_compressed_extension(extension: &str) -> bool {
    [
        "mp3", "m4a", "aac", "flac", "ogg", "opus", "mp4", "mov", "m4v", "aif", "aiff", "caf",
    ]
    .contains(&extension)
        || (cfg!(not(target_os = "macos")) && ["webm", "mkv", "oga"].contains(&extension))
}

fn media_supports_audio_denoise(media: &MediaAsset) -> bool {
    if matches!(media.kind, MediaKind::Audio | MediaKind::Video) {
        return true;
    }
    if media.kind != MediaKind::Generated {
        return false;
    }
    let extension = Path::new(&media.relative_path)
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase();
    extension == "wav" || is_supported_compressed_extension(&extension)
}

#[cfg(test)]
fn preferred_conversion_backend(extension: &str) -> Option<&'static str> {
    if extension.eq_ignore_ascii_case("wav") {
        Some("pcm-wav-local")
    } else if is_supported_compressed_extension(&extension.to_ascii_lowercase()) {
        #[cfg(target_os = "macos")]
        {
            Some("macos-afconvert-wave-pcm16-v1")
        }
        #[cfg(all(not(target_os = "macos"), feature = "ges-render"))]
        {
            Some(gstreamer_decode::CONVERSION_BACKEND)
        }
        #[cfg(all(not(target_os = "macos"), not(feature = "ges-render")))]
        {
            None
        }
    } else {
        None
    }
}

struct AudioDenoiseRequest<'a> {
    source_in: f64,
    source_out: f64,
    strength: f64,
    fingerprint: &'a str,
    source_sha256: &'a str,
    conversion: String,
    item_id: &'a str,
}

fn process_wav(
    source: &Path,
    destination: &Path,
    request: AudioDenoiseRequest<'_>,
    cancellation: Option<&RenderCancellationToken>,
) -> PipelineResult<Manifest> {
    let AudioDenoiseRequest {
        source_in,
        source_out,
        strength,
        fingerprint,
        source_sha256,
        conversion,
        item_id,
    } = request;
    let mut reader = WavReader::open(source).map_err(|error| {
        vec![audio_error(
            item_id,
            &format!("Decoded WAV is invalid: {error}"),
        )]
    })?;
    let spec = reader.spec();
    if spec.channels == 0 || spec.sample_rate == 0 {
        return Err(vec![audio_error(
            item_id,
            "Decoded WAV has invalid channel or sample-rate metadata.",
        )]);
    }
    let samples = match (spec.sample_format, spec.bits_per_sample) {
        (SampleFormat::Int, bits) if bits <= 16 => reader
            .samples::<i16>()
            .map(|sample| sample.map(|value| f32::from(value) / 32768.0))
            .collect::<Result<Vec<_>, _>>(),
        (SampleFormat::Int, _) => reader
            .samples::<i32>()
            .map(|sample| sample.map(|value| value as f32 / 2_147_483_648.0))
            .collect::<Result<Vec<_>, _>>(),
        (SampleFormat::Float, _) => reader.samples::<f32>().collect::<Result<Vec<_>, _>>(),
    }
    .map_err(|error| {
        vec![audio_error(
            item_id,
            &format!("Decoded WAV samples could not be read: {error}"),
        )]
    })?;
    let channels = usize::from(spec.channels);
    let total_frames = samples.len() / channels;
    let start = (source_in * f64::from(spec.sample_rate)).round().max(0.0) as usize;
    let end = (source_out * f64::from(spec.sample_rate))
        .round()
        .min(total_frames as f64) as usize;
    if end <= start {
        return Err(vec![audio_error(
            item_id,
            "Denoise trim contains no PCM frames.",
        )]);
    }
    let selected = &samples[start * channels..end * channels];
    // Denoise runs over the source span at 1x; retimed clips keep their speed and
    // are retimed with pitch preserved afterwards (`audio_retime.rs`).
    let output_frames = (((end - start) as f64 / f64::from(spec.sample_rate))
        * f64::from(MODEL_SAMPLE_RATE))
    .round()
    .max(1.0) as usize;
    let mut resampled = vec![0_f32; output_frames * channels];
    for frame in 0..output_frames {
        if frame % 4096 == 0 {
            ensure_not_cancelled(cancellation)?;
        }
        let source_position =
            frame as f64 * f64::from(spec.sample_rate) / f64::from(MODEL_SAMPLE_RATE);
        let left = source_position.floor().min((end - start - 1) as f64) as usize;
        let right = (left + 1).min(end - start - 1);
        let fraction = (source_position - left as f64) as f32;
        for channel in 0..channels {
            resampled[frame * channels + channel] = selected[left * channels + channel]
                * (1.0 - fraction)
                + selected[right * channels + channel] * fraction;
        }
    }
    let noise_before = quiet_rms(&resampled);
    let output_spec = WavSpec {
        channels: spec.channels,
        sample_rate: MODEL_SAMPLE_RATE,
        bits_per_sample: 16,
        sample_format: SampleFormat::Int,
    };
    let model_input = destination.with_extension("model-input.wav");
    write_pcm16(&model_input, output_spec, &resampled, item_id)?;
    run_model_enhancer(&model_input, destination, strength, item_id)?;
    let enhanced = read_pcm_f32(destination, item_id)?;
    let noise_after = quiet_rms(&enhanced);
    let _ = fs::remove_file(&model_input);
    let output_sha256 = super::cache::sha256_file(destination).map_err(|error| {
        vec![audio_error(
            item_id,
            &format!("Denoised WAV hash read failed: {error}"),
        )]
    })?;
    Ok(Manifest {
        schema_version: CACHE_VERSION,
        fingerprint: fingerprint.to_string(),
        algorithm: ALGORITHM.to_string(),
        source_sha256: source_sha256.to_string(),
        output_sha256,
        sample_rate: MODEL_SAMPLE_RATE,
        channels: spec.channels,
        input_frames: (end - start) as u64,
        output_frames: output_frames as u64,
        duration_seconds: output_frames as f64 / f64::from(MODEL_SAMPLE_RATE),
        strength,
        noise_rms_before: noise_before,
        noise_rms_after: noise_after,
        source_conversion: conversion,
    })
}

fn write_pcm16(
    destination: &Path,
    spec: WavSpec,
    samples: &[f32],
    item_id: &str,
) -> PipelineResult<()> {
    let mut writer = WavWriter::create(destination, spec).map_err(|error| {
        vec![audio_error(
            item_id,
            &format!("Enhancement input WAV could not be created: {error}"),
        )]
    })?;
    for sample in samples {
        writer
            .write_sample((sample.clamp(-1.0, 1.0) * 32767.0).round() as i16)
            .map_err(|error| {
                vec![audio_error(
                    item_id,
                    &format!("Denoised WAV write failed: {error}"),
                )]
            })?;
    }
    writer.finalize().map_err(|error| {
        vec![audio_error(
            item_id,
            &format!("Enhancement input WAV finalize failed: {error}"),
        )]
    })
}

fn read_pcm_f32(path: &Path, item_id: &str) -> PipelineResult<Vec<f32>> {
    let mut reader = WavReader::open(path).map_err(|error| {
        vec![audio_error(
            item_id,
            &format!("Enhanced WAV is invalid: {error}"),
        )]
    })?;
    reader
        .samples::<i16>()
        .map(|sample| sample.map(|value| f32::from(value) / 32768.0))
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| {
            vec![audio_error(
                item_id,
                &format!("Enhanced WAV samples are invalid: {error}"),
            )]
        })
}

fn run_model_enhancer(
    input: &Path,
    output: &Path,
    strength: f64,
    item_id: &str,
) -> PipelineResult<()> {
    let helper = audio_enhancer_path()?;
    let model_directory = std::env::var_os("VIDEO_CREATER_DEEPFILTERNET3_MODEL_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(default_model_directory);
    fs::create_dir_all(&model_directory).map_err(|error| {
        vec![audio_error(
            item_id,
            &format!("Speech enhancement model cache could not be created: {error}"),
        )]
    })?;
    let result = Command::new(&helper)
        .args([
            "--input",
            &input.display().to_string(),
            "--output",
            &output.display().to_string(),
            "--model-directory",
            &model_directory.display().to_string(),
            "--strength",
            &strength.to_string(),
        ])
        .output()
        .map_err(|error| {
            vec![audio_error(
                item_id,
                &format!("DeepFilterNet3 helper could not start: {error}"),
            )]
        })?;
    if !result.status.success() || !output.is_file() {
        return Err(vec![audio_error(
            item_id,
            &format!(
                "DeepFilterNet3 enhancement failed: {}",
                String::from_utf8_lossy(&result.stderr)
            ),
        )]);
    }
    Ok(())
}

#[cfg(target_os = "macos")]
fn default_model_directory() -> PathBuf {
    std::env::var_os("HOME")
        .map(PathBuf::from)
        .unwrap_or_else(std::env::temp_dir)
        .join("Library/Application Support/com.olhapi.video-creater/models/speech-enhancement/deepfilternet3-coreml")
}

/// Tauri's `app_data_dir` on Linux: `$XDG_DATA_HOME/<identifier>` or
/// `$HOME/.local/share/<identifier>`.
#[cfg(not(target_os = "macos"))]
fn default_model_directory() -> PathBuf {
    linux_app_data_dir(
        std::env::var_os("XDG_DATA_HOME").map(PathBuf::from),
        std::env::var_os("HOME").map(PathBuf::from),
    )
    .join("models/speech-enhancement/deepfilternet3")
}

#[cfg(not(target_os = "macos"))]
fn linux_app_data_dir(xdg_data_home: Option<PathBuf>, home: Option<PathBuf>) -> PathBuf {
    xdg_data_home
        .filter(|path| path.is_absolute())
        .or_else(|| home.map(|home| home.join(".local/share")))
        .unwrap_or_else(std::env::temp_dir)
        .join("com.olhapi.video-creater")
}

#[cfg(target_os = "macos")]
fn audio_enhancer_path() -> PipelineResult<PathBuf> {
    if let Some(path) = std::env::var_os("VIDEO_CREATER_AUDIO_ENHANCER") {
        return Ok(path.into());
    }
    #[cfg(debug_assertions)]
    {
        let staged = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("binaries/video-creater-audio-enhance-aarch64-apple-darwin");
        if staged.is_file() {
            return Ok(staged);
        }
    }
    let current = std::env::current_exe().map_err(|error| {
        vec![audio_error(
            "helper",
            &format!("Current executable path could not be resolved: {error}"),
        )]
    })?;
    let mut directory = current.parent().unwrap_or(Path::new("."));
    if directory.file_name().and_then(|value| value.to_str()) == Some("deps") {
        directory = directory.parent().unwrap_or(directory);
    }
    Ok(directory.join("video-creater-audio-enhance"))
}

#[cfg(not(target_os = "macos"))]
const AUDIO_ENHANCER_EXECUTABLE: &str = "video-creater-audio-enhance";

#[cfg(not(target_os = "macos"))]
fn audio_enhancer_path() -> PipelineResult<PathBuf> {
    if let Some(path) = std::env::var_os("VIDEO_CREATER_AUDIO_ENHANCER") {
        return Ok(path.into());
    }
    let current = std::env::current_exe().map_err(|error| {
        vec![audio_error(
            "helper",
            &format!("Current executable path could not be resolved: {error}"),
        )]
    })?;
    audio_enhancer_candidates(&current, cfg!(debug_assertions))
        .into_iter()
        .find(|candidate| is_runnable_helper(candidate))
        .ok_or_else(|| {
            vec![audio_error(
                "helper",
                "The bundled DeepFilterNet3 speech enhancement helper was not found next to the application.",
            )]
        })
}

/// Packaged builds install the sidecar next to the application executable. Development builds
/// also accept the staged Tauri sidecar and the Cargo target directories.
#[cfg(not(target_os = "macos"))]
fn audio_enhancer_candidates(current_exe: &Path, development: bool) -> Vec<PathBuf> {
    let mut directory = current_exe.parent().unwrap_or(Path::new("."));
    if directory.file_name().and_then(|value| value.to_str()) == Some("deps") {
        directory = directory.parent().unwrap_or(directory);
    }
    let mut candidates = vec![directory.join(AUDIO_ENHANCER_EXECUTABLE)];
    if development {
        let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
        candidates.push(manifest_dir.join("binaries").join(format!(
            "{AUDIO_ENHANCER_EXECUTABLE}-{}",
            linux_host_target_triple()
        )));
        for profile in ["release", "debug"] {
            candidates.push(
                manifest_dir
                    .join("target")
                    .join(profile)
                    .join(AUDIO_ENHANCER_EXECUTABLE),
            );
        }
    }
    candidates
}

#[cfg(not(target_os = "macos"))]
fn linux_host_target_triple() -> &'static str {
    if cfg!(target_arch = "aarch64") {
        "aarch64-unknown-linux-gnu"
    } else {
        "x86_64-unknown-linux-gnu"
    }
}

/// Empty placeholder sidecars (used to satisfy Tauri's build-time `externalBin` check) are
/// not runnable helpers.
#[cfg(not(target_os = "macos"))]
fn is_runnable_helper(path: &Path) -> bool {
    fs::metadata(path).is_ok_and(|metadata| metadata.is_file() && metadata.len() > 0)
}

fn quiet_rms(samples: &[f32]) -> f64 {
    let mut magnitudes = samples
        .iter()
        .map(|sample| sample.abs())
        .collect::<Vec<_>>();
    magnitudes.sort_by(f32::total_cmp);
    let count = (magnitudes.len() / 5).max(1);
    (magnitudes[..count]
        .iter()
        .map(|sample| f64::from(*sample).powi(2))
        .sum::<f64>()
        / count as f64)
        .sqrt()
}

fn validate_cache(manifest: &Path, output: &Path, fingerprint: &str) -> bool {
    let Ok(manifest) = fs::read(manifest)
        .ok()
        .and_then(|bytes| serde_json::from_slice::<Manifest>(&bytes).ok())
        .ok_or(())
    else {
        return false;
    };
    if manifest.schema_version != CACHE_VERSION
        || manifest.fingerprint != fingerprint
        || manifest.algorithm != ALGORITHM
    {
        return false;
    }
    super::cache::sha256_file(output)
        .ok()
        .is_some_and(|hash| hash == manifest.output_sha256)
}

fn remove_denoise_effect(effects: Option<&mut Value>) {
    let Some(effects) = effects.and_then(Value::as_array_mut) else {
        return;
    };
    effects.retain(|effect| {
        effect
            .get("effectType")
            .or_else(|| effect.get("type"))
            .and_then(Value::as_str)
            != Some("audio.denoise")
    });
}

fn number(item: &crate::project::model::TimelineItem, key: &str) -> Option<f64> {
    item.properties.get(key).and_then(Value::as_f64)
}
fn audio_error(item_id: &str, message: &str) -> PipelineError {
    PipelineError::new(PipelineErrorCode::RenderBackendFailed, format!("precompose.audioDenoise.{item_id}"), message, "Retry audio preparation; if native conversion is unavailable, convert the source to PCM/WAV first.")
}

#[cfg(test)]
mod tests {
    use super::*;
    #[cfg(target_os = "macos")]
    #[test]
    fn production_contract_identifies_deepfilternet3_coreml_wet_dry_processing() {
        assert_eq!(ALGORITHM, "deepfilternet3-coreml-wet-dry-v1");
        assert_eq!(MODEL_SAMPLE_RATE, 48_000);
        assert_eq!(CACHE_VERSION, 3);
    }

    #[test]
    fn generated_audio_and_video_outputs_are_valid_denoise_sources_but_images_are_not() {
        let generated = |relative_path: &str| MediaAsset {
            id: "generated-output".to_string(),
            name: None,
            relative_path: relative_path.to_string(),
            kind: MediaKind::Generated,
            duration_seconds: 2.0,
            width: None,
            height: None,
            fps: None,
            folder_id: None,
        };
        assert!(media_supports_audio_denoise(&generated(
            "generated/output.mp3"
        )));
        assert!(media_supports_audio_denoise(&generated(
            "generated/output.wav"
        )));
        assert!(media_supports_audio_denoise(&generated(
            "generated/output.mp4"
        )));
        assert!(!media_supports_audio_denoise(&generated(
            "generated/output.png"
        )));
    }

    #[test]
    fn wav_fixture_preserves_trim_and_speed_timing_while_reducing_noise() {
        let temporary = tempfile::tempdir().expect("audio fixture tempdir");
        let source = temporary.path().join("speech-noise.wav");
        let output = temporary.path().join("denoised.wav");
        let helper = temporary.path().join("fake-enhancer.sh");
        fs::write(
            &helper,
            "#!/bin/sh\nwhile [ $# -gt 0 ]; do case \"$1\" in --input) input=$2; shift 2;; --output) output=$2; shift 2;; *) shift 2;; esac; done\ncp \"$input\" \"$output\"\n",
        )
        .expect("fake helper");
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(&helper, fs::Permissions::from_mode(0o755)).expect("helper mode");
        }
        std::env::set_var("VIDEO_CREATER_AUDIO_ENHANCER", &helper);
        std::env::set_var(
            "VIDEO_CREATER_DEEPFILTERNET3_MODEL_DIR",
            temporary.path().join("model"),
        );
        let spec = WavSpec {
            channels: 1,
            sample_rate: 48_000,
            bits_per_sample: 16,
            sample_format: SampleFormat::Int,
        };
        let mut writer = WavWriter::create(&source, spec).expect("fixture writer");
        for index in 0..48_000 {
            let seconds = index as f32 / 48_000.0;
            let speech = if (0.25..0.75).contains(&seconds) {
                (seconds * 440.0 * std::f32::consts::TAU).sin() * 0.45
            } else {
                0.0
            };
            let noise = if index % 2 == 0 { 0.03 } else { -0.03 };
            writer
                .write_sample(((speech + noise).clamp(-1.0, 1.0) * 32767.0) as i16)
                .expect("fixture sample");
        }
        writer.finalize().expect("fixture finalize");

        let manifest = process_wav(
            &source,
            &output,
            AudioDenoiseRequest {
                source_in: 0.1,
                source_out: 0.9,
                strength: 0.8,
                fingerprint: "fixture-fingerprint",
                source_sha256: "fixture-source",
                conversion: "pcm-wav-local".to_string(),
                item_id: "audio-fixture",
            },
            None,
        )
        .expect("denoise fixture");
        assert_eq!(manifest.input_frames, 38_400);
        // Denoise keeps the source span at 1x; the clip speed is applied later with pitch kept.
        assert_eq!(manifest.output_frames, 38_400);
        assert!((manifest.duration_seconds - 0.8).abs() < 1.0 / 48_000.0);
        assert!((manifest.noise_rms_after - manifest.noise_rms_before).abs() < 0.0001);
        assert_eq!(manifest.algorithm, ALGORITHM);
        assert_eq!(manifest.sample_rate, 48_000);
        let reader = WavReader::open(output).expect("denoised fixture");
        assert_eq!(reader.duration(), 38_400);
        std::env::remove_var("VIDEO_CREATER_AUDIO_ENHANCER");
        std::env::remove_var("VIDEO_CREATER_DEEPFILTERNET3_MODEL_DIR");
    }

    #[test]
    fn video_container_audio_uses_reviewed_native_conversion_contract() {
        assert_eq!(preferred_conversion_backend("wav"), Some("pcm-wav-local"));
        #[cfg(target_os = "macos")]
        for extension in ["mp4", "mov", "m4v", "m4a", "aif", "aiff", "caf"] {
            assert_eq!(
                preferred_conversion_backend(extension),
                Some("macos-afconvert-wave-pcm16-v1"),
                "{extension}",
            );
        }
        assert_eq!(preferred_conversion_backend("txt"), None);
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn system_afconvert_decodes_the_retained_mp4_speech_fixture() {
        let temporary = tempfile::tempdir().expect("afconvert tempdir");
        let source = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures/media/edison-speech-1920s-30s.mp4");
        let output = temporary.path().join("decoded.wav");

        let backend = decode_to_wav(&source, &output, "afconvert-fixture")
            .expect("system afconvert should decode the retained MP4 fixture");

        assert_eq!(backend, "macos-afconvert-wave-pcm16-v1");
        let reader = WavReader::open(output).expect("decoded WAV");
        assert_eq!(reader.spec().sample_rate, 48_000);
        assert!(reader.duration() > 48_000);
    }

    #[cfg(not(target_os = "macos"))]
    #[test]
    fn linux_contract_identifies_tract_deepfilternet3_and_xdg_model_directory() {
        assert_eq!(ALGORITHM, "deepfilternet3-onnx-tract-wet-dry-v1");
        assert_eq!(COMPOSITOR_BACKEND, "tract-deepfilternet3");
        assert_eq!(MODEL_SAMPLE_RATE, 48_000);
        assert_eq!(CACHE_VERSION, 3);
        assert_eq!(
            linux_app_data_dir(
                Some(PathBuf::from("/xdg/data")),
                Some(PathBuf::from("/home/u"))
            ),
            PathBuf::from("/xdg/data/com.olhapi.video-creater")
        );
        assert_eq!(
            linux_app_data_dir(
                Some(PathBuf::from("relative")),
                Some(PathBuf::from("/home/u"))
            ),
            PathBuf::from("/home/u/.local/share/com.olhapi.video-creater")
        );
        assert_eq!(
            linux_app_data_dir(None, Some(PathBuf::from("/home/u"))),
            PathBuf::from("/home/u/.local/share/com.olhapi.video-creater")
        );
    }

    #[cfg(not(target_os = "macos"))]
    #[test]
    fn linux_helper_resolution_covers_packaged_staged_and_target_locations() {
        let packaged =
            audio_enhancer_candidates(Path::new("/opt/video-creater/video-creater"), false);
        assert_eq!(
            packaged,
            vec![PathBuf::from(
                "/opt/video-creater/video-creater-audio-enhance"
            )]
        );
        let development = audio_enhancer_candidates(
            Path::new("/work/src-tauri/target/debug/deps/video_creater_lib-1234"),
            true,
        );
        assert_eq!(
            development[0],
            PathBuf::from("/work/src-tauri/target/debug/video-creater-audio-enhance")
        );
        let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
        assert!(development.contains(&manifest_dir.join(format!(
            "binaries/video-creater-audio-enhance-{}",
            linux_host_target_triple()
        ))));
        assert!(
            development.contains(&manifest_dir.join("target/release/video-creater-audio-enhance"))
        );

        let temporary = tempfile::tempdir().expect("helper tempdir");
        let placeholder = temporary.path().join("placeholder");
        fs::write(&placeholder, b"").expect("placeholder");
        assert!(!is_runnable_helper(&placeholder));
        fs::write(&placeholder, b"#!/bin/sh\n").expect("helper");
        assert!(is_runnable_helper(&placeholder));
    }

    #[cfg(all(not(target_os = "macos"), feature = "ges-render"))]
    fn init_test_gstreamer() {
        let _ = crate::render_runtime::start_render_process_runtime();
        gstreamer::init().expect("initialize GStreamer for audio decode fixtures");
    }

    #[cfg(all(not(target_os = "macos"), feature = "ges-render"))]
    fn encode_fixture(description: &str) {
        use gstreamer::prelude::*;
        let pipeline = gstreamer::parse::launch(description).expect("fixture pipeline");
        pipeline
            .set_state(gstreamer::State::Playing)
            .expect("fixture pipeline plays");
        let bus = pipeline.bus().expect("fixture bus");
        let message = bus
            .timed_pop_filtered(
                gstreamer::ClockTime::from_seconds(60),
                &[gstreamer::MessageType::Eos, gstreamer::MessageType::Error],
            )
            .expect("fixture pipeline finishes");
        if let gstreamer::MessageView::Error(error) = message.view() {
            panic!(
                "fixture encode failed: {} {:?}",
                error.error(),
                error.debug()
            );
        }
        pipeline
            .set_state(gstreamer::State::Null)
            .expect("fixture pipeline stops");
    }

    #[cfg(all(not(target_os = "macos"), feature = "ges-render"))]
    fn assert_decoded_sine(path: &Path, expected_seconds: f64) {
        let mut reader = WavReader::open(path).expect("decoded WAV");
        let spec = reader.spec();
        assert_eq!(spec.sample_rate, 48_000);
        assert_eq!(spec.channels, 1);
        assert_eq!(spec.bits_per_sample, 16);
        assert_eq!(spec.sample_format, SampleFormat::Int);
        let samples = reader
            .samples::<i16>()
            .map(|sample| f64::from(sample.expect("sample")) / 32768.0)
            .collect::<Vec<_>>();
        let seconds = samples.len() as f64 / 48_000.0;
        assert!(
            (seconds - expected_seconds).abs() < 0.1,
            "decoded duration {seconds}s, expected {expected_seconds}s"
        );
        let rms =
            (samples.iter().map(|value| value * value).sum::<f64>() / samples.len() as f64).sqrt();
        assert!(
            rms > 0.05,
            "decoded audio should carry the sine tone, rms {rms}"
        );
    }

    #[cfg(all(not(target_os = "macos"), feature = "ges-render"))]
    #[test]
    fn gstreamer_decodes_compressed_opus_and_webm_audio_to_mono_pcm16_48k() {
        init_test_gstreamer();
        let temporary = tempfile::tempdir().expect("decode tempdir");
        let opus = temporary.path().join("tone.opus");
        encode_fixture(&format!(
            "audiotestsrc num-buffers=100 samplesperbuffer=441 wave=sine freq=440 volume=0.5 ! audio/x-raw,rate=44100,channels=2 ! audioconvert ! audioresample ! opusenc ! oggmux ! filesink location={}",
            opus.display()
        ));
        let decoded = temporary.path().join("opus.wav");
        gstreamer_decode::decode_to_pcm16_mono_wav(
            &opus,
            &decoded,
            &|| false,
            std::time::Duration::from_secs(60),
        )
        .expect("decode Ogg Opus");
        assert_decoded_sine(&decoded, 1.0);

        let webm = temporary.path().join("tone-with-video.webm");
        encode_fixture(&format!(
            "webmmux name=mux ! filesink location={} videotestsrc num-buffers=30 ! video/x-raw,width=160,height=120,framerate=30/1 ! vp8enc deadline=1 ! mux. audiotestsrc num-buffers=48 samplesperbuffer=1000 wave=sine freq=330 volume=0.5 ! audio/x-raw,rate=48000,channels=1 ! audioconvert ! vorbisenc ! mux.",
            webm.display()
        ));
        let decoded = temporary.path().join("webm.wav");
        gstreamer_decode::decode_to_pcm16_mono_wav(
            &webm,
            &decoded,
            &|| false,
            std::time::Duration::from_secs(60),
        )
        .expect("decode WebM audio while exposing VP8 video undecoded");
        assert_decoded_sine(&decoded, 1.0);
        assert_eq!(
            preferred_conversion_backend("webm"),
            Some(gstreamer_decode::CONVERSION_BACKEND)
        );
    }

    #[cfg(all(not(target_os = "macos"), feature = "ges-render"))]
    #[test]
    fn gstreamer_decode_reports_missing_audio_and_honors_cancellation() {
        init_test_gstreamer();
        let temporary = tempfile::tempdir().expect("decode tempdir");
        let video_only = temporary.path().join("video-only.webm");
        encode_fixture(&format!(
            "videotestsrc num-buffers=10 ! video/x-raw,width=64,height=48,framerate=30/1 ! vp8enc deadline=1 ! webmmux ! filesink location={}",
            video_only.display()
        ));
        let decoded = temporary.path().join("none.wav");
        let error = gstreamer_decode::decode_to_pcm16_mono_wav(
            &video_only,
            &decoded,
            &|| false,
            std::time::Duration::from_secs(60),
        )
        .expect_err("video-only input has no audio");
        assert!(error.contains("no decodable audio stream"), "{error}");
        assert!(!decoded.exists());

        let tone = temporary.path().join("tone.ogg");
        encode_fixture(&format!(
            "audiotestsrc num-buffers=20 ! audio/x-raw,rate=48000,channels=1 ! audioconvert ! vorbisenc ! oggmux ! filesink location={}",
            tone.display()
        ));
        let error = gstreamer_decode::decode_to_pcm16_mono_wav(
            &tone,
            &decoded,
            &|| true,
            std::time::Duration::from_secs(60),
        )
        .expect_err("cancelled decode");
        assert!(error.contains("cancelled"), "{error}");
        assert!(!decoded.exists());
    }

    #[test]
    fn cache_fingerprint_covers_trim_speed_strength_and_algorithm() {
        let fingerprint = |source_in_micros, speed_micros, strength_micros| {
            super::super::cache::fingerprint(&Fingerprint {
                schema_version: CACHE_VERSION,
                source_sha256: "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
                source_in_micros,
                source_out_micros: 2_000_000,
                speed_micros,
                strength_micros,
                algorithm: ALGORITHM,
            })
            .expect("fingerprint")
        };
        let baseline = fingerprint(0, 1_000_000, 600_000);
        assert_eq!(baseline, fingerprint(0, 1_000_000, 600_000));
        assert_ne!(baseline, fingerprint(100_000, 1_000_000, 600_000));
        assert_ne!(baseline, fingerprint(0, 2_000_000, 600_000));
        assert_ne!(baseline, fingerprint(0, 1_000_000, 800_000));
    }
}
