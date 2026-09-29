use serde_json::json;
use std::collections::BTreeMap;
use video_creater_lib::project::fixtures::sample_project;
use video_creater_lib::project::model::{
    GeneratedAsset, GeneratedAssetReferences, GeneratedAssetSettings, GeneratedAssetStatus,
    GenerationModel, MediaAsset, MediaKind, ProjectTimeline, TimelineItem, TimelineItemKind,
    TimelineSource, TimelineTrack, TrackKind,
};
use video_creater_lib::project::nle_export::{
    export_project_timeline_to_nle_xml, nle_xml_export_artifact, write_nle_xml_export,
    NleXmlExportError, NleXmlFormat,
};

#[path = "project_nle_export/audio_edits.rs"]
mod audio_edits;
#[path = "project_nle_export/captions.rs"]
mod captions;
#[path = "project_nle_export/fade_to_color.rs"]
mod fade_to_color;
#[path = "project_nle_export/fcpxml_elements.rs"]
mod fcpxml_elements;
#[path = "project_nle_export/goldens.rs"]
mod goldens;
#[path = "project_nle_export/reversed_clips.rs"]
mod reversed_clips;
#[path = "project_nle_export/storylines.rs"]
mod storylines;
#[path = "project_nle_export/structure.rs"]
mod structure;
#[path = "project_nle_export/transitions.rs"]
mod transitions;
#[path = "project_nle_export/validation.rs"]
mod validation;
#[path = "project_nle_export/wipes.rs"]
mod wipes;

#[test]
fn exports_premiere_xmeml_with_timeline_and_source_ranges() {
    let mut project = sample_project();
    project.timeline.tracks[0].items[0]
        .properties
        .insert("sourceIn".to_string(), json!(2.5));
    project.timeline.tracks[0].items[0]
        .properties
        .insert("sourceOut".to_string(), json!(6.5));

    let export = export_project_timeline_to_nle_xml(&project, NleXmlFormat::PremiereXmeml)
        .expect("export xmeml");

    assert_eq!(export.filename, "project-test-premiere.xml");
    assert_eq!(export.mime_type, "application/xml");
    assert!(export
        .xml
        .starts_with("<?xml version=\"1.0\" encoding=\"UTF-8\"?>"));
    assert!(export.xml.contains("<xmeml version=\"5\">"));
    assert!(export.xml.contains("<name>Test Project</name>"));
    assert!(export.xml.contains("<timebase>24</timebase>"));
    assert!(export.xml.contains("<width>1920</width>"));
    assert!(export.xml.contains("<height>1080</height>"));
    assert!(export.xml.contains("<clipitem id=\"item-1\">"));
    assert!(export.xml.contains("<name>Opening clip</name>"));
    assert!(export.xml.contains("<start>0</start>"));
    assert!(export.xml.contains("<end>96</end>"));
    assert!(export.xml.contains("<in>60</in>"));
    assert!(export.xml.contains("<out>156</out>"));
    assert!(export
        .xml
        .contains("<pathurl>file://media/input.mp4</pathurl>"));
}

#[test]
fn recursively_expands_nested_timeline_clips_for_nle_export() {
    let mut project = sample_project();
    let mut alternate = project.timeline.clone();
    alternate.tracks[0].items[0]
        .properties
        .insert("sourceIn".to_string(), json!(0.0));
    alternate.tracks[0].items[0]
        .properties
        .insert("sourceOut".to_string(), json!(4.0));
    project.timelines.push(ProjectTimeline {
        id: "alternate".to_string(),
        name: "Alternate cut".to_string(),
        timeline: alternate,
    });
    let root_item = &mut project.timeline.tracks[0].items[0];
    root_item.start_seconds = 1.0;
    root_item.duration_seconds = 3.0;
    root_item.source = TimelineSource::Timeline {
        timeline_id: "alternate".to_string(),
    };
    root_item.properties = BTreeMap::from([("opacity".to_string(), json!(0.5))]);

    let export = export_project_timeline_to_nle_xml(&project, NleXmlFormat::DavinciFcpxml)
        .expect("nested sequence export");

    assert!(export.xml.contains(
        "<asset-clip name=\"Opening clip\" ref=\"media-1\" offset=\"24/24s\" duration=\"72/24s\" start=\"0/24s\">"
    ));
    assert!(!export.xml.contains("ref=\"alternate\""));
    assert!(export.xml.contains("<adjust-blend amount=\"0.5\"/>"));
}

#[test]
fn nested_timeline_wrapper_composes_opacity_keyframes_for_nle_export() {
    let mut project = sample_project();
    let mut alternate = project.timeline.clone();
    let child = &mut alternate.tracks[0].items[0];
    child.properties.insert("opacity".to_string(), json!(0.8));
    child.properties.insert(
        "keyframes".to_string(),
        json!({ "opacity": [
            { "atSeconds": 0.0, "value": 0.8 },
            { "atSeconds": 3.0, "value": 0.4 }
        ] }),
    );
    project.timelines.push(ProjectTimeline {
        id: "alternate".to_string(),
        name: "Alternate cut".to_string(),
        timeline: alternate,
    });
    let wrapper = &mut project.timeline.tracks[0].items[0];
    wrapper.duration_seconds = 3.0;
    wrapper.source = TimelineSource::Timeline {
        timeline_id: "alternate".to_string(),
    };
    wrapper.properties = BTreeMap::from([(
        "keyframes".to_string(),
        json!({ "opacity": [
            { "atSeconds": 0.0, "value": 0.5 },
            { "atSeconds": 3.0, "value": 0.25 }
        ] }),
    )]);

    let export = export_project_timeline_to_nle_xml(&project, NleXmlFormat::DavinciFcpxml)
        .expect("nested opacity keyframes should export");

    assert!(export.xml.contains("<adjust-blend amount=\"0.4\">"));
    assert!(export
        .xml
        .contains("<keyframe time=\"36/24s\" value=\"0.225\"/>"));
    assert!(export
        .xml
        .contains("<keyframe time=\"72/24s\" value=\"0.1\"/>"));
}

#[test]
fn nested_timeline_wrapper_composes_visual_fades_for_nle_export() {
    let mut project = sample_project();
    let mut alternate = project.timeline.clone();
    alternate.tracks[0].items[0]
        .properties
        .insert("opacity".to_string(), json!(0.8));
    project.timelines.push(ProjectTimeline {
        id: "alternate".to_string(),
        name: "Alternate cut".to_string(),
        timeline: alternate,
    });
    let wrapper = &mut project.timeline.tracks[0].items[0];
    wrapper.duration_seconds = 3.0;
    wrapper.source = TimelineSource::Timeline {
        timeline_id: "alternate".to_string(),
    };
    wrapper.properties = BTreeMap::from([
        ("fadeInSeconds".to_string(), json!(1.0)),
        ("fadeOutSeconds".to_string(), json!(1.0)),
    ]);

    let export = export_project_timeline_to_nle_xml(&project, NleXmlFormat::DavinciFcpxml)
        .expect("nested visual fades should export");

    assert!(export.xml.contains("<adjust-blend amount=\"0\">"));
    assert!(export
        .xml
        .contains("<keyframe time=\"12/24s\" value=\"0.4\"/>"));
    assert!(export
        .xml
        .contains("<keyframe time=\"24/24s\" value=\"0.8\"/>"));
    assert!(export
        .xml
        .contains("<keyframe time=\"72/24s\" value=\"0\"/>"));
}

