//! Native transcription and speech-analysis pipelines (sherpa-onnx + ONNX Runtime + GStreamer).

use std::path::Path;
use std::sync::OnceLock;
use std::time::Instant;

use ort::session::Session;
use ort::value::Tensor;

use crate::audio::decode_mono_16k;
use crate::chunking::transcription_windows;
use crate::models::{
    locate_speech_model, parakeet_paths, PYANNOTE_SEGMENTATION, SILERO_VAD, WESPEAKER_EMBEDDING,
};
use crate::protocol::{
    resolve_language_hint, validate_speech_analysis_request, DiarizationEmbeddingSegment,
    HelperError, SpeechAnalysisModelMetadata, SpeechAnalysisRequest, SpeechAnalysisResponse,
    TranscriptionRequest, TranscriptionResponse, DIARIZATION_ARTIFACT_FORMAT, SAMPLE_RATE,
    SCHEMA_VERSION, SPEECH_ANALYSIS_EMBEDDING_DIMENSIONS, SPEECH_ANALYSIS_MODE,
    SPEECH_ANALYSIS_RUNTIME_ID, TRANSCRIPTION_RUNTIME_ID, VAD_ARTIFACT_FORMAT,
};
use crate::sherpa::{
    DiarizationSettings, OfflineRecognizer, ParakeetModelFiles, SpeakerDiarization,
    SpeakerEmbeddingExtractor,
};
use crate::speakers::{
    embedding_window, l2_normalized, mean_vector, order_of_appearance_labels, segment_confidence,
};
use crate::vad::{chunk_frame_inputs, noisy_or, vad_segments, CHUNK_SAMPLES, CONTEXT_SAMPLES};
use crate::words::{token_timings_from_result, words_from_tokens};

/// FastClustering threshold for WeSpeaker ResNet34-LM embeddings (smaller values split more
/// speakers). 0.45 separated both sherpa-onnx two-speaker English reference clips correctly while
/// 0.5 merged the faster dialogue into one speaker.
const CLUSTERING_THRESHOLD: f32 = 0.45;
const SILERO_STATE_SHAPE: [i64; 3] = [2, 1, 128];

pub fn transcribe(request: &TranscriptionRequest) -> Result<TranscriptionResponse, HelperError> {
    if request.schema_version != SCHEMA_VERSION {
        return Err(HelperError::UnsupportedTranscriptionSchemaVersion(
            request.schema_version,
        ));
    }
    let paths = parakeet_paths(Path::new(&request.model_path))?;
    let media = Path::new(&request.media_path);
    if !media.exists() {
        return Err(HelperError::MissingMediaFile(request.media_path.clone()));
    }
    // Validated for parity with the macOS helper; Parakeet TDT v3 identifies the language itself.
    let _language = resolve_language_hint(request.language_mode.as_deref())?;

    let started = Instant::now();
    let samples = decode_mono_16k(media)?;
    let recognizer = OfflineRecognizer::parakeet_tdt(&ParakeetModelFiles {
        encoder: &paths.encoder,
        decoder: &paths.decoder,
        joiner: &paths.joiner,
        tokens: &paths.tokens,
    })?;

    let mut texts = Vec::new();
    let mut timings = Vec::new();
    for window in transcription_windows(&samples, SAMPLE_RATE) {
        let result = recognizer.decode(&samples[window.start..window.end])?;
        let offset = window.start as f64 / f64::from(SAMPLE_RATE);
        let text = result.text.trim();
        if !text.is_empty() {
            texts.push(text.to_string());
        }
        timings.extend(token_timings_from_result(&result, offset));
    }

    Ok(TranscriptionResponse {
        duration_seconds: samples.len() as f64 / f64::from(SAMPLE_RATE),
        model_id: request.model_id.clone(),
        processing_seconds: started.elapsed().as_secs_f64(),
        runtime_id: TRANSCRIPTION_RUNTIME_ID.to_string(),
        schema_version: SCHEMA_VERSION,
        text: texts.join(" "),
        words: words_from_tokens(&timings),
    })
}

