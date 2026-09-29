use super::{safe_path_segment, sampled_visual_frame_times, SearchIndexError};
use crate::precompose::compatibility::extract_compatibility_frames;
use crate::project::model::{MediaAsset, MediaKind};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::fs;
use std::io::{BufReader, Read};
use std::path::Path;
use video_creater_compatibility_protocol::CompatibilityFrameRequest;

pub const VISUAL_FRAME_CACHE_ANALYZER_VERSION: &str = "frame-extraction-v1";
pub const VISUAL_FRAME_CACHE_SAMPLING_POLICY: &str = "opening-middle-closing-640png-v1";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct VisualFrameCacheReport {
    pub media_id: String,
    pub source_fingerprint: String,
    pub cache_key: String,
    pub cache_hit: bool,
    pub frame_count: usize,
    pub cache_relative_path: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VisualFrameCacheError {
    UnsupportedMediaKind,
    Cancelled,
    Io(String),
    Extraction(String),
    InvalidCache(String),
}

impl std::fmt::Display for VisualFrameCacheError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::UnsupportedMediaKind => {
                formatter.write_str("visual frame extraction is not available for this media kind")
            }
            Self::Cancelled => formatter.write_str("visual frame extraction was cancelled"),
            Self::Io(message) => write!(formatter, "visual frame cache IO failed: {message}"),
            Self::Extraction(message) => {
                write!(formatter, "visual frame extraction failed: {message}")
            }
            Self::InvalidCache(message) => {
                write!(formatter, "visual frame cache is invalid: {message}")
            }
        }
    }
}

impl std::error::Error for VisualFrameCacheError {}

impl From<std::io::Error> for VisualFrameCacheError {
    fn from(error: std::io::Error) -> Self {
        Self::Io(error.to_string())
    }
}

pub trait VisualFrameExtractor {
    fn extract_frame(
        &self,
        source_path: &Path,
        time_seconds: f64,
        output_path: &Path,
    ) -> Result<(), VisualFrameCacheError>;
}

pub struct SystemVisualFrameExtractor;

impl VisualFrameExtractor for SystemVisualFrameExtractor {
    fn extract_frame(
        &self,
        source_path: &Path,
        time_seconds: f64,
        output_path: &Path,
    ) -> Result<(), VisualFrameCacheError> {
        let output_root = output_path.parent().ok_or_else(|| {
            VisualFrameCacheError::Extraction("visual frame output has no parent".to_string())
        })?;
        let frames = extract_compatibility_frames(
            source_path,
            output_root,
            640,
            360,
            vec![CompatibilityFrameRequest {
                time_seconds,
                output_path: output_path.to_string_lossy().into_owned(),
            }],
        )
        .map_err(VisualFrameCacheError::Extraction)?;
        if frames.len() == 1 && frames[0].bytes > 0 && output_path.is_file() {
            return Ok(());
        }

        Err(VisualFrameCacheError::Extraction(
            "bundled compatibility decoder returned no visual frame".to_string(),
        ))
    }
}

pub fn cache_visual_frames(
    project_dir: &Path,
    media: &MediaAsset,
    source_path: &Path,
    is_cancelled: impl Fn() -> bool,
) -> Result<VisualFrameCacheReport, VisualFrameCacheError> {
    cache_visual_frames_with_extractor(
        project_dir,
        media,
        source_path,
        &SystemVisualFrameExtractor,
        is_cancelled,
    )
}

