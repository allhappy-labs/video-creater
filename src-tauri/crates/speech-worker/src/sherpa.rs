//! Minimal RAII wrappers over the sherpa-onnx C API (TTS-disabled source build).

use std::ffi::{CStr, CString};
use std::path::Path;

use sherpa_onnx_sys as sys;

use crate::protocol::{HelperError, SAMPLE_RATE};
use crate::words::OfflineRecognizerJson;

fn c_path(path: &Path) -> Result<CString, HelperError> {
    CString::new(path.to_string_lossy().as_bytes())
        .map_err(|_| HelperError::Runtime(format!("path contains NUL: {}", path.display())))
}

fn c_string(value: &str) -> CString {
    CString::new(value).unwrap_or_default()
}

pub fn inference_threads() -> i32 {
    std::thread::available_parallelism()
        .map(|threads| threads.get().min(4))
        .unwrap_or(2) as i32
}

pub struct ParakeetModelFiles<'a> {
    pub encoder: &'a Path,
    pub decoder: &'a Path,
    pub joiner: &'a Path,
    pub tokens: &'a Path,
}

pub struct OfflineRecognizer {
    raw: *const sys::offline_asr::OfflineRecognizer,
}

impl OfflineRecognizer {
    pub fn parakeet_tdt(files: &ParakeetModelFiles<'_>) -> Result<Self, HelperError> {
        let encoder = c_path(files.encoder)?;
        let decoder = c_path(files.decoder)?;
        let joiner = c_path(files.joiner)?;
        let tokens = c_path(files.tokens)?;
        let provider = c_string("cpu");
        let model_type = c_string("nemo_transducer");
        let decoding = c_string("greedy_search");
        // SAFETY: every config field is a plain integer, float, or nullable C string pointer, and
        // the C API treats zero/NULL as "unset" (sherpa-onnx's own C examples memset to zero).
        let mut config: sys::offline_asr::OfflineRecognizerConfig = unsafe { std::mem::zeroed() };
        config.feat_config.sample_rate = SAMPLE_RATE as i32;
        config.feat_config.feature_dim = 80;
        config.model_config.transducer.encoder = encoder.as_ptr();
        config.model_config.transducer.decoder = decoder.as_ptr();
        config.model_config.transducer.joiner = joiner.as_ptr();
        config.model_config.tokens = tokens.as_ptr();
        config.model_config.num_threads = inference_threads();
        config.model_config.provider = provider.as_ptr();
        config.model_config.model_type = model_type.as_ptr();
        config.decoding_method = decoding.as_ptr();
        config.max_active_paths = 4;
        // SAFETY: `config` and the CStrings it points to outlive the call; sherpa copies strings.
        let raw = unsafe { sys::offline_asr::SherpaOnnxCreateOfflineRecognizer(&config) };
        if raw.is_null() {
            return Err(HelperError::Runtime(
                "sherpa-onnx could not load the Parakeet TDT ONNX model".to_string(),
            ));
        }
        Ok(Self { raw })
    }

    pub fn decode(&self, samples: &[f32]) -> Result<OfflineRecognizerJson, HelperError> {
        let length = i32::try_from(samples.len())
            .map_err(|_| HelperError::Runtime("audio window is too long".to_string()))?;
        // SAFETY: `self.raw` is a live recognizer; the stream is destroyed before returning and the
        // sample slice outlives `AcceptWaveformOffline`, which copies the samples.
        unsafe {
            let stream = sys::offline_asr::SherpaOnnxCreateOfflineStream(self.raw);
            if stream.is_null() {
                return Err(HelperError::Runtime(
                    "could not create an offline stream".to_string(),
                ));
            }
            sys::offline_asr::SherpaOnnxAcceptWaveformOffline(
                stream,
                SAMPLE_RATE as i32,
                samples.as_ptr(),
                length,
            );
            sys::offline_asr::SherpaOnnxDecodeOfflineStream(self.raw, stream);
            let json = sys::offline_asr::SherpaOnnxGetOfflineStreamResultAsJson(stream);
            let parsed = if json.is_null() {
                Err(HelperError::Runtime(
                    "recognizer returned no result".to_string(),
                ))
            } else {
                let text = CStr::from_ptr(json).to_string_lossy().into_owned();
                sys::offline_asr::SherpaOnnxDestroyOfflineStreamResultJson(json);
                serde_json::from_str::<OfflineRecognizerJson>(&text).map_err(|error| {
                    HelperError::Runtime(format!("invalid recognizer JSON: {error}"))
                })
            };
            sys::offline_asr::SherpaOnnxDestroyOfflineStream(stream);
            parsed
        }
    }
}

