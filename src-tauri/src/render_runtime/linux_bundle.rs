//! Linux packages cannot carry the plugin symlink directory, because bundlers copy symlink
//! targets. The package ships `manifest.json`, `bundled-plugins/` and `lib/`; the plugin
//! directory is rebuilt beneath the app-support root from the manifest's explicit file list, so
//! only reviewed system plugins are ever visible to GStreamer.

use super::{default_app_support_root, runtime_fingerprint, RenderRuntimeError};
use serde::Deserialize;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct PackagedManifest {
    linux: Option<LinuxRuntimeLayout>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct LinuxRuntimeLayout {
    pub system_plugin_directory: PathBuf,
    pub system_plugin_files: Vec<String>,
    pub bundled_plugin_directory: String,
    pub bundled_plugin_files: Vec<String>,
    pub scanner: PathBuf,
    pub gstreamer_core_library: PathBuf,
}

const COMPLETE_MARKER: &str = ".materialized";

pub(super) fn materialize_if_packaged(
    bundle_root: &Path,
) -> Result<Option<PathBuf>, RenderRuntimeError> {
    if !cfg!(target_os = "linux") {
        return Ok(None);
    }
    let manifest_path = bundle_root.join("manifest.json");
    let Ok(bytes) = fs::read(&manifest_path) else {
        return Ok(None);
    };
    let Some(layout) = serde_json::from_slice::<PackagedManifest>(&bytes)
        .ok()
        .and_then(|manifest| manifest.linux)
    else {
        return Ok(None);
    };
    let fingerprint = runtime_fingerprint(&manifest_path, bundle_root)?;
    let destination = default_app_support_root()
        .join("render-runtime")
        .join(format!("linux-{}", &fingerprint[..32]));
    materialize(bundle_root, &layout, &bytes, &destination).map(Some)
}

pub(super) fn materialize(
    bundle_root: &Path,
    layout: &LinuxRuntimeLayout,
    manifest_bytes: &[u8],
    destination: &Path,
) -> Result<PathBuf, RenderRuntimeError> {
    validate_layout(layout)?;
    if destination.join(COMPLETE_MARKER).is_file() && links_resolve(destination) {
        return Ok(destination.to_path_buf());
    }
    let parent = destination
        .parent()
        .ok_or_else(|| materialize_error("destination has no parent", destination))?;
    fs::create_dir_all(parent).map_err(|error| materialize_error(error, parent))?;
    let staging = parent.join(format!(
        ".{}.staging-{}-{}",
        destination
            .file_name()
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_default(),
        std::process::id(),
        uuid::Uuid::new_v4().simple()
    ));
    let _ = fs::remove_dir_all(&staging);
    for directory in ["plugins", "lib", "libexec"] {
        fs::create_dir_all(staging.join(directory))
            .map_err(|error| materialize_error(error, &staging))?;
    }
    fs::write(staging.join("manifest.json"), manifest_bytes)
        .map_err(|error| materialize_error(error, &staging))?;
    for file in &layout.system_plugin_files {
        let target = layout.system_plugin_directory.join(file);
        // A missing optional distribution plugin is reported through factory health checks.
        if target.is_file() {
            link(&target, &staging.join("plugins").join(file))?;
        }
    }
    for file in &layout.bundled_plugin_files {
        let target = bundle_root
            .join(&layout.bundled_plugin_directory)
            .join(file);
        if !target.is_file() {
            return Err(materialize_error("bundled plugin is missing", &target));
        }
        link(&target, &staging.join("plugins").join(file))?;
    }
    if let Ok(entries) = fs::read_dir(bundle_root.join("lib")) {
        for entry in entries.flatten() {
            link(&entry.path(), &staging.join("lib").join(entry.file_name()))?;
        }
    }
    link(
        &layout.gstreamer_core_library,
        &staging.join("lib/libgstreamer-1.0.so.0"),
    )?;
    link(&layout.scanner, &staging.join("libexec/gst-plugin-scanner"))?;
    fs::write(staging.join(COMPLETE_MARKER), b"")
        .map_err(|error| materialize_error(error, &staging))?;
    if destination.join(COMPLETE_MARKER).is_file() && links_resolve(destination) {
        // A concurrent launch completed the same fingerprinted runtime; keep the live copy.
        let _ = fs::remove_dir_all(&staging);
        return Ok(destination.to_path_buf());
    }
    if destination.exists() {
        // Move a stale or broken runtime aside rather than deleting files a process may map.
        let retired = parent.join(format!(
            ".{}.retired-{}",
            destination
                .file_name()
                .map(|name| name.to_string_lossy().into_owned())
                .unwrap_or_default(),
            uuid::Uuid::new_v4().simple()
        ));
        if fs::rename(destination, &retired).is_ok() {
            let _ = fs::remove_dir_all(&retired);
        }
    }
    match fs::rename(&staging, destination) {
        Ok(()) => Ok(destination.to_path_buf()),
        // Another process finished the same fingerprinted materialization first.
        Err(_) if destination.join(COMPLETE_MARKER).is_file() => {
            let _ = fs::remove_dir_all(&staging);
            Ok(destination.to_path_buf())
        }
        Err(error) => Err(materialize_error(error, destination)),
    }
}

fn validate_layout(layout: &LinuxRuntimeLayout) -> Result<(), RenderRuntimeError> {
    let unsafe_name = layout
        .system_plugin_files
        .iter()
        .chain(&layout.bundled_plugin_files)
        .chain(std::iter::once(&layout.bundled_plugin_directory))
        .find(|name| {
            name.is_empty() || name.contains('/') || name.contains('\\') || name.starts_with('.')
        });
    if let Some(name) = unsafe_name {
        return Err(materialize_error(
            "manifest contains an unsafe file name",
            Path::new(name),
        ));
    }
    if !layout.system_plugin_directory.is_absolute()
        || !layout.scanner.is_absolute()
        || !layout.gstreamer_core_library.is_absolute()
    {
        return Err(materialize_error(
            "manifest system paths must be absolute",
            &layout.system_plugin_directory,
        ));
    }
    for required in [&layout.scanner, &layout.gstreamer_core_library] {
        if !required.is_file() {
            return Err(materialize_error(
                "required system GStreamer file is missing; reinstall the GStreamer packages",
                required,
            ));
        }
    }
    Ok(())
}

fn links_resolve(root: &Path) -> bool {
    ["plugins", "lib", "libexec"].iter().all(|directory| {
        fs::read_dir(root.join(directory))
            .is_ok_and(|entries| entries.flatten().all(|entry| entry.path().exists()))
    })
}

#[cfg(unix)]
fn link(target: &Path, link_path: &Path) -> Result<(), RenderRuntimeError> {
    std::os::unix::fs::symlink(target, link_path)
        .map_err(|error| materialize_error(error, link_path))
}

#[cfg(not(unix))]
fn link(target: &Path, link_path: &Path) -> Result<(), RenderRuntimeError> {
    fs::copy(target, link_path)
        .map(|_| ())
        .map_err(|error| materialize_error(error, link_path))
}

fn materialize_error(error: impl std::fmt::Display, path: &Path) -> RenderRuntimeError {
    RenderRuntimeError::new(
        "render.runtime.materialize_failed",
        "The bundled Linux render runtime could not be prepared.",
        format!("{}: {error}", path.display()),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn layout_for(temp: &Path) -> (PathBuf, LinuxRuntimeLayout) {
        let system = temp.join("system");
        fs::create_dir_all(&system).expect("system dir");
        for file in ["libgstcoreelements.so", "libgstfaad.so"] {
            fs::write(system.join(file), b"plugin").expect("plugin");
        }
        let scanner = system.join("gst-plugin-scanner");
        let core = system.join("libgstreamer-1.0.so.0");
        fs::write(&scanner, b"scanner").expect("scanner");
        fs::write(&core, b"core").expect("core");
        let bundle = temp.join("bundle");
        fs::create_dir_all(bundle.join("bundled-plugins")).expect("bundled dir");
        fs::create_dir_all(bundle.join("lib")).expect("lib dir");
        fs::write(bundle.join("bundled-plugins/libgstlibav.so"), b"libav").expect("libav");
        fs::write(bundle.join("lib/libavcodec.so.61"), b"avcodec").expect("avcodec");
        (
            bundle,
            LinuxRuntimeLayout {
                system_plugin_directory: system,
                system_plugin_files: vec![
                    "libgstcoreelements.so".to_string(),
                    "libgstmissing.so".to_string(),
                ],
                bundled_plugin_directory: "bundled-plugins".to_string(),
                bundled_plugin_files: vec!["libgstlibav.so".to_string()],
                scanner,
                gstreamer_core_library: core,
            },
        )
    }

    #[test]
    fn materializes_only_listed_plugins_and_bundled_libraries() {
        let temp = tempfile::tempdir().expect("temp");
        let (bundle, layout) = layout_for(temp.path());
        let destination = temp.path().join("support/render-runtime/linux-test");

        let root = materialize(&bundle, &layout, b"{}", &destination).expect("materialized");

        assert!(super::super::runtime_layout_is_complete(&root));
        assert!(root.join("plugins/libgstcoreelements.so").is_file());
        assert!(root.join("plugins/libgstlibav.so").is_file());
        assert!(!root.join("plugins/libgstfaad.so").exists());
        assert!(!root.join("plugins/libgstmissing.so").exists());
        assert!(root.join("lib/libavcodec.so.61").is_file());
        assert!(root.join("lib/libgstreamer-1.0.so.0").is_file());
        assert_eq!(
            materialize(&bundle, &layout, b"{}", &destination).expect("reused"),
            root
        );
    }

    #[test]
    fn rejects_manifest_file_names_that_escape_the_plugin_directory() {
        let temp = tempfile::tempdir().expect("temp");
        let (bundle, mut layout) = layout_for(temp.path());
        layout.system_plugin_files = vec!["../libgstfaad.so".to_string()];

        assert!(materialize(&bundle, &layout, b"{}", &temp.path().join("out")).is_err());
    }

    #[test]
    fn rejects_missing_system_scanner_instead_of_linking_dangling_paths() {
        let temp = tempfile::tempdir().expect("temp");
        let (bundle, mut layout) = layout_for(temp.path());
        layout.scanner = temp.path().join("missing-scanner");

        assert!(materialize(&bundle, &layout, b"{}", &temp.path().join("out")).is_err());
        assert!(!temp.path().join("out").exists());
    }

    #[test]
    fn keeps_a_completed_runtime_when_another_launch_finished_first() {
        let temp = tempfile::tempdir().expect("temp");
        let (bundle, layout) = layout_for(temp.path());
        let destination = temp.path().join("support/render-runtime/linux-test");
        let first = materialize(&bundle, &layout, b"{}", &destination).expect("first");
        let marker = first.join("plugins/libgstlibav.so");
        let identity = fs::symlink_metadata(&marker).expect("link").modified().ok();

        let second = materialize(&bundle, &layout, b"{}", &destination).expect("second");

        assert_eq!(first, second);
        assert_eq!(
            fs::symlink_metadata(&marker).expect("link").modified().ok(),
            identity
        );
    }
}
