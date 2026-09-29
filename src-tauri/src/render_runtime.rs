use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::env;
#[cfg(all(feature = "ges-render", unix))]
use std::ffi::c_void;
use std::ffi::OsString;
use std::fmt;
use std::fs;
use std::path::{Path, PathBuf};
#[cfg(feature = "ges-render")]
use std::sync::{Arc, Mutex, OnceLock};

mod linux_bundle;

pub const RENDER_RUNTIME_ROOT_ENV: &str = "VIDEO_CREATER_RENDER_RUNTIME_ROOT";
pub const APP_SUPPORT_ROOT_ENV: &str = "VIDEO_CREATER_APP_SUPPORT_DIR";
/// Target triple used for development sidecar names in `src-tauri/binaries`.
#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
pub const HOST_TARGET_TRIPLE: &str = "aarch64-apple-darwin";
#[cfg(all(target_os = "macos", target_arch = "x86_64"))]
pub const HOST_TARGET_TRIPLE: &str = "x86_64-apple-darwin";
#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
pub const HOST_TARGET_TRIPLE: &str = "x86_64-unknown-linux-gnu";
#[cfg(all(target_os = "linux", target_arch = "aarch64"))]
pub const HOST_TARGET_TRIPLE: &str = "aarch64-unknown-linux-gnu";
#[cfg(not(any(
    all(
        target_os = "macos",
        any(target_arch = "aarch64", target_arch = "x86_64")
    ),
    all(
        target_os = "linux",
        any(target_arch = "aarch64", target_arch = "x86_64")
    )
)))]
pub const HOST_TARGET_TRIPLE: &str = "unsupported";
/// Directory below `usr/lib` where Linux bundles install Tauri resources (the product name).
pub const LINUX_PRODUCT_RESOURCE_DIR: &str = "Video Creater";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum RenderRuntimeSource {
    Bundled,
    DevelopmentConfigured,
    DevelopmentSystem,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RenderRuntimeEnvironment {
    pub source: RenderRuntimeSource,
    pub root: PathBuf,
    pub plugin_path: PathBuf,
    pub scanner_path: PathBuf,
    pub manifest_path: PathBuf,
    pub manifest_hash: String,
    pub registry_path: PathBuf,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RenderRuntimeReadiness {
    Pending,
    Ready,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RenderRuntimeError {
    pub code: &'static str,
    pub message: String,
    pub detail: String,
}

impl RenderRuntimeError {
    pub(crate) fn new(
        code: &'static str,
        message: impl Into<String>,
        detail: impl Into<String>,
    ) -> Self {
        Self {
            code,
            message: message.into(),
            detail: detail.into(),
        }
    }
}

impl fmt::Display for RenderRuntimeError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "{}: {} ({})",
            self.code, self.message, self.detail
        )
    }
}

impl std::error::Error for RenderRuntimeError {}

#[derive(Debug, Clone, Default)]
pub struct RenderRuntimeResolutionInputs {
    pub resource_dir: Option<PathBuf>,
    pub configured_root: Option<PathBuf>,
    pub system_plugin_paths: Vec<PathBuf>,
    pub loaded_gstreamer_library_path: Option<PathBuf>,
    pub executable_path: Option<PathBuf>,
    pub development: bool,
}

pub fn resolve_render_runtime() -> Result<RenderRuntimeEnvironment, RenderRuntimeError> {
    let executable_path = env::current_exe().ok();
    let resource_dir = executable_path
        .as_deref()
        .and_then(packaged_resource_dir_from_executable);
    let configured_root = env::var_os(RENDER_RUNTIME_ROOT_ENV).map(PathBuf::from);
    // Linux distributions ship GPL plugins beside the LGPL ones, so an unfiltered system
    // plugin directory is never used; Linux requires the staged, license-filtered runtime.
    let system_plugin_paths = if cfg!(target_os = "linux") {
        Vec::new()
    } else {
        configured_system_plugin_paths(
            env::var_os("GST_PLUGIN_SYSTEM_PATH_1_0"),
            env::var_os("GST_PLUGIN_PATH_1_0"),
        )
    };
    resolve_render_runtime_with(&RenderRuntimeResolutionInputs {
        resource_dir,
        configured_root,
        system_plugin_paths,
        loaded_gstreamer_library_path: loaded_gstreamer_library_path(),
        executable_path,
        development: cfg!(debug_assertions),
    })
}