#[test]
fn nested_timeline_wrapper_composes_audio_gain_for_nle_export() {
    let mut project = sample_project();
    project.media.push(MediaAsset {
        id: "voiceover".to_string(),
        name: Some("Voiceover".to_string()),
        relative_path: "media/voiceover.wav".to_string(),
        kind: MediaKind::Audio,
        duration_seconds: 4.0,
        width: None,
        height: None,
        fps: None,
        folder_id: None,
    });
    let mut alternate = project.timeline.clone();
    alternate.tracks.push(TimelineTrack {
        transitions: Vec::new(),
        id: "alternate-audio".to_string(),
        name: "Voiceover".to_string(),
        kind: TrackKind::Audio,
        locked: false,
        sync_locked: false,
        enabled: true,
        items: vec![TimelineItem {
            id: "alternate-audio-item".to_string(),
            kind: TimelineItemKind::AudioClip,
            start_seconds: 0.0,
            duration_seconds: 4.0,
            source: TimelineSource::Media {
                media_id: "voiceover".to_string(),
            },
            label: "Voiceover".to_string(),
            properties: BTreeMap::from([("volumeDb".to_string(), json!(-3.0))]),
        }],
    });
    project.timelines.push(ProjectTimeline {
        id: "alternate".to_string(),
        name: "Alternate cut".to_string(),
        timeline: alternate,
    });
    let wrapper = &mut project.timeline.tracks[0].items[0];
    wrapper.source = TimelineSource::Timeline {
        timeline_id: "alternate".to_string(),
    };
    wrapper.properties = BTreeMap::from([("volumeDb".to_string(), json!(-6.0))]);

    let export = export_project_timeline_to_nle_xml(&project, NleXmlFormat::DavinciFcpxml)
        .expect("nested audio gain should export");

    assert!(export.xml.contains("<adjust-volume amount=\"-9\"/>"));
}

#[test]
fn nested_timeline_wrapper_composes_static_canvas_transform_for_nle_export() {
    let mut project = sample_project();
    let mut alternate = project.timeline.clone();
    let child = &mut alternate.tracks[0].items[0];
    child.properties.insert("sourceIn".to_string(), json!(0.0));
    child.properties.insert("sourceOut".to_string(), json!(4.0));
    child.properties.insert(
        "transform".to_string(),
        json!({ "centerX": 0.2, "centerY": 0.5, "width": 0.5, "height": 1.0, "flipHorizontal": true }),
    );
    project.timelines.push(ProjectTimeline {
        id: "alternate".to_string(),
        name: "Alternate cut".to_string(),
        timeline: alternate,
    });
    let root_item = &mut project.timeline.tracks[0].items[0];
    root_item.source = TimelineSource::Timeline {
        timeline_id: "alternate".to_string(),
    };
    root_item.properties = BTreeMap::from([(
        "transform".to_string(),
        json!({ "centerX": 0.25, "centerY": 0.5, "width": 0.5, "height": 1.0, "flipHorizontal": true }),
    )]);

    let export = export_project_timeline_to_nle_xml(&project, NleXmlFormat::DavinciFcpxml)
        .expect("nested sequence transform export");

    assert!(export
        .xml
        .contains("<adjust-transform scale=\"0.25 1\" anchor=\"0 0\" position=\"-17.7778 0\"/>"));
}

#[test]
fn exports_davinci_fcpxml_with_project_media_assets() {
    let mut project = sample_project();
    project.timeline.tracks[0].items[0]
        .properties
        .insert("sourceIn".to_string(), json!(1.25));
    project.timeline.tracks[0].items[0]
        .properties
        .insert("sourceOut".to_string(), json!(5.25));

    let export = export_project_timeline_to_nle_xml(&project, NleXmlFormat::DavinciFcpxml)
        .expect("export fcpxml");

    assert_eq!(export.filename, "project-test-davinci.fcpxml");
    assert!(export.xml.contains("<fcpxml version=\"1.10\">"));
    assert!(export
        .xml
        .contains("<format id=\"r1\" frameDuration=\"1/24s\" width=\"1920\" height=\"1080\"/>"));
    assert!(export.xml.contains(
        "<asset id=\"media-1\" name=\"input.mp4\" start=\"0/1s\" duration=\"288/24s\" hasVideo=\"1\" format=\"r1\" hasAudio=\"1\" audioSources=\"1\" audioChannels=\"2\">"
    ));
    assert!(export
        .xml
        .contains("<media-rep kind=\"original-media\" src=\"file://media/input.mp4\"/>"));
    assert!(export.xml.contains("<project name=\"Test Project\">"));
    assert!(export
        .xml
        .contains("<sequence format=\"r1\" duration=\"96/24s\">"));
    assert!(export.xml.contains("<asset-clip name=\"Opening clip\" ref=\"media-1\" offset=\"0/24s\" duration=\"96/24s\" start=\"30/24s\"/>"));
    assert!(!export
        .xml
        .contains("<asset-clip name=\"Opening clip\" ref=\"media-1\" lane="));
}

#[test]
fn exports_davinci_fcpxml_with_retimed_clip_time_map() {
    let mut project = sample_project();
    let item = &mut project.timeline.tracks[0].items[0];
    item.duration_seconds = 2.0;
    item.properties.insert("speed".to_string(), json!(2.0));

    let export = export_project_timeline_to_nle_xml(&project, NleXmlFormat::DavinciFcpxml)
        .expect("export fcpxml");

    assert!(export.xml.contains(
        "<asset-clip name=\"Opening clip\" ref=\"media-1\" offset=\"0/24s\" duration=\"48/24s\" start=\"0s\">"
    ));
    assert!(export.xml.contains("<timeMap frameSampling=\"floor\">"));
    assert!(export
        .xml
        .contains("<timept time=\"0s\" value=\"0s\" interp=\"linear\"/>"));
    assert!(export
        .xml
        .contains("<timept time=\"6s\" value=\"12s\" interp=\"linear\"/>"));
}

#[test]
fn exports_davinci_fcpxml_with_disabled_track_state() {
    let mut project = sample_project();
    project.timeline.tracks[0].enabled = false;

    let export = export_project_timeline_to_nle_xml(&project, NleXmlFormat::DavinciFcpxml)
        .expect("export fcpxml");

    assert!(export.xml.contains(
        "<asset-clip name=\"Opening clip\" ref=\"media-1\" offset=\"0/24s\" duration=\"96/24s\" start=\"0/24s\" enabled=\"0\"/>"
    ));
}

#[test]
fn exports_davinci_fcpxml_with_video_track_lanes() {
    let mut project = sample_project();
    project.timeline.tracks.push(TimelineTrack::empty(
        "video-track-2",
        "Overlay video",
        TrackKind::Video,
    ));
    project
        .timeline
        .tracks
        .last_mut()
        .expect("second video track")
        .items
        .push(TimelineItem {
            id: "item-2".to_string(),
            kind: TimelineItemKind::VideoClip,
            start_seconds: 0.5,
            duration_seconds: 1.5,
            source: TimelineSource::Media {
                media_id: "media-1".to_string(),
            },
            label: "Overlay clip".to_string(),
            properties: BTreeMap::new(),
        });

    let export = export_project_timeline_to_nle_xml(&project, NleXmlFormat::DavinciFcpxml)
        .expect("export fcpxml");

    assert!(export.xml.contains(
        "<asset-clip name=\"Opening clip\" ref=\"media-1\" lane=\"1\" offset=\"0/24s\" duration=\"96/24s\" start=\"0/24s\"/>"
    ));
    assert!(export.xml.contains(
        "<asset-clip name=\"Overlay clip\" ref=\"media-1\" offset=\"12/24s\" duration=\"36/24s\" start=\"0/24s\"/>"
    ));
    assert!(!export
        .xml
        .contains("<asset-clip name=\"Overlay clip\" ref=\"media-1\" lane="));
}

#[test]
fn exports_premiere_xmeml_with_audio_track_items() {
    let project = sample_project_with_audio_clip();

    let export = export_project_timeline_to_nle_xml(&project, NleXmlFormat::PremiereXmeml)
        .expect("export xmeml");

    assert!(export.xml.contains("<audio>"));
    assert!(export.xml.contains("<clipitem id=\"audio-item-1\">"));
    assert!(export.xml.contains("<name>Voiceover bed</name>"));
    assert!(export.xml.contains("<start>24</start>"));
    assert!(export.xml.contains("<end>72</end>"));
    assert!(export.xml.contains("<in>12</in>"));
    assert!(export.xml.contains("<out>60</out>"));
    assert!(export
        .xml
        .contains("<pathurl>file://media/voiceover.wav</pathurl>"));
}

