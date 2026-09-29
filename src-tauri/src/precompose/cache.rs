use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::fs::{self, File, OpenOptions};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::thread;
use std::time::{Duration, Instant, SystemTime};

use crate::frame_compositor::PreparedEffectStack;
use crate::render_pipeline::error::{PipelineError, PipelineErrorCode, PipelineResult};

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LottieFingerprint<'a> {
    pub schema_version: u32,
    pub source_sha256: &'a str,
    pub animation_id: Option<&'a str>,
    pub runtime_inputs: &'a video_creater_precompose_protocol::LottieRuntimeInputs,
    pub width: u32,
    pub height: u32,
    pub fps_numerator: u32,
    pub fps_denominator: u32,
    pub source_start_micros: u64,
    pub source_stop_micros: u64,
    pub playback_rate_micros: u32,
    pub timeline_duration_micros: u64,
    pub frame_count: u32,
    pub looping: bool,
    pub expressions_enabled: bool,
    pub pixel_contract: &'static str,
    pub rng_policy: &'static str,
    pub dotlottie_revision: &'static str,
    pub thorvg_revision: &'static str,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LutFingerprint<'a> {
    pub schema_version: u32,
    pub source_sha256: &'a str,
    pub lut_sha256: &'a str,
    pub lut_strength_micros: u32,
    pub lut_effect_stack: &'a PreparedEffectStack,
    pub pre_lut_effect_stack: &'a PreparedEffectStack,
    pub width: u32,
    pub height: u32,
    pub fps_numerator: u32,
    pub fps_denominator: u32,
    pub source_start_micros: u64,
    pub source_stop_micros: u64,
    pub playback_rate_micros: u32,
    pub timeline_duration_micros: u64,
    pub frame_count: u32,
    /// Transition head handle the intermediate starts with; omitted when zero
    /// so fingerprints of clips without transitions are unchanged.
    #[serde(skip_serializing_if = "is_zero_micros")]
    pub transition_head_micros: u64,
    pub pixel_contract: &'static str,
    pub interpolation: &'static str,
    pub sampling_policy: &'static str,
    pub compositor_revision: &'static str,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CacheManifest {
    pub schema_version: u32,
    pub fingerprint: String,
    pub worker_manifest: String,
    pub intermediate: String,
    pub intermediate_sha256: String,
    pub frame_count: u32,
    pub width: u32,
    pub height: u32,
    pub fps_numerator: u32,
    pub fps_denominator: u32,
}

pub fn fingerprint(input: &impl Serialize) -> PipelineResult<String> {
    let bytes = serde_json::to_vec(input).map_err(|error| {
        vec![PipelineError::new(
            PipelineErrorCode::RenderBackendFailed,
            "precompose.cache.fingerprint",
            "Precompose cache fingerprint could not be serialized.",
            "Validate the deterministic precompose contract and retry.",
        )
        .with_detail("error", error.to_string())]
    })?;
    Ok(format!("{:x}", Sha256::digest(bytes)))
}

pub fn cache_entry_dir(project_dir: &Path, fingerprint: &str) -> PathBuf {
    project_dir
        .join("cache/precompose/v1/sha256")
        .join(&fingerprint[..2])
        .join(fingerprint)
}

pub fn validate_cache_entry(directory: &Path, fingerprint: &str) -> Option<CacheManifest> {
    let manifest_path = safe_cache_file(directory, Path::new("cache-manifest.json"))?;
    let manifest: CacheManifest = serde_json::from_slice(&fs::read(manifest_path).ok()?).ok()?;
    if manifest.schema_version != 1
        || manifest.fingerprint != fingerprint
        || manifest.frame_count == 0
        || manifest.width == 0
        || manifest.height == 0
        || manifest.fps_numerator == 0
        || manifest.fps_denominator == 0
        || manifest.intermediate_sha256.len() != 64
    {
        return None;
    }
    let worker_manifest = safe_cache_file(directory, Path::new(&manifest.worker_manifest))?;
    let intermediate = safe_cache_file(directory, Path::new(&manifest.intermediate))?;
    if sha256_file(&intermediate).ok()? != manifest.intermediate_sha256 {
        return None;
    }
    let stage_manifest = fs::read(worker_manifest).ok()?;
    let stage_manifest: serde_json::Value = serde_json::from_slice(&stage_manifest).ok()?;
    if !validate_stage_contract(&stage_manifest, &manifest)
        || !validate_stage_frames(directory, &stage_manifest, &manifest)
    {
        return None;
    }
    Some(manifest)
}

