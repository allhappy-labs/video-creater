//! Fails closed on reversed clips that reach rendering unprepared.
//!
//! Renderers decode media forwards. Reversed clips (`properties.reverse`) play
//! only from prepared, reversed intermediates (`precompose::reverse` and
//! `precompose::reverse_audio`), and a prepared clip no longer carries
//! `reverse`. Preparation rejects reversed clips it cannot reverse, and both
//! render backends reject plan clips that still carry `reverse`.

use crate::edit::render_plan::RenderPlan;
use crate::project::model::{TimelineItem, VideoProject};
use crate::project::reverse::{is_reversed, REVERSE_PROPERTY};

use super::error::{PipelineError, PipelineErrorCode, PipelineResult};

/// Rejects a project whose enabled tracks hold reversed clips.
pub(crate) fn reject_unprepared_reversed_items(project: &VideoProject) -> PipelineResult<()> {
    for (track_index, track) in project.timeline.tracks.iter().enumerate() {
        if !track.enabled {
            continue;
        }
        if let Some((item_index, item)) = track
            .items
            .iter()
            .enumerate()
            .find(|(_, item)| is_reversed(item))
        {
            return Err(vec![unprepared_reversed_item_error(
                track_index,
                item_index,
                item,
            )]);
        }
    }
    Ok(())
}

/// The error for a reversed clip that preparation did not reverse.
pub(crate) fn unprepared_reversed_item_error(
    track_index: usize,
    item_index: usize,
    item: &TimelineItem,
) -> PipelineError {
    PipelineError::new(
        PipelineErrorCode::PipelineInputInvalid,
        format!("timeline.tracks[{track_index}].items[{item_index}].properties.{REVERSE_PROPERTY}"),
        format!(
            "Reversed clip `{}` cannot be prepared for rendering or preview.",
            item.label
        ),
        "Only video clips and audio clips play in reverse; play this clip forward, then render or preview again.",
    )
    .with_detail("itemId", item.id.clone())
}

/// Rejects a render plan with a reversed video or audio clip.
pub(crate) fn reject_reversed_plan_clips(plan: &RenderPlan) -> PipelineResult<()> {
    let reversed = |clips: &[crate::edit::render_plan::RenderClip]| {
        clips.iter().position(|clip| {
            clip.properties
                .get(REVERSE_PROPERTY)
                .and_then(serde_json::Value::as_bool)
                .unwrap_or(false)
        })
    };
    let (list, index) = match (reversed(&plan.clips), reversed(&plan.audio_clips)) {
        (Some(index), _) => ("clips", index),
        (None, Some(index)) => ("audioClips", index),
        (None, None) => return Ok(()),
    };
    Err(vec![PipelineError::new(
        PipelineErrorCode::RenderPlanInvalidClip,
        format!("renderPlan.{list}[{index}].properties.{REVERSE_PROPERTY}"),
        "Reversed clips must be prepared before rendering.",
        "Prepare reversed media before rendering, or play the clip forward.",
    )])
}