pub fn cache_visual_frames_with_extractor(
    project_dir: &Path,
    media: &MediaAsset,
    source_path: &Path,
    extractor: &impl VisualFrameExtractor,
    is_cancelled: impl Fn() -> bool,
) -> Result<VisualFrameCacheReport, VisualFrameCacheError> {
    if !matches!(
        media.kind,
        MediaKind::Video | MediaKind::Image | MediaKind::Generated
    ) {
        return Err(VisualFrameCacheError::UnsupportedMediaKind);
    }
    if is_cancelled() {
        return Err(VisualFrameCacheError::Cancelled);
    }

    let source_fingerprint = source_fingerprint(source_path)?;
    let sample_times = cache_sample_times(media);
    if sample_times.is_empty() {
        return Err(VisualFrameCacheError::InvalidCache(
            "no visual frame sample times are available".to_string(),
        ));
    }
    let cache_key = cache_key(&source_fingerprint);
    let visual_root = project_dir.join("search").join("visual");
    let cache_dir = visual_root.join(&cache_key);
    let cache_relative_path = format!("search/visual/{cache_key}");

    fs::create_dir_all(&visual_root)?;
    let cache_hit = cache_is_valid(&cache_dir, &source_fingerprint, sample_times.len());
    if !cache_hit {
        let temporary_dir = visual_root.join(format!(".{cache_key}-{}", uuid::Uuid::new_v4()));
        let result = (|| {
            fs::create_dir_all(temporary_dir.join("frames"))?;
            for (index, time_seconds) in sample_times.iter().enumerate() {
                if is_cancelled() {
                    return Err(VisualFrameCacheError::Cancelled);
                }
                let frame_path = temporary_dir
                    .join("frames")
                    .join(format!("frame-{index:06}.png"));
                extractor.extract_frame(source_path, *time_seconds, &frame_path)?;
                if !fs::metadata(&frame_path)
                    .map(|metadata| metadata.len() > 0)
                    .unwrap_or(false)
                {
                    return Err(VisualFrameCacheError::Extraction(format!(
                        "frame {index} was not written"
                    )));
                }
            }
            if is_cancelled() {
                return Err(VisualFrameCacheError::Cancelled);
            }
            let manifest = json!({
                "status": "ready",
                "sourceFingerprint": source_fingerprint,
                "analyzerVersion": VISUAL_FRAME_CACHE_ANALYZER_VERSION,
                "samplingPolicy": VISUAL_FRAME_CACHE_SAMPLING_POLICY,
                "frameCount": sample_times.len(),
                "sampleTimes": sample_times,
                "format": "png"
            });
            write_json_atomically(&temporary_dir.join("manifest.json"), &manifest)?;
            match fs::rename(&temporary_dir, &cache_dir) {
                Ok(()) => Ok(()),
                Err(_error)
                    if cache_is_valid(&cache_dir, &source_fingerprint, sample_times.len()) =>
                {
                    Ok(())
                }
                Err(error) => Err(VisualFrameCacheError::Io(error.to_string())),
            }
        })();
        if temporary_dir.exists() {
            let _ = fs::remove_dir_all(&temporary_dir);
        }
        result?;
    }

    publish_media_sidecar(
        project_dir,
        media,
        &source_fingerprint,
        &cache_key,
        &cache_relative_path,
        &sample_times,
    )?;

    Ok(VisualFrameCacheReport {
        media_id: media.id.clone(),
        source_fingerprint,
        cache_key,
        cache_hit,
        frame_count: sample_times.len(),
        cache_relative_path,
    })
}

pub fn source_fingerprint(path: &Path) -> Result<String, VisualFrameCacheError> {
    let file = fs::File::open(path)?;
    let mut reader = BufReader::new(file);
    let mut hasher = Sha256::new();
    let mut buffer = [0_u8; 32 * 1024];
    loop {
        let read = reader.read(&mut buffer)?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }
    Ok(format!("{:x}", hasher.finalize()))
}

fn cache_key(source_fingerprint: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(source_fingerprint.as_bytes());
    hasher.update(b":");
    hasher.update(VISUAL_FRAME_CACHE_ANALYZER_VERSION.as_bytes());
    hasher.update(b":");
    hasher.update(VISUAL_FRAME_CACHE_SAMPLING_POLICY.as_bytes());
    format!("{:x}", hasher.finalize())
}

fn cache_sample_times(media: &MediaAsset) -> Vec<f64> {
    let duration = media
        .duration_seconds
        .is_finite()
        .then_some(media.duration_seconds)
        .filter(|duration| *duration > 0.0);
    sampled_visual_frame_times(media)
        .into_iter()
        .map(|time_seconds| match duration {
            Some(duration) if time_seconds >= duration => (duration - 0.02).max(0.0),
            _ => time_seconds.max(0.0),
        })
        .map(|time_seconds| (time_seconds * 1000.0).round() / 1000.0)
        .collect()
}

fn cache_is_valid(cache_dir: &Path, source_fingerprint: &str, frame_count: usize) -> bool {
    let manifest = fs::read_to_string(cache_dir.join("manifest.json"))
        .ok()
        .and_then(|source| serde_json::from_str::<Value>(&source).ok());
    let Some(manifest) = manifest else {
        return false;
    };
    manifest.get("status").and_then(Value::as_str) == Some("ready")
        && manifest.get("sourceFingerprint").and_then(Value::as_str) == Some(source_fingerprint)
        && manifest.get("analyzerVersion").and_then(Value::as_str)
            == Some(VISUAL_FRAME_CACHE_ANALYZER_VERSION)
        && manifest.get("samplingPolicy").and_then(Value::as_str)
            == Some(VISUAL_FRAME_CACHE_SAMPLING_POLICY)
        && manifest.get("frameCount").and_then(Value::as_u64) == Some(frame_count as u64)
        && (0..frame_count).all(|index| {
            fs::metadata(
                cache_dir
                    .join("frames")
                    .join(format!("frame-{index:06}.png")),
            )
            .map(|metadata| metadata.len() > 0)
            .unwrap_or(false)
        })
}

