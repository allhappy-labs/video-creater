#[cfg(target_os = "macos")]
use crate::render_pipeline::avfoundation_backend::{
    AvFoundationCapabilities, AVFOUNDATION_EXPORT_PROTOCOL, AVFOUNDATION_EXPORT_PROTOCOL_VERSION,
};
#[cfg(any(feature = "ges-render", test))]
use crate::render_pipeline::plugin_policy::{
    evaluate_gstreamer_factory, GstFactoryInfo, PluginPolicyVerdict,
};
#[cfg(feature = "ges-render")]
use crate::render_runtime::render_runtime_environment;
use crate::render_runtime::RenderRuntimeError;
#[cfg(feature = "ges-render")]
use crate::render_runtime::{
    render_runtime_readiness, RenderRuntimeEnvironment, RenderRuntimeReadiness, RenderRuntimeSource,
};
use crate::settings::health::{SettingsComponentHealth, SettingsHealthState};
use chrono::{SecondsFormat, Utc};
#[cfg(feature = "ges-render")]
use gstreamer as gst;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::env;
#[cfg(feature = "ges-render")]
use std::fs;
use std::io::Write;
#[cfg(target_os = "macos")]
use std::path::Path;
use std::path::PathBuf;
use std::process::{Child, Command, Output, Stdio};
use std::thread;
use std::time::{Duration, Instant};
use video_creater_compatibility_protocol::{
    CompatibilityOperation, CompatibilityProfile, CompatibilityRequest, CompatibilityResult,
    WorkerBudgets, WorkerEvent, COMPATIBILITY_PROTOCOL_NAME, COMPATIBILITY_PROTOCOL_VERSION,
};

const RENDER_PROBE_TIMEOUT: Duration = Duration::from_secs(3);
const RENDER_HEALTH_TARGET_ID: &str = "render-system";
const RENDER_HEALTH_PROBE_ARG: &str = "--render-health-probe";
#[cfg(feature = "ges-render")]
const RENDER_HEALTH_PROBE_EXECUTABLE_ENV: &str = "VIDEO_CREATER_RENDER_HEALTH_PROBE_EXECUTABLE";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct RenderSystemHealth {
    pub checked_at: String,
    pub state: SettingsHealthState,
    pub composition_ready: bool,
    pub native_delivery_degraded: bool,
    pub compatibility_degraded: bool,
    pub items: Vec<SettingsComponentHealth>,
}

#[derive(Debug, Clone)]
struct RenderProbeFailure {
    state: SettingsHealthState,
    code: String,
    summary: String,
    detail: String,
    provenance: BTreeMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "status", rename_all = "camelCase")]
enum RenderHealthProbePayload {
    Ready {
        gstreamer: SettingsComponentHealth,
        plugin_policy: Box<SettingsComponentHealth>,
    },
    Failed {
        code: String,
        message: String,
        detail: String,
    },
}

#[cfg(any(feature = "ges-render", test))]
#[derive(Debug, Deserialize)]
struct RenderHealthProbeErrorPayload {
    code: String,
    message: String,
    detail: String,
}

impl RenderProbeFailure {
    fn failed(
        code: impl Into<String>,
        summary: impl Into<String>,
        detail: impl Into<String>,
    ) -> Self {
        Self {
            state: SettingsHealthState::Failed,
            code: code.into(),
            summary: summary.into(),
            detail: detail.into(),
            provenance: BTreeMap::new(),
        }
    }

    fn provenance(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.provenance.insert(key.into(), value.into());
        self
    }
}

#[cfg(feature = "ges-render")]
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RenderRuntimeManifest {
    gstreamer_version: String,
    ges_version: String,
    #[serde(default)]
    plugins: Vec<String>,
    #[serde(default)]
    factories: Vec<RenderRuntimeFactory>,
}

#[cfg(feature = "ges-render")]
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RenderRuntimeFactory {
    name: String,
    plugin_name: Option<String>,
    package: Option<String>,
    license: Option<String>,
}

#[cfg(feature = "ges-render")]
impl From<RenderRuntimeFactory> for GstFactoryInfo {
    fn from(factory: RenderRuntimeFactory) -> Self {
        GstFactoryInfo {
            name: factory.name,
            plugin_name: factory.plugin_name,
            package: factory.package,
            license: factory.license,
        }
    }
}

pub fn get_render_system_health() -> RenderSystemHealth {
    let checked_at = Utc::now().to_rfc3339_opts(SecondsFormat::Secs, true);
    let (gstreamer, plugin_policy) = composition_health_items(&checked_at);
    let (delivery_id, delivery_label) = native_delivery_capability();
    let avfoundation = probe_native_delivery()
        .map(|(summary, provenance)| {
            ready_component(
                delivery_id,
                delivery_label,
                &summary,
                &checked_at,
                provenance,
            )
        })
        .unwrap_or_else(|failure| {
            component_from_failure(delivery_id, delivery_label, &checked_at, failure)
        });
    let compatibility = probe_compatibility_decoder()
        .map(|(summary, provenance)| {
            ready_component(
                "render.compatibilityDecoder",
                "Compatibility decoder",
                &summary,
                &checked_at,
                provenance,
            )
        })
        .unwrap_or_else(|failure| {
            component_from_failure(
                "render.compatibilityDecoder",
                "Compatibility decoder",
                &checked_at,
                failure,
            )
        });

    aggregate_render_system_health(
        &checked_at,
        gstreamer,
        plugin_policy,
        avfoundation,
        compatibility,
    )
}

pub const fn render_health_target_id() -> &'static str {
    RENDER_HEALTH_TARGET_ID
}

pub fn render_health_probe_mode_requested() -> bool {
    env::args_os().any(|argument| argument == RENDER_HEALTH_PROBE_ARG)
}

pub fn run_render_health_probe_mode() -> i32 {
    let payload = match crate::render_runtime::start_render_process_runtime() {
        Ok(_) => {
            let checked_at = Utc::now().to_rfc3339_opts(SecondsFormat::Secs, true);
            match (probe_gstreamer_ges(), probe_plugin_policy()) {
                (
                    Ok((gstreamer_summary, gstreamer_provenance)),
                    Ok((policy_summary, policy_provenance)),
                ) => RenderHealthProbePayload::Ready {
                    gstreamer: ready_component(
                        "render.gstreamerGes",
                        "GStreamer / GES",
                        &gstreamer_summary,
                        &checked_at,
                        gstreamer_provenance,
                    ),
                    plugin_policy: Box::new(ready_component(
                        "render.pluginPolicy",
                        "GStreamer plugin policy",
                        &policy_summary,
                        &checked_at,
                        policy_provenance,
                    )),
                },
                (Err(failure), _) | (_, Err(failure)) => RenderHealthProbePayload::Failed {
                    code: failure.code,
                    message: failure.summary,
                    detail: failure.detail,
                },
            }
        }
        Err(error) => RenderHealthProbePayload::Failed {
            code: error.code.to_string(),
            message: error.message,
            detail: error.detail,
        },
    };
    let success = matches!(payload, RenderHealthProbePayload::Ready { .. });
    match serde_json::to_writer(std::io::stdout().lock(), &payload) {
        Ok(()) if success => 0,
        Ok(()) => 1,
        Err(error) => {
            eprintln!("render health probe JSON serialization failed: {error}");
            2
        }
    }
}