impl Drop for OfflineRecognizer {
    fn drop(&mut self) {
        // SAFETY: created by SherpaOnnxCreateOfflineRecognizer and destroyed exactly once.
        unsafe { sys::offline_asr::SherpaOnnxDestroyOfflineRecognizer(self.raw) }
    }
}

pub struct SpeakerEmbeddingExtractor {
    raw: *const sys::speaker_embedding::SpeakerEmbeddingExtractor,
}

impl SpeakerEmbeddingExtractor {
    pub fn new(model: &Path) -> Result<Self, HelperError> {
        let model = c_path(model)?;
        let provider = c_string("cpu");
        let config = sys::speaker_embedding::SpeakerEmbeddingExtractorConfig {
            model: model.as_ptr(),
            num_threads: inference_threads(),
            debug: 0,
            provider: provider.as_ptr(),
        };
        // SAFETY: the config strings outlive the call.
        let raw =
            unsafe { sys::speaker_embedding::SherpaOnnxCreateSpeakerEmbeddingExtractor(&config) };
        if raw.is_null() {
            return Err(HelperError::Runtime(
                "sherpa-onnx could not load the speaker embedding model".to_string(),
            ));
        }
        Ok(Self { raw })
    }

    pub fn dimensions(&self) -> usize {
        // SAFETY: live extractor.
        unsafe { sys::speaker_embedding::SherpaOnnxSpeakerEmbeddingExtractorDim(self.raw) }.max(0)
            as usize
    }

    /// Returns `None` when the audio is too short for the extractor.
    pub fn embed(&self, samples: &[f32]) -> Result<Option<Vec<f32>>, HelperError> {
        let length = i32::try_from(samples.len())
            .map_err(|_| HelperError::Runtime("speaker segment is too long".to_string()))?;
        let dimensions = self.dimensions();
        // SAFETY: the stream belongs to this extractor, input samples are copied by the stream,
        // and the embedding buffer (`dimensions` floats) is copied before it is freed.
        unsafe {
            let stream =
                sys::speaker_embedding::SherpaOnnxSpeakerEmbeddingExtractorCreateStream(self.raw);
            if stream.is_null() {
                return Err(HelperError::Runtime(
                    "could not create an embedding stream".to_string(),
                ));
            }
            sys::online_asr::SherpaOnnxOnlineStreamAcceptWaveform(
                stream,
                SAMPLE_RATE as i32,
                samples.as_ptr(),
                length,
            );
            sys::online_asr::SherpaOnnxOnlineStreamInputFinished(stream);
            let result = if sys::speaker_embedding::SherpaOnnxSpeakerEmbeddingExtractorIsReady(
                self.raw, stream,
            ) == 0
            {
                None
            } else {
                let embedding =
                    sys::speaker_embedding::SherpaOnnxSpeakerEmbeddingExtractorComputeEmbedding(
                        self.raw, stream,
                    );
                if embedding.is_null() {
                    None
                } else {
                    let values = std::slice::from_raw_parts(embedding, dimensions).to_vec();
                    sys::speaker_embedding::SherpaOnnxSpeakerEmbeddingExtractorDestroyEmbedding(
                        embedding,
                    );
                    Some(values)
                }
            };
            sys::online_asr::SherpaOnnxDestroyOnlineStream(stream);
            Ok(result)
        }
    }
}

