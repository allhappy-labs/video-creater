//! FCPXML captions as connected titles. Times are 24 fps frames. In
//! `adjacent_clips_project(true)`, `up-1` is the only primary (lane 0) clip,
//! at `[24, 72)` with source in 0, and the sequence ends at 192.

use super::structure::check_fcpxml_structure;
use super::transitions::{adjacent_clips_project, export};
use serde_json::json;
use std::collections::BTreeMap;
use video_creater_lib::project::model::{
    TimelineItem, TimelineItemKind, TimelineSource, TrackKind, VideoProject,
};
use video_creater_lib::project::nle_export::NleXmlFormat;

fn caption(id: &str, start: f64, duration: f64) -> TimelineItem {
    TimelineItem {
        id: id.to_string(),
        kind: TimelineItemKind::Caption,
        start_seconds: start,
        duration_seconds: duration,
        source: TimelineSource::Text {
            text: format!("Line {id}"),
        },
        label: format!("Caption {id}"),
        properties: BTreeMap::new(),
    }
}

/// `in-clip` starts 12 frames into `up-1`; `in-hole` starts at 96, after it.
pub(super) fn captions_project() -> VideoProject {
    let mut project = adjacent_clips_project(true);
    project
        .timeline
        .tracks
        .iter_mut()
        .find(|track| track.kind == TrackKind::Caption)
        .expect("sample project has a caption track")
        .items = vec![caption("in-clip", 1.5, 1.0), caption("in-hole", 4.0, 1.0)];
    project
}

/// The lines from `from` up to and including the first line starting with `to`, trimmed.
fn block<'a>(xml: &'a str, from: &str, to: &str) -> Vec<&'a str> {
    let lines = xml
        .lines()
        .map(str::trim)
        .skip_while(|line| !line.starts_with(from))
        .collect::<Vec<_>>();
    let end = lines
        .iter()
        .position(|line| line.starts_with(to))
        .expect("block end");
    lines[..=end].to_vec()
}

#[test]
fn fcpxml_captions_anchor_in_the_primary_clip_covering_their_start() {
    let xml = export(&captions_project(), NleXmlFormat::DavinciFcpxml);

    let host = block(&xml, "<asset-clip name=\"Clip up-1\"", "</asset-clip>");
    assert!(
        host.contains(
            &"<title name=\"Caption in-clip\" ref=\"vc-title-basic\" lane=\"2\" offset=\"12/24s\" duration=\"24/24s\">"
        ),
        "{xml}"
    );
    assert_eq!(check_fcpxml_structure(&xml), Vec::<String>::new());
}

#[test]
fn fcpxml_captions_in_a_primary_hole_anchor_in_a_gap() {
    let xml = export(&captions_project(), NleXmlFormat::DavinciFcpxml);

    let gap = block(
        &xml,
        "<gap name=\"Gap\" offset=\"96/24s\" duration=\"96/24s\">",
        "</gap>",
    );
    assert!(
        gap.contains(
            &"<title name=\"Caption in-hole\" ref=\"vc-title-basic\" lane=\"2\" offset=\"0/24s\" duration=\"24/24s\">"
        ),
        "{xml}"
    );
    assert_eq!(xml.matches("<title ").count(), 2, "{xml}");
    assert_eq!(check_fcpxml_structure(&xml), Vec::<String>::new());
}

#[test]
fn fcpxml_captions_over_a_retimed_primary_clip_stay_connected_in_the_spine() {
    // Anchor offsets on a retimed host are unverified (see the storyline module docs), so the
    // caption keeps the flat connected layout with a lane instead.
    let mut project = captions_project();
    let host = project
        .timeline
        .tracks
        .iter_mut()
        .flat_map(|track| track.items.iter_mut())
        .find(|item| item.id == "up-1")
        .expect("up-1");
    host.properties.insert("speed".to_string(), json!(2.0));
    host.properties.insert("sourceOut".to_string(), json!(4.0));

    let xml = export(&project, NleXmlFormat::DavinciFcpxml);

    assert!(
        xml.contains(
            "\n            <title name=\"Caption in-clip\" ref=\"vc-title-basic\" lane=\"2\" offset=\"36/24s\" duration=\"24/24s\">"
        ),
        "{xml}"
    );
    assert_eq!(check_fcpxml_structure(&xml), Vec::<String>::new());
}