fn composition_health_items(
    checked_at: &str,
) -> (SettingsComponentHealth, SettingsComponentHealth) {
    #[cfg(feature = "ges-render")]
    let readiness = match render_runtime_readiness() {
        Ok(readiness) => readiness,
        Err(error) => return composition_items_from_startup_failure(error, checked_at),
    };
    #[cfg(not(feature = "ges-render"))]
    {
        composition_items_from_startup_failure(
            RenderRuntimeError::new(
                "render.runtime.feature_disabled",
                "The required GStreamer/GES render runtime is disabled in this build.",
                "Rebuild with the ges-render feature.",
            ),
            checked_at,
        )
    }

    #[cfg(feature = "ges-render")]
    {
        let executable = render_health_probe_executable(
            env::current_exe().ok(),
            env::var_os(RENDER_HEALTH_PROBE_EXECUTABLE_ENV),
            cfg!(debug_assertions),
        );
        let Some(executable) = executable else {
            return composition_items_from_probe_failure(
                "render.healthProbe.executableUnavailable",
                "The render health helper executable could not be resolved.",
                "current executable path is unavailable",
                checked_at,
            );
        };
        let mut command = Command::new(&executable);
        command.arg(RENDER_HEALTH_PROBE_ARG);
        let helper_items = match run_bounded_command(command, None, RENDER_PROBE_TIMEOUT) {
            Ok(output) => parse_render_health_probe_output(
                output.status.success(),
                &output.stdout,
                &output.stderr,
                checked_at,
            ),
            Err(detail) => composition_items_from_probe_failure(
                "render.healthProbe.timeout",
                "The GStreamer/GES health helper did not finish.",
                &detail,
                checked_at,
            ),
        };
        if readiness == RenderRuntimeReadiness::Pending {
            composition_items_while_pending(helper_items, checked_at)
        } else {
            helper_items
        }
    }
}

#[cfg(any(feature = "ges-render", test))]
fn composition_items_while_pending(
    helper_items: (SettingsComponentHealth, SettingsComponentHealth),
    checked_at: &str,
) -> (SettingsComponentHealth, SettingsComponentHealth) {
    let (helper_gstreamer, mut plugin_policy) = helper_items;
    let mut provenance = helper_gstreamer.provenance;
    provenance.insert("startupResult".to_string(), "pending".to_string());
    provenance.insert(
        "helperDiagnosticState".to_string(),
        settings_health_state_name(&helper_gstreamer.state).to_string(),
    );
    let gstreamer = checking_component(
        "render.gstreamerGes",
        "GStreamer / GES",
        "The desktop composition runtime is still initializing.",
        "render.runtime.initializing",
        "The isolated helper completed, but the desktop runtime has not reached Ready.",
        checked_at,
        provenance,
    );
    plugin_policy
        .provenance
        .insert("desktopRuntimeReadiness".to_string(), "pending".to_string());
    if plugin_policy.state == SettingsHealthState::Ready {
        plugin_policy.summary =
            "Plugin policy passed in isolation; desktop composition is still initializing."
                .to_string();
    }
    (gstreamer, plugin_policy)
}

#[cfg(any(feature = "ges-render", test))]
fn checking_component(
    id: &str,
    label: &str,
    summary: &str,
    code: &str,
    detail: impl Into<String>,
    checked_at: &str,
    mut provenance: BTreeMap<String, String>,
) -> SettingsComponentHealth {
    provenance.insert("lastCheckTime".to_string(), checked_at.to_string());
    SettingsComponentHealth {
        id: id.to_string(),
        label: label.to_string(),
        state: SettingsHealthState::Checking,
        summary: summary.to_string(),
        action_id: None,
        action_label: None,
        last_checked_at: checked_at.to_string(),
        diagnostic_code: Some(code.to_string()),
        diagnostic_detail: Some(detail.into()),
        provenance,
    }
}

#[cfg(any(feature = "ges-render", test))]
fn settings_health_state_name(state: &SettingsHealthState) -> &'static str {
    match state {
        SettingsHealthState::Ready => "ready",
        SettingsHealthState::ActionRequired => "actionRequired",
        SettingsHealthState::NeedsAction => "needsAction",
        SettingsHealthState::Checking => "checking",
        SettingsHealthState::Failed => "failed",
        SettingsHealthState::NotConfigured => "notConfigured",
        SettingsHealthState::NotEnabled => "notEnabled",
        SettingsHealthState::NotApplicable => "notApplicable",
        SettingsHealthState::Unavailable => "unavailable",
    }
}

#[cfg(any(feature = "ges-render", test))]
fn render_health_probe_executable(
    current_executable: Option<PathBuf>,
    development_override: Option<std::ffi::OsString>,
    development: bool,
) -> Option<PathBuf> {
    if development {
        development_override
            .map(PathBuf::from)
            .or(current_executable)
    } else {
        current_executable
    }
}

fn composition_items_from_startup_failure(
    error: RenderRuntimeError,
    checked_at: &str,
) -> (SettingsComponentHealth, SettingsComponentHealth) {
    let mut runtime_provenance = BTreeMap::new();
    runtime_provenance.insert("startupResult".to_string(), "failed".to_string());
    let gstreamer = failed_component(
        "render.gstreamerGes",
        "GStreamer / GES",
        &error.message,
        error.code,
        error.detail,
        checked_at,
        runtime_provenance,
    );
    let policy = failed_component(
        "render.pluginPolicy",
        "GStreamer plugin policy",
        "The plugin policy cannot be checked until the required runtime starts.",
        "render.pluginPolicy.runtimeUnavailable",
        "The desktop startup result recorded a render-runtime failure.",
        checked_at,
        BTreeMap::new(),
    );
    (gstreamer, policy)
}

