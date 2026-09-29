use super::error::{PipelineError, PipelineErrorCode};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GstFactoryInfo {
    pub name: String,
    pub plugin_name: Option<String>,
    pub package: Option<String>,
    pub license: Option<String>,
}

impl GstFactoryInfo {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            plugin_name: None,
            package: None,
            license: None,
        }
    }

    pub fn plugin_name(mut self, plugin_name: impl Into<String>) -> Self {
        self.plugin_name = Some(plugin_name.into());
        self
    }

    pub fn package(mut self, package: impl Into<String>) -> Self {
        self.package = Some(package.into());
        self
    }

    pub fn license(mut self, license: impl Into<String>) -> Self {
        self.license = Some(license.into());
        self
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PluginPolicyVerdict {
    Allowed,
    Denied,
    ReviewRequired,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PluginPolicyDecision {
    pub verdict: PluginPolicyVerdict,
    pub reason: String,
    pub fix: String,
}

pub fn evaluate_gstreamer_factory(factory: &GstFactoryInfo) -> PluginPolicyDecision {
    let name = factory.name.to_ascii_lowercase();
    let plugin_name = normalized_optional(&factory.plugin_name);
    let package = normalized_optional(&factory.package);
    let license = normalized_optional(&factory.license);

    if !is_linux_reviewed_codec(&name, plugin_name.as_deref(), package.as_deref())
        && is_known_denied_factory(&name, plugin_name.as_deref())
        || license.as_deref().is_some_and(is_denied_license)
    {
        return decision(
            PluginPolicyVerdict::Denied,
            format!(
                "GStreamer factory '{}' is denied by the render plugin policy.",
                factory.name
            ),
            "Update Video Creater; this build requested an unsupported render component.",
        );
    }

    if let Some(policy) = reviewed_factory_policy(&name) {
        if matches_reviewed_provenance(policy, plugin_name.as_deref(), package.as_deref())
            && license
                .as_deref()
                .is_some_and(|license| policy.license_predicate.allows(license))
        {
            return decision(
                PluginPolicyVerdict::Allowed,
                format!(
                    "GStreamer factory '{}' is in the reviewed LGPL-compatible allowlist.",
                    factory.name
                ),
                "No action required.",
            );
        }

        return default_denied_decision(factory);
    }

    if package.as_deref().is_some_and(is_bad_plugin_package) {
        return decision(
            PluginPolicyVerdict::ReviewRequired,
            format!(
                "GStreamer factory '{}' is from a plugin set that requires review.",
                factory.name
            ),
            "Review the plugin wrapper, linked libraries, patent implications, and distribution terms before allowing it.",
        );
    }

    default_denied_decision(factory)
}

pub fn policy_error_for_factory(factory: &GstFactoryInfo) -> Option<PipelineError> {
    let decision = evaluate_gstreamer_factory(factory);
    if decision.verdict == PluginPolicyVerdict::Allowed {
        return None;
    }

    let verdict = match decision.verdict {
        PluginPolicyVerdict::Allowed => "allowed",
        PluginPolicyVerdict::Denied => "denied",
        PluginPolicyVerdict::ReviewRequired => "review_required",
    };

    let mut error = PipelineError::new(
        PipelineErrorCode::RenderBackendPolicyDenied,
        format!("gstreamer.plugins.{}", factory.name),
        decision.reason,
        decision.fix,
    )
    .with_detail("factory", factory.name.clone())
    .with_detail("verdict", verdict);
    if let Some(plugin_name) = &factory.plugin_name {
        error = error.with_detail("pluginName", plugin_name.clone());
    }
    if let Some(package) = &factory.package {
        error = error.with_detail("package", package.clone());
    }
    if let Some(license) = &factory.license {
        error = error.with_detail("license", license.clone());
    }

    Some(error)
}

fn default_denied_decision(factory: &GstFactoryInfo) -> PluginPolicyDecision {
    decision(
        PluginPolicyVerdict::Denied,
        format!(
            "GStreamer factory '{}' is not in the reviewed allowlist.",
            factory.name
        ),
        "Update Video Creater; this build requested an unsupported render component.",
    )
}

fn normalized_optional(value: &Option<String>) -> Option<String> {
    value
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_ascii_lowercase)
}

fn is_known_denied_factory(name: &str, plugin_name: Option<&str>) -> bool {
    name == "x264enc"
        || name.starts_with("avenc_")
        || name.starts_with("avdec_")
        || name.contains("fdkaac")
        || matches!(name, "faac" | "voaacenc" | "openh264enc")
        || plugin_name.is_some_and(|plugin| {
            plugin == "libav" || plugin == "x264" || plugin.contains("fdkaac")
        })
}

#[derive(Debug, Clone, Copy)]
struct AllowedFactoryPolicy {
    name: &'static str,
    plugin_names: &'static [&'static str],
    package_families: &'static [&'static str],
    license_predicate: LicensePredicate,
}

