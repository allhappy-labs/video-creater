use crate::project::model::{MediaSilenceRange, TimelineSource, VideoProject};
use crate::speech_models::{
    DIARIZATION_REPO, DIARIZATION_REVISION, SPEECH_ANALYSIS_RUNTIME_ID, VAD_REPO, VAD_REVISION,
};
use hound::{SampleFormat, WavReader};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};
use std::{
    fs,
    io::Write,
    process::{Command, Stdio},
};
use thiserror::Error;

pub const ANALYZER_VERSION: &str = "energy-zcr-vad-v1";
pub const EMBEDDING_VERSION: &str = "dsp-speaker-embedding-v1";
#[cfg(not(target_os = "linux"))]
pub const PRODUCTION_ANALYZER_VERSION: &str = "silero-vad-coreml-v6.0.0";
#[cfg(not(target_os = "linux"))]
pub const PRODUCTION_EMBEDDING_VERSION: &str = "wespeaker-vbx-coreml-256-v1";
#[cfg(target_os = "linux")]
pub const PRODUCTION_ANALYZER_VERSION: &str = "silero-vad-onnx-v6.0";
#[cfg(target_os = "linux")]
pub const PRODUCTION_EMBEDDING_VERSION: &str = "wespeaker-resnet34-lm-onnx-256-v1";
/// Production (model-backed) analyzer/embedding identities from every platform. Speech sidecars
/// live in the project folder, so a project analyzed on macOS stays usable on Linux and vice versa.
const PRODUCTION_SPEECH_VERSIONS: [(&str, &str); 2] = [
    ("silero-vad-coreml-v6.0.0", "wespeaker-vbx-coreml-256-v1"),
    ("silero-vad-onnx-v6.0", "wespeaker-resnet34-lm-onnx-256-v1"),
];

fn is_production_analyzer(analyzer_version: &str) -> bool {
    PRODUCTION_SPEECH_VERSIONS
        .iter()
        .any(|(analyzer, _)| *analyzer == analyzer_version)
}
const FRAME_MILLIS: u32 = 20;
const SPEAKER_COLORS: [&str; 6] = [
    "#38bdf8", "#f59e0b", "#a78bfa", "#34d399", "#fb7185", "#facc15",
];

