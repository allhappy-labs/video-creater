//! The DaVinci FCPXML document: resources, the primary spine, and the
//! asset clips placed in it. Titles live in `fcpxml_titles.rs`, transitions
//! in `fcpxml_transitions.rs`, and connected storylines in
//! `fcpxml_storylines.rs`.

use super::fcpxml_storylines::{
    anchored_fcpxml_storylines, plan_fcpxml_storylines, push_fcpxml_gap,
};
use super::fcpxml_titles::{
    push_fcpxml_basic_title_effect, push_fcpxml_caption, push_fcpxml_text_overlay,
};
use super::fcpxml_transitions::{
    fcpxml_emits, push_fcpxml_transition, push_fcpxml_transition_effects,
};
use super::{
    clip_export_note, fcpxml_clip_start, fcpxml_opacity_node, fcpxml_static_audio_volume_node,
    fcpxml_static_crop_node, fcpxml_time_map_node, fcpxml_transform_node, file_name,
    nle_clip_limitation_note, push_text_element, seconds_to_frames, xml_escape, NleCaption,
    NleClip, NleTextOverlay,
};
use crate::project::model::{MediaAsset, MediaKind, TrackKind, VideoProject};

pub(super) fn render_davinci_fcpxml(
    project: &VideoProject,
    clips: &[NleClip<'_>],
    captions: &[NleCaption<'_>],
    text_overlays: &[NleTextOverlay<'_>],
    fps: i64,
) -> String {
    let duration_frames = seconds_to_frames(project.timeline.duration_seconds, fps);
    let mut xml = String::new();
    xml.push_str("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n");
    xml.push_str("<fcpxml version=\"1.10\">\n");
    xml.push_str("  <resources>\n");
    xml.push_str(&format!(
        "    <format id=\"r1\" frameDuration=\"1/{}s\" width=\"{}\" height=\"{}\"/>\n",
        fps, project.render_settings.width, project.render_settings.height
    ));
    push_fcpxml_transition_effects(&mut xml, clips);
    if !captions.is_empty() || !text_overlays.is_empty() {
        push_fcpxml_basic_title_effect(&mut xml);
    }
    for media in project
        .media
        .iter()
        .filter(|media| !matches!(media.kind, MediaKind::Lottie))
    {
        push_fcpxml_asset(&mut xml, media, fps);
    }
    xml.push_str("  </resources>\n");
    xml.push_str("  <library>\n");
    xml.push_str("    <event name=\"Video Creater Export\">\n");
    xml.push_str(&format!(
        "      <project name=\"{}\">\n",
        xml_escape(&project.name)
    ));
    xml.push_str(&format!(
        "        <sequence format=\"r1\" duration=\"{}/{}s\">\n",
        duration_frames, fps
    ));
    xml.push_str("          <spine>\n");
    let plan = plan_fcpxml_storylines(clips, captions, duration_frames);
    // Captions sit above every video lane and the text overlays on lane 1.
    let caption_lane = clips
        .iter()
        .filter(|clip| clip.track_kind == TrackKind::Video)
        .map(|clip| clip.track_lane)
        .max()
        .unwrap_or(0)
        .max(1)
        + 1;
    // The DTD's spine holds "elements ordered serially in time": each gap goes before the first
    // lane-0 clip after it. A gap fills a primary hole, so it never splits a primary transition
    // from its clips.
    let mut gaps = (0..plan.gaps.len()).peekable();
    let chained = &plan.chained;
    let flat = |kind: TrackKind| {
        clips
            .iter()
            .enumerate()
            .filter(move |(index, clip)| clip.track_kind == kind && !chained.contains(index))
    };
    for (index, clip) in flat(TrackKind::Video) {
        if clip.track_lane == 0 {
            while let Some(gap_index) =
                gaps.next_if(|&gap| plan.gaps[gap].start_frames < clip.timeline_start_frames)
            {
                push_fcpxml_gap(
                    &mut xml,
                    &plan,
                    gap_index,
                    (clips, captions),
                    caption_lane,
                    fps,
                );
            }
        }
        let anchored =
            anchored_fcpxml_storylines(&plan, index, (clips, captions), caption_lane, fps);
        push_fcpxml_video_clip(&mut xml, clip, fps, FcpxmlPlacement::flat(clip), &anchored);
        if let Some(span) = clip.outgoing_transition.filter(fcpxml_emits) {
            push_fcpxml_transition(&mut xml, span, span.start_frames, fps, 12);
        }
    }
    for gap_index in gaps {
        push_fcpxml_gap(
            &mut xml,
            &plan,
            gap_index,
            (clips, captions),
            caption_lane,
            fps,
        );
    }
    for (_, clip) in flat(TrackKind::Audio) {
        push_fcpxml_audio_clip(&mut xml, clip, fps, FcpxmlPlacement::flat(clip));
    }
    // Captions whose covering clip is retimed stay connected directly in the spine.
    for (caption, _) in captions
        .iter()
        .zip(&plan.caption_hosts)
        .filter(|(_, host)| host.is_none())
    {
        let offset = caption.timeline_start_frames;
        push_fcpxml_caption(&mut xml, caption, caption_lane, offset, fps, 12);
    }
    for overlay in text_overlays {
        push_fcpxml_text_overlay(&mut xml, overlay, fps);
    }
    xml.push_str("          </spine>\n");
    xml.push_str("        </sequence>\n");
    xml.push_str("      </project>\n");
    xml.push_str("    </event>\n");
    xml.push_str("  </library>\n");
    xml.push_str("</fcpxml>\n");
    xml
}

/// Where a clip is written: its `lane` attribute (none inside a storyline),
/// its `offset` in frames within its spine, and its indentation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct FcpxmlPlacement {
    pub(super) lane: Option<i64>,
    pub(super) offset_frames: i64,
    pub(super) indent: usize,
}

impl FcpxmlPlacement {
    /// A clip placed directly in the sequence spine, as before storylines.
    fn flat(clip: &NleClip<'_>) -> Self {
        Self {
            lane: (clip.track_lane != 0).then_some(clip.track_lane),
            offset_frames: clip.timeline_start_frames,
            indent: 12,
        }
    }

    fn attributes(self, clip: &NleClip<'_>, fps: i64) -> String {
        let lane_attribute = self
            .lane
            .map(|lane| format!(" lane=\"{lane}\""))
            .unwrap_or_default();
        format!(
            "name=\"{}\" ref=\"{}\"{} offset=\"{}/{}s\" duration=\"{}/{}s\" start=\"{}\"{}{}",
            xml_escape(&clip.item.label),
            xml_escape(&clip.media.id),
            lane_attribute,
            self.offset_frames,
            fps,
            clip.duration_frames,
            fps,
            fcpxml_clip_start(clip, fps),
            fcpxml_enabled_attribute(clip),
            fcpxml_source_enable_attribute(clip)
        )
    }
}

/// Writes a video clip. `anchored` holds the connected storylines it hosts,
/// already indented, written after its intrinsic params.
pub(super) fn push_fcpxml_video_clip(
    xml: &mut String,
    clip: &NleClip<'_>,
    fps: i64,
    placement: FcpxmlPlacement,
    anchored: &str,
) {
    let children = fcpxml_visual_clip_children(clip, fps);
    push_fcpxml_asset_clip(
        xml,
        &placement.attributes(clip, fps),
        &children,
        clip_export_note(
            clip.generated_asset,
            nle_clip_limitation_note(clip, "davinciFcpxml"),
        ),
        anchored,
        placement.indent,
    );
}

pub(super) fn push_fcpxml_audio_clip(
    xml: &mut String,
    clip: &NleClip<'_>,
    fps: i64,
    placement: FcpxmlPlacement,
) {
    let attributes = format!("{} audioRole=\"dialogue\"", placement.attributes(clip, fps));
    let children = fcpxml_audio_clip_children(clip, fps);
    push_fcpxml_asset_clip(
        xml,
        &attributes,
        &children,
        clip_export_note(
            clip.generated_asset,
            nle_clip_limitation_note(clip, "davinciFcpxml"),
        ),
        "",
        placement.indent,
    );
}

pub(super) fn push_fcpxml_asset(xml: &mut String, media: &MediaAsset, fps: i64) {
    let duration_frames = seconds_to_frames(media.duration_seconds, fps);
    let media_attributes = match media.kind {
        MediaKind::Audio => "hasAudio=\"1\" audioSources=\"1\" audioChannels=\"2\"".to_string(),
        MediaKind::Video | MediaKind::Generated => {
            "hasVideo=\"1\" format=\"r1\" hasAudio=\"1\" audioSources=\"1\" audioChannels=\"2\""
                .to_string()
        }
        MediaKind::Image => "hasVideo=\"1\" format=\"r1\"".to_string(),
        MediaKind::Lottie => return,
    };
    xml.push_str(&format!(
        "    <asset id=\"{}\" name=\"{}\" start=\"0/1s\" duration=\"{}/{}s\" {}>\n",
        xml_escape(&media.id),
        xml_escape(&file_name(&media.relative_path)),
        duration_frames,
        fps,
        media_attributes,
    ));
    xml.push_str(&format!(
        "      <media-rep kind=\"original-media\" src=\"file://{}\"/>\n",
        xml_escape(&media.relative_path)
    ));
    xml.push_str("    </asset>\n");
}

pub(super) fn push_fcpxml_asset_clip(
    xml: &mut String,
    attributes: &str,
    children: &[String],
    note: Option<String>,
    anchored: &str,
    indent: usize,
) {
    let pad = " ".repeat(indent);
    if children.is_empty() && note.is_none() && anchored.is_empty() {
        xml.push_str(&format!("{pad}<asset-clip {attributes}/>\n"));
        return;
    }

    xml.push_str(&format!("{pad}<asset-clip {attributes}>\n"));
    // The DTD puts `note?` before the timing and intrinsic params.
    if let Some(note) = note {
        push_text_element(xml, indent + 2, "note", &note);
    }
    // Child nodes carry their inner lines indented for a clip at 12 spaces.
    let nested_pad = format!("\n{}", " ".repeat(indent - 12));
    for child in children {
        xml.push_str(&pad);
        xml.push_str("  ");
        xml.push_str(&child.replace('\n', &nested_pad));
        xml.push('\n');
    }
    xml.push_str(anchored);
    xml.push_str(&format!("{pad}</asset-clip>\n"));
}

pub(super) fn fcpxml_visual_clip_children(clip: &NleClip<'_>, fps: i64) -> Vec<String> {
    let mut children = Vec::new();
    if let Some(time_map) = fcpxml_time_map_node(clip, fps) {
        children.push(time_map);
    }
    if let Some(crop) = fcpxml_static_crop_node(clip) {
        children.push(crop);
    }
    if let Some(transform) = fcpxml_transform_node(clip, fps) {
        children.push(transform);
    }
    if let Some(opacity) = fcpxml_opacity_node(clip.item, fps) {
        children.push(opacity);
    }
    if let Some(volume) = fcpxml_static_audio_volume_node(clip.item) {
        children.push(volume);
    }
    children
}

pub(super) fn fcpxml_audio_clip_children(clip: &NleClip<'_>, fps: i64) -> Vec<String> {
    let mut children = Vec::new();
    if let Some(time_map) = fcpxml_time_map_node(clip, fps) {
        children.push(time_map);
    }
    if let Some(volume) = fcpxml_static_audio_volume_node(clip.item) {
        children.push(volume);
    }
    children
}

pub(super) fn fcpxml_enabled_attribute(clip: &NleClip<'_>) -> &'static str {
    if clip.track_enabled {
        ""
    } else {
        " enabled=\"0\""
    }
}

/// FCPXML `srcEnable` for clips whose asset has both video and audio: a video
/// clip with detached sound plays only its video, and an audio clip reading
/// video media plays only its audio.
fn fcpxml_source_enable_attribute(clip: &NleClip<'_>) -> &'static str {
    let media_has_video = matches!(clip.media.kind, MediaKind::Video | MediaKind::Generated);
    if !media_has_video {
        return "";
    }
    match clip.track_kind {
        TrackKind::Audio => " srcEnable=\"audio\"",
        _ if super::audio_is_detached(clip.item) => " srcEnable=\"video\"",
        _ => "",
    }
}