#[derive(Debug, Clone, Copy)]
enum LicensePredicate {
    LgplCompatible,
    Permissive,
}

impl LicensePredicate {
    fn allows(self, license: &str) -> bool {
        match self {
            Self::LgplCompatible => is_lgpl_compatible_license(license),
            Self::Permissive => is_reviewed_permissive_license(license),
        }
    }
}

const LGPL_COMPATIBLE: LicensePredicate = LicensePredicate::LgplCompatible;
const PERMISSIVE: LicensePredicate = LicensePredicate::Permissive;
const GSTREAMER_CORE_PACKAGES: &[&str] = &["gstreamer", "gstreamersourcerelease"];
const GSTREAMER_BASE_PACKAGES: &[&str] =
    &["gstreamerbaseplugins", "gstreamerbasepluginssourcerelease"];
const GSTREAMER_GOOD_PACKAGES: &[&str] =
    &["gstreamergoodplugins", "gstreamergoodpluginssourcerelease"];
const GSTREAMER_BAD_PACKAGES: &[&str] =
    &["gstreamerbadplugins", "gstreamerbadpluginssourcerelease"];

const ALLOWED_FACTORY_POLICIES: &[AllowedFactoryPolicy] = &[
    AllowedFactoryPolicy {
        name: "filesrc",
        plugin_names: &["coreelements"],
        package_families: GSTREAMER_CORE_PACKAGES,
        license_predicate: LGPL_COMPATIBLE,
    },
    AllowedFactoryPolicy {
        name: "filesink",
        plugin_names: &["coreelements"],
        package_families: GSTREAMER_CORE_PACKAGES,
        license_predicate: LGPL_COMPATIBLE,
    },
    AllowedFactoryPolicy {
        name: "multifilesrc",
        plugin_names: &["multifile"],
        package_families: GSTREAMER_GOOD_PACKAGES,
        license_predicate: LGPL_COMPATIBLE,
    },
    AllowedFactoryPolicy {
        name: "queue",
        plugin_names: &["coreelements"],
        package_families: GSTREAMER_CORE_PACKAGES,
        license_predicate: LGPL_COMPATIBLE,
    },
    AllowedFactoryPolicy {
        name: "videoconvert",
        plugin_names: &["videoconvertscale", "videoconvert"],
        package_families: GSTREAMER_BASE_PACKAGES,
        license_predicate: LGPL_COMPATIBLE,
    },
    AllowedFactoryPolicy {
        name: "videoscale",
        plugin_names: &["videoconvertscale", "videoscale"],
        package_families: GSTREAMER_BASE_PACKAGES,
        license_predicate: LGPL_COMPATIBLE,
    },
    AllowedFactoryPolicy {
        name: "videorate",
        plugin_names: &["videorate"],
        package_families: GSTREAMER_BASE_PACKAGES,
        license_predicate: LGPL_COMPATIBLE,
    },
    AllowedFactoryPolicy {
        name: "videoflip",
        plugin_names: &["videofilter"],
        package_families: GSTREAMER_GOOD_PACKAGES,
        license_predicate: LGPL_COMPATIBLE,
    },
    AllowedFactoryPolicy {
        name: "videocrop",
        plugin_names: &["videocrop"],
        package_families: GSTREAMER_GOOD_PACKAGES,
        license_predicate: LGPL_COMPATIBLE,
    },
    AllowedFactoryPolicy {
        name: "rotate",
        plugin_names: &["geometrictransform"],
        package_families: GSTREAMER_BAD_PACKAGES,
        license_predicate: LGPL_COMPATIBLE,
    },
    AllowedFactoryPolicy {
        name: "videotestsrc",
        plugin_names: &["videotestsrc"],
        package_families: GSTREAMER_BASE_PACKAGES,
        license_predicate: LGPL_COMPATIBLE,
    },
    AllowedFactoryPolicy {
        name: "pngdec",
        plugin_names: &["png"],
        package_families: GSTREAMER_GOOD_PACKAGES,
        license_predicate: LGPL_COMPATIBLE,
    },
    AllowedFactoryPolicy {
        name: "pngenc",
        plugin_names: &["png"],
        package_families: GSTREAMER_GOOD_PACKAGES,
        license_predicate: LGPL_COMPATIBLE,
    },
    AllowedFactoryPolicy {
        name: "audioconvert",
        plugin_names: &["audioconvert"],
        package_families: GSTREAMER_BASE_PACKAGES,
        license_predicate: LGPL_COMPATIBLE,
    },
    AllowedFactoryPolicy {
        name: "audiomixer",
        plugin_names: &["audiomixer"],
        package_families: GSTREAMER_BASE_PACKAGES,
        license_predicate: LGPL_COMPATIBLE,
    },
    AllowedFactoryPolicy {
        name: "audiorate",
        plugin_names: &["audiorate"],
        package_families: GSTREAMER_BASE_PACKAGES,
        license_predicate: LGPL_COMPATIBLE,
    },
    AllowedFactoryPolicy {
        name: "audiotestsrc",
        plugin_names: &["audiotestsrc"],
        package_families: GSTREAMER_BASE_PACKAGES,
        license_predicate: LGPL_COMPATIBLE,
    },
    AllowedFactoryPolicy {
        name: "audioresample",
        plugin_names: &["audioresample"],
        package_families: GSTREAMER_BASE_PACKAGES,
        license_predicate: LGPL_COMPATIBLE,
    },
    AllowedFactoryPolicy {
        name: "compositor",
        plugin_names: &["compositor"],
        package_families: GSTREAMER_BASE_PACKAGES,
        license_predicate: LGPL_COMPATIBLE,
    },
    AllowedFactoryPolicy {
        name: "gldownload",
        plugin_names: &["opengl"],
        package_families: GSTREAMER_BASE_PACKAGES,
        license_predicate: LGPL_COMPATIBLE,
    },
    AllowedFactoryPolicy {
        name: "videoflip",
        plugin_names: &["videoflip", "videofilter"],
        package_families: GSTREAMER_GOOD_PACKAGES,
        license_predicate: LGPL_COMPATIBLE,
    },
    AllowedFactoryPolicy {
        name: "volume",
        plugin_names: &["volume"],
        package_families: GSTREAMER_BASE_PACKAGES,
        license_predicate: LGPL_COMPATIBLE,
    },
    AllowedFactoryPolicy {
        name: "scaletempo",
        plugin_names: &["audiofx"],
        package_families: GSTREAMER_GOOD_PACKAGES,
        license_predicate: LGPL_COMPATIBLE,
    },
    AllowedFactoryPolicy {
        name: "autoaudiosink",
        plugin_names: &["autodetect"],
        package_families: GSTREAMER_GOOD_PACKAGES,
        license_predicate: LGPL_COMPATIBLE,
    },
    AllowedFactoryPolicy {
        name: "autovideosink",
        plugin_names: &["autodetect"],
        package_families: GSTREAMER_GOOD_PACKAGES,
        license_predicate: LGPL_COMPATIBLE,
    },
    AllowedFactoryPolicy {
        name: "appsrc",
        plugin_names: &["app"],
        package_families: GSTREAMER_BASE_PACKAGES,
        license_predicate: LGPL_COMPATIBLE,
    },
    AllowedFactoryPolicy {
        name: "appsink",
        plugin_names: &["app"],
        package_families: GSTREAMER_BASE_PACKAGES,
        license_predicate: LGPL_COMPATIBLE,
    },
    AllowedFactoryPolicy {
        name: "decodebin",
        plugin_names: &["playback"],
        package_families: GSTREAMER_BASE_PACKAGES,
        license_predicate: LGPL_COMPATIBLE,
    },
    AllowedFactoryPolicy {
        name: "capsfilter",
        plugin_names: &["coreelements"],
        package_families: GSTREAMER_CORE_PACKAGES,
        license_predicate: LGPL_COMPATIBLE,
    },
    AllowedFactoryPolicy {
        name: "mp4mux",
        plugin_names: &["isomp4"],
        package_families: GSTREAMER_GOOD_PACKAGES,
        license_predicate: LGPL_COMPATIBLE,
    },
    AllowedFactoryPolicy {
        name: "qtmux",
        plugin_names: &["isomp4"],
        package_families: GSTREAMER_GOOD_PACKAGES,
        license_predicate: LGPL_COMPATIBLE,
    },
    AllowedFactoryPolicy {
        name: "qtdemux",
        plugin_names: &["isomp4"],
        package_families: GSTREAMER_GOOD_PACKAGES,
        license_predicate: LGPL_COMPATIBLE,
    },
    AllowedFactoryPolicy {
        name: "matroskademux",
        plugin_names: &["matroska"],
        package_families: GSTREAMER_GOOD_PACKAGES,
        license_predicate: LGPL_COMPATIBLE,
    },
    AllowedFactoryPolicy {
        name: "vp8dec",
        plugin_names: &["vpx"],
        package_families: GSTREAMER_GOOD_PACKAGES,
        license_predicate: LGPL_COMPATIBLE,
    },
    AllowedFactoryPolicy {
        name: "vp9dec",
        plugin_names: &["vpx"],
        package_families: GSTREAMER_GOOD_PACKAGES,
        license_predicate: LGPL_COMPATIBLE,
    },
    AllowedFactoryPolicy {
        name: "h264parse",
        plugin_names: &["videoparsersbad"],
        package_families: GSTREAMER_BAD_PACKAGES,
        license_predicate: LGPL_COMPATIBLE,
    },
    AllowedFactoryPolicy {
        name: "h265parse",
        plugin_names: &["videoparsersbad"],
        package_families: GSTREAMER_BAD_PACKAGES,
        license_predicate: LGPL_COMPATIBLE,
    },
    AllowedFactoryPolicy {
        name: "vtdec",
        plugin_names: &["applemedia"],
        package_families: GSTREAMER_BAD_PACKAGES,
        license_predicate: LGPL_COMPATIBLE,
    },
    AllowedFactoryPolicy {
        name: "vtdec_hw",
        plugin_names: &["applemedia"],
        package_families: GSTREAMER_BAD_PACKAGES,
        license_predicate: LGPL_COMPATIBLE,
    },
    AllowedFactoryPolicy {
        name: "aacparse",
        plugin_names: &["audioparsers"],
        package_families: GSTREAMER_GOOD_PACKAGES,
        license_predicate: LGPL_COMPATIBLE,
    },
    AllowedFactoryPolicy {
        name: "atdec",
        plugin_names: &["osxaudio"],
        package_families: GSTREAMER_GOOD_PACKAGES,
        license_predicate: LGPL_COMPATIBLE,
    },
    AllowedFactoryPolicy {
        name: "atenc",
        plugin_names: &["osxaudio"],
        package_families: GSTREAMER_GOOD_PACKAGES,
        license_predicate: LGPL_COMPATIBLE,
    },
    AllowedFactoryPolicy {
        name: "vtenc_h264",
        plugin_names: &["applemedia"],
        package_families: GSTREAMER_BAD_PACKAGES,
        license_predicate: LGPL_COMPATIBLE,
    },
    AllowedFactoryPolicy {
        name: "vtenc_h265",
        plugin_names: &["applemedia"],
        package_families: GSTREAMER_BAD_PACKAGES,
        license_predicate: LGPL_COMPATIBLE,
    },
    AllowedFactoryPolicy {
        name: "vtenc_prores",
        plugin_names: &["applemedia"],
        package_families: GSTREAMER_BAD_PACKAGES,
        license_predicate: LGPL_COMPATIBLE,
    },
    AllowedFactoryPolicy {
        name: "matroskamux",
        plugin_names: &["matroska"],
        package_families: GSTREAMER_GOOD_PACKAGES,
        license_predicate: LGPL_COMPATIBLE,
    },
    AllowedFactoryPolicy {
        name: "webmmux",
        plugin_names: &["matroska"],
        package_families: GSTREAMER_GOOD_PACKAGES,
        license_predicate: LGPL_COMPATIBLE,
    },
    AllowedFactoryPolicy {
        name: "vp8enc",
        plugin_names: &["vpx"],
        package_families: GSTREAMER_GOOD_PACKAGES,
        license_predicate: LGPL_COMPATIBLE,
    },
    AllowedFactoryPolicy {
        name: "vp9enc",
        plugin_names: &["vpx"],
        package_families: GSTREAMER_GOOD_PACKAGES,
        license_predicate: LGPL_COMPATIBLE,
    },
    AllowedFactoryPolicy {
        name: "opusenc",
        plugin_names: &["opus"],
        package_families: GSTREAMER_BASE_PACKAGES,
        license_predicate: LGPL_COMPATIBLE,
    },
    AllowedFactoryPolicy {
        name: "opusdec",
        plugin_names: &["opus"],
        package_families: GSTREAMER_BASE_PACKAGES,
        license_predicate: LGPL_COMPATIBLE,
    },
];