pub fn resolve_render_runtime_with(
    inputs: &RenderRuntimeResolutionInputs,
) -> Result<RenderRuntimeEnvironment, RenderRuntimeError> {
    if let Some(root) = bundled_runtime_root(inputs) {
        if runtime_layout_is_complete(&root) {
            return environment_for_root(RenderRuntimeSource::Bundled, root, None);
        }
        if let Some(materialized) = linux_bundle::materialize_if_packaged(&root)? {
            return environment_for_root(RenderRuntimeSource::Bundled, materialized, None);
        }
    }

    if !inputs.development {
        return Err(RenderRuntimeError::new(
            "render.runtime.bundled_missing",
            "The required bundled GStreamer/GES runtime is unavailable.",
            "Reinstall this build with its signed render-runtime resource.",
        ));
    }

    if let Some(root) = inputs.configured_root.clone() {
        if runtime_layout_is_complete(&root) {
            require_matching_loaded_runtime(
                &root,
                inputs.loaded_gstreamer_library_path.as_deref(),
            )?;
            return environment_for_root(RenderRuntimeSource::DevelopmentConfigured, root, None);
        }
        return Err(RenderRuntimeError::new(
            "render.runtime.development_invalid",
            "The configured development render runtime is incomplete.",
            format!(
                "{} must contain manifest.json, plugins, and libexec/gst-plugin-scanner.",
                root.display()
            ),
        ));
    }

    for plugin_path in &inputs.system_plugin_paths {
        if plugin_path.as_os_str().is_empty() {
            continue;
        }
        if plugin_path.is_dir() {
            let (root, scanner_path) = system_runtime_layout(plugin_path);
            if scanner_path.is_file() {
                if require_matching_loaded_runtime(
                    &root,
                    inputs.loaded_gstreamer_library_path.as_deref(),
                )
                .is_err()
                {
                    continue;
                }
                return environment_for_root(
                    RenderRuntimeSource::DevelopmentSystem,
                    root,
                    Some((plugin_path.clone(), scanner_path)),
                );
            }
        }
    }

    let staged_development_root =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("resources/render-runtime");
    if runtime_layout_is_complete(&staged_development_root) {
        require_matching_loaded_runtime(
            &staged_development_root,
            inputs.loaded_gstreamer_library_path.as_deref(),
        )?;
        return environment_for_root(
            RenderRuntimeSource::DevelopmentConfigured,
            staged_development_root,
            None,
        );
    }

    Err(RenderRuntimeError::new(
        "render.runtime.development_missing",
        "No development GStreamer/GES runtime could be resolved.",
        format!(
            "Build src-tauri/resources/render-runtime or set {RENDER_RUNTIME_ROOT_ENV} to a reviewed runtime root."
        ),
    ))
}

#[cfg(feature = "ges-render")]
#[derive(Debug, Clone, PartialEq, Eq, Default)]
enum RenderRuntimeStatus {
    #[default]
    Unconfigured,
    Pending {
        environment: RenderRuntimeEnvironment,
        initialization_started: bool,
    },
    Ready {
        environment: RenderRuntimeEnvironment,
    },
    Failed {
        error: RenderRuntimeError,
    },
}

#[cfg(feature = "ges-render")]
#[derive(Default)]
struct RenderRuntimeStartupState {
    status: Mutex<RenderRuntimeStatus>,
}

#[cfg(feature = "ges-render")]
impl RenderRuntimeStartupState {
    fn start_with(
        &self,
        initializer: impl FnOnce() -> Result<RenderRuntimeEnvironment, RenderRuntimeError>,
    ) -> Result<RenderRuntimeEnvironment, RenderRuntimeError> {
        let mut status = self
            .status
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        match &*status {
            RenderRuntimeStatus::Ready { environment } => return Ok(environment.clone()),
            RenderRuntimeStatus::Failed { error } => return Err(error.clone()),
            RenderRuntimeStatus::Pending { .. } => {
                return Err(initializing_error());
            }
            RenderRuntimeStatus::Unconfigured => {}
        }
        match initializer() {
            Ok(environment) => {
                *status = RenderRuntimeStatus::Ready {
                    environment: environment.clone(),
                };
                Ok(environment)
            }
            Err(error) => {
                *status = RenderRuntimeStatus::Failed {
                    error: error.clone(),
                };
                Err(error)
            }
        }
    }

    fn configure_with(
        &self,
        resolver: impl FnOnce() -> Result<RenderRuntimeEnvironment, RenderRuntimeError>,
    ) -> Result<RenderRuntimeEnvironment, RenderRuntimeError> {
        let mut status = self
            .status
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        match &*status {
            RenderRuntimeStatus::Pending { environment, .. }
            | RenderRuntimeStatus::Ready { environment } => return Ok(environment.clone()),
            RenderRuntimeStatus::Failed { error } => return Err(error.clone()),
            RenderRuntimeStatus::Unconfigured => {}
        }
        match resolver() {
            Ok(environment) => {
                *status = RenderRuntimeStatus::Pending {
                    environment: environment.clone(),
                    initialization_started: false,
                };
                Ok(environment)
            }
            Err(error) => {
                *status = RenderRuntimeStatus::Failed {
                    error: error.clone(),
                };
                Err(error)
            }
        }
    }

    fn begin_background_with<F>(self: &Arc<Self>, initializer: F) -> Result<(), RenderRuntimeError>
    where
        F: FnOnce(RenderRuntimeEnvironment) -> Result<(), RenderRuntimeError> + Send + 'static,
    {
        let environment =
            {
                let mut status = self
                    .status
                    .lock()
                    .unwrap_or_else(|error| error.into_inner());
                match &mut *status {
                    RenderRuntimeStatus::Pending {
                        environment,
                        initialization_started,
                    } => {
                        if *initialization_started {
                            return Ok(());
                        }
                        *initialization_started = true;
                        environment.clone()
                    }
                    RenderRuntimeStatus::Ready { .. } => return Ok(()),
                    RenderRuntimeStatus::Failed { error } => return Err(error.clone()),
                    RenderRuntimeStatus::Unconfigured => return Err(RenderRuntimeError::new(
                        "render.runtime.startup_required",
                        "The render runtime was not configured before background initialization.",
                        "Call prepare_desktop_render_runtime before constructing Tauri.",
                    )),
                }
            };
        let state = Arc::clone(self);
        std::thread::spawn(move || {
            let result = initializer(environment.clone());
            let mut status = state
                .status
                .lock()
                .unwrap_or_else(|error| error.into_inner());
            *status = match result {
                Ok(()) => RenderRuntimeStatus::Ready { environment },
                Err(error) => RenderRuntimeStatus::Failed { error },
            };
        });
        Ok(())
    }

