//! FCPXML connected secondary storylines.
//!
//! FCPXML can only hold transitions inside a spine, and per the FCPXML 1.10
//! DTD a `spine` is an anchor item: it can't sit directly inside another spine.
//! A **chain** is a maximal run of clips on one non-zero lane joined by emitted
//! transitions. Each chain is written as `<spine lane="N" offset="…">` whose
//! clips and transitions use offsets relative to the chain start, and it is
//! anchored in one of two places (structure verified from Final Cut Pro
//! exports; see `docs/research/2026-09-16-nle-transition-interchange-references.md`, R3):
//!
//! - inside the lane-0 video clip covering the chain start, with `offset` in
//!   that clip's local time (`start` plus the distance into the clip);
//! - otherwise inside a `<gap>` in the primary spine that fills the primary
//!   hole from the earliest chain start in that hole to the next lane-0 clip
//!   (or the sequence end). Chains starting in the same hole share the gap.
//!
//! Anchor offsets on a retimed (timeMap) host are unverified, so chains whose
//! host is retimed are marked before planning and export as cuts with a note.
//! Clips that belong to no chain keep the flat `lane="N"` layout.
//!
//! Captions are connected titles anchored the same way: in the primary clip
//! covering their start, or in the gap filling that hole. A caption whose
//! covering clip is retimed keeps the flat `lane="N"` layout in the spine.

