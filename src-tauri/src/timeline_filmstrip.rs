use crate::precompose::compatibility::extract_compatibility_frames;
use crate::project::model::{MediaAsset, MediaKind};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;
use video_creater_compatibility_protocol::CompatibilityFrameRequest;

pub const FILMSTRIP_POLICY: &str = "timeline-filmstrip-120x68-bundled-v2";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct TimelineFilmstripFrame {
    pub time_seconds: f64,
    pub relative_path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct TimelineFilmstripReport {
    pub media_id: String,
    pub source_fingerprint: String,
    pub cache_key: String,
    pub cache_hit: bool,
    pub source_in: f64,
    pub source_out: f64,
    pub speed: f64,
    pub zoom_bucket: u32,
    pub height_bucket: u32,
    pub sampling_policy: String,
    pub frames: Vec<TimelineFilmstripFrame>,
}

pub trait TimelineFrameExtractor {
    fn extract_frames(
        &self,
        source: &Path,
        output_root: &Path,
        frames: &[(f64, PathBuf)],
    ) -> Result<(), String>;
}

pub struct BundledTimelineFrameExtractor;

impl TimelineFrameExtractor for BundledTimelineFrameExtractor {
    fn extract_frames(
        &self,
        source: &Path,
        output_root: &Path,
        frames: &[(f64, PathBuf)],
    ) -> Result<(), String> {
        let requests = frames
            .iter()
            .map(|(time_seconds, output)| CompatibilityFrameRequest {
                time_seconds: *time_seconds,
                output_path: output.to_string_lossy().into_owned(),
            })
            .collect();
        let results = extract_compatibility_frames(source, output_root, 120, 68, requests)?;
        if results.len() != frames.len()
            || results.iter().zip(frames).any(|(result, (time, output))| {
                (result.time_seconds - time).abs() > 0.000_1
                    || !output.is_file()
                    || result.bytes == 0
            })
        {
            return Err("bundled decoder returned an incomplete filmstrip".into());
        }
        Ok(())
    }
}

pub fn source_fingerprint(source: &Path) -> Result<String, String> {
    let metadata = fs::metadata(source).map_err(|error| error.to_string())?;
    let modified = metadata
        .modified()
        .ok()
        .and_then(|value| value.duration_since(UNIX_EPOCH).ok())
        .map(|value| value.as_nanos())
        .unwrap_or_default();
    let canonical = source
        .canonicalize()
        .unwrap_or_else(|_| source.to_path_buf());
    Ok(format!(
        "{:x}",
        Sha256::digest(format!(
            "{}:{}:{}",
            canonical.display(),
            metadata.len(),
            modified
        ))
    ))
}

pub fn sample_times(
    source_in: f64,
    source_out: f64,
    clip_pixel_width: f64,
    height: u32,
) -> Vec<f64> {
    let tile_width = (height.max(32) as f64 * 120.0 / 68.0).max(40.0);
    let count = ((clip_pixel_width / tile_width).ceil() as usize).clamp(1, 32);
    (0..count)
        .map(|index| {
            let normalized = (index as f64 + 0.5) / count as f64;
            ((source_in + (source_out - source_in) * normalized) * 1000.0).round() / 1000.0
        })
        .collect()
}

pub fn request_cache_key(
    fingerprint: &str,
    source_in: f64,
    source_out: f64,
    speed: f64,
    zoom_bucket: u32,
    height_bucket: u32,
    clip_pixel_width: f64,
) -> String {
    format!(
        "{:x}",
        Sha256::digest(format!(
            "{FILMSTRIP_POLICY}:{fingerprint}:{source_in:.3}:{source_out:.3}:{speed:.3}:{zoom_bucket}:{height_bucket}:{clip_pixel_width:.0}"
        ))
    )
}

fn manifest_is_complete(path: &Path, key: &str) -> Option<TimelineFilmstripReport> {
    let report: TimelineFilmstripReport =
        serde_json::from_slice(&fs::read(path.join("manifest.json")).ok()?).ok()?;
    if report.cache_key != key || report.frames.is_empty() {
        return None;
    }
    report
        .frames
        .iter()
        .all(|frame| {
            Path::new(&frame.relative_path)
                .file_name()
                .is_some_and(|filename| path.join(filename).is_file())
        })
        .then_some(report)
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TimelineFilmstripRequest {
    pub source_in: f64,
    pub source_out: f64,
    pub speed: f64,
    pub zoom_bucket: u32,
    pub height_bucket: u32,
    pub clip_pixel_width: f64,
}

pub fn cache_timeline_filmstrip_with_extractor(
    project_dir: &Path,
    media: &MediaAsset,
    source: &Path,
    request: TimelineFilmstripRequest,
    extractor: &impl TimelineFrameExtractor,
) -> Result<TimelineFilmstripReport, String> {
    let TimelineFilmstripRequest {
        source_in,
        source_out,
        speed,
        zoom_bucket,
        height_bucket,
        clip_pixel_width,
    } = request;
    if !matches!(media.kind, MediaKind::Video | MediaKind::Generated) {
        return Err("timeline filmstrips require video media".to_string());
    }
    if !source_in.is_finite()
        || !source_out.is_finite()
        || source_in < 0.0
        || source_out <= source_in
    {
        return Err("timeline filmstrip source range is invalid".to_string());
    }
    if !speed.is_finite()
        || speed <= 0.0
        || !clip_pixel_width.is_finite()
        || clip_pixel_width <= 0.0
    {
        return Err("timeline filmstrip geometry is invalid".to_string());
    }
    let bounded_out = if media.duration_seconds > 0.0 {
        source_out.min((media.duration_seconds - 0.001).max(source_in + 0.001))
    } else {
        source_out
    };
    let fingerprint = source_fingerprint(source)?;
    let key = request_cache_key(
        &fingerprint,
        source_in,
        bounded_out,
        speed,
        zoom_bucket,
        height_bucket,
        clip_pixel_width,
    );
    let relative_root = PathBuf::from("cache")
        .join("filmstrips")
        .join("v2")
        .join(&key);
    let cache_dir = project_dir.join(&relative_root);
    if let Some(mut report) = manifest_is_complete(&cache_dir, &key) {
        report.cache_hit = true;
        return Ok(report);
    }
    let root = project_dir.join("cache").join("filmstrips").join("v2");
    fs::create_dir_all(&root).map_err(|error| error.to_string())?;
    let staging = root.join(format!(".{key}-{}", uuid::Uuid::new_v4()));
    fs::create_dir_all(&staging).map_err(|error| error.to_string())?;
    let result = (|| {
        let times = sample_times(source_in, bounded_out, clip_pixel_width, height_bucket);
        let requests = times
            .iter()
            .enumerate()
            .map(|(index, time_seconds)| {
                (*time_seconds, staging.join(format!("frame-{index:03}.bmp")))
            })
            .collect::<Vec<_>>();
        extractor.extract_frames(source, &staging, &requests)?;
        let frames = requests
            .into_iter()
            .map(|(time_seconds, output)| TimelineFilmstripFrame {
                time_seconds,
                relative_path: relative_root
                    .join(output.file_name().expect("filmstrip output filename"))
                    .to_string_lossy()
                    .replace('\\', "/"),
            })
            .collect();
        let report = TimelineFilmstripReport {
            media_id: media.id.clone(),
            source_fingerprint: fingerprint,
            cache_key: key.clone(),
            cache_hit: false,
            source_in,
            source_out: bounded_out,
            speed,
            zoom_bucket,
            height_bucket,
            sampling_policy: FILMSTRIP_POLICY.to_string(),
            frames,
        };
        fs::write(
            staging.join("manifest.json"),
            serde_json::to_vec_pretty(&report).map_err(|error| error.to_string())?,
        )
        .map_err(|error| error.to_string())?;
        if cache_dir.exists() {
            fs::remove_dir_all(&cache_dir).map_err(|error| error.to_string())?;
        }
        fs::rename(&staging, &cache_dir).map_err(|error| error.to_string())?;
        Ok(report)
    })();
    if result.is_err() {
        let _ = fs::remove_dir_all(&staging);
    }
    result
}

pub fn cache_timeline_filmstrip(
    project_dir: &Path,
    media: &MediaAsset,
    source: &Path,
    request: TimelineFilmstripRequest,
) -> Result<TimelineFilmstripReport, String> {
    cache_timeline_filmstrip_with_extractor(
        project_dir,
        media,
        source,
        request,
        &BundledTimelineFrameExtractor,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    struct FakeExtractor(Mutex<Vec<f64>>);
    impl TimelineFrameExtractor for FakeExtractor {
        fn extract_frames(
            &self,
            _source: &Path,
            _output_root: &Path,
            frames: &[(f64, PathBuf)],
        ) -> Result<(), String> {
            for (time, output) in frames {
                self.0.lock().unwrap().push(*time);
                fs::write(output, b"bmp").map_err(|error| error.to_string())?;
            }
            Ok(())
        }
    }

    fn media() -> MediaAsset {
        MediaAsset {
            id: "media-1".into(),
            name: None,
            relative_path: "media/input.mp4".into(),
            kind: MediaKind::Video,
            duration_seconds: 20.0,
            width: None,
            height: None,
            fps: None,
            folder_id: None,
        }
    }

    #[test]
    fn samples_trimmed_ranges_as_bmp_and_reuses_complete_cache() {
        let temp = tempfile::tempdir().unwrap();
        let source = temp.path().join("input.mp4");
        fs::write(&source, b"video").unwrap();
        let extractor = FakeExtractor(Mutex::new(Vec::new()));
        let first = cache_timeline_filmstrip_with_extractor(
            temp.path(),
            &media(),
            &source,
            TimelineFilmstripRequest {
                source_in: 10.0,
                source_out: 18.0,
                speed: 2.0,
                zoom_bucket: 100,
                height_bucket: 68,
                clip_pixel_width: 480.0,
            },
            &extractor,
        )
        .unwrap();
        assert_eq!(
            first
                .frames
                .iter()
                .map(|frame| frame.time_seconds)
                .collect::<Vec<_>>(),
            vec![11.0, 13.0, 15.0, 17.0]
        );
        assert!(first
            .frames
            .iter()
            .all(|frame| frame.relative_path.ends_with(".bmp")));
        assert!(!first.cache_hit);
        let second = cache_timeline_filmstrip_with_extractor(
            temp.path(),
            &media(),
            &source,
            TimelineFilmstripRequest {
                source_in: 10.0,
                source_out: 18.0,
                speed: 2.0,
                zoom_bucket: 100,
                height_bucket: 68,
                clip_pixel_width: 480.0,
            },
            &extractor,
        )
        .unwrap();
        assert!(second.cache_hit);
        assert_eq!(extractor.0.lock().unwrap().len(), 4);
    }

    #[test]
    fn request_dimensions_change_key_and_media_bounds_clamp_samples() {
        assert_ne!(
            request_cache_key("source", 0.0, 4.0, 1.0, 100, 68, 240.0),
            request_cache_key("source", 0.0, 4.0, 2.0, 100, 68, 240.0)
        );
        let temp = tempfile::tempdir().unwrap();
        let source = temp.path().join("input.mp4");
        fs::write(&source, b"video").unwrap();
        let extractor = FakeExtractor(Mutex::new(Vec::new()));
        let report = cache_timeline_filmstrip_with_extractor(
            temp.path(),
            &media(),
            &source,
            TimelineFilmstripRequest {
                source_in: 19.0,
                source_out: 30.0,
                speed: 1.0,
                zoom_bucket: 100,
                height_bucket: 68,
                clip_pixel_width: 120.0,
            },
            &extractor,
        )
        .unwrap();
        assert!(report.frames.iter().all(|frame| frame.time_seconds < 20.0));
    }

    #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
    #[test]
    fn bundled_decoder_extracts_ordered_distinct_frames_from_sample_media() {
        let project = tempfile::tempdir().unwrap();
        let source = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("resources/sample-project/media/input.mp4");
        let sample = MediaAsset {
            id: "sample-input".into(),
            name: Some("Bundled Edison source".into()),
            relative_path: "media/input.mp4".into(),
            kind: MediaKind::Video,
            duration_seconds: 4.0,
            width: Some(640),
            height: Some(360),
            fps: Some(24.0),
            folder_id: None,
        };
        let report = cache_timeline_filmstrip(
            project.path(),
            &sample,
            &source,
            TimelineFilmstripRequest {
                source_in: 0.0,
                source_out: 4.0,
                speed: 1.0,
                zoom_bucket: 100,
                height_bucket: 68,
                clip_pixel_width: 480.0,
            },
        )
        .expect("bundled decoder should extract the sample filmstrip");
        assert_eq!(
            report
                .frames
                .iter()
                .map(|frame| frame.time_seconds)
                .collect::<Vec<_>>(),
            vec![0.5, 1.5, 2.499, 3.499]
        );
        let hashes = report
            .frames
            .iter()
            .map(|frame| {
                let bytes = fs::read(project.path().join(&frame.relative_path)).unwrap();
                assert_eq!(&bytes[..2], b"BM");
                format!("{:x}", Sha256::digest(bytes))
            })
            .collect::<std::collections::BTreeSet<_>>();
        assert_eq!(
            hashes.len(),
            4,
            "sample timestamps should produce distinct frames"
        );
    }
}