/// The Linux gst-libav plugin is built by Video Creater against a bundled FFmpeg configured
/// with `--disable-gpl --disable-nonfree`. Distribution builds link GPL FFmpeg and keep their
/// own package name, so they never match this provenance.
const VIDEO_CREATER_LIBAV_PACKAGES: &[&str] = &["videocreaterlgplffmpegplugins"];

const fn libav_policy(name: &'static str) -> AllowedFactoryPolicy {
    AllowedFactoryPolicy {
        name,
        plugin_names: &["libav"],
        package_families: VIDEO_CREATER_LIBAV_PACKAGES,
        license_predicate: LGPL_COMPATIBLE,
    }
}

const fn linux_policy(
    name: &'static str,
    plugin_names: &'static [&'static str],
    package_families: &'static [&'static str],
    license_predicate: LicensePredicate,
) -> AllowedFactoryPolicy {
    AllowedFactoryPolicy {
        name,
        plugin_names,
        package_families,
        license_predicate,
    }
}

/// Codec and container factories reviewed for Linux, where Apple media frameworks are absent.
const LINUX_CODEC_FACTORY_POLICIES: &[AllowedFactoryPolicy] = &[
    libav_policy("avdec_h264"),
    libav_policy("avdec_h265"),
    libav_policy("avdec_aac"),
    libav_policy("avdec_aac_fixed"),
    libav_policy("avdec_aac_latm"),
    libav_policy("avdec_mp3"),
    libav_policy("avdec_mp3float"),
    libav_policy("avdec_prores"),
    libav_policy("avdec_mpeg4"),
    libav_policy("avdec_mjpeg"),
    libav_policy("avdec_dnxhd"),
    libav_policy("avdec_ac3"),
    libav_policy("avdec_eac3"),
    libav_policy("avdec_alac"),
    libav_policy("avenc_aac"),
    libav_policy("avenc_prores_ks"),
    linux_policy(
        "openh264enc",
        &["openh264"],
        GSTREAMER_BAD_PACKAGES,
        PERMISSIVE,
    ),
    linux_policy(
        "openh264dec",
        &["openh264"],
        GSTREAMER_BAD_PACKAGES,
        PERMISSIVE,
    ),
    linux_policy(
        "vah264dec",
        &["va"],
        GSTREAMER_BAD_PACKAGES,
        LGPL_COMPATIBLE,
    ),
    linux_policy(
        "vah265dec",
        &["va"],
        GSTREAMER_BAD_PACKAGES,
        LGPL_COMPATIBLE,
    ),
    linux_policy(
        "vah264enc",
        &["va"],
        GSTREAMER_BAD_PACKAGES,
        LGPL_COMPATIBLE,
    ),
    linux_policy(
        "vah265enc",
        &["va"],
        GSTREAMER_BAD_PACKAGES,
        LGPL_COMPATIBLE,
    ),
    linux_policy(
        "vorbisdec",
        &["vorbis"],
        GSTREAMER_BASE_PACKAGES,
        LGPL_COMPATIBLE,
    ),
    linux_policy(
        "theoradec",
        &["theora"],
        GSTREAMER_BASE_PACKAGES,
        LGPL_COMPATIBLE,
    ),
    linux_policy(
        "oggdemux",
        &["ogg"],
        GSTREAMER_BASE_PACKAGES,
        LGPL_COMPATIBLE,
    ),
    linux_policy(
        "flacdec",
        &["flac"],
        GSTREAMER_GOOD_PACKAGES,
        LGPL_COMPATIBLE,
    ),
    linux_policy(
        "jpegdec",
        &["jpeg"],
        GSTREAMER_GOOD_PACKAGES,
        LGPL_COMPATIBLE,
    ),
    linux_policy(
        "wavparse",
        &["wavparse"],
        GSTREAMER_GOOD_PACKAGES,
        LGPL_COMPATIBLE,
    ),
    linux_policy(
        "avidemux",
        &["avi"],
        GSTREAMER_GOOD_PACKAGES,
        LGPL_COMPATIBLE,
    ),
    linux_policy(
        "id3demux",
        &["id3demux"],
        GSTREAMER_GOOD_PACKAGES,
        LGPL_COMPATIBLE,
    ),
];

