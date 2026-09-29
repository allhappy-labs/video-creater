//! JSON contract shared with the macOS FluidAudio helper
//! (`native/fluidaudio-parakeet/Sources/VideoCreaterFluidAudioTranscribeCore`).
//!
//! Requests arrive as one JSON object on stdin. Successful responses are one JSON line on stdout;
//! failures print `{schemaVersion: 1, error}` and exit with status 1. Field order in the response
//! structs is alphabetical to match the Swift helper's `sortedKeys` encoding.

use std::fmt;

use serde::{Deserialize, Serialize};

pub const SCHEMA_VERSION: i64 = 1;
pub const TRANSCRIPTION_RUNTIME_ID: &str = "sherpa_onnx";
pub const SPEECH_ANALYSIS_RUNTIME_ID: &str = "sherpa_onnx_speech_analysis";
pub const SPEECH_ANALYSIS_MODE: &str = "speechAnalysis";
pub const SPEECH_ANALYSIS_EMBEDDING_DIMENSIONS: usize = 256;
pub const SAMPLE_RATE: u32 = 16_000;
pub const VAD_ARTIFACT_FORMAT: &str = "onnx";
pub const DIARIZATION_ARTIFACT_FORMAT: &str = "onnx";

/// Languages accepted by the macOS helper's FluidAudio `Language` hint. Parakeet TDT v3 detects
/// the spoken language itself; sherpa-onnx has no script-constrained decoding, so on Linux a valid
/// hint is accepted and decoding stays automatic.
pub const SUPPORTED_LANGUAGE_HINTS: [&str; 28] = [
    "en", "es", "fr", "de", "it", "pt", "ro", "nl", "da", "sv", "fi", "hu", "et", "lv", "lt", "mt",
    "pl", "cs", "sk", "sl", "hr", "bs", "ru", "uk", "be", "bg", "sr", "el",
];

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TranscriptionRequest {
    pub schema_version: i64,
    pub model_id: String,
    pub model_path: String,
    pub media_path: String,
    #[serde(default)]
    pub language_mode: Option<String>,
    #[serde(default)]
    pub output_artifact_path: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SpeechAnalysisRequest {
    pub schema_version: i64,
    pub mode: String,
    pub media_path: String,
    pub vad_model_id: String,
    pub vad_model_revision: String,
    pub vad_model_root: String,
    pub diarization_model_id: String,
    pub diarization_model_revision: String,
    pub diarization_model_root: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HelperRequest {
    Transcription(TranscriptionRequest),
    SpeechAnalysis(SpeechAnalysisRequest),
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Word {
    pub confidence: f64,
    pub end_seconds: f64,
    pub start_seconds: f64,
    pub text: String,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TranscriptionResponse {
    pub duration_seconds: f64,
    pub model_id: String,
    pub processing_seconds: f64,
    pub runtime_id: String,
    pub schema_version: i64,
    pub text: String,
    pub words: Vec<Word>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SpeechAnalysisModelMetadata {
    pub artifact_format: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub embedding_dimensions: Option<usize>,
    pub id: String,
    pub revision: String,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VadProbabilitySegment {
    pub end_seconds: f64,
    pub speech_probability: f64,
    pub start_seconds: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DiarizationEmbeddingSegment {
    pub confidence: f64,
    pub embedding: Vec<f64>,
    pub end_seconds: f64,
    pub speaker_id: String,
    pub start_seconds: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SpeechAnalysisResponse {
    pub diarization_model: SpeechAnalysisModelMetadata,
    pub diarization_segments: Vec<DiarizationEmbeddingSegment>,
    pub duration_seconds: f64,
    pub mode: String,
    pub processing_seconds: f64,
    pub runtime_id: String,
    pub sample_rate: u32,
    pub schema_version: i64,
    pub vad_model: SpeechAnalysisModelMetadata,
    pub vad_segments: Vec<VadProbabilitySegment>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ErrorResponse {
    pub error: String,
    pub schema_version: i64,
}

#[derive(Debug, Clone, PartialEq)]
pub enum HelperError {
    InvalidRequest(String),
    UnsupportedMode(String),
    UnsupportedTranscriptionSchemaVersion(i64),
    UnsupportedSpeechAnalysisSchemaVersion(i64),
    MissingRequestValue(&'static str),
    MissingModelDirectory(String),
    MissingModelFile(String, String),
    MissingModelRoot(String),
    MissingMediaFile(String),
    UnsupportedLanguage(String),
    InvalidDuration(f64),
    InvalidSegment(String),
    InvalidEmbeddingDimensions(usize),
    Decode(String),
    Runtime(String),
    #[cfg_attr(all(feature = "native", target_os = "linux"), allow(dead_code))]
    NativeRuntimeUnavailable,
}

impl fmt::Display for HelperError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidRequest(detail) => write!(formatter, "Invalid helper request JSON: {detail}"),
            Self::UnsupportedMode(mode) => write!(formatter, "Unsupported helper request mode '{mode}'"),
            Self::UnsupportedTranscriptionSchemaVersion(version) => write!(
                formatter,
                "Unsupported transcription request schemaVersion {version}; expected 1"
            ),
            Self::UnsupportedSpeechAnalysisSchemaVersion(version) => write!(
                formatter,
                "Unsupported speech-analysis request schemaVersion {version}; expected 1"
            ),
            Self::MissingRequestValue(name) => {
                write!(formatter, "Speech-analysis request value '{name}' must not be blank")
            }
            Self::MissingModelDirectory(path) => write!(formatter, "Model directory not found at {path}"),
            Self::MissingModelFile(name, location) => {
                write!(formatter, "Required ONNX model file {name} not found under {location}")
            }
            Self::MissingModelRoot(path) => {
                write!(formatter, "Preinstalled speech model root not found at {path}")
            }
            Self::MissingMediaFile(path) => write!(formatter, "Media file not found at {path}"),
            Self::UnsupportedLanguage(language) => {
                write!(formatter, "Unsupported Parakeet language hint '{language}'")
            }
            Self::InvalidDuration(duration) => {
                write!(formatter, "Speech-analysis duration is invalid: {duration}")
            }
            Self::InvalidSegment(message) => write!(formatter, "Speech-analysis segment is invalid: {message}"),
            Self::InvalidEmbeddingDimensions(count) => write!(
                formatter,
                "Offline diarization embedding must contain {SPEECH_ANALYSIS_EMBEDDING_DIMENSIONS} values; found {count}"
            ),
            Self::Decode(detail) => write!(formatter, "Audio decoding failed: {detail}"),
            Self::Runtime(detail) => write!(formatter, "ONNX speech runtime failed: {detail}"),
            Self::NativeRuntimeUnavailable => formatter.write_str(
                "video-creater-speech was built without the native sherpa-onnx runtime",
            ),
        }
    }
}

impl std::error::Error for HelperError {}

#[derive(Deserialize)]
struct RequestModeProbe {
    #[serde(default)]
    mode: Option<String>,
}

pub fn decode_helper_request(bytes: &[u8]) -> Result<HelperRequest, HelperError> {
    let probe: RequestModeProbe = serde_json::from_slice(bytes)
        .map_err(|error| HelperError::InvalidRequest(error.to_string()))?;
    match probe.mode.as_deref() {
        None | Some("") | Some("transcription") => serde_json::from_slice(bytes)
            .map(HelperRequest::Transcription)
            .map_err(|error| HelperError::InvalidRequest(error.to_string())),
        Some(SPEECH_ANALYSIS_MODE) => serde_json::from_slice(bytes)
            .map(HelperRequest::SpeechAnalysis)
            .map_err(|error| HelperError::InvalidRequest(error.to_string())),
        Some(mode) => Err(HelperError::UnsupportedMode(mode.to_string())),
    }
}

/// Mirrors the Swift `resolvedLanguageHint`: blank or `auto` means automatic detection, a known
/// language code is accepted, and anything else is rejected.
pub fn resolve_language_hint(mode: Option<&str>) -> Result<Option<String>, HelperError> {
    let normalized = mode
        .map(|mode| mode.trim().to_lowercase())
        .unwrap_or_default();
    if normalized.is_empty() || normalized == "auto" {
        return Ok(None);
    }
    if SUPPORTED_LANGUAGE_HINTS.contains(&normalized.as_str()) {
        Ok(Some(normalized))
    } else {
        Err(HelperError::UnsupportedLanguage(normalized))
    }
}

pub fn validate_speech_analysis_request(
    request: &SpeechAnalysisRequest,
) -> Result<(), HelperError> {
    if request.schema_version != SCHEMA_VERSION {
        return Err(HelperError::UnsupportedSpeechAnalysisSchemaVersion(
            request.schema_version,
        ));
    }
    if request.mode != SPEECH_ANALYSIS_MODE {
        return Err(HelperError::UnsupportedMode(request.mode.clone()));
    }
    for (name, value) in [
        ("mediaPath", &request.media_path),
        ("vadModelId", &request.vad_model_id),
        ("vadModelRevision", &request.vad_model_revision),
        ("vadModelRoot", &request.vad_model_root),
        ("diarizationModelId", &request.diarization_model_id),
        (
            "diarizationModelRevision",
            &request.diarization_model_revision,
        ),
        ("diarizationModelRoot", &request.diarization_model_root),
    ] {
        if value.trim().is_empty() {
            return Err(HelperError::MissingRequestValue(name));
        }
    }
    Ok(())
}

pub fn error_response(error: &HelperError) -> ErrorResponse {
    ErrorResponse {
        error: error.to_string(),
        schema_version: SCHEMA_VERSION,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decodes_transcription_request_without_mode_and_ignores_unknown_fields() {
        let request = decode_helper_request(
            br#"{"schemaVersion":1,"modelId":"nvidia/parakeet-tdt-0.6b-v3","modelPath":"/m","mediaPath":"/a.wav","languageMode":"auto","outputArtifactPath":"/t.json","extra":true}"#,
        )
        .expect("transcription request");
        let HelperRequest::Transcription(request) = request else {
            panic!("expected transcription request");
        };
        assert_eq!(request.model_id, "nvidia/parakeet-tdt-0.6b-v3");
        assert_eq!(request.language_mode.as_deref(), Some("auto"));
        assert_eq!(request.output_artifact_path.as_deref(), Some("/t.json"));
    }

    #[test]
    fn decodes_explicit_transcription_and_speech_analysis_modes() {
        assert!(matches!(
            decode_helper_request(
                br#"{"mode":"transcription","schemaVersion":1,"modelId":"m","modelPath":"/m","mediaPath":"/a"}"#
            ),
            Ok(HelperRequest::Transcription(_))
        ));
        let request = decode_helper_request(
            br#"{"schemaVersion":1,"mode":"speechAnalysis","mediaPath":"/a.wav","vadModelId":"v","vadModelRevision":"r","vadModelRoot":"/root","diarizationModelId":"d","diarizationModelRevision":"r2","diarizationModelRoot":"/root"}"#,
        )
        .expect("speech analysis request");
        let HelperRequest::SpeechAnalysis(request) = request else {
            panic!("expected speech analysis request");
        };
        validate_speech_analysis_request(&request).expect("valid request");
    }

    #[test]
    fn rejects_unknown_modes_and_blank_speech_analysis_values() {
        assert_eq!(
            decode_helper_request(br#"{"mode":"diarize"}"#),
            Err(HelperError::UnsupportedMode("diarize".to_string()))
        );
        let request = SpeechAnalysisRequest {
            schema_version: 1,
            mode: SPEECH_ANALYSIS_MODE.to_string(),
            media_path: "/a.wav".to_string(),
            vad_model_id: "v".to_string(),
            vad_model_revision: " ".to_string(),
            vad_model_root: "/root".to_string(),
            diarization_model_id: "d".to_string(),
            diarization_model_revision: "r".to_string(),
            diarization_model_root: "/root".to_string(),
        };
        assert_eq!(
            validate_speech_analysis_request(&request),
            Err(HelperError::MissingRequestValue("vadModelRevision"))
        );
        let mut wrong_version = request;
        wrong_version.schema_version = 2;
        assert_eq!(
            validate_speech_analysis_request(&wrong_version),
            Err(HelperError::UnsupportedSpeechAnalysisSchemaVersion(2))
        );
    }

    #[test]
    fn language_hints_match_the_macos_helper_contract() {
        assert_eq!(resolve_language_hint(None), Ok(None));
        assert_eq!(resolve_language_hint(Some("  AUTO ")), Ok(None));
        assert_eq!(resolve_language_hint(Some("")), Ok(None));
        assert_eq!(
            resolve_language_hint(Some("UK")),
            Ok(Some("uk".to_string()))
        );
        assert_eq!(
            resolve_language_hint(Some("tlh")),
            Err(HelperError::UnsupportedLanguage("tlh".to_string()))
        );
    }

    #[test]
    fn responses_serialize_with_sorted_camel_case_keys() {
        let response = TranscriptionResponse {
            duration_seconds: 1.5,
            model_id: "m".to_string(),
            processing_seconds: 0.25,
            runtime_id: TRANSCRIPTION_RUNTIME_ID.to_string(),
            schema_version: 1,
            text: "hi".to_string(),
            words: vec![Word {
                confidence: 0.9,
                end_seconds: 0.5,
                start_seconds: 0.1,
                text: "hi".to_string(),
            }],
        };
        assert_eq!(
            serde_json::to_string(&response).expect("json"),
            r#"{"durationSeconds":1.5,"modelId":"m","processingSeconds":0.25,"runtimeId":"sherpa_onnx","schemaVersion":1,"text":"hi","words":[{"confidence":0.9,"endSeconds":0.5,"startSeconds":0.1,"text":"hi"}]}"#
        );
        assert_eq!(
            serde_json::to_string(&error_response(&HelperError::MissingMediaFile(
                "/x".to_string()
            )))
            .expect("json"),
            r#"{"error":"Media file not found at /x","schemaVersion":1}"#
        );
    }
}
