//! NLE export of retimed audio clips and of a video clip's detached sound.
//!
//! Detached sound is an audio clip whose source is the video's own media, and
//! the video is marked `audioDetached`. Each format must carry that sound once:
//! - XMEML writes no source-audio clipitem for the video, whatever volume the
//!   video still carries.
//! - FCPXML enables only the video of the video's asset clip and only the
//!   audio of the audio clip, since both reference an asset with video and
//!   audio (FCPXML 1.10 DTD: `srcEnable (all | audio | video) "all"`).

use serde_json::json;
use std::collections::BTreeMap;
use video_creater_lib::project::action::{apply_project_action, ProjectAction};
use video_creater_lib::project::fixtures::sample_project;
use video_creater_lib::project::model::{
    MediaAsset, MediaKind, TimelineItem, TimelineItemKind, TimelineSource, VideoProject,
};
use video_creater_lib::project::nle_export::{export_project_timeline_to_nle_xml, NleXmlFormat};

fn xml(project: &VideoProject, format: NleXmlFormat) -> String {
    export_project_timeline_to_nle_xml(project, format)
        .expect("export")
        .xml
}

fn audio_track_index(project: &VideoProject) -> usize {
    project
        .timeline
        .tracks
        .iter()
        .position(|track| track.id == "track-audio")
        .expect("audio track")
}

/// `sample_project` plus a 10 s music file played at speed 2 over 0-2 s from
/// source 1-5 s.
fn retimed_audio_project() -> VideoProject {
    let mut project = sample_project();
    project.media.push(MediaAsset {
        id: "music".to_string(),
        name: None,
        relative_path: "media/music.wav".to_string(),
        kind: MediaKind::Audio,
        duration_seconds: 10.0,
        width: None,
        height: None,
        fps: None,
        folder_id: None,
    });
    let audio_track = audio_track_index(&project);
    project.timeline.tracks[audio_track]
        .items
        .push(TimelineItem {
            id: "music-clip".to_string(),
            kind: TimelineItemKind::AudioClip,
            start_seconds: 0.0,
            duration_seconds: 2.0,
            source: TimelineSource::Media {
                media_id: "music".to_string(),
            },
            label: "Music".to_string(),
            properties: BTreeMap::from([
                ("sourceIn".to_string(), json!(1.0)),
                ("sourceOut".to_string(), json!(5.0)),
                ("speed".to_string(), json!(2.0)),
            ]),
        });
    project
}

/// `sample_project` with the opening clip's sound (at -6 dB) detached.
fn detached_project() -> VideoProject {
    let mut project = sample_project();
    project.timeline.tracks[0].items[0].properties = BTreeMap::from([
        ("sourceIn".to_string(), json!(1.0)),
        ("sourceOut".to_string(), json!(5.0)),
        ("volumeDb".to_string(), json!(-6.0)),
    ]);
    apply_project_action(
        &mut project,
        ProjectAction::DetachAudio {
            item_id: "item-1".to_string(),
            audio_item_id: "item-1-audio".to_string(),
            target_track_id: "track-audio".to_string(),
            link_group_id: "link-item-1".to_string(),
        },
    )
    .expect("detach audio");
    project
}

#[test]
fn fcpxml_retimed_audio_clip_writes_a_time_map_and_scaled_start() {
    let xml = xml(&retimed_audio_project(), NleXmlFormat::DavinciFcpxml);
    let open = xml
        .find("<asset-clip name=\"Music\"")
        .unwrap_or_else(|| panic!("music clip: {xml}"));
    let clip = &xml[open..open + xml[open..].find("</asset-clip>").expect("clip end")];
    // Source 1 s at speed 2 starts half a second into the retimed clip.
    assert!(
        clip.starts_with(
            "<asset-clip name=\"Music\" ref=\"music\" lane=\"-1\" offset=\"0/24s\" duration=\"48/24s\" start=\"1/2s\""
        ),
        "{clip}"
    );
    assert!(clip.contains("<timeMap frameSampling=\"floor\">"), "{clip}");
    // 10 s of media play over 5 s.
    assert!(
        clip.contains("<timept time=\"5s\" value=\"10s\" interp=\"linear\"/>"),
        "{clip}"
    );
}