#[test]
fn exports_premiere_xmeml_with_static_audio_volume() {
    let mut project = sample_project_with_audio_clip();
    let audio_item = project
        .timeline
        .tracks
        .iter_mut()
        .find(|track| track.kind == TrackKind::Audio)
        .and_then(|track| track.items.first_mut())
        .expect("sample project has an audio clip");
    audio_item
        .properties
        .insert("volumeDb".to_string(), json!(-6.0));

    let export = export_project_timeline_to_nle_xml(&project, NleXmlFormat::PremiereXmeml)
        .expect("export xmeml");

    assert!(export.xml.contains("<clipitem id=\"audio-item-1\">"));
    assert!(export.xml.contains("<name>Audio Levels</name>"));
    assert!(export.xml.contains("<effectid>audiolevels</effectid>"));
    assert!(export.xml.contains("<parameterid>level</parameterid>"));
    assert!(export.xml.contains("<value>0.5012</value>"));
    assert!(!export.xml.contains("audio volume are not round-tripped"));
}

#[test]
fn exports_premiere_xmeml_with_audio_volume_keyframes() {
    let mut project = sample_project_with_audio_clip();
    let audio_item = project
        .timeline
        .tracks
        .iter_mut()
        .find(|track| track.kind == TrackKind::Audio)
        .and_then(|track| track.items.first_mut())
        .expect("sample project has an audio clip");
    audio_item.properties.insert(
        "keyframes".to_string(),
        json!({
            "volumeDb": [
                { "atSeconds": 0.0, "value": -6.0 },
                { "atSeconds": 1.5, "value": 0.0 }
            ]
        }),
    );

    let export = export_project_timeline_to_nle_xml(&project, NleXmlFormat::PremiereXmeml)
        .expect("export xmeml");

    assert!(export.xml.contains("<clipitem id=\"audio-item-1\">"));
    assert!(export.xml.contains("<name>Audio Levels</name>"));
    assert!(export.xml.contains("<effectid>audiolevels</effectid>"));
    assert!(export.xml.contains("<parameterid>level</parameterid>"));
    assert!(export.xml.contains("<value>0.5012</value>"));
    assert!(export.xml.contains("<when>0</when>"));
    assert!(export.xml.contains("<when>36</when>"));
    assert!(export.xml.contains("<value>1.0000</value>"));
    assert!(!export.xml.contains("keyframes are not round-tripped"));
}

#[test]
fn exports_davinci_fcpxml_with_audio_asset_clips() {
    let project = sample_project_with_audio_clip();

    let export = export_project_timeline_to_nle_xml(&project, NleXmlFormat::DavinciFcpxml)
        .expect("export fcpxml");

    assert!(export.xml.contains(
        "<asset id=\"audio-1\" name=\"voiceover.wav\" start=\"0/1s\" duration=\"144/24s\" hasAudio=\"1\" audioSources=\"1\" audioChannels=\"2\">"
    ));
    assert!(export
        .xml
        .contains("<media-rep kind=\"original-media\" src=\"file://media/voiceover.wav\"/>"));
    assert!(export.xml.contains(
        "<asset-clip name=\"Voiceover bed\" ref=\"audio-1\" lane=\"-1\" offset=\"24/24s\" duration=\"48/24s\" start=\"12/24s\" audioRole=\"dialogue\"/>"
    ));
}

#[test]
fn exports_davinci_fcpxml_with_static_audio_volume() {
    let mut project = sample_project_with_audio_clip();
    let audio_item = project
        .timeline
        .tracks
        .iter_mut()
        .find(|track| track.kind == TrackKind::Audio)
        .and_then(|track| track.items.first_mut())
        .expect("sample project has an audio clip");
    audio_item
        .properties
        .insert("volumeDb".to_string(), json!(-6.0));

    let export = export_project_timeline_to_nle_xml(&project, NleXmlFormat::DavinciFcpxml)
        .expect("export fcpxml");

    assert!(export.xml.contains(
        "<asset-clip name=\"Voiceover bed\" ref=\"audio-1\" lane=\"-1\" offset=\"24/24s\" duration=\"48/24s\" start=\"12/24s\" audioRole=\"dialogue\">"
    ));
    assert!(export.xml.contains("<adjust-volume amount=\"-6\"/>"));
    assert!(!export.xml.contains("audio volume are not round-tripped"));
}

#[test]
fn exports_premiere_xmeml_with_video_source_audio_volume() {
    let mut project = sample_project();
    project.timeline.tracks[0].items[0]
        .properties
        .insert("volumeDb".to_string(), json!(-3.0));

    let export = export_project_timeline_to_nle_xml(&project, NleXmlFormat::PremiereXmeml)
        .expect("export xmeml");

    assert!(export.xml.contains("<clipitem id=\"item-1-audio\">"));
    assert!(export
        .xml
        .contains("<masterclipid>masterclip-item-1</masterclipid>"));
    assert!(export.xml.contains("<linkclipref>item-1</linkclipref>"));
    assert!(export
        .xml
        .contains("<linkclipref>item-1-audio</linkclipref>"));
    assert!(export.xml.contains("<value>0.7079</value>"));
    assert!(!export.xml.contains("audio volume are not round-tripped"));
}

#[test]
fn exports_premiere_xmeml_with_video_source_audio_volume_keyframes() {
    let mut project = sample_project();
    project.timeline.tracks[0].items[0].properties.insert(
        "keyframes".to_string(),
        json!({
            "volumeDb": [
                { "atSeconds": 0.0, "value": -3.0 },
                { "atSeconds": 1.0, "value": -9.0 }
            ]
        }),
    );

    let export = export_project_timeline_to_nle_xml(&project, NleXmlFormat::PremiereXmeml)
        .expect("export xmeml");

    assert!(export.xml.contains("<clipitem id=\"item-1-audio\">"));
    assert!(export
        .xml
        .contains("<masterclipid>masterclip-item-1</masterclipid>"));
    assert!(export.xml.contains("<linkclipref>item-1</linkclipref>"));
    assert!(export
        .xml
        .contains("<linkclipref>item-1-audio</linkclipref>"));
    assert!(export.xml.contains("<value>0.7079</value>"));
    assert!(export.xml.contains("<when>24</when>"));
    assert!(export.xml.contains("<value>0.3548</value>"));
    assert!(!export.xml.contains("keyframes are not round-tripped"));
}

#[test]
fn exports_davinci_fcpxml_with_video_source_audio_volume() {
    let mut project = sample_project();
    project.timeline.tracks[0].items[0]
        .properties
        .insert("volumeDb".to_string(), json!(-3.0));

    let export = export_project_timeline_to_nle_xml(&project, NleXmlFormat::DavinciFcpxml)
        .expect("export fcpxml");

    assert!(export
        .xml
        .contains("<asset-clip name=\"Opening clip\" ref=\"media-1\""));
    assert!(export.xml.contains("<adjust-volume amount=\"-3\"/>"));
    assert!(!export.xml.contains("audio volume are not round-tripped"));
}

#[test]
fn exports_premiere_xmeml_with_generated_clip_provenance() {
    let project = sample_project_with_generated_clip();

    let export = export_project_timeline_to_nle_xml(&project, NleXmlFormat::PremiereXmeml)
        .expect("export xmeml");

    assert!(export.xml.contains("<clipitem id=\"generated-item-1\">"));
    assert!(export
        .xml
        .contains("<description>Generated asset: Product hero; Model: fal.ai/fal-ai/wan-25-preview/text-to-video; Prompt: Product on a clean studio turntable; References: media-1</description>"));
}