fn validate_stage_contract(stage: &serde_json::Value, cache: &CacheManifest) -> bool {
    let identity = stage
        .get("fingerprint")
        .or_else(|| stage.get("cacheKey"))
        .and_then(serde_json::Value::as_str);
    stage
        .get("schemaVersion")
        .and_then(serde_json::Value::as_u64)
        == Some(1)
        && identity == Some(cache.fingerprint.as_str())
        && stage.get("frameCount").and_then(serde_json::Value::as_u64)
            == Some(u64::from(cache.frame_count))
        && stage.get("width").and_then(serde_json::Value::as_u64) == Some(u64::from(cache.width))
        && stage.get("height").and_then(serde_json::Value::as_u64) == Some(u64::from(cache.height))
        && stage
            .get("fpsNumerator")
            .and_then(serde_json::Value::as_u64)
            == Some(u64::from(cache.fps_numerator))
        && stage
            .get("fpsDenominator")
            .and_then(serde_json::Value::as_u64)
            == Some(u64::from(cache.fps_denominator))
}

fn validate_stage_frames(
    directory: &Path,
    stage_manifest: &serde_json::Value,
    cache: &CacheManifest,
) -> bool {
    if let Some(frames) = stage_manifest
        .get("frames")
        .and_then(serde_json::Value::as_array)
    {
        if frames.len() != cache.frame_count as usize {
            return false;
        }
        return frames.iter().enumerate().all(|(index, frame)| {
            if frame.get("index").and_then(serde_json::Value::as_u64) != Some(index as u64) {
                return false;
            }
            let Some(path) = frame.get("path").and_then(serde_json::Value::as_str) else {
                return false;
            };
            if path != format!("frames/frame-{index:06}.png") {
                return false;
            }
            let Some(expected) = frame.get("pngSha256").and_then(serde_json::Value::as_str) else {
                return false;
            };
            let Some(expected_rgba) = frame.get("rgbaSha256").and_then(serde_json::Value::as_str)
            else {
                return false;
            };
            validate_frame_hash(
                directory,
                path,
                expected,
                expected_rgba,
                cache.width,
                cache.height,
            )
        });
    }
    let Some(png_hashes) = stage_manifest
        .get("framePngSha256")
        .and_then(serde_json::Value::as_array)
    else {
        return false;
    };
    let Some(rgba_hashes) = stage_manifest
        .get("frameRgbaSha256")
        .and_then(serde_json::Value::as_array)
    else {
        return false;
    };
    png_hashes.len() == cache.frame_count as usize
        && rgba_hashes.len() == cache.frame_count as usize
        && png_hashes.iter().zip(rgba_hashes).enumerate().all(
            |(index, (expected_png, expected_rgba))| {
                expected_png.as_str().is_some_and(|expected_png| {
                    expected_rgba.as_str().is_some_and(|expected_rgba| {
                        validate_frame_hash(
                            directory,
                            &format!("frames/frame-{index:06}.png"),
                            expected_png,
                            expected_rgba,
                            cache.width,
                            cache.height,
                        )
                    })
                })
            },
        )
}

fn safe_cache_file(directory: &Path, relative: &Path) -> Option<PathBuf> {
    if relative.is_absolute()
        || relative.components().any(|component| {
            matches!(
                component,
                std::path::Component::ParentDir | std::path::Component::RootDir
            )
        })
    {
        return None;
    }
    let root = directory.canonicalize().ok()?;
    let candidate = directory.join(relative);
    let metadata = fs::symlink_metadata(&candidate).ok()?;
    if metadata.file_type().is_symlink() || !metadata.is_file() || metadata.len() == 0 {
        return None;
    }
    let canonical = candidate.canonicalize().ok()?;
    canonical.starts_with(&root).then_some(canonical)
}

fn validate_frame_hash(
    directory: &Path,
    relative: &str,
    expected_png: &str,
    expected_rgba: &str,
    expected_width: u32,
    expected_height: u32,
) -> bool {
    let relative = Path::new(relative);
    if expected_png.len() != 64 || expected_rgba.len() != 64 {
        return false;
    }
    let Some(path) = safe_cache_file(directory, relative) else {
        return false;
    };
    if sha256_file(&path).ok().as_deref() != Some(expected_png) {
        return false;
    }
    let Ok(image) = image::open(path).map(image::DynamicImage::into_rgba8) else {
        return false;
    };
    image.width() == expected_width
        && image.height() == expected_height
        && format!("{:x}", Sha256::digest(image.as_raw())) == expected_rgba
}

pub fn sha256_file(path: &Path) -> std::io::Result<String> {
    let mut file = File::open(path)?;
    let mut digest = Sha256::new();
    let mut buffer = [0_u8; 64 * 1024];
    loop {
        let read = file.read(&mut buffer)?;
        if read == 0 {
            break;
        }
        digest.update(&buffer[..read]);
    }
    Ok(format!("{:x}", digest.finalize()))
}