#[test]
fn xmeml_notes_speed_it_does_not_write_for_retimed_clips() {
    let mut project = retimed_audio_project();
    project.timeline.tracks[0].items[0]
        .properties
        .insert("speed".to_string(), json!(2.0));
    let xml = xml(&project, NleXmlFormat::PremiereXmeml);
    for id in ["item-1", "music-clip"] {
        let open = xml
            .find(&format!("<clipitem id=\"{id}\">"))
            .unwrap_or_else(|| panic!("{id}: {xml}"));
        let clip = &xml[open..open + xml[open..].find("</clipitem>").expect("clip end")];
        assert!(
            clip.contains(
                "<description>Video Creater NLE export limitation: speed are not round-tripped into premiereXmeml;"
            ),
            "{id}: {clip}"
        );
    }

    let plain = xml_without_speed();
    assert!(!plain.contains("speed"), "{plain}");
}

fn xml_without_speed() -> String {
    let mut project = retimed_audio_project();
    let audio_track = audio_track_index(&project);
    let music = &mut project.timeline.tracks[audio_track].items[0];
    music.properties.remove("speed");
    music.properties.insert("sourceOut".to_string(), json!(3.0));
    xml(&project, NleXmlFormat::PremiereXmeml)
}

#[test]
fn xmeml_writes_detached_sound_once() {
    let project = detached_project();
    let xml = xml(&project, NleXmlFormat::PremiereXmeml);
    assert_eq!(
        xml.matches("<clipitem id=\"item-1-audio\">").count(),
        1,
        "{xml}"
    );
    assert!(!xml.contains("<masterclipid>"), "{xml}");

    // A volume set on the video after detaching does not bring its source
    // audio back.
    let mut with_volume = project;
    with_volume.timeline.tracks[0].items[0]
        .properties
        .insert("volumeDb".to_string(), json!(-3.0));
    let xml = self::xml(&with_volume, NleXmlFormat::PremiereXmeml);
    assert_eq!(
        xml.matches("<clipitem id=\"item-1-audio\">").count(),
        1,
        "{xml}"
    );
    assert!(!xml.contains("<masterclipid>"), "{xml}");
}

#[test]
fn fcpxml_writes_detached_sound_once() {
    let xml = xml(&detached_project(), NleXmlFormat::DavinciFcpxml);
    let clips = xml
        .lines()
        .map(str::trim)
        .filter(|line| line.starts_with("<asset-clip "))
        .collect::<Vec<_>>();
    assert_eq!(clips.len(), 2, "{xml}");
    let video = clips
        .iter()
        .find(|line| line.contains("name=\"Opening clip\""))
        .unwrap_or_else(|| panic!("video clip: {xml}"));
    let audio = clips
        .iter()
        .find(|line| line.contains("audioRole=\"dialogue\""))
        .unwrap_or_else(|| panic!("audio clip: {xml}"));
    assert!(video.contains(" srcEnable=\"video\""), "{video}");
    assert!(!video.contains("audioRole"), "{video}");
    assert!(audio.contains("ref=\"media-1\""), "{audio}");
    assert!(audio.contains(" srcEnable=\"audio\""), "{audio}");
}

#[test]
fn fcpxml_audio_from_audio_media_enables_all_sources() {
    let xml = xml(&retimed_audio_project(), NleXmlFormat::DavinciFcpxml);
    assert!(!xml.contains("srcEnable"), "{xml}");
}

#[test]
fn both_formats_note_reverse_they_do_not_write() {
    let mut project = retimed_audio_project();
    let audio_track = audio_track_index(&project);
    for track in [0, audio_track] {
        project.timeline.tracks[track].items[0]
            .properties
            .insert("reverse".to_string(), json!(true));
    }

    let xmeml = xml(&project, NleXmlFormat::PremiereXmeml);
    let open = xmeml
        .find("<clipitem id=\"item-1\">")
        .unwrap_or_else(|| panic!("item-1: {xmeml}"));
    let clip = &xmeml[open..open + xmeml[open..].find("</clipitem>").expect("clip end")];
    assert!(
        clip.contains(
            "<description>Video Creater NLE export limitation: reverse are not round-tripped into premiereXmeml;"
        ),
        "{clip}"
    );
    assert!(
        xmeml.contains("limitation: speed and reverse are not round-tripped into premiereXmeml;"),
        "{xmeml}"
    );

    let fcpxml = xml(&project, NleXmlFormat::DavinciFcpxml);
    assert!(
        fcpxml.contains(
            "<note>Video Creater NLE export limitation: reverse are not round-tripped into davinciFcpxml;"
        ),
        "{fcpxml}"
    );
    assert_eq!(
        fcpxml
            .matches("limitation: reverse are not round-tripped")
            .count(),
        2,
        "{fcpxml}"
    );

    let plain = xml_without_speed();
    assert!(!plain.contains("reverse"), "{plain}");
}