pub fn analyze_speech(
    request: &SpeechAnalysisRequest,
) -> Result<SpeechAnalysisResponse, HelperError> {
    validate_speech_analysis_request(request)?;
    let media = Path::new(&request.media_path);
    if !media.exists() {
        return Err(HelperError::MissingMediaFile(request.media_path.clone()));
    }
    let vad_model = locate_speech_model(&request.vad_model_root, SILERO_VAD)?;
    let segmentation_model =
        locate_speech_model(&request.diarization_model_root, PYANNOTE_SEGMENTATION)?;
    let embedding_model =
        locate_speech_model(&request.diarization_model_root, WESPEAKER_EMBEDDING)?;

    let started = Instant::now();
    let samples = decode_mono_16k(media)?;
    let duration_seconds = samples.len() as f64 / f64::from(SAMPLE_RATE);
    if !duration_seconds.is_finite() || duration_seconds <= 0.0 {
        return Err(HelperError::InvalidDuration(duration_seconds));
    }

    let probabilities = silero_chunk_probabilities(&vad_model, &samples)?;
    let vad_segments = vad_segments(&probabilities, duration_seconds, SAMPLE_RATE);
    let diarization_segments = diarize(
        &segmentation_model,
        &embedding_model,
        &samples,
        duration_seconds,
    )?;

    Ok(SpeechAnalysisResponse {
        diarization_model: SpeechAnalysisModelMetadata {
            artifact_format: DIARIZATION_ARTIFACT_FORMAT.to_string(),
            embedding_dimensions: Some(SPEECH_ANALYSIS_EMBEDDING_DIMENSIONS),
            id: request.diarization_model_id.clone(),
            revision: request.diarization_model_revision.clone(),
        },
        diarization_segments,
        duration_seconds,
        mode: SPEECH_ANALYSIS_MODE.to_string(),
        processing_seconds: started.elapsed().as_secs_f64(),
        runtime_id: SPEECH_ANALYSIS_RUNTIME_ID.to_string(),
        sample_rate: SAMPLE_RATE,
        schema_version: SCHEMA_VERSION,
        vad_model: SpeechAnalysisModelMetadata {
            artifact_format: VAD_ARTIFACT_FORMAT.to_string(),
            embedding_dimensions: None,
            id: request.vad_model_id.clone(),
            revision: request.vad_model_revision.clone(),
        },
        vad_segments,
    })
}

extern "C" {
    // Provided by the bundled libonnxruntime.so, which sherpa-onnx also links.
    fn OrtGetApiBase() -> *const ort::sys::OrtApiBase;
}

fn install_onnxruntime_api() -> Result<(), HelperError> {
    static INSTALLED: OnceLock<Result<(), String>> = OnceLock::new();
    INSTALLED
        .get_or_init(|| {
            // SAFETY: OrtGetApiBase returns a static table and GetApi a static OrtApi for every
            // supported API version (the bundled ONNX Runtime 1.28 supports `ort`'s API 17).
            unsafe {
                let base = OrtGetApiBase();
                if base.is_null() {
                    return Err("bundled ONNX Runtime does not export OrtGetApiBase".to_string());
                }
                let api = ((*base).GetApi)(ort::MINOR_VERSION);
                if api.is_null() {
                    return Err(format!(
                        "bundled ONNX Runtime does not support C API version {}",
                        ort::MINOR_VERSION
                    ));
                }
                ort::set_api(std::ptr::read(api));
            }
            Ok(())
        })
        .clone()
        .map_err(HelperError::Runtime)
}

