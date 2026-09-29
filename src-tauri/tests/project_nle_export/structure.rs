//! Structure checks for exported XMEML and FCPXML that the DTDs can't express:
//! reference resolution, transition placement, storyline anchoring and media
//! handles. Each checker returns a list of problems (empty when the XML is sound).

use roxmltree::{Document, Node, ParsingOptions};
use std::cmp::Ordering;
use std::collections::BTreeSet;

const FCPXML_STORY_ELEMENTS: [&str; 4] = ["asset-clip", "gap", "title", "clip"];

fn parse(xml: &str) -> Result<Document<'_>, String> {
    Document::parse_with_options(
        xml,
        ParsingOptions {
            allow_dtd: true,
            ..ParsingOptions::default()
        },
    )
    .map_err(|error| format!("XML does not parse: {error}"))
}

fn element_children<'a, 'input>(node: Node<'a, 'input>) -> Vec<Node<'a, 'input>> {
    node.children().filter(Node::is_element).collect()
}

fn child_text<'a>(node: Node<'a, '_>, name: &str) -> Option<&'a str> {
    node.children()
        .find(|child| child.has_tag_name(name))
        .and_then(|child| child.text())
        .map(str::trim)
}

fn child_number(node: Node<'_, '_>, name: &str) -> Option<i64> {
    child_text(node, name).and_then(|text| text.parse().ok())
}

/// An exact rational time in seconds, `(numerator, denominator)` with a positive denominator.
type Time = (i128, i128);

/// Parses FCPXML times: `N/Ds`, `Ns` or `0s`.
fn parse_time(value: &str) -> Option<Time> {
    let value = value.strip_suffix('s')?;
    let (numerator, denominator) = value.split_once('/').unwrap_or((value, "1"));
    let denominator: i128 = denominator.parse().ok()?;
    (denominator > 0).then_some((numerator.parse().ok()?, denominator))
}

fn add(a: Time, b: Time) -> Time {
    (a.0 * b.1 + b.0 * a.1, a.1 * b.1)
}

fn sub(a: Time, b: Time) -> Time {
    add(a, (-b.0, b.1))
}

fn compare(a: Time, b: Time) -> Ordering {
    (a.0 * b.1).cmp(&(b.0 * a.1))
}

fn time_attribute(node: Node<'_, '_>, name: &str) -> Option<Time> {
    node.attribute(name).and_then(parse_time)
}

/// Checks exported FCPXML against the structure rules of the NLE interchange plan.
pub(super) fn check_fcpxml_structure(xml: &str) -> Vec<String> {
    let document = match parse(xml) {
        Ok(document) => document,
        Err(problem) => return vec![problem],
    };
    let mut problems = Vec::new();
    let root = document.root_element();
    if !root.has_tag_name("fcpxml") || root.attribute("version") != Some("1.10") {
        problems.push("root is not <fcpxml version=\"1.10\">".to_string());
    }
    let resources: Vec<Node> = root
        .children()
        .filter(|node| node.has_tag_name("resources"))
        .flat_map(|node| node.descendants().filter(Node::is_element))
        .collect();
    let resource_ids: BTreeSet<&str> = resources
        .iter()
        .filter_map(|node| node.attribute("id"))
        .collect();
    let style_ids: BTreeSet<&str> = document
        .descendants()
        .filter(|node| node.has_tag_name("text-style-def"))
        .filter_map(|node| node.attribute("id"))
        .collect();
    let asset = |id: &str| {
        resources
            .iter()
            .copied()
            .find(|node| node.has_tag_name("asset") && node.attribute("id") == Some(id))
    };

    for node in document.descendants().filter(Node::is_element) {
        let tag = node.tag_name().name();
        if let Some(reference) = node.attribute("ref") {
            let ids = if tag == "text-style" {
                &style_ids
            } else {
                &resource_ids
            };
            if !ids.contains(reference) {
                problems.push(format!("<{tag} ref=\"{reference}\"> does not resolve"));
            }
        }
        match tag {
            "transition" => check_fcpxml_transition(node, &asset, &mut problems),
            "spine" => check_fcpxml_spine(node, &mut problems),
            "title" => check_fcpxml_connected_title(node, &mut problems),
            _ => {}
        }
    }
    problems
}