#[cfg(any(feature = "ges-render", test))]
fn composition_items_from_probe_failure(
    code: &str,
    message: &str,
    detail: &str,
    checked_at: &str,
) -> (SettingsComponentHealth, SettingsComponentHealth) {
    (
        failed_component(
            "render.gstreamerGes",
            "GStreamer / GES",
            message,
            code,
            detail,
            checked_at,
            BTreeMap::new(),
        ),
        failed_component(
            "render.pluginPolicy",
            "GStreamer plugin policy",
            "The plugin policy helper did not return a trustworthy result.",
            "render.pluginPolicy.probeFailed",
            detail,
            checked_at,
            BTreeMap::new(),
        ),
    )
}

#[cfg(any(feature = "ges-render", test))]
fn parse_render_health_probe_output(
    success: bool,
    stdout: &[u8],
    stderr: &[u8],
    checked_at: &str,
) -> (SettingsComponentHealth, SettingsComponentHealth) {
    if success {
        return match serde_json::from_slice::<RenderHealthProbePayload>(stdout) {
            Ok(RenderHealthProbePayload::Ready {
                gstreamer,
                plugin_policy,
            }) => (gstreamer, *plugin_policy),
            Ok(RenderHealthProbePayload::Failed {
                code,
                message,
                detail,
            }) => composition_items_from_probe_failure(&code, &message, &detail, checked_at),
            Err(error) => composition_items_from_probe_failure(
                "render.healthProbe.outputInvalid",
                "The render health helper returned invalid data.",
                &error.to_string(),
                checked_at,
            ),
        };
    }

    if let Ok(RenderHealthProbePayload::Failed {
        code,
        message,
        detail,
    }) = serde_json::from_slice::<RenderHealthProbePayload>(stdout)
    {
        return composition_items_from_probe_failure(&code, &message, &detail, checked_at);
    }
    if let Ok(error) = serde_json::from_slice::<RenderHealthProbeErrorPayload>(stdout) {
        return composition_items_from_probe_failure(
            &error.code,
            &error.message,
            &error.detail,
            checked_at,
        );
    }
    composition_items_from_probe_failure(
        "render.healthProbe.nonzeroExit",
        "The render health helper exited unsuccessfully.",
        &bounded_text(stderr),
        checked_at,
    )
}

fn aggregate_render_system_health(
    checked_at: &str,
    gstreamer: SettingsComponentHealth,
    plugin_policy: SettingsComponentHealth,
    avfoundation: SettingsComponentHealth,
    compatibility: SettingsComponentHealth,
) -> RenderSystemHealth {
    let composition_ready = gstreamer.state == SettingsHealthState::Ready
        && plugin_policy.state == SettingsHealthState::Ready;
    let composition_checking = !composition_ready
        && (gstreamer.state == SettingsHealthState::Checking
            || plugin_policy.state == SettingsHealthState::Checking);
    let native_delivery_degraded = avfoundation.state != SettingsHealthState::Ready;
    let compatibility_degraded = compatibility.state != SettingsHealthState::Ready;
    RenderSystemHealth {
        checked_at: checked_at.to_string(),
        state: if composition_ready {
            SettingsHealthState::Ready
        } else if composition_checking {
            SettingsHealthState::Checking
        } else {
            SettingsHealthState::ActionRequired
        },
        composition_ready,
        native_delivery_degraded,
        compatibility_degraded,
        items: vec![gstreamer, plugin_policy, avfoundation, compatibility],
    }
}

fn ready_component(
    id: &str,
    label: &str,
    summary: &str,
    checked_at: &str,
    mut provenance: BTreeMap<String, String>,
) -> SettingsComponentHealth {
    provenance.insert("lastCheckTime".to_string(), checked_at.to_string());
    SettingsComponentHealth {
        id: id.to_string(),
        label: label.to_string(),
        state: SettingsHealthState::Ready,
        summary: summary.to_string(),
        action_id: None,
        action_label: None,
        last_checked_at: checked_at.to_string(),
        diagnostic_code: None,
        diagnostic_detail: None,
        provenance,
    }
}

fn failed_component(
    id: &str,
    label: &str,
    summary: &str,
    code: &str,
    detail: impl Into<String>,
    checked_at: &str,
    mut provenance: BTreeMap<String, String>,
) -> SettingsComponentHealth {
    provenance.insert("lastCheckTime".to_string(), checked_at.to_string());
    SettingsComponentHealth {
        id: id.to_string(),
        label: label.to_string(),
        state: SettingsHealthState::Failed,
        summary: summary.to_string(),
        action_id: Some("checkRenderSystem".to_string()),
        action_label: Some("Check render system".to_string()),
        last_checked_at: checked_at.to_string(),
        diagnostic_code: Some(code.to_string()),
        diagnostic_detail: Some(detail.into()),
        provenance,
    }
}

fn component_from_failure(
    id: &str,
    label: &str,
    checked_at: &str,
    mut failure: RenderProbeFailure,
) -> SettingsComponentHealth {
    failure
        .provenance
        .insert("lastCheckTime".to_string(), checked_at.to_string());
    SettingsComponentHealth {
        id: id.to_string(),
        label: label.to_string(),
        state: failure.state,
        summary: failure.summary,
        action_id: Some("checkRenderSystem".to_string()),
        action_label: Some("Check render system".to_string()),
        last_checked_at: checked_at.to_string(),
        diagnostic_code: Some(failure.code),
        diagnostic_detail: Some(failure.detail),
        provenance: failure.provenance,
    }
}

#[cfg(feature = "ges-render")]
fn probe_gstreamer_ges() -> Result<(String, BTreeMap<String, String>), RenderProbeFailure> {
    let environment = render_runtime_environment().map_err(|error| {
        RenderProbeFailure::failed(
            error.code,
            "The required composition runtime is unavailable.",
            error.detail,
        )
    })?;
    let manifest = read_runtime_manifest(&environment)?;
    let mut provenance = runtime_provenance(&environment);
    provenance.insert(
        "version".to_string(),
        format!(
            "GStreamer {}; GES {}",
            manifest.gstreamer_version, manifest.ges_version
        ),
    );
    provenance.insert(
        "loadedVersion".to_string(),
        gst::version_string().to_string(),
    );
    provenance.insert(
        "registryPath".to_string(),
        environment.registry_path.display().to_string(),
    );
    Ok((
        "Required GStreamer/GES composition runtime is initialized.".to_string(),
        provenance,
    ))
}

#[cfg(not(feature = "ges-render"))]
fn probe_gstreamer_ges() -> Result<(String, BTreeMap<String, String>), RenderProbeFailure> {
    Err(RenderProbeFailure::failed(
        "render.gstreamerGes.featureDisabled",
        "The required composition runtime is disabled in this build.",
        "Rebuild with the ges-render feature.",
    ))
}