    fn current(&self) -> Result<RenderRuntimeEnvironment, RenderRuntimeError> {
        match self.status() {
            RenderRuntimeStatus::Ready { environment } => Ok(environment),
            RenderRuntimeStatus::Failed { error } => Err(error),
            RenderRuntimeStatus::Pending { .. } => Err(initializing_error()),
            RenderRuntimeStatus::Unconfigured => Err(RenderRuntimeError::new(
                "render.runtime.startup_required",
                "The render runtime was not initialized during application startup.",
                "Start the required render runtime before creating workers or render consumers.",
            )),
        }
    }

    fn status(&self) -> RenderRuntimeStatus {
        self.status
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .clone()
    }
}

#[cfg(feature = "ges-render")]
static STARTED_RUNTIME: OnceLock<Arc<RenderRuntimeStartupState>> = OnceLock::new();

#[cfg(feature = "ges-render")]
fn started_runtime() -> &'static Arc<RenderRuntimeStartupState> {
    STARTED_RUNTIME.get_or_init(|| Arc::new(RenderRuntimeStartupState::default()))
}

#[cfg(feature = "ges-render")]
pub fn start_render_runtime() -> Result<RenderRuntimeEnvironment, RenderRuntimeError> {
    started_runtime().start_with(start_render_runtime_once)
}

#[cfg(feature = "ges-render")]
pub fn start_render_process_runtime() -> Result<RenderRuntimeEnvironment, RenderRuntimeError> {
    start_render_runtime()
}

#[cfg(not(feature = "ges-render"))]
pub fn start_render_process_runtime() -> Result<RenderRuntimeEnvironment, RenderRuntimeError> {
    Err(RenderRuntimeError::new(
        "render.runtime.feature_disabled",
        "The required GStreamer/GES render runtime is disabled in this build.",
        "Rebuild with the ges-render feature.",
    ))
}

#[cfg(feature = "ges-render")]
pub fn render_runtime_environment() -> Result<RenderRuntimeEnvironment, RenderRuntimeError> {
    started_runtime().current()
}

#[cfg(feature = "ges-render")]
pub fn render_runtime_configuration() -> Result<RenderRuntimeEnvironment, RenderRuntimeError> {
    match started_runtime().status() {
        RenderRuntimeStatus::Pending { environment, .. }
        | RenderRuntimeStatus::Ready { environment } => Ok(environment),
        RenderRuntimeStatus::Failed { error } => Err(error),
        RenderRuntimeStatus::Unconfigured => Err(RenderRuntimeError::new(
            "render.runtime.startup_required",
            "The render runtime was not configured during application startup.",
            "Configure the required render runtime before checking or consuming it.",
        )),
    }
}

#[cfg(feature = "ges-render")]
pub fn render_runtime_readiness() -> Result<RenderRuntimeReadiness, RenderRuntimeError> {
    match started_runtime().status() {
        RenderRuntimeStatus::Pending { .. } => Ok(RenderRuntimeReadiness::Pending),
        RenderRuntimeStatus::Ready { .. } => Ok(RenderRuntimeReadiness::Ready),
        RenderRuntimeStatus::Failed { error } => Err(error),
        RenderRuntimeStatus::Unconfigured => Err(RenderRuntimeError::new(
            "render.runtime.startup_required",
            "The render runtime was not configured during application startup.",
            "Configure the required render runtime before checking or consuming it.",
        )),
    }
}

#[cfg(not(feature = "ges-render"))]
pub fn render_runtime_readiness() -> Result<RenderRuntimeReadiness, RenderRuntimeError> {
    Err(RenderRuntimeError::new(
        "render.runtime.feature_disabled",
        "The required GStreamer/GES render runtime is disabled in this build.",
        "Rebuild with the ges-render feature.",
    ))
}

#[cfg(feature = "ges-render")]
pub fn prepare_desktop_render_runtime() -> Result<RenderRuntimeEnvironment, RenderRuntimeError> {
    started_runtime().configure_with(prepare_render_runtime_environment)
}

#[cfg(not(feature = "ges-render"))]
pub fn prepare_desktop_render_runtime() -> Result<RenderRuntimeEnvironment, RenderRuntimeError> {
    Err(RenderRuntimeError::new(
        "render.runtime.feature_disabled",
        "The required GStreamer/GES render runtime is disabled in this build.",
        "Rebuild with the ges-render feature.",
    ))
}

#[cfg(feature = "ges-render")]
pub fn begin_desktop_render_runtime_initialization() -> Result<(), RenderRuntimeError> {
    started_runtime().begin_background_with(initialize_prepared_render_runtime)
}

#[cfg(not(feature = "ges-render"))]
pub fn begin_desktop_render_runtime_initialization() -> Result<(), RenderRuntimeError> {
    Err(RenderRuntimeError::new(
        "render.runtime.feature_disabled",
        "The required GStreamer/GES render runtime is disabled in this build.",
        "Rebuild with the ges-render feature.",
    ))
}

#[cfg(feature = "ges-render")]
fn start_render_runtime_once() -> Result<RenderRuntimeEnvironment, RenderRuntimeError> {
    let environment = prepare_render_runtime_environment()?;
    initialize_prepared_render_runtime(environment.clone())?;
    Ok(environment)
}

#[cfg(feature = "ges-render")]
fn prepare_render_runtime_environment() -> Result<RenderRuntimeEnvironment, RenderRuntimeError> {
    let environment = resolve_render_runtime().inspect_err(|_| fail_closed_plugin_environment())?;
    apply_render_runtime_environment(&environment)
        .inspect_err(|_| fail_closed_plugin_environment())?;
    Ok(environment)
}

/// Without the license-filtered runtime, GStreamer in this process, the webview, and inherited
/// helpers would scan distribution plugin directories that contain GPL plugins. Linux therefore
/// exposes no plugins at all when the runtime cannot be prepared.
#[cfg(feature = "ges-render")]
fn fail_closed_plugin_environment() {
    if cfg!(target_os = "linux") {
        env::set_var("GST_PLUGIN_SYSTEM_PATH_1_0", "");
        env::set_var("GST_PLUGIN_PATH_1_0", "");
    }
}