#[test]
fn exports_davinci_fcpxml_with_generated_clip_provenance_note() {
    let project = sample_project_with_generated_clip();

    let export = export_project_timeline_to_nle_xml(&project, NleXmlFormat::DavinciFcpxml)
        .expect("export fcpxml");

    assert!(export.xml.contains(
        "<asset-clip name=\"Generated hero\" ref=\"generated-output-1\" offset=\"96/24s\" duration=\"72/24s\" start=\"0/24s\">"
    ));
    assert!(export
        .xml
        .contains("<note>Generated asset: Product hero; Model: fal.ai/fal-ai/wan-25-preview/text-to-video; Prompt: Product on a clean studio turntable; References: media-1</note>"));
}

#[test]
fn exports_premiere_xmeml_with_image_clip_source_ranges() {
    let project = sample_project_with_image_clip();

    let export = export_project_timeline_to_nle_xml(&project, NleXmlFormat::PremiereXmeml)
        .expect("export xmeml");

    assert!(export.xml.contains("<clipitem id=\"image-item-1\">"));
    assert!(export.xml.contains("<name>Product still</name>"));
    assert!(export.xml.contains("<start>48</start>"));
    assert!(export.xml.contains("<end>96</end>"));
    assert!(export.xml.contains("<in>6</in>"));
    assert!(export.xml.contains("<out>54</out>"));
    assert!(export
        .xml
        .contains("<pathurl>file://media/product-still.png</pathurl>"));
}

#[test]
fn exports_davinci_fcpxml_with_image_clip_asset() {
    let project = sample_project_with_image_clip();

    let export = export_project_timeline_to_nle_xml(&project, NleXmlFormat::DavinciFcpxml)
        .expect("export fcpxml");

    assert!(export.xml.contains(
        "<asset id=\"still-1\" name=\"product-still.png\" start=\"0/1s\" duration=\"72/24s\" hasVideo=\"1\" format=\"r1\">"
    ));
    assert!(export
        .xml
        .contains("<media-rep kind=\"original-media\" src=\"file://media/product-still.png\"/>"));
    assert!(export.xml.contains(
        "<asset-clip name=\"Product still\" ref=\"still-1\" offset=\"48/24s\" duration=\"48/24s\" start=\"6/24s\"/>"
    ));
}

#[test]
fn exports_premiere_xmeml_with_static_opacity_filter() {
    let mut project = sample_project();
    project.timeline.tracks[0].items[0]
        .properties
        .insert("opacity".to_string(), json!(0.72));

    let export = export_project_timeline_to_nle_xml(&project, NleXmlFormat::PremiereXmeml)
        .expect("export xmeml");

    assert!(export.xml.contains("<clipitem id=\"item-1\">"));
    assert!(export.xml.contains("<name>Opacity</name>"));
    assert!(export.xml.contains("<effectid>opacity</effectid>"));
    assert!(export.xml.contains("<parameterid>opacity</parameterid>"));
    assert!(export.xml.contains("<value>72.0</value>"));
    assert!(!export.xml.contains("opacity are not round-tripped"));
}

#[test]
fn exports_premiere_xmeml_with_opacity_keyframes() {
    let mut project = sample_project();
    project.timeline.tracks[0].items[0].properties.insert(
        "keyframes".to_string(),
        json!({
            "opacity": [
                { "atSeconds": 0.0, "value": 0.25 },
                { "atSeconds": 1.5, "value": 0.75 }
            ]
        }),
    );

    let export = export_project_timeline_to_nle_xml(&project, NleXmlFormat::PremiereXmeml)
        .expect("export xmeml");

    assert!(export.xml.contains("<name>Opacity</name>"));
    assert!(export.xml.contains("<effectid>opacity</effectid>"));
    assert!(export.xml.contains("<parameterid>opacity</parameterid>"));
    assert!(export.xml.contains("<value>25.0</value>"));
    assert!(export.xml.contains("<when>0</when>"));
    assert!(export.xml.contains("<when>36</when>"));
    assert!(export.xml.contains("<value>75.0</value>"));
    assert!(!export.xml.contains("keyframes are not round-tripped"));
}

#[test]
fn exports_premiere_xmeml_with_static_transform_filter() {
    let mut project = sample_project();
    project.timeline.tracks[0].items[0].properties.insert(
        "transform".to_string(),
        json!({
            "centerX": 0.25,
            "centerY": 0.75,
            "width": 0.5,
            "height": 0.5
        }),
    );

    let export = export_project_timeline_to_nle_xml(&project, NleXmlFormat::PremiereXmeml)
        .expect("export xmeml");

    assert!(export.xml.contains("<name>Basic Motion</name>"));
    assert!(export.xml.contains("<effectid>basic</effectid>"));
    assert!(export.xml.contains("<parameterid>scale</parameterid>"));
    assert!(export.xml.contains("<value>50.00</value>"));
    assert!(export.xml.contains("<parameterid>center</parameterid>"));
    assert!(export.xml.contains("<horiz>-0.25000</horiz>"));
    assert!(export.xml.contains("<vert>0.25000</vert>"));
    assert!(!export.xml.contains("transform are not round-tripped"));
}

#[test]
fn exports_premiere_xmeml_with_scale_keyframes() {
    let mut project = sample_project();
    project.timeline.tracks[0].items[0].properties.insert(
        "keyframes".to_string(),
        json!({
            "scale": [
                { "atSeconds": 0.0, "value": 0.75 },
                { "atSeconds": 1.5, "value": 1.25 }
            ]
        }),
    );

    let export = export_project_timeline_to_nle_xml(&project, NleXmlFormat::PremiereXmeml)
        .expect("export xmeml");

    assert!(export.xml.contains("<name>Basic Motion</name>"));
    assert!(export.xml.contains("<effectid>basic</effectid>"));
    assert!(export.xml.contains("<parameterid>scale</parameterid>"));
    assert!(export.xml.contains("<value>75.00</value>"));
    assert!(export.xml.contains("<when>0</when>"));
    assert!(export.xml.contains("<when>36</when>"));
    assert!(export.xml.contains("<value>125.00</value>"));
    assert!(!export.xml.contains("keyframes are not round-tripped"));
}

#[test]
fn exports_premiere_xmeml_with_center_keyframes() {
    let mut project = sample_project();
    project.timeline.tracks[0].items[0].properties.insert(
        "keyframes".to_string(),
        json!({
            "positionX": [
                { "atSeconds": 0.0, "value": 0.25 },
                { "atSeconds": 1.5, "value": 0.75 }
            ],
            "positionY": [
                { "atSeconds": 0.0, "value": 0.35 },
                { "atSeconds": 1.5, "value": 0.65 }
            ]
        }),
    );

    let export = export_project_timeline_to_nle_xml(&project, NleXmlFormat::PremiereXmeml)
        .expect("export xmeml");

    assert!(export.xml.contains("<name>Basic Motion</name>"));
    assert!(export.xml.contains("<effectid>basic</effectid>"));
    assert!(export.xml.contains("<parameterid>center</parameterid>"));
    assert!(export.xml.contains("<horiz>-0.25000</horiz>"));
    assert!(export.xml.contains("<vert>-0.15000</vert>"));
    assert!(export.xml.contains("<when>0</when>"));
    assert!(export.xml.contains("<when>36</when>"));
    assert!(export.xml.contains("<horiz>0.25000</horiz>"));
    assert!(export.xml.contains("<vert>0.15000</vert>"));
    assert!(!export.xml.contains("keyframes are not round-tripped"));
}

#[test]
fn exports_premiere_xmeml_with_static_rotation_filter() {
    let mut project = sample_project();
    project.timeline.tracks[0].items[0].properties.insert(
        "transform".to_string(),
        json!({
            "rotation": 15.0
        }),
    );

    let export = export_project_timeline_to_nle_xml(&project, NleXmlFormat::PremiereXmeml)
        .expect("export xmeml");

    assert!(export.xml.contains("<name>Basic Motion</name>"));
    assert!(export.xml.contains("<effectid>basic</effectid>"));
    assert!(export.xml.contains("<parameterid>rotation</parameterid>"));
    assert!(export.xml.contains("<name>Rotation</name>"));
    assert!(export.xml.contains("<valuemin>-100000</valuemin>"));
    assert!(export.xml.contains("<valuemax>100000</valuemax>"));
    assert!(export.xml.contains("<value>-15.00</value>"));
    assert!(!export.xml.contains("rotation are not round-tripped"));
}

