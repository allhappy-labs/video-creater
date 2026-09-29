//! Transitions of clips that precompose prepares: LUT intermediates include
//! their transition handles, and flattened composites bake the transitions
//! they cover. Both render like the canonical preview sampler.

use super::parity::{assert_render_parity, fixture_dir, load_fixture, transition_sample_times};
use serde_json::json;
use video_creater_lib::precompose::{prepare_project_for_render, render_canonical_frame_rgba};
use video_creater_lib::project::model::{TimelineSource, TransitionKind, VideoProject};
use video_creater_lib::render_pipeline::transition_plan::plan_clip_transitions;

const INVERT_LUT: &str = "LUT_3D_SIZE 2\n1 1 1\n0 1 1\n1 0 1\n0 0 1\n1 1 0\n0 1 0\n1 0 0\n0 0 0\n";

#[test]
fn lut_prepared_clips_keep_their_transition_handles() {
    let dir = fixture_dir();
    std::fs::create_dir_all(dir.path().join("looks")).expect("looks directory");
    std::fs::write(dir.path().join("looks/invert.cube"), INVERT_LUT).expect("LUT");
    let mut project = load_fixture();
    // clip-1 (2-4 s) is incoming for the crossfade and outgoing for the dip to black.
    project.timeline.tracks[0].items[1].properties.insert(
        "colorGrade".to_string(),
        json!({ "lut": { "path": "looks/invert.cube", "strength": 1.0 } }),
    );

    let prepared = prepare_project_for_render(dir.path(), &project).expect("prepare LUT");
    let item = &prepared.project.timeline.tracks[0].items[1];
    let TimelineSource::Media { media_id } = &item.source else {
        panic!("prepared LUT clip is media-backed");
    };
    let media = prepared
        .project
        .media
        .iter()
        .find(|media| media.id == *media_id)
        .expect("prepared LUT media");
    assert!(media_id.starts_with("precompose-"));
    assert_eq!(item.properties["sourceIn"], json!(0.5));
    assert_eq!(item.properties["sourceOut"], json!(2.5));
    assert_eq!(
        media.duration_seconds, 3.0,
        "0.5 s head + 2 s clip + 0.5 s tail"
    );
    let planned = plan_clip_transitions(&prepared.project);
    assert_eq!(planned.len(), 4, "the LUT clip keeps both transitions");
    assert!(planned
        .iter()
        .all(|transition| (transition.duration_seconds - 1.0).abs() < 1e-6));

    // The intermediate's head handle holds the LUT applied to the source
    // before sourceIn: prepared 0.25 s is source 0.75 s, inverted.
    let probe = |base: &VideoProject, source_in: f64| {
        let mut probe = base.clone();
        let track = &mut probe.timeline.tracks[0];
        track.transitions.clear();
        track.items.retain(|item| item.id == "clip-1");
        let item = &mut track.items[0];
        item.properties.remove("colorGrade");
        item.properties
            .insert("sourceIn".to_string(), json!(source_in));
        item.properties
            .insert("sourceOut".to_string(), json!(source_in + 2.0));
        render_canonical_frame_rgba(dir.path(), &probe, 2.0, None).expect("probe frame")
    };
    let prepared_handle = probe(&prepared.project, 0.25);
    let source = probe(&project, 0.75);
    for (prepared_pixel, source_pixel) in
        prepared_handle.chunks_exact(4).zip(source.chunks_exact(4))
    {
        for channel in 0..3 {
            let inverted = 255 - source_pixel[channel];
            assert!(
                prepared_pixel[channel].abs_diff(inverted) <= 2,
                "handle frame {prepared_pixel:?} is not the inverted source {source_pixel:?}"
            );
        }
    }

    assert_render_parity(
        "lut-handles",
        dir.path(),
        &prepared.project,
        &prepared.project,
        &transition_sample_times(&[2.0, 4.0]),
    );
}

#[test]
fn flattened_rich_clips_bake_the_transitions_they_cover() {
    let dir = fixture_dir();
    let mut project = load_fixture();
    project.timeline.tracks[0].items[1]
        .properties
        .insert("blendMode".to_string(), json!("screen"));

    let prepared = prepare_project_for_render(dir.path(), &project).expect("prepare flatten");
    let track = &prepared.project.timeline.tracks[0];
    let spans = track
        .items
        .iter()
        .map(|item| {
            (
                item.id.split('-').take(2).collect::<Vec<_>>().join("-"),
                item.start_seconds,
                item.start_seconds + item.duration_seconds,
            )
        })
        .collect::<Vec<_>>();
    assert_eq!(
        spans,
        [
            ("clip-0".to_string(), 0.0, 1.5),
            ("flatten-clip".to_string(), 1.5, 4.5),
            ("clip-2".to_string(), 4.5, 6.0),
            ("clip-3".to_string(), 6.0, 8.0),
            ("clip-4".to_string(), 8.0, 10.0),
        ],
        "the flattened group covers both windows of the rich clip"
    );
    let kinds = plan_clip_transitions(&prepared.project)
        .into_iter()
        .map(|transition| (transition.left_item_id, transition.kind))
        .collect::<Vec<_>>();
    assert_eq!(
        kinds,
        [
            (
                "clip-2-precompose-segment-1".to_string(),
                TransitionKind::DipToWhite
            ),
            ("clip-3".to_string(), TransitionKind::Wipe),
        ],
        "baked transitions are dropped; the others follow the split clip"
    );

    // Canonical frames of the original project against GES renders of the
    // prepared one, through the baked and the native transitions.
    assert_render_parity(
        "flattened-transitions",
        dir.path(),
        &project,
        &prepared.project,
        &transition_sample_times(&[2.0, 4.0, 6.0]),
    );
}
