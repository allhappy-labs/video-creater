use super::media::{push_truncation_marker, timeline_item_count, transcript_excerpt};
use super::timeline::{canonical_timeline_selection, TimelineSelection};
use super::{
    CodexConversationEditRequest, MAX_CONTEXT_CONVERSATION_TRANSCRIPTS, MAX_CONTEXT_TIMELINE_ITEMS,
    MAX_CONTEXT_TRACKS,
};
use crate::project::model::{
    GeneratedAsset, MediaFolder, TimelineItem, TimelineSource, VideoProject,
};
use std::collections::{BTreeMap, BTreeSet};

const TIER_EXPLICIT: u8 = 0;
const TIER_FOCUS_REFERENCE: u8 = 1;
const TIER_ACTIVE_TIMELINE: u8 = 2;
const TIER_REMAINING: u8 = 3;

/// Canonical IDs referenced by a set of timeline items.
#[derive(Default)]
struct ContextReferences<'a> {
    media: BTreeSet<&'a str>,
    generated_assets: BTreeSet<&'a str>,
    templates: BTreeSet<&'a str>,
}

impl<'a> ContextReferences<'a> {
    fn add_item(&mut self, item: &'a TimelineItem) {
        match &item.source {
            TimelineSource::Media { media_id } => {
                self.media.insert(media_id);
            }
            TimelineSource::Generated { artifact_id } => {
                self.generated_assets.insert(artifact_id);
            }
            TimelineSource::Timeline { .. } | TimelineSource::Text { .. } => {}
        }
        let property = |key: &str| item.properties.get(key).and_then(serde_json::Value::as_str);
        if let Some(asset_id) = property("generatedAssetId") {
            self.generated_assets.insert(asset_id);
        }
        if let Some(media_id) = property("generatedOutputMediaId") {
            self.media.insert(media_id);
        }
        if let Some(template_id) = property("templateId") {
            self.templates.insert(template_id);
        }
    }
}

pub(super) struct ConversationRelevance<'a> {
    project: &'a VideoProject,
    explicit_media: BTreeSet<&'a str>,
    explicit_items: BTreeSet<&'a str>,
    range: Option<(f64, f64)>,
    focused: ContextReferences<'a>,
    active_timeline: ContextReferences<'a>,
    folder_tiers: BTreeMap<&'a str, u8>,
}

impl<'a> ConversationRelevance<'a> {
    pub(super) fn new(
        project: &'a VideoProject,
        request: &'a CodexConversationEditRequest,
    ) -> Self {
        let focus = &request.focus;
        let mut relevance = Self {
            project,
            explicit_media: focus
                .primary_media_id
                .iter()
                .chain(&focus.media_ids)
                .map(String::as_str)
                .collect(),
            explicit_items: focus.timeline_item_ids.iter().map(String::as_str).collect(),
            range: focus
                .timeline_range
                .as_ref()
                .map(|range| (range.start_seconds, range.end_seconds)),
            focused: ContextReferences::default(),
            active_timeline: ContextReferences::default(),
            folder_tiers: BTreeMap::new(),
        };
        let mut focused = ContextReferences::default();
        let mut active_timeline = ContextReferences::default();
        for item in active_timeline_items(project) {
            active_timeline.add_item(item);
            if relevance.item_tier(item) <= TIER_FOCUS_REFERENCE {
                focused.add_item(item);
            }
        }
        relevance.focused = focused;
        relevance.active_timeline = active_timeline;
        relevance.folder_tiers = relevance.folder_tiers();
        relevance
    }

    fn item_tier(&self, item: &TimelineItem) -> u8 {
        if self.explicit_items.contains(item.id.as_str()) {
            return TIER_EXPLICIT;
        }
        let in_range = self.range.is_some_and(|(start, end)| {
            item.start_seconds < end && item.start_seconds + item.duration_seconds > start
        });
        let from_focused_media = matches!(
            &item.source,
            TimelineSource::Media { media_id } if self.explicit_media.contains(media_id.as_str())
        );
        if in_range || from_focused_media {
            TIER_FOCUS_REFERENCE
        } else {
            TIER_ACTIVE_TIMELINE
        }
    }

    pub(super) fn media_tier(&self, media_id: &str) -> u8 {
        if self.explicit_media.contains(media_id) {
            TIER_EXPLICIT
        } else if self.focused.media.contains(media_id) {
            TIER_FOCUS_REFERENCE
        } else if self.active_timeline.media.contains(media_id) {
            TIER_ACTIVE_TIMELINE
        } else {
            TIER_REMAINING
        }
    }

