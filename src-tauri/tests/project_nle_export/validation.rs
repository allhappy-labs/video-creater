//! Structure and DTD validation of a corpus of exported XMEML and FCPXML.
//!
//! `nle_exports_have_consistent_structure` always runs. The DTD check needs
//! `xmllint` and the Apple DTDs, which are downloaded (never committed) by
//! `rtk pnpm verify:nle-xml`, so it is ignored by default.

use super::structure::{check_fcpxml_structure, check_xmeml_structure};
use super::transitions::{
    add_transition, adjacent_clips_project, audio_track_index, export, media_clip,
};
use serde_json::json;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::Command;
use video_creater_lib::project::fixtures::sample_project;
use video_creater_lib::project::model::{
    ProjectTimeline, TimelineItemKind, TimelineSource, TransitionKind, VideoProject,
};
use video_creater_lib::project::nle_export::NleXmlFormat;

const TRANSITION_KINDS: [(&str, TransitionKind); 4] = [
    ("crossfade", TransitionKind::Crossfade),
    ("dip-to-black", TransitionKind::DipToBlack),
    ("dip-to-white", TransitionKind::DipToWhite),
    ("wipe", TransitionKind::Wipe),
];

fn with_transition(
    with_upper_video_track: bool,
    audio: bool,
    kind: TransitionKind,
) -> VideoProject {
    let mut project = adjacent_clips_project(with_upper_video_track);
    if audio {
        let audio_index = audio_track_index(&project);
        add_transition(&mut project, audio_index, ("a-1", "a-2"), kind, 0.5);
    } else {
        add_transition(&mut project, 0, ("item-1", "item-2"), kind, 1.0);
    }
    project
}

/// Three clips joined by two crossfades on the lane-1 video track.
fn lane_chain_project() -> VideoProject {
    let mut project = adjacent_clips_project(true);
    project.timeline.tracks[0].items = vec![
        media_clip(
            "item-1",
            "media-1",
            TimelineItemKind::VideoClip,
            0.0,
            3.0,
            2.0,
        ),
        media_clip(
            "item-2",
            "media-1",
            TimelineItemKind::VideoClip,
            3.0,
            3.0,
            6.0,
        ),
        media_clip(
            "item-3",
            "media-1",
            TimelineItemKind::VideoClip,
            6.0,
            2.0,
            9.5,
        ),
    ];
    for pair in [("item-1", "item-2"), ("item-2", "item-3")] {
        add_transition(&mut project, 0, pair, TransitionKind::Crossfade, 1.0);
    }
    project
}

/// A lane-1 crossfade starting 4 s in, in the primary hole between `up-1`
/// [1, 3) and `up-2` [5, 7), so a gap at 96 hosts it.
fn gap_hosted_chain_project() -> VideoProject {
    let mut project = with_transition(true, false, TransitionKind::Crossfade);
    project.timeline.duration_seconds = 12.0;
    for item in &mut project.timeline.tracks[0].items {
        item.start_seconds += 4.0;
    }
    let upper = project.timeline.tracks.len() - 1;
    project.timeline.tracks[upper].items.push(media_clip(
        "up-2",
        "media-1",
        TimelineItemKind::VideoClip,
        5.0,
        2.0,
        0.0,
    ));
    project
}

/// A lane-1 crossfade starting 1 s into `up-1` [0, 10), which has 2 s of source in.
fn clip_hosted_chain_project() -> VideoProject {
    let mut project = with_transition(true, false, TransitionKind::Crossfade);
    project.timeline.duration_seconds = 10.0;
    for item in &mut project.timeline.tracks[0].items {
        item.start_seconds += 1.0;
    }
    let upper = project.timeline.tracks.len() - 1;
    project.timeline.tracks[upper].items[0] = media_clip(
        "up-1",
        "media-1",
        TimelineItemKind::VideoClip,
        0.0,
        10.0,
        2.0,
    );
    project
}

/// A lane-1 crossfade whose chain start is covered by `up-1` played at 2x.
fn retimed_host_project() -> VideoProject {
    let mut project = with_transition(true, false, TransitionKind::Crossfade);
    let upper = project.timeline.tracks.len() - 1;
    let host = &mut project.timeline.tracks[upper].items[0];
    host.start_seconds = 0.0;
    host.duration_seconds = 2.0;
    host.properties.insert("speed".to_string(), json!(2.0));
    host.properties.insert("sourceOut".to_string(), json!(4.0));
    project
}

/// The nested-sequence case: a crossfade inside a timeline used as a clip.
fn nested_sequence_project() -> VideoProject {
    let mut project = sample_project();
    let nested = with_transition(false, false, TransitionKind::Crossfade);
    project.media = nested.media.clone();
    project.timelines.push(ProjectTimeline {
        id: "alternate".to_string(),
        name: "Alternate".to_string(),
        timeline: nested.timeline.clone(),
    });
    let wrapper = &mut project.timeline.tracks[0].items[0];
    wrapper.start_seconds = 1.0;
    wrapper.duration_seconds = 8.0;
    wrapper.source = TimelineSource::Timeline {
        timeline_id: "alternate".to_string(),
    };
    wrapper.properties = BTreeMap::new();
    project.timeline.duration_seconds = 9.0;
    project
}