use super::fcpxml_spine::{push_fcpxml_audio_clip, push_fcpxml_video_clip, FcpxmlPlacement};
use super::fcpxml_titles::push_fcpxml_caption;
use super::fcpxml_transitions::{fcpxml_emits, fcpxml_joins, push_fcpxml_transition};
use super::transitions::NleTransitionSpan;
use super::{clip_is_retimed, NleCaption, NleClip};
use crate::project::model::TrackKind;
use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct FcpxmlChain {
    pub(super) lane: i64,
    pub(super) start_frames: i64,
    pub(super) clip_indices: Vec<usize>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum FcpxmlChainHost {
    Clip { clip_index: usize },
    Gap { gap_index: usize },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct FcpxmlGap {
    pub(super) start_frames: i64,
    pub(super) end_frames: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct FcpxmlStorylinePlan {
    /// Sorted by start, then by clip order; `hosts[i]` hosts `chains[i]`.
    pub(super) chains: Vec<FcpxmlChain>,
    pub(super) hosts: Vec<FcpxmlChainHost>,
    /// `caption_hosts[i]` hosts caption `i`; none when its covering clip is retimed.
    pub(super) caption_hosts: Vec<Option<FcpxmlChainHost>>,
    /// Sorted by offset.
    pub(super) gaps: Vec<FcpxmlGap>,
    /// Indices of every clip written inside a storyline.
    pub(super) chained: BTreeSet<usize>,
}

pub(super) fn plan_fcpxml_storylines(
    clips: &[NleClip<'_>],
    captions: &[NleCaption<'_>],
    sequence_frames: i64,
) -> FcpxmlStorylinePlan {
    let caption_starts = captions
        .iter()
        .map(|caption| caption.timeline_start_frames)
        .collect::<Vec<_>>();
    plan_storylines_joined_by(clips, &caption_starts, sequence_frames, fcpxml_emits)
}

/// Marks the transitions of chains whose host would be a retimed clip, so
/// FCPXML cuts them (and notes it) instead of anchoring on the clip.
pub(super) fn mark_retimed_storyline_hosts(clips: &mut [NleClip<'_>]) {
    let plan = plan_storylines_joined_by(clips, &[], i64::MAX, fcpxml_joins);
    for (chain, host) in plan.chains.iter().zip(&plan.hosts) {
        let FcpxmlChainHost::Clip { clip_index } = *host else {
            continue;
        };
        if !clip_is_retimed(&clips[clip_index]) {
            continue;
        }
        for pair in chain.clip_indices.windows(2) {
            for span in [
                &mut clips[pair[0]].outgoing_transition,
                &mut clips[pair[1]].incoming_transition,
            ]
            .into_iter()
            .flatten()
            {
                span.retimed_host = true;
            }
        }
    }
}

fn is_primary_video(clip: &NleClip<'_>) -> bool {
    clip.track_lane == 0 && clip.track_kind == TrackKind::Video
}

/// Where an anchored item starting at a frame goes: the covering primary clip,
/// or the primary hole ending at `end_frames`.
#[derive(Debug, Clone, Copy)]
enum Anchor {
    Clip(usize),
    Hole { end_frames: i64 },
}

fn anchor_at(clips: &[NleClip<'_>], start: i64, sequence_frames: i64) -> Anchor {
    if let Some(clip_index) = clips.iter().position(|clip| {
        is_primary_video(clip)
            && clip.timeline_start_frames <= start
            && start < clip.timeline_end_frames
    }) {
        return Anchor::Clip(clip_index);
    }
    let end_frames = clips
        .iter()
        .filter(|clip| is_primary_video(clip) && clip.timeline_start_frames > start)
        .map(|clip| clip.timeline_start_frames)
        .min()
        .unwrap_or(sequence_frames)
        .max(start + 1);
    Anchor::Hole { end_frames }
}

fn plan_storylines_joined_by(
    clips: &[NleClip<'_>],
    caption_starts: &[i64],
    sequence_frames: i64,
    joins: fn(&NleTransitionSpan) -> bool,
) -> FcpxmlStorylinePlan {
    let mut chains = Vec::new();
    let mut index = 0;
    while index < clips.len() {
        let mut clip_indices = vec![index];
        // `attach_nle_transitions` stores a span on a clip and the next clip.
        while clips[index].track_lane != 0
            && index + 1 < clips.len()
            && clips[index]
                .outgoing_transition
                .is_some_and(|span| joins(&span))
        {
            index += 1;
            clip_indices.push(index);
        }
        if clip_indices.len() > 1 {
            let first = &clips[clip_indices[0]];
            chains.push(FcpxmlChain {
                lane: first.track_lane,
                start_frames: first.timeline_start_frames,
                clip_indices,
            });
        }
        index += 1;
    }
    chains.sort_by_key(|chain| (chain.start_frames, chain.clip_indices[0]));

    let chain_anchors = chains
        .iter()
        .map(|chain| anchor_at(clips, chain.start_frames, sequence_frames))
        .collect::<Vec<_>>();
    let caption_anchors = caption_starts
        .iter()
        .map(|&start| match anchor_at(clips, start, sequence_frames) {
            Anchor::Clip(clip_index) if clip_is_retimed(&clips[clip_index]) => None,
            anchor => Some(anchor),
        })
        .collect::<Vec<_>>();
    // Items starting in the same hole share one gap from the earliest of them.
    let mut holes = BTreeMap::<i64, i64>::new();
    let starts = chains.iter().map(|chain| chain.start_frames);
    for (start, anchor) in starts.zip(chain_anchors.iter().copied().map(Some)).chain(
        caption_starts
            .iter()
            .copied()
            .zip(caption_anchors.iter().copied()),
    ) {
        if let Some(Anchor::Hole { end_frames }) = anchor {
            let hole_start = holes.entry(end_frames).or_insert(start);
            *hole_start = (*hole_start).min(start);
        }
    }
    let host = |anchor: Anchor| match anchor {
        Anchor::Clip(clip_index) => FcpxmlChainHost::Clip { clip_index },
        Anchor::Hole { end_frames } => FcpxmlChainHost::Gap {
            gap_index: holes
                .keys()
                .position(|&end| end == end_frames)
                .expect("every hole has a gap"),
        },
    };
    let hosts = chain_anchors.iter().copied().map(host).collect();
    let caption_hosts = caption_anchors
        .iter()
        .map(|anchor| anchor.map(host))
        .collect();
    let gaps = holes
        .iter()
        .map(|(&end_frames, &start_frames)| FcpxmlGap {
            start_frames,
            end_frames,
        })
        .collect();
    let chained = chains
        .iter()
        .flat_map(|chain| chain.clip_indices.iter().copied())
        .collect();
    FcpxmlStorylinePlan {
        chains,
        hosts,
        caption_hosts,
        gaps,
        chained,
    }
}

/// Writes the gap `gap_index` and the storylines and captions it hosts, in the primary spine.
pub(super) fn push_fcpxml_gap(
    xml: &mut String,
    plan: &FcpxmlStorylinePlan,
    gap_index: usize,
    (clips, captions): (&[NleClip<'_>], &[NleCaption<'_>]),
    caption_lane: i64,
    fps: i64,
) {
    let gap = plan.gaps[gap_index];
    xml.push_str(&format!(
        "            <gap name=\"Gap\" offset=\"{}/{fps}s\" duration=\"{}/{fps}s\">\n",
        gap.start_frames,
        gap.end_frames - gap.start_frames
    ));
    for (chain, _) in plan
        .chains
        .iter()
        .zip(&plan.hosts)
        .filter(|(_, host)| **host == FcpxmlChainHost::Gap { gap_index })
    {
        let offset = chain.start_frames - gap.start_frames;
        push_fcpxml_storyline(xml, chain, offset, clips, fps, 14);
    }
    // A gap has no `start`, so its local time begins at 0s.
    let host = FcpxmlChainHost::Gap { gap_index };
    push_anchored_fcpxml_captions(
        xml,
        plan,
        host,
        (gap.start_frames, 0),
        captions,
        caption_lane,
        fps,
    );
    xml.push_str("            </gap>\n");
}

/// Writes the captions `host` anchors as connected titles on `caption_lane`.
/// `(timeline_start, local_start)` maps the host's timeline start to its local time.
fn push_anchored_fcpxml_captions(
    xml: &mut String,
    plan: &FcpxmlStorylinePlan,
    host: FcpxmlChainHost,
    (timeline_start, local_start): (i64, i64),
    captions: &[NleCaption<'_>],
    caption_lane: i64,
    fps: i64,
) {
    for (caption, _) in captions
        .iter()
        .zip(&plan.caption_hosts)
        .filter(|(_, caption_host)| **caption_host == Some(host))
    {
        let offset = local_start + (caption.timeline_start_frames - timeline_start);
        push_fcpxml_caption(xml, caption, caption_lane, offset, fps, 14);
    }
}

/// The storylines and captions anchored in the primary clip `clip_index`,
/// indented for a clip written in the sequence spine. Empty when it hosts none.
pub(super) fn anchored_fcpxml_storylines(
    plan: &FcpxmlStorylinePlan,
    clip_index: usize,
    (clips, captions): (&[NleClip<'_>], &[NleCaption<'_>]),
    caption_lane: i64,
    fps: i64,
) -> String {
    let host = &clips[clip_index];
    let mut xml = String::new();
    for (chain, _) in plan
        .chains
        .iter()
        .zip(&plan.hosts)
        .filter(|(_, host)| **host == FcpxmlChainHost::Clip { clip_index })
    {
        // Hosts are never retimed, so local time is `start` plus the distance
        // into the clip, the same frames `fcpxml_clip_start` writes.
        let offset = host.source_in_frames + (chain.start_frames - host.timeline_start_frames);
        push_fcpxml_storyline(&mut xml, chain, offset, clips, fps, 14);
    }
    push_anchored_fcpxml_captions(
        &mut xml,
        plan,
        FcpxmlChainHost::Clip { clip_index },
        (host.timeline_start_frames, host.source_in_frames),
        captions,
        caption_lane,
        fps,
    );
    xml
}

/// Writes `chain` as `<spine lane="N">` at `host_local_offset`, with clip and
/// transition offsets relative to the chain start.
pub(super) fn push_fcpxml_storyline(
    xml: &mut String,
    chain: &FcpxmlChain,
    host_local_offset: i64,
    clips: &[NleClip<'_>],
    fps: i64,
    indent: usize,
) {
    let pad = " ".repeat(indent);
    xml.push_str(&format!(
        "{pad}<spine lane=\"{}\" offset=\"{host_local_offset}/{fps}s\">\n",
        chain.lane
    ));
    for (position, &clip_index) in chain.clip_indices.iter().enumerate() {
        let clip = &clips[clip_index];
        let placement = FcpxmlPlacement {
            lane: None,
            offset_frames: clip.timeline_start_frames - chain.start_frames,
            indent: indent + 2,
        };
        if clip.track_kind == TrackKind::Audio {
            push_fcpxml_audio_clip(xml, clip, fps, placement);
        } else {
            push_fcpxml_video_clip(xml, clip, fps, placement, "");
        }
        if position + 1 < chain.clip_indices.len() {
            if let Some(span) = clip.outgoing_transition {
                let offset = span.start_frames - chain.start_frames;
                push_fcpxml_transition(xml, span, offset, fps, indent + 2);
            }
        }
    }
    xml.push_str(&format!("{pad}</spine>\n"));
}

#[cfg(test)]
mod tests {
    use super::{plan_fcpxml_storylines, FcpxmlChain, FcpxmlChainHost, FcpxmlGap};
    use crate::project::model::{
        MediaAsset, MediaKind, TimelineItem, TimelineItemKind, TimelineSource, TrackKind,
        TransitionKind,
    };
    use crate::project::nle_export::transitions::{NleTransitionSpan, TransitionMedia};
    use crate::project::nle_export::NleClip;
    use std::collections::BTreeMap;

    fn fixtures() -> (TimelineItem, MediaAsset) {
        let item = TimelineItem {
            id: "item".to_string(),
            kind: TimelineItemKind::VideoClip,
            start_seconds: 0.0,
            duration_seconds: 1.0,
            source: TimelineSource::Media {
                media_id: "media".to_string(),
            },
            label: "Item".to_string(),
            properties: BTreeMap::new(),
        };
        let media = MediaAsset {
            id: "media".to_string(),
            name: None,
            relative_path: "media/input.mp4".to_string(),
            kind: MediaKind::Video,
            duration_seconds: 100.0,
            width: None,
            height: None,
            fps: None,
            folder_id: None,
        };
        (item, media)
    }

    fn clip<'a>(
        (item, media): &'a (TimelineItem, MediaAsset),
        lane: i64,
        (start, end): (i64, i64),
    ) -> NleClip<'a> {
        NleClip {
            item,
            media,
            generated_asset: None,
            track_kind: if lane < 0 {
                TrackKind::Audio
            } else {
                TrackKind::Video
            },
            track_lane: lane,
            track_enabled: true,
            timeline_start_frames: start,
            timeline_end_frames: end,
            duration_frames: end - start,
            source_in_frames: 0,
            source_out_frames: end - start,
            speed: 1.0,
            sequence_width: 1920,
            sequence_height: 1080,
            incoming_transition: None,
            outgoing_transition: None,
        }
    }

    /// Joins `clips[left]` and `clips[left + 1]` with a crossfade at their cut.
    fn join(clips: &mut [NleClip<'_>], left: usize) {
        let cut = clips[left + 1].timeline_start_frames;
        let span = NleTransitionSpan {
            kind: TransitionKind::Crossfade,
            start_frames: cut - 6,
            cut_frames: cut,
            end_frames: cut + 6,
            media: if clips[left].track_lane < 0 {
                TransitionMedia::Audio
            } else {
                TransitionMedia::Video
            },
            retimed_host: false,
            reversed_clip: false,
        };
        clips[left].outgoing_transition = Some(span);
        clips[left + 1].incoming_transition = Some(span);
    }

    #[test]
    fn chains_start_in_holes_and_share_their_gap() {
        let fixture = fixtures();
        let mut clips = vec![
            clip(&fixture, 1, (0, 48)),
            clip(&fixture, 1, (48, 96)),
            clip(&fixture, 0, (120, 240)),
            clip(&fixture, -1, (24, 72)),
            clip(&fixture, -1, (72, 96)),
        ];
        join(&mut clips, 0);
        join(&mut clips, 3);

        let plan = plan_fcpxml_storylines(&clips, &[], 240);

        assert_eq!(
            plan.chains,
            vec![
                FcpxmlChain {
                    lane: 1,
                    start_frames: 0,
                    clip_indices: vec![0, 1],
                },
                FcpxmlChain {
                    lane: -1,
                    start_frames: 24,
                    clip_indices: vec![3, 4],
                },
            ]
        );
        assert_eq!(
            plan.gaps,
            vec![FcpxmlGap {
                start_frames: 0,
                end_frames: 120,
            }]
        );
        assert_eq!(plan.hosts, vec![FcpxmlChainHost::Gap { gap_index: 0 }; 2]);
        assert_eq!(
            plan.chained.into_iter().collect::<Vec<_>>(),
            vec![0, 1, 3, 4]
        );
    }

    #[test]
    fn a_gap_after_the_last_primary_clip_runs_to_the_sequence_end() {
        let fixture = fixtures();
        let mut clips = vec![
            clip(&fixture, 0, (0, 48)),
            clip(&fixture, 2, (96, 120)),
            clip(&fixture, 2, (120, 168)),
        ];
        join(&mut clips, 1);

        let plan = plan_fcpxml_storylines(&clips, &[], 192);

        assert_eq!(
            plan.gaps,
            vec![FcpxmlGap {
                start_frames: 96,
                end_frames: 192,
            }]
        );
    }

    #[test]
    fn the_covering_primary_clip_hosts_a_chain() {
        let fixture = fixtures();
        let mut clips = vec![
            clip(&fixture, 0, (0, 48)),
            clip(&fixture, 0, (48, 192)),
            clip(&fixture, 1, (48, 96)),
            clip(&fixture, 1, (96, 144)),
        ];
        join(&mut clips, 2);

        let plan = plan_fcpxml_storylines(&clips, &[], 192);

        assert_eq!(plan.hosts, vec![FcpxmlChainHost::Clip { clip_index: 1 }]);
        assert!(plan.gaps.is_empty());
    }

    #[test]
    fn a_missing_transition_splits_chains() {
        let fixture = fixtures();
        let mut clips = vec![
            clip(&fixture, 1, (0, 24)),
            clip(&fixture, 1, (24, 48)),
            clip(&fixture, 1, (48, 72)),
            clip(&fixture, 1, (72, 96)),
            clip(&fixture, 1, (96, 120)),
        ];
        join(&mut clips, 0);
        join(&mut clips, 2);
        join(&mut clips, 3);

        let plan = plan_fcpxml_storylines(&clips, &[], 120);

        let indices: Vec<Vec<usize>> = plan
            .chains
            .iter()
            .map(|chain| chain.clip_indices.clone())
            .collect();
        assert_eq!(indices, vec![vec![0, 1], vec![2, 3, 4]]);
        assert_eq!(plan.gaps.len(), 1);
    }
}