pub struct CacheLock {
    path: PathBuf,
}

impl CacheLock {
    pub fn acquire(
        project_dir: &Path,
        fingerprint: &str,
        timeout: Duration,
    ) -> PipelineResult<Self> {
        let locks = project_dir.join("cache/precompose/v1/locks");
        fs::create_dir_all(&locks).map_err(|error| cache_io_error("locks", error))?;
        let path = locks.join(format!("{fingerprint}.lock"));
        let started = Instant::now();
        loop {
            match OpenOptions::new().write(true).create_new(true).open(&path) {
                Ok(mut file) => {
                    writeln!(file, "{}", std::process::id())
                        .map_err(|error| cache_io_error("lockWrite", error))?;
                    return Ok(Self { path });
                }
                Err(error)
                    if error.kind() == std::io::ErrorKind::AlreadyExists
                        && started.elapsed() < timeout =>
                {
                    if lock_is_abandoned(&path) {
                        let _ = fs::remove_file(&path);
                        continue;
                    }
                    thread::sleep(Duration::from_millis(25));
                }
                Err(error) => return Err(cache_io_error("lock", error)),
            }
        }
    }
}

fn lock_is_abandoned(path: &Path) -> bool {
    let Ok(contents) = fs::read_to_string(path) else {
        return fs::metadata(path)
            .and_then(|metadata| metadata.modified())
            .ok()
            .and_then(|modified| SystemTime::now().duration_since(modified).ok())
            .is_some_and(|age| age > Duration::from_secs(300));
    };
    let Some(pid) = contents.trim().parse::<u32>().ok() else {
        return true;
    };
    !process_is_alive(pid)
}

#[cfg(unix)]
fn process_is_alive(pid: u32) -> bool {
    unsafe extern "C" {
        fn kill(pid: i32, signal: i32) -> i32;
    }
    i32::try_from(pid)
        .ok()
        .is_some_and(|pid| unsafe { kill(pid, 0) == 0 })
}

#[cfg(not(unix))]
fn process_is_alive(_pid: u32) -> bool {
    true
}

pub struct StagingCleanup {
    path: PathBuf,
    published: bool,
}

impl StagingCleanup {
    pub fn new(path: PathBuf) -> Self {
        Self {
            path,
            published: false,
        }
    }

    pub fn disarm(&mut self) {
        self.published = true;
    }
}

impl Drop for StagingCleanup {
    fn drop(&mut self) {
        if !self.published {
            let _ = fs::remove_dir_all(&self.path);
        }
    }
}

impl Drop for CacheLock {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.path);
    }
}

fn cache_io_error(path: &str, error: std::io::Error) -> Vec<PipelineError> {
    vec![PipelineError::new(
        PipelineErrorCode::RenderBackendFailed,
        format!("precompose.cache.{path}"),
        "Precompose cache operation failed.",
        "Check project cache permissions and retry.",
    )
    .with_detail("error", error.to_string())]
}