#[test]
fn exports_premiere_xmeml_with_rotation_keyframes() {
    let mut project = sample_project();
    project.timeline.tracks[0].items[0].properties.insert(
        "keyframes".to_string(),
        json!({
            "rotationDegrees": [
                { "atSeconds": 0.0, "value": 15.0 },
                { "atSeconds": 1.5, "value": -30.0 }
            ]
        }),
    );

    let export = export_project_timeline_to_nle_xml(&project, NleXmlFormat::PremiereXmeml)
        .expect("export xmeml");

    assert!(export.xml.contains("<name>Basic Motion</name>"));
    assert!(export.xml.contains("<effectid>basic</effectid>"));
    assert!(export.xml.contains("<parameterid>rotation</parameterid>"));
    assert!(export.xml.contains("<value>-15.00</value>"));
    assert!(export.xml.contains("<when>0</when>"));
    assert!(export.xml.contains("<when>36</when>"));
    assert!(export.xml.contains("<value>30.00</value>"));
    assert!(!export.xml.contains("keyframes are not round-tripped"));
}

#[test]
fn exports_premiere_xmeml_with_static_crop_filter() {
    let mut project = sample_project();
    project.timeline.tracks[0].items[0].properties.insert(
        "keyframes".to_string(),
        json!({
            "cropTop": [{ "atSeconds": 0.0, "value": 0.2, "easing": "hold" }],
            "cropRight": [{ "atSeconds": 0.0, "value": 0.3, "easing": "hold" }],
            "cropBottom": [{ "atSeconds": 0.0, "value": 0.4, "easing": "hold" }],
            "cropLeft": [{ "atSeconds": 0.0, "value": 0.1, "easing": "hold" }]
        }),
    );

    let export = export_project_timeline_to_nle_xml(&project, NleXmlFormat::PremiereXmeml)
        .expect("export xmeml");

    assert!(export.xml.contains("<name>Crop</name>"));
    assert!(export.xml.contains("<effectid>crop</effectid>"));
    assert!(export.xml.contains("<parameterid>left</parameterid>"));
    assert!(export.xml.contains("<value>10.00</value>"));
    assert!(export.xml.contains("<parameterid>right</parameterid>"));
    assert!(export.xml.contains("<value>30.00</value>"));
    assert!(export.xml.contains("<parameterid>top</parameterid>"));
    assert!(export.xml.contains("<value>20.00</value>"));
    assert!(export.xml.contains("<parameterid>bottom</parameterid>"));
    assert!(export.xml.contains("<value>40.00</value>"));
    assert!(!export.xml.contains("keyframes are not round-tripped"));
}

#[test]
fn exports_premiere_xmeml_with_crop_keyframes() {
    let mut project = sample_project();
    project.timeline.tracks[0].items[0].properties.insert(
        "keyframes".to_string(),
        json!({
            "cropTop": [
                { "atSeconds": 0.0, "value": 0.1 },
                { "atSeconds": 1.5, "value": 0.2 }
            ],
            "cropRight": [
                { "atSeconds": 0.0, "value": 0.15 },
                { "atSeconds": 1.5, "value": 0.25 }
            ],
            "cropBottom": [
                { "atSeconds": 0.0, "value": 0.05 },
                { "atSeconds": 1.5, "value": 0.15 }
            ],
            "cropLeft": [
                { "atSeconds": 0.0, "value": 0.2 },
                { "atSeconds": 1.5, "value": 0.3 }
            ]
        }),
    );

    let export = export_project_timeline_to_nle_xml(&project, NleXmlFormat::PremiereXmeml)
        .expect("export xmeml");

    assert!(export.xml.contains("<name>Crop</name>"));
    assert!(export.xml.contains("<effectid>crop</effectid>"));
    assert!(export.xml.contains("<parameterid>left</parameterid>"));
    assert!(export.xml.contains("<value>20.00</value>"));
    assert!(export.xml.contains("<value>30.00</value>"));
    assert!(export.xml.contains("<parameterid>right</parameterid>"));
    assert!(export.xml.contains("<value>15.00</value>"));
    assert!(export.xml.contains("<value>25.00</value>"));
    assert!(export.xml.contains("<parameterid>top</parameterid>"));
    assert!(export.xml.contains("<value>10.00</value>"));
    assert!(export.xml.contains("<parameterid>bottom</parameterid>"));
    assert!(export.xml.contains("<value>5.00</value>"));
    assert!(export.xml.contains("<when>36</when>"));
    assert!(!export.xml.contains("keyframes are not round-tripped"));
}

#[test]
fn exports_premiere_xmeml_documents_unsupported_effect_and_keyframe_limitations() {
    let project = sample_project_with_effect_keyframes();

    let export = export_project_timeline_to_nle_xml(&project, NleXmlFormat::PremiereXmeml)
        .expect("export xmeml");

    assert!(export.xml.contains("<clipitem id=\"item-1\">"));
    assert!(export.xml.contains("<clipitem id=\"item-1-audio\">"));
    assert!(export.xml.contains("<value>0.5012</value>"));
    assert!(export.xml.contains("<effectid>opacity</effectid>"));
    assert!(export.xml.contains("<value>30.0</value>"));
    assert!(export.xml.contains("<when>24</when>"));
    assert!(export.xml.contains("<value>100.0</value>"));
    assert!(export.xml.contains(
        "<description>Video Creater NLE export limitation: effect stack and color grade are not round-tripped into premiereXmeml; render a final video for baked visual/audio look.</description>"
    ));
}

#[test]
fn exports_davinci_fcpxml_documents_unsupported_effect_and_keyframe_limitations() {
    let project = sample_project_with_effect_keyframes();

    let export = export_project_timeline_to_nle_xml(&project, NleXmlFormat::DavinciFcpxml)
        .expect("export fcpxml");

    assert!(export
        .xml
        .contains("<asset-clip name=\"Opening clip\" ref=\"media-1\""));
    assert!(export.xml.contains("<adjust-blend amount=\"0.3\">"));
    assert!(export.xml.contains("<adjust-volume amount=\"-6\"/>"));
    assert!(export.xml.contains(
        "<note>Video Creater NLE export limitation: effect stack and color grade are not round-tripped into davinciFcpxml; render a final video for baked visual/audio look.</note>"
    ));
}

#[test]
fn exports_premiere_xmeml_documents_every_unsupported_clip_finishing_property() {
    let mut project = sample_project();
    let item = &mut project.timeline.tracks[0].items[0];
    item.properties
        .insert("fadeInSeconds".to_string(), json!(0.25));
    item.properties
        .insert("fadeOutSeconds".to_string(), json!(0.5));
    item.properties
        .insert("blendMode".to_string(), json!("multiply"));
    item.properties.insert("cropTop".to_string(), json!(0.1));
    item.properties
        .insert("transform".to_string(), json!({ "flipHorizontal": true }));
    item.properties.insert(
        "keyframes".to_string(),
        json!({
            "opacity": [
                { "atSeconds": 0.0, "value": 0.25, "easing": "easeInOut" },
                { "atSeconds": 1.0, "value": 1.0, "easing": "linear" }
            ]
        }),
    );

    let export = export_project_timeline_to_nle_xml(&project, NleXmlFormat::PremiereXmeml)
        .expect("export xmeml");

    assert!(export.xml.contains(
        "<description>Video Creater NLE export limitation: keyframe easing, clip fades, blend mode, static crop, and flip transform are not round-tripped into premiereXmeml; render a final video for baked visual/audio look.</description>"
    ));
}