fn reviewed_factory_policy(name: &str) -> Option<&'static AllowedFactoryPolicy> {
    ALLOWED_FACTORY_POLICIES
        .iter()
        .chain(platform_codec_factory_policies())
        .find(|policy| policy.name == name)
}

fn platform_codec_factory_policies() -> &'static [AllowedFactoryPolicy] {
    if cfg!(target_os = "linux") {
        LINUX_CODEC_FACTORY_POLICIES
    } else {
        &[]
    }
}

fn is_linux_reviewed_codec(name: &str, plugin_name: Option<&str>, package: Option<&str>) -> bool {
    platform_codec_factory_policies()
        .iter()
        .find(|policy| policy.name == name)
        .is_some_and(|policy| matches_reviewed_provenance(policy, plugin_name, package))
}

fn matches_reviewed_provenance(
    policy: &AllowedFactoryPolicy,
    plugin_name: Option<&str>,
    package: Option<&str>,
) -> bool {
    let plugin_matches = plugin_name.is_some_and(|plugin| policy.plugin_names.contains(&plugin));
    let package_matches = package
        .map(canonical_package_family)
        .as_deref()
        .is_some_and(|package| policy.package_families.contains(&package));

    plugin_matches && package_matches
}

fn is_bad_plugin_package(package: &str) -> bool {
    let package = canonical_package_family(package);

    package.contains("badplugins") || package.contains("pluginsbad")
}

