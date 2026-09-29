use crate::project::export_profiles::ExportProfile;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GstreamerAudioMode {
    EncodedAac,
    RawPcm,
    Opus,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GstreamerOutputProfileTarget {
    pub profile: ExportProfile,
    pub extension: &'static str,
    pub mime_type: &'static str,
    pub container_factory: &'static str,
    pub container_caps: &'static str,
    pub video_factory: &'static str,
    pub video_caps: &'static str,
    pub audio_factory: Option<&'static str>,
    pub audio_caps: Option<&'static str>,
    pub audio_mode: GstreamerAudioMode,
    pub parser_factories: Vec<&'static str>,
    pub macos_only: bool,
}

impl GstreamerOutputProfileTarget {
    pub fn required_factories(&self) -> Vec<&'static str> {
        let mut factories = vec![self.container_factory, self.video_factory];
        if let Some(audio_factory) = self.audio_factory {
            factories.push(audio_factory);
        }
        factories.extend(self.parser_factories.iter().copied());
        factories
    }
}

/// Returns the GStreamer delivery target for this platform's reviewed codec set.
pub fn gstreamer_output_profile_target(
    profile: ExportProfile,
) -> Option<GstreamerOutputProfileTarget> {
    if cfg!(target_os = "macos") {
        macos_output_profile_target(profile)
    } else {
        linux_output_profile_target(profile)
    }
}

/// Linux delivery uses the bundled LGPL FFmpeg plugin (AAC, ProRes), OpenH264 (BSD) for
/// H.264, and VA-API hardware encoding for HEVC.
pub fn linux_output_profile_target(profile: ExportProfile) -> Option<GstreamerOutputProfileTarget> {
    match profile {
        ExportProfile::Webm => macos_output_profile_target(profile),
        ExportProfile::Mp4H264 => Some(GstreamerOutputProfileTarget {
            profile,
            extension: "mp4",
            mime_type: "video/mp4",
            container_factory: "mp4mux",
            container_caps: "video/quicktime",
            video_factory: "openh264enc",
            video_caps: "video/x-h264",
            audio_factory: Some("avenc_aac"),
            audio_caps: Some("audio/mpeg,mpegversion=4"),
            audio_mode: GstreamerAudioMode::EncodedAac,
            parser_factories: vec!["h264parse", "aacparse"],
            macos_only: false,
        }),
        ExportProfile::Mp4H265 => Some(GstreamerOutputProfileTarget {
            profile,
            extension: "mp4",
            mime_type: "video/mp4",
            container_factory: "mp4mux",
            container_caps: "video/quicktime",
            video_factory: "vah265enc",
            video_caps: "video/x-h265",
            audio_factory: Some("avenc_aac"),
            audio_caps: Some("audio/mpeg,mpegversion=4"),
            audio_mode: GstreamerAudioMode::EncodedAac,
            parser_factories: vec!["h265parse", "aacparse"],
            macos_only: false,
        }),
        ExportProfile::ProResMov => Some(GstreamerOutputProfileTarget {
            profile,
            extension: "mov",
            mime_type: "video/quicktime",
            container_factory: "qtmux",
            container_caps: "video/quicktime",
            video_factory: "avenc_prores_ks",
            video_caps: "video/x-prores",
            audio_factory: None,
            audio_caps: Some("audio/x-raw"),
            audio_mode: GstreamerAudioMode::RawPcm,
            parser_factories: Vec::new(),
            macos_only: false,
        }),
        ExportProfile::PalmierProject => None,
    }
}

/// macOS delivery uses VideoToolbox and AudioToolbox through `applemedia`/`osxaudio`.
pub fn macos_output_profile_target(profile: ExportProfile) -> Option<GstreamerOutputProfileTarget> {
    match profile {
        ExportProfile::Webm => Some(GstreamerOutputProfileTarget {
            profile,
            extension: "webm",
            mime_type: "video/webm",
            container_factory: "webmmux",
            container_caps: "video/webm",
            video_factory: "vp8enc",
            video_caps: "video/x-vp8",
            audio_factory: Some("opusenc"),
            audio_caps: Some("audio/x-opus"),
            audio_mode: GstreamerAudioMode::Opus,
            parser_factories: Vec::new(),
            macos_only: false,
        }),
        ExportProfile::Mp4H264 => Some(GstreamerOutputProfileTarget {
            profile,
            extension: "mp4",
            mime_type: "video/mp4",
            container_factory: "mp4mux",
            container_caps: "video/quicktime",
            video_factory: "vtenc_h264",
            video_caps: "video/x-h264",
            audio_factory: Some("atenc"),
            audio_caps: Some("audio/mpeg"),
            audio_mode: GstreamerAudioMode::EncodedAac,
            parser_factories: vec!["aacparse"],
            macos_only: true,
        }),
        ExportProfile::Mp4H265 => Some(GstreamerOutputProfileTarget {
            profile,
            extension: "mp4",
            mime_type: "video/mp4",
            container_factory: "mp4mux",
            container_caps: "video/quicktime",
            video_factory: "vtenc_h265",
            video_caps: "video/x-h265",
            audio_factory: Some("atenc"),
            audio_caps: Some("audio/mpeg"),
            audio_mode: GstreamerAudioMode::EncodedAac,
            parser_factories: vec!["aacparse"],
            macos_only: true,
        }),
        ExportProfile::ProResMov => Some(GstreamerOutputProfileTarget {
            profile,
            extension: "mov",
            mime_type: "video/quicktime",
            container_factory: "qtmux",
            container_caps: "video/quicktime",
            video_factory: "vtenc_prores",
            video_caps: "video/x-prores",
            audio_factory: None,
            audio_caps: Some("audio/x-raw"),
            audio_mode: GstreamerAudioMode::RawPcm,
            parser_factories: Vec::new(),
            macos_only: true,
        }),
        ExportProfile::PalmierProject => None,
    }
}