#[cfg(feature = "ges-render")]
fn probe_plugin_policy() -> Result<(String, BTreeMap<String, String>), RenderProbeFailure> {
    let environment = render_runtime_environment().map_err(|error| {
        RenderProbeFailure::failed(
            error.code,
            "The GStreamer plugin policy could not be checked.",
            error.detail,
        )
    })?;
    let manifest = read_runtime_manifest(&environment)?;
    if manifest.plugins.is_empty() || manifest.factories.is_empty() {
        return Err(RenderProbeFailure::failed(
            "render.pluginPolicy.manifestIncomplete",
            "The GStreamer plugin manifest is incomplete.",
            "The signed runtime manifest must list reviewed plugins and factories.",
        )
        .provenance("manifestHash", environment.manifest_hash));
    }
    for factory in &manifest.factories {
        if gst::ElementFactory::find(&factory.name).is_none() {
            return Err(RenderProbeFailure::failed(
                "render.pluginPolicy.missingFactory",
                "A required reviewed GStreamer factory is missing.",
                format!("Missing factory: {}.", factory.name),
            )
            .provenance("missingFactory", factory.name.clone())
            .provenance("manifestHash", environment.manifest_hash));
        }
    }
    let mut item = plugin_policy_component_from_factories(
        manifest.factories.into_iter().map(Into::into).collect(),
        &environment.manifest_hash,
        "",
    );
    if item.state != SettingsHealthState::Ready {
        return Err(RenderProbeFailure {
            state: item.state,
            code: item
                .diagnostic_code
                .take()
                .unwrap_or_else(|| "render.pluginPolicy.failed".to_string()),
            summary: item.summary,
            detail: item.diagnostic_detail.take().unwrap_or_default(),
            provenance: item.provenance,
        });
    }
    let mut provenance = item.provenance;
    provenance.insert(
        "pluginCount".to_string(),
        manifest.plugins.len().to_string(),
    );
    Ok((
        "All staged GStreamer factories pass the reviewed plugin policy.".to_string(),
        provenance,
    ))
}

#[cfg(not(feature = "ges-render"))]
fn probe_plugin_policy() -> Result<(String, BTreeMap<String, String>), RenderProbeFailure> {
    Err(RenderProbeFailure::failed(
        "render.pluginPolicy.featureDisabled",
        "The GStreamer plugin policy cannot be checked in this build.",
        "Rebuild with the ges-render feature.",
    ))
}

#[cfg(any(feature = "ges-render", test))]
fn plugin_policy_component_from_factories(
    factories: Vec<GstFactoryInfo>,
    manifest_hash: &str,
    checked_at: &str,
) -> SettingsComponentHealth {
    let factory_count = factories.len();
    for factory in factories {
        let decision = evaluate_gstreamer_factory(&factory);
        if decision.verdict != PluginPolicyVerdict::Allowed {
            let verdict = match decision.verdict {
                PluginPolicyVerdict::Allowed => "allowed",
                PluginPolicyVerdict::Denied => "denied",
                PluginPolicyVerdict::ReviewRequired => "reviewRequired",
            };
            let mut provenance = BTreeMap::new();
            provenance.insert("manifestHash".to_string(), manifest_hash.to_string());
            provenance.insert("pluginCount".to_string(), factory_count.to_string());
            provenance.insert("deniedFactory".to_string(), factory.name.clone());
            provenance.insert("policyVerdict".to_string(), verdict.to_string());
            return failed_component(
                "render.pluginPolicy",
                "GStreamer plugin policy",
                "A GStreamer factory failed the reviewed plugin policy.",
                "render.pluginPolicy.denied",
                format!("{}: {}", factory.name, decision.reason),
                checked_at,
                provenance,
            );
        }
    }
    let mut provenance = BTreeMap::new();
    provenance.insert("manifestHash".to_string(), manifest_hash.to_string());
    provenance.insert("pluginCount".to_string(), factory_count.to_string());
    ready_component(
        "render.pluginPolicy",
        "GStreamer plugin policy",
        "All GStreamer factories pass the reviewed plugin policy.",
        checked_at,
        provenance,
    )
}

/// Health item identity for final delivery: AVFoundation on macOS, the reviewed GStreamer
/// codec set (bundled LGPL FFmpeg, OpenH264, VA-API) on Linux.
pub const fn native_delivery_capability() -> (&'static str, &'static str) {
    if cfg!(target_os = "macos") {
        ("render.avfoundation", "AVFoundation final delivery")
    } else {
        ("render.linuxDelivery", "Linux codec delivery")
    }
}

#[cfg(not(target_os = "macos"))]
fn probe_native_delivery() -> Result<(String, BTreeMap<String, String>), RenderProbeFailure> {
    use crate::project::export_profiles::{export_profile_availability, ExportProfile};

    let profiles = [
        ExportProfile::Mp4H264,
        ExportProfile::Mp4H265,
        ExportProfile::ProResMov,
    ]
    .map(export_profile_availability);
    let supported = profiles
        .iter()
        .filter(|profile| profile.available)
        .map(|profile| profile.label.clone())
        .collect::<Vec<_>>();
    let unavailable = profiles
        .iter()
        .filter(|profile| !profile.available)
        .map(|profile| {
            format!(
                "{}: {}",
                profile.label,
                profile
                    .unavailable_reason
                    .as_deref()
                    .unwrap_or("unavailable")
            )
        })
        .collect::<Vec<_>>();
    let mut provenance = BTreeMap::new();
    provenance.insert("supportedProfiles".to_string(), supported.join(", "));
    let baseline_ready = profiles
        .iter()
        .filter(|profile| profile.profile != ExportProfile::Mp4H265)
        .all(|profile| profile.available);
    if !baseline_ready {
        return Err(RenderProbeFailure {
            state: SettingsHealthState::Failed,
            code: "render.linuxDelivery.codecsMissing".to_string(),
            summary: "MP4 and ProRes delivery codecs are unavailable.".to_string(),
            detail: unavailable.join("; "),
            provenance,
        });
    }
    if !unavailable.is_empty() {
        provenance.insert("unavailableProfiles".to_string(), unavailable.join("; "));
    }
    Ok((
        if unavailable.is_empty() {
            "MP4 H.264, MP4 H.265, and ProRes delivery are ready.".to_string()
        } else {
            "MP4 H.264 and ProRes delivery are ready; H.265 needs a VA-API hardware encoder."
                .to_string()
        },
        provenance,
    ))
}