fn publish_media_sidecar(
    project_dir: &Path,
    media: &MediaAsset,
    source_fingerprint: &str,
    cache_key: &str,
    cache_relative_path: &str,
    sample_times: &[f64],
) -> Result<(), VisualFrameCacheError> {
    let media_dir = project_dir
        .join("search")
        .join("visual")
        .join(safe_path_segment(&media.id));
    fs::create_dir_all(&media_dir)?;
    let sidecar_path = media_dir.join("frames.json");
    let mut sidecar = fs::read_to_string(&sidecar_path)
        .ok()
        .and_then(|source| serde_json::from_str::<Value>(&source).ok())
        .unwrap_or_else(|| {
            json!({
                "schemaVersion": 1,
                "mediaId": media.id,
                "mediaKind": media.kind,
                "relativePath": media.relative_path,
                "durationSeconds": media.duration_seconds,
                "status": "notInstalled",
                "visualStatus": "notInstalled",
                "embeddingModel": null
            })
        });
    let existing_frames = sidecar
        .get("frames")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let frames = sample_times
        .iter()
        .enumerate()
        .map(|(index, time_seconds)| {
            let mut frame = existing_frames
                .get(index)
                .cloned()
                .unwrap_or_else(|| json!({}));
            let object = frame
                .as_object_mut()
                .expect("frame object is constructed above");
            object.insert("index".to_string(), json!(index));
            object.insert("timeSeconds".to_string(), json!(time_seconds));
            object.insert(
                "relativeFramePath".to_string(),
                json!(format!("{cache_relative_path}/frames/frame-{index:06}.png")),
            );
            frame
        })
        .collect::<Vec<_>>();
    let object = sidecar.as_object_mut().ok_or_else(|| {
        VisualFrameCacheError::InvalidCache("sidecar is not an object".to_string())
    })?;
    object.insert("frames".to_string(), Value::Array(frames));
    object.insert(
        "frameCache".to_string(),
        json!({
            "status": "ready",
            "sourceFingerprint": source_fingerprint,
            "cacheKey": cache_key,
            "analyzerVersion": VISUAL_FRAME_CACHE_ANALYZER_VERSION,
            "samplingPolicy": VISUAL_FRAME_CACHE_SAMPLING_POLICY,
            "format": "png"
        }),
    );
    // Frame pixels are ready for a future model, but no captioner or embedding
    // model is bundled. Keep the search capability boundary truthful.
    object
        .entry("status".to_string())
        .or_insert_with(|| json!("notInstalled"));
    object
        .entry("visualStatus".to_string())
        .or_insert_with(|| json!("notInstalled"));
    object
        .entry("embeddingModel".to_string())
        .or_insert(Value::Null);
    write_json_atomically(&sidecar_path, &sidecar)
}

pub(crate) fn write_json_atomically(
    path: &Path,
    value: &Value,
) -> Result<(), VisualFrameCacheError> {
    let parent = path
        .parent()
        .ok_or_else(|| VisualFrameCacheError::Io("cache path has no parent".to_string()))?;
    fs::create_dir_all(parent)?;
    let temporary_path = parent.join(format!(
        ".{}-{}.tmp",
        path.file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("cache"),
        uuid::Uuid::new_v4()
    ));
    fs::write(
        &temporary_path,
        serde_json::to_vec_pretty(value)
            .map_err(|error| VisualFrameCacheError::Io(error.to_string()))?,
    )?;
    fs::rename(temporary_path, path)?;
    Ok(())
}