fn is_zero_micros(value: &u64) -> bool {
    *value == 0
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn lottie_fingerprint(duration: u64, stop: u64) -> LottieFingerprint<'static> {
        static INPUTS: std::sync::LazyLock<video_creater_precompose_protocol::LottieRuntimeInputs> =
            std::sync::LazyLock::new(Default::default);
        LottieFingerprint {
            schema_version: 1,
            source_sha256: "source",
            animation_id: None,
            runtime_inputs: &INPUTS,
            width: 64,
            height: 64,
            fps_numerator: 30,
            fps_denominator: 1,
            source_start_micros: 0,
            source_stop_micros: stop,
            playback_rate_micros: 1_000_000,
            timeline_duration_micros: duration,
            frame_count: 31,
            looping: true,
            expressions_enabled: true,
            pixel_contract: "rgba8-srgb-straight-v1",
            rng_policy: "expression-code-frame-xorshift64star-v1",
            dotlottie_revision: "dotlottie",
            thorvg_revision: "thorvg",
        }
    }

    fn write_valid_entry(root: &Path, fingerprint: &str) -> PathBuf {
        let directory = root.join("entry");
        let frames = directory.join("frames");
        fs::create_dir_all(&frames).expect("frames directory");
        let mut frame_manifests = Vec::new();
        for index in 0..2 {
            let rgba = vec![index as u8, 20, 30, 255, 40, 50, 60, 128];
            let path = frames.join(format!("frame-{index:06}.png"));
            image::RgbaImage::from_raw(2, 1, rgba.clone())
                .expect("RGBA image")
                .save(&path)
                .expect("PNG frame");
            frame_manifests.push(json!({
                "index": index,
                "path": format!("frames/frame-{index:06}.png"),
                "rgbaSha256": format!("{:x}", Sha256::digest(&rgba)),
                "pngSha256": sha256_file(&path).expect("PNG hash")
            }));
        }
        let stage = json!({
            "schemaVersion": 1,
            "cacheKey": fingerprint,
            "frameCount": 2,
            "width": 2,
            "height": 1,
            "fpsNumerator": 30,
            "fpsDenominator": 1,
            "frames": frame_manifests
        });
        fs::write(
            directory.join("manifest.json"),
            serde_json::to_vec(&stage).expect("stage JSON"),
        )
        .expect("stage manifest");
        let intermediate = directory.join("intermediate.mov");
        fs::write(&intermediate, b"deterministic intermediate").expect("intermediate");
        let cache = CacheManifest {
            schema_version: 1,
            fingerprint: fingerprint.to_string(),
            worker_manifest: "manifest.json".to_string(),
            intermediate: "intermediate.mov".to_string(),
            intermediate_sha256: sha256_file(&intermediate).expect("intermediate hash"),
            frame_count: 2,
            width: 2,
            height: 1,
            fps_numerator: 30,
            fps_denominator: 1,
        };
        fs::write(
            directory.join("cache-manifest.json"),
            serde_json::to_vec(&cache).expect("cache JSON"),
        )
        .expect("cache manifest");
        directory
    }

    #[test]
    fn exact_lottie_duration_and_stop_are_part_of_the_cache_key() {
        let first = fingerprint(&lottie_fingerprint(1_001_000, 1_001_000)).expect("fingerprint");
        let second = fingerprint(&lottie_fingerprint(1_020_000, 1_020_000)).expect("fingerprint");
        assert_ne!(first, second);
    }

    #[test]
    fn typed_lottie_inputs_are_part_of_the_cache_key() {
        let baseline = fingerprint(&lottie_fingerprint(1_000_000, 1_000_000)).expect("baseline");
        let inputs = Box::leak(Box::new(
            video_creater_precompose_protocol::LottieRuntimeInputs {
                marker: Some("active".to_string()),
                ..Default::default()
            },
        ));
        let mut changed = lottie_fingerprint(1_000_000, 1_000_000);
        changed.runtime_inputs = inputs;
        assert_ne!(baseline, fingerprint(&changed).expect("changed"));
    }

    #[test]
    fn cache_validation_binds_stage_contract_and_decoded_rgba() {
        let temporary = tempfile::tempdir().expect("temporary directory");
        let fingerprint = "a".repeat(64);
        let directory = write_valid_entry(temporary.path(), &fingerprint);
        assert!(validate_cache_entry(&directory, &fingerprint).is_some());

        let manifest_path = directory.join("manifest.json");
        let mut stage: serde_json::Value =
            serde_json::from_slice(&fs::read(&manifest_path).expect("stage manifest"))
                .expect("stage JSON");
        stage["frames"][1]["path"] = json!("frames/frame-000000.png");
        fs::write(
            &manifest_path,
            serde_json::to_vec(&stage).expect("stage JSON"),
        )
        .expect("stage manifest");
        assert!(validate_cache_entry(&directory, &fingerprint).is_none());
    }

    #[test]
    fn abandoned_lock_is_recovered_and_staging_cleanup_is_automatic() {
        let temporary = tempfile::tempdir().expect("temporary directory");
        let fingerprint = "b".repeat(64);
        let locks = temporary.path().join("cache/precompose/v1/locks");
        fs::create_dir_all(&locks).expect("locks directory");
        fs::write(locks.join(format!("{fingerprint}.lock")), "4294967294\n").expect("stale lock");
        let lock = CacheLock::acquire(temporary.path(), &fingerprint, Duration::from_millis(100))
            .expect("recover abandoned lock");
        drop(lock);

        let staging = temporary.path().join("staging");
        fs::create_dir_all(&staging).expect("staging directory");
        {
            let _cleanup = StagingCleanup::new(staging.clone());
        }
        assert!(!staging.exists());
    }

    #[cfg(unix)]
    #[test]
    fn cache_validation_rejects_symlinked_frames() {
        use std::os::unix::fs::symlink;

        let temporary = tempfile::tempdir().expect("temporary directory");
        let fingerprint = "c".repeat(64);
        let directory = write_valid_entry(temporary.path(), &fingerprint);
        let frame = directory.join("frames/frame-000001.png");
        let target = directory.join("frames/target.png");
        fs::rename(&frame, &target).expect("move frame");
        symlink(&target, &frame).expect("frame symlink");
        assert!(validate_cache_entry(&directory, &fingerprint).is_none());
    }
}