pub(super) fn corpus() -> Vec<(String, VideoProject)> {
    let mut cases = vec![
        ("caption".to_string(), super::sample_project_with_caption()),
        (
            "text-overlay".to_string(),
            super::sample_project_with_text_overlay(),
        ),
        (
            "effect-keyframes".to_string(),
            super::sample_project_with_effect_keyframes(),
        ),
        (
            "generated-clip".to_string(),
            super::sample_project_with_generated_clip(),
        ),
        (
            "audio-clip".to_string(),
            super::sample_project_with_audio_clip(),
        ),
        (
            "image-clip".to_string(),
            super::sample_project_with_image_clip(),
        ),
        (
            "lottie-clip".to_string(),
            super::sample_project_with_lottie_clip(),
        ),
        ("adjacent-clips".to_string(), adjacent_clips_project(false)),
        (
            "adjacent-clips-with-upper-track".to_string(),
            adjacent_clips_project(true),
        ),
    ];
    for (name, kind) in TRANSITION_KINDS {
        cases.push((
            format!("primary-{name}"),
            with_transition(false, false, kind),
        ));
        cases.push((format!("lane-1-{name}"), with_transition(true, false, kind)));
        cases.push((format!("audio-{name}"), with_transition(false, true, kind)));
    }
    cases.push(("lane-1-chain".to_string(), lane_chain_project()));
    cases.push((
        "lane-1-gap-hosted-chain".to_string(),
        gap_hosted_chain_project(),
    ));
    cases.push((
        "lane-1-clip-hosted-chain".to_string(),
        clip_hosted_chain_project(),
    ));
    cases.push(("retimed-host".to_string(), retimed_host_project()));
    cases.push(("nested-sequence".to_string(), nested_sequence_project()));
    cases.push((
        "reversed-transition".to_string(),
        super::reversed_clips::reversed_transition_project(),
    ));
    cases.push((
        "captions-in-clip-and-gap".to_string(),
        super::captions::captions_project(),
    ));
    cases
}

#[test]
fn nle_exports_have_consistent_structure() {
    let mut problems = Vec::new();
    for (case, project) in corpus() {
        for (extension, format, check) in [
            (
                "xml",
                NleXmlFormat::PremiereXmeml,
                check_xmeml_structure as fn(&str) -> Vec<String>,
            ),
            (
                "fcpxml",
                NleXmlFormat::DavinciFcpxml,
                check_fcpxml_structure,
            ),
        ] {
            for problem in check(&export(&project, format)) {
                problems.push(format!("{case}.{extension}: {problem}"));
            }
        }
    }
    assert!(problems.is_empty(), "{problems:#?}");
}

fn required_env(name: &str) -> PathBuf {
    std::env::var_os(name).map(PathBuf::from).unwrap_or_else(|| {
        panic!(
            "needs VIDEO_CREATER_XMLLINT, VIDEO_CREATER_NLE_DTD_DIR and VIDEO_CREATER_NLE_VALIDATION_OUTPUT; run rtk pnpm verify:nle-xml ({name} is unset)"
        )
    })
}

#[test]
#[ignore = "needs VIDEO_CREATER_XMLLINT, VIDEO_CREATER_NLE_DTD_DIR and VIDEO_CREATER_NLE_VALIDATION_OUTPUT; run rtk pnpm verify:nle-xml"]
fn nle_exports_validate_against_the_dtds() {
    let xmllint = required_env("VIDEO_CREATER_XMLLINT");
    let dtd_dir = required_env("VIDEO_CREATER_NLE_DTD_DIR");
    let output = required_env("VIDEO_CREATER_NLE_VALIDATION_OUTPUT");
    let corpus_dir = output.join("corpus");
    std::fs::create_dir_all(&corpus_dir).expect("create the corpus folder");

    let mut results = Vec::new();
    for (case, project) in corpus() {
        for (extension, format, format_name, dtd) in [
            ("xml", NleXmlFormat::PremiereXmeml, "xmeml", "xmeml-v5.dtd"),
            (
                "fcpxml",
                NleXmlFormat::DavinciFcpxml,
                "fcpxml",
                "FCPXMLv1_10.dtd",
            ),
        ] {
            let file = format!("corpus/{case}.{extension}");
            let path = output.join(&file);
            std::fs::write(&path, export(&project, format)).expect("write a corpus file");
            let dtd_path = dtd_dir.join(dtd);
            let run = xmllint_dtdvalid(&xmllint, &dtd_path, &path);
            results.push(json!({
                "case": case,
                "format": format_name,
                "file": file,
                "dtd": dtd_path,
                "exitCode": run.0,
                "stderr": run.1,
            }));
        }
    }
    std::fs::write(
        output.join("results.json"),
        serde_json::to_string_pretty(&results).expect("serialize results"),
    )
    .expect("write results.json");

    let failures: Vec<String> = results
        .iter()
        .filter(|result| result["exitCode"] != 0)
        .map(|result| {
            format!(
                "{}\n{}",
                result["file"],
                result["stderr"].as_str().unwrap_or("")
            )
        })
        .collect();
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

fn xmllint_dtdvalid(xmllint: &Path, dtd: &Path, file: &Path) -> (i32, String) {
    let output = Command::new(xmllint)
        .arg("--noout")
        .arg("--dtdvalid")
        .arg(dtd)
        .arg(file)
        .output()
        .unwrap_or_else(|error| panic!("run {}: {error}", xmllint.display()));
    (
        output.status.code().unwrap_or(-1),
        String::from_utf8_lossy(&output.stderr).into_owned(),
    )
}