/// GStreamer environment variables that must not leak into unrelated desktop programs started
/// by the app, such as the file manager.
pub const APP_GSTREAMER_ENVIRONMENT_VARIABLES: [&str; 5] = [
    "GST_PLUGIN_SYSTEM_PATH_1_0",
    "GST_PLUGIN_PATH_1_0",
    "GST_PLUGIN_SCANNER",
    "GST_REGISTRY_1_0",
    "GST_REGISTRY",
];

#[cfg(feature = "ges-render")]
fn initialize_prepared_render_runtime(
    _environment: RenderRuntimeEnvironment,
) -> Result<(), RenderRuntimeError> {
    gstreamer::init().map_err(|error| {
        RenderRuntimeError::new(
            "render.runtime.gstreamer_init_failed",
            "GStreamer could not be initialized from the selected render runtime.",
            error.to_string(),
        )
    })?;
    gstreamer_editing_services::init().map_err(|error| {
        RenderRuntimeError::new(
            "render.runtime.ges_init_failed",
            "GStreamer Editing Services could not be initialized.",
            error.to_string(),
        )
    })?;
    if cfg!(target_os = "linux") {
        apply_linux_codec_ranks();
    }
    Ok(())
}

/// gst-libav registers FFmpeg encoders with rank NONE, which encodebin never auto-plugs, and
/// OpenH264 decodes only the constrained baseline profile. Reviewed Linux codecs are ranked so
/// automatic selection matches the reviewed codec set.
#[cfg(feature = "ges-render")]
fn apply_linux_codec_ranks() {
    use gstreamer::prelude::PluginFeatureExtManual;
    for (factory_name, rank) in [
        ("avenc_aac", gstreamer::Rank::PRIMARY),
        ("avenc_prores_ks", gstreamer::Rank::PRIMARY),
        ("voaacenc", gstreamer::Rank::NONE),
        ("openh264dec", gstreamer::Rank::NONE),
    ] {
        if let Some(factory) = gstreamer::ElementFactory::find(factory_name) {
            factory.set_rank(rank);
        }
    }
}

#[cfg(feature = "ges-render")]
fn initializing_error() -> RenderRuntimeError {
    RenderRuntimeError::new(
        "render.runtime.initializing",
        "The required GStreamer/GES runtime is still initializing.",
        "Wait for render-system health to report Ready before starting a render.",
    )
}

#[cfg(feature = "ges-render")]
fn apply_render_runtime_environment(
    environment: &RenderRuntimeEnvironment,
) -> Result<(), RenderRuntimeError> {
    if let Some(parent) = environment.registry_path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            RenderRuntimeError::new(
                "render.runtime.registry_unavailable",
                "The render runtime registry directory could not be created.",
                format!("{}: {error}", parent.display()),
            )
        })?;
    }
    env::set_var("GST_PLUGIN_SYSTEM_PATH_1_0", "");
    env::set_var("GST_PLUGIN_PATH_1_0", &environment.plugin_path);
    env::set_var("GST_PLUGIN_SCANNER", &environment.scanner_path);
    env::set_var("GST_REGISTRY_1_0", &environment.registry_path);
    env::set_var("GST_REGISTRY", &environment.registry_path);
    Ok(())
}

fn bundled_runtime_root(inputs: &RenderRuntimeResolutionInputs) -> Option<PathBuf> {
    inputs
        .resource_dir
        .as_ref()
        .map(|resource_dir| resource_dir.join("render-runtime"))
        .or_else(|| {
            inputs
                .executable_path
                .as_deref()
                .and_then(packaged_resource_dir_from_executable)
                .map(|resource_dir| resource_dir.join("render-runtime"))
        })
}

pub(crate) fn packaged_resource_dir_from_executable(executable_path: &Path) -> Option<PathBuf> {
    let executable_dir = executable_path.parent()?;
    if executable_dir
        .file_name()
        .is_some_and(|name| name == "MacOS")
    {
        return executable_dir
            .parent()
            .map(|contents| contents.join("Resources"));
    }
    if let Some(linux_resources) = linux_packaged_resource_dir(executable_dir) {
        return Some(linux_resources);
    }
    Some(executable_dir.join("resources"))
}

/// Tauri's deb and AppImage bundles install the executable in `usr/bin` and resources in
/// `usr/lib/<productName>`.
fn linux_packaged_resource_dir(executable_dir: &Path) -> Option<PathBuf> {
    if !cfg!(target_os = "linux") || executable_dir.file_name()? != "bin" {
        return None;
    }
    let resources = executable_dir
        .parent()?
        .join("lib")
        .join(LINUX_PRODUCT_RESOURCE_DIR);
    resources.is_dir().then_some(resources)
}

