use super::media::{canonical_prefix, push_truncation_marker};
use super::timeline::{compact_context_text, context_property_label};
use super::MAX_CONTEXT_TEMPLATE_OVERRIDES;
use crate::project::model::{ProjectTemplateOverride, VideoProject};
use crate::project::storage::PROJECT_FILE_NAME;
use std::collections::BTreeMap;
use std::path::Path;

pub(super) fn project_files_summary(project: &VideoProject, project_dir: Option<&Path>) -> String {
    let mut lines = vec![
        "Inspect split project files when useful; return projectActions for mutations so Rust validates canonical state.".to_string(),
    ];

    let Some(project_dir) = project_dir else {
        lines.push("- split project directory not supplied".to_string());
        return lines.join("\n");
    };
    if project.schema_version < 2 {
        lines.push("- project is not using schema-v2 split files".to_string());
        return lines.join("\n");
    }

    lines.extend([
        format!("- root: {}", project_dir.display()),
        format!(
            "- manifest: {}",
            project_dir.join(PROJECT_FILE_NAME).display()
        ),
        format!(
            "- timeline: {}",
            project_dir.join("timeline.json").display()
        ),
        format!(
            "- media: {}",
            project_dir.join("media/index.json").display()
        ),
        format!(
            "- transcripts index: {}",
            project_dir.join("transcripts/index.json").display()
        ),
        format!(
            "- transcript sidecars: {}/<media-id>.json",
            project_dir.join("transcripts").display()
        ),
        format!(
            "- templates index: {}",
            project_dir.join("templates/index.json").display()
        ),
        format!(
            "- template sidecars: {}/<template-id>.json",
            project_dir.join("templates").display()
        ),
        format!(
            "- generated index: {}",
            project_dir.join("generated/index.json").display()
        ),
        format!(
            "- generated sidecars: {}/<asset-id>/asset.json",
            project_dir.join("generated").display()
        ),
        format!(
            "- renders index: {}",
            project_dir.join("renders/index.json").display()
        ),
        format!(
            "- render sidecars: {}/<render-id>/report.json",
            project_dir.join("renders").display()
        ),
        format!(
            "- jobs index: {}",
            project_dir.join("jobs/index.json").display()
        ),
        format!(
            "- job sidecars: {}/<job-id>/job.json",
            project_dir.join("jobs").display()
        ),
        format!(
            "- exports index: {}",
            project_dir.join("exports/index.json").display()
        ),
        format!(
            "- export sidecars: {}/<export-id>/artifact.json",
            project_dir.join("exports").display()
        ),
        format!(
            "- context summary: {}",
            project_dir.join("context/project.json").display()
        ),
        format!("- logs: {}/", project_dir.join("logs").display()),
        "- scan indexes before opening canonical sidecars; mutate canonical state only through returned projectActions.".to_string(),
        "- context/project.json is derived discovery metadata with files, indexes, counts, and latest ids; use it for orientation, not canonical mutation.".to_string(),
    ]);

    lines.join("\n")
}

pub(super) fn template_overrides_summary(project: &VideoProject) -> String {
    template_overrides_summary_for(
        project,
        &canonical_prefix(&project.template_overrides, MAX_CONTEXT_TEMPLATE_OVERRIDES),
    )
}

pub(super) fn template_overrides_summary_for(
    project: &VideoProject,
    templates: &[&ProjectTemplateOverride],
) -> String {
    let mut lines = vec![
        "Use templateId with updateTemplateOverride to revise project-level fields, style, visualTreatment, motion, safeZone, and avoid.".to_string(),
    ];

    if project.template_overrides.is_empty() {
        lines.push("- none".to_string());
        return lines.join("\n");
    }

    for template in templates {
        lines.push(template_override_summary_line(template));
    }
    push_truncation_marker(
        &mut lines,
        project.template_overrides.len(),
        templates.len(),
        "template overrides",
    );

    lines.join("\n")
}

fn template_override_summary_line(template: &ProjectTemplateOverride) -> String {
    format!(
        "- template {}: {} | fields: {} | style: {} | visualTreatment: {} | motion: {} | safeZone: {} | avoid: {}",
        template.template_id,
        empty_context_label(&template.name),
        string_map_context_label(&template.fields),
        json_map_context_label(&template.style),
        empty_context_label(&template.visual_treatment),
        empty_context_label(&template.motion),
        empty_context_label(&template.safe_zone),
        empty_context_label(&template.avoid)
    )
}

pub(super) fn empty_context_label(value: &str) -> String {
    let compact = compact_context_text(value);
    if compact.is_empty() {
        "none".to_string()
    } else {
        compact
    }
}

fn string_map_context_label(values: &BTreeMap<String, String>) -> String {
    if values.is_empty() {
        return "none".to_string();
    }

    values
        .iter()
        .map(|(key, value)| {
            format!(
                "{}={}",
                compact_context_text(key),
                compact_context_text(value)
            )
        })
        .collect::<Vec<_>>()
        .join(", ")
}

fn json_map_context_label(values: &BTreeMap<String, serde_json::Value>) -> String {
    if values.is_empty() {
        return "none".to_string();
    }

    values
        .iter()
        .map(|(key, value)| {
            format!(
                "{}={}",
                compact_context_text(key),
                context_property_label(value)
            )
        })
        .collect::<Vec<_>>()
        .join(", ")
}