#[derive(Debug, Error)]
pub enum SpeechAnalysisError {
    #[error("speech analyzer input is unavailable: {0}")]
    Unavailable(String),
    #[error("speech analyzer sidecar is invalid: {0}")]
    Invalid(String),
    #[error("speech analyzer I/O failed: {0}")]
    Io(String),
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct SpeechFrame {
    pub start_seconds: f64,
    pub end_seconds: f64,
    pub speech_probability: f64,
    pub embedding: Vec<f64>,
    pub speaker_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct SpeechRange {
    pub start_seconds: f64,
    pub end_seconds: f64,
    pub confidence: f64,
    pub speaker_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct SpeakerIdentity {
    pub id: String,
    pub name: String,
    pub color: String,
    pub centroid: Vec<f64>,
    pub sample_count: u64,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct SpeakerRegistry {
    pub schema_version: u32,
    pub analyzer_version: String,
    pub embedding_version: String,
    pub speakers: Vec<SpeakerIdentity>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct MediaSpeechSidecar {
    pub schema_version: u32,
    pub media_id: String,
    pub source_sha256: String,
    pub fingerprint: String,
    pub analyzer_version: String,
    pub embedding_version: String,
    pub sample_rate: u32,
    pub duration_seconds: f64,
    pub frames: Vec<SpeechFrame>,
    pub speech_ranges: Vec<SpeechRange>,
    pub dead_air_ranges: Vec<SpeechRange>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ProductionHelperRequest {
    schema_version: u32,
    mode: &'static str,
    media_path: String,
    vad_model_id: &'static str,
    vad_model_revision: &'static str,
    vad_model_root: String,
    diarization_model_id: &'static str,
    diarization_model_revision: &'static str,
    diarization_model_root: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ProductionHelperResponse {
    schema_version: u32,
    mode: String,
    runtime_id: String,
    sample_rate: u32,
    duration_seconds: f64,
    vad_model: HelperModelMetadata,
    diarization_model: HelperModelMetadata,
    vad_segments: Vec<HelperVadSegment>,
    diarization_segments: Vec<HelperDiarizationSegment>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct HelperModelMetadata {
    id: String,
    revision: String,
    embedding_dimensions: Option<usize>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct HelperVadSegment {
    start_seconds: f64,
    end_seconds: f64,
    speech_probability: f64,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct HelperDiarizationSegment {
    start_seconds: f64,
    end_seconds: f64,
    speaker_id: String,
    confidence: f64,
    embedding: Vec<f64>,
}

#[cfg(target_os = "linux")]
pub fn default_speech_helper_path() -> PathBuf {
    crate::transcription::sherpa_onnx::default_speech_helper_path()
}

#[cfg(not(target_os = "linux"))]
pub fn default_speech_helper_path() -> PathBuf {
    if let Ok(executable) = std::env::current_exe() {
        if let Some(parent) = executable.parent() {
            let packaged = parent.join("video-creater-fluidaudio-transcribe");
            if packaged.is_file() {
                return packaged;
            }
        }
    }
    let package = Path::new(env!("CARGO_MANIFEST_DIR")).join("native/fluidaudio-parakeet/.build");
    let release = package.join("release/video-creater-fluidaudio-transcribe");
    if release.is_file() {
        release
    } else {
        package.join("debug/video-creater-fluidaudio-transcribe")
    }
}

pub fn analyze_wav_cached_production(
    project_dir: &Path,
    media_id: &str,
    wav_path: &Path,
    helper_path: &Path,
    model_root: &Path,
) -> Result<MediaSpeechSidecar, SpeechAnalysisError> {
    let bytes =
        fs::read(wav_path).map_err(|error| SpeechAnalysisError::Unavailable(error.to_string()))?;
    let source_sha256 = format!("{:x}", Sha256::digest(&bytes));
    let fingerprint = format!("{:x}", Sha256::digest(format!(
        "{source_sha256}:{PRODUCTION_ANALYZER_VERSION}:{PRODUCTION_EMBEDDING_VERSION}:{VAD_REVISION}:{DIARIZATION_REVISION}"
    ).as_bytes()));
    let directory = project_dir
        .join("analysis/speech/v1/sha256")
        .join(&fingerprint[..2])
        .join(&fingerprint);
    let sidecar_path = directory.join("analysis.json");
    if let Ok(bytes) = fs::read(&sidecar_path) {
        if let Ok(sidecar) = serde_json::from_slice::<MediaSpeechSidecar>(&bytes) {
            if sidecar.fingerprint == fingerprint && sidecar.source_sha256 == source_sha256 {
                return Ok(sidecar);
            }
        }
    }
    if !helper_path.is_file() {
        return Err(SpeechAnalysisError::Unavailable(format!(
            "production speech helper is unavailable at {}",
            helper_path.display()
        )));
    }
    let request = ProductionHelperRequest {
        schema_version: 1,
        mode: "speechAnalysis",
        media_path: wav_path.display().to_string(),
        vad_model_id: VAD_REPO,
        vad_model_revision: VAD_REVISION,
        vad_model_root: model_root.display().to_string(),
        diarization_model_id: DIARIZATION_REPO,
        diarization_model_revision: DIARIZATION_REVISION,
        diarization_model_root: model_root.display().to_string(),
    };
    let mut child = Command::new(helper_path)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|error| SpeechAnalysisError::Unavailable(error.to_string()))?;
    child
        .stdin
        .take()
        .ok_or_else(|| SpeechAnalysisError::Unavailable("speech helper stdin unavailable".into()))?
        .write_all(
            &serde_json::to_vec(&request)
                .map_err(|error| SpeechAnalysisError::Invalid(error.to_string()))?,
        )
        .map_err(|error| SpeechAnalysisError::Io(error.to_string()))?;
    let output = child
        .wait_with_output()
        .map_err(|error| SpeechAnalysisError::Io(error.to_string()))?;
    if !output.status.success() {
        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(SpeechAnalysisError::Unavailable(format!(
            "production speech helper failed: {}{}",
            stdout.trim(),
            if stderr.trim().is_empty() {
                String::new()
            } else {
                format!("; {}", stderr.trim())
            }
        )));
    }
    let response: ProductionHelperResponse =
        serde_json::from_slice(&output.stdout).map_err(|error| {
            SpeechAnalysisError::Invalid(format!("speech helper response: {error}"))
        })?;
    validate_production_response(&response)?;
    let mut registry = load_speaker_registry(project_dir)?;
    if registry.embedding_version != PRODUCTION_EMBEDDING_VERSION {
        registry = SpeakerRegistry {
            schema_version: 1,
            analyzer_version: PRODUCTION_ANALYZER_VERSION.to_string(),
            embedding_version: PRODUCTION_EMBEDDING_VERSION.to_string(),
            speakers: Vec::new(),
        };
    }
    let mut local_speakers = std::collections::BTreeMap::<String, Vec<Vec<f64>>>::new();
    for segment in &response.diarization_segments {
        local_speakers
            .entry(segment.speaker_id.clone())
            .or_default()
            .push(segment.embedding.clone());
    }
    let mut speaker_ids = std::collections::BTreeMap::new();
    for (local_id, embeddings) in local_speakers {
        let mut centroid = vec![0.0; 256];
        for embedding in &embeddings {
            for (value, sample) in centroid.iter_mut().zip(embedding) {
                *value += *sample;
            }
        }
        for value in &mut centroid {
            *value /= embeddings.len() as f64;
        }
        let centroid = normalize(centroid);
        if let Some(id) = assign_embedding(&mut registry, &centroid) {
            speaker_ids.insert(local_id, id);
        }
    }
    let frames = response
        .vad_segments
        .iter()
        .map(|vad| {
            let diarization = response
                .diarization_segments
                .iter()
                .filter(|segment| {
                    segment.start_seconds < vad.end_seconds
                        && segment.end_seconds > vad.start_seconds
                })
                .max_by(|left, right| left.confidence.total_cmp(&right.confidence));
            SpeechFrame {
                start_seconds: round(vad.start_seconds),
                end_seconds: round(vad.end_seconds),
                speech_probability: round(vad.speech_probability),
                embedding: diarization
                    .map(|segment| segment.embedding.clone())
                    .unwrap_or_default(),
                speaker_id: diarization
                    .and_then(|segment| speaker_ids.get(&segment.speaker_id).cloned()),
            }
        })
        .collect::<Vec<_>>();
    let sidecar = MediaSpeechSidecar {
        schema_version: 1,
        media_id: media_id.to_string(),
        source_sha256,
        fingerprint,
        analyzer_version: PRODUCTION_ANALYZER_VERSION.to_string(),
        embedding_version: PRODUCTION_EMBEDDING_VERSION.to_string(),
        sample_rate: response.sample_rate,
        duration_seconds: response.duration_seconds,
        speech_ranges: merge_ranges(&frames, true),
        dead_air_ranges: merge_ranges(&frames, false)
            .into_iter()
            .filter(|range| range.end_seconds - range.start_seconds >= 0.5)
            .collect(),
        frames,
    };
    fs::create_dir_all(&directory).map_err(|error| SpeechAnalysisError::Io(error.to_string()))?;
    atomic_json(&sidecar_path, &sidecar)?;
    save_speaker_registry(project_dir, &registry)?;
    Ok(sidecar)
}

fn validate_production_response(
    response: &ProductionHelperResponse,
) -> Result<(), SpeechAnalysisError> {
    if response.schema_version != 1
        || response.mode != "speechAnalysis"
        || response.runtime_id != SPEECH_ANALYSIS_RUNTIME_ID
        || response.sample_rate != 16_000
        || !response.duration_seconds.is_finite()
        || response.duration_seconds <= 0.0
        || response.vad_model.id != VAD_REPO
        || response.vad_model.revision != VAD_REVISION
        || response.diarization_model.id != DIARIZATION_REPO
        || response.diarization_model.revision != DIARIZATION_REVISION
        || response.diarization_model.embedding_dimensions != Some(256)
    {
        return Err(SpeechAnalysisError::Invalid(
            "production speech helper metadata mismatch".to_string(),
        ));
    }
    let invalid_vad = response.vad_segments.iter().any(|segment| {
        !segment.start_seconds.is_finite()
            || !segment.end_seconds.is_finite()
            || segment.start_seconds < 0.0
            || segment.end_seconds <= segment.start_seconds
            || segment.end_seconds > response.duration_seconds + 1e-6
            || !segment.speech_probability.is_finite()
            || !(0.0..=1.0).contains(&segment.speech_probability)
    });
    let invalid_diarization = response.diarization_segments.iter().any(|segment| {
        !segment.start_seconds.is_finite()
            || !segment.end_seconds.is_finite()
            || segment.start_seconds < 0.0
            || segment.end_seconds <= segment.start_seconds
            || segment.embedding.len() != 256
            || segment.embedding.iter().any(|value| !value.is_finite())
            || !segment.confidence.is_finite()
            || !(0.0..=1.0).contains(&segment.confidence)
    });
    if invalid_vad || invalid_diarization {
        return Err(SpeechAnalysisError::Invalid(
            "production speech helper returned invalid segments".to_string(),
        ));
    }
    Ok(())
}

pub fn analyze_wav_cached(
    project_dir: &Path,
    media_id: &str,
    wav_path: &Path,
) -> Result<MediaSpeechSidecar, SpeechAnalysisError> {
    let bytes =
        fs::read(wav_path).map_err(|error| SpeechAnalysisError::Unavailable(error.to_string()))?;
    let source_sha256 = format!("{:x}", Sha256::digest(&bytes));
    let fingerprint = format!(
        "{:x}",
        Sha256::digest(
            format!("{source_sha256}:{ANALYZER_VERSION}:{EMBEDDING_VERSION}").as_bytes()
        )
    );
    let directory = project_dir
        .join("analysis/speech/v1/sha256")
        .join(&fingerprint[..2])
        .join(&fingerprint);
    let sidecar_path = directory.join("analysis.json");
    if let Ok(sidecar) = fs::read(&sidecar_path)
        .ok()
        .and_then(|bytes| serde_json::from_slice::<MediaSpeechSidecar>(&bytes).ok())
        .ok_or(())
    {
        if sidecar.fingerprint == fingerprint && sidecar.source_sha256 == source_sha256 {
            return Ok(sidecar);
        }
    }
    let (samples, sample_rate) = read_mono_wav(wav_path)?;
    let mut registry = load_speaker_registry(project_dir)?;
    let sidecar = analyze_pcm(
        media_id,
        &source_sha256,
        &fingerprint,
        &samples,
        sample_rate,
        &mut registry,
    )?;
    fs::create_dir_all(&directory).map_err(|error| SpeechAnalysisError::Io(error.to_string()))?;
    atomic_json(&sidecar_path, &sidecar)?;
    save_speaker_registry(project_dir, &registry)?;
    Ok(sidecar)
}

pub fn load_speaker_registry(project_dir: &Path) -> Result<SpeakerRegistry, SpeechAnalysisError> {
    let path = registry_path(project_dir);
    if !path.exists() {
        return Ok(SpeakerRegistry {
            schema_version: 1,
            analyzer_version: ANALYZER_VERSION.to_string(),
            embedding_version: EMBEDDING_VERSION.to_string(),
            speakers: Vec::new(),
        });
    }
    serde_json::from_slice(
        &fs::read(path).map_err(|error| SpeechAnalysisError::Io(error.to_string()))?,
    )
    .map_err(|error| SpeechAnalysisError::Invalid(error.to_string()))
}

pub fn save_speaker_registry(
    project_dir: &Path,
    registry: &SpeakerRegistry,
) -> Result<(), SpeechAnalysisError> {
    atomic_json(&registry_path(project_dir), registry)
}

pub fn rename_speaker(
    project_dir: &Path,
    speaker_id: &str,
    name: &str,
) -> Result<SpeakerRegistry, SpeechAnalysisError> {
    if name.trim().is_empty() {
        return Err(SpeechAnalysisError::Invalid(
            "speaker name must not be blank".to_string(),
        ));
    }
    update_speaker(project_dir, speaker_id, |speaker| {
        speaker.name = name.trim().to_string()
    })
}

pub fn recolor_speaker(
    project_dir: &Path,
    speaker_id: &str,
    color: &str,
) -> Result<SpeakerRegistry, SpeechAnalysisError> {
    if !valid_color(color) {
        return Err(SpeechAnalysisError::Invalid(
            "speaker color must be #RRGGBB".to_string(),
        ));
    }
    update_speaker(project_dir, speaker_id, |speaker| {
        speaker.color = color.to_ascii_lowercase()
    })
}

pub fn assign_media_speaker(
    project_dir: &Path,
    fingerprint: &str,
    start_seconds: f64,
    end_seconds: f64,
    speaker_id: &str,
) -> Result<MediaSpeechSidecar, SpeechAnalysisError> {
    let path = project_dir
        .join("analysis/speech/v1/sha256")
        .join(&fingerprint[..2])
        .join(fingerprint)
        .join("analysis.json");
    let mut sidecar: MediaSpeechSidecar = serde_json::from_slice(
        &fs::read(&path).map_err(|error| SpeechAnalysisError::Io(error.to_string()))?,
    )
    .map_err(|error| SpeechAnalysisError::Invalid(error.to_string()))?;
    if !start_seconds.is_finite() || !end_seconds.is_finite() || end_seconds <= start_seconds {
        return Err(SpeechAnalysisError::Invalid(
            "speaker assignment range is invalid".to_string(),
        ));
    }
    let registry = load_speaker_registry(project_dir)?;
    if !registry
        .speakers
        .iter()
        .any(|speaker| speaker.id == speaker_id)
    {
        return Err(SpeechAnalysisError::Invalid(
            "speaker id was not found".to_string(),
        ));
    }
    for frame in &mut sidecar.frames {
        if frame.start_seconds < end_seconds && frame.end_seconds > start_seconds {
            frame.speaker_id = Some(speaker_id.to_string());
        }
    }
    sidecar.speech_ranges = merge_ranges(&sidecar.frames, true);
    atomic_json(&path, &sidecar)?;
    Ok(sidecar)
}

pub fn project_persisted_speech_analysis(
    project_dir: &Path,
    project: &mut VideoProject,
) -> Result<usize, SpeechAnalysisError> {
    let registry = load_speaker_registry(project_dir)?;
    let root = project_dir.join("analysis/speech/v1/sha256");
    if !root.exists() {
        return Ok(0);
    }
    let mut sidecars = Vec::new();
    for prefix in fs::read_dir(&root).map_err(|error| SpeechAnalysisError::Io(error.to_string()))? {
        let prefix = prefix
            .map_err(|error| SpeechAnalysisError::Io(error.to_string()))?
            .path();
        if !prefix.is_dir() {
            continue;
        }
        for entry in
            fs::read_dir(prefix).map_err(|error| SpeechAnalysisError::Io(error.to_string()))?
        {
            let path = entry
                .map_err(|error| SpeechAnalysisError::Io(error.to_string()))?
                .path()
                .join("analysis.json");
            if !path.is_file() {
                continue;
            }
            let sidecar: MediaSpeechSidecar = serde_json::from_slice(
                &fs::read(path).map_err(|error| SpeechAnalysisError::Io(error.to_string()))?,
            )
            .map_err(|error| SpeechAnalysisError::Invalid(error.to_string()))?;
            let supported = (sidecar.analyzer_version == ANALYZER_VERSION
                && sidecar.embedding_version == EMBEDDING_VERSION)
                || PRODUCTION_SPEECH_VERSIONS
                    .iter()
                    .any(|(analyzer, embedding)| {
                        sidecar.analyzer_version == *analyzer
                            && sidecar.embedding_version == *embedding
                    });
            if !supported {
                continue;
            }
            sidecars.push(sidecar);
        }
    }
    sidecars.sort_by(|left, right| {
        left.media_id
            .cmp(&right.media_id)
            .then_with(|| {
                is_production_analyzer(&left.analyzer_version)
                    .cmp(&is_production_analyzer(&right.analyzer_version))
            })
            .then_with(|| left.fingerprint.cmp(&right.fingerprint))
    });
    let mut selected = std::collections::BTreeMap::new();
    for sidecar in sidecars {
        selected.insert(sidecar.media_id.clone(), sidecar);
    }
    for sidecar in selected.values() {
        project_sidecar(project, sidecar, &registry);
    }
    Ok(selected.len())
}

pub fn project_sidecar(
    project: &mut VideoProject,
    sidecar: &MediaSpeechSidecar,
    registry: &SpeakerRegistry,
) {
    project
        .media_silence_ranges
        .retain(|range| range.media_id != sidecar.media_id);
    project
        .media_silence_ranges
        .extend(
            sidecar
                .dead_air_ranges
                .iter()
                .map(|range| MediaSilenceRange {
                    media_id: sidecar.media_id.clone(),
                    source_in: range.start_seconds,
                    source_out: range.end_seconds,
                    confidence: range.confidence,
                    label: if is_production_analyzer(&sidecar.analyzer_version) {
                        "Silero VAD dead air".to_string()
                    } else {
                        "heuristic VAD dead air".to_string()
                    },
                }),
        );
    for track in &mut project.timeline.tracks {
        for item in &mut track.items {
            let TimelineSource::Media { media_id } = &item.source else {
                continue;
            };
            if media_id != &sidecar.media_id {
                continue;
            }
            let source_in = item
                .properties
                .get("sourceIn")
                .and_then(serde_json::Value::as_f64)
                .unwrap_or(0.0);
            let source_out = item
                .properties
                .get("sourceOut")
                .and_then(serde_json::Value::as_f64)
                .unwrap_or(source_in + item.duration_seconds);
            let span = source_out - source_in;
            if !span.is_finite() || span <= 0.0 {
                continue;
            }
            let mask = sidecar
                .speech_ranges
                .iter()
                .filter_map(|range| {
                    let start = range.start_seconds.max(source_in);
                    let end = range.end_seconds.min(source_out);
                    if end <= start {
                        return None;
                    }
                    let speaker = range
                        .speaker_id
                        .as_deref()
                        .and_then(|id| registry.speakers.iter().find(|speaker| speaker.id == id));
                    Some(serde_json::json!({
                        "startRatio": (start-source_in)/span, "endRatio": (end-source_in)/span,
                        "probability": range.confidence, "speakerId": range.speaker_id,
                        "color": speaker.map(|speaker| speaker.color.as_str()),
                    }))
                })
                .collect::<Vec<_>>();
            let dead_air = sidecar.dead_air_ranges.iter().filter_map(|range| {
                let start=range.start_seconds.max(source_in); let end=range.end_seconds.min(source_out);
                (end>start).then(|| serde_json::json!({"startRatio":(start-source_in)/span,"endRatio":(end-source_in)/span,"confidence":range.confidence}))
            }).collect::<Vec<_>>();
            item.properties
                .insert("speechMask".to_string(), serde_json::Value::Array(mask));
            item.properties.insert(
                "deadAirMask".to_string(),
                serde_json::Value::Array(dead_air),
            );
            let quality = if is_production_analyzer(&sidecar.analyzer_version) {
                "production"
            } else {
                "heuristic"
            };
            item.properties.insert("speechAnalysis".to_string(), serde_json::json!({"status":"ready","quality":quality,"fingerprint":sidecar.fingerprint,"analyzerVersion":sidecar.analyzer_version,"embeddingVersion":sidecar.embedding_version}));
        }
    }
}

pub fn analyze_pcm(
    media_id: &str,
    source_sha256: &str,
    fingerprint: &str,
    samples: &[f32],
    sample_rate: u32,
    registry: &mut SpeakerRegistry,
) -> Result<MediaSpeechSidecar, SpeechAnalysisError> {
    if samples.is_empty() || sample_rate < 8_000 {
        return Err(SpeechAnalysisError::Unavailable(
            "prepared PCM must contain mono samples at 8 kHz or higher".to_string(),
        ));
    }
    let frame_samples = (u64::from(sample_rate) * u64::from(FRAME_MILLIS) / 1000) as usize;
    let metrics = samples
        .chunks(frame_samples)
        .map(frame_metrics)
        .collect::<Vec<_>>();
    let mut energies = metrics.iter().map(|metric| metric.0).collect::<Vec<_>>();
    energies.sort_by(f64::total_cmp);
    let noise_floor = energies[(energies.len() / 5).min(energies.len() - 1)].max(1e-7);
    let mut frames = Vec::with_capacity(metrics.len());
    for (index, (energy, zcr, tilt)) in metrics.into_iter().enumerate() {
        let snr = (energy / noise_floor).log10().max(0.0);
        let probability = ((snr - 0.18) * 2.5).clamp(0.0, 1.0);
        let embedding = normalize(vec![energy.max(1e-9).log10(), zcr, tilt]);
        let speaker_id = (probability >= 0.55)
            .then(|| assign_embedding(registry, &embedding))
            .flatten();
        frames.push(SpeechFrame {
            start_seconds: round(index as f64 * frame_samples as f64 / f64::from(sample_rate)),
            end_seconds: round(
                ((index + 1) * frame_samples).min(samples.len()) as f64 / f64::from(sample_rate),
            ),
            speech_probability: round(probability),
            embedding,
            speaker_id,
        });
    }
    Ok(MediaSpeechSidecar {
        schema_version: 1,
        media_id: media_id.to_string(),
        source_sha256: source_sha256.to_string(),
        fingerprint: fingerprint.to_string(),
        analyzer_version: ANALYZER_VERSION.to_string(),
        embedding_version: EMBEDDING_VERSION.to_string(),
        sample_rate,
        duration_seconds: samples.len() as f64 / f64::from(sample_rate),
        speech_ranges: merge_ranges(&frames, true),
        dead_air_ranges: merge_ranges(&frames, false)
            .into_iter()
            .filter(|range| range.end_seconds - range.start_seconds >= 0.5)
            .collect(),
        frames,
    })
}

fn frame_metrics(frame: &[f32]) -> (f64, f64, f64) {
    let energy = (frame
        .iter()
        .map(|sample| f64::from(*sample).powi(2))
        .sum::<f64>()
        / frame.len().max(1) as f64)
        .sqrt();
    let crossings = frame
        .windows(2)
        .filter(|pair| pair[0].is_sign_positive() != pair[1].is_sign_positive())
        .count();
    let zcr = crossings as f64 / frame.len().max(1) as f64;
    let difference = frame
        .windows(2)
        .map(|pair| f64::from((pair[1] - pair[0]).abs()))
        .sum::<f64>()
        / frame.len().max(1) as f64;
    (energy, zcr, difference / (energy + 1e-9))
}

fn assign_embedding(registry: &mut SpeakerRegistry, embedding: &[f64]) -> Option<String> {
    if embedding.is_empty()
        || registry
            .speakers
            .iter()
            .any(|speaker| speaker.centroid.len() != embedding.len())
    {
        registry.speakers.clear();
    }
    let nearest = registry
        .speakers
        .iter()
        .enumerate()
        .map(|(index, speaker)| (index, distance(&speaker.centroid, embedding)))
        .min_by(|left, right| left.1.total_cmp(&right.1));
    let index = match nearest {
        Some((index, distance)) if distance <= 0.08 => index,
        _ => {
            let index = registry.speakers.len();
            registry.speakers.push(SpeakerIdentity {
                id: format!("speaker-{}", index + 1),
                name: format!("Speaker {}", index + 1),
                color: SPEAKER_COLORS[index % SPEAKER_COLORS.len()].to_string(),
                centroid: embedding.to_vec(),
                sample_count: 0,
            });
            index
        }
    };
    let speaker = &mut registry.speakers[index];
    speaker.sample_count += 1;
    let count = speaker.sample_count as f64;
    for (centroid, value) in speaker.centroid.iter_mut().zip(embedding) {
        *centroid += (*value - *centroid) / count;
    }
    Some(speaker.id.clone())
}

fn merge_ranges(frames: &[SpeechFrame], speech: bool) -> Vec<SpeechRange> {
    let mut ranges: Vec<SpeechRange> = Vec::new();
    for frame in frames
        .iter()
        .filter(|frame| (frame.speech_probability >= 0.55) == speech)
    {
        let speaker_id = if speech {
            frame.speaker_id.clone()
        } else {
            None
        };
        if let Some(last) = ranges.last_mut() {
            if (last.end_seconds - frame.start_seconds).abs() < 1e-6
                && last.speaker_id == speaker_id
            {
                last.end_seconds = frame.end_seconds;
                last.confidence = last.confidence.max(if speech {
                    frame.speech_probability
                } else {
                    1.0 - frame.speech_probability
                });
                continue;
            }
        }
        ranges.push(SpeechRange {
            start_seconds: frame.start_seconds,
            end_seconds: frame.end_seconds,
            confidence: if speech {
                frame.speech_probability
            } else {
                1.0 - frame.speech_probability
            },
            speaker_id,
        });
    }
    ranges
}

fn read_mono_wav(path: &Path) -> Result<(Vec<f32>, u32), SpeechAnalysisError> {
    let mut reader = WavReader::open(path)
        .map_err(|error| SpeechAnalysisError::Unavailable(error.to_string()))?;
    let spec = reader.spec();
    if spec.channels == 0 {
        return Err(SpeechAnalysisError::Invalid(
            "WAV channels are invalid".to_string(),
        ));
    }
    let interleaved = match (spec.sample_format, spec.bits_per_sample) {
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
    .map_err(|error| SpeechAnalysisError::Invalid(error.to_string()))?;
    let channels = usize::from(spec.channels);
    Ok((
        interleaved
            .chunks(channels)
            .map(|frame| frame.iter().copied().sum::<f32>() / channels as f32)
            .collect(),
        spec.sample_rate,
    ))
}

fn normalize(mut values: Vec<f64>) -> Vec<f64> {
    let norm = values
        .iter()
        .map(|value| value * value)
        .sum::<f64>()
        .sqrt()
        .max(1e-9);
    for value in &mut values {
        *value /= norm;
    }
    values
}
fn distance(left: &[f64], right: &[f64]) -> f64 {
    if left.len() != right.len() || left.is_empty() {
        return f64::INFINITY;
    }
    left.iter()
        .zip(right)
        .map(|(left, right)| (left - right).powi(2))
        .sum::<f64>()
        .sqrt()
}
fn update_speaker(
    project_dir: &Path,
    speaker_id: &str,
    update: impl FnOnce(&mut SpeakerIdentity),
) -> Result<SpeakerRegistry, SpeechAnalysisError> {
    let mut registry = load_speaker_registry(project_dir)?;
    let speaker = registry
        .speakers
        .iter_mut()
        .find(|speaker| speaker.id == speaker_id)
        .ok_or_else(|| SpeechAnalysisError::Invalid("speaker id was not found".to_string()))?;
    update(speaker);
    save_speaker_registry(project_dir, &registry)?;
    Ok(registry)
}
fn registry_path(project_dir: &Path) -> PathBuf {
    project_dir.join("analysis/speech/v1/speakers.json")
}
fn valid_color(color: &str) -> bool {
    color.len() == 7
        && color.starts_with('#')
        && color[1..]
            .chars()
            .all(|character| character.is_ascii_hexdigit())
}
fn atomic_json(path: &Path, value: &impl Serialize) -> Result<(), SpeechAnalysisError> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|error| SpeechAnalysisError::Io(error.to_string()))?;
    }
    let temporary = path.with_extension("tmp");
    fs::write(
        &temporary,
        serde_json::to_vec_pretty(value)
            .map_err(|error| SpeechAnalysisError::Invalid(error.to_string()))?,
    )
    .map_err(|error| SpeechAnalysisError::Io(error.to_string()))?;
    fs::rename(temporary, path).map_err(|error| SpeechAnalysisError::Io(error.to_string()))
}
fn round(value: f64) -> f64 {
    (value * 1_000_000.0).round() / 1_000_000.0
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::project::fixtures::sample_project;
    #[test]
    fn two_speakers_and_silence_are_deterministic_without_quality_overclaim() {
        let sample_rate = 16_000;
        let tone = |frequency: f32, seconds: f32| {
            (0..(sample_rate as f32 * seconds) as usize)
                .map(|index| {
                    (index as f32 * frequency * std::f32::consts::TAU / sample_rate as f32).sin()
                        * 0.5
                })
                .collect::<Vec<_>>()
        };
        let mut samples = tone(180.0, 1.0);
        samples.extend(vec![0.0001; 16_000]);
        samples.extend(tone(620.0, 1.0));
        let mut registry = SpeakerRegistry::default();
        let first = analyze_pcm(
            "media-1",
            "source",
            "fingerprint",
            &samples,
            sample_rate,
            &mut registry,
        )
        .expect("analysis");
        let mut second_registry = SpeakerRegistry::default();
        let second = analyze_pcm(
            "media-1",
            "source",
            "fingerprint",
            &samples,
            sample_rate,
            &mut second_registry,
        )
        .expect("analysis");
        assert_eq!(first, second);
        assert!(!first.dead_air_ranges.is_empty());
        assert!(first
            .dead_air_ranges
            .iter()
            .any(|range| range.start_seconds <= 1.1 && range.end_seconds >= 1.9));
        let speakers = first
            .speech_ranges
            .iter()
            .filter_map(|range| range.speaker_id.as_deref())
            .collect::<std::collections::BTreeSet<_>>();
        assert!(
            speakers.len() >= 2,
            "heuristic should separate distinct tone fixtures"
        );
    }

    /// Real end-to-end speech analysis through the app code path: downloads and verifies the
    /// pinned ONNX model set, then analyzes a clip with speech, a 3 s digital-silence gap, and
    /// speech again (`VIDEO_CREATER_LINUX_SPEECH_E2E_CLIP`, 16-bit PCM WAV).
    #[cfg(target_os = "linux")]
    #[test]
    #[ignore = "downloads ~35 MB of models and needs the staged video-creater-speech helper"]
    fn real_onnx_speech_analysis_detects_speech_silence_and_speaker_embeddings() {
        let root = PathBuf::from(
            std::env::var_os("VIDEO_CREATER_LINUX_SPEECH_E2E_ROOT")
                .expect("set VIDEO_CREATER_LINUX_SPEECH_E2E_ROOT"),
        );
        let clip = PathBuf::from(
            std::env::var_os("VIDEO_CREATER_LINUX_SPEECH_E2E_CLIP")
                .expect("set VIDEO_CREATER_LINUX_SPEECH_E2E_CLIP"),
        );
        let store = crate::speech_models::ProductionSpeechModelStore::new(root.join("models"));
        let status = store
            .download_and_verify()
            .expect("download pinned speech models");
        assert!(status.ready);
        let project = tempfile::tempdir().expect("project dir");
        let sidecar = analyze_wav_cached_production(
            project.path(),
            "clip",
            &clip,
            &default_speech_helper_path(),
            store.root(),
        )
        .expect("real speech analysis");
        println!(
            "speech ranges: {:?}\ndead air: {:?}",
            sidecar.speech_ranges, sidecar.dead_air_ranges
        );
        assert_eq!(sidecar.analyzer_version, PRODUCTION_ANALYZER_VERSION);
        assert!(!sidecar.speech_ranges.is_empty());
        assert!(sidecar
            .frames
            .iter()
            .any(|frame| frame.embedding.len() == 256 && frame.speaker_id.is_some()));
        assert!(sidecar
            .dead_air_ranges
            .iter()
            .any(|range| range.start_seconds <= 4.4 && range.end_seconds >= 6.4));
        let registry = load_speaker_registry(project.path()).expect("speaker registry");
        assert_eq!(registry.embedding_version, PRODUCTION_EMBEDDING_VERSION);
        assert!(!registry.speakers.is_empty());
    }

    #[test]
    fn registry_rename_color_and_assignment_persist() {
        let temporary = tempfile::tempdir().expect("registry tempdir");
        let registry = SpeakerRegistry {
            schema_version: 1,
            analyzer_version: ANALYZER_VERSION.to_string(),
            embedding_version: EMBEDDING_VERSION.to_string(),
            speakers: vec![SpeakerIdentity {
                id: "speaker-1".to_string(),
                name: "Speaker 1".to_string(),
                color: "#38bdf8".to_string(),
                centroid: vec![1.0, 0.0, 0.0],
                sample_count: 3,
            }],
        };
        save_speaker_registry(temporary.path(), &registry).expect("save");
        rename_speaker(temporary.path(), "speaker-1", "Host").expect("rename");
        let updated = recolor_speaker(temporary.path(), "speaker-1", "#ABCDEF").expect("color");
        assert_eq!(updated.speakers[0].name, "Host");
        assert_eq!(updated.speakers[0].color, "#abcdef");
        assert_eq!(
            load_speaker_registry(temporary.path()).expect("reload"),
            updated
        );
    }

    #[test]
    fn persisted_sidecar_projects_masks_silence_and_speaker_color_on_reload() {
        let temporary = tempfile::tempdir().expect("projection tempdir");
        let registry = SpeakerRegistry {
            schema_version: 1,
            analyzer_version: ANALYZER_VERSION.to_string(),
            embedding_version: EMBEDDING_VERSION.to_string(),
            speakers: vec![SpeakerIdentity {
                id: "speaker-1".to_string(),
                name: "Host".to_string(),
                color: "#38bdf8".to_string(),
                centroid: vec![1.0, 0.0, 0.0],
                sample_count: 2,
            }],
        };
        save_speaker_registry(temporary.path(), &registry).expect("registry");
        let sidecar = MediaSpeechSidecar {
            schema_version: 1,
            media_id: "media-1".to_string(),
            source_sha256: "source".to_string(),
            fingerprint: "aa00000000000000000000000000000000000000000000000000000000000000"
                .to_string(),
            analyzer_version: ANALYZER_VERSION.to_string(),
            embedding_version: EMBEDDING_VERSION.to_string(),
            sample_rate: 16_000,
            duration_seconds: 4.0,
            frames: Vec::new(),
            speech_ranges: vec![SpeechRange {
                start_seconds: 0.0,
                end_seconds: 1.0,
                confidence: 0.9,
                speaker_id: Some("speaker-1".to_string()),
            }],
            dead_air_ranges: vec![SpeechRange {
                start_seconds: 1.0,
                end_seconds: 2.0,
                confidence: 0.95,
                speaker_id: None,
            }],
        };
        let directory = temporary
            .path()
            .join("analysis/speech/v1/sha256/aa")
            .join(&sidecar.fingerprint);
        atomic_json(&directory.join("analysis.json"), &sidecar).expect("sidecar");
        let mut project = sample_project();
        project_persisted_speech_analysis(temporary.path(), &mut project).expect("project");
        assert_eq!(project.media_silence_ranges.len(), 1);
        let item = &project.timeline.tracks[0].items[0];
        assert_eq!(item.properties["speechMask"][0]["speakerId"], "speaker-1");
        assert_eq!(item.properties["speechMask"][0]["color"], "#38bdf8");
        assert_eq!(item.properties["deadAirMask"][0]["confidence"], 0.95);
        assert_eq!(item.properties["speechAnalysis"]["quality"], "heuristic");
    }
}