fn canonical_package_family(package: &str) -> String {
    // Linux distribution builds append their vendor, e.g. "GStreamer Base Plug-ins (Ubuntu)".
    // macOS runtimes keep their reviewed exact package spelling.
    let package = package.trim();
    let package = if cfg!(target_os = "linux") {
        package
            .strip_suffix(')')
            .and_then(|without_close| without_close.rsplit_once('('))
            .map(|(family, _vendor)| family)
            .unwrap_or(package)
    } else {
        package
    };
    package
        .chars()
        .filter(|character| character.is_ascii_alphanumeric())
        .collect()
}

fn is_denied_license(license: &str) -> bool {
    let compact_license = compact_alphanumeric(license);
    let tokens: Vec<&str> = license_tokens(license).collect();

    compact_license.contains("nonfree")
        || compact_license.contains("proprietary")
        || contains_standalone_gpl_marker(&compact_license)
        || tokens.iter().any(|token| is_gpl_license_token(token))
        || contains_token_phrase(&tokens, &["gnu", "general", "public", "license"])
}

fn is_lgpl_compatible_license(license: &str) -> bool {
    let tokens: Vec<&str> = license_tokens(license).collect();

    let has_lgpl_marker = tokens.iter().any(|token| is_lgpl_license_token(token))
        || contains_token_phrase(&tokens, &["gnu", "lesser", "general", "public", "license"]);

    has_lgpl_marker
        && !tokens.is_empty()
        && tokens.iter().all(|token| is_lgpl_license_component(token))
        && has_valid_lgpl_or_later_usage(&tokens)
}