/// Locates the staged render runtime for helper processes that load GStreamer themselves.
/// Uses the same precedence as [`resolve_render_runtime`] without in-process identity checks.
pub fn helper_render_runtime_environment() -> Option<RenderRuntimeEnvironment> {
    let executable_path = env::current_exe().ok();
    let inputs = RenderRuntimeResolutionInputs {
        resource_dir: executable_path
            .as_deref()
            .and_then(packaged_resource_dir_from_executable),
        configured_root: env::var_os(RENDER_RUNTIME_ROOT_ENV).map(PathBuf::from),
        executable_path,
        development: cfg!(debug_assertions),
        ..RenderRuntimeResolutionInputs::default()
    };
    let staged_development_root = inputs
        .development
        .then(|| PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("resources/render-runtime"));
    let bundled_root = bundled_runtime_root(&inputs);
    if let Some(materialized) = bundled_root
        .as_deref()
        .filter(|root| !runtime_layout_is_complete(root))
        .and_then(|root| linux_bundle::materialize_if_packaged(root).ok().flatten())
    {
        return environment_for_root(RenderRuntimeSource::Bundled, materialized, None).ok();
    }
    bundled_root
        .into_iter()
        .chain(
            inputs
                .development
                .then(|| inputs.configured_root.clone())
                .flatten(),
        )
        .chain(staged_development_root)
        .find(|root| runtime_layout_is_complete(root))
        .and_then(|root| {
            let source = if Some(&root) == bundled_runtime_root(&inputs).as_ref() {
                RenderRuntimeSource::Bundled
            } else {
                RenderRuntimeSource::DevelopmentConfigured
            };
            environment_for_root(source, root, None).ok()
        })
}

fn runtime_layout_is_complete(root: &Path) -> bool {
    root.join("manifest.json").is_file()
        && root.join("plugins").is_dir()
        && root.join("libexec/gst-plugin-scanner").is_file()
}

fn require_matching_loaded_runtime(
    root: &Path,
    loaded_library_path: Option<&Path>,
) -> Result<(), RenderRuntimeError> {
    let loaded_library_path = loaded_library_path.ok_or_else(|| {
        RenderRuntimeError::new(
            "render.runtime.loaded_identity_unavailable",
            "The loaded GStreamer library identity could not be determined.",
            "Use a build whose linked GStreamer runtime can be resolved before startup.",
        )
    })?;
    let candidate = gstreamer_core_library_path(root).ok_or_else(|| {
        RenderRuntimeError::new(
            "render.runtime.development_invalid",
            "The development render runtime has no GStreamer core library.",
            format!("Expected libgstreamer in {}/lib.", root.display()),
        )
    })?;
    let loaded = loaded_library_path
        .canonicalize()
        .unwrap_or_else(|_| loaded_library_path.to_path_buf());
    let candidate = candidate
        .canonicalize()
        .unwrap_or_else(|_| candidate.clone());
    if loaded == candidate {
        return Ok(());
    }
    Err(RenderRuntimeError::new(
        "render.runtime.development_mismatch",
        "The selected development runtime does not match the GStreamer library loaded by this process.",
        format!(
            "loaded={}, candidate={}",
            loaded.display(),
            candidate.display()
        ),
    ))
}

fn gstreamer_core_library_path(root: &Path) -> Option<PathBuf> {
    [
        "lib/libgstreamer-1.0.0.dylib",
        "lib/libgstreamer-1.0.so.0",
        "lib/libgstreamer-1.0.so",
        "bin/libgstreamer-1.0-0.dll",
    ]
    .into_iter()
    .map(|relative| root.join(relative))
    .find(|path| path.is_file())
}

fn environment_for_root(
    source: RenderRuntimeSource,
    root: PathBuf,
    system_layout: Option<(PathBuf, PathBuf)>,
) -> Result<RenderRuntimeEnvironment, RenderRuntimeError> {
    let manifest_path = root.join("manifest.json");
    let (plugin_path, scanner_path) = system_layout.unwrap_or_else(|| {
        (
            root.join("plugins"),
            root.join("libexec/gst-plugin-scanner"),
        )
    });
    let manifest_hash = runtime_fingerprint(&manifest_path, &root)?;
    let registry_path = default_app_support_root()
        .join("render-runtime")
        .join(format!("registry-{manifest_hash}.bin"));
    Ok(RenderRuntimeEnvironment {
        source,
        root,
        plugin_path,
        scanner_path,
        manifest_path,
        manifest_hash,
        registry_path,
    })
}

fn runtime_fingerprint(manifest_path: &Path, root: &Path) -> Result<String, RenderRuntimeError> {
    let mut hasher = Sha256::new();
    if manifest_path.is_file() {
        let bytes = fs::read(manifest_path).map_err(|error| {
            RenderRuntimeError::new(
                "render.runtime.manifest_unreadable",
                "The render runtime manifest could not be read.",
                format!("{}: {error}", manifest_path.display()),
            )
        })?;
        hasher.update(bytes);
    } else {
        hasher.update(root.to_string_lossy().as_bytes());
    }
    Ok(format!("{:x}", hasher.finalize()))
}

fn default_app_support_root() -> PathBuf {
    if let Some(root) = env::var_os(APP_SUPPORT_ROOT_ENV) {
        return PathBuf::from(root);
    }
    #[cfg(target_os = "macos")]
    if let Some(home) = env::var_os("HOME") {
        return PathBuf::from(home).join("Library/Application Support/com.olhapi.video-creater");
    }
    #[cfg(target_os = "linux")]
    if let Some(root) = linux_data_home() {
        return root.join("com.olhapi.video-creater");
    }
    if let Some(root) = env::var_os("XDG_DATA_HOME") {
        return PathBuf::from(root).join("video-creater");
    }
    env::temp_dir().join("video-creater")
}

#[cfg(target_os = "linux")]
fn linux_data_home() -> Option<PathBuf> {
    env::var_os("XDG_DATA_HOME")
        .map(PathBuf::from)
        .filter(|path| path.is_absolute())
        .or_else(|| env::var_os("HOME").map(|home| PathBuf::from(home).join(".local/share")))
}