#[test]
fn exports_davinci_fcpxml_documents_every_unsupported_clip_finishing_property() {
    let mut project = sample_project();
    let item = &mut project.timeline.tracks[0].items[0];
    item.properties
        .insert("fadeInSeconds".to_string(), json!(0.25));
    item.properties
        .insert("fadeOutSeconds".to_string(), json!(0.5));
    item.properties
        .insert("blendMode".to_string(), json!("multiply"));
    item.properties.insert("cropTop".to_string(), json!(0.1));
    item.properties
        .insert("transform".to_string(), json!({ "flipHorizontal": true }));
    item.properties.insert(
        "keyframes".to_string(),
        json!({
            "opacity": [
                { "atSeconds": 0.0, "value": 0.25, "easing": "easeInOut" },
                { "atSeconds": 1.0, "value": 1.0, "easing": "linear" }
            ]
        }),
    );

    let export = export_project_timeline_to_nle_xml(&project, NleXmlFormat::DavinciFcpxml)
        .expect("export fcpxml");

    assert!(export.xml.contains(
        "<note>Video Creater NLE export limitation: keyframe easing, clip fades, blend mode, and static crop are not round-tripped into davinciFcpxml; render a final video for baked visual/audio look.</note>"
    ));
    assert!(!export.xml.contains("flip transform"));
}

#[test]
fn exports_davinci_fcpxml_with_static_transform_and_opacity() {
    let mut project = sample_project();
    let item = &mut project.timeline.tracks[0].items[0];
    item.properties.insert(
        "transform".to_string(),
        json!({
            "centerX": 0.25,
            "centerY": 0.75,
            "width": 0.5,
            "height": 0.5,
            "flipHorizontal": true
        }),
    );
    item.properties.insert("opacity".to_string(), json!(0.72));

    let export = export_project_timeline_to_nle_xml(&project, NleXmlFormat::DavinciFcpxml)
        .expect("export fcpxml");

    assert!(export
        .xml
        .contains("<asset-clip name=\"Opening clip\" ref=\"media-1\""));
    assert!(export.xml.contains(
        "<adjust-transform scale=\"-0.5 0.5\" anchor=\"0 0\" position=\"-44.4444 -25\"/>"
    ));
    assert!(export.xml.contains("<adjust-blend amount=\"0.72\"/>"));
    assert!(!export.xml.contains("opacity are not round-tripped"));
}

#[test]
fn exports_davinci_fcpxml_with_opacity_keyframes() {
    let mut project = sample_project();
    project.timeline.tracks[0].items[0].properties.insert(
        "keyframes".to_string(),
        json!({
            "opacity": [
                { "atSeconds": 0.0, "value": 0.25 },
                { "atSeconds": 1.5, "value": 0.75 }
            ]
        }),
    );

    let export = export_project_timeline_to_nle_xml(&project, NleXmlFormat::DavinciFcpxml)
        .expect("export fcpxml");

    assert!(export.xml.contains("<adjust-blend amount=\"0.25\">"));
    assert!(export
        .xml
        .contains("<param name=\"amount\" value=\"0.25\">"));
    assert!(export.xml.contains("<keyframeAnimation>"));
    assert!(export
        .xml
        .contains("<keyframe time=\"0/24s\" value=\"0.25\"/>"));
    assert!(export
        .xml
        .contains("<keyframe time=\"36/24s\" value=\"0.75\"/>"));
    assert!(!export.xml.contains("keyframes are not round-tripped"));
}

#[test]
fn exports_davinci_fcpxml_with_static_rotation_transform() {
    let mut project = sample_project();
    project.timeline.tracks[0].items[0].properties.insert(
        "transform".to_string(),
        json!({
            "rotation": 15.0
        }),
    );

    let export = export_project_timeline_to_nle_xml(&project, NleXmlFormat::DavinciFcpxml)
        .expect("export fcpxml");

    assert!(export.xml.contains(
        "<adjust-transform scale=\"1 1\" rotation=\"-15\" anchor=\"0 0\" position=\"0 0\"/>"
    ));
    assert!(!export.xml.contains("rotation are not round-tripped"));
}

#[test]
fn exports_davinci_fcpxml_with_transform_keyframes() {
    let mut project = sample_project();
    project.timeline.tracks[0].items[0].properties.insert(
        "keyframes".to_string(),
        json!({
            "scale": [
                { "atSeconds": 0.0, "value": 0.5 },
                { "atSeconds": 1.5, "value": 1.25 }
            ],
            "positionX": [
                { "atSeconds": 0.0, "value": 0.25 },
                { "atSeconds": 1.5, "value": 0.75 }
            ],
            "positionY": [
                { "atSeconds": 0.0, "value": 0.35 },
                { "atSeconds": 1.5, "value": 0.65 }
            ],
            "rotationDegrees": [
                { "atSeconds": 0.0, "value": 15.0 },
                { "atSeconds": 1.5, "value": -30.0 }
            ]
        }),
    );

    let export = export_project_timeline_to_nle_xml(&project, NleXmlFormat::DavinciFcpxml)
        .expect("export fcpxml");

    assert!(export.xml.contains(
        "<adjust-transform scale=\"0.5 0.5\" rotation=\"-15\" anchor=\"0 0\" position=\"-44.4444 15\">"
    ));
    assert!(export
        .xml
        .contains("<param name=\"scale\" value=\"0.5 0.5\">"));
    assert!(export
        .xml
        .contains("<keyframe time=\"36/24s\" value=\"1.25 1.25\"/>"));
    assert!(export
        .xml
        .contains("<param name=\"position\" value=\"-44.4444 15\">"));
    assert!(export
        .xml
        .contains("<keyframe time=\"36/24s\" value=\"44.4444 -15\"/>"));
    assert!(export
        .xml
        .contains("<param name=\"rotation\" value=\"-15\">"));
    assert!(export
        .xml
        .contains("<keyframe time=\"36/24s\" value=\"30\"/>"));
    assert!(!export.xml.contains("keyframes are not round-tripped"));
}

#[test]
fn exports_davinci_fcpxml_with_static_crop_keyframes_as_trim_rect() {
    let mut project = sample_project();
    project.timeline.tracks[0].items[0].properties.insert(
        "keyframes".to_string(),
        json!({
            "cropTop": [{ "atSeconds": 0.0, "value": 0.2, "easing": "hold" }],
            "cropRight": [{ "atSeconds": 0.0, "value": 0.3, "easing": "hold" }],
            "cropBottom": [{ "atSeconds": 0.0, "value": 0.4, "easing": "hold" }],
            "cropLeft": [{ "atSeconds": 0.0, "value": 0.1, "easing": "hold" }]
        }),
    );

    let export = export_project_timeline_to_nle_xml(&project, NleXmlFormat::DavinciFcpxml)
        .expect("export fcpxml");

    assert!(export.xml.contains("<adjust-crop mode=\"trim\">"));
    assert!(export
        .xml
        .contains("<trim-rect top=\"20\" right=\"53.3333\" bottom=\"40\" left=\"17.7778\"/>"));
    assert!(!export.xml.contains("keyframes are not round-tripped"));
}

#[test]
fn exports_davinci_fcpxml_limitation_note_omits_static_opacity() {
    let mut project = sample_project_with_effect_keyframes();
    project.timeline.tracks[0].items[0]
        .properties
        .remove("keyframes");

    let export = export_project_timeline_to_nle_xml(&project, NleXmlFormat::DavinciFcpxml)
        .expect("export fcpxml");

    assert!(export.xml.contains("<adjust-blend amount=\"0.72\"/>"));
    assert!(export.xml.contains("<adjust-volume amount=\"-6\"/>"));
    assert!(export.xml.contains(
        "<note>Video Creater NLE export limitation: effect stack and color grade are not round-tripped into davinciFcpxml; render a final video for baked visual/audio look.</note>"
    ));
}