#[cfg(target_os = "macos")]
fn probe_native_delivery() -> Result<(String, BTreeMap<String, String>), RenderProbeFailure> {
    {
        let executable = avfoundation_exporter_path().map_err(|detail| {
            RenderProbeFailure::failed(
                "render.avfoundation.path",
                "The AVFoundation final-delivery exporter is unavailable.",
                detail,
            )
        })?;
        let mut command = Command::new(&executable);
        command.arg("--capabilities").env_clear();
        let output =
            run_bounded_command(command, None, RENDER_PROBE_TIMEOUT).map_err(|detail| {
                RenderProbeFailure::failed(
                    "render.avfoundation.probeFailed",
                    "The AVFoundation final-delivery exporter failed its capability probe.",
                    detail,
                )
                .provenance("executablePath", executable.display().to_string())
            })?;
        if !output.status.success() {
            return Err(RenderProbeFailure::failed(
                "render.avfoundation.probeRejected",
                "The AVFoundation final-delivery exporter rejected its capability probe.",
                bounded_text(&output.stderr),
            )
            .provenance("executablePath", executable.display().to_string()));
        }
        let capabilities = serde_json::from_slice::<AvFoundationCapabilities>(&output.stdout)
            .map_err(|error| {
                RenderProbeFailure::failed(
                    "render.avfoundation.protocolInvalid",
                    "The AVFoundation exporter returned invalid capability data.",
                    error.to_string(),
                )
                .provenance("executablePath", executable.display().to_string())
            })?;
        if capabilities.protocol != AVFOUNDATION_EXPORT_PROTOCOL
            || capabilities.schema_version != AVFOUNDATION_EXPORT_PROTOCOL_VERSION
            || capabilities.backend != "avfoundation-native"
        {
            return Err(RenderProbeFailure::failed(
                "render.avfoundation.protocolMismatch",
                "The AVFoundation exporter protocol does not match this build.",
                format!(
                    "protocol={}, schemaVersion={}, backend={}",
                    capabilities.protocol, capabilities.schema_version, capabilities.backend
                ),
            )
            .provenance("executablePath", executable.display().to_string()));
        }
        let supported_profiles = capabilities
            .profiles
            .iter()
            .filter_map(|(profile, supported)| supported.then_some(profile.as_str()))
            .collect::<Vec<_>>()
            .join(", ");
        let mut provenance = BTreeMap::new();
        provenance.insert(
            "executablePath".to_string(),
            executable.display().to_string(),
        );
        provenance.insert("supportedProfiles".to_string(), supported_profiles);
        Ok((
            "AVFoundation final-delivery exporter is ready.".to_string(),
            provenance,
        ))
    }
}

fn probe_compatibility_decoder() -> Result<(String, BTreeMap<String, String>), RenderProbeFailure> {
    let executable = compatibility_decoder_path().map_err(|detail| {
        RenderProbeFailure::failed(
            "render.compatibilityDecoder.path",
            "The compatibility decoder is unavailable.",
            detail,
        )
    })?;
    let request = CompatibilityRequest {
        protocol: COMPATIBILITY_PROTOCOL_NAME.to_string(),
        schema_version: COMPATIBILITY_PROTOCOL_VERSION,
        request_id: "settings-render-health".to_string(),
        operation: CompatibilityOperation::Capabilities,
        budgets: WorkerBudgets {
            timeout_millis: RENDER_PROBE_TIMEOUT.as_millis() as u64,
            max_output_bytes: 1024 * 1024,
        },
    };
    let mut input = serde_json::to_vec(&request).map_err(|error| {
        RenderProbeFailure::failed(
            "render.compatibilityDecoder.requestInvalid",
            "The compatibility decoder probe could not be created.",
            error.to_string(),
        )
    })?;
    input.push(b'\n');
    let mut command = Command::new(&executable);
    command.env_clear().envs(
        crate::precompose::compatibility::compatibility_worker_environment(
            &executable,
            &env::temp_dir(),
        ),
    );
    let output =
        run_bounded_command(command, Some(input), RENDER_PROBE_TIMEOUT).map_err(|detail| {
            RenderProbeFailure::failed(
                "render.compatibilityDecoder.probeFailed",
                "The compatibility decoder failed its capability probe.",
                detail,
            )
            .provenance("executablePath", executable.display().to_string())
        })?;
    let line = String::from_utf8_lossy(&output.stdout);
    let event = serde_json::from_str::<WorkerEvent>(
        line.lines()
            .next()
            .filter(|line| !line.trim().is_empty())
            .ok_or_else(|| {
                RenderProbeFailure::failed(
                    "render.compatibilityDecoder.emptyResponse",
                    "The compatibility decoder returned no capability data.",
                    bounded_text(&output.stderr),
                )
            })?,
    )
    .map_err(|error| {
        RenderProbeFailure::failed(
            "render.compatibilityDecoder.protocolInvalid",
            "The compatibility decoder returned invalid capability data.",
            error.to_string(),
        )
    })?;
    match event {
        WorkerEvent::Completed {
            result:
                CompatibilityResult::Capabilities {
                    protocol,
                    schema_version,
                    factories,
                },
            ..
        } if protocol == COMPATIBILITY_PROTOCOL_NAME
            && schema_version == COMPATIBILITY_PROTOCOL_VERSION =>
        {
            let mut provenance = BTreeMap::new();
            provenance.insert(
                "executablePath".to_string(),
                executable.display().to_string(),
            );
            provenance.insert("pluginCount".to_string(), factories.len().to_string());
            provenance.insert(
                "supportedProfiles".to_string(),
                [
                    CompatibilityProfile::H264PcmMov,
                    CompatibilityProfile::ProRes422PcmMov,
                ]
                .into_iter()
                .map(|profile| format!("{profile:?}"))
                .collect::<Vec<_>>()
                .join(", "),
            );
            Ok((
                "Compatibility decoder is ready for unsupported input media.".to_string(),
                provenance,
            ))
        }
        WorkerEvent::Failed { error, .. } => Err(RenderProbeFailure::failed(
            "render.compatibilityDecoder.rejected",
            "The compatibility decoder rejected its capability probe.",
            format!("{}: {}", error.code, error.message),
        )
        .provenance("executablePath", executable.display().to_string())),
        other => Err(RenderProbeFailure::failed(
            "render.compatibilityDecoder.protocolMismatch",
            "The compatibility decoder protocol does not match this build.",
            format!("Unexpected response: {other:?}"),
        )
        .provenance("executablePath", executable.display().to_string())),
    }
}