fn configured_system_plugin_paths(
    system_paths: Option<OsString>,
    plugin_paths: Option<OsString>,
) -> Vec<PathBuf> {
    system_paths
        .into_iter()
        .chain(plugin_paths)
        .flat_map(|paths| env::split_paths(&paths).collect::<Vec<_>>())
        .filter(|path| !path.as_os_str().is_empty())
        .chain(
            [
                "/opt/homebrew/lib/gstreamer-1.0",
                "/usr/local/lib/gstreamer-1.0",
                "/usr/lib/gstreamer-1.0",
                "/usr/lib/x86_64-linux-gnu/gstreamer-1.0",
                "/usr/lib/aarch64-linux-gnu/gstreamer-1.0",
            ]
            .into_iter()
            .map(PathBuf::from),
        )
        .collect()
}

fn system_runtime_layout(plugin_path: &Path) -> (PathBuf, PathBuf) {
    let resolved_plugin_path = plugin_path
        .canonicalize()
        .unwrap_or_else(|_| plugin_path.to_path_buf());
    let prefix = resolved_plugin_path
        .parent()
        .and_then(Path::parent)
        .unwrap_or(&resolved_plugin_path);
    (
        prefix.to_path_buf(),
        prefix.join("libexec/gstreamer-1.0/gst-plugin-scanner"),
    )
}

#[cfg(all(feature = "ges-render", unix))]
fn loaded_gstreamer_library_path() -> Option<PathBuf> {
    let mut info = std::mem::MaybeUninit::<libc::Dl_info>::zeroed();
    let address = gstreamer::ffi::gst_version as *const () as *const c_void;
    let found = unsafe { libc::dladdr(address, info.as_mut_ptr()) };
    if found == 0 {
        return None;
    }
    let info = unsafe { info.assume_init() };
    if info.dli_fname.is_null() {
        return None;
    }
    let path = unsafe { std::ffi::CStr::from_ptr(info.dli_fname) };
    Some(PathBuf::from(path.to_string_lossy().into_owned()))
}

#[cfg(any(not(feature = "ges-render"), not(unix)))]
fn loaded_gstreamer_library_path() -> Option<PathBuf> {
    None
}

#[cfg(test)]
mod tests {
    use super::{resolve_render_runtime_with, RenderRuntimeResolutionInputs, RenderRuntimeSource};
    use std::fs;
    use std::path::{Path, PathBuf};

    fn create_runtime(root: &Path) {
        fs::create_dir_all(root.join("plugins")).expect("plugins");
        fs::create_dir_all(root.join("libexec")).expect("libexec");
        fs::create_dir_all(root.join("lib")).expect("lib");
        fs::write(root.join("manifest.json"), b"{\"schemaVersion\":1}").expect("manifest");
        fs::write(root.join("libexec/gst-plugin-scanner"), b"scanner").expect("scanner");
        fs::write(root.join("lib/libgstreamer-1.0.0.dylib"), b"core").expect("core");
    }

    #[test]
    fn bundled_runtime_wins_over_development_configuration() {
        let temp = tempfile::tempdir().expect("temp");
        let bundled = temp.path().join("Resources/render-runtime");
        let configured = temp.path().join("configured");
        create_runtime(&bundled);
        create_runtime(&configured);

        let environment = resolve_render_runtime_with(&RenderRuntimeResolutionInputs {
            resource_dir: Some(temp.path().join("Resources")),
            configured_root: Some(configured),
            system_plugin_paths: Vec::new(),
            loaded_gstreamer_library_path: Some(bundled.join("lib/libgstreamer-1.0.0.dylib")),
            executable_path: Some(temp.path().join("Video Creater")),
            development: true,
        })
        .expect("bundled runtime");

        assert_eq!(environment.source, RenderRuntimeSource::Bundled);
        assert_eq!(environment.root, bundled);
        assert_eq!(environment.plugin_path, bundled.join("plugins"));
        assert_eq!(
            environment.scanner_path,
            bundled.join("libexec/gst-plugin-scanner")
        );
        assert_eq!(environment.manifest_path, bundled.join("manifest.json"));
    }

    #[test]
    fn development_configuration_is_used_when_bundle_is_absent() {
        let temp = tempfile::tempdir().expect("temp");
        let configured = temp.path().join("configured");
        create_runtime(&configured);

        let environment = resolve_render_runtime_with(&RenderRuntimeResolutionInputs {
            resource_dir: Some(temp.path().join("missing-resources")),
            configured_root: Some(configured.clone()),
            system_plugin_paths: Vec::new(),
            loaded_gstreamer_library_path: Some(configured.join("lib/libgstreamer-1.0.0.dylib")),
            executable_path: Some(PathBuf::from("/tmp/video-creater")),
            development: true,
        })
        .expect("configured runtime");

        assert_eq!(
            environment.source,
            RenderRuntimeSource::DevelopmentConfigured
        );
        assert_eq!(environment.root, configured);
    }

    #[test]
    fn development_system_runtime_precedes_the_repository_staging_bundle() {
        let temp = tempfile::tempdir().expect("temp");
        let prefix = temp.path().join("prefix");
        let plugins = prefix.join("lib/gstreamer-1.0");
        fs::create_dir_all(&plugins).expect("plugins");
        fs::create_dir_all(prefix.join("libexec/gstreamer-1.0")).expect("libexec");
        fs::create_dir_all(prefix.join("lib")).expect("lib");
        fs::write(
            prefix.join("libexec/gstreamer-1.0/gst-plugin-scanner"),
            b"scanner",
        )
        .expect("scanner");
        fs::write(prefix.join("lib/libgstreamer-1.0.0.dylib"), b"core").expect("core");

        let environment = resolve_render_runtime_with(&RenderRuntimeResolutionInputs {
            resource_dir: Some(temp.path().join("missing-resources")),
            configured_root: None,
            system_plugin_paths: vec![plugins.clone()],
            loaded_gstreamer_library_path: Some(prefix.join("lib/libgstreamer-1.0.0.dylib")),
            executable_path: Some(PathBuf::from("/tmp/video-creater")),
            development: true,
        })
        .expect("system runtime");

        assert_eq!(environment.source, RenderRuntimeSource::DevelopmentSystem);
        assert_eq!(environment.plugin_path, plugins);
    }