    /// A folder ranks with the most relevant media inside it or any subfolder.
    fn folder_tiers(&self) -> BTreeMap<&'a str, u8> {
        let parents = self
            .project
            .media_folders
            .iter()
            .map(|folder| (folder.id.as_str(), folder.parent_id.as_deref()))
            .collect::<BTreeMap<_, _>>();
        let mut tiers = BTreeMap::<&str, u8>::new();
        for media in &self.project.media {
            let tier = self.media_tier(&media.id);
            let mut visited = BTreeSet::new();
            let mut folder_id = media.folder_id.as_deref();
            while let Some(id) = folder_id.filter(|id| visited.insert(*id)) {
                let entry = tiers.entry(id).or_insert(TIER_REMAINING);
                *entry = (*entry).min(tier);
                folder_id = parents.get(id).copied().flatten();
            }
        }
        tiers
    }

    pub(super) fn folder_tier(&self, folder: &MediaFolder) -> u8 {
        self.folder_tiers
            .get(folder.id.as_str())
            .copied()
            .unwrap_or(TIER_REMAINING)
    }

    pub(super) fn generated_asset_tier(&self, asset: &GeneratedAsset) -> u8 {
        let output_tier = asset
            .outputs
            .iter()
            .map(|output| self.media_tier(&output.media_id))
            .min()
            .unwrap_or(TIER_REMAINING);
        let references = &asset.references;
        let references_focused_media = references
            .first_frame_media_id
            .iter()
            .chain(&references.last_frame_media_id)
            .chain(&references.media_ids)
            .any(|media_id| self.explicit_media.contains(media_id.as_str()));
        let asset_tier = if self.focused.generated_assets.contains(asset.id.as_str())
            || references_focused_media
        {
            TIER_FOCUS_REFERENCE
        } else if self
            .active_timeline
            .generated_assets
            .contains(asset.id.as_str())
        {
            TIER_ACTIVE_TIMELINE
        } else {
            TIER_REMAINING
        };
        output_tier.min(asset_tier)
    }

    pub(super) fn template_tier(&self, template_id: &str) -> u8 {
        if self.focused.templates.contains(template_id) {
            TIER_FOCUS_REFERENCE
        } else if self.active_timeline.templates.contains(template_id) {
            TIER_ACTIVE_TIMELINE
        } else {
            TIER_REMAINING
        }
    }

    /// Complete timeline when it fits; otherwise tracks rank by their most
    /// relevant item and items rank globally, both ties in canonical order.
    pub(super) fn timeline_selection(
        &self,
        bounded: &mut Vec<&'static str>,
    ) -> TimelineSelection<'a> {
        let tracks = &self.project.timeline.tracks;
        if tracks.len() <= MAX_CONTEXT_TRACKS
            && timeline_item_count(self.project) <= MAX_CONTEXT_TIMELINE_ITEMS
        {
            return canonical_timeline_selection(self.project);
        }
        bounded.push("timeline");

        let mut ranked_tracks = tracks
            .iter()
            .enumerate()
            .map(|(index, track)| {
                let tier = track
                    .items
                    .iter()
                    .map(|item| self.item_tier(item))
                    .min()
                    .unwrap_or(TIER_ACTIVE_TIMELINE);
                (tier, index, track)
            })
            .collect::<Vec<_>>();
        ranked_tracks.sort_by_key(|(tier, index, _)| (*tier, *index));
        ranked_tracks.truncate(MAX_CONTEXT_TRACKS);

        let mut ranked_items = ranked_tracks
            .iter()
            .enumerate()
            .flat_map(|(track_rank, (_, _, track))| {
                track
                    .items
                    .iter()
                    .enumerate()
                    .map(move |(index, item)| (self.item_tier(item), track_rank, index))
            })
            .collect::<Vec<_>>();
        ranked_items.sort_unstable();
        ranked_items.truncate(MAX_CONTEXT_TIMELINE_ITEMS);

        ranked_tracks
            .iter()
            .enumerate()
            .map(|(track_rank, (_, _, track))| {
                let items = ranked_items
                    .iter()
                    .filter(|(_, rank, _)| *rank == track_rank)
                    .map(|(_, _, index)| &track.items[*index])
                    .collect();
                (*track, items)
            })
            .collect()
    }

    pub(super) fn transcript_excerpts(&self, bounded: &mut Vec<&'static str>) -> String {
        let eligible = self
            .project
            .transcripts
            .iter()
            .filter_map(|transcript| {
                self.project
                    .media
                    .iter()
                    .find(|media| media.id == transcript.media_id)
                    .map(|media| (media, transcript))
            })
            .collect::<Vec<_>>();
        if eligible.is_empty() {
            return "- none".to_string();
        }
        let selected = select_ranked(
            &eligible,
            MAX_CONTEXT_CONVERSATION_TRANSCRIPTS,
            "transcripts",
            bounded,
            |(media, _)| self.media_tier(&media.id),
        );
        let mut lines = selected
            .iter()
            .map(|(media, transcript)| {
                format!(
                    "- media {}: {}\n{}",
                    media.id,
                    media.relative_path,
                    transcript_excerpt(media, &transcript.words)
                )
            })
            .collect::<Vec<_>>();
        push_truncation_marker(&mut lines, eligible.len(), selected.len(), "transcripts");
        lines.join("\n")
    }
}