#[cfg(feature = "ges-render")]
fn read_runtime_manifest(
    environment: &RenderRuntimeEnvironment,
) -> Result<RenderRuntimeManifest, RenderProbeFailure> {
    let bytes = fs::read(&environment.manifest_path).map_err(|error| {
        RenderProbeFailure::failed(
            "render.runtime.manifestUnreadable",
            "The render runtime manifest could not be read.",
            format!("{}: {error}", environment.manifest_path.display()),
        )
    })?;
    serde_json::from_slice(&bytes).map_err(|error| {
        RenderProbeFailure::failed(
            "render.runtime.manifestInvalid",
            "The render runtime manifest is invalid.",
            error.to_string(),
        )
    })
}

#[cfg(feature = "ges-render")]
fn runtime_provenance(environment: &RenderRuntimeEnvironment) -> BTreeMap<String, String> {
    BTreeMap::from([
        (
            "runtimeSource".to_string(),
            render_runtime_source_name(environment.source).to_string(),
        ),
        (
            "manifestHash".to_string(),
            environment.manifest_hash.clone(),
        ),
        (
            "executablePath".to_string(),
            env::current_exe()
                .map(|path| path.display().to_string())
                .unwrap_or_else(|_| "unavailable".to_string()),
        ),
    ])
}

#[cfg(feature = "ges-render")]
fn render_runtime_source_name(source: RenderRuntimeSource) -> &'static str {
    match source {
        RenderRuntimeSource::Bundled => "bundled",
        RenderRuntimeSource::DevelopmentConfigured => "developmentConfigured",
        RenderRuntimeSource::DevelopmentSystem => "developmentSystem",
    }
}

#[cfg(target_os = "macos")]
fn avfoundation_exporter_path() -> Result<PathBuf, String> {
    if let Some(path) = env::var_os("VIDEO_CREATER_AVFOUNDATION_EXPORTER") {
        return executable_file(PathBuf::from(path), "configured AVFoundation exporter");
    }
    let current = env::current_exe().map_err(|error| error.to_string())?;
    let mut directory = current.parent().unwrap_or_else(|| Path::new("."));
    if directory.file_name().and_then(|name| name.to_str()) == Some("deps") {
        directory = directory.parent().unwrap_or(directory);
    }
    executable_file(
        directory.join("video-creater-avfoundation-exporter"),
        "bundled AVFoundation exporter",
    )
}

fn compatibility_decoder_path() -> Result<PathBuf, String> {
    let path = crate::precompose::compatibility::worker_path().map_err(|errors| {
        errors
            .into_iter()
            .map(|error| error.message)
            .collect::<Vec<_>>()
            .join("; ")
    })?;
    executable_file(path, "bundled compatibility decoder")
}

fn executable_file(path: PathBuf, label: &str) -> Result<PathBuf, String> {
    if path.is_file() {
        Ok(path)
    } else {
        Err(format!("{label} is missing at {}.", path.display()))
    }
}

fn run_bounded_command(
    command: Command,
    input: Option<Vec<u8>>,
    timeout: Duration,
) -> Result<Output, String> {
    run_bounded_command_with_hooks(
        command,
        input,
        timeout,
        |child, input| {
            let mut stdin = child
                .stdin
                .take()
                .ok_or_else(|| std::io::Error::other("probe stdin is unavailable"))?;
            stdin.write_all(&input)?;
            Ok(())
        },
        |child| child.try_wait(),
    )
}

fn run_bounded_command_with_hooks<W, T>(
    mut command: Command,
    input: Option<Vec<u8>>,
    timeout: Duration,
    write_input: W,
    mut try_wait: T,
) -> Result<Output, String>
where
    W: FnOnce(&mut Child, Vec<u8>) -> std::io::Result<()>,
    T: FnMut(&mut Child) -> std::io::Result<Option<std::process::ExitStatus>>,
{
    command
        .stdin(if input.is_some() {
            Stdio::piped()
        } else {
            Stdio::null()
        })
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        command.process_group(0);
    }
    let child = command.spawn().map_err(|error| error.to_string())?;
    let mut child = ChildProcessGroupGuard::new(child);
    if let Some(input) = input {
        if let Err(error) = write_input(child.child_mut(), input) {
            return Err(child.fail(format!("probe stdin write failed: {error}")));
        }
    }
    let started = Instant::now();
    loop {
        match try_wait(child.child_mut()) {
            Ok(Some(_)) => return child.finish_output(),
            Ok(None) => {}
            Err(error) => {
                return Err(child.fail(format!("probe status check failed: {error}")));
            }
        }
        if started.elapsed() >= timeout {
            return Err(child.fail(format!(
                "capability probe exceeded its {} ms budget",
                timeout.as_millis()
            )));
        }
        thread::sleep(Duration::from_millis(10));
    }
}

struct ChildProcessGroupGuard {
    child: Option<Child>,
    process_group_id: Option<i32>,
}

impl ChildProcessGroupGuard {
    fn new(child: Child) -> Self {
        #[cfg(unix)]
        let process_group_id = i32::try_from(child.id()).ok();
        #[cfg(not(unix))]
        let process_group_id = None;
        Self {
            child: Some(child),
            process_group_id,
        }
    }

    fn child_mut(&mut self) -> &mut Child {
        self.child.as_mut().expect("managed child is present")
    }

    fn finish_output(mut self) -> Result<Output, String> {
        let group_cleanup = self.terminate_process_group();
        let child = self.child.take().expect("managed child is present");
        let output = child.wait_with_output().map_err(|error| error.to_string());
        match (output, group_cleanup) {
            (Ok(output), Ok(())) => Ok(output),
            (Ok(_), Err(cleanup)) => Err(format!(
                "probe process exited, but process-group cleanup failed: {cleanup}"
            )),
            (Err(wait), Ok(())) => Err(format!("probe output wait failed: {wait}")),
            (Err(wait), Err(cleanup)) => Err(format!(
                "probe output wait failed: {wait}; process-group cleanup failed: {cleanup}"
            )),
        }
    }

    fn fail(mut self, primary: String) -> String {
        match self.terminate_and_reap() {
            Ok(()) => format!("{primary}; cleanup completed"),
            Err(cleanup) => format!("{primary}; cleanup failed: {cleanup}"),
        }
    }

    fn terminate_and_reap(&mut self) -> Result<(), String> {
        let Some(mut child) = self.child.take() else {
            return Ok(());
        };
        let mut failures = Vec::new();
        if let Err(error) = self.terminate_process_group() {
            failures.push(error);
        }
        #[cfg(not(unix))]
        if let Err(error) = child.kill() {
            failures.push(format!("child kill failed: {error}"));
        }
        if let Err(error) = child.wait() {
            failures.push(format!("child wait failed: {error}"));
        }
        if failures.is_empty() {
            Ok(())
        } else {
            Err(failures.join("; "))
        }
    }