fn check_fcpxml_spine(spine: Node<'_, '_>, problems: &mut Vec<String>) {
    let parent = spine.parent_element();
    if parent.is_some_and(|parent| parent.has_tag_name("sequence")) {
        check_fcpxml_primary_order(spine, problems);
        return;
    }
    let offset = spine.attribute("offset").unwrap_or("?");
    let Some(lane) = spine.attribute("lane") else {
        problems.push(format!("storyline at offset {offset} has no lane"));
        return;
    };
    let Some(parent) =
        parent.filter(|parent| FCPXML_STORY_ELEMENTS.contains(&parent.tag_name().name()))
    else {
        problems.push(format!(
            "storyline lane {lane} at offset {offset} is not anchored in a clip, gap or title"
        ));
        return;
    };
    check_anchor_in_parent_time(spine, parent, &format!("storyline lane {lane}"), problems);
    // A storyline has no `start`, so its sequential local time begins at 0s.
    if let Some(first) = element_children(spine).first() {
        if time_attribute(*first, "offset")
            .is_some_and(|first_offset| compare(first_offset, (0, 1)) != Ordering::Equal)
        {
            problems.push(format!(
                "storyline lane {lane} at offset {offset} does not start at 0s"
            ));
        }
    }
    for child in element_children(spine) {
        if child.attribute("lane").is_some() {
            problems.push(format!(
                "<{}> inside storyline lane {lane} carries a lane attribute",
                child.tag_name().name()
            ));
        }
    }
}

/// An anchored item's `offset` lies in its parent's local time, which starts at the parent's `start`.
fn check_anchor_in_parent_time(
    item: Node<'_, '_>,
    parent: Node<'_, '_>,
    name: &str,
    problems: &mut Vec<String>,
) {
    let parent_start = time_attribute(parent, "start").unwrap_or((0, 1));
    if let (Some(anchor), Some(parent_duration)) = (
        time_attribute(item, "offset"),
        time_attribute(parent, "duration"),
    ) {
        if compare(anchor, parent_start) == Ordering::Less
            || compare(anchor, add(parent_start, parent_duration)) != Ordering::Less
        {
            problems.push(format!(
                "{name} at offset {} is outside its parent's local time",
                item.attribute("offset").unwrap_or("?")
            ));
        }
    }
}

/// A title with a lane anchored in a story element starts inside that element's local time.
fn check_fcpxml_connected_title(title: Node<'_, '_>, problems: &mut Vec<String>) {
    let (Some(lane), Some(parent)) = (title.attribute("lane"), title.parent_element()) else {
        return;
    };
    if FCPXML_STORY_ELEMENTS.contains(&parent.tag_name().name()) {
        check_anchor_in_parent_time(title, parent, &format!("title lane {lane}"), problems);
    }
}

/// The DTD's spine holds "elements ordered serially in time". Every lane-less
/// child is a primary element: each starts no earlier than the one before it,
/// and story elements don't overlap (a transition sits over the cut it joins).
fn check_fcpxml_primary_order(spine: Node<'_, '_>, problems: &mut Vec<String>) {
    let mut previous: Option<(Time, &str)> = None;
    let mut previous_story: Option<(Time, &str)> = None;
    for child in element_children(spine) {
        let tag = child.tag_name().name();
        if child.attribute("lane").is_some() {
            continue;
        }
        let Some(offset) = time_attribute(child, "offset") else {
            continue;
        };
        let offset_label = child.attribute("offset").unwrap_or("?");
        if let Some((previous_offset, previous_tag)) = previous {
            if compare(offset, previous_offset) == Ordering::Less {
                problems.push(format!(
                    "primary <{tag}> at offset {offset_label} comes after a later <{previous_tag}>"
                ));
            }
        }
        previous = Some((offset, tag));
        if tag == "transition" {
            continue;
        }
        if let Some((previous_end, previous_tag)) = previous_story {
            if compare(offset, previous_end) == Ordering::Less {
                problems.push(format!(
                    "primary <{tag}> at offset {offset_label} overlaps the <{previous_tag}> before it"
                ));
            }
        }
        if let Some(duration) = time_attribute(child, "duration") {
            previous_story = Some((add(offset, duration), tag));
        }
    }
}

