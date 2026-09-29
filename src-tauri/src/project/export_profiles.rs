use serde::{Deserialize, Serialize};
use std::path::Path;

use crate::edit::render_plan::RenderQuality;
use crate::render_pipeline::avfoundation_backend::{
    AvFoundationExportProfile, AvFoundationRenderBackend,
};
use crate::render_pipeline::output_profile::{
    gstreamer_output_profile_target, GstreamerOutputProfileTarget,
};
#[cfg(feature = "ges-render")]
use crate::render_pipeline::plugin_policy::{
    evaluate_gstreamer_factory, GstFactoryInfo, PluginPolicyVerdict,
};
#[cfg(feature = "ges-render")]
use crate::render_runtime::render_runtime_environment;

#[cfg(feature = "ges-render")]
use gst::prelude::*;
#[cfg(feature = "ges-render")]
use gstreamer as gst;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ExportProfile {
    Webm,
    Mp4H264,
    Mp4H265,
    ProResMov,
    PalmierProject,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ExportPolicyStatus {
    Approved,
    MissingRuntime,
    PolicyGated,
    UnsupportedBuild,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportQualityAvailability {
    pub draft: bool,
    #[serde(rename = "final")]
    pub final_quality: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportQualityUnavailableReasons {
    pub draft: Option<String>,
    #[serde(rename = "final")]
    pub final_quality: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportProfileAvailability {
    pub profile: ExportProfile,
    pub label: String,
    pub available: bool,
    pub container: String,
    pub extension: String,
    pub mime_type: String,
    pub video_codec: String,
    pub audio_codec: Option<String>,
    pub required_runtime: Vec<String>,
    pub policy_status: ExportPolicyStatus,
    pub unavailable_reason: Option<String>,
    pub quality_availability: ExportQualityAvailability,
    pub quality_unavailable_reasons: ExportQualityUnavailableReasons,
}

impl ExportProfileAvailability {
    pub fn quality_available(&self, quality: RenderQuality) -> bool {
        match quality {
            RenderQuality::Draft => self.quality_availability.draft,
            RenderQuality::Final => self.quality_availability.final_quality,
        }
    }

    pub fn quality_unavailable_reason(&self, quality: RenderQuality) -> Option<&str> {
        match quality {
            RenderQuality::Draft => self.quality_unavailable_reasons.draft.as_deref(),
            RenderQuality::Final => self.quality_unavailable_reasons.final_quality.as_deref(),
        }
    }

    pub fn draft_unavailable_reason(&self) -> Option<&str> {
        self.quality_unavailable_reason(RenderQuality::Draft)
    }

    pub fn final_unavailable_reason(&self) -> Option<&str> {
        self.quality_unavailable_reason(RenderQuality::Final)
    }
}

pub fn mp4_export_profile_availability_report() -> Vec<ExportProfileAvailability> {
    vec![
        export_profile_availability(ExportProfile::Webm),
        export_profile_availability(ExportProfile::Mp4H264),
        export_profile_availability(ExportProfile::Mp4H265),
        export_profile_availability(ExportProfile::ProResMov),
    ]
}

pub fn export_profile_availability(profile: ExportProfile) -> ExportProfileAvailability {
    match profile {
        ExportProfile::Webm => {
            delivery_profile_availability(profile, "Legacy WebM", "webm", "vp8", Some("opus"))
        }
        ExportProfile::Mp4H264 => {
            delivery_profile_availability(profile, "MP4 / H.264", "mp4", "h264", Some("aac"))
        }
        // `hevc` is the codec name a probe of the rendered MP4 reports, and the
        // render report has to match this payload field for field, so the
        // profile descriptor spells the codec the way the file does. The label
        // keeps the H.265 wording people recognise.
        ExportProfile::Mp4H265 => {
            delivery_profile_availability(profile, "MP4 / H.265", "mp4", "hevc", Some("aac"))
        }
        ExportProfile::ProResMov => {
            delivery_profile_availability(profile, "ProRes MOV", "mov", "prores", Some("pcm"))
        }
        ExportProfile::PalmierProject => {
            unsupported_profile_availability(profile, "Palmier project", "", "", None)
        }
    }
}

fn delivery_profile_availability(
    profile: ExportProfile,
    label: &str,
    container: &str,
    video_codec: &str,
    audio_codec: Option<&str>,
) -> ExportProfileAvailability {
    let avfoundation_profile = match profile {
        ExportProfile::Webm => None,
        ExportProfile::Mp4H264 => Some(AvFoundationExportProfile::H264),
        ExportProfile::Mp4H265 => Some(AvFoundationExportProfile::Hevc),
        ExportProfile::ProResMov => Some(AvFoundationExportProfile::ProRes422),
        ExportProfile::PalmierProject => None,
    };
    let availability = if cfg!(target_os = "macos")
        && avfoundation_profile
            .is_some_and(|profile| AvFoundationRenderBackend::new().supports_profile(profile))
    {
        ExportProfileAvailability {
            profile,
            label: label.to_string(),
            available: true,
            container: container.to_string(),
            extension: match profile {
                ExportProfile::Webm => "webm",
                ExportProfile::Mp4H264 | ExportProfile::Mp4H265 => "mp4",
                ExportProfile::ProResMov => "mov",
                ExportProfile::PalmierProject => "",
            }
            .to_string(),
            mime_type: match profile {
                ExportProfile::Webm => "video/webm",
                ExportProfile::Mp4H264 | ExportProfile::Mp4H265 => "video/mp4",
                ExportProfile::ProResMov => "video/quicktime",
                ExportProfile::PalmierProject => "application/octet-stream",
            }
            .to_string(),
            video_codec: video_codec.to_string(),
            audio_codec: audio_codec.map(str::to_string),
            required_runtime: vec![
                "system:avfoundation".to_string(),
                "bundled:video-creater-avfoundation-exporter".to_string(),
            ],
            policy_status: ExportPolicyStatus::Approved,
            unavailable_reason: None,
            quality_availability: ExportQualityAvailability {
                draft: true,
                final_quality: true,
            },
            quality_unavailable_reasons: ExportQualityUnavailableReasons {
                draft: None,
                final_quality: None,
            },
        }
    } else {
        gstreamer_profile_availability(profile, label, container, video_codec, audio_codec)
    };

    quality_aware_availability(availability)
}

fn quality_aware_availability(
    mut availability: ExportProfileAvailability,
) -> ExportProfileAvailability {
    let final_available = availability.available;
    availability.quality_availability = ExportQualityAvailability {
        draft: final_available,
        final_quality: final_available,
    };
    availability.quality_unavailable_reasons = ExportQualityUnavailableReasons {
        draft: availability.unavailable_reason.clone(),
        final_quality: availability.unavailable_reason.clone(),
    };

    if availability
        .required_runtime
        .iter()
        .any(|runtime| runtime == "system:avfoundation")
    {
        if let Some(profile) = match availability.profile {
            ExportProfile::Mp4H264 => Some(AvFoundationExportProfile::H264),
            ExportProfile::Mp4H265 => Some(AvFoundationExportProfile::Hevc),
            _ => None,
        } {
            let backend = AvFoundationRenderBackend::new();
            let draft_available = backend
                .capabilities()
                .is_ok_and(|report| report.supports_profile_quality(profile, RenderQuality::Draft));
            availability.quality_availability.draft = draft_available;
            availability.quality_unavailable_reasons.draft = (!draft_available).then(|| {
                "Draft export requires a distinct native low-latency preset for the selected codec."
                    .to_string()
            });
        }
    }

    // Linux ProRes comes from the bundled FFmpeg encoder, which provides the Proxy profile
    // for draft quality whenever the encoder itself is available.
    if availability.profile == ExportProfile::ProResMov && cfg!(target_os = "macos") {
        let proxy_available = cfg!(target_os = "macos")
            && AvFoundationRenderBackend::new()
                .supports_profile(AvFoundationExportProfile::ProResProxy);
        availability.quality_availability.draft = proxy_available;
        availability.quality_unavailable_reasons.draft = (!proxy_available).then(|| {
            "Draft ProRes requires native ProRes Proxy capability; the exporter does not report ProRes Proxy support."
                .to_string()
        });
    }

    availability
}

pub fn approved_export_encoder_command(profile: ExportProfile) -> Option<String> {
    let requirement = primary_encoder_runtime(profile)?;
    approved_runtime_command(&requirement)
}

#[derive(Debug, Clone, Copy)]
struct RuntimeRequirement {
    name: &'static str,
    env_var: &'static str,
}

fn primary_encoder_runtime(profile: ExportProfile) -> Option<RuntimeRequirement> {
    match profile {
        ExportProfile::Webm => None,
        ExportProfile::Mp4H264 => Some(RuntimeRequirement::new(
            "approved-h264-encoder",
            "VIDEO_CREATER_APPROVED_H264_ENCODER",
        )),
        ExportProfile::Mp4H265 => Some(RuntimeRequirement::new(
            "approved-h265-encoder",
            "VIDEO_CREATER_APPROVED_H265_ENCODER",
        )),
        ExportProfile::ProResMov => Some(RuntimeRequirement::new(
            "approved-prores-encoder",
            "VIDEO_CREATER_APPROVED_PRORES_ENCODER",
        )),
        ExportProfile::PalmierProject => None,
    }
}

impl RuntimeRequirement {
    const fn new(name: &'static str, env_var: &'static str) -> Self {
        Self { name, env_var }
    }
}

fn gstreamer_profile_availability(
    profile: ExportProfile,
    label: &str,
    container: &str,
    video_codec: &str,
    audio_codec: Option<&str>,
) -> ExportProfileAvailability {
    let Some(target) = gstreamer_output_profile_target(profile) else {
        return unsupported_profile_availability(
            profile,
            label,
            container,
            video_codec,
            audio_codec,
        );
    };
    let required_factories = target.required_factories();
    let factory_status = gstreamer_factory_status(&target, &required_factories);
    let unavailable_reason = factory_status.unavailable_reason.clone();

    ExportProfileAvailability {
        profile,
        label: label.to_string(),
        available: factory_status.available,
        container: container.to_string(),
        extension: target.extension.to_string(),
        mime_type: target.mime_type.to_string(),
        video_codec: video_codec.to_string(),
        audio_codec: audio_codec.map(str::to_string),
        required_runtime: required_factories
            .iter()
            .map(|factory| format!("gstreamer:{factory}"))
            .collect(),
        policy_status: factory_status.policy_status,
        unavailable_reason: unavailable_reason.clone(),
        quality_availability: ExportQualityAvailability {
            draft: factory_status.available,
            final_quality: factory_status.available,
        },
        quality_unavailable_reasons: ExportQualityUnavailableReasons {
            draft: unavailable_reason.clone(),
            final_quality: unavailable_reason,
        },
    }
}

fn unsupported_profile_availability(
    profile: ExportProfile,
    label: &str,
    container: &str,
    video_codec: &str,
    audio_codec: Option<&str>,
) -> ExportProfileAvailability {
    ExportProfileAvailability {
        profile,
        label: label.to_string(),
        available: false,
        container: container.to_string(),
        extension: String::new(),
        mime_type: String::new(),
        video_codec: video_codec.to_string(),
        audio_codec: audio_codec.map(str::to_string),
        required_runtime: Vec::new(),
        policy_status: ExportPolicyStatus::UnsupportedBuild,
        unavailable_reason: Some("Export profile is not supported by this build.".to_string()),
        quality_availability: ExportQualityAvailability {
            draft: false,
            final_quality: false,
        },
        quality_unavailable_reasons: ExportQualityUnavailableReasons {
            draft: Some("Export profile is not supported by this build.".to_string()),
            final_quality: Some("Export profile is not supported by this build.".to_string()),
        },
    }
}

struct GstreamerFactoryStatus {
    available: bool,
    policy_status: ExportPolicyStatus,
    unavailable_reason: Option<String>,
}

fn gstreamer_factory_status(
    _target: &GstreamerOutputProfileTarget,
    required_factories: &[&'static str],
) -> GstreamerFactoryStatus {
    match probe_gstreamer_factories(required_factories) {
        FactoryProbeResult::UnsupportedBuild(reason) => GstreamerFactoryStatus {
            available: false,
            policy_status: ExportPolicyStatus::UnsupportedBuild,
            unavailable_reason: Some(reason),
        },
        FactoryProbeResult::Missing(factory) => GstreamerFactoryStatus {
            available: false,
            policy_status: ExportPolicyStatus::MissingRuntime,
            unavailable_reason: Some(format!("Missing GStreamer factory: {factory}")),
        },
        FactoryProbeResult::Denied(factory, reason) => GstreamerFactoryStatus {
            available: false,
            policy_status: ExportPolicyStatus::PolicyGated,
            unavailable_reason: Some(format!("Denied GStreamer factory: {factory}: {reason}")),
        },
        FactoryProbeResult::Allowed => GstreamerFactoryStatus {
            available: true,
            policy_status: ExportPolicyStatus::Approved,
            unavailable_reason: None,
        },
    }
}

#[cfg_attr(not(feature = "ges-render"), allow(dead_code))]
enum FactoryProbeResult {
    Allowed,
    Missing(String),
    Denied(String, String),
    UnsupportedBuild(String),
}

#[cfg(feature = "ges-render")]
fn probe_gstreamer_factories(required_factories: &[&'static str]) -> FactoryProbeResult {
    if let Err(error) = render_runtime_environment() {
        return FactoryProbeResult::UnsupportedBuild(format!(
            "GStreamer/GES render runtime is unavailable: {error}"
        ));
    }

    for factory in required_factories {
        let Some(factory_info) = gstreamer_factory_info(factory) else {
            return FactoryProbeResult::Missing((*factory).to_string());
        };
        let decision = evaluate_gstreamer_factory(&factory_info);
        if decision.verdict != PluginPolicyVerdict::Allowed {
            return FactoryProbeResult::Denied(factory_info.name, decision.reason);
        }
    }

    FactoryProbeResult::Allowed
}

#[cfg(feature = "ges-render")]
fn gstreamer_factory_info(factory_name: &str) -> Option<GstFactoryInfo> {
    let factory = gst::ElementFactory::find(factory_name)?;
    let mut info = GstFactoryInfo::new(factory_name);
    if let Some(plugin_name) = factory.plugin_name() {
        info = info.plugin_name(plugin_name.to_string());
    }
    if let Some(plugin) = factory.plugin() {
        info = info.package(plugin.package().to_string());
        info = info.license(plugin.license().to_string());
    }
    Some(info)
}

#[cfg(not(feature = "ges-render"))]
fn probe_gstreamer_factories(_required_factories: &[&'static str]) -> FactoryProbeResult {
    FactoryProbeResult::UnsupportedBuild(
        "GStreamer/GES render backend is not enabled in this build.".to_string(),
    )
}

fn approved_runtime_command(runtime: &RuntimeRequirement) -> Option<String> {
    if let Some(path) = std::env::var_os(runtime.env_var) {
        let path = Path::new(&path);
        if executable_path(path) {
            return Some(path.to_string_lossy().into_owned());
        }
    }
    if command_available(runtime.name) {
        return Some(runtime.name.to_string());
    }
    None
}

fn command_available(command: &str) -> bool {
    let Some(path_var) = std::env::var_os("PATH") else {
        return false;
    };
    std::env::split_paths(&path_var).any(|dir| executable_path(&dir.join(command)))
}

fn executable_path(path: &Path) -> bool {
    if !path.is_file() {
        return false;
    }

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        path.metadata()
            .map(|metadata| metadata.permissions().mode() & 0o111 != 0)
            .unwrap_or(false)
    }

    #[cfg(not(unix))]
    {
        true
    }
}