    fn terminate_process_group(&self) -> Result<(), String> {
        #[cfg(unix)]
        if let Some(process_group_id) = self.process_group_id {
            let result = unsafe { libc::kill(-process_group_id, libc::SIGKILL) };
            if result != 0 {
                let error = std::io::Error::last_os_error();
                if error.raw_os_error() != Some(libc::ESRCH) {
                    return Err(format!("process-group kill failed: {error}"));
                }
            }
        }
        Ok(())
    }
}

impl Drop for ChildProcessGroupGuard {
    fn drop(&mut self) {
        if self.child.is_some() {
            if let Err(error) = self.terminate_and_reap() {
                eprintln!("render health probe cleanup failed during drop: {error}");
            }
        }
    }
}

fn bounded_text(bytes: &[u8]) -> String {
    String::from_utf8_lossy(&bytes[..bytes.len().min(4 * 1024)]).into_owned()
}

#[cfg(test)]
mod tests {
    use super::{
        aggregate_render_system_health, composition_items_from_startup_failure,
        composition_items_while_pending, failed_component, parse_render_health_probe_output,
        plugin_policy_component_from_factories, ready_component, render_health_probe_executable,
        run_bounded_command, run_bounded_command_with_hooks,
    };
    use crate::render_pipeline::plugin_policy::GstFactoryInfo;
    use crate::render_runtime::RenderRuntimeError;
    use crate::settings::health::SettingsHealthState;
    use std::collections::BTreeMap;
    use std::process::Command;
    use std::time::Duration;

    fn ready(id: &str) -> crate::settings::health::SettingsComponentHealth {
        ready_component(id, id, "Ready.", "2026-07-16T12:00:00Z", BTreeMap::new())
    }

    fn failed(id: &str) -> crate::settings::health::SettingsComponentHealth {
        failed_component(
            id,
            id,
            "Failed.",
            "render.probe.failed",
            "probe failed",
            "2026-07-16T12:00:00Z",
            BTreeMap::new(),
        )
    }

    #[test]
    fn gstreamer_failure_requires_action_even_when_avfoundation_is_ready() {
        let health = aggregate_render_system_health(
            "2026-07-16T12:00:00Z",
            failed("render.gstreamerGes"),
            ready("render.pluginPolicy"),
            ready("render.avfoundation"),
            ready("render.compatibilityDecoder"),
        );

        assert_eq!(health.state, SettingsHealthState::ActionRequired);
        assert!(!health.composition_ready);
        assert!(!health.native_delivery_degraded);
    }

    #[test]
    fn avfoundation_failure_degrades_delivery_without_hiding_ready_composition() {
        let health = aggregate_render_system_health(
            "2026-07-16T12:00:00Z",
            ready("render.gstreamerGes"),
            ready("render.pluginPolicy"),
            failed("render.avfoundation"),
            ready("render.compatibilityDecoder"),
        );

        assert_eq!(health.state, SettingsHealthState::Ready);
        assert!(health.composition_ready);
        assert!(health.native_delivery_degraded);
    }

    #[test]
    fn helper_ready_cannot_promote_persistently_pending_desktop_runtime() {
        let pending_items = composition_items_while_pending(
            (ready("render.gstreamerGes"), ready("render.pluginPolicy")),
            "2026-07-16T12:00:00Z",
        );
        let health = aggregate_render_system_health(
            "2026-07-16T12:00:00Z",
            pending_items.0,
            pending_items.1,
            ready("render.avfoundation"),
            ready("render.compatibilityDecoder"),
        );

        assert_eq!(health.items[0].state, SettingsHealthState::Checking);
        assert_eq!(
            health.items[0].diagnostic_code.as_deref(),
            Some("render.runtime.initializing")
        );
        assert!(!health.composition_ready);
        assert_eq!(health.state, SettingsHealthState::Checking);
        assert!(health.items[1]
            .summary
            .contains("desktop composition is still initializing"));
    }

    #[test]
    fn pending_composition_stays_checking_when_optional_delivery_is_degraded() {
        let pending_items = composition_items_while_pending(
            (ready("render.gstreamerGes"), ready("render.pluginPolicy")),
            "2026-07-16T12:00:00Z",
        );
        let health = aggregate_render_system_health(
            "2026-07-16T12:00:00Z",
            pending_items.0,
            pending_items.1,
            failed("render.avfoundation"),
            failed("render.compatibilityDecoder"),
        );

        assert_eq!(health.state, SettingsHealthState::Checking);
        assert!(!health.composition_ready);
        assert!(health.native_delivery_degraded);
        assert!(health.compatibility_degraded);
    }

    #[test]
    fn denied_factory_reports_policy_failure_and_exact_factory() {
        let item = plugin_policy_component_from_factories(
            vec![
                GstFactoryInfo::new("videoconvert")
                    .plugin_name("videoconvertscale")
                    .package("GStreamer Base Plug-ins source release")
                    .license("LGPL"),
                GstFactoryInfo::new("x264enc")
                    .plugin_name("x264")
                    .package("GStreamer Bad Plug-ins source release")
                    .license("GPL"),
            ],
            "manifest-hash",
            "2026-07-16T12:00:00Z",
        );

        assert_eq!(item.state, SettingsHealthState::Failed);
        assert_eq!(
            item.diagnostic_code.as_deref(),
            Some("render.pluginPolicy.denied")
        );
        assert_eq!(
            item.provenance.get("deniedFactory").map(String::as_str),
            Some("x264enc")
        );
        assert!(item
            .diagnostic_detail
            .as_deref()
            .is_some_and(|detail| detail.contains("x264enc")));
    }

    #[test]
    fn recorded_startup_failure_becomes_required_composition_health() {
        let (gstreamer, policy) = composition_items_from_startup_failure(
            RenderRuntimeError::new(
                "render.runtime.bundled_missing",
                "The required bundled runtime is unavailable.",
                "fixture missing",
            ),
            "2026-07-16T12:00:00Z",
        );

        assert_eq!(gstreamer.state, SettingsHealthState::Failed);
        assert_eq!(
            gstreamer.diagnostic_code.as_deref(),
            Some("render.runtime.bundled_missing")
        );
        assert_eq!(policy.state, SettingsHealthState::Failed);
        assert_eq!(
            policy.diagnostic_code.as_deref(),
            Some("render.pluginPolicy.runtimeUnavailable")
        );
    }