#[test]
fn exports_premiere_xmeml_with_caption_items() {
    let project = sample_project_with_caption();

    let export = export_project_timeline_to_nle_xml(&project, NleXmlFormat::PremiereXmeml)
        .expect("export xmeml");

    assert!(export.xml.contains("<generatoritem id=\"caption-item-1\">"));
    assert!(export.xml.contains("<name>Hook caption</name>"));
    assert!(export.xml.contains("<start>12</start>"));
    assert!(export.xml.contains("<end>48</end>"));
    assert!(export
        .xml
        .contains("<value>Launch &amp; grow &lt;fast&gt;</value>"));
}

#[test]
fn exports_davinci_fcpxml_with_caption_titles() {
    let project = sample_project_with_caption();

    let export = export_project_timeline_to_nle_xml(&project, NleXmlFormat::DavinciFcpxml)
        .expect("export fcpxml");

    // A connected title anchored in the clip covering its start, in that clip's local time.
    assert!(export.xml.contains(
        "<asset-clip name=\"Opening clip\" ref=\"media-1\" offset=\"0/24s\" duration=\"96/24s\" start=\"0/24s\">\n              <title name=\"Hook caption\" ref=\"vc-title-basic\" lane=\"2\" offset=\"12/24s\" duration=\"36/24s\">"
    ));
    assert!(export.xml.contains(
        "<text-style ref=\"caption-item-1-style\">Launch &amp; grow &lt;fast&gt;</text-style>"
    ));
}

#[test]
fn exports_premiere_xmeml_with_text_overlay_items() {
    let project = sample_project_with_text_overlay();

    let export = export_project_timeline_to_nle_xml(&project, NleXmlFormat::PremiereXmeml)
        .expect("export xmeml");

    assert!(export.xml.contains("<generatoritem id=\"overlay-item-1\">"));
    assert!(export.xml.contains("<name>Manual callout</name>"));
    assert!(export.xml.contains("<start>72</start>"));
    assert!(export.xml.contains("<end>120</end>"));
    assert!(export.xml.contains("<value>Look here</value>"));
}

#[test]
fn exports_davinci_fcpxml_with_text_overlay_titles_above_video() {
    let mut project = sample_project_with_text_overlay();
    let overlay = project
        .timeline
        .tracks
        .iter_mut()
        .find(|track| track.kind == TrackKind::Overlay)
        .and_then(|track| track.items.first_mut())
        .expect("sample project has overlay");
    overlay
        .properties
        .insert("fontName".to_string(), json!("Helvetica-Bold"));
    overlay
        .properties
        .insert("fontSize".to_string(), json!(72.0));
    overlay
        .properties
        .insert("color".to_string(), json!("#3366CC"));
    overlay
        .properties
        .insert("alignment".to_string(), json!("right"));

    let export = export_project_timeline_to_nle_xml(&project, NleXmlFormat::DavinciFcpxml)
        .expect("export fcpxml");

    assert!(export.xml.contains(
        "<title name=\"Manual callout\" ref=\"vc-title-basic\" lane=\"1\" offset=\"72/24s\" duration=\"48/24s\">"
    ));
    assert!(export
        .xml
        .contains("<text-style ref=\"overlay-item-1-style\">Look here</text-style>"));
    assert!(export
        .xml
        .contains("<text-style-def id=\"overlay-item-1-style\">"));
    assert!(export.xml.contains(
        "<text-style font=\"Helvetica\" fontFace=\"Bold\" fontSize=\"72\" fontColor=\"0.2 0.4 0.8 1\" alignment=\"right\"/>"
    ));
}

#[test]
fn exports_premiere_xmeml_without_lottie_clips() {
    let project = sample_project_with_lottie_clip();

    let export = export_project_timeline_to_nle_xml(&project, NleXmlFormat::PremiereXmeml)
        .expect("export xmeml");

    assert!(!export.xml.contains("lottie-item-1"));
    assert!(!export.xml.contains("brand-burst.json"));
    assert!(export.xml.contains("<clipitem id=\"item-1\">"));
}

#[test]
fn exports_davinci_fcpxml_without_lottie_assets_or_clips() {
    let project = sample_project_with_lottie_clip();

    let export = export_project_timeline_to_nle_xml(&project, NleXmlFormat::DavinciFcpxml)
        .expect("export fcpxml");

    assert!(!export.xml.contains("lottie-1"));
    assert!(!export.xml.contains("brand-burst.json"));
    assert!(!export.xml.contains("Lottie burst"));
    assert!(export.xml.contains("ref=\"media-1\""));
}

#[test]
fn nle_export_rejects_timeline_items_with_missing_media() {
    let mut project = sample_project();
    project.timeline.tracks[0].items[0].source = TimelineSource::Media {
        media_id: "missing-media".to_string(),
    };

    let error = export_project_timeline_to_nle_xml(&project, NleXmlFormat::PremiereXmeml)
        .expect_err("missing media should reject export");

    assert_eq!(
        error,
        NleXmlExportError::MissingMedia {
            item_id: "item-1".to_string(),
            media_id: "missing-media".to_string()
        }
    );
}

#[test]
fn nle_export_rejects_source_ranges_outside_media_duration() {
    let mut project = sample_project();
    project.timeline.tracks[0].items[0]
        .properties
        .insert("sourceIn".to_string(), json!(10.0));
    project.timeline.tracks[0].items[0]
        .properties
        .insert("sourceOut".to_string(), json!(14.0));

    let error = export_project_timeline_to_nle_xml(&project, NleXmlFormat::DavinciFcpxml)
        .expect_err("out-of-range source should reject export");

    assert_eq!(
        error,
        NleXmlExportError::InvalidSourceRange {
            item_id: "item-1".to_string()
        }
    );
}

#[test]
fn writes_nle_xml_export_under_project_exports_folder() {
    let dir = tempfile::tempdir().expect("project dir");
    let project = sample_project();
    let export = export_project_timeline_to_nle_xml(&project, NleXmlFormat::PremiereXmeml)
        .expect("export xmeml");

    let export_path = write_nle_xml_export(dir.path(), &export).expect("write nle xml export");

    assert_eq!(
        export_path,
        dir.path().join("exports/project-test-premiere.xml")
    );
    assert!(export_path.exists());
    assert_eq!(
        std::fs::read_to_string(export_path).expect("export xml"),
        export.xml
    );
}

#[test]
fn nle_xml_export_artifact_records_relative_project_path() {
    let project = sample_project();
    let export = export_project_timeline_to_nle_xml(&project, NleXmlFormat::PremiereXmeml)
        .expect("export xmeml");

    let artifact = nle_xml_export_artifact(
        &export,
        NleXmlFormat::PremiereXmeml,
        "nle-export-premiere-1",
        "2026-06-23T12:00:00Z",
    );

    assert_eq!(artifact.id, "nle-export-premiere-1");
    assert_eq!(artifact.format, "premiereXmeml");
    assert_eq!(artifact.path, "exports/project-test-premiere.xml");
    assert_eq!(artifact.mime_type, "application/xml");
    assert_eq!(artifact.job_id.as_deref(), Some("nle-export-premiere-1"));
}

fn sample_project_with_audio_clip() -> video_creater_lib::project::model::VideoProject {
    let mut project = sample_project();
    project.media.push(MediaAsset {
        id: "audio-1".to_string(),
        name: None,
        relative_path: "media/voiceover.wav".to_string(),
        kind: MediaKind::Audio,
        duration_seconds: 6.0,
        width: None,
        height: None,
        fps: None,
        folder_id: None,
    });

    let audio_track = project
        .timeline
        .tracks
        .iter_mut()
        .find(|track| track.kind == TrackKind::Audio)
        .expect("sample project has audio track");
    audio_track.items.push(TimelineItem {
        id: "audio-item-1".to_string(),
        kind: TimelineItemKind::AudioClip,
        start_seconds: 1.0,
        duration_seconds: 2.0,
        source: TimelineSource::Media {
            media_id: "audio-1".to_string(),
        },
        label: "Voiceover bed".to_string(),
        properties: BTreeMap::from([
            ("sourceIn".to_string(), json!(0.5)),
            ("sourceOut".to_string(), json!(2.5)),
        ]),
    });

    project
}