fn silero_chunk_probabilities(model: &Path, samples: &[f32]) -> Result<Vec<f64>, HelperError> {
    install_onnxruntime_api()?;
    let runtime_error = |detail: String| HelperError::Runtime(format!("Silero VAD: {detail}"));
    let mut session = Session::builder()
        .map_err(|error| runtime_error(error.to_string()))?
        .with_intra_threads(1)
        .map_err(|error| runtime_error(error.to_string()))?
        .commit_from_file(model)
        .map_err(|error| runtime_error(error.to_string()))?;

    let mut state = vec![0.0_f32; SILERO_STATE_SHAPE.iter().product::<i64>() as usize];
    let mut context = [0.0_f32; CONTEXT_SAMPLES];
    let mut probabilities = Vec::with_capacity(samples.len().div_ceil(CHUNK_SAMPLES));
    for chunk in samples.chunks(CHUNK_SAMPLES) {
        let (frames, next_context) = chunk_frame_inputs(chunk, &context);
        context = next_context;
        let mut frame_probabilities = Vec::with_capacity(frames.len());
        for frame in frames {
            let inputs = ort::inputs![
                "input" => Tensor::from_array((vec![1_i64, frame.len() as i64], frame.to_vec()))
                    .map_err(|error| runtime_error(error.to_string()))?,
                "state" => Tensor::from_array((SILERO_STATE_SHAPE.to_vec(), state.clone()))
                    .map_err(|error| runtime_error(error.to_string()))?,
                "sr" => Tensor::from_array((Vec::<i64>::new(), vec![i64::from(SAMPLE_RATE)]))
                    .map_err(|error| runtime_error(error.to_string()))?,
            ];
            let outputs = session
                .run(inputs)
                .map_err(|error| runtime_error(error.to_string()))?;
            let (_, probability) = outputs["output"]
                .try_extract_tensor::<f32>()
                .map_err(|error| runtime_error(error.to_string()))?;
            let (_, next_state) = outputs["stateN"]
                .try_extract_tensor::<f32>()
                .map_err(|error| runtime_error(error.to_string()))?;
            if next_state.len() != state.len() {
                return Err(runtime_error(format!(
                    "unexpected state size {}",
                    next_state.len()
                )));
            }
            state.copy_from_slice(next_state);
            frame_probabilities.push(probability.first().copied().unwrap_or(0.0));
        }
        probabilities.push(noisy_or(&frame_probabilities));
    }
    Ok(probabilities)
}

fn diarize(
    segmentation_model: &Path,
    embedding_model: &Path,
    samples: &[f32],
    duration_seconds: f64,
) -> Result<Vec<DiarizationEmbeddingSegment>, HelperError> {
    let diarization = SpeakerDiarization::new(
        segmentation_model,
        embedding_model,
        &DiarizationSettings {
            clustering_threshold: CLUSTERING_THRESHOLD,
            min_duration_on: 0.3,
            min_duration_off: 0.5,
        },
    )?;
    let turns = diarization.process(samples)?;
    drop(diarization);
    if turns.is_empty() {
        return Ok(Vec::new());
    }

    let extractor = SpeakerEmbeddingExtractor::new(embedding_model)?;
    let dimensions = extractor.dimensions();
    if dimensions != SPEECH_ANALYSIS_EMBEDDING_DIMENSIONS {
        return Err(HelperError::InvalidEmbeddingDimensions(dimensions));
    }

    let mut embedded = Vec::with_capacity(turns.len());
    for (index, turn) in turns.iter().enumerate() {
        let start = f64::from(turn.start_seconds);
        let end = duration_seconds.min(f64::from(turn.end_seconds));
        if !start.is_finite()
            || !end.is_finite()
            || start < 0.0
            || end <= start
            || start >= duration_seconds
        {
            return Err(HelperError::InvalidSegment(format!(
                "diarization window {index} has invalid timing"
            )));
        }
        let (first, last) = embedding_window(start, end, samples.len(), SAMPLE_RATE);
        let Some(embedding) = extractor.embed(&samples[first..last])? else {
            continue;
        };
        if embedding.len() != SPEECH_ANALYSIS_EMBEDDING_DIMENSIONS {
            return Err(HelperError::InvalidEmbeddingDimensions(embedding.len()));
        }
        if embedding.iter().any(|value| !value.is_finite()) {
            return Err(HelperError::InvalidSegment(format!(
                "diarization window {index} has a non-finite embedding"
            )));
        }
        embedded.push((turn, start, end, l2_normalized(&embedding)));
    }

    let labels = order_of_appearance_labels(
        &embedded
            .iter()
            .map(|(turn, ..)| turn.speaker)
            .collect::<Vec<_>>(),
    );
    let mut segments = Vec::with_capacity(embedded.len());
    for ((turn, start, end, embedding), speaker_id) in embedded.iter().zip(labels) {
        let same_speaker = embedded
            .iter()
            .filter(|(other, ..)| other.speaker == turn.speaker)
            .map(|(_, _, _, vector)| vector.as_slice())
            .collect::<Vec<_>>();
        let centroid = mean_vector(&same_speaker);
        segments.push(DiarizationEmbeddingSegment {
            confidence: segment_confidence(turn.silhouette, embedding, &centroid),
            embedding: embedding.clone(),
            end_seconds: *end,
            speaker_id,
            start_seconds: *start,
        });
    }
    Ok(segments)
}