    #[test]
    fn mismatched_development_runtime_is_rejected() {
        let temp = tempfile::tempdir().expect("temp");
        let loaded = temp.path().join("loaded");
        let candidate = temp.path().join("candidate");
        create_runtime(&candidate);
        fs::create_dir_all(loaded.join("lib")).expect("loaded lib");
        fs::write(loaded.join("lib/libgstreamer-1.0.0.dylib"), b"loaded core")
            .expect("loaded core");
        fs::create_dir_all(candidate.join("lib")).expect("candidate lib");
        fs::write(
            candidate.join("lib/libgstreamer-1.0.0.dylib"),
            b"candidate core",
        )
        .expect("candidate core");

        let error = resolve_render_runtime_with(&RenderRuntimeResolutionInputs {
            resource_dir: Some(temp.path().join("missing-resources")),
            configured_root: Some(candidate),
            system_plugin_paths: Vec::new(),
            loaded_gstreamer_library_path: Some(loaded.join("lib/libgstreamer-1.0.0.dylib")),
            executable_path: Some(PathBuf::from("/tmp/video-creater")),
            development: true,
        })
        .expect_err("mismatched runtime must be rejected");

        assert_eq!(error.code, "render.runtime.development_mismatch");
    }

    #[test]
    fn plugin_path_lists_skip_empty_and_invalid_entries_before_matching_candidate() {
        let temp = tempfile::tempdir().expect("temp");
        let prefix = temp.path().join("matching");
        let plugins = prefix.join("lib/gstreamer-1.0");
        fs::create_dir_all(&plugins).expect("plugins");
        fs::create_dir_all(prefix.join("libexec/gstreamer-1.0")).expect("libexec");
        fs::create_dir_all(prefix.join("lib")).expect("lib");
        fs::write(
            prefix.join("libexec/gstreamer-1.0/gst-plugin-scanner"),
            b"scanner",
        )
        .expect("scanner");
        fs::write(prefix.join("lib/libgstreamer-1.0.0.dylib"), b"loaded core").expect("core");

        let environment = resolve_render_runtime_with(&RenderRuntimeResolutionInputs {
            resource_dir: Some(temp.path().join("missing-resources")),
            configured_root: None,
            system_plugin_paths: vec![PathBuf::new(), temp.path().join("invalid"), plugins.clone()],
            loaded_gstreamer_library_path: Some(prefix.join("lib/libgstreamer-1.0.0.dylib")),
            executable_path: Some(PathBuf::from("/tmp/video-creater")),
            development: true,
        })
        .expect("matching later entry");

        assert_eq!(environment.plugin_path, plugins);
    }

    #[test]
    fn both_gstreamer_path_variables_contribute_all_non_empty_entries() {
        let system = std::env::join_paths([
            PathBuf::new(),
            PathBuf::from("/first/system"),
            PathBuf::from("/second/system"),
        ])
        .expect("system paths");
        let plugin = std::env::join_paths([
            PathBuf::new(),
            PathBuf::from("/first/plugin"),
            PathBuf::from("/second/plugin"),
        ])
        .expect("plugin paths");

        let paths = super::configured_system_plugin_paths(Some(system), Some(plugin));

        assert_eq!(
            &paths[..4],
            &[
                PathBuf::from("/first/system"),
                PathBuf::from("/second/system"),
                PathBuf::from("/first/plugin"),
                PathBuf::from("/second/plugin"),
            ]
        );
    }

    #[cfg(feature = "ges-render")]
    #[test]
    fn concurrent_consumers_observe_one_complete_immutable_startup_environment() {
        use super::{RenderRuntimeEnvironment, RenderRuntimeStartupState};
        use std::sync::atomic::{AtomicUsize, Ordering};
        use std::sync::Arc;

        let temp = tempfile::tempdir().expect("temp");
        let expected = RenderRuntimeEnvironment {
            source: RenderRuntimeSource::Bundled,
            root: temp.path().join("runtime"),
            plugin_path: temp.path().join("runtime/plugins"),
            scanner_path: temp.path().join("runtime/libexec/gst-plugin-scanner"),
            manifest_path: temp.path().join("runtime/manifest.json"),
            manifest_hash: "immutable".to_string(),
            registry_path: temp.path().join("registry.bin"),
        };
        let state = Arc::new(RenderRuntimeStartupState::default());
        let initialization_count = Arc::new(AtomicUsize::new(0));
        let threads = (0..8)
            .map(|_| {
                let state = Arc::clone(&state);
                let initialization_count = Arc::clone(&initialization_count);
                let expected = expected.clone();
                std::thread::spawn(move || {
                    state
                        .start_with(|| {
                            initialization_count.fetch_add(1, Ordering::SeqCst);
                            Ok(expected)
                        })
                        .expect("startup")
                })
            })
            .collect::<Vec<_>>();

        for thread in threads {
            assert_eq!(thread.join().expect("consumer"), expected);
        }
        assert_eq!(initialization_count.load(Ordering::SeqCst), 1);
        for _ in 0..8 {
            assert_eq!(state.current().expect("immutable current"), expected);
        }
        assert_eq!(
            initialization_count.load(Ordering::SeqCst),
            1,
            "consumers must not rerun environment application after startup"
        );
    }