fn sample_project_with_generated_clip() -> video_creater_lib::project::model::VideoProject {
    let mut project = sample_project();
    project.timeline.duration_seconds = 7.0;
    project.media.push(MediaAsset {
        id: "generated-output-1".to_string(),
        name: None,
        relative_path: "generated/generated-shot-1/output.mp4".to_string(),
        kind: MediaKind::Generated,
        duration_seconds: 3.0,
        width: Some(1280),
        height: Some(720),
        fps: Some(24.0),
        folder_id: None,
    });
    project.generated_assets.push(GeneratedAsset {
        schema_version: 1,
        id: "generated-shot-1".to_string(),
        kind: MediaKind::Generated,
        status: GeneratedAssetStatus::Completed,
        name: Some("Product hero".to_string()),
        target_folder_id: None,
        placement_intent: Some("timeline".to_string()),
        prompt: "Product on a clean studio turntable".to_string(),
        model: GenerationModel {
            provider: "fal.ai".to_string(),
            id: "fal-ai/wan-25-preview/text-to-video".to_string(),
        },
        references: GeneratedAssetReferences {
            media_ids: vec!["media-1".to_string()],
            first_frame_media_id: None,
            last_frame_media_id: None,
            provider_input_urls: Vec::new(),
            ..Default::default()
        },
        settings: GeneratedAssetSettings::default(),
        outputs: vec![video_creater_lib::project::model::GeneratedAssetOutput {
            media_id: "generated-output-1".to_string(),
            relative_path: "generated/generated-shot-1/output.mp4".to_string(),
            source_url: None,
            width: 1280,
            height: 720,
            duration_seconds: 3.0,
            fps: 24.0,
        }],
        created_at: "2026-06-24T10:00:00Z".to_string(),
        parent_asset_id: None,
        retry_of_asset_id: None,
    });

    let video_track = project
        .timeline
        .tracks
        .iter_mut()
        .find(|track| track.kind == TrackKind::Video)
        .expect("sample project has video track");
    video_track.items.push(TimelineItem {
        id: "generated-item-1".to_string(),
        kind: TimelineItemKind::VideoClip,
        start_seconds: 4.0,
        duration_seconds: 3.0,
        source: TimelineSource::Media {
            media_id: "generated-output-1".to_string(),
        },
        label: "Generated hero".to_string(),
        properties: BTreeMap::from([
            ("generatedAssetId".to_string(), json!("generated-shot-1")),
            (
                "generatedOutputMediaId".to_string(),
                json!("generated-output-1"),
            ),
            ("sourceIn".to_string(), json!(0.0)),
            ("sourceOut".to_string(), json!(3.0)),
        ]),
    });

    project
}

fn sample_project_with_image_clip() -> video_creater_lib::project::model::VideoProject {
    let mut project = sample_project();
    project.timeline.duration_seconds = 4.0;
    project.media.push(MediaAsset {
        id: "still-1".to_string(),
        name: Some("product-still.png".to_string()),
        relative_path: "media/product-still.png".to_string(),
        kind: MediaKind::Image,
        duration_seconds: 3.0,
        width: Some(1920),
        height: Some(1080),
        fps: Some(24.0),
        folder_id: None,
    });

    let video_track = project
        .timeline
        .tracks
        .iter_mut()
        .find(|track| track.kind == TrackKind::Video)
        .expect("sample project has video track");
    video_track.items.clear();
    video_track.items.push(TimelineItem {
        id: "image-item-1".to_string(),
        kind: TimelineItemKind::ImageClip,
        start_seconds: 2.0,
        duration_seconds: 2.0,
        source: TimelineSource::Media {
            media_id: "still-1".to_string(),
        },
        label: "Product still".to_string(),
        properties: BTreeMap::from([
            ("sourceIn".to_string(), json!(0.25)),
            ("sourceOut".to_string(), json!(2.25)),
        ]),
    });

    project
}

fn sample_project_with_lottie_clip() -> video_creater_lib::project::model::VideoProject {
    let mut project = sample_project();
    project.timeline.duration_seconds = 5.0;
    project.media.push(MediaAsset {
        id: "lottie-1".to_string(),
        name: Some("Brand burst".to_string()),
        relative_path: "media/brand-burst.json".to_string(),
        kind: MediaKind::Lottie,
        duration_seconds: 2.0,
        width: Some(1080),
        height: Some(1080),
        fps: Some(30.0),
        folder_id: None,
    });

    let video_track = project
        .timeline
        .tracks
        .iter_mut()
        .find(|track| track.kind == TrackKind::Video)
        .expect("sample project has video track");
    video_track.items.push(TimelineItem {
        id: "lottie-item-1".to_string(),
        kind: TimelineItemKind::VideoClip,
        start_seconds: 1.0,
        duration_seconds: 2.0,
        source: TimelineSource::Media {
            media_id: "lottie-1".to_string(),
        },
        label: "Lottie burst".to_string(),
        properties: BTreeMap::from([
            ("sourceIn".to_string(), json!(0.0)),
            ("sourceOut".to_string(), json!(2.0)),
        ]),
    });

    project
}

fn sample_project_with_effect_keyframes() -> video_creater_lib::project::model::VideoProject {
    let mut project = sample_project();
    let item = &mut project.timeline.tracks[0].items[0];
    item.properties.insert(
        "effects".to_string(),
        json!([
            {
                "effectType": "stylize.glow",
                "enabled": true,
                "params": {
                    "radius": 12,
                    "intensity": 0.5
                }
            }
        ]),
    );
    item.properties.insert(
        "keyframes".to_string(),
        json!({
            "opacity": [
                { "timeSeconds": 0.0, "value": 0.3 },
                { "timeSeconds": 1.0, "value": 1.0 }
            ]
        }),
    );
    item.properties.insert(
        "colorGrade".to_string(),
        json!({
            "exposure": 0.25,
            "vibrance": 0.4
        }),
    );
    item.properties.insert("opacity".to_string(), json!(0.72));
    item.properties.insert("volumeDb".to_string(), json!(-6.0));
    project
}

fn sample_project_with_caption() -> video_creater_lib::project::model::VideoProject {
    let mut project = sample_project();
    let caption_track = project
        .timeline
        .tracks
        .iter_mut()
        .find(|track| track.kind == TrackKind::Caption)
        .expect("sample project has caption track");
    caption_track.items.push(TimelineItem {
        id: "caption-item-1".to_string(),
        kind: TimelineItemKind::Caption,
        start_seconds: 0.5,
        duration_seconds: 1.5,
        source: TimelineSource::Text {
            text: "Launch & grow <fast>".to_string(),
        },
        label: "Hook caption".to_string(),
        properties: BTreeMap::new(),
    });

    project
}

fn sample_project_with_text_overlay() -> video_creater_lib::project::model::VideoProject {
    let mut project = sample_project();
    project.timeline.duration_seconds = 5.0;
    let overlay_track = project
        .timeline
        .tracks
        .iter_mut()
        .find(|track| track.kind == TrackKind::Overlay)
        .expect("sample project has overlay track");
    overlay_track.items.push(TimelineItem {
        id: "overlay-item-1".to_string(),
        kind: TimelineItemKind::Overlay,
        start_seconds: 3.0,
        duration_seconds: 2.0,
        source: TimelineSource::Text {
            text: "Look here".to_string(),
        },
        label: "Manual callout".to_string(),
        properties: BTreeMap::from([
            (
                "visualTreatment".to_string(),
                json!("small pointer label with transparent backing"),
            ),
            ("motion".to_string(), json!("quick slide and fade")),
            ("safeZone".to_string(), json!("inside 10 percent margins")),
            ("avoid".to_string(), json!("covering faces")),
        ]),
    });

    project
}