fn is_reviewed_permissive_license(license: &str) -> bool {
    matches!(
        compact_alphanumeric(license).as_str(),
        "bsd" | "bsd2clause" | "bsd3clause" | "mit" | "mitx11" | "x11" | "isc" | "zlib"
    )
}

fn is_gpl_license_token(token: &str) -> bool {
    token == "gpl"
        || token
            .strip_prefix("gpl")
            .is_some_and(is_license_version_suffix)
}

fn is_lgpl_license_token(token: &str) -> bool {
    token == "lgpl"
        || token
            .strip_prefix("lgpl")
            .is_some_and(is_license_version_suffix)
}

fn is_lgpl_license_component(token: &str) -> bool {
    is_lgpl_license_token(token)
        || matches!(
            token,
            "gnu" | "lesser" | "general" | "public" | "license" | "only" | "or" | "later"
        )
        || is_license_version_token(token)
}

fn has_valid_lgpl_or_later_usage(tokens: &[&str]) -> bool {
    tokens
        .iter()
        .enumerate()
        .all(|(index, token)| match *token {
            "or" => tokens.get(index + 1).copied() == Some("later"),
            "later" => index > 0 && tokens.get(index - 1).copied() == Some("or"),
            _ => true,
        })
}

fn is_license_version_token(token: &str) -> bool {
    let token = token.strip_prefix('v').unwrap_or(token);

    !token.is_empty() && token.chars().all(|character| character.is_ascii_digit())
}

fn is_license_version_suffix(suffix: &str) -> bool {
    let suffix = suffix.strip_prefix('v').unwrap_or(suffix);

    !suffix.is_empty() && suffix.chars().all(|character| character.is_ascii_digit())
}

fn contains_standalone_gpl_marker(compact_license: &str) -> bool {
    compact_license.match_indices("gpl").any(|(index, _)| {
        index == 0 || compact_license.as_bytes().get(index - 1).copied() != Some(b'l')
    })
}

fn license_tokens(license: &str) -> impl Iterator<Item = &str> {
    license
        .split(|character: char| !character.is_ascii_alphanumeric())
        .filter(|token| !token.is_empty())
}

fn compact_alphanumeric(value: &str) -> String {
    value
        .chars()
        .filter(|character| character.is_ascii_alphanumeric())
        .collect()
}

fn contains_token_phrase(tokens: &[&str], phrase: &[&str]) -> bool {
    tokens.windows(phrase.len()).any(|window| window == phrase)
}

fn decision(
    verdict: PluginPolicyVerdict,
    reason: impl Into<String>,
    fix: impl Into<String>,
) -> PluginPolicyDecision {
    PluginPolicyDecision {
        verdict,
        reason: reason.into(),
        fix: fix.into(),
    }
}