fn active_timeline_items(project: &VideoProject) -> impl Iterator<Item = &TimelineItem> {
    project
        .timeline
        .tracks
        .iter()
        .flat_map(|track| track.items.iter())
}

pub(super) fn generated_output_count(project: &VideoProject) -> usize {
    project
        .generated_assets
        .iter()
        .map(|asset| asset.outputs.len())
        .sum()
}

/// The whole collection in canonical order when it fits `cap`; otherwise the
/// `cap` best-ranked entries (lowest tier, then canonical order).
pub(super) fn select_ranked<'e, T>(
    entries: &'e [T],
    cap: usize,
    name: &'static str,
    bounded: &mut Vec<&'static str>,
    tier: impl Fn(&T) -> u8,
) -> Vec<&'e T> {
    if entries.len() <= cap {
        return entries.iter().collect();
    }
    bounded.push(name);
    let mut ranked = entries
        .iter()
        .enumerate()
        .map(|(index, entry)| (tier(entry), index))
        .collect::<Vec<_>>();
    ranked.sort_unstable();
    ranked
        .into_iter()
        .take(cap)
        .map(|(_, index)| &entries[index])
        .collect()
}

/// The whole collection in canonical order when it fits `cap`; otherwise the
/// `cap` newest entries by RFC 3339 timestamp, newest first.
pub(super) fn select_newest<'e, T>(
    entries: &'e [T],
    cap: usize,
    name: &'static str,
    bounded: &mut Vec<&'static str>,
    timestamp: impl Fn(&T) -> &str,
) -> Vec<&'e T> {
    if entries.len() <= cap {
        return entries.iter().collect();
    }
    bounded.push(name);
    let mut ranked = entries.iter().enumerate().collect::<Vec<_>>();
    ranked.sort_by(|(left_index, left), (right_index, right)| {
        timestamp(right)
            .cmp(timestamp(left))
            .then(right_index.cmp(left_index))
    });
    ranked
        .into_iter()
        .take(cap)
        .map(|(_, entry)| entry)
        .collect()
}

pub(super) fn conversation_focus_summary(
    request: &CodexConversationEditRequest,
    bounded: &[&'static str],
) -> String {
    let focus = &request.focus;
    let mut lines = Vec::new();
    if let Some(primary_media_id) = &focus.primary_media_id {
        lines.push(format!("- primaryMediaId: {primary_media_id}"));
    }
    if !focus.media_ids.is_empty() {
        lines.push(format!("- mediaIds: {}", focus.media_ids.join(", ")));
    }
    if !focus.timeline_item_ids.is_empty() {
        lines.push(format!(
            "- timelineItemIds: {}",
            focus.timeline_item_ids.join(", ")
        ));
    }
    if let Some(range) = &focus.timeline_range {
        lines.push(format!(
            "- timelineRange: {:.3}-{:.3}s",
            range.start_seconds, range.end_seconds
        ));
    }
    if lines.is_empty() {
        lines.push("- none; reason over the whole project".to_string());
    }
    if bounded.is_empty() {
        lines.push("- context: complete project context".to_string());
    } else {
        lines.push(format!(
            "- context: focus-first bounded subset of {}; focused entries come first and truncation markers count omitted entries; read the split project files when an omitted entry matters",
            bounded.join(", ")
        ));
    }
    lines.join("\n")
}