fn check_fcpxml_transition<'a, 'input>(
    transition: Node<'a, 'input>,
    asset: &dyn Fn(&str) -> Option<Node<'a, 'input>>,
    problems: &mut Vec<String>,
) {
    let offset = transition.attribute("offset").unwrap_or("?");
    let Some(spine) = transition
        .parent_element()
        .filter(|parent| parent.has_tag_name("spine"))
    else {
        problems.push(format!("transition at {offset} is not in a spine"));
        return;
    };
    let siblings = element_children(spine);
    let index = siblings
        .iter()
        .position(|sibling| *sibling == transition)
        .expect("transition is a child of its spine");
    let (Some(left), Some(right)) = (
        index.checked_sub(1).map(|left| siblings[left]),
        siblings.get(index + 1).copied(),
    ) else {
        problems.push(format!(
            "transition at {offset} is the first or last element of its spine"
        ));
        return;
    };
    for neighbor in [left, right] {
        if !FCPXML_STORY_ELEMENTS.contains(&neighbor.tag_name().name()) {
            problems.push(format!(
                "transition at {offset} neighbors <{}>, not a story element",
                neighbor.tag_name().name()
            ));
            return;
        }
    }
    let neighbor_asset = |clip: Node<'a, 'input>| {
        clip.has_tag_name("asset-clip")
            .then(|| clip.attribute("ref").and_then(asset))
            .flatten()
    };
    let audio_only = [left, right].iter().all(|clip| {
        neighbor_asset(*clip).is_some_and(|asset| asset.attribute("hasVideo") != Some("1"))
    });
    if audio_only
        && transition
            .children()
            .any(|child| child.has_tag_name("filter-video"))
    {
        problems.push(format!(
            "transition at {offset} between audio-only clips carries filter-video"
        ));
    }
    let (Some(start), Some(duration)) = (
        time_attribute(transition, "offset"),
        time_attribute(transition, "duration"),
    ) else {
        problems.push(format!("transition at {offset} has no offset or duration"));
        return;
    };
    // The transition covers the cut: the outgoing clip's end and the incoming
    // clip's offset both fall inside it.
    let end = add(start, duration);
    let outgoing_end = time_attribute(left, "offset")
        .zip(time_attribute(left, "duration"))
        .map(|(clip_offset, clip_duration)| add(clip_offset, clip_duration));
    if [outgoing_end, time_attribute(right, "offset")]
        .into_iter()
        .flatten()
        .any(|edge| {
            compare(edge, start) == Ordering::Less || compare(edge, end) == Ordering::Greater
        })
    {
        problems.push(format!(
            "transition at {offset} does not cover the cut between its clips"
        ));
    }
    let retimed = |clip: Node| clip.children().any(|child| child.has_tag_name("timeMap"));
    let clip_times = |clip: Node<'a, 'input>| {
        let asset = neighbor_asset(clip)?;
        let asset_start = time_attribute(asset, "start").unwrap_or((0, 1));
        Some((
            time_attribute(clip, "start").unwrap_or((0, 1)),
            time_attribute(clip, "offset")?,
            asset_start,
            add(asset_start, time_attribute(asset, "duration")?),
        ))
    };
    if let Some((clip_start, clip_offset, _, asset_end)) =
        clip_times(left).filter(|_| !retimed(left))
    {
        let media_end = add(clip_start, sub(add(start, duration), clip_offset));
        if compare(media_end, asset_end) == Ordering::Greater {
            problems.push(format!(
                "transition at {offset}: the outgoing clip's media ends before the transition does"
            ));
        }
    }
    if let Some((clip_start, clip_offset, asset_start, _)) =
        clip_times(right).filter(|_| !retimed(right))
    {
        let media_start = sub(clip_start, sub(clip_offset, start));
        if compare(media_start, asset_start) == Ordering::Less {
            problems.push(format!(
                "transition at {offset}: the incoming clip's media starts after the transition does"
            ));
        }
    }
}