impl Drop for SpeakerEmbeddingExtractor {
    fn drop(&mut self) {
        // SAFETY: created by SherpaOnnxCreateSpeakerEmbeddingExtractor and destroyed exactly once.
        unsafe { sys::speaker_embedding::SherpaOnnxDestroySpeakerEmbeddingExtractor(self.raw) }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DiarizedSegment {
    pub start_seconds: f32,
    pub end_seconds: f32,
    pub speaker: i32,
    /// Mean silhouette in `[-1, 1]`, or sherpa's `-2` sentinel when unavailable.
    pub silhouette: f32,
}

pub struct SpeakerDiarization {
    raw: *const sys::offline_speaker_diarization::OfflineSpeakerDiarization,
}

pub struct DiarizationSettings {
    pub clustering_threshold: f32,
    pub min_duration_on: f32,
    pub min_duration_off: f32,
}

impl SpeakerDiarization {
    pub fn new(
        segmentation: &Path,
        embedding: &Path,
        settings: &DiarizationSettings,
    ) -> Result<Self, HelperError> {
        let segmentation = c_path(segmentation)?;
        let embedding = c_path(embedding)?;
        let provider = c_string("cpu");
        let threads = inference_threads();
        let config = sys::offline_speaker_diarization::OfflineSpeakerDiarizationConfig {
            segmentation: sys::offline_speaker_diarization::OfflineSpeakerSegmentationModelConfig {
                pyannote: sys::offline_speaker_diarization::OfflineSpeakerSegmentationPyannoteModelConfig {
                    model: segmentation.as_ptr(),
                    window_shift_ratio: 0.0,
                },
                num_threads: threads,
                debug: 0,
                provider: provider.as_ptr(),
            },
            embedding: sys::speaker_embedding::SpeakerEmbeddingExtractorConfig {
                model: embedding.as_ptr(),
                num_threads: threads,
                debug: 0,
                provider: provider.as_ptr(),
            },
            clustering: sys::offline_speaker_diarization::FastClusteringConfig {
                num_clusters: -1,
                threshold: settings.clustering_threshold,
                compute_confidence: 1,
            },
            min_duration_on: settings.min_duration_on,
            min_duration_off: settings.min_duration_off,
        };
        // SAFETY: the config strings outlive the call.
        let raw = unsafe {
            sys::offline_speaker_diarization::SherpaOnnxCreateOfflineSpeakerDiarization(&config)
        };
        if raw.is_null() {
            return Err(HelperError::Runtime(
                "sherpa-onnx could not load the diarization models".to_string(),
            ));
        }
        Ok(Self { raw })
    }

    pub fn process(&self, samples: &[f32]) -> Result<Vec<DiarizedSegment>, HelperError> {
        let length = i32::try_from(samples.len())
            .map_err(|_| HelperError::Runtime("audio is too long for diarization".to_string()))?;
        // SAFETY: the result and segment array are owned by sherpa and freed before returning;
        // the segment array has exactly `NumSegments` entries.
        unsafe {
            let result =
                sys::offline_speaker_diarization::SherpaOnnxOfflineSpeakerDiarizationProcess(
                    self.raw,
                    samples.as_ptr(),
                    length,
                );
            if result.is_null() {
                return Ok(Vec::new());
            }
            let count = sys::offline_speaker_diarization::SherpaOnnxOfflineSpeakerDiarizationResultGetNumSegments(result)
                .max(0) as usize;
            let mut segments = Vec::with_capacity(count);
            if count > 0 {
                let array =
                    sys::offline_speaker_diarization::SherpaOnnxOfflineSpeakerDiarizationResultSortByStartTime(result);
                if !array.is_null() {
                    segments.extend(std::slice::from_raw_parts(array, count).iter().map(
                        |segment| DiarizedSegment {
                            start_seconds: segment.start,
                            end_seconds: segment.end,
                            speaker: segment.speaker,
                            silhouette: segment.confidence,
                        },
                    ));
                    sys::offline_speaker_diarization::SherpaOnnxOfflineSpeakerDiarizationDestroySegment(array);
                }
            }
            sys::offline_speaker_diarization::SherpaOnnxOfflineSpeakerDiarizationDestroyResult(
                result,
            );
            Ok(segments)
        }
    }
}

impl Drop for SpeakerDiarization {
    fn drop(&mut self) {
        // SAFETY: created by SherpaOnnxCreateOfflineSpeakerDiarization and destroyed exactly once.
        unsafe {
            sys::offline_speaker_diarization::SherpaOnnxDestroyOfflineSpeakerDiarization(self.raw)
        }
    }
}

pub fn runtime_versions() -> String {
    // SAFETY: both functions return static NUL-terminated strings.
    unsafe {
        let sherpa = sys::SherpaOnnxGetVersionStr();
        let onnxruntime = sys::SherpaOnnxGetOnnxruntimeVersionStr();
        let read = |value: *const std::os::raw::c_char| {
            if value.is_null() {
                String::new()
            } else {
                CStr::from_ptr(value).to_string_lossy().into_owned()
            }
        };
        format!(
            "sherpa-onnx {} / onnxruntime {}",
            read(sherpa),
            read(onnxruntime)
        )
    }
}