    #[cfg(feature = "ges-render")]
    #[test]
    fn startup_failure_is_recorded_immutably_for_health_and_render_consumers() {
        use super::{RenderRuntimeError, RenderRuntimeStartupState};
        use std::sync::atomic::{AtomicUsize, Ordering};

        let state = std::sync::Arc::new(RenderRuntimeStartupState::default());
        let attempts = AtomicUsize::new(0);
        let first = state.start_with(|| {
            attempts.fetch_add(1, Ordering::SeqCst);
            Err(RenderRuntimeError::new(
                "render.runtime.test_missing",
                "Render runtime is missing.",
                "fixture",
            ))
        });
        let second = state.start_with(|| {
            attempts.fetch_add(1, Ordering::SeqCst);
            panic!("recorded startup failure must not be retried")
        });

        assert_eq!(
            first.expect_err("first startup failure").code,
            "render.runtime.test_missing"
        );
        assert_eq!(
            second.expect_err("recorded startup failure").code,
            "render.runtime.test_missing"
        );
        assert_eq!(
            state
                .current()
                .expect_err("health sees startup failure")
                .code,
            "render.runtime.test_missing"
        );
        assert_eq!(attempts.load(Ordering::SeqCst), 1);
    }

    #[cfg(feature = "ges-render")]
    #[test]
    fn desktop_pending_initialization_never_blocks_consumers_or_ui_progress() {
        use super::{RenderRuntimeStartupState, RenderRuntimeStatus};
        use std::sync::mpsc;
        use std::time::{Duration, Instant};

        let temp = tempfile::tempdir().expect("temp");
        let environment = super::environment_for_root(
            RenderRuntimeSource::DevelopmentConfigured,
            temp.path().to_path_buf(),
            Some((
                temp.path().join("plugins"),
                temp.path().join("gst-plugin-scanner"),
            )),
        )
        .expect("environment");
        let state = std::sync::Arc::new(RenderRuntimeStartupState::default());
        state
            .configure_with(|| Ok(environment.clone()))
            .expect("desktop configuration");
        let (release_sender, release_receiver) = mpsc::channel::<()>();
        state
            .begin_background_with(move |_| {
                let _ = release_receiver.recv();
                Ok(())
            })
            .expect("background initialization started");

        let started = Instant::now();
        let error = state
            .current()
            .expect_err("consumer must fail fast while initialization is pending");
        let status = state.status();
        let ui_reached = true;

        assert!(ui_reached);
        assert!(started.elapsed() < Duration::from_millis(100));
        assert_eq!(error.code, "render.runtime.initializing");
        assert!(matches!(status, RenderRuntimeStatus::Pending { .. }));
        release_sender.send(()).expect("release initializer");
    }

    #[test]
    fn desktop_records_render_startup_before_tauri_without_panicking() {
        let main_source = include_str!("main.rs");
        let helper_mode = main_source
            .find("render_health_probe_mode_requested()")
            .expect("early render health helper mode");
        let startup = main_source
            .find("prepare_desktop_render_runtime()")
            .expect("desktop configuration call");
        let tauri_builder = main_source
            .find("tauri::Builder::default()")
            .expect("Tauri builder");

        assert!(helper_mode < startup);
        assert!(startup < tauri_builder);
        let desktop_startup = &main_source[startup..tauri_builder];
        assert!(!desktop_startup.contains("panic!"));
        assert!(desktop_startup.contains("prepare_desktop_render_runtime"));
        assert!(
            main_source[tauri_builder..].contains("begin_desktop_render_runtime_initialization")
        );
    }

    #[test]
    fn every_in_process_render_binary_starts_the_shared_runtime() {
        for (name, source) in [
            (
                "render-proposal",
                include_str!("bin/video-creater-render-proposal.rs"),
            ),
            (
                "render-template",
                include_str!("bin/video-creater-render-template.rs"),
            ),
            (
                "hyperframe-evidence",
                include_str!("bin/video-creater-hyperframe-evidence.rs"),
            ),
            (
                "native-export-evidence",
                include_str!("bin/video-creater-native-export-evidence.rs"),
            ),
            (
                "combined-e2e",
                include_str!("bin/video-creater-e2e-combined.rs"),
            ),
            (
                "compatibility-evidence",
                include_str!("bin/video-creater-compatibility-evidence.rs"),
            ),
            (
                "provider-app-e2e",
                include_str!("bin/video-creater-provider-app-e2e.rs"),
            ),
            (
                "provider-e2e",
                include_str!("bin/video-creater-provider-e2e.rs"),
            ),
            (
                "temporal-worker",
                include_str!("bin/video-creater-temporal-worker.rs"),
            ),
        ] {
            assert!(
                source.contains("start_render_process_runtime"),
                "{name} must initialize the shared in-process render runtime"
            );
        }

        let compatibility_worker = include_str!("../crates/compatibility-worker/src/main.rs");
        assert!(
            !compatibility_worker.contains("start_render_process_runtime"),
            "the isolated compatibility worker must retain its own runtime"
        );
    }

    #[test]
    fn release_resolution_rejects_development_configuration() {
        let temp = tempfile::tempdir().expect("temp");
        let configured = temp.path().join("configured");
        create_runtime(&configured);

        let error = resolve_render_runtime_with(&RenderRuntimeResolutionInputs {
            resource_dir: Some(temp.path().join("missing-resources")),
            configured_root: Some(configured),
            system_plugin_paths: Vec::new(),
            loaded_gstreamer_library_path: None,
            executable_path: Some(PathBuf::from("/tmp/video-creater")),
            development: false,
        })
        .expect_err("release must require bundled runtime");

        assert_eq!(error.code, "render.runtime.bundled_missing");
        assert!(!error.detail.contains("Homebrew"));
        assert!(!error.detail.contains("/usr/local"));
    }
}