    #[cfg(unix)]
    #[test]
    fn timed_out_probe_child_is_killed_and_reaped() {
        let temp = tempfile::tempdir().expect("temp");
        let pid_path = temp.path().join("probe.pid");
        let mut command = Command::new("/bin/sh");
        command
            .arg("-c")
            .arg("echo $$ > \"$1\"; exec sleep 30")
            .arg("render-health-probe")
            .arg(&pid_path);

        let error = run_bounded_command(command, None, Duration::from_millis(50))
            .expect_err("probe must time out");
        let pid = std::fs::read_to_string(&pid_path)
            .expect("probe pid")
            .trim()
            .parse::<i32>()
            .expect("numeric pid");

        assert!(error.contains("exceeded"));
        let process_exists = unsafe { libc::kill(pid, 0) } == 0;
        assert!(!process_exists, "timed-out probe child must be reaped");
    }

    #[cfg(unix)]
    #[test]
    fn timed_out_probe_terminates_descendant_process_group() {
        let temp = tempfile::tempdir().expect("temp");
        let pid_path = temp.path().join("probe-pids");
        let mut command = Command::new("/bin/sh");
        command
            .arg("-c")
            .arg("sleep 30 & child=$!; echo \"$$ $child\" > \"$1\"; wait")
            .arg("render-health-probe")
            .arg(&pid_path);

        run_bounded_command(command, None, Duration::from_millis(100))
            .expect_err("probe must time out");
        let pids = std::fs::read_to_string(&pid_path)
            .expect("probe pids")
            .split_whitespace()
            .map(|value| value.parse::<i32>().expect("numeric pid"))
            .collect::<Vec<_>>();

        assert_eq!(pids.len(), 2);
        for pid in pids {
            assert_process_gone(pid);
        }
    }

    #[cfg(unix)]
    #[test]
    fn completed_nonzero_probe_reaps_leftover_descendants() {
        let temp = tempfile::tempdir().expect("temp");
        let pid_path = temp.path().join("descendant-pid");
        let mut command = Command::new("/bin/sh");
        command
            .arg("-c")
            .arg("sleep 30 & echo $! > \"$1\"; exit 7")
            .arg("render-health-probe")
            .arg(&pid_path);

        let output = run_bounded_command(command, None, Duration::from_secs(1))
            .expect("completed nonzero probe output");
        let descendant = std::fs::read_to_string(&pid_path)
            .expect("descendant pid")
            .trim()
            .parse::<i32>()
            .expect("numeric pid");

        assert_eq!(output.status.code(), Some(7));
        assert_process_gone(descendant);
    }

    #[cfg(unix)]
    #[test]
    fn injected_stdin_write_error_cleans_up_process_group() {
        let temp = tempfile::tempdir().expect("temp");
        let pid_path = temp.path().join("stdin-error-pids");
        let mut command = Command::new("/bin/sh");
        command
            .arg("-c")
            .arg("sleep 30 & child=$!; echo \"$$ $child\" > \"$1\"; wait")
            .arg("render-health-probe")
            .arg(&pid_path);
        let hook_pid_path = pid_path.clone();

        let error = run_bounded_command_with_hooks(
            command,
            Some(b"probe".to_vec()),
            Duration::from_secs(1),
            move |_, _| {
                wait_for_pid_file(&hook_pid_path)?;
                Err(std::io::Error::other("injected stdin failure"))
            },
            |child| child.try_wait(),
        )
        .expect_err("stdin error");

        assert!(error.contains("injected stdin failure"));
        assert!(error.contains("cleanup"));
        for pid in read_pids(&pid_path) {
            assert_process_gone(pid);
        }
    }

    #[cfg(unix)]
    #[test]
    fn injected_try_wait_error_cleans_up_process_group() {
        let temp = tempfile::tempdir().expect("temp");
        let pid_path = temp.path().join("try-wait-error-pids");
        let mut command = Command::new("/bin/sh");
        command
            .arg("-c")
            .arg("sleep 30 & child=$!; echo \"$$ $child\" > \"$1\"; wait")
            .arg("render-health-probe")
            .arg(&pid_path);
        let hook_pid_path = pid_path.clone();

        let error = run_bounded_command_with_hooks(
            command,
            None,
            Duration::from_secs(1),
            |_, _| Ok(()),
            move |_| {
                wait_for_pid_file(&hook_pid_path)?;
                Err(std::io::Error::other("injected try_wait failure"))
            },
        )
        .expect_err("try_wait error");

        assert!(error.contains("injected try_wait failure"));
        assert!(error.contains("cleanup"));
        for pid in read_pids(&pid_path) {
            assert_process_gone(pid);
        }
    }

    #[test]
    fn release_probe_resolution_ignores_executable_override() {
        let selected = render_health_probe_executable(
            Some(std::path::PathBuf::from("/Applications/Video Creater")),
            Some(std::ffi::OsString::from("/tmp/untrusted-helper")),
            false,
        )
        .expect("release helper path");

        assert_eq!(
            selected,
            std::path::PathBuf::from("/Applications/Video Creater")
        );
    }

    #[test]
    fn malformed_probe_output_maps_to_failed_composition_items() {
        let (gstreamer, policy) =
            parse_render_health_probe_output(true, b"not-json", b"", "2026-07-16T12:00:00Z");

        assert_eq!(gstreamer.state, SettingsHealthState::Failed);
        assert_eq!(
            gstreamer.diagnostic_code.as_deref(),
            Some("render.healthProbe.outputInvalid")
        );
        assert_eq!(policy.state, SettingsHealthState::Failed);
    }

    #[test]
    fn nonzero_probe_output_maps_to_failed_composition_items() {
        let (gstreamer, policy) = parse_render_health_probe_output(
            false,
            br#"{"code":"render.runtime.ges_init_failed","message":"GES failed","detail":"fixture"}"#,
            b"fixture stderr",
            "2026-07-16T12:00:00Z",
        );

        assert_eq!(
            gstreamer.diagnostic_code.as_deref(),
            Some("render.runtime.ges_init_failed")
        );
        assert_eq!(policy.state, SettingsHealthState::Failed);
    }

    #[cfg(unix)]
    fn assert_process_gone(pid: i32) {
        for _ in 0..20 {
            if unsafe { libc::kill(pid, 0) } != 0 {
                return;
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        panic!("process-group member {pid} must be terminated");
    }

    #[cfg(unix)]
    fn wait_for_pid_file(path: &std::path::Path) -> std::io::Result<()> {
        for _ in 0..100 {
            if path.is_file() {
                return Ok(());
            }
            std::thread::sleep(Duration::from_millis(2));
        }
        Err(std::io::Error::other("pid file was not published"))
    }

    #[cfg(unix)]
    fn read_pids(path: &std::path::Path) -> Vec<i32> {
        std::fs::read_to_string(path)
            .expect("probe pids")
            .split_whitespace()
            .map(|value| value.parse::<i32>().expect("numeric pid"))
            .collect()
    }
}