/// Checks exported XMEML against the structure rules of the NLE interchange plan.
pub(super) fn check_xmeml_structure(xml: &str) -> Vec<String> {
    let document = match parse(xml) {
        Ok(document) => document,
        Err(problem) => return vec![problem],
    };
    let mut problems = Vec::new();
    let root = document.root_element();
    if !root.has_tag_name("xmeml") || root.attribute("version") != Some("5") {
        problems.push("root is not <xmeml version=\"5\">".to_string());
    }
    let mut clip_ids = BTreeSet::new();
    for clip in document
        .descendants()
        .filter(|node| node.has_tag_name("clipitem"))
    {
        let id = clip.attribute("id").unwrap_or("?");
        if !clip_ids.insert(id) {
            problems.push(format!("clipitem id {id} is not unique"));
        }
        let source = (child_number(clip, "in"), child_number(clip, "out"));
        let file_duration = clip
            .children()
            .find(|child| child.has_tag_name("file"))
            .and_then(|file| child_number(file, "duration"));
        match (source, file_duration) {
            ((Some(source_in), Some(source_out)), Some(duration))
                if 0 <= source_in && source_in < source_out && source_out <= duration => {}
            ((Some(source_in), Some(source_out)), None)
                if 0 <= source_in && source_in < source_out => {}
            _ => problems.push(format!(
                "clipitem {id}: in/out {source:?} is not within the file duration {file_duration:?}"
            )),
        }
    }
    for reference in document
        .descendants()
        .filter(|node| node.has_tag_name("linkclipref"))
        .filter_map(|node| node.text())
    {
        if !clip_ids.contains(reference.trim()) {
            problems.push(format!("linkclipref {reference} does not resolve"));
        }
    }
    for track in document
        .descendants()
        .filter(|node| node.has_tag_name("track"))
    {
        check_xmeml_track(track, &mut problems);
    }
    problems
}

fn check_xmeml_track(track: Node<'_, '_>, problems: &mut Vec<String>) {
    let section = track
        .ancestors()
        .find(|node| node.has_tag_name("video") || node.has_tag_name("audio"))
        .map_or("?", |node| node.tag_name().name());
    let items: Vec<Node> = element_children(track)
        .into_iter()
        .filter(|node| {
            ["clipitem", "transitionitem", "generatoritem"].contains(&node.tag_name().name())
        })
        .collect();
    let tag_at = |index: Option<usize>| {
        index
            .and_then(|index| items.get(index))
            .map(|node| node.tag_name().name())
    };
    for (index, item) in items.iter().enumerate() {
        let before = index.checked_sub(1);
        let after = Some(index + 1);
        let start = child_number(*item, "start");
        let end = child_number(*item, "end");
        if item.has_tag_name("transitionitem") {
            let clip_edge = |position: Option<usize>, edge: &str| {
                position
                    .and_then(|position| items.get(position))
                    .filter(|node| node.has_tag_name("clipitem"))
                    .and_then(|node| child_number(*node, edge))
                    == Some(-1)
            };
            if !clip_edge(before, "end") || !clip_edge(after, "start") {
                problems.push(format!(
                    "{section} transition {start:?}-{end:?} is not between a clip ending at -1 and a clip starting at -1"
                ));
            }
            if !matches!((start, end), (Some(start), Some(end)) if start < end) {
                problems.push(format!(
                    "{section} transition {start:?}-{end:?} does not start before it ends"
                ));
            }
            let effect = item.children().find(|child| child.has_tag_name("effect"));
            if effect.and_then(|effect| child_text(effect, "effecttype")) != Some("transition")
                || effect.and_then(|effect| child_text(effect, "mediatype")) != Some(section)
            {
                problems.push(format!(
                    "{section} transition {start:?}-{end:?} effect is not a {section} transition"
                ));
            }
        } else if item.has_tag_name("clipitem") {
            let id = item.attribute("id").unwrap_or("?");
            if start == Some(-1) && tag_at(before) != Some("transitionitem") {
                problems.push(format!("clipitem {id} starts at -1 without a transition"));
            }
            if end == Some(-1) && tag_at(after) != Some("transitionitem") {
                problems.push(format!("clipitem {id} ends at -1 without a transition"));
            }
        }
    }
}

#[cfg(test)]
#[path = "structure_tests.rs"]
mod tests;
