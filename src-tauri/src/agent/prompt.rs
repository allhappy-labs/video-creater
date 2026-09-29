//! The turn prompt, and the rule that the hidden context never reaches stored history.
//!
//! Both pieces are provider-neutral: every backend renders the same prompt from the same
//! context, and every backend echoes that prompt back in its transcript, so every backend has
//! to scrub it before the turn is persisted.

use crate::codex::context::CodexConversationContext;
use serde_json::Value;

/// Agent responses echo user input: `userMessage` items carry the text
/// sent with `turn/start`, and a thread's `preview` repeats its first input.
/// For conversation turns that text is the hidden adaptive context, so strip
/// it before the responses reach the client or the persisted history.
pub fn scrub_echoed_user_input(mut value: Value) -> Value {
    fn strip(value: &mut Value) {
        match value {
            Value::Array(entries) => {
                entries.retain(|entry| {
                    entry.get("type").and_then(Value::as_str) != Some("userMessage")
                });
                entries.iter_mut().for_each(strip);
            }
            Value::Object(fields) => fields.values_mut().for_each(strip),
            _ => {}
        }
    }
    strip(&mut value);
    if let Some(thread) = value.get_mut("thread").and_then(Value::as_object_mut) {
        thread.remove("preview");
    }
    value
}

pub fn render_agent_turn_prompt(context: &CodexConversationContext) -> String {
    format!(
        "\
Return only a structured project-edit proposal.
Use projectActions for edits to the existing timeline.
Include an EDL before adding a new primary video/audio cut.
Do not infer a trailer, highlight, or story preset.

Project:
- id: {project_id}
- name: {project_name}

Project files:
{project_files_summary}

User request:
{prompt}

Internal edit focus:
{internal_focus_summary}

Project library:
{media_library_summary}

Project timeline:
{timeline_summary}

Transcript excerpts:
{transcript_excerpts_summary}

Generated assets:
{generated_assets_summary}

Project template overrides:
{template_overrides_summary}

Render reports:
{render_reports_summary}

Workflow jobs:
{workflow_jobs_summary}

Export artifacts:
{export_artifacts_summary}

Export capabilities:
{export_capabilities_summary}

Proposal contract:
- summary: one plain-language sentence describing the outcome for the editor.
- edl: ordered source ranges (mediaId, sourceIn, sourceOut, reason) for a new primary video/audio cut; use [] when editing the existing timeline.
- projectActions: ordered actions Rust validates against a cloned project before anything is written; reference canonical track and item ids from the timeline above.
- renderReview: render-review criteria whose durationSeconds matches the EDL duration; required when an EDL or a new visual layer is present, otherwise null.
- Never write project files directly; Rust owns every canonical mutation.

Rules:
- Use video-creater-video-pipeline first.
- Keep the edit scoped to what the user asked for; do not rebuild the timeline unless requested.
- Ground every cut in canonical source ranges before adding captions, overlays, titles, effects, or generated visual layers.
- Use video-creater-graphics for captions, overlays, title cards, lower thirds, and HyperFrames.
- Avoid full-width opaque black caption slabs, generic text-on-box layouts, static text-only cards, and long unmoving holds.
",
        project_id = context.project_id,
        project_name = context.project_name,
        project_files_summary = context.project_files_summary,
        prompt = context.request.prompt,
        internal_focus_summary = context.internal_focus_summary,
        media_library_summary = context.media_library_summary,
        timeline_summary = context.timeline_summary,
        transcript_excerpts_summary = context.transcript_excerpts_summary,
        generated_assets_summary = context.generated_assets_summary,
        template_overrides_summary = context.template_overrides_summary,
        render_reports_summary = context.render_reports_summary,
        workflow_jobs_summary = context.workflow_jobs_summary,
        export_artifacts_summary = context.export_artifacts_summary,
        export_capabilities_summary = context.export_capabilities_summary,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn the_scrub_removes_echoed_user_input_and_the_thread_preview() {
        let scrubbed = scrub_echoed_user_input(json!({
            "thread": { "id": "thread-1", "preview": "hidden context line one" },
            "output": [
                { "type": "userMessage", "text": "hidden context line one" },
                { "type": "agentMessage", "text": "Trimmed the first clip." }
            ]
        }));

        assert_eq!(
            scrubbed,
            json!({
                "thread": { "id": "thread-1" },
                "output": [{ "type": "agentMessage", "text": "Trimmed the first clip." }]
            })
        );
    }

    #[test]
    fn the_scrub_reaches_nested_output_arrays() {
        let scrubbed = scrub_echoed_user_input(json!({
            "turn": { "items": [{ "type": "userMessage", "text": "secret" }] }
        }));

        assert_eq!(scrubbed, json!({ "turn": { "items": [] } }));
    }
}