impl From<SearchIndexError> for VisualFrameCacheError {
    fn from(error: SearchIndexError) -> Self {
        Self::InvalidCache(error.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::project::model::{MediaAsset, MediaKind};
    use std::sync::atomic::{AtomicUsize, Ordering};

    struct RecordingExtractor {
        calls: AtomicUsize,
        fail_after: Option<usize>,
    }

    impl VisualFrameExtractor for RecordingExtractor {
        fn extract_frame(
            &self,
            _source_path: &Path,
            _time_seconds: f64,
            output_path: &Path,
        ) -> Result<(), VisualFrameCacheError> {
            let call = self.calls.fetch_add(1, Ordering::SeqCst);
            if self.fail_after.is_some_and(|limit| call >= limit) {
                return Err(VisualFrameCacheError::Extraction(
                    "fixture failure".to_string(),
                ));
            }
            fs::write(output_path, format!("frame-{call}")).map_err(VisualFrameCacheError::from)
        }
    }

    fn media() -> MediaAsset {
        MediaAsset {
            id: "video-1".to_string(),
            name: Some("Visual source".to_string()),
            relative_path: "media/source.mp4".to_string(),
            kind: MediaKind::Video,
            duration_seconds: 4.0,
            width: None,
            height: None,
            fps: None,
            folder_id: None,
        }
    }

    #[test]
    fn fingerprints_change_when_source_bytes_change() {
        let temporary = tempfile::tempdir().expect("temporary directory");
        let source = temporary.path().join("source.mp4");
        fs::write(&source, b"first").expect("source bytes");
        let first = source_fingerprint(&source).expect("first fingerprint");
        fs::write(&source, b"second").expect("updated source bytes");
        assert_ne!(
            first,
            source_fingerprint(&source).expect("second fingerprint")
        );
    }

    #[test]
    fn caches_frames_atomically_and_reuses_a_valid_matching_cache() {
        let temporary = tempfile::tempdir().expect("temporary directory");
        let source = temporary.path().join("source.mp4");
        fs::write(&source, b"source bytes").expect("source bytes");
        let extractor = RecordingExtractor {
            calls: AtomicUsize::new(0),
            fail_after: None,
        };

        let first = cache_visual_frames_with_extractor(
            temporary.path(),
            &media(),
            &source,
            &extractor,
            || false,
        )
        .expect("cache frames");
        assert!(!first.cache_hit);
        assert_eq!(first.frame_count, 3);
        assert_eq!(extractor.calls.load(Ordering::SeqCst), 3);
        assert!(temporary
            .path()
            .join(&first.cache_relative_path)
            .join("frames/frame-000000.png")
            .is_file());

        let second = cache_visual_frames_with_extractor(
            temporary.path(),
            &media(),
            &source,
            &extractor,
            || false,
        )
        .expect("reuse cache");
        assert!(second.cache_hit);
        assert_eq!(extractor.calls.load(Ordering::SeqCst), 3);
        let sidecar: Value = serde_json::from_str(
            &fs::read_to_string(temporary.path().join("search/visual/video-1/frames.json"))
                .expect("sidecar"),
        )
        .expect("sidecar JSON");
        assert_eq!(sidecar["frameCache"]["status"], "ready");
        assert_eq!(sidecar["visualStatus"], "notInstalled");
        assert!(sidecar["frames"][0]["relativeFramePath"]
            .as_str()
            .expect("frame path")
            .contains(&first.cache_key));
    }

    #[test]
    fn failed_extraction_never_publishes_a_ready_cache_or_sidecar() {
        let temporary = tempfile::tempdir().expect("temporary directory");
        let source = temporary.path().join("source.mp4");
        fs::write(&source, b"source bytes").expect("source bytes");
        let extractor = RecordingExtractor {
            calls: AtomicUsize::new(0),
            fail_after: Some(1),
        };

        let error = cache_visual_frames_with_extractor(
            temporary.path(),
            &media(),
            &source,
            &extractor,
            || false,
        )
        .expect_err("extraction fails");
        assert!(matches!(error, VisualFrameCacheError::Extraction(_)));
        assert!(!temporary
            .path()
            .join("search/visual/video-1/frames.json")
            .exists());
        assert!(fs::read_dir(temporary.path().join("search/visual"))
            .expect("visual cache root")
            .all(|entry| {
                !entry
                    .expect("directory entry")
                    .file_name()
                    .to_string_lossy()
                    .starts_with('.')
            }));
    }

    #[test]
    fn cancellation_between_frames_never_publishes_partial_cache() {
        let temporary = tempfile::tempdir().expect("temporary directory");
        let source = temporary.path().join("source.mp4");
        fs::write(&source, b"source bytes").expect("source bytes");
        let extractor = RecordingExtractor {
            calls: AtomicUsize::new(0),
            fail_after: None,
        };

        let error = cache_visual_frames_with_extractor(
            temporary.path(),
            &media(),
            &source,
            &extractor,
            || extractor.calls.load(Ordering::SeqCst) >= 1,
        )
        .expect_err("cancellation stops the next frame");
        assert_eq!(error, VisualFrameCacheError::Cancelled);
        assert_eq!(extractor.calls.load(Ordering::SeqCst), 1);
        assert!(!temporary
            .path()
            .join("search/visual/video-1/frames.json")
            .exists());
    }
}
