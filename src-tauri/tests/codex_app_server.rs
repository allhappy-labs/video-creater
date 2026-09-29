use serde_json::{json, Value};
use std::collections::BTreeMap;
use std::fs;
use std::path::Path;
use video_creater_lib::codex::app_server::{
    build_app_server_initialize_request, build_video_edit_turn_request, build_video_thread_request,
    codex_app_server_command, decode_app_server_response_result, start_codex_video_edit_turn,
    CodexAppServerTransport, CodexThreadAction,
};
use video_creater_lib::codex::context::{
    build_video_edit_context, build_video_edit_context_with_project_dir, load_project_skill_bundle,
    ProjectSkillBundle,
};
use video_creater_lib::codex::proposal::{
    validate_codex_edit_proposal, CodexEditProposal, CodexProposalClip, CodexProposalError,
    CodexRenderReview,
};
use video_creater_lib::codex::tools::{call_codex_local_tool, list_codex_local_tools};
use video_creater_lib::edit::preset::{CaptionStyle, EditJobRequest, EditPreset, LanguageMode};
use video_creater_lib::gpu_graphics::profile::supported_profile_ids;
use video_creater_lib::project::action::{ProjectAction, ProjectActionTranscriptWordEdit};
use video_creater_lib::project::export_profiles::{
    mp4_export_profile_availability_report, ExportProfile,
};
use video_creater_lib::project::model::{
    GeneratedAsset, GeneratedAssetOutput, GeneratedAssetReferences, GeneratedAssetSettings,
    GeneratedAssetStatus, GenerationModel, JobStatus, JobSummary, MediaAsset, MediaFolder,
    MediaKind, ProjectExportArtifact, ProjectExportArtifactKind, ProjectRenderReport,
    ProjectTemplateOverride, RenderReportCheckStatus, RenderReportStatus, RenderReportStreams,
    TemporalWorkflowMetadata, TimelineItem, TimelineItemKind, TimelineSource, Transcript,
    TranscriptWord, VideoProject,
};
use video_creater_lib::project::split::{
    load_split_project, save_split_project, ProjectValidationIssue,
};

#[path = "codex_app_server/model_fallback.rs"]
mod model_fallback;

static EXPORT_PROFILE_ENV_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());
const EXPORT_RUNTIME_ENV_VARS: &[&str] = &[
    "VIDEO_CREATER_APPROVED_H264_ENCODER",
    "VIDEO_CREATER_APPROVED_H265_ENCODER",
    "VIDEO_CREATER_APPROVED_PRORES_ENCODER",
    "VIDEO_CREATER_APPROVED_AAC_ENCODER",
    "VIDEO_CREATER_APPROVED_AUDIO_ENCODER",
];

#[test]
fn codex_app_server_command_uses_stdio_transport() {
    let command = codex_app_server_command("codex");

    assert_eq!(command.program, "codex");
    assert_eq!(command.args, vec!["app-server", "--stdio"]);
}

#[test]
fn initialize_request_opts_into_experimental_app_server_api() {
    let request = build_app_server_initialize_request(1);

    assert_eq!(request["id"], json!(1));
    assert_eq!(request["method"], json!("initialize"));
    assert_eq!(
        request["params"]["clientInfo"]["name"],
        json!("video-creater")
    );
    assert_eq!(
        request["params"]["capabilities"]["experimentalApi"],
        json!(true)
    );
}

#[test]
fn thread_start_request_injects_project_skills_and_workspace() {
    let bundle = sample_skill_bundle();
    let request = build_video_thread_request(
        7,
        CodexThreadAction::Start,
        "/Users/olhapi/Documents/video-creater",
        &bundle,
    );

    assert_eq!(request["id"], json!(7));
    assert_eq!(request["method"], json!("thread/start"));
    assert_eq!(
        request["params"]["cwd"],
        json!("/Users/olhapi/Documents/video-creater")
    );
    // Proposal threads only return structured proposals that Rust validates;
    // Codex must never write project files or prompt for approvals.
    assert_eq!(request["params"]["sandbox"], json!("read-only"));
    assert_eq!(request["params"]["approvalPolicy"], json!("never"));
    let developer_instructions = request["params"]["developerInstructions"]
        .as_str()
        .expect("developer instructions");
    assert!(developer_instructions.contains("Video Creater Skill Policy"));
    assert!(developer_instructions.contains("video-creater-video-pipeline"));
    assert!(developer_instructions.contains("EDL-first"));
    assert!(developer_instructions.contains("shader-background-template-catalog"));
    assert!(developer_instructions.contains("shadertoy-octagrams-v1"));
}

#[test]
fn thread_resume_request_preserves_existing_project_thread_id() {
    let bundle = sample_skill_bundle();
    let request = build_video_thread_request(
        8,
        CodexThreadAction::Resume {
            thread_id: "thread-123".to_string(),
        },
        "/Users/olhapi/Documents/video-creater",
        &bundle,
    );

    assert_eq!(request["method"], json!("thread/resume"));
    assert_eq!(request["params"]["threadId"], json!("thread-123"));
    assert!(request["params"]["developerInstructions"]
        .as_str()
        .expect("developer instructions")
        .contains("App-Server Video Generation Contract"));
}

#[test]
fn turn_request_contains_bounded_context_and_structured_output_schema() {
    let project = sample_project();
    let request = sample_edit_request();
    let context = build_video_edit_context(&project, &request).expect("context");

    let turn = build_video_edit_turn_request(9, "thread-123", &context);

    assert_eq!(turn["method"], json!("turn/start"));
    assert_eq!(turn["params"]["threadId"], json!("thread-123"));
    assert_eq!(turn["params"]["input"][0]["type"], json!("text"));
    let text = turn["params"]["input"][0]["text"].as_str().expect("text");
    assert!(text.contains("Return only a structured Codex edit proposal"));
    assert!(text.contains("media-1"));
    assert!(text.contains("TrailerCut"));
    assert!(text.contains("Transcript excerpt"));
    assert!(text.contains("Visual quality guardrails"));
    assert!(text.contains("Avoid full-width opaque black caption slabs"));
    assert!(text.contains("phone-size legible"));
    assert!(text.contains("ProjectAction workflow"));
    assert!(text.contains("projectActions"));
    assert!(text.contains("applyCaptionRepair"));
    assert!(text.contains("updateTemplateOverride"));
    assert!(
        text.contains("updateAudioVolume, updateAudioClipSpeed, detachAudio, updateClipReverse,")
    );
    assert!(
        text.contains("updateTemplateOverride, addTransition, updateTransition, removeTransition.")
    );
    assert!(
        text.contains("crossfade, dipToBlack, dipToWhite, or wipe transitions centered on the cut")
    );
    assert!(text.contains("recordJob"));
    assert!(text.contains("updateJobStatus"));
    assert!(text.contains("recordExportArtifact"));
    assert!(text.contains("workflow-backed render, generation, transcription, or Codex jobs"));
    assert!(text.contains("replaceTimelineItemWithGeneratedOutput"));
    assert!(text.contains("editTextItem"));
    assert!(text.contains("updateTextOverlayItems"));
    assert!(text.contains("setTrackLocked"));
    assert!(text.contains("setTrackEnabled"));
    assert!(text.contains("updateAudioFades"));
    assert!(text.contains("updateAudioFadeOut"));
    assert!(text.contains("updateAudioVolume"));
    assert!(text.contains(
        "updateTextOverlayItems for manual text overlay timing, copy, visualTreatment, motion, safeZone, and avoid edits"
    ));
    assert!(text.contains("Use [] when no direct project-file action is needed"));
    assert_eq!(
        turn["params"]["outputSchema"]["required"],
        json!([
            "mediaId",
            "clips",
            "captions",
            "overlays",
            "hyperframes",
            "gpuVisuals",
            "projectActions",
            "renderReview"
        ])
    );
    assert_eq!(
        turn["params"]["outputSchema"]["properties"]["captions"]["items"]["additionalProperties"],
        json!(false)
    );
    assert_eq!(
        turn["params"]["outputSchema"]["properties"]["mediaId"],
        json!({ "type": "string", "minLength": 1 })
    );
    assert_eq!(
        turn["params"]["outputSchema"]["properties"]["clips"]["items"]["properties"]["sourceOut"],
        json!({ "type": "number", "exclusiveMinimum": 0 })
    );
    assert_eq!(
        turn["params"]["outputSchema"]["properties"]["clips"]["items"]["properties"]["mediaId"],
        json!({ "type": "string", "minLength": 1 })
    );
    assert!(
        turn["params"]["outputSchema"]["properties"]["clips"]["items"]["required"]
            .as_array()
            .expect("clip required fields")
            .contains(&json!("mediaId"))
    );
    assert_eq!(
        turn["params"]["outputSchema"]["properties"]["captions"]["items"]["required"],
        json!([
            "text",
            "startSeconds",
            "durationSeconds",
            "sourceBeat",
            "dimensions",
            "frameRate",
            "alpha",
            "visualTreatment",
            "motion",
            "safeZone",
            "avoid"
        ])
    );
    assert_eq!(
        turn["params"]["outputSchema"]["properties"]["captions"]["items"]["properties"]
            ["durationSeconds"],
        json!({ "type": "number", "minimum": 0.001 })
    );
    assert_eq!(
        turn["params"]["outputSchema"]["properties"]["overlays"]["items"]["required"],
        json!([
            "kind",
            "templateId",
            "fields",
            "motionPresetId",
            "nodes",
            "sourceBeat",
            "startSeconds",
            "durationSeconds",
            "dimensions",
            "frameRate",
            "alpha",
            "brief",
            "visualTreatment",
            "motion",
            "safeZone",
            "avoid"
        ])
    );
    assert_eq!(
        turn["params"]["outputSchema"]["properties"]["hyperframes"]["items"]["required"],
        json!([
            "kind",
            "templateId",
            "fields",
            "motionPresetId",
            "sourceBeat",
            "startSeconds",
            "durationSeconds",
            "dimensions",
            "frameRate",
            "alpha",
            "brief",
            "visualTreatment",
            "motion",
            "safeZone",
            "avoid"
        ])
    );
    assert_eq!(
        turn["params"]["outputSchema"]["properties"]["overlays"]["items"]["additionalProperties"],
        json!(false)
    );
    assert_eq!(
        turn["params"]["outputSchema"]["properties"]["overlays"]["items"]["properties"]
            ["durationSeconds"],
        json!({ "type": "number", "minimum": 0.001 })
    );
    assert_eq!(
        turn["params"]["outputSchema"]["properties"]["overlays"]["items"]["properties"]["kind"],
        json!({
            "type": "string",
            "enum": ["overlay", "lower_third", "title_card", "diagram", "transition"]
        })
    );
    assert_eq!(
        turn["params"]["outputSchema"]["properties"]["hyperframes"]["items"]
            ["additionalProperties"],
        json!(false)
    );
    assert_eq!(
        turn["params"]["outputSchema"]["properties"]["hyperframes"]["items"]["properties"]
            ["durationSeconds"],
        json!({ "type": "number", "minimum": 0.001 })
    );
    assert_eq!(
        turn["params"]["outputSchema"]["properties"]["hyperframes"]["items"]["properties"]["kind"],
        json!({
            "type": "string",
            "enum": [
                "template_overlay",
                "title_card",
                "lower_third",
                "diagram",
                "transition",
                "immersive_scene"
            ]
        })
    );
    assert_eq!(
        turn["params"]["outputSchema"]["properties"]["renderReview"]["properties"]
            ["durationSeconds"],
        json!({ "type": "number", "minimum": 0.001 })
    );
    assert_eq!(
        turn["params"]["outputSchema"]["properties"]["renderReview"]["required"],
        json!([
            "durationSeconds",
            "streamCheckRequired",
            "captionAlignmentRequired",
            "overlayTimingRequired",
            "visualFrameEvidenceRequired",
            "artifactPathsRequired",
            "logReferenceRequired"
        ])
    );
    assert_eq!(
        turn["params"]["outputSchema"]["properties"]["renderReview"]["properties"]
            ["visualFrameEvidenceRequired"],
        json!({ "type": "boolean" })
    );
    assert_eq!(
        turn["params"]["outputSchema"]["properties"]["projectActions"]["type"],
        json!("array")
    );
    let project_action_schema =
        &turn["params"]["outputSchema"]["properties"]["projectActions"]["items"];
    let project_action_variants = project_action_schema["anyOf"]
        .as_array()
        .expect("project action anyOf variants");
    let mut project_action_types: Vec<&str> = Vec::new();
    for variant in project_action_variants {
        let variant_types = variant["properties"]["type"]["enum"]
            .as_array()
            .expect("project action type enum");
        project_action_types.extend(
            variant_types
                .iter()
                .map(|action_type| action_type.as_str().expect("project action type string")),
        );
    }
    project_action_types.sort_unstable();
    let mut expected_project_action_types = vec![
        "addItems",
        "insertItems",
        "removeItems",
        "moveItems",
        "reorderItems",
        "resizeItems",
        "trimItems",
        "rippleDeleteRanges",
        "splitItems",
        "setTrackLocked",
        "setTrackSyncLocked",
        "setTrackEnabled",
        "editCaptionText",
        "editTextItem",
        "updateAudioFades",
        "updateAudioFadeOut",
        "updateAudioVolume",
        "updateAudioClipSpeed",
        "detachAudio",
        "updateClipReverse",
        "updateVisualClipOpacity",
        "updateVisualClipTransform",
        "updateVisualClipCrop",
        "setItemKeyframes",
        "updateItemEffects",
        "updateItemColorGrade",
        "updateTextOverlayItems",
        "applyCaptionRepair",
        "editTranscriptWords",
        "recordGeneratedAsset",
        "updateGeneratedAssetStatus",
        "updateGeneratedAssetReferences",
        "completeGeneratedAsset",
        "replaceTimelineItemWithGeneratedOutput",
        "assignMediaFolder",
        "createMediaFolder",
        "createTrack",
        "renameMediaFolder",
        "deleteMediaFolder",
        "renameMedia",
        "deleteMedia",
        "removeTracks",
        "updateRenderSettings",
        "recordJob",
        "updateJobStatus",
        "attachRenderReport",
        "recordExportArtifact",
        "updateTemplateItems",
        "updateTemplateOverride",
        "addTransition",
        "updateTransition",
        "removeTransition",
    ];
    expected_project_action_types.sort_unstable();
    assert_eq!(project_action_types, expected_project_action_types);

    let update_audio_clip_speed_schema = project_action_variants
        .iter()
        .find(|variant| variant["properties"]["type"]["enum"] == json!(["updateAudioClipSpeed"]))
        .expect("updateAudioClipSpeed action schema");
    assert_eq!(
        update_audio_clip_speed_schema["required"],
        json!(["type", "itemId", "speed"])
    );
    assert_eq!(
        update_audio_clip_speed_schema["properties"]["speed"],
        json!({ "type": "number", "minimum": 0.1, "maximum": 8 })
    );
    assert_eq!(
        update_audio_clip_speed_schema["additionalProperties"],
        json!(false)
    );

    let detach_audio_schema = project_action_variants
        .iter()
        .find(|variant| variant["properties"]["type"]["enum"] == json!(["detachAudio"]))
        .expect("detachAudio action schema");
    assert_eq!(
        detach_audio_schema["required"],
        json!([
            "type",
            "itemId",
            "audioItemId",
            "targetTrackId",
            "linkGroupId"
        ])
    );
    assert_eq!(detach_audio_schema["additionalProperties"], json!(false));

    let update_clip_reverse_schema = project_action_variants
        .iter()
        .find(|variant| variant["properties"]["type"]["enum"] == json!(["updateClipReverse"]))
        .expect("updateClipReverse action schema");
    assert_eq!(
        update_clip_reverse_schema["required"],
        json!(["type", "itemId", "reverse"])
    );
    assert_eq!(
        update_clip_reverse_schema["properties"]["reverse"],
        json!({ "type": "boolean" })
    );
    assert_eq!(
        update_clip_reverse_schema["additionalProperties"],
        json!(false)
    );

    let update_visual_transform_schema = project_action_variants
        .iter()
        .find(|variant| {
            variant["properties"]["type"]["enum"] == json!(["updateVisualClipTransform"])
        })
        .expect("updateVisualClipTransform action schema");
    assert_eq!(
        update_visual_transform_schema["properties"]["transform"]["properties"]["centerX"],
        json!({ "type": ["number", "null"], "minimum": 0, "maximum": 1 })
    );

    let update_visual_crop_schema = project_action_variants
        .iter()
        .find(|variant| variant["properties"]["type"]["enum"] == json!(["updateVisualClipCrop"]))
        .expect("updateVisualClipCrop action schema");
    assert_eq!(
        update_visual_crop_schema["properties"]["crop"]["properties"]["cropTop"],
        json!({ "type": ["number", "null"], "minimum": 0, "exclusiveMaximum": 1 })
    );

    let edit_text_item_schema = project_action_variants
        .iter()
        .find(|variant| variant["properties"]["type"]["enum"] == json!(["editTextItem"]))
        .expect("editTextItem action schema");
    assert_eq!(edit_text_item_schema["additionalProperties"], json!(false));
    assert_eq!(
        edit_text_item_schema["required"],
        json!(["type", "itemId", "text"])
    );
    assert_eq!(
        edit_text_item_schema["properties"]["itemId"],
        json!({ "type": "string", "minLength": 1 })
    );
    assert_eq!(
        edit_text_item_schema["properties"]["text"],
        json!({ "type": "string", "minLength": 1 })
    );

    let update_text_overlay_items_schema = project_action_variants
        .iter()
        .find(|variant| variant["properties"]["type"]["enum"] == json!(["updateTextOverlayItems"]))
        .expect("updateTextOverlayItems action schema");
    assert_eq!(
        update_text_overlay_items_schema["additionalProperties"],
        json!(false)
    );
    assert_eq!(
        update_text_overlay_items_schema["required"],
        json!(["type", "updates"])
    );
    assert_eq!(
        update_text_overlay_items_schema["properties"]["updates"]["minItems"],
        json!(1)
    );
    assert_eq!(
        update_text_overlay_items_schema["properties"]["updates"]["items"]["additionalProperties"],
        json!(false)
    );
    assert_eq!(
        update_text_overlay_items_schema["properties"]["updates"]["items"]["required"],
        json!([
            "itemId",
            "startSeconds",
            "durationSeconds",
            "text",
            "visualTreatment",
            "motion",
            "safeZone",
            "avoid",
            "fontName",
            "fontSize",
            "color",
            "alignment"
        ])
    );
    assert_eq!(
        update_text_overlay_items_schema["properties"]["updates"]["items"]["properties"]
            ["durationSeconds"],
        json!({ "type": "number", "exclusiveMinimum": 0 })
    );
    assert_eq!(
        update_text_overlay_items_schema["properties"]["updates"]["items"]["properties"]
            ["fontName"],
        json!({ "type": ["string", "null"], "minLength": 1 })
    );

    let update_template_items_schema = project_action_variants
        .iter()
        .find(|variant| variant["properties"]["type"]["enum"] == json!(["updateTemplateItems"]))
        .expect("updateTemplateItems action schema");
    assert_eq!(
        update_template_items_schema["additionalProperties"],
        json!(false)
    );
    assert_eq!(
        update_template_items_schema["required"],
        json!(["type", "updates"])
    );
    assert_eq!(
        update_template_items_schema["properties"]["updates"]["minItems"],
        json!(1)
    );
    let template_update_schema = &update_template_items_schema["properties"]["updates"]["items"];
    assert_eq!(template_update_schema["additionalProperties"], json!(false));
    assert_eq!(
        template_update_schema["required"],
        json!([
            "itemId",
            "startSeconds",
            "durationSeconds",
            "templateFields"
        ])
    );
    assert_eq!(
        template_update_schema["properties"]["itemId"],
        json!({ "type": "string", "minLength": 1 })
    );
    assert_eq!(
        template_update_schema["properties"]["startSeconds"],
        json!({ "type": "number", "minimum": 0 })
    );
    assert_eq!(
        template_update_schema["properties"]["durationSeconds"],
        json!({ "type": "number", "exclusiveMinimum": 0 })
    );
    assert_eq!(
        template_update_schema["properties"]["templateFields"],
        json!({
            "type": "object",
            "required": ["headline", "subline", "logoAssetId"],
            "additionalProperties": false,
            "properties": {
                "headline": { "type": ["string", "null"] },
                "subline": { "type": ["string", "null"] },
                "logoAssetId": { "type": ["string", "null"] }
            }
        })
    );

    let update_template_override_schema = project_action_variants
        .iter()
        .find(|variant| variant["properties"]["type"]["enum"] == json!(["updateTemplateOverride"]))
        .expect("updateTemplateOverride action schema");
    assert_eq!(
        update_template_override_schema["additionalProperties"],
        json!(false)
    );
    assert_eq!(
        update_template_override_schema["required"],
        json!(["type", "override"])
    );
    let template_override_schema = &update_template_override_schema["properties"]["override"];
    assert_eq!(
        template_override_schema["additionalProperties"],
        json!(false)
    );
    assert_eq!(
        template_override_schema["required"],
        json!([
            "templateId",
            "name",
            "fields",
            "style",
            "visualTreatment",
            "motion",
            "safeZone",
            "avoid"
        ])
    );
    assert_eq!(
        template_override_schema["properties"]["templateId"],
        json!({ "type": "string", "minLength": 1 })
    );
    assert_eq!(
        template_override_schema["properties"]["name"],
        json!({ "type": "string", "minLength": 1 })
    );
    assert_eq!(
        template_override_schema["properties"]["fields"],
        json!({
            "type": "object",
            "required": ["headline", "subline", "logoAssetId"],
            "additionalProperties": false,
            "properties": {
                "headline": { "type": ["string", "null"] },
                "subline": { "type": ["string", "null"] },
                "logoAssetId": { "type": ["string", "null"] }
            }
        })
    );
    assert_eq!(
        template_override_schema["properties"]["style"],
        json!({
            "type": "object",
            "required": ["accentColor", "backgroundColor"],
            "additionalProperties": false,
            "properties": {
                "accentColor": { "type": ["string", "null"] },
                "backgroundColor": { "type": ["string", "null"] }
            }
        })
    );
    assert_eq!(
        template_override_schema["properties"]["visualTreatment"],
        json!({ "type": "string", "minLength": 1 })
    );
    assert_eq!(
        template_override_schema["properties"]["motion"],
        json!({ "type": "string", "minLength": 1 })
    );
    assert_eq!(
        template_override_schema["properties"]["safeZone"],
        json!({ "type": "string", "minLength": 1 })
    );
    assert_eq!(
        template_override_schema["properties"]["avoid"],
        json!({ "type": "string", "minLength": 1 })
    );

    let create_track_schema = project_action_variants
        .iter()
        .find(|variant| variant["properties"]["type"]["enum"] == json!(["createTrack"]))
        .expect("createTrack action schema");
    assert_eq!(create_track_schema["additionalProperties"], json!(false));
    assert_eq!(
        create_track_schema["required"],
        json!(["type", "track", "afterTrackId"])
    );
    assert_eq!(
        create_track_schema["properties"]["afterTrackId"],
        json!({
            "anyOf": [
                { "type": "string", "minLength": 1 },
                { "type": "null" }
            ]
        })
    );
    let track_schema = &create_track_schema["properties"]["track"];
    assert_eq!(track_schema["additionalProperties"], json!(false));
    assert_eq!(
        track_schema["required"],
        json!(["id", "name", "kind", "locked", "enabled", "items"])
    );
    assert_eq!(
        track_schema["properties"]["kind"],
        json!({
            "type": "string",
            "enum": ["video", "hyperframe_scene", "overlay", "caption", "audio"]
        })
    );
    assert_eq!(
        track_schema["properties"]["items"],
        json!({
            "type": "array",
            "maxItems": 0,
            "items": {
                "type": "object",
                "required": [],
                "additionalProperties": false,
                "properties": {}
            }
        })
    );

    let split_items_schema = project_action_variants
        .iter()
        .find(|variant| variant["properties"]["type"]["enum"] == json!(["splitItems"]))
        .expect("splitItems action schema");
    assert_eq!(split_items_schema["additionalProperties"], json!(false));
    assert_eq!(split_items_schema["required"], json!(["type", "splits"]));
    let split_schema = &split_items_schema["properties"]["splits"]["items"];
    assert_eq!(
        split_items_schema["properties"]["splits"]["minItems"],
        json!(1)
    );
    assert_eq!(split_schema["additionalProperties"], json!(false));
    assert_eq!(
        split_schema["required"],
        json!(["itemId", "newItemId", "splitSeconds"])
    );
    assert_eq!(
        split_schema["properties"]["itemId"],
        json!({ "type": "string", "minLength": 1 })
    );
    assert_eq!(
        split_schema["properties"]["newItemId"],
        json!({ "type": "string", "minLength": 1 })
    );
    assert_eq!(
        split_schema["properties"]["splitSeconds"],
        json!({ "type": "number", "exclusiveMinimum": 0 })
    );

    let trim_items_schema = project_action_variants
        .iter()
        .find(|variant| variant["properties"]["type"]["enum"] == json!(["trimItems"]))
        .expect("trimItems action schema");
    assert_eq!(trim_items_schema["additionalProperties"], json!(false));
    assert_eq!(trim_items_schema["required"], json!(["type", "trims"]));
    assert_eq!(
        trim_items_schema["properties"]["trims"]["minItems"],
        json!(1)
    );
    let trim_schema = &trim_items_schema["properties"]["trims"]["items"];
    assert_eq!(trim_schema["additionalProperties"], json!(false));
    assert_eq!(
        trim_schema["required"],
        json!([
            "itemId",
            "startSeconds",
            "durationSeconds",
            "sourceIn",
            "sourceOut"
        ])
    );
    assert_eq!(
        trim_schema["properties"]["itemId"],
        json!({ "type": "string", "minLength": 1 })
    );
    assert_eq!(
        trim_schema["properties"]["startSeconds"],
        json!({ "type": "number", "minimum": 0 })
    );
    assert_eq!(
        trim_schema["properties"]["durationSeconds"],
        json!({ "type": "number", "exclusiveMinimum": 0 })
    );
    assert_eq!(
        trim_schema["properties"]["sourceIn"],
        json!({
            "anyOf": [
                { "type": "number", "minimum": 0 },
                { "type": "null" }
            ]
        })
    );
    assert_eq!(
        trim_schema["properties"]["sourceOut"],
        json!({
            "anyOf": [
                { "type": "number", "exclusiveMinimum": 0 },
                { "type": "null" }
            ]
        })
    );

    let reorder_items_schema = project_action_variants
        .iter()
        .find(|variant| variant["properties"]["type"]["enum"] == json!(["reorderItems"]))
        .expect("reorderItems action schema");
    assert_eq!(reorder_items_schema["additionalProperties"], json!(false));
    assert_eq!(reorder_items_schema["required"], json!(["type", "reorder"]));
    let reorder_schema = &reorder_items_schema["properties"]["reorder"];
    assert_eq!(reorder_schema["additionalProperties"], json!(false));
    assert_eq!(
        reorder_schema["required"],
        json!(["targetTrackId", "itemIds", "startSeconds", "gapSeconds"])
    );
    assert_eq!(
        reorder_schema["properties"]["targetTrackId"],
        json!({ "type": "string", "minLength": 1 })
    );
    assert_eq!(
        reorder_schema["properties"]["itemIds"]["minItems"],
        json!(1)
    );
    assert_eq!(
        reorder_schema["properties"]["itemIds"]["items"],
        json!({ "type": "string", "minLength": 1 })
    );
    assert_eq!(
        reorder_schema["properties"]["startSeconds"],
        json!({ "type": "number", "minimum": 0 })
    );
    assert_eq!(
        reorder_schema["properties"]["gapSeconds"],
        json!({ "type": "number", "minimum": 0 })
    );

    let move_items_schema = project_action_variants
        .iter()
        .find(|variant| variant["properties"]["type"]["enum"] == json!(["moveItems"]))
        .expect("moveItems action schema");
    assert_eq!(move_items_schema["additionalProperties"], json!(false));
    assert_eq!(move_items_schema["required"], json!(["type", "moves"]));
    assert_eq!(
        move_items_schema["properties"]["moves"]["minItems"],
        json!(1)
    );
    let move_schema = &move_items_schema["properties"]["moves"]["items"];
    assert_eq!(move_schema["additionalProperties"], json!(false));
    assert_eq!(
        move_schema["required"],
        json!(["itemId", "targetTrackId", "startSeconds"])
    );
    assert_eq!(
        move_schema["properties"]["itemId"],
        json!({ "type": "string", "minLength": 1 })
    );
    assert_eq!(
        move_schema["properties"]["targetTrackId"],
        json!({ "type": "string", "minLength": 1 })
    );
    assert_eq!(
        move_schema["properties"]["startSeconds"],
        json!({ "type": "number", "minimum": 0 })
    );

    let resize_items_schema = project_action_variants
        .iter()
        .find(|variant| variant["properties"]["type"]["enum"] == json!(["resizeItems"]))
        .expect("resizeItems action schema");
    assert_eq!(resize_items_schema["additionalProperties"], json!(false));
    assert_eq!(resize_items_schema["required"], json!(["type", "resizes"]));
    assert_eq!(
        resize_items_schema["properties"]["resizes"]["minItems"],
        json!(1)
    );
    let resize_schema = &resize_items_schema["properties"]["resizes"]["items"];
    assert_eq!(resize_schema["additionalProperties"], json!(false));
    assert_eq!(
        resize_schema["required"],
        json!(["itemId", "durationSeconds"])
    );
    assert_eq!(
        resize_schema["properties"]["itemId"],
        json!({ "type": "string", "minLength": 1 })
    );
    assert_eq!(
        resize_schema["properties"]["durationSeconds"],
        json!({ "type": "number", "exclusiveMinimum": 0 })
    );

    let remove_items_schema = project_action_variants
        .iter()
        .find(|variant| variant["properties"]["type"]["enum"] == json!(["removeItems"]))
        .expect("removeItems action schema");
    assert_eq!(remove_items_schema["additionalProperties"], json!(false));
    assert_eq!(remove_items_schema["required"], json!(["type", "itemIds"]));
    assert_eq!(
        remove_items_schema["properties"]["itemIds"]["minItems"],
        json!(1)
    );
    assert_eq!(
        remove_items_schema["properties"]["itemIds"]["items"],
        json!({ "type": "string", "minLength": 1 })
    );

    let insert_items_schema = project_action_variants
        .iter()
        .find(|variant| variant["properties"]["type"]["enum"] == json!(["insertItems"]))
        .expect("insertItems action schema");
    assert_eq!(insert_items_schema["additionalProperties"], json!(false));
    assert_eq!(
        insert_items_schema["required"],
        json!(["type", "targetTrackId", "insertSeconds", "items"])
    );
    assert_eq!(
        insert_items_schema["properties"]["targetTrackId"],
        json!({ "type": "string", "minLength": 1 })
    );
    assert_eq!(
        insert_items_schema["properties"]["insertSeconds"],
        json!({ "type": "number", "minimum": 0 })
    );
    assert_eq!(
        insert_items_schema["properties"]["items"]["minItems"],
        json!(1)
    );
    let insert_timeline_item_schema = &insert_items_schema["properties"]["items"]["items"];
    assert_eq!(insert_timeline_item_schema["type"], json!("object"));
    assert_eq!(
        insert_timeline_item_schema["additionalProperties"],
        json!(false)
    );
    assert_eq!(
        insert_timeline_item_schema["required"],
        json!([
            "id",
            "kind",
            "startSeconds",
            "durationSeconds",
            "source",
            "label",
            "properties"
        ])
    );
    assert_eq!(
        insert_timeline_item_schema["properties"]["id"],
        json!({ "type": "string", "minLength": 1 })
    );
    assert_eq!(
        insert_timeline_item_schema["properties"]["kind"],
        json!({
            "type": "string",
            "enum": ["video_clip", "hyperframe_scene", "overlay", "caption", "audio_clip"]
        })
    );
    assert_eq!(
        insert_timeline_item_schema["properties"]["startSeconds"],
        json!({ "type": "number", "minimum": 0 })
    );
    assert_eq!(
        insert_timeline_item_schema["properties"]["durationSeconds"],
        json!({ "type": "number", "exclusiveMinimum": 0 })
    );
    assert_eq!(
        insert_timeline_item_schema["properties"]["label"],
        json!({ "type": "string", "minLength": 1 })
    );
    let item_properties_schema = &insert_timeline_item_schema["properties"]["properties"];
    assert_eq!(item_properties_schema["type"], json!("object"));
    assert_eq!(item_properties_schema["additionalProperties"], json!(false));
    assert!(item_properties_schema["required"]
        .as_array()
        .expect("timeline item properties required")
        .contains(&json!("sourceIn")));
    assert_eq!(
        item_properties_schema["properties"]["sourceIn"],
        json!({
            "anyOf": [
                { "type": "number", "minimum": 0 },
                { "type": "null" }
            ]
        })
    );
    assert_eq!(
        insert_timeline_item_schema["properties"]["source"],
        json!({
            "anyOf": [
                {
                    "type": "object",
                    "required": ["type", "mediaId"],
                    "additionalProperties": false,
                    "properties": {
                        "type": { "type": "string", "enum": ["media"] },
                        "mediaId": { "type": "string", "minLength": 1 }
                    }
                },
                {
                    "type": "object",
                    "required": ["type", "artifactId"],
                    "additionalProperties": false,
                    "properties": {
                        "type": { "type": "string", "enum": ["generated"] },
                        "artifactId": { "type": "string", "minLength": 1 }
                    }
                },
                {
                    "type": "object",
                    "required": ["type", "text"],
                    "additionalProperties": false,
                    "properties": {
                        "type": { "type": "string", "enum": ["text"] },
                        "text": { "type": "string", "minLength": 1 }
                    }
                }
            ]
        })
    );

    let add_items_schema = project_action_variants
        .iter()
        .find(|variant| variant["properties"]["type"]["enum"] == json!(["addItems"]))
        .expect("addItems action schema");
    assert_eq!(add_items_schema["additionalProperties"], json!(false));
    assert_eq!(
        add_items_schema["required"],
        json!(["type", "targetTrackId", "items"])
    );
    assert_eq!(
        add_items_schema["properties"]["targetTrackId"],
        json!({ "type": "string", "minLength": 1 })
    );
    assert_eq!(
        add_items_schema["properties"]["items"]["minItems"],
        json!(1)
    );
    assert_eq!(
        add_items_schema["properties"]["items"]["items"],
        *insert_timeline_item_schema
    );

    let set_track_locked_schema = project_action_variants
        .iter()
        .find(|variant| variant["properties"]["type"]["enum"] == json!(["setTrackLocked"]))
        .expect("setTrackLocked action schema");
    assert_eq!(
        set_track_locked_schema["additionalProperties"],
        json!(false)
    );
    assert_eq!(
        set_track_locked_schema["required"],
        json!(["type", "trackId", "locked"])
    );
    assert_eq!(
        set_track_locked_schema["properties"]["trackId"],
        json!({ "type": "string", "minLength": 1 })
    );
    assert_eq!(
        set_track_locked_schema["properties"]["locked"],
        json!({ "type": "boolean" })
    );

    let set_track_enabled_schema = project_action_variants
        .iter()
        .find(|variant| variant["properties"]["type"]["enum"] == json!(["setTrackEnabled"]))
        .expect("setTrackEnabled action schema");
    assert_eq!(
        set_track_enabled_schema["additionalProperties"],
        json!(false)
    );
    assert_eq!(
        set_track_enabled_schema["required"],
        json!(["type", "trackId", "enabled"])
    );
    assert_eq!(
        set_track_enabled_schema["properties"]["trackId"],
        json!({ "type": "string", "minLength": 1 })
    );
    assert_eq!(
        set_track_enabled_schema["properties"]["enabled"],
        json!({ "type": "boolean" })
    );

    let update_generated_asset_status_schema = project_action_variants
        .iter()
        .find(|variant| {
            variant["properties"]["type"]["enum"] == json!(["updateGeneratedAssetStatus"])
        })
        .expect("updateGeneratedAssetStatus action schema");
    assert_eq!(
        update_generated_asset_status_schema["additionalProperties"],
        json!(false)
    );
    assert_eq!(
        update_generated_asset_status_schema["required"],
        json!(["type", "assetId", "status"])
    );
    assert_eq!(
        update_generated_asset_status_schema["properties"]["assetId"],
        json!({ "type": "string", "minLength": 1 })
    );
    assert_eq!(
        update_generated_asset_status_schema["properties"]["status"],
        json!({
            "type": "string",
            "enum": ["queued", "running", "failed", "completed"]
        })
    );

    let update_generated_asset_references_schema = project_action_variants
        .iter()
        .find(|variant| {
            variant["properties"]["type"]["enum"] == json!(["updateGeneratedAssetReferences"])
        })
        .expect("updateGeneratedAssetReferences action schema");
    assert_eq!(
        update_generated_asset_references_schema["additionalProperties"],
        json!(false)
    );
    assert_eq!(
        update_generated_asset_references_schema["required"],
        json!(["type", "assetId", "references"])
    );
    assert_eq!(
        update_generated_asset_references_schema["properties"]["assetId"],
        json!({ "type": "string", "minLength": 1 })
    );
    assert_eq!(
        update_generated_asset_references_schema["properties"]["references"]["properties"]
            ["providerInputUrls"],
        json!({
            "type": "array",
            "items": { "type": "string", "minLength": 1 }
        })
    );
    assert_eq!(
        update_generated_asset_references_schema["properties"]["references"]["properties"]
            ["referenceVideoMediaRefs"],
        json!({
            "type": "array",
            "items": { "type": "string", "minLength": 1 }
        })
    );

    let record_generated_asset_schema = project_action_variants
        .iter()
        .find(|variant| variant["properties"]["type"]["enum"] == json!(["recordGeneratedAsset"]))
        .expect("recordGeneratedAsset action schema");
    assert_eq!(
        record_generated_asset_schema["additionalProperties"],
        json!(false)
    );
    assert_eq!(
        record_generated_asset_schema["required"],
        json!(["type", "asset"])
    );
    let generated_asset_schema = &record_generated_asset_schema["properties"]["asset"];
    assert_eq!(generated_asset_schema["additionalProperties"], json!(false));
    assert_eq!(
        generated_asset_schema["required"],
        json!([
            "id",
            "kind",
            "status",
            "prompt",
            "model",
            "references",
            "settings",
            "outputs",
            "createdAt",
            "name",
            "targetFolderId",
            "placementIntent",
            "parentAssetId",
            "retryOfAssetId"
        ])
    );
    assert_eq!(
        generated_asset_schema["properties"]["id"],
        json!({ "type": "string", "minLength": 1 })
    );
    assert_eq!(
        generated_asset_schema["properties"]["kind"],
        json!({ "type": "string", "enum": ["generated"] })
    );
    assert_eq!(
        generated_asset_schema["properties"]["status"],
        json!({
            "type": "string",
            "enum": ["queued", "running", "failed", "completed"]
        })
    );
    assert_eq!(
        generated_asset_schema["properties"]["name"],
        json!({
            "anyOf": [
                { "type": "string", "minLength": 1 },
                { "type": "null" }
            ]
        })
    );
    assert_eq!(
        generated_asset_schema["properties"]["targetFolderId"],
        json!({
            "anyOf": [
                { "type": "string", "minLength": 1 },
                { "type": "null" }
            ]
        })
    );
    assert_eq!(
        generated_asset_schema["properties"]["placementIntent"],
        json!({
            "anyOf": [
                { "type": "string", "enum": ["library", "timeline"] },
                { "type": "null" }
            ]
        })
    );
    assert_eq!(
        generated_asset_schema["properties"]["prompt"],
        json!({ "type": "string" })
    );
    assert_eq!(
        generated_asset_schema["properties"]["model"],
        json!({
            "type": "object",
            "required": ["provider", "id"],
            "additionalProperties": false,
            "properties": {
                "provider": { "type": "string", "minLength": 1 },
                "id": { "type": "string", "minLength": 1 }
            }
        })
    );
    assert_eq!(
        generated_asset_schema["properties"]["references"],
        json!({
            "type": "object",
            "required": [
                "mediaIds",
                "sourceVideoMediaRef",
                "firstFrameMediaId",
                "lastFrameMediaId",
                "referenceImageMediaRefs",
                "referenceVideoMediaRefs",
                "referenceAudioMediaRefs",
                "providerInputUrls"
            ],
            "additionalProperties": false,
            "properties": {
                "mediaIds": {
                    "type": "array",
                    "items": { "type": "string", "minLength": 1 }
                },
                "sourceVideoMediaRef": {
                    "anyOf": [
                        { "type": "string", "minLength": 1 },
                        { "type": "null" }
                    ]
                },
                "firstFrameMediaId": {
                    "anyOf": [
                        { "type": "string", "minLength": 1 },
                        { "type": "null" }
                    ]
                },
                "lastFrameMediaId": {
                    "anyOf": [
                        { "type": "string", "minLength": 1 },
                        { "type": "null" }
                    ]
                },
                "referenceImageMediaRefs": {
                    "type": "array",
                    "items": { "type": "string", "minLength": 1 }
                },
                "referenceVideoMediaRefs": {
                    "type": "array",
                    "items": { "type": "string", "minLength": 1 }
                },
                "referenceAudioMediaRefs": {
                    "type": "array",
                    "items": { "type": "string", "minLength": 1 }
                },
                "providerInputUrls": {
                    "type": "array",
                    "items": { "type": "string", "minLength": 1 }
                }
            }
        })
    );
    assert_eq!(
        generated_asset_schema["properties"]["settings"],
        json!({
            "type": "object",
            "required": ["width", "height", "durationSeconds", "fps", "aspectRatio"],
            "additionalProperties": false,
            "properties": {
                "width": {
                    "anyOf": [
                        { "type": "integer", "minimum": 1 },
                        { "type": "null" }
                    ]
                },
                "height": {
                    "anyOf": [
                        { "type": "integer", "minimum": 1 },
                        { "type": "null" }
                    ]
                },
                "durationSeconds": {
                    "anyOf": [
                        { "type": "number", "exclusiveMinimum": 0 },
                        { "type": "null" }
                    ]
                },
                "fps": {
                    "anyOf": [
                        { "type": "number", "exclusiveMinimum": 0 },
                        { "type": "null" }
                    ]
                },
                "aspectRatio": {
                    "anyOf": [
                        { "type": "string", "minLength": 1 },
                        { "type": "null" }
                    ]
                }
            }
        })
    );
    assert_eq!(
        generated_asset_schema["properties"]["outputs"],
        json!({
            "type": "array",
            "items": {
                "type": "object",
                "required": ["mediaId", "relativePath", "width", "height", "durationSeconds", "fps"],
                "additionalProperties": false,
                "properties": {
                    "mediaId": { "type": "string", "minLength": 1 },
                    "relativePath": { "type": "string", "minLength": 1 },
                    "width": { "type": "integer", "minimum": 1 },
                    "height": { "type": "integer", "minimum": 1 },
                    "durationSeconds": { "type": "number", "exclusiveMinimum": 0 },
                    "fps": { "type": "number", "exclusiveMinimum": 0 }
                }
            }
        })
    );
    assert_eq!(
        generated_asset_schema["properties"]["createdAt"],
        json!({ "type": "string", "minLength": 1 })
    );
    assert_eq!(
        generated_asset_schema["properties"]["parentAssetId"],
        json!({
            "anyOf": [
                { "type": "string", "minLength": 1 },
                { "type": "null" }
            ]
        })
    );
    assert_eq!(
        generated_asset_schema["properties"]["retryOfAssetId"],
        json!({
            "anyOf": [
                { "type": "string", "minLength": 1 },
                { "type": "null" }
            ]
        })
    );

    let complete_generated_asset_schema = project_action_variants
        .iter()
        .find(|variant| variant["properties"]["type"]["enum"] == json!(["completeGeneratedAsset"]))
        .expect("completeGeneratedAsset action schema");
    assert_eq!(
        complete_generated_asset_schema["additionalProperties"],
        json!(false)
    );
    assert_eq!(
        complete_generated_asset_schema["required"],
        json!(["type", "assetId", "outputs", "replacement"])
    );
    assert_eq!(
        complete_generated_asset_schema["properties"]["assetId"],
        json!({ "type": "string", "minLength": 1 })
    );
    assert_eq!(
        complete_generated_asset_schema["properties"]["outputs"],
        json!({
            "type": "array",
            "minItems": 1,
            "items": {
                "type": "object",
                "required": ["mediaId", "relativePath", "width", "height", "durationSeconds", "fps"],
                "additionalProperties": false,
                "properties": {
                    "mediaId": { "type": "string", "minLength": 1 },
                    "relativePath": { "type": "string", "minLength": 1 },
                    "width": { "type": "integer", "minimum": 1 },
                    "height": { "type": "integer", "minimum": 1 },
                    "durationSeconds": { "type": "number", "exclusiveMinimum": 0 },
                    "fps": { "type": "number", "exclusiveMinimum": 0 }
                }
            }
        })
    );
    assert_eq!(
        complete_generated_asset_schema["properties"]["replacement"],
        json!({
            "anyOf": [
                {
                    "type": "object",
                    "required": ["itemId", "mediaId"],
                    "additionalProperties": false,
                    "properties": {
                        "itemId": { "type": "string", "minLength": 1 },
                        "mediaId": { "type": "string", "minLength": 1 }
                    }
                },
                { "type": "null" }
            ]
        })
    );

    let replace_generated_output_schema = project_action_variants
        .iter()
        .find(|variant| {
            variant["properties"]["type"]["enum"]
                == json!(["replaceTimelineItemWithGeneratedOutput"])
        })
        .expect("replaceTimelineItemWithGeneratedOutput action schema");
    assert_eq!(
        replace_generated_output_schema["additionalProperties"],
        json!(false)
    );
    assert_eq!(
        replace_generated_output_schema["required"],
        json!(["type", "replacement"])
    );
    assert_eq!(
        replace_generated_output_schema["properties"]["replacement"],
        json!({
            "type": "object",
            "required": ["itemId", "mediaId"],
            "additionalProperties": false,
            "properties": {
                "itemId": { "type": "string", "minLength": 1 },
                "mediaId": { "type": "string", "minLength": 1 }
            }
        })
    );

    let edit_caption_text_schema = project_action_variants
        .iter()
        .find(|variant| variant["properties"]["type"]["enum"] == json!(["editCaptionText"]))
        .expect("editCaptionText action schema");
    assert_eq!(
        edit_caption_text_schema["additionalProperties"],
        json!(false)
    );
    assert_eq!(
        edit_caption_text_schema["required"],
        json!(["type", "itemId", "text"])
    );
    assert_eq!(
        edit_caption_text_schema["properties"]["itemId"],
        json!({ "type": "string", "minLength": 1 })
    );
    assert_eq!(
        edit_caption_text_schema["properties"]["text"],
        json!({ "type": "string", "minLength": 1 })
    );

    let update_audio_fade_out_schema = project_action_variants
        .iter()
        .find(|variant| variant["properties"]["type"]["enum"] == json!(["updateAudioFadeOut"]))
        .expect("updateAudioFadeOut action schema");
    assert_eq!(
        update_audio_fade_out_schema["additionalProperties"],
        json!(false)
    );
    assert_eq!(
        update_audio_fade_out_schema["required"],
        json!(["type", "itemId", "fadeOutSeconds"])
    );
    assert_eq!(
        update_audio_fade_out_schema["properties"]["itemId"],
        json!({ "type": "string", "minLength": 1 })
    );
    assert_eq!(
        update_audio_fade_out_schema["properties"]["fadeOutSeconds"],
        json!({ "type": "number", "minimum": 0 })
    );

    let update_audio_fades_schema = project_action_variants
        .iter()
        .find(|variant| variant["properties"]["type"]["enum"] == json!(["updateAudioFades"]))
        .expect("updateAudioFades action schema");
    assert_eq!(
        update_audio_fades_schema["required"],
        json!(["type", "itemId", "fadeInSeconds", "fadeOutSeconds"])
    );

    let update_audio_volume_schema = project_action_variants
        .iter()
        .find(|variant| variant["properties"]["type"]["enum"] == json!(["updateAudioVolume"]))
        .expect("updateAudioVolume action schema");
    assert_eq!(
        update_audio_volume_schema["additionalProperties"],
        json!(false)
    );
    assert_eq!(
        update_audio_volume_schema["required"],
        json!(["type", "itemId", "volumeDb"])
    );
    assert_eq!(
        update_audio_volume_schema["properties"]["itemId"],
        json!({ "type": "string", "minLength": 1 })
    );
    assert_eq!(
        update_audio_volume_schema["properties"]["volumeDb"],
        json!({
            "anyOf": [
                { "type": "number", "minimum": -60, "maximum": 24 },
                { "type": "null" }
            ]
        })
    );

    let update_item_effects_schema = project_action_variants
        .iter()
        .find(|variant| variant["properties"]["type"]["enum"] == json!(["updateItemEffects"]))
        .expect("updateItemEffects action schema");
    assert_eq!(
        update_item_effects_schema["additionalProperties"],
        json!(false)
    );
    assert_eq!(
        update_item_effects_schema["required"],
        json!(["type", "itemIds", "effects"])
    );
    assert_eq!(
        update_item_effects_schema["properties"]["itemIds"],
        json!({
            "type": "array",
            "minItems": 1,
            "items": { "type": "string", "minLength": 1 }
        })
    );
    assert_eq!(
        update_item_effects_schema["properties"]["effects"]["items"]["required"],
        json!(["effectType", "enabled", "params"])
    );
    assert_eq!(
        update_item_effects_schema["properties"]["effects"]["items"]["properties"]["params"],
        json!({
            "type": "object",
            "required": [],
            "additionalProperties": false,
            "properties": {}
        })
    );

    let set_item_keyframes_schema = project_action_variants
        .iter()
        .find(|variant| variant["properties"]["type"]["enum"] == json!(["setItemKeyframes"]))
        .expect("setItemKeyframes action schema");
    assert_eq!(
        set_item_keyframes_schema["additionalProperties"],
        json!(false)
    );
    assert_eq!(
        set_item_keyframes_schema["required"],
        json!(["type", "itemId", "property", "keyframes"])
    );
    assert_eq!(
        set_item_keyframes_schema["properties"]["property"],
        json!({
            "type": "string",
            "enum": [
                "opacity",
                "volumeDb",
                "positionX",
                "positionY",
                "scale",
                "scaleX",
                "scaleY",
                "rotationDegrees",
                "cropTop",
                "cropRight",
                "cropBottom",
                "cropLeft"
            ]
        })
    );
    assert_eq!(
        set_item_keyframes_schema["properties"]["keyframes"]["items"]["required"],
        json!(["atSeconds", "value", "easing"])
    );

    let update_item_color_grade_schema = project_action_variants
        .iter()
        .find(|variant| variant["properties"]["type"]["enum"] == json!(["updateItemColorGrade"]))
        .expect("updateItemColorGrade action schema");
    assert_eq!(
        update_item_color_grade_schema["additionalProperties"],
        json!(false)
    );
    assert_eq!(
        update_item_color_grade_schema["required"],
        json!(["type", "itemIds", "reset", "grade"])
    );
    assert_eq!(
        update_item_color_grade_schema["properties"]["grade"]["properties"]["exposure"],
        json!({
            "anyOf": [
                { "type": "number", "minimum": -3, "maximum": 3 },
                { "type": "null" }
            ]
        })
    );
    assert_eq!(
        update_item_color_grade_schema["properties"]["grade"]["required"],
        // Keep the schema contract in the editor's processing order so agents emit
        // primary balance, tonal ranges, wheels, curves, then LUT controls.
        json!([
            "exposure",
            "contrast",
            "saturation",
            "temperature",
            "tint",
            "vibrance",
            "highlights",
            "shadows",
            "blacks",
            "whites",
            "shadowsHue",
            "shadowsAmount",
            "shadowsLum",
            "midsHue",
            "midsAmount",
            "midsGamma",
            "highsHue",
            "highsAmount",
            "highsGain",
            "masterCurve",
            "redCurve",
            "greenCurve",
            "blueCurve",
            "hueCurves",
            "lut"
        ])
    );
    assert_eq!(
        update_item_color_grade_schema["properties"]["grade"]["properties"]["vibrance"],
        json!({
            "anyOf": [
                { "type": "number", "minimum": -1, "maximum": 1 },
                { "type": "null" }
            ]
        })
    );
    assert_eq!(
        update_item_color_grade_schema["properties"]["grade"]["properties"]["hueCurves"],
        json!({
            "anyOf": [
                {
                    "type": "object",
                    "required": ["targets"],
                    "additionalProperties": false,
                    "properties": {
                        "targets": {
                            "type": "array",
                            "minItems": 1,
                            "items": {
                                "type": "object",
                                "required": ["targetHue", "hueShift", "satScale", "lumShift"],
                                "additionalProperties": false,
                                "properties": {
                                    "targetHue": {
                                        "type": "number",
                                        "minimum": 0,
                                        "maximum": 360
                                    },
                                    "hueShift": {
                                        "anyOf": [
                                            { "type": "number", "minimum": -30, "maximum": 30 },
                                            { "type": "null" }
                                        ]
                                    },
                                    "satScale": {
                                        "anyOf": [
                                            { "type": "number", "minimum": 0, "maximum": 2 },
                                            { "type": "null" }
                                        ]
                                    },
                                    "lumShift": {
                                        "anyOf": [
                                            { "type": "number", "minimum": -0.5, "maximum": 0.5 },
                                            { "type": "null" }
                                        ]
                                    }
                                }
                            }
                        }
                    }
                },
                { "type": "null" }
            ]
        })
    );
    assert_eq!(
        update_item_color_grade_schema["properties"]["grade"]["properties"]["lut"],
        json!({
            "anyOf": [
                {
                    "type": "object",
                    "required": ["path", "strength"],
                    "additionalProperties": false,
                    "properties": {
                        "path": {
                            "anyOf": [
                                { "type": "string", "minLength": 1 },
                                { "type": "null" }
                            ]
                        },
                        "strength": {
                            "anyOf": [
                                { "type": "number", "minimum": 0, "maximum": 1 },
                                { "type": "null" }
                            ]
                        }
                    }
                },
                { "type": "null" }
            ]
        })
    );

    let apply_caption_repair_schema = project_action_variants
        .iter()
        .find(|variant| variant["properties"]["type"]["enum"] == json!(["applyCaptionRepair"]))
        .expect("applyCaptionRepair action schema");
    assert_eq!(
        apply_caption_repair_schema["additionalProperties"],
        json!(false)
    );
    assert_eq!(
        apply_caption_repair_schema["required"],
        json!(["type", "repair"])
    );
    let caption_repair_schema = &apply_caption_repair_schema["properties"]["repair"];
    assert_eq!(caption_repair_schema["additionalProperties"], json!(false));
    assert_eq!(
        caption_repair_schema["required"],
        json!([
            "captionItemId",
            "transcriptId",
            "wordIndex",
            "text",
            "startSeconds",
            "endSeconds",
            "repairId",
            "createdAt"
        ])
    );
    assert_eq!(
        caption_repair_schema["properties"]["captionItemId"],
        json!({ "type": "string", "minLength": 1 })
    );
    assert_eq!(
        caption_repair_schema["properties"]["transcriptId"],
        json!({ "type": "string", "minLength": 1 })
    );
    assert_eq!(
        caption_repair_schema["properties"]["wordIndex"],
        json!({ "type": "integer", "minimum": 0 })
    );
    assert_eq!(
        caption_repair_schema["properties"]["text"],
        json!({ "type": "string", "minLength": 1 })
    );
    assert_eq!(
        caption_repair_schema["properties"]["startSeconds"],
        json!({ "type": "number", "minimum": 0 })
    );
    assert_eq!(
        caption_repair_schema["properties"]["endSeconds"],
        json!({ "type": "number", "exclusiveMinimum": 0 })
    );
    assert_eq!(
        caption_repair_schema["properties"]["repairId"],
        json!({ "type": "string", "minLength": 1 })
    );
    assert_eq!(
        caption_repair_schema["properties"]["createdAt"],
        json!({ "type": "string", "minLength": 1 })
    );

    let edit_transcript_words_schema = project_action_variants
        .iter()
        .find(|variant| variant["properties"]["type"]["enum"] == json!(["editTranscriptWords"]))
        .expect("editTranscriptWords action schema");
    assert_eq!(
        edit_transcript_words_schema["additionalProperties"],
        json!(false)
    );
    assert_eq!(
        edit_transcript_words_schema["required"],
        json!(["type", "edits"])
    );
    assert_eq!(
        edit_transcript_words_schema["properties"]["edits"]["minItems"],
        json!(1)
    );
    let transcript_word_edit_schema = &edit_transcript_words_schema["properties"]["edits"]["items"];
    assert_eq!(
        transcript_word_edit_schema["additionalProperties"],
        json!(false)
    );
    assert_eq!(
        transcript_word_edit_schema["required"],
        json!([
            "transcriptId",
            "wordIndex",
            "text",
            "startSeconds",
            "endSeconds",
            "repairId",
            "createdAt"
        ])
    );
    assert_eq!(
        transcript_word_edit_schema["properties"]["transcriptId"],
        json!({ "type": "string", "minLength": 1 })
    );
    assert_eq!(
        transcript_word_edit_schema["properties"]["wordIndex"],
        json!({ "type": "integer", "minimum": 0 })
    );
    assert_eq!(
        transcript_word_edit_schema["properties"]["text"],
        json!({
            "anyOf": [
                { "type": "string", "minLength": 1 },
                { "type": "null" }
            ]
        })
    );
    assert_eq!(
        transcript_word_edit_schema["properties"]["startSeconds"],
        json!({
            "anyOf": [
                { "type": "number", "minimum": 0 },
                { "type": "null" }
            ]
        })
    );
    assert_eq!(
        transcript_word_edit_schema["properties"]["endSeconds"],
        json!({
            "anyOf": [
                { "type": "number", "exclusiveMinimum": 0 },
                { "type": "null" }
            ]
        })
    );
    assert_eq!(
        transcript_word_edit_schema["properties"]["repairId"],
        json!({ "type": "string", "minLength": 1 })
    );
    assert_eq!(
        transcript_word_edit_schema["properties"]["createdAt"],
        json!({ "type": "string", "minLength": 1 })
    );

    let update_job_status_schema = project_action_variants
        .iter()
        .find(|variant| variant["properties"]["type"]["enum"] == json!(["updateJobStatus"]))
        .expect("updateJobStatus action schema");
    assert_eq!(
        update_job_status_schema["additionalProperties"],
        json!(false)
    );
    assert_eq!(
        update_job_status_schema["required"],
        json!(["type", "jobId", "status", "updatedAt", "runId"])
    );
    assert_eq!(
        update_job_status_schema["properties"]["jobId"],
        json!({ "type": "string", "minLength": 1 })
    );
    assert_eq!(
        update_job_status_schema["properties"]["status"],
        json!({
            "type": "string",
            "enum": ["queued", "running", "progress", "blocked", "failed", "cancelled", "completed"]
        })
    );
    assert_eq!(
        update_job_status_schema["properties"]["updatedAt"],
        json!({ "type": "string", "minLength": 1 })
    );
    assert_eq!(
        update_job_status_schema["properties"]["runId"],
        json!({
            "anyOf": [
                { "type": "string", "minLength": 1 },
                { "type": "null" }
            ]
        })
    );

    let record_job_schema = project_action_variants
        .iter()
        .find(|variant| variant["properties"]["type"]["enum"] == json!(["recordJob"]))
        .expect("recordJob action schema");
    assert_eq!(record_job_schema["additionalProperties"], json!(false));
    assert_eq!(record_job_schema["required"], json!(["type", "job"]));
    let job_schema = &record_job_schema["properties"]["job"];
    assert_eq!(job_schema["additionalProperties"], json!(false));
    assert_eq!(
        job_schema["required"],
        json!([
            "id",
            "kind",
            "status",
            "updatedAt",
            "workflow",
            "startRequest"
        ])
    );
    assert_eq!(
        job_schema["properties"]["id"],
        json!({ "type": "string", "minLength": 1 })
    );
    assert_eq!(
        job_schema["properties"]["kind"],
        json!({
            "type": "string",
            "enum": [
                "generate_media",
                "render_draft",
                "transcribe_media",
                "codex_edit",
                "export_media",
                "export_nle_xml"
            ]
        })
    );
    assert_eq!(
        job_schema["properties"]["status"],
        json!({
            "type": "string",
            "enum": ["queued", "running", "progress", "blocked", "failed", "cancelled", "completed"]
        })
    );
    assert_eq!(
        job_schema["properties"]["updatedAt"],
        json!({ "type": "string", "minLength": 1 })
    );
    assert_eq!(
        job_schema["properties"]["workflow"],
        json!({
            "anyOf": [
                {
                    "type": "object",
                    "required": ["workflowId", "workflowType", "taskQueue", "runId", "activityTypes"],
                    "additionalProperties": false,
                    "properties": {
                        "workflowId": { "type": "string", "minLength": 1 },
                        "workflowType": {
                            "type": "string",
                            "enum": [
                                "VideoCreaterGenerateMediaWorkflow",
                                "VideoCreaterRenderDraftWorkflow",
                                "VideoCreaterTranscribeMediaWorkflow",
                                "VideoCreaterCodexEditWorkflow",
                                "VideoCreaterExportMediaWorkflow",
                                "VideoCreaterExportNleXmlWorkflow"
                            ]
                        },
                        "taskQueue": {
                            "type": "string",
                            "enum": ["video-creater-workflows"]
                        },
                        "runId": {
                            "anyOf": [
                                { "type": "string", "minLength": 1 },
                                { "type": "null" }
                            ]
                        },
                        "activityTypes": {
                            "type": "array",
                            "minItems": 1,
                            "items": { "type": "string", "minLength": 1 }
                        }
                    }
                },
                { "type": "null" }
            ]
        })
    );
    assert_eq!(
        job_schema["properties"]["startRequest"],
        json!({
            "anyOf": [
                {
                    "type": "object",
                    "required": [
                        "workflowId",
                        "workflowType",
                        "taskQueue",
                        "input",
                        "searchAttributes",
                        "activityTypes",
                        "idReusePolicy"
                    ],
                    "additionalProperties": false,
                    "properties": {
                        "workflowId": { "type": "string", "minLength": 1 },
                        "workflowType": {
                            "type": "string",
                            "enum": [
                                "VideoCreaterGenerateMediaWorkflow",
                                "VideoCreaterRenderDraftWorkflow",
                                "VideoCreaterTranscribeMediaWorkflow",
                                "VideoCreaterCodexEditWorkflow",
                                "VideoCreaterExportMediaWorkflow",
                                "VideoCreaterExportNleXmlWorkflow"
                            ]
                        },
                        "taskQueue": {
                            "type": "string",
                            "enum": ["video-creater-workflows"]
                        },
                        "input": {
                            "type": "object",
                            "required": [],
                            "additionalProperties": false,
                            "properties": {}
                        },
                        "searchAttributes": {
                            "type": "object",
                            "required": [],
                            "additionalProperties": false,
                            "properties": {}
                        },
                        "activityTypes": {
                            "type": "array",
                            "minItems": 1,
                            "items": { "type": "string", "minLength": 1 }
                        },
                        "idReusePolicy": {
                            "type": "string",
                            "enum": ["rejectDuplicate"]
                        }
                    }
                },
                { "type": "null" }
            ]
        })
    );

    let attach_render_report_schema = project_action_variants
        .iter()
        .find(|variant| variant["properties"]["type"]["enum"] == json!(["attachRenderReport"]))
        .expect("attachRenderReport action schema");
    assert_eq!(
        attach_render_report_schema["additionalProperties"],
        json!(false)
    );
    assert_eq!(
        attach_render_report_schema["required"],
        json!(["type", "report"])
    );
    let render_report_schema = &attach_render_report_schema["properties"]["report"];
    assert_eq!(render_report_schema["additionalProperties"], json!(false));
    assert_eq!(
        render_report_schema["required"],
        json!([
            "schemaVersion",
            "id",
            "status",
            "outputPath",
            "durationSeconds",
            "streams",
            "checks",
            "artifacts",
            "logPath",
            "createdAt"
        ])
    );
    assert_eq!(
        render_report_schema["properties"]["schemaVersion"],
        json!({ "type": "integer", "minimum": 1 })
    );
    assert_eq!(
        render_report_schema["properties"]["id"],
        json!({ "type": "string", "minLength": 1 })
    );
    assert_eq!(
        render_report_schema["properties"]["status"],
        json!({
            "type": "string",
            "enum": ["queued", "running", "failed", "completed"]
        })
    );
    assert_eq!(
        render_report_schema["properties"]["outputPath"],
        json!({ "type": "string", "minLength": 1 })
    );
    assert_eq!(
        render_report_schema["properties"]["durationSeconds"],
        json!({ "type": "number", "exclusiveMinimum": 0 })
    );
    assert_eq!(
        render_report_schema["properties"]["streams"],
        json!({
            "type": "object",
            "required": ["video", "audio"],
            "additionalProperties": false,
            "properties": {
                "video": { "type": "boolean" },
                "audio": { "type": "boolean" }
            }
        })
    );
    assert_eq!(
        render_report_schema["properties"]["checks"],
        json!({
            "type": "object",
            "required": [
                "duration",
                "captionAlignment",
                "overlayTiming",
                "visualFrameEvidence",
                "artifactPaths"
            ],
            "additionalProperties": false,
            "properties": {
                "duration": {
                    "type": "string",
                    "enum": ["passed", "failed", "skipped"]
                },
                "captionAlignment": {
                    "type": "string",
                    "enum": ["passed", "failed", "skipped"]
                },
                "overlayTiming": {
                    "type": "string",
                    "enum": ["passed", "failed", "skipped"]
                },
                "visualFrameEvidence": {
                    "type": "string",
                    "enum": ["passed", "failed", "skipped"]
                },
                "artifactPaths": {
                    "type": "string",
                    "enum": ["passed", "failed", "skipped"]
                }
            }
        })
    );
    assert_eq!(
        render_report_schema["properties"]["artifacts"],
        json!({
            "type": "array",
            "minItems": 1,
            "items": { "type": "string", "minLength": 1 }
        })
    );
    assert_eq!(
        render_report_schema["properties"]["logPath"],
        json!({ "type": "string", "minLength": 1 })
    );
    assert_eq!(
        render_report_schema["properties"]["createdAt"],
        json!({ "type": "string", "minLength": 1 })
    );

    let record_export_artifact_schema = project_action_variants
        .iter()
        .find(|variant| variant["properties"]["type"]["enum"] == json!(["recordExportArtifact"]))
        .expect("recordExportArtifact action schema");
    assert_eq!(
        record_export_artifact_schema["additionalProperties"],
        json!(false)
    );
    assert_eq!(
        record_export_artifact_schema["required"],
        json!(["type", "artifact"])
    );
    let export_artifact_schema = &record_export_artifact_schema["properties"]["artifact"];
    assert_eq!(export_artifact_schema["additionalProperties"], json!(false));
    assert_eq!(
        export_artifact_schema["required"],
        json!([
            "schemaVersion",
            "id",
            "kind",
            "format",
            "path",
            "mimeType",
            "jobId",
            "createdAt"
        ])
    );
    assert_eq!(
        export_artifact_schema["properties"]["schemaVersion"],
        json!({ "type": "integer", "minimum": 1 })
    );
    assert_eq!(
        export_artifact_schema["properties"]["id"],
        json!({ "type": "string", "minLength": 1 })
    );
    assert_eq!(
        export_artifact_schema["properties"]["kind"],
        json!({
            "type": "string",
            "enum": ["nle_xml", "webm", "mp4", "mov"]
        })
    );
    assert_eq!(
        export_artifact_schema["properties"]["format"],
        json!({ "type": "string", "minLength": 1 })
    );
    assert_eq!(
        export_artifact_schema["properties"]["path"],
        json!({ "type": "string", "minLength": 1 })
    );
    assert_eq!(
        export_artifact_schema["properties"]["mimeType"],
        json!({ "type": "string", "minLength": 1 })
    );
    assert_eq!(
        export_artifact_schema["properties"]["jobId"],
        json!({
            "anyOf": [
                { "type": "string", "minLength": 1 },
                { "type": "null" }
            ]
        })
    );
    assert_eq!(
        export_artifact_schema["properties"]["createdAt"],
        json!({ "type": "string", "minLength": 1 })
    );

    let assign_media_folder_schema = project_action_variants
        .iter()
        .find(|variant| variant["properties"]["type"]["enum"] == json!(["assignMediaFolder"]))
        .expect("assignMediaFolder action schema");
    assert_eq!(
        assign_media_folder_schema["additionalProperties"],
        json!(false)
    );
    assert_eq!(
        assign_media_folder_schema["required"],
        json!(["type", "mediaId", "folderId"])
    );
    assert_eq!(
        assign_media_folder_schema["properties"]["mediaId"],
        json!({ "type": "string", "minLength": 1 })
    );
    assert_eq!(
        assign_media_folder_schema["properties"]["folderId"],
        json!({
            "anyOf": [
                { "type": "string", "minLength": 1 },
                { "type": "null" }
            ]
        })
    );

    let create_media_folder_schema = project_action_variants
        .iter()
        .find(|variant| variant["properties"]["type"]["enum"] == json!(["createMediaFolder"]))
        .expect("createMediaFolder action schema");
    assert_eq!(
        create_media_folder_schema["additionalProperties"],
        json!(false)
    );
    assert_eq!(
        create_media_folder_schema["required"],
        json!(["type", "folder"])
    );
    let media_folder_schema = &create_media_folder_schema["properties"]["folder"];
    assert_eq!(media_folder_schema["additionalProperties"], json!(false));
    assert_eq!(
        media_folder_schema["required"],
        json!(["id", "name", "parentId"])
    );
    assert_eq!(
        media_folder_schema["properties"]["id"],
        json!({ "type": "string", "minLength": 1 })
    );
    assert_eq!(
        media_folder_schema["properties"]["name"],
        json!({ "type": "string", "minLength": 1 })
    );
    assert_eq!(
        media_folder_schema["properties"]["parentId"],
        json!({
            "anyOf": [
                { "type": "string", "minLength": 1 },
                { "type": "null" }
            ]
        })
    );

    let rename_media_folder_schema = project_action_variants
        .iter()
        .find(|variant| variant["properties"]["type"]["enum"] == json!(["renameMediaFolder"]))
        .expect("renameMediaFolder action schema");
    assert_eq!(
        rename_media_folder_schema["additionalProperties"],
        json!(false)
    );
    assert_eq!(
        rename_media_folder_schema["required"],
        json!(["type", "folderId", "name"])
    );
    assert_eq!(
        rename_media_folder_schema["properties"]["folderId"],
        json!({ "type": "string", "minLength": 1 })
    );
    assert_eq!(
        rename_media_folder_schema["properties"]["name"],
        json!({ "type": "string", "minLength": 1 })
    );

    let delete_media_folder_schema = project_action_variants
        .iter()
        .find(|variant| variant["properties"]["type"]["enum"] == json!(["deleteMediaFolder"]))
        .expect("deleteMediaFolder action schema");
    assert_eq!(
        delete_media_folder_schema["additionalProperties"],
        json!(false)
    );
    assert_eq!(
        delete_media_folder_schema["required"],
        json!(["type", "folderId"])
    );
    assert_eq!(
        delete_media_folder_schema["properties"]["folderId"],
        json!({ "type": "string", "minLength": 1 })
    );

    let rename_media_schema = project_action_variants
        .iter()
        .find(|variant| variant["properties"]["type"]["enum"] == json!(["renameMedia"]))
        .expect("renameMedia action schema");
    assert_eq!(rename_media_schema["additionalProperties"], json!(false));
    assert_eq!(
        rename_media_schema["required"],
        json!(["type", "mediaId", "name"])
    );
    assert_eq!(
        rename_media_schema["properties"]["mediaId"],
        json!({ "type": "string", "minLength": 1 })
    );
    assert_eq!(
        rename_media_schema["properties"]["name"],
        json!({ "type": "string", "minLength": 1 })
    );

    let delete_media_schema = project_action_variants
        .iter()
        .find(|variant| variant["properties"]["type"]["enum"] == json!(["deleteMedia"]))
        .expect("deleteMedia action schema");
    assert_eq!(delete_media_schema["additionalProperties"], json!(false));
    assert_eq!(delete_media_schema["required"], json!(["type", "mediaIds"]));
    assert_eq!(
        delete_media_schema["properties"]["mediaIds"],
        json!({
            "type": "array",
            "minItems": 1,
            "items": { "type": "string", "minLength": 1 }
        })
    );

    let remove_tracks_schema = project_action_variants
        .iter()
        .find(|variant| variant["properties"]["type"]["enum"] == json!(["removeTracks"]))
        .expect("removeTracks action schema");
    assert_eq!(remove_tracks_schema["additionalProperties"], json!(false));
    assert_eq!(
        remove_tracks_schema["required"],
        json!(["type", "trackIds"])
    );
    assert_eq!(
        remove_tracks_schema["properties"]["trackIds"],
        json!({
            "type": "array",
            "minItems": 1,
            "items": { "type": "string", "minLength": 1 }
        })
    );

    let update_render_settings_schema = project_action_variants
        .iter()
        .find(|variant| variant["properties"]["type"]["enum"] == json!(["updateRenderSettings"]))
        .expect("updateRenderSettings action schema");
    assert_eq!(
        update_render_settings_schema["additionalProperties"],
        json!(false)
    );
    assert_eq!(
        update_render_settings_schema["required"],
        json!(["type", "settings"])
    );
    assert_eq!(
        update_render_settings_schema["properties"]["settings"]["required"],
        json!(["width", "height", "fps", "loudnessLufs", "captions"])
    );

    assert!(
        !project_action_variants
            .iter()
            .any(|variant| variant["additionalProperties"] == json!(true)),
        "ProjectAction schema must not include a permissive fallback variant"
    );
    assert_openai_strict_schema(&turn["params"]["outputSchema"], "turn.params.outputSchema");
}

#[test]
fn turn_output_schema_requires_complete_task_8_visual_metadata() {
    let project = sample_project();
    let request = sample_edit_request();
    let context = build_video_edit_context(&project, &request).expect("context");
    let turn = build_video_edit_turn_request(9, "thread-123", &context);
    let schema = &turn["params"]["outputSchema"];

    for collection in ["captions", "overlays", "hyperframes", "gpuVisuals"] {
        let item = &schema["properties"][collection]["items"];
        let required = item["required"].as_array().expect("required visual fields");
        for field in [
            "sourceBeat",
            "dimensions",
            "frameRate",
            "alpha",
            "visualTreatment",
            "motion",
            "safeZone",
            "avoid",
        ] {
            assert!(
                required.contains(&json!(field)),
                "{collection} must require {field}"
            );
        }
        assert_eq!(
            item["properties"]["dimensions"]["required"],
            json!(["width", "height"])
        );
        assert_eq!(
            item["properties"]["frameRate"],
            json!({ "type": "number", "exclusiveMinimum": 0 })
        );
        assert_eq!(item["properties"]["alpha"], json!({ "type": "boolean" }));
        assert_eq!(
            item["properties"]["sourceBeat"],
            json!({ "type": "string", "minLength": 1 })
        );
    }

    let prompt = turn["params"]["input"][0]["text"]
        .as_str()
        .expect("turn prompt");
    assert!(prompt.contains("sourceBeat, dimensions, positive frameRate, alpha"));
}

#[test]
fn turn_request_exposes_media_library_folders_to_agents() {
    let mut project = sample_project();
    project.media_folders = vec![
        MediaFolder {
            id: "generated".to_string(),
            name: "Generated selects".to_string(),
            parent_id: None,
        },
        MediaFolder {
            id: "scene-a".to_string(),
            name: "Scene A".to_string(),
            parent_id: Some("generated".to_string()),
        },
    ];
    project.media[0].folder_id = Some("scene-a".to_string());
    project.media.push(MediaAsset {
        id: "media-2".to_string(),
        name: None,
        relative_path: "media/generated/variation-a.mp4".to_string(),
        kind: MediaKind::Generated,
        duration_seconds: 5.0,
        width: Some(1080),
        height: Some(1920),
        fps: Some(30.0),
        folder_id: Some("generated".to_string()),
    });
    let request = sample_edit_request();
    let context = build_video_edit_context(&project, &request).expect("context");

    let turn = build_video_edit_turn_request(9, "thread-123", &context);

    let text = turn["params"]["input"][0]["text"].as_str().expect("text");
    assert!(text.contains("Project library:"));
    assert!(text.contains("- folder generated: Generated selects"));
    assert!(text.contains("- folder scene-a: Generated selects / Scene A"));
    assert!(text.contains(
        "- media media-1: media/input.mp4 | video | 120.000s | folder: Generated selects / Scene A"
    ));
    assert!(text.contains(
        "- media media-2: media/generated/variation-a.mp4 | generated | 5.000s | folder: Generated selects"
    ));
    assert!(text.contains("Use folder ids, not display paths, in projectActions"));
}

#[test]
fn turn_request_exposes_timeline_context_to_agents() {
    let mut project = sample_project();
    project.timeline.duration_seconds = 3.6;
    project.timeline.tracks[0].items.push(TimelineItem {
        id: "clip-1".to_string(),
        kind: TimelineItemKind::VideoClip,
        start_seconds: 0.0,
        duration_seconds: 3.6,
        source: TimelineSource::Media {
            media_id: "media-1".to_string(),
        },
        label: "Hook clip".to_string(),
        properties: BTreeMap::from([
            ("sourceIn".to_string(), json!(0.2)),
            ("sourceOut".to_string(), json!(3.8)),
            ("reason".to_string(), json!("hook")),
            ("generatedAssetId".to_string(), json!("generated-shot-1")),
            (
                "generatedOutputMediaId".to_string(),
                json!("generated-shot-1-output"),
            ),
        ]),
    });
    project.timeline.tracks[3].items.push(TimelineItem {
        id: "caption-1".to_string(),
        kind: TimelineItemKind::Caption,
        start_seconds: 0.65,
        duration_seconds: 1.35,
        source: TimelineSource::Text {
            text: "Original caption text".to_string(),
        },
        label: "Caption 1".to_string(),
        properties: BTreeMap::from([
            ("transcriptId".to_string(), json!("transcript-1")),
            ("wordIndex".to_string(), json!(1)),
        ]),
    });
    project.timeline.tracks[2].items.push(TimelineItem {
        id: "overlay-title-1".to_string(),
        kind: TimelineItemKind::Overlay,
        start_seconds: 2.2,
        duration_seconds: 1.0,
        source: TimelineSource::Text {
            text: "Launch title".to_string(),
        },
        label: "Launch title".to_string(),
        properties: BTreeMap::from([
            ("text".to_string(), json!("Launch title")),
            (
                "visualTreatment".to_string(),
                json!("bold upper-left title with transparent backing"),
            ),
            (
                "motion".to_string(),
                json!("fade in quickly, hold, then drift upward"),
            ),
            (
                "safeZone".to_string(),
                json!("keep inside title safe margins"),
            ),
            (
                "avoid".to_string(),
                json!("covering faces or using opaque full-width slabs"),
            ),
            ("textEdited".to_string(), json!(true)),
        ]),
    });
    let audio_track = project
        .timeline
        .tracks
        .iter_mut()
        .find(|track| track.id == "track-audio")
        .expect("audio track");
    audio_track.enabled = false;
    audio_track.items.push(TimelineItem {
        id: "audio-bed-1".to_string(),
        kind: TimelineItemKind::AudioClip,
        start_seconds: 0.0,
        duration_seconds: 3.6,
        source: TimelineSource::Media {
            media_id: "media-1".to_string(),
        },
        label: "Music bed".to_string(),
        properties: BTreeMap::from([
            ("fadeInSeconds".to_string(), json!(0.5)),
            ("fadeOutSeconds".to_string(), json!(1.25)),
            ("volumeDb".to_string(), json!(-6.0)),
        ]),
    });
    let request = sample_edit_request();
    let context = build_video_edit_context(&project, &request).expect("context");

    let turn = build_video_edit_turn_request(9, "thread-123", &context);

    let text = turn["params"]["input"][0]["text"].as_str().expect("text");
    assert!(text.contains("Project timeline:"));
    assert!(text.contains("- durationSeconds: 3.600"));
    assert!(text.contains("- track track-video: Video | video | unlocked | enabled | items: 1"));
    assert!(text.contains(
        "- item clip-1: video_clip | 0.000-3.600s | source: media media-1 | label: Hook clip | sourceIn: 0.200 | sourceOut: 3.800 | reason: hook | generatedAssetId: generated-shot-1 | generatedOutputMediaId: generated-shot-1-output"
    ));
    assert!(
        text.contains("- track track-captions: Captions | caption | unlocked | enabled | items: 1")
    );
    assert!(
        text.contains("- track track-overlays: Overlays | overlay | unlocked | enabled | items: 1")
    );
    assert!(text.contains(
        "- item overlay-title-1: overlay | 2.200-3.200s | source: text \"Launch title\" | label: Launch title | text: Launch title | visualTreatment: bold upper-left title with transparent backing | motion: fade in quickly, hold, then drift upward | safeZone: keep inside title safe margins | avoid: covering faces or using opaque full-width slabs | textEdited: true"
    ));
    assert!(text.contains(
        "- item caption-1: caption | 0.650-2.000s | source: text \"Original caption text\" | label: Caption 1 | transcriptId: transcript-1 | wordIndex: 1"
    ));
    assert!(text.contains("- track track-audio: Audio | audio | unlocked | disabled | items: 1"));
    assert!(text.contains(
        "- item audio-bed-1: audio_clip | 0.000-3.600s | source: media media-1 | label: Music bed | fadeInSeconds: 0.500 | fadeOutSeconds: 1.250 | volumeDb: -6.000"
    ));
    assert!(text.contains(
        "Use timeline item ids and track ids when returning trimItems, splitItems, reorderItems, moveItems, resizeItems, insertItems, or removeItems projectActions."
    ));
    assert!(text.contains(
        "Use track ids for createTrack, setTrackLocked, and setTrackEnabled; use audio item ids for updateAudioFades, updateAudioFadeOut, and updateAudioVolume; use visual item ids for updateVisualClipOpacity, updateVisualClipTransform, and updateVisualClipCrop."
    ));
    assert!(text.contains("Use transition ids for updateTransition and removeTransition."));
}

#[test]
fn turn_request_lists_timeline_transitions_for_agents() {
    let mut project = sample_project();
    for (id, start_seconds) in [("item-1", 0.0), ("item-2", 4.0)] {
        project.timeline.tracks[0].items.push(TimelineItem {
            id: id.to_string(),
            kind: TimelineItemKind::VideoClip,
            start_seconds,
            duration_seconds: 4.0,
            source: TimelineSource::Media {
                media_id: "media-1".to_string(),
            },
            label: id.to_string(),
            properties: BTreeMap::from([("sourceIn".to_string(), json!(start_seconds + 10.0))]),
        });
    }
    project.timeline.tracks[0].transitions.push(
        video_creater_lib::project::model::TimelineTransition {
            id: "fade-1".to_string(),
            left_item_id: "item-1".to_string(),
            right_item_id: "item-2".to_string(),
            kind: video_creater_lib::project::model::TransitionKind::DipToBlack,
            duration_seconds: 0.5,
        },
    );
    let request = sample_edit_request();
    let context = build_video_edit_context(&project, &request).expect("context");

    let turn = build_video_edit_turn_request(9, "thread-123", &context);

    let text = turn["params"]["input"][0]["text"].as_str().expect("text");
    assert!(
        text.contains("- transition fade-1: dipToBlack | item-1 -> item-2 | 0.500s"),
        "{text}"
    );
}

#[test]
fn turn_request_exposes_generated_asset_provenance_to_agents() {
    let mut project = sample_project();
    project.media.push(MediaAsset {
        id: "media-first-frame".to_string(),
        name: None,
        relative_path: "media/first-frame.png".to_string(),
        kind: MediaKind::Image,
        duration_seconds: 0.0,
        width: Some(1080),
        height: Some(1920),
        fps: None,
        folder_id: None,
    });
    project.media.push(MediaAsset {
        id: "media-style-ref".to_string(),
        name: None,
        relative_path: "media/style-reference.png".to_string(),
        kind: MediaKind::Image,
        duration_seconds: 0.0,
        width: Some(1080),
        height: Some(1920),
        fps: None,
        folder_id: None,
    });
    project.generated_assets.push(GeneratedAsset {
        schema_version: 1,
        id: "generated-shot-1".to_string(),
        kind: MediaKind::Generated,
        status: GeneratedAssetStatus::Completed,
        name: None,
        target_folder_id: None,
        placement_intent: None,
        prompt: "slow push-in on the product with warm sunset light".to_string(),
        model: GenerationModel {
            provider: "seedance".to_string(),
            id: "seedance-2-fast".to_string(),
        },
        references: GeneratedAssetReferences {
            media_ids: vec!["media-style-ref".to_string()],
            first_frame_media_id: Some("media-first-frame".to_string()),
            last_frame_media_id: None,
            provider_input_urls: Vec::new(),
            ..Default::default()
        },
        settings: GeneratedAssetSettings {
            width: Some(1080),
            height: Some(1920),
            duration_seconds: Some(4.0),
            fps: Some(30.0),
            aspect_ratio: Some("9:16".to_string()),
            resolution: None,
            generate_audio: None,
            ..GeneratedAssetSettings::default()
        },
        outputs: vec![GeneratedAssetOutput {
            media_id: "generated-shot-1-output".to_string(),
            relative_path: "generated/generated-shot-1/output.mp4".to_string(),
            source_url: None,
            width: 1080,
            height: 1920,
            duration_seconds: 4.0,
            fps: 30.0,
        }],
        created_at: "2026-06-22T10:00:00Z".to_string(),
        parent_asset_id: Some("generated-parent-1".to_string()),
        retry_of_asset_id: None,
    });
    let request = sample_edit_request();
    let context = build_video_edit_context(&project, &request).expect("context");

    let turn = build_video_edit_turn_request(9, "thread-123", &context);

    let text = turn["params"]["input"][0]["text"].as_str().expect("text");
    assert!(text.contains("Generated assets:"));
    assert!(text.contains(
        "Use generated asset ids for parentAssetId, retryOfAssetId, updateGeneratedAssetStatus assetId, and completeGeneratedAsset assetId; completeGeneratedAsset can include replacement { itemId, mediaId } when the completed output should swap a timeline clip, and replaceTimelineItemWithGeneratedOutput remains available for already-completed outputs."
    ));
    assert!(text.contains(
        "- asset generated-shot-1: generated | completed | model: seedance/seedance-2-fast | settings: 1080x1920, 4.000s, 30.000fps, aspect 9:16 | prompt: slow push-in on the product with warm sunset light | firstFrame: media-first-frame | lastFrame: none | references: media-style-ref | parent: generated-parent-1 | retryOf: none"
    ));
    assert!(text.contains(
        "- output generated-shot-1-output: generated/generated-shot-1/output.mp4 | 1080x1920 | 4.000s | fps: 30.000"
    ));
}

#[test]
fn turn_request_exposes_project_template_overrides_to_agents() {
    let mut project = sample_project();
    project.template_overrides.push(ProjectTemplateOverride {
        schema_version: 1,
        template_id: "kinetic-lower-third-v1".to_string(),
        name: "Launch Lower Third".to_string(),
        fields: BTreeMap::from([
            ("headline".to_string(), "Launch day".to_string()),
            (
                "subline".to_string(),
                "Built with Video Creater".to_string(),
            ),
        ]),
        style: BTreeMap::from([
            ("accentColor".to_string(), json!("#22d3ee")),
            ("backgroundColor".to_string(), json!("rgba(2, 6, 23, 0.72)")),
        ]),
        visual_treatment: "compact translucent lower third with cyan accent and strong hierarchy"
            .to_string(),
        motion: "slide in, hold, soft fade".to_string(),
        safe_zone: "keep essential text inside 10% margins".to_string(),
        avoid: "full-width opaque black slabs".to_string(),
    });
    let request = sample_edit_request();
    let context = build_video_edit_context(&project, &request).expect("context");

    let turn = build_video_edit_turn_request(9, "thread-123", &context);

    let text = turn["params"]["input"][0]["text"].as_str().expect("text");
    assert!(text.contains("Project template overrides:"));
    assert!(text.contains(
        "Use templateId with updateTemplateOverride to revise project-level fields, style, visualTreatment, motion, safeZone, and avoid."
    ));
    assert!(text.contains(
        "- template kinetic-lower-third-v1: Launch Lower Third | fields: headline=Launch day, subline=Built with Video Creater | style: accentColor=#22d3ee, backgroundColor=rgba(2, 6, 23, 0.72) | visualTreatment: compact translucent lower third with cyan accent and strong hierarchy | motion: slide in, hold, soft fade | safeZone: keep essential text inside 10% margins | avoid: full-width opaque black slabs"
    ));
}

#[test]
fn turn_request_exposes_render_reports_to_agents() {
    let mut project = sample_project();
    project.render_reports.push(ProjectRenderReport {
        schema_version: 1,
        id: "render-draft-1".to_string(),
        status: RenderReportStatus::Completed,
        output_path: "renders/render-draft-1/output.mp4".to_string(),
        duration_seconds: 45.0,
        quality: None,
        requested_width: None,
        requested_height: None,
        actual_width: None,
        actual_height: None,
        streams: RenderReportStreams {
            video: true,
            audio: true,
        },
        checks: BTreeMap::from([
            ("duration".to_string(), RenderReportCheckStatus::Passed),
            (
                "captionAlignment".to_string(),
                RenderReportCheckStatus::Skipped,
            ),
            ("overlayTiming".to_string(), RenderReportCheckStatus::Passed),
            (
                "visualFrameEvidence".to_string(),
                RenderReportCheckStatus::Skipped,
            ),
            ("artifactPaths".to_string(), RenderReportCheckStatus::Passed),
            ("streams".to_string(), RenderReportCheckStatus::Passed),
            ("logPath".to_string(), RenderReportCheckStatus::Passed),
        ]),
        artifacts: vec![
            "renders/render-draft-1/output.mp4".to_string(),
            "renders/render-draft-1/report.json".to_string(),
        ],
        preview_comparison_request: None,
        preview_comparison: None,
        log_path: "logs/render-draft-1.log".to_string(),
        created_at: "2026-06-23T10:00:00Z".to_string(),
    });
    let request = sample_edit_request();
    let context = build_video_edit_context(&project, &request).expect("context");

    let turn = build_video_edit_turn_request(9, "thread-123", &context);

    let text = turn["params"]["input"][0]["text"].as_str().expect("text");
    assert!(text.contains("Render reports:"));
    assert!(text.contains(
        "Use attachRenderReport for new render-review artifacts; preserve outputPath, streams, checks, artifact paths, and logPath."
    ));
    assert!(text.contains(
        "- report render-draft-1: completed | output: renders/render-draft-1/output.mp4 | duration: 45.000s | streams: video=true,audio=true | checks: artifactPaths=passed, captionAlignment=skipped, duration=passed, logPath=passed, overlayTiming=passed, streams=passed, visualFrameEvidence=skipped | visualEvidence: frameArtifacts=0 | artifacts: renders/render-draft-1/output.mp4, renders/render-draft-1/report.json | log: logs/render-draft-1.log"
    ));
}

#[test]
fn turn_request_exposes_workflow_jobs_to_agents() {
    let mut project = sample_project();
    project.jobs.push(JobSummary {
        id: "codex-edit-1".to_string(),
        kind: "codex_edit".to_string(),
        status: JobStatus::Queued,
        updated_at: "2026-06-23T12:30:00Z".to_string(),
        workflow: Some(TemporalWorkflowMetadata {
            workflow_id: "video-creater/project-1/codex-edit/codex-edit-1".to_string(),
            workflow_type: "VideoCreaterCodexEditWorkflow".to_string(),
            task_queue: "video-creater-workflows".to_string(),
            run_id: None,
            activity_types: vec![
                "CollectProjectContext".to_string(),
                "RequestCodexProposal".to_string(),
                "ValidateProjectActions".to_string(),
                "PersistAcceptedProposal".to_string(),
                "AttachCodexEditFailure".to_string(),
            ],
        }),
        start_request: None,
        provider_request: None,
        failure_reason: None,
        export_settings: None,
    });
    let request = sample_edit_request();
    let context = build_video_edit_context(&project, &request).expect("context");

    let turn = build_video_edit_turn_request(9, "thread-123", &context);

    let text = turn["params"]["input"][0]["text"].as_str().expect("text");
    assert!(text.contains("Workflow jobs:"));
    assert!(text.contains(
        "Use recordJob before starting durable work, updateJobStatus as Temporal workflows progress, and updateGeneratedAssetStatus when generation assets move between queued, running, failed, or completed."
    ));
    assert!(text.contains(
        "- job codex-edit-1: codex_edit | queued | updated: 2026-06-23T12:30:00Z | workflow: VideoCreaterCodexEditWorkflow @ video-creater-workflows | workflowId: video-creater/project-1/codex-edit/codex-edit-1 | runId: none | activities: CollectProjectContext, RequestCodexProposal, ValidateProjectActions, PersistAcceptedProposal, AttachCodexEditFailure"
    ));
}

#[test]
fn turn_request_exposes_export_artifacts_to_agents() {
    let mut project = sample_project();
    project.export_artifacts.push(ProjectExportArtifact {
        schema_version: 1,
        id: "nle-export-premiere-1".to_string(),
        kind: ProjectExportArtifactKind::NleXml,
        format: "premiereXmeml".to_string(),
        path: "exports/project-1-premiere.xml".to_string(),
        mime_type: "application/xml".to_string(),
        job_id: Some("nle-export-premiere-1".to_string()),
        created_at: "2026-06-23T12:00:00Z".to_string(),
    });
    let request = sample_edit_request();
    let context = build_video_edit_context(&project, &request).expect("context");

    let turn = build_video_edit_turn_request(9, "thread-123", &context);

    let text = turn["params"]["input"][0]["text"].as_str().expect("text");
    assert!(text.contains("Export artifacts:"));
    assert!(text.contains(
        "Use recordExportArtifact after creating durable export outputs; keep paths project-relative under exports/."
    ));
    assert!(text.contains(
        "- export nle-export-premiere-1: nle_xml | premiereXmeml | path: exports/project-1-premiere.xml | mimeType: application/xml | job: nle-export-premiere-1 | created: 2026-06-23T12:00:00Z"
    ));
}

#[test]
fn turn_request_exposes_export_capabilities_to_agents() {
    let project = sample_project();
    let request = sample_edit_request();
    let context = build_video_edit_context(&project, &request).expect("context");

    let turn = build_video_edit_turn_request(9, "thread-123", &context);

    let text = turn["params"]["input"][0]["text"].as_str().expect("text");
    assert!(text.contains("Export capabilities:"));
    assert!(
        text.contains("Supported exports: draft WebM, final WebM, Premiere XMEML, DaVinci FCPXML.")
    );
    assert!(text.contains(
        "NLE XML command: export_nle_xml_to_split_project_folder(format: premiereXmeml | davinciFcpxml)."
    ));
    assert!(text.contains(
        "Temporal workflow: export_nle_xml uses VideoCreaterExportNleXmlWorkflow on video-creater-workflows."
    ));
    #[cfg(target_os = "macos")]
    let native_delivery_guidance = "MP4/H.264/H.265/ProRes: use the bundled macOS AVFoundation exporter when available; reviewed GStreamer factories remain the compatibility fallback.";
    #[cfg(not(target_os = "macos"))]
    let native_delivery_guidance = "MP4/H.264/H.265/ProRes: use the reviewed GStreamer encoders; H.264 uses a VA-API hardware encoder or OpenH264 with FFmpeg AAC audio, ProRes uses the FFmpeg ProRes encoder with PCM audio, and H.265 is available only when a VA-API hardware encoder is present. Check the export profile availability report before proposing MP4 or ProRes delivery.";
    assert!(text.contains(native_delivery_guidance));
    #[cfg(not(target_os = "macos"))]
    assert!(!text.contains("AVFoundation"));
}

#[test]
fn turn_request_exposes_split_project_files_to_agents() {
    let mut project = sample_project();
    project.schema_version = 2;
    let request = sample_edit_request();
    let context = build_video_edit_context_with_project_dir(
        &project,
        &request,
        Path::new("/tmp/video-creater-project"),
    )
    .expect("context");

    let turn = build_video_edit_turn_request(9, "thread-123", &context);

    let text = turn["params"]["input"][0]["text"].as_str().expect("text");
    assert!(text.contains("Project files:"));
    assert!(text.contains("- root: /tmp/video-creater-project"));
    assert!(text.contains("- manifest: /tmp/video-creater-project/video-creater.project.json"));
    assert!(text.contains("- timeline: /tmp/video-creater-project/timeline.json"));
    assert!(text.contains("- media: /tmp/video-creater-project/media/index.json"));
    assert!(text.contains("- transcripts index: /tmp/video-creater-project/transcripts/index.json"));
    assert!(text
        .contains("- transcript sidecars: /tmp/video-creater-project/transcripts/<media-id>.json"));
    assert!(text.contains("- templates index: /tmp/video-creater-project/templates/index.json"));
    assert!(text
        .contains("- template sidecars: /tmp/video-creater-project/templates/<template-id>.json"));
    assert!(text.contains("- generated index: /tmp/video-creater-project/generated/index.json"));
    assert!(text.contains(
        "- generated sidecars: /tmp/video-creater-project/generated/<asset-id>/asset.json"
    ));
    assert!(text.contains("- renders index: /tmp/video-creater-project/renders/index.json"));
    assert!(text
        .contains("- render sidecars: /tmp/video-creater-project/renders/<render-id>/report.json"));
    assert!(text.contains("- jobs index: /tmp/video-creater-project/jobs/index.json"));
    assert!(text.contains("- job sidecars: /tmp/video-creater-project/jobs/<job-id>/job.json"));
    assert!(text.contains("- exports index: /tmp/video-creater-project/exports/index.json"));
    assert!(text.contains(
        "- export sidecars: /tmp/video-creater-project/exports/<export-id>/artifact.json"
    ));
    assert!(text.contains("- context summary: /tmp/video-creater-project/context/project.json"));
    assert!(text.contains("- logs: /tmp/video-creater-project/logs/"));
    assert!(text.contains(
        "context/project.json is derived discovery metadata with files, indexes, counts, and latest ids"
    ));
    assert!(text.contains(
        "Inspect split project files when useful; return projectActions for mutations so Rust validates canonical state."
    ));
    assert!(!text.contains("providerCredentialEnvVar"));
    assert!(!text.contains("projectDir"));
}

#[test]
fn local_tool_manifest_exposes_palmier_parity_control_plane() {
    let tools = list_codex_local_tools();
    let names = tools
        .iter()
        .map(|tool| tool.name.as_str())
        .collect::<Vec<_>>();

    for expected in [
        "video_creater.project_context",
        "video_creater.timeline",
        "video_creater.media_library",
        "video_creater.generation_defaults",
        "video_creater.validate_project_actions",
        "video_creater.validate_codex_edit_proposal",
    ] {
        assert!(
            names.contains(&expected),
            "missing baseline tool {expected}"
        );
    }
    assert!(tools.iter().any(|tool| tool.category == "query"));
    assert!(tools.iter().any(|tool| tool.category == "generation"));
    assert!(tools.iter().any(|tool| tool.category == "edit_validation"));
    assert!(tools
        .iter()
        .all(|tool| tool.input_schema["type"] == json!("object")));
}

#[test]
fn mcp_parity_tool_manifest_exposes_transcript_words_query() {
    let tools = list_codex_local_tools();
    let tool = tools
        .iter()
        .find(|tool| tool.name == "video_creater.transcript_words")
        .expect("transcript words tool");

    assert_eq!(tool.category, "transcription");
    assert!(tool.description.contains("word"));
    assert_eq!(
        tool.input_schema["anyOf"],
        serde_json::json!([{ "required": ["mediaId"] }, { "required": ["mediaRef"] }])
    );
    assert_eq!(
        tool.input_schema["properties"]["limit"]["maximum"],
        serde_json::json!(500)
    );
}

#[test]
fn local_tool_project_context_returns_bounded_project_summary() {
    let mut project = sample_project();
    project.schema_version = 2;
    project.codex_thread_id = Some("thread-1".to_string());

    let result = call_codex_local_tool(
        &project,
        "video_creater.project_context",
        json!({ "projectDir": "/tmp/video-creater-project" }),
    )
    .expect("project context tool");

    assert_eq!(result.tool_name, "video_creater.project_context");
    assert!(!result.mutates_project);
    assert_eq!(result.payload["project"]["id"], json!("project-1"));
    assert_eq!(result.payload["project"]["name"], json!("Codex Test"));
    assert_eq!(result.payload["project"]["schemaVersion"], json!(2));
    assert_eq!(
        result.payload["project"]["codexThreadId"],
        json!("thread-1")
    );
    assert_eq!(result.payload["counts"]["media"], json!(1));
    assert_eq!(result.payload["counts"]["timelineTracks"], json!(5));
    assert_eq!(
        result.payload["splitProject"]["timeline"],
        json!("/tmp/video-creater-project/timeline.json")
    );
    assert_eq!(
        result.payload["policy"]["canonicalMutation"],
        json!("projectActionsValidatedByRust")
    );
}

#[test]
fn local_tool_timeline_returns_tracks_and_items() {
    let mut project = sample_project();
    project.timeline.duration_seconds = 5.0;
    project.timeline.tracks[0].items.push(TimelineItem {
        id: "clip-1".to_string(),
        kind: TimelineItemKind::VideoClip,
        start_seconds: 0.0,
        duration_seconds: 5.0,
        source: TimelineSource::Media {
            media_id: "media-1".to_string(),
        },
        label: "Hook".to_string(),
        properties: BTreeMap::from([
            ("sourceIn".to_string(), json!(1.0)),
            ("sourceOut".to_string(), json!(6.0)),
        ]),
    });

    let result = call_codex_local_tool(&project, "video_creater.timeline", json!({}))
        .expect("timeline tool");

    assert_eq!(result.tool_name, "video_creater.timeline");
    assert_eq!(result.payload["durationSeconds"], json!(5.0));
    assert_eq!(result.payload["tracks"][0]["id"], json!("track-video"));
    assert_eq!(
        result.payload["tracks"][0]["items"][0]["id"],
        json!("clip-1")
    );
    assert_eq!(
        result.payload["tracks"][0]["items"][0]["source"],
        json!({ "type": "media", "mediaId": "media-1" })
    );
    assert_eq!(
        result.payload["tracks"][0]["items"][0]["properties"]["sourceIn"],
        json!(1.0)
    );
}

#[test]
fn local_tool_media_library_returns_folders_and_assets() {
    let mut project = sample_project();
    project.media_folders.push(MediaFolder {
        id: "folder-1".to_string(),
        name: "Selects".to_string(),
        parent_id: None,
    });
    project.media[0].folder_id = Some("folder-1".to_string());

    let result = call_codex_local_tool(&project, "video_creater.media_library", json!({}))
        .expect("media library tool");

    assert_eq!(result.tool_name, "video_creater.media_library");
    assert_eq!(result.payload["folders"][0]["id"], json!("folder-1"));
    assert_eq!(result.payload["media"][0]["id"], json!("media-1"));
    assert_eq!(
        result.payload["media"][0]["relativePath"],
        json!("media/input.mp4")
    );
    assert_eq!(result.payload["media"][0]["kind"], json!("video"));
    assert_eq!(result.payload["media"][0]["folderId"], json!("folder-1"));
}

#[test]
fn local_tool_generation_defaults_derive_from_selected_media() {
    let project = sample_project();

    let result = call_codex_local_tool(
        &project,
        "video_creater.generation_defaults",
        json!({ "selectedMediaId": "media-1", "prompt": "make a vertical variation" }),
    )
    .expect("generation defaults tool");

    assert_eq!(result.tool_name, "video_creater.generation_defaults");
    assert_eq!(result.payload["kind"], json!("generated"));
    assert_eq!(result.payload["prompt"], json!("make a vertical variation"));
    assert_eq!(result.payload["settings"]["width"], json!(1080));
    assert_eq!(result.payload["settings"]["height"], json!(1920));
    assert_eq!(result.payload["settings"]["durationSeconds"], json!(8.0));
    assert_eq!(result.payload["settings"]["fps"], json!(30.0));
    assert_eq!(result.payload["settings"]["aspectRatio"], json!("9:16"));
    assert_eq!(result.payload["references"]["mediaIds"], json!(["media-1"]));
}

#[test]
fn local_tool_validate_project_actions_uses_clone_without_mutating_project() {
    let project = sample_project();

    let result = call_codex_local_tool(
        &project,
        "video_creater.validate_project_actions",
        json!({
            "actions": [
                { "type": "setTrackLocked", "trackId": "track-video", "locked": true }
            ]
        }),
    )
    .expect("project action validation tool");

    assert_eq!(result.tool_name, "video_creater.validate_project_actions");
    assert!(!result.mutates_project);
    assert_eq!(result.payload["valid"], json!(true));
    assert_eq!(result.payload["actionCount"], json!(1));
    assert_eq!(result.payload["projectAfter"]["lockedTracks"], json!(1));
    assert!(!project.timeline.tracks[0].locked);
}

#[test]
fn local_tool_validate_project_actions_returns_structured_errors() {
    let project = sample_project();

    let error = call_codex_local_tool(
        &project,
        "video_creater.validate_project_actions",
        json!({
            "actions": [
                { "type": "setTrackLocked", "trackId": "missing-track", "locked": true }
            ]
        }),
    )
    .expect_err("invalid action should fail");

    assert!(error
        .to_string()
        .contains("project action validation failed"));
}

#[test]
fn local_tool_validate_codex_edit_proposal_returns_edl_summary() {
    let project = sample_project();

    let result = call_codex_local_tool(
        &project,
        "video_creater.validate_codex_edit_proposal",
        json!({
            "request": sample_edit_request(),
            "proposal": valid_template_proposal()
        }),
    )
    .expect("proposal validation tool");

    assert_eq!(
        result.tool_name,
        "video_creater.validate_codex_edit_proposal"
    );
    assert_eq!(result.payload["valid"], json!(true));
    assert_eq!(result.payload["clipCount"], json!(1));
    assert_eq!(result.payload["durationSeconds"], json!(45.0));
    assert_eq!(result.payload["mediaId"], json!("media-1"));
}

#[test]
fn local_tool_unknown_name_returns_explicit_error() {
    let project = sample_project();

    let error = call_codex_local_tool(&project, "video_creater.missing", json!({}))
        .expect_err("unknown tool should fail");

    assert!(error
        .to_string()
        .contains("unknown Codex local tool: video_creater.missing"));
}

#[test]
fn mcp_parity_tool_manifest_covers_queries_generation_exports_transcription_and_editing() {
    let tools = list_codex_local_tools();
    let names = tools
        .iter()
        .map(|tool| tool.name.as_str())
        .collect::<Vec<_>>();

    for expected in [
        "video_creater.generated_assets",
        "video_creater.workflow_jobs",
        "video_creater.render_reports",
        "video_creater.export_artifacts",
        "video_creater.export_profiles",
        "video_creater.transcription_readiness",
        "video_creater.build_generate_media_start_request",
        "video_creater.build_codex_edit_start_request",
        "video_creater.build_export_media_start_request",
        "video_creater.build_export_nle_xml_start_request",
        "video_creater.apply_project_actions",
    ] {
        assert!(names.contains(&expected), "missing tool {expected}");
    }
    assert!(tools
        .iter()
        .any(|tool| tool.name == "video_creater.apply_project_actions"
            && tool.category == "edit_mutation"));
    assert!(tools.iter().any(
        |tool| tool.name == "video_creater.build_export_media_start_request"
            && tool.category == "export"
    ));
    assert!(tools
        .iter()
        .any(|tool| tool.name == "video_creater.transcription_readiness"
            && tool.category == "transcription"));
}

#[test]
fn mcp_parity_tool_queries_generated_assets_jobs_render_reports_and_exports() {
    let mut project = sample_project();
    project.generated_assets.push(sample_generated_asset());
    project
        .jobs
        .push(sample_job("generate-job-1", "generate_media"));
    project.render_reports.push(sample_render_report());
    project.export_artifacts.push(sample_export_artifact());

    let generated = call_codex_local_tool(&project, "video_creater.generated_assets", json!({}))
        .expect("generated assets");
    assert_eq!(
        generated.payload["assets"][0]["id"],
        json!("generated-shot-1")
    );
    assert_eq!(
        generated.payload["assets"][0]["outputs"][0]["mediaId"],
        json!("generated-shot-1-output")
    );

    let jobs = call_codex_local_tool(&project, "video_creater.workflow_jobs", json!({}))
        .expect("workflow jobs");
    assert_eq!(jobs.payload["jobs"][0]["id"], json!("generate-job-1"));
    assert_eq!(jobs.payload["jobs"][0]["kind"], json!("generate_media"));

    let reports = call_codex_local_tool(&project, "video_creater.render_reports", json!({}))
        .expect("render reports");
    assert_eq!(
        reports.payload["renderReports"][0]["id"],
        json!("render-draft-1")
    );
    assert_eq!(
        reports.payload["renderReports"][0]["streams"],
        json!({ "video": true, "audio": true })
    );

    let exports = call_codex_local_tool(&project, "video_creater.export_artifacts", json!({}))
        .expect("export artifacts");
    assert_eq!(
        exports.payload["exportArtifacts"][0]["id"],
        json!("export-1")
    );
    assert_eq!(
        exports.payload["exportArtifacts"][0]["kind"],
        json!("nle_xml")
    );
}

#[test]
fn mcp_parity_tool_builds_replayable_start_requests_without_secret_values() {
    let _lock = EXPORT_PROFILE_ENV_LOCK.lock().expect("export env lock");
    let temp = tempfile::tempdir().expect("export runtime dir");
    let _env = IsolatedExportRuntimeEnv::new(temp.path());
    executable_fixture(temp.path(), "approved-h264-encoder");
    executable_fixture(temp.path(), "approved-aac-encoder");
    std::env::set_var("VIDEO_CREATER_ENABLE_LOCAL_EXPORT_PROFILES", "1");

    let project = sample_project();

    let generate = call_codex_local_tool(
        &project,
        "video_creater.build_generate_media_start_request",
        json!({
            "projectDir": "/tmp/video-creater-project",
            "assetId": "generated-shot-1",
            "jobId": "generate-job-1",
            "mockMode": true,
            "brief": {
                "prompt": "A product closeup with kinetic lighting",
                "placementIntent": "library"
            }
        }),
    )
    .expect("generate start request");
    assert_eq!(
        generate.payload["startRequest"]["workflowType"],
        json!("VideoCreaterGenerateMediaWorkflow")
    );
    let generate_payload = generate.payload.to_string();
    assert!(
        generate.payload["startRequest"]["input"]["providerCredentialEnvVar"].is_null(),
        "replayable requests must resolve credentials at execution time"
    );
    assert!(!generate_payload.contains("providerCredentialValue"));
    assert!(!generate_payload.contains("FAL_KEY"));

    let codex = call_codex_local_tool(
        &project,
        "video_creater.build_codex_edit_start_request",
        json!({
            "projectRoot": "/tmp/video-creater-root",
            "projectDir": "/tmp/video-creater-project",
            "jobId": "codex-edit-1",
            "request": sample_edit_request()
        }),
    )
    .expect("codex start request");
    assert_eq!(
        codex.payload["startRequest"]["workflowType"],
        json!("VideoCreaterCodexEditWorkflow")
    );
    assert_eq!(
        codex.payload["startRequest"]["input"]["request"]["mediaId"],
        json!("media-1")
    );

    let export_media_result = call_codex_local_tool(
        &project,
        "video_creater.build_export_media_start_request",
        json!({
            "projectDir": "/tmp/video-creater-project",
            "jobId": "export-media-1",
            "profile": "mp4H264",
            "quality": "draft",
            "width": 1280,
            "height": 720,
            "outputPath": "exports/draft.mp4"
        }),
    );
    let h264_available = mp4_export_profile_availability_report()
        .into_iter()
        .find(|candidate| candidate.profile == ExportProfile::Mp4H264)
        .map(|availability| availability.available)
        .unwrap_or(false);
    if h264_available {
        let export_media = export_media_result.expect("export media start request");
        assert_eq!(
            export_media.payload["startRequest"]["workflowType"],
            json!("VideoCreaterExportMediaWorkflow")
        );
        assert_eq!(
            export_media.payload["startRequest"]["input"]["quality"],
            "draft"
        );
        assert_eq!(export_media.payload["startRequest"]["input"]["width"], 1280);
        assert_eq!(export_media.payload["startRequest"]["input"]["height"], 720);
    } else {
        let error = export_media_result.expect_err("unavailable MP4 runtime should be reported");
        assert!(error.to_string().contains("mp4H264 export is unavailable"));
    }

    let export_nle = call_codex_local_tool(
        &project,
        "video_creater.build_export_nle_xml_start_request",
        json!({
            "projectDir": "/tmp/video-creater-project",
            "jobId": "export-nle-1",
            "format": "premiereXmeml",
            "outputPath": "exports/project.xml"
        }),
    )
    .expect("export nle start request");
    assert_eq!(
        export_nle.payload["startRequest"]["workflowType"],
        json!("VideoCreaterExportNleXmlWorkflow")
    );
    assert_eq!(
        export_nle.payload["startRequest"]["input"]["format"],
        json!("premiereXmeml")
    );
}

#[test]
fn mcp_parity_tool_reports_export_profiles_and_transcription_readiness() {
    let mut project = sample_project();
    let profiles = call_codex_local_tool(&project, "video_creater.export_profiles", json!({}))
        .expect("export profiles");
    assert!(profiles
        .payload
        .get("profiles")
        .and_then(serde_json::Value::as_array)
        .expect("profiles")
        .iter()
        .any(|profile| profile["profile"] == json!("mp4H264")));

    let ready = call_codex_local_tool(
        &project,
        "video_creater.transcription_readiness",
        json!({ "mediaId": "media-1", "modelId": "nvidia/parakeet-tdt-0.6b-v3" }),
    )
    .expect("transcription readiness");
    assert_eq!(ready.payload["ready"], json!(true));

    project.transcripts.clear();
    let not_ready = call_codex_local_tool(
        &project,
        "video_creater.transcription_readiness",
        json!({ "mediaId": "media-1", "modelId": "nvidia/parakeet-tdt-0.6b-v3" }),
    )
    .expect("transcription readiness failure payload");
    assert_eq!(not_ready.payload["ready"], json!(false));
    assert!(not_ready.payload["error"]
        .as_str()
        .expect("error")
        .contains("transcript"));
}

#[test]
fn transcript_words_tool_returns_word_indices_and_timings() {
    let mut project = sample_project();
    project.transcripts[0].words = vec![
        TranscriptWord {
            text: "Hook".to_string(),
            start_seconds: 0.4,
            end_seconds: 0.8,
            confidence: Some(0.91),
            speaker: Some("A".to_string()),
        },
        TranscriptWord {
            text: "moment".to_string(),
            start_seconds: 1.0,
            end_seconds: 1.4,
            confidence: None,
            speaker: None,
        },
    ];

    let result = call_codex_local_tool(
        &project,
        "video_creater.transcript_words",
        json!({ "mediaId": "media-1" }),
    )
    .expect("transcript words");

    assert!(!result.mutates_project);
    assert_eq!(result.payload["transcriptId"], json!("transcript-1"));
    assert_eq!(result.payload["totalWords"], json!(2));
    assert_eq!(result.payload["words"][0]["wordIndex"], json!(0));
    assert_eq!(result.payload["words"][0]["startSeconds"], json!(0.4));
    assert_eq!(result.payload["words"][0]["endSeconds"], json!(0.8));
    assert_eq!(result.payload["words"][0]["speaker"], json!("A"));
}

#[test]
fn transcript_words_tool_filters_ranges_and_paginates() {
    let mut project = sample_project();
    project.transcripts[0].words = (0..6)
        .map(|index| TranscriptWord {
            text: format!("word-{index}"),
            start_seconds: index as f64,
            end_seconds: index as f64 + 0.5,
            confidence: None,
            speaker: None,
        })
        .collect();

    let result = call_codex_local_tool(
        &project,
        "video_creater.transcript_words",
        json!({
            "mediaId": "media-1",
            "startSeconds": 1.2,
            "endSeconds": 4.2,
            "offset": 1,
            "limit": 2
        }),
    )
    .expect("transcript words");

    assert_eq!(result.payload["matchedWords"], json!(4));
    assert_eq!(result.payload["returnedWords"], json!(2));
    assert_eq!(result.payload["nextOffset"], json!(3));
    assert_eq!(result.payload["truncated"], json!(true));
    assert_eq!(result.payload["words"][0]["wordIndex"], json!(2));
    assert_eq!(result.payload["words"][1]["wordIndex"], json!(3));
}

#[test]
fn transcript_words_tool_returns_full_shape_for_missing_media() {
    let project = sample_project();

    let result = call_codex_local_tool(
        &project,
        "video_creater.transcript_words",
        json!({
            "mediaId": "missing-media",
            "startSeconds": 1.0,
            "endSeconds": 2.0,
            "offset": 4,
            "limit": 5
        }),
    )
    .expect("transcript words missing media");

    assert_eq!(result.payload["mediaId"], json!("missing-media"));
    assert_eq!(result.payload["transcriptId"], json!(null));
    assert_eq!(result.payload["engine"], json!(null));
    assert_eq!(result.payload["rawArtifactPath"], json!(null));
    assert_eq!(
        result.payload["range"],
        json!({ "startSeconds": 1.0, "endSeconds": 2.0 })
    );
    assert_eq!(result.payload["offset"], json!(4));
    assert_eq!(result.payload["limit"], json!(5));
    assert_eq!(result.payload["words"], json!([]));
    assert_eq!(result.payload["totalWords"], json!(0));
    assert_eq!(result.payload["matchedWords"], json!(0));
    assert_eq!(result.payload["returnedWords"], json!(0));
    assert_eq!(result.payload["nextOffset"], json!(null));
    assert_eq!(result.payload["truncated"], json!(false));
    assert!(result.payload["error"]
        .as_str()
        .expect("error")
        .contains("media was not found: missing-media"));
}

#[test]
fn transcript_words_tool_returns_full_shape_for_missing_transcript() {
    let mut project = sample_project();
    project.transcripts.clear();

    let result = call_codex_local_tool(
        &project,
        "video_creater.transcript_words",
        json!({
            "mediaId": "media-1",
            "startSeconds": 1.0,
            "endSeconds": 2.0,
            "offset": 4,
            "limit": 5
        }),
    )
    .expect("transcript words missing transcript");

    assert_eq!(result.payload["mediaId"], json!("media-1"));
    assert_eq!(result.payload["transcriptId"], json!(null));
    assert_eq!(result.payload["engine"], json!(null));
    assert_eq!(result.payload["rawArtifactPath"], json!(null));
    assert_eq!(
        result.payload["range"],
        json!({ "startSeconds": 1.0, "endSeconds": 2.0 })
    );
    assert_eq!(result.payload["offset"], json!(4));
    assert_eq!(result.payload["limit"], json!(5));
    assert_eq!(result.payload["words"], json!([]));
    assert_eq!(result.payload["totalWords"], json!(0));
    assert_eq!(result.payload["matchedWords"], json!(0));
    assert_eq!(result.payload["returnedWords"], json!(0));
    assert_eq!(result.payload["nextOffset"], json!(null));
    assert_eq!(result.payload["truncated"], json!(false));
    assert!(result.payload["error"]
        .as_str()
        .expect("error")
        .contains("transcript was not found for media: media-1"));
}

#[test]
fn transcript_words_tool_rejects_invalid_range() {
    let project = sample_project();
    let error = call_codex_local_tool(
        &project,
        "video_creater.transcript_words",
        json!({
            "mediaId": "media-1",
            "startSeconds": 4.0,
            "endSeconds": 4.0
        }),
    )
    .expect_err("invalid range");

    assert!(error.to_string().contains("endSeconds"));
}

#[test]
fn transcript_words_tool_rejects_zero_limit() {
    let project = sample_project();
    let error = call_codex_local_tool(
        &project,
        "video_creater.transcript_words",
        json!({
            "mediaId": "media-1",
            "limit": 0
        }),
    )
    .expect_err("zero limit");

    assert!(error.to_string().contains("limit"));
}

#[test]
fn transcript_words_tool_rejects_blank_media_id() {
    let project = sample_project();
    let error = call_codex_local_tool(
        &project,
        "video_creater.transcript_words",
        json!({
            "mediaId": " "
        }),
    )
    .expect_err("blank media id");

    assert!(error.to_string().contains("mediaId"));
}

#[test]
fn mcp_parity_tool_applies_project_actions_through_split_project_validation() {
    let dir = tempfile::tempdir().expect("project dir");
    let mut project = sample_project();
    project.schema_version = 2;
    save_split_project(dir.path(), &project).expect("save split project");

    let result = call_codex_local_tool(
        &project,
        "video_creater.apply_project_actions",
        json!({
            "projectDir": dir.path().to_str().expect("utf-8 project dir"),
            "actions": [
                { "type": "setTrackLocked", "trackId": "track-video", "locked": true }
            ]
        }),
    )
    .expect("apply project actions");

    assert!(result.mutates_project);
    assert_eq!(result.payload["actionCount"], json!(1));
    assert!(result.payload["writeReport"]["report"]["writtenFiles"]
        .as_array()
        .expect("written files")
        .iter()
        .any(|path| path
            .as_str()
            .expect("path")
            .ends_with("video-creater.project.json")));

    let reloaded = load_split_project(dir.path()).expect("reload split project");
    assert!(
        reloaded
            .timeline
            .tracks
            .iter()
            .find(|track| track.id == "track-video")
            .expect("video track")
            .locked
    );
}

#[test]
fn proposal_validation_rejects_full_source_pass_through() {
    let project = sample_project();
    let request = sample_edit_request();
    let proposal = CodexEditProposal {
        media_id: "media-1".to_string(),
        clips: vec![CodexProposalClip {
            media_id: "media-1".to_string(),
            source_in: 0.0,
            source_out: 120.0,
            reason: "style the whole source".to_string(),
        }],
        captions: Vec::new(),
        overlays: Vec::new(),
        hyperframes: Vec::new(),
        gpu_visuals: Vec::new(),
        project_actions: Vec::new(),
        render_review: CodexRenderReview {
            duration_seconds: 120.0,
            stream_check_required: true,
            caption_alignment_required: true,
            overlay_timing_required: true,
            visual_frame_evidence_required: true,
            artifact_paths_required: true,
            log_reference_required: true,
        },
    };

    let error = validate_codex_edit_proposal(&project, &request, &proposal)
        .expect_err("full source pass-through should be rejected");

    assert!(error.to_string().contains("full source"));
}

#[test]
fn turn_request_lists_available_motion_templates() {
    let project = sample_project();
    let request = sample_edit_request();
    let context = build_video_edit_context(&project, &request).expect("context");

    let turn = build_video_edit_turn_request(9, "thread-123", &context);
    let text = turn["params"]["input"][0]["text"].as_str().expect("text");

    assert!(text.contains("Available motion templates"));
    assert!(text.contains("kinetic-lower-third-v1"));
    assert!(text.contains("punchy-caption-v1"));
    assert!(text.contains("metric-callout-v1"));
    assert!(text.contains("chapter-card-v1"));
    assert!(text.contains("tracking-highlight-v1"));
    assert!(text.contains("holographic-logo-cutout-v1"));
    assert!(text.contains("gradient-background-loop-v1"));
    assert!(text.contains("category: text"));
    assert!(text.contains("required fields: logoAssetId"));
    assert!(text.contains("required fields: headline, subline"));
    assert_eq!(
        turn["params"]["outputSchema"]["properties"]["overlays"]["items"]["properties"]
            ["templateId"],
        serde_json::json!({ "type": ["string", "null"] })
    );
    assert_eq!(
        turn["params"]["outputSchema"]["properties"]["overlays"]["items"]["properties"]["fields"]
            ["type"],
        serde_json::json!("object")
    );
    assert_eq!(
        turn["params"]["outputSchema"]["properties"]["overlays"]["items"]["properties"]["fields"]
            ["required"],
        serde_json::json!(["headline", "subline", "logoAssetId"])
    );
}

#[test]
fn turn_request_lists_motion_presets_and_overlay_schema_allows_motion_preset_id() {
    let project = sample_project();
    let request = sample_edit_request();
    let context = build_video_edit_context(&project, &request).expect("context");

    let turn = build_video_edit_turn_request(9, "thread-123", &context);
    let text = turn["params"]["input"][0]["text"].as_str().expect("text");

    assert!(text.contains("Available motion presets"));
    assert!(text.contains("slide-fade-up-v1"));
    assert!(text.contains("tracking-draw-v1"));
    assert!(text.contains("slide-rotate-settle-v2"));
    assert!(text.contains("line-draw-v2"));
    assert_eq!(
        turn["params"]["outputSchema"]["properties"]["overlays"]["items"]["properties"]
            ["motionPresetId"]["anyOf"][0],
        json!({ "type": "string", "enum": [
            "slide-fade-up-v1",
            "snap-pop-v1",
            "underline-wipe-v1",
            "metric-count-pop-v1",
            "vertical-reveal-v1",
            "tracking-draw-v1",
            "spring-pop-v2",
            "slide-rotate-settle-v2",
            "mask-wipe-v2",
            "line-draw-v2",
            "word-pop-stagger-v2",
            "soft-depth-card-v2",
            "pulse-emphasis-v2",
            "exit-snap-v2"
        ]})
    );
}

#[test]
fn turn_request_lists_available_animation_primitives() {
    let project = sample_project();
    let request = sample_edit_request();
    let context = build_video_edit_context(&project, &request).expect("context");

    let turn = build_video_edit_turn_request(9, "thread-123", &context);
    let text = turn["params"]["input"][0]["text"].as_str().expect("text");
    let overlay_properties =
        &turn["params"]["outputSchema"]["properties"]["overlays"]["items"]["properties"];
    let nodes_schema = &overlay_properties["nodes"]["anyOf"][0];
    let animate_schema = &nodes_schema["items"]["properties"]["animate"]["anyOf"][0];

    assert!(text.contains("Animated primitive nodes"));
    assert!(text.contains("animate.keyframes"));
    assert_eq!(nodes_schema["type"], serde_json::json!("array"));
    assert_eq!(
        animate_schema["properties"]["keyframes"]["type"],
        serde_json::json!("array")
    );
    assert_eq!(nodes_schema["minItems"], serde_json::json!(1));
    assert_eq!(
        animate_schema["required"],
        serde_json::json!([
            "ease",
            "delaySeconds",
            "repeat",
            "yoyo",
            "origin",
            "keyframes"
        ])
    );
    assert_eq!(
        overlay_properties["sourceBeat"],
        serde_json::json!({ "type": "string", "minLength": 1 })
    );
    assert_eq!(
        animate_schema["properties"]["ease"]["anyOf"][0]["enum"],
        serde_json::json!([
            "linear",
            "inQuad",
            "outQuad",
            "inOutQuad",
            "inCubic",
            "outCubic",
            "inOutCubic",
            "outBack",
            "outElastic"
        ])
    );
}

#[test]
fn turn_request_lists_motion_v2_schema_fields() {
    let project = sample_project();
    let request = sample_edit_request();
    let context = build_video_edit_context(&project, &request).expect("context");

    let turn = build_video_edit_turn_request(9, "thread-123", &context);
    let text = turn["params"]["input"][0]["text"].as_str().expect("text");
    let overlay_properties =
        &turn["params"]["outputSchema"]["properties"]["overlays"]["items"]["properties"];
    let nodes_schema = &overlay_properties["nodes"]["anyOf"][0];
    let animate = &nodes_schema["items"]["properties"]["animate"]["anyOf"][0]["properties"];
    let text_reveal = &nodes_schema["items"]["properties"]["textReveal"]["anyOf"][0];

    assert!(text.contains("rotationDegrees"));
    assert!(text.contains("pathProgress"));
    assert!(text.contains("textReveal"));
    assert_eq!(
        animate["origin"]["anyOf"][0]["properties"]["x"]["anyOf"][0]["type"],
        "string"
    );
    assert_eq!(
        animate["keyframes"]["items"]["properties"]["rotationDegrees"]["type"],
        json!(["number", "null"])
    );
    assert_eq!(
        animate["keyframes"]["items"]["properties"]["clipProgress"]["maximum"],
        1
    );
    assert_eq!(
        text_reveal["properties"]["mode"]["enum"],
        json!(["whole", "line", "word", "character"])
    );
}

#[test]
fn turn_request_lists_gpu_visual_hq_profile_schema() {
    let project = sample_project();
    let request = sample_edit_request();
    let context = build_video_edit_context(&project, &request).expect("context");

    let turn = build_video_edit_turn_request(9, "thread-123", &context);
    let text = turn["params"]["input"][0]["text"].as_str().expect("text");
    let schema = &turn["params"]["outputSchema"];
    let gpu_visuals = &schema["properties"]["gpuVisuals"]["items"];

    assert!(text.contains("GPU visuals"));
    assert!(!text.contains("video_creater_fragment"));
    assert!(text.contains("shader_background"));
    assert_eq!(
        schema["required"],
        json!([
            "mediaId",
            "clips",
            "captions",
            "overlays",
            "hyperframes",
            "gpuVisuals",
            "projectActions",
            "renderReview"
        ])
    );
    assert_eq!(gpu_visuals["additionalProperties"], json!(false));
    assert_eq!(
        gpu_visuals["required"],
        json!([
            "id",
            "kind",
            "startSeconds",
            "durationSeconds",
            "qualityProfile",
            "sourceBeat",
            "dimensions",
            "frameRate",
            "alpha",
            "visualTreatment",
            "motion",
            "safeZone",
            "avoid"
        ])
    );
    assert_eq!(
        gpu_visuals["properties"]["kind"]["enum"],
        json!(["hybrid_scene", "shader_background"])
    );
    let required = gpu_visuals["required"]
        .as_array()
        .expect("gpu visual required fields");
    assert!(!required.contains(&json!("shader")));
    assert!(!required.contains(&json!("primitives")));
    assert_eq!(
        gpu_visuals["properties"]["sourceBeat"]["minLength"],
        json!(1)
    );
    assert_eq!(
        gpu_visuals["properties"]["qualityProfile"]["enum"],
        json!(supported_profile_ids())
    );
    let properties = gpu_visuals["properties"]
        .as_object()
        .expect("gpu visual properties");
    assert!(!properties.contains_key("shader"));
    assert!(!properties.contains_key("primitives"));
}

#[test]
fn turn_request_defaults_gpu_visuals_to_hq_neon_wireframe_style() {
    let project = sample_project();
    let request = sample_edit_request();
    let context = build_video_edit_context(&project, &request).expect("context");

    let turn = build_video_edit_turn_request(9, "thread-123", &context);
    let text = turn["params"]["input"][0]["text"].as_str().expect("text");
    let gpu_visuals = &turn["params"]["outputSchema"]["properties"]["gpuVisuals"]["items"];

    assert!(text.contains("Default GPU visual quality profile: hq-neon-wireframe-shader-v1"));
    assert!(text.contains("Collected Shadertoy background profiles"));
    assert!(text.contains("shadertoy-octagrams-v1"));
    assert!(text.contains("shader-background-templates/builtin/<template-id>/template.json"));
    assert!(text.contains("sourceKind user"));
    assert!(text.contains("project-local shader-background-templates folder"));
    assert!(text.contains("restrained animated gradient shader background"));
    assert!(text.contains("crisp neon wireframe 3D primitives"));
    assert!(text.contains("subtle motion trails"));
    assert!(text.contains("Rust expands the canonical shader background"));
    assert!(!text.contains("Raw GLSL must define"));
    assert!(!text.contains("video_creater_fragment"));
    assert!(text.contains("shader_background"));
    assert_eq!(
        gpu_visuals["required"],
        json!([
            "id",
            "kind",
            "startSeconds",
            "durationSeconds",
            "qualityProfile",
            "sourceBeat",
            "dimensions",
            "frameRate",
            "alpha",
            "visualTreatment",
            "motion",
            "safeZone",
            "avoid"
        ])
    );
    assert_eq!(
        gpu_visuals["properties"]["kind"]["enum"],
        json!(["hybrid_scene", "shader_background"])
    );
    assert_eq!(
        gpu_visuals["properties"]["qualityProfile"]["enum"],
        json!(supported_profile_ids())
    );
}

#[test]
fn proposal_validation_accepts_other_known_template_overlay() {
    let project = sample_project();
    let request = sample_edit_request();
    let mut proposal = valid_template_proposal();
    proposal.overlays = vec![serde_json::json!({
        "kind": "overlay",
        "templateId": "metric-callout-v1",
        "startSeconds": 1.0,
        "durationSeconds": 2.2,
        "fields": { "headline": "42%", "subline": "faster workflow" },
        "brief": "Emphasize the metric.",
        "visualTreatment": "floating metric tile with high-contrast number",
        "motion": "count-up feel and accent sweep",
        "safeZone": "keep tile inside 10% margins",
        "avoid": "spreadsheet-like boxes"
    })];

    validate_codex_edit_proposal(&project, &request, &proposal)
        .expect("known metric template overlay should pass");
}

#[test]
fn proposal_validation_accepts_holographic_logo_template_overlay() {
    let project = sample_project();
    let request = sample_edit_request();
    let mut proposal = valid_template_proposal();
    proposal.overlays = vec![serde_json::json!({
        "kind": "overlay",
        "templateId": "holographic-logo-cutout-v1",
        "startSeconds": 0.0,
        "durationSeconds": 3.2,
        "fields": { "logoAssetId": "builtin:v-photo-light" },
        "brief": "Reveal the built-in logo as a holographic cutout.",
        "visualTreatment": "dark gradient title card with holographic logo cutout",
        "motion": "shader shimmer through a crisp logo mask",
        "safeZone": "keep logo inside the central 80% safe zone",
        "avoid": "plain boxes and static text-only cards"
    })];

    validate_codex_edit_proposal(&project, &request, &proposal)
        .expect("holographic logo template overlay should pass");
}

#[test]
fn proposal_validation_accepts_gradient_background_loop_template_overlay() {
    let project = sample_project();
    let request = sample_edit_request();
    let mut proposal = valid_template_proposal();
    proposal.overlays = vec![serde_json::json!({
        "kind": "overlay",
        "templateId": "gradient-background-loop-v1",
        "startSeconds": 0.0,
        "durationSeconds": 4.0,
        "fields": { "headline": "Love\nwins." },
        "brief": "Show a bold text card over looping blue gradient panels.",
        "visualTreatment": "full-frame vertical blue gradient panels with oversized white text",
        "motion": "seamless vertical gradient loop with subtle panel drift",
        "safeZone": "keep text inside 10% margins",
        "avoid": "plain boxes, opaque caption slabs, and static default-font title cards"
    })];

    validate_codex_edit_proposal(&project, &request, &proposal)
        .expect("gradient background loop template overlay should pass");
}

#[test]
fn proposal_validation_rejects_unknown_template_ids() {
    let project = sample_project();
    let request = sample_edit_request();
    let mut proposal = valid_template_proposal();
    proposal.overlays = vec![serde_json::json!({
        "kind": "lower_third",
        "templateId": "missing-template",
        "startSeconds": 1.0,
        "durationSeconds": 2.4,
        "fields": { "headline": "Olha API", "subline": "Founder" },
        "brief": "Introduce the speaker.",
        "visualTreatment": "compact lower-third block with translucent backing",
        "motion": "slide-and-fade in over 8 frames",
        "safeZone": "keep essential text inside 10% margins",
        "avoid": "full-width opaque black slabs"
    })];

    let error = validate_codex_edit_proposal(&project, &request, &proposal)
        .expect_err("unknown template must fail");

    assert!(error.to_string().contains("unknown motion template"));
}

#[test]
fn proposal_validation_rejects_template_overlays_without_required_fields() {
    let project = sample_project();
    let request = sample_edit_request();
    let mut proposal = valid_template_proposal();
    proposal.overlays = vec![serde_json::json!({
        "kind": "lower_third",
        "templateId": "kinetic-lower-third-v1",
        "startSeconds": 1.0,
        "durationSeconds": 2.4,
        "fields": { "headline": "   ", "subline": "Founder" },
        "brief": "Introduce the speaker.",
        "visualTreatment": "compact lower-third block with translucent backing",
        "motion": "slide-and-fade in over 8 frames",
        "safeZone": "keep essential text inside 10% margins",
        "avoid": "full-width opaque black slabs"
    })];

    let error = validate_codex_edit_proposal(&project, &request, &proposal)
        .expect_err("empty template field must fail");

    assert!(error
        .to_string()
        .contains("template field headline cannot be empty"));
}

#[test]
fn proposal_validation_rejects_template_overlay_without_fields_object() {
    let project = sample_project();
    let request = sample_edit_request();
    let mut proposal = valid_template_proposal();
    proposal.overlays[0]
        .as_object_mut()
        .expect("template overlay object")
        .remove("fields");

    let error = validate_codex_edit_proposal(&project, &request, &proposal)
        .expect_err("template overlay without fields must fail");

    assert_eq!(error, CodexProposalError::MissingTemplateMetadata("fields"));
}

#[test]
fn proposal_validation_rejects_template_overlay_without_visual_metadata() {
    for key in ["visualTreatment", "motion", "safeZone", "avoid"] {
        let project = sample_project();
        let request = sample_edit_request();
        let mut proposal = valid_template_proposal();
        proposal.overlays[0]
            .as_object_mut()
            .expect("template overlay object")
            .remove(key);

        let error = validate_codex_edit_proposal(&project, &request, &proposal)
            .expect_err("template overlay without visual metadata must fail");

        assert_eq!(error, CodexProposalError::MissingTemplateMetadata(key));

        let mut proposal = valid_template_proposal();
        proposal.overlays[0]
            .as_object_mut()
            .expect("template overlay object")
            .insert(key.to_string(), serde_json::json!("   "));

        let error = validate_codex_edit_proposal(&project, &request, &proposal)
            .expect_err("template overlay with blank visual metadata must fail");

        assert_eq!(error, CodexProposalError::MissingTemplateMetadata(key));
    }
}

#[test]
fn proposal_validation_rejects_hyperframe_without_visual_metadata() {
    let project = sample_project();
    let request = sample_edit_request();
    let mut proposal = valid_template_proposal();
    proposal.hyperframes = vec![serde_json::json!({
        "kind": "template_overlay",
        "templateId": "chapter-card-v1",
        "startSeconds": 0.0,
        "durationSeconds": 2.0,
        "brief": "Open with an editorial chapter card.",
        "fields": { "headline": "Opening", "subline": "Setup" }
    })];

    let error = validate_codex_edit_proposal(&project, &request, &proposal)
        .expect_err("hyperframe without visual metadata must fail before render");

    assert!(error
        .to_string()
        .contains("hyperframe metadata is missing: visualTreatment"));
}

#[test]
fn proposal_validation_accepts_title_card_hyperframe_scene() {
    let project = sample_project();
    let request = sample_edit_request();
    let mut proposal = valid_template_proposal();
    proposal.hyperframes = vec![serde_json::json!({
        "kind": "title_card",
        "role": "title_card",
        "startSeconds": 0.0,
        "durationSeconds": 2.0,
        "brief": "Open with an editorial chapter card.",
        "visualTreatment": "full-frame editorial title with transparent motion layers",
        "motion": "fast type-on with a short camera push",
        "safeZone": "keep text inside 10% margins",
        "avoid": "static text-only cards and opaque black slabs"
    })];

    validate_codex_edit_proposal(&project, &request, &proposal)
        .expect("title-card HyperFrame should be accepted before render");
}

#[test]
fn proposal_validation_accepts_lower_third_hyperframe_scene() {
    let project = sample_project();
    let request = sample_edit_request();
    let mut proposal = valid_template_proposal();
    proposal.hyperframes = vec![serde_json::json!({
        "kind": "lower_third",
        "role": "lower_third",
        "startSeconds": 0.5,
        "durationSeconds": 2.0,
        "brief": "Identify the speaker before the quote.",
        "fields": {
            "headline": "Olha API",
            "subline": "Creator and editor"
        },
        "visualTreatment": "compact translucent lower third with cyan accent and strong hierarchy",
        "motion": "slide in, hold, soft fade",
        "safeZone": "keep essential text inside lower-third safe margins",
        "avoid": "full-width opaque black slabs and static name cards"
    })];

    validate_codex_edit_proposal(&project, &request, &proposal)
        .expect("lower-third HyperFrame should be accepted before render");
}

#[test]
fn proposal_validation_accepts_diagram_hyperframe_scene() {
    let project = sample_project();
    let request = sample_edit_request();
    let mut proposal = valid_template_proposal();
    proposal.hyperframes = vec![serde_json::json!({
        "kind": "diagram",
        "role": "diagram",
        "startSeconds": 0.5,
        "durationSeconds": 2.0,
        "brief": "Explain the edit workflow as a compact visual system.",
        "fields": {
            "headline": "Select -> Cut -> Render",
            "subline": "Timeline-native graphics after a real EDL"
        },
        "visualTreatment": "transparent process diagram with three concise labeled nodes",
        "motion": "staggered node reveal with connector wipe",
        "safeZone": "keep all labels inside the center safe area",
        "avoid": "static slide design and paragraph labels"
    })];

    validate_codex_edit_proposal(&project, &request, &proposal)
        .expect("diagram HyperFrame should be accepted before render");
}

#[test]
fn proposal_validation_accepts_transition_hyperframe_scene() {
    let project = sample_project();
    let request = sample_edit_request();
    let mut proposal = valid_template_proposal();
    proposal.hyperframes = vec![serde_json::json!({
        "kind": "transition",
        "role": "transition",
        "startSeconds": 2.0,
        "durationSeconds": 1.0,
        "brief": "Bridge the hook into the proof.",
        "fields": {
            "headline": "Then the proof"
        },
        "visualTreatment": "short full-frame kinetic color wipe with readable cue text",
        "motion": "fast panel wipe with a clean exit",
        "safeZone": "keep text inside center safe area",
        "avoid": "long static cards and plain template slides"
    })];

    validate_codex_edit_proposal(&project, &request, &proposal)
        .expect("transition HyperFrame should be accepted before render");
}

#[test]
fn proposal_validation_accepts_immersive_hyperframe_scene() {
    let project = sample_project();
    let request = sample_edit_request();
    let mut proposal = valid_template_proposal();
    proposal.hyperframes = vec![serde_json::json!({
        "kind": "immersive_scene",
        "role": "scene",
        "startSeconds": 0.0,
        "durationSeconds": 2.0,
        "brief": "Open with a dimensional generated scene.",
        "fields": {
            "headline": "Inside the cut"
        },
        "visualTreatment": "full-frame immersive editorial scene with dimensional color panels",
        "motion": "slow parallax drift with clean entrance and exit",
        "safeZone": "keep cue text inside center safe area",
        "avoid": "static slide design and opaque black slabs"
    })];

    validate_codex_edit_proposal(&project, &request, &proposal)
        .expect("immersive HyperFrame should be accepted before render");
}

#[test]
fn proposal_validation_applies_project_actions_to_a_project_clone() {
    let project = sample_project();
    let request = sample_edit_request();
    let mut proposal = valid_template_proposal();
    proposal.project_actions = vec![ProjectAction::EditTranscriptWords {
        edits: vec![ProjectActionTranscriptWordEdit {
            transcript_id: "transcript-1".to_string(),
            word_index: 0,
            text: Some("Sharper".to_string()),
            start_seconds: None,
            end_seconds: None,
            repair_id: "repair-1".to_string(),
            created_at: "2026-06-12T00:00:00Z".to_string(),
        }],
    }];

    validate_codex_edit_proposal(&project, &request, &proposal)
        .expect("valid project action should be accepted");
    assert_eq!(project.transcripts[0].words[0].text, "Strong");
}

#[test]
fn proposal_validation_rejects_invalid_project_actions() {
    let project = sample_project();
    let request = sample_edit_request();
    let mut proposal = valid_template_proposal();
    proposal.project_actions = vec![ProjectAction::EditTranscriptWords {
        edits: vec![ProjectActionTranscriptWordEdit {
            transcript_id: "transcript-1".to_string(),
            word_index: 99,
            text: Some("Missing".to_string()),
            start_seconds: None,
            end_seconds: None,
            repair_id: "repair-2".to_string(),
            created_at: "2026-06-12T00:00:00Z".to_string(),
        }],
    }];

    let error = validate_codex_edit_proposal(&project, &request, &proposal)
        .expect_err("invalid project action should be rejected");

    assert!(error.to_string().contains("transcript word was not found"));
}

#[test]
fn proposal_validation_rejects_known_template_overlay_with_wrong_kind() {
    let project = sample_project();
    let request = sample_edit_request();
    let mut proposal = valid_template_proposal();
    proposal.overlays[0]
        .as_object_mut()
        .expect("template overlay object")
        .insert("kind".to_string(), serde_json::json!("transition"));

    let error = validate_codex_edit_proposal(&project, &request, &proposal)
        .expect_err("template overlay with unsupported kind must fail");

    assert_eq!(
        error,
        CodexProposalError::UnsupportedTemplateKind("transition".to_string())
    );
}

#[test]
fn proposal_validation_rejects_known_template_overlay_with_zero_or_negative_duration() {
    for duration_seconds in [0.0, -1.0] {
        let project = sample_project();
        let request = sample_edit_request();
        let mut proposal = valid_template_proposal();
        proposal.overlays[0]
            .as_object_mut()
            .expect("template overlay object")
            .insert(
                "durationSeconds".to_string(),
                serde_json::json!(duration_seconds),
            );

        let error = validate_codex_edit_proposal(&project, &request, &proposal)
            .expect_err("template overlay with invalid duration must fail");

        assert_eq!(
            error,
            CodexProposalError::InvalidTemplateTiming(
                "durationSeconds must be finite and greater than zero"
            )
        );
    }
}

#[test]
fn proposal_validation_rejects_known_template_overlay_with_negative_start() {
    let project = sample_project();
    let request = sample_edit_request();
    let mut proposal = valid_template_proposal();
    proposal.overlays[0]
        .as_object_mut()
        .expect("template overlay object")
        .insert("startSeconds".to_string(), serde_json::json!(-0.1));

    let error = validate_codex_edit_proposal(&project, &request, &proposal)
        .expect_err("template overlay with invalid start must fail");

    assert_eq!(
        error,
        CodexProposalError::InvalidTemplateTiming("startSeconds must be finite and non-negative")
    );
}

#[test]
fn proposal_validation_rejects_known_template_overlay_that_exceeds_render_duration() {
    let project = sample_project();
    let request = sample_edit_request();
    let mut proposal = valid_template_proposal();
    let overlay = proposal.overlays[0]
        .as_object_mut()
        .expect("template overlay object");
    overlay.insert("startSeconds".to_string(), serde_json::json!(44.0));
    overlay.insert("durationSeconds".to_string(), serde_json::json!(2.0));

    let error = validate_codex_edit_proposal(&project, &request, &proposal)
        .expect_err("template overlay exceeding render duration must fail");

    assert_eq!(
        error,
        CodexProposalError::InvalidTemplateTiming("overlay must fit within render duration")
    );
}

#[test]
fn proposal_validation_uses_edl_duration_for_template_overlay_timing() {
    let project = sample_project();
    let request = sample_edit_request();
    let mut proposal = valid_template_proposal();
    proposal.clips[0].source_in = 1.0;
    proposal.clips[0].source_out = 31.0;
    proposal.render_review.duration_seconds = 30.0;
    let overlay = proposal.overlays[0]
        .as_object_mut()
        .expect("template overlay object");
    overlay.insert("startSeconds".to_string(), serde_json::json!(31.0));
    overlay.insert("durationSeconds".to_string(), serde_json::json!(2.4));

    let error = validate_codex_edit_proposal(&project, &request, &proposal)
        .expect_err("overlay beyond computed EDL duration must fail");

    assert_eq!(
        error,
        CodexProposalError::InvalidTemplateTiming("overlay must fit within render duration")
    );
}

#[test]
fn proposal_validation_requires_render_review_duration_to_match_edl_duration() {
    let project = sample_project();
    let request = sample_edit_request();
    let mut proposal = valid_template_proposal();
    proposal.clips[0].source_in = 1.0;
    proposal.clips[0].source_out = 31.0;
    proposal.render_review.duration_seconds = 45.0;
    proposal.captions = Vec::new();
    proposal.overlays = Vec::new();
    proposal.hyperframes = Vec::new();

    let error = validate_codex_edit_proposal(&project, &request, &proposal)
        .expect_err("render review duration must match the selected EDL duration");

    assert_eq!(
        error,
        CodexProposalError::InvalidRenderReviewDuration(
            "renderReview.durationSeconds must match selected EDL duration"
        )
    );
}

#[test]
fn proposal_validation_requires_visual_frame_evidence_review() {
    let project = sample_project();
    let request = sample_edit_request();
    let mut proposal = valid_template_proposal();
    proposal.render_review.visual_frame_evidence_required = false;

    let error = validate_codex_edit_proposal(&project, &request, &proposal)
        .expect_err("render review must require visual frame evidence");

    assert_eq!(error, CodexProposalError::IncompleteRenderReview);
}

#[test]
fn proposal_validation_uses_edl_duration_for_caption_timing() {
    let project = sample_project();
    let request = sample_edit_request();
    let mut proposal = valid_template_proposal();
    proposal.clips[0].source_in = 1.0;
    proposal.clips[0].source_out = 31.0;
    proposal.render_review.duration_seconds = 30.0;
    proposal.captions = vec![serde_json::json!({
        "id": "caption-too-late",
        "text": "This caption starts after the selected EDL is over.",
        "startSeconds": 31.0,
        "durationSeconds": 2.0,
        "sourceIn": 1.0,
        "sourceOut": 3.0,
        "visualTreatment": "bold phone-readable caption with translucent backing",
        "motion": "snap in and settle",
        "safeZone": "keep inside lower third safe area",
        "avoid": "full-width opaque black slabs"
    })];

    let error = validate_codex_edit_proposal(&project, &request, &proposal)
        .expect_err("caption beyond computed EDL duration must fail");

    assert_eq!(
        error,
        CodexProposalError::InvalidCaptionTiming("caption must fit within render duration")
    );
}

#[test]
fn proposal_validation_uses_edl_duration_for_hyperframe_timing() {
    let project = sample_project();
    let request = sample_edit_request();
    let mut proposal = valid_template_proposal();
    proposal.clips[0].source_in = 1.0;
    proposal.clips[0].source_out = 31.0;
    proposal.render_review.duration_seconds = 30.0;
    proposal.overlays = Vec::new();
    proposal.hyperframes = vec![serde_json::json!({
        "kind": "title_card",
        "role": "title_card",
        "startSeconds": 31.0,
        "durationSeconds": 2.0,
        "brief": "This title card starts after the selected EDL is over.",
        "visualTreatment": "full-frame editorial title with transparent motion layers",
        "motion": "fast type-on with a short camera push",
        "safeZone": "keep title inside 10% margins",
        "avoid": "static text-only cards"
    })];

    let error = validate_codex_edit_proposal(&project, &request, &proposal)
        .expect_err("hyperframe beyond computed EDL duration must fail");

    assert_eq!(
        error,
        CodexProposalError::InvalidHyperframeTiming("hyperframe must fit within render duration")
    );
}

#[test]
fn proposal_validation_uses_edl_duration_for_gpu_visual_timing() {
    let project = sample_project();
    let request = sample_edit_request();
    let mut proposal = valid_template_proposal();
    proposal.clips[0].source_in = 1.0;
    proposal.clips[0].source_out = 31.0;
    proposal.render_review.duration_seconds = 30.0;
    proposal.gpu_visuals = vec![serde_json::json!({
        "id": "shader-hook-bg",
        "kind": "hybrid_scene",
        "startSeconds": 29.5,
        "durationSeconds": 2.0,
        "qualityProfile": "hq-neon-wireframe-shader-v1",
        "sourceBeat": "open with a generated shader hook",
        "visualTreatment": "procedural gradient field with a rotating cube",
        "motion": "slow shader drift with cube rotation",
        "safeZone": "center stays low contrast",
        "avoid": "strobing and tiny high-frequency noise"
    })];

    let error = validate_codex_edit_proposal(&project, &request, &proposal)
        .expect_err("gpu visual beyond computed EDL duration must fail");

    assert_eq!(
        error,
        CodexProposalError::InvalidGpuVisualTiming("gpu visual must fit within render duration")
    );
}

#[test]
fn proposal_validation_reports_edl_errors_before_template_overlay_errors() {
    let project = sample_project();
    let request = sample_edit_request();
    let mut proposal = valid_template_proposal();
    proposal.clips[0].source_out = 121.0;
    proposal.overlays[0]
        .as_object_mut()
        .expect("template overlay object")
        .insert("kind".to_string(), serde_json::json!("transition"));

    let error = validate_codex_edit_proposal(&project, &request, &proposal)
        .expect_err("invalid EDL should fail before invalid template overlay");

    assert!(matches!(error, CodexProposalError::Edl(_)));
}

#[test]
fn proposal_validation_allows_generic_overlay_without_template_id() {
    let project = sample_project();
    let request = sample_edit_request();
    let mut proposal = valid_template_proposal();
    let overlay = proposal.overlays[0]
        .as_object_mut()
        .expect("template overlay object");
    overlay.remove("templateId");
    overlay.remove("fields");

    let edl = validate_codex_edit_proposal(&project, &request, &proposal)
        .expect("generic overlay without template id should pass");

    assert_eq!(edl.clips.len(), 1);
}

#[test]
fn proposal_validation_rejects_generic_overlay_with_banned_visual_treatment() {
    let project = sample_project();
    let request = sample_edit_request();
    let mut proposal = valid_template_proposal();
    let overlay = proposal.overlays[0]
        .as_object_mut()
        .expect("template overlay object");
    overlay.remove("templateId");
    overlay.remove("fields");
    overlay.insert("kind".to_string(), serde_json::json!("overlay"));
    overlay.insert(
        "visualTreatment".to_string(),
        serde_json::json!("full-width opaque black caption slab with centered default text"),
    );
    overlay.insert(
        "avoid".to_string(),
        serde_json::json!("small translucent callouts that keep faces visible"),
    );

    let error = validate_codex_edit_proposal(&project, &request, &proposal)
        .expect_err("generic black-slab overlay treatment must fail");

    assert!(error
        .to_string()
        .contains("visual treatment is not allowed"));
}

#[test]
fn proposal_validation_rejects_generic_hyperframe_visual_layer() {
    let project = sample_project();
    let request = sample_edit_request();
    let mut proposal = valid_template_proposal();
    proposal.hyperframes.push(serde_json::json!({
        "kind":"title_card","role":"title_card","startSeconds":0.0,"durationSeconds":1.0,
        "brief":"Generic title","fields":{"headline":"Title","subline":"Beat"},
        "visualTreatment":"generic visual layer with a plain text box",
        "motion":"quick pop then exit","safeZone":"inside 10% margins","avoid":"covering faces"
    }));

    let error = validate_codex_edit_proposal(&project, &request, &proposal)
        .expect_err("generic HyperFrame treatment must fail");
    assert!(error
        .to_string()
        .contains("visual treatment is not allowed"));
}

#[test]
fn proposal_validation_uses_edl_duration_for_generic_overlay_timing() {
    let project = sample_project();
    let request = sample_edit_request();
    let mut proposal = valid_template_proposal();
    proposal.clips[0].source_in = 1.0;
    proposal.clips[0].source_out = 31.0;
    proposal.render_review.duration_seconds = 30.0;
    let overlay = proposal.overlays[0]
        .as_object_mut()
        .expect("template overlay object");
    overlay.remove("templateId");
    overlay.remove("fields");
    overlay.insert("kind".to_string(), serde_json::json!("overlay"));
    overlay.insert("startSeconds".to_string(), serde_json::json!(31.0));
    overlay.insert("durationSeconds".to_string(), serde_json::json!(2.4));

    let error = validate_codex_edit_proposal(&project, &request, &proposal)
        .expect_err("generic overlay beyond computed EDL duration must fail");

    assert_eq!(
        error,
        CodexProposalError::InvalidOverlayTiming("overlay must fit within render duration")
    );
}

#[test]
fn proposal_validation_accepts_known_template_overlay_kind_synonym() {
    let project = sample_project();
    let request = sample_edit_request();
    let mut proposal = valid_template_proposal();
    proposal.overlays[0]
        .as_object_mut()
        .expect("template overlay object")
        .insert("kind".to_string(), serde_json::json!("overlay"));

    let edl = validate_codex_edit_proposal(&project, &request, &proposal)
        .expect("known template overlay kind synonym should pass");

    assert_eq!(edl.clips.len(), 1);
}

#[test]
fn proposal_validation_accepts_known_template_overlay_after_valid_edl() {
    let project = sample_project();
    let request = sample_edit_request();
    let proposal = valid_template_proposal();

    let edl = validate_codex_edit_proposal(&project, &request, &proposal)
        .expect("known template overlay should pass");

    assert_eq!(edl.clips.len(), 1);
}

#[test]
fn start_codex_video_edit_turn_sends_thread_then_turn_with_skills() {
    let mut project = sample_project();
    let request = sample_edit_request();
    let bundle = sample_skill_bundle();
    let mut transport = FakeTransport {
        responses: vec![
            json!({"ok": true}),
            json!({"thread": {"id": "thread-new"}}),
            json!({"accepted": true}),
        ],
        requests: Vec::new(),
        messages: Default::default(),
    };

    let result = start_codex_video_edit_turn(
        &mut transport,
        41,
        "/Users/olhapi/Documents/video-creater",
        &mut project,
        request,
        &bundle,
        None,
    )
    .expect("start turn");

    assert_eq!(result.thread_id, "thread-new");
    assert_eq!(project.codex_thread_id.as_deref(), Some("thread-new"));
    assert_eq!(transport.requests.len(), 3);
    assert_eq!(transport.requests[0]["method"], json!("initialize"));
    assert_eq!(transport.requests[1]["method"], json!("thread/start"));
    assert!(transport.requests[1]["params"]["developerInstructions"]
        .as_str()
        .expect("developer instructions")
        .contains("video-creater-video-pipeline"));
    assert_eq!(transport.requests[2]["method"], json!("turn/start"));
    assert_eq!(
        transport.requests[2]["params"]["threadId"],
        json!("thread-new")
    );
}

#[test]
fn start_codex_video_edit_turn_extracts_structured_proposal() {
    let mut project = sample_project();
    let request = sample_edit_request();
    let bundle = sample_skill_bundle();
    let proposal = valid_template_proposal();
    let mut transport = FakeTransport {
        responses: vec![
            json!({"ok": true}),
            json!({"thread": {"id": "thread-new"}}),
            json!({"structuredOutput": proposal}),
        ],
        requests: Vec::new(),
        messages: Default::default(),
    };

    let result = start_codex_video_edit_turn(
        &mut transport,
        41,
        "/Users/olhapi/Documents/video-creater",
        &mut project,
        request,
        &bundle,
        None,
    )
    .expect("start turn");

    assert_eq!(result.proposal, Some(valid_template_proposal()));
}

#[test]
fn start_codex_video_edit_turn_returns_invalid_proposal_with_structured_issue() {
    let mut project = sample_project();
    let request = sample_edit_request();
    let bundle = sample_skill_bundle();
    let mut proposal = valid_template_proposal();
    proposal.clips = vec![CodexProposalClip {
        media_id: "media-1".to_string(),
        source_in: 0.0,
        source_out: 120.0,
        reason: "full source pass-through".to_string(),
    }];
    proposal.render_review.duration_seconds = 120.0;
    let mut transport = FakeTransport {
        responses: vec![
            json!({"ok": true}),
            json!({"thread": {"id": "thread-invalid"}}),
            json!({"structuredOutput": proposal.clone()}),
        ],
        requests: Vec::new(),
        messages: Default::default(),
    };

    let result = start_codex_video_edit_turn(
        &mut transport,
        41,
        "/Users/olhapi/Documents/video-creater",
        &mut project,
        request,
        &bundle,
        None,
    )
    .expect("invalid structured proposal should return review issues");

    assert_eq!(result.proposal, Some(proposal));
    assert_eq!(
        result.proposal_validation_issues,
        Some(vec![ProjectValidationIssue {
            path: "clips".to_string(),
            message: "proposal EDL is invalid: one-click edit cannot pass through the full source as one clip".to_string(),
            fix: "Select a real rough cut that omits source material before adding visual layers".to_string(),
        }])
    );
}

#[test]
fn start_codex_video_edit_turn_persists_app_server_conversation_for_split_project() {
    let temp = tempfile::tempdir().expect("project temp dir");
    let project_dir = temp.path().join("project");
    let mut project = sample_project();
    save_split_project(&project_dir, &project).expect("save split project");
    let request = sample_edit_request();
    let bundle = sample_skill_bundle();
    let proposal = valid_template_proposal();
    let mut transport = FakeTransport {
        responses: vec![
            json!({"ok": true}),
            json!({"thread": {"id": "thread-new"}}),
            json!({
                "turn": { "id": "turn-1", "status": "completed" },
                "structuredOutput": proposal
            }),
        ],
        requests: Vec::new(),
        messages: Default::default(),
    };

    start_codex_video_edit_turn(
        &mut transport,
        41,
        "/Users/olhapi/Documents/video-creater",
        &mut project,
        request,
        &bundle,
        Some(&project_dir),
    )
    .expect("start turn");

    let conversations_path = project_dir
        .join("context")
        .join("app-server-conversations.json");
    let conversations: Value = serde_json::from_str(
        &fs::read_to_string(&conversations_path).expect("conversation history file"),
    )
    .expect("conversation history json");
    let entries = conversations["entries"]
        .as_array()
        .expect("conversation entries");

    assert_eq!(conversations["schemaVersion"], json!(1));
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0]["threadId"], json!("thread-new"));
    assert_eq!(entries[0]["turnId"], json!("turn-1"));
    assert_eq!(entries[0]["turnStatus"], json!("completed"));
    assert_eq!(
        entries[0]["prompt"],
        json!("Make an action edit with bold captions")
    );
    assert_eq!(entries[0]["request"]["preset"], json!("trailer_cut"));
    assert_eq!(entries[0]["hasProposal"], json!(true));
}

#[test]
fn decode_app_server_response_result_unwraps_json_rpc_wire_response() {
    let result = decode_app_server_response_result(
        41,
        json!({"id": 41, "result": {"thread": {"id": "thread-new"}}}),
    )
    .expect("decode response");

    assert_eq!(result["thread"]["id"], json!("thread-new"));
}

#[test]
fn decode_app_server_response_result_reports_json_rpc_errors() {
    let error = decode_app_server_response_result(
        41,
        json!({"id": 41, "error": {"code": -32000, "message": "bad request"}}),
    )
    .expect_err("wire error should fail");

    assert!(error.to_string().contains("bad request"));
}

#[test]
fn project_skill_bundle_loads_repo_skills() {
    let repo_root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("repo root");
    let bundle = load_project_skill_bundle(repo_root).expect("load skills");

    assert!(bundle.agents_md.contains("Video Creater Skill Policy"));
    assert!(bundle.video_pipeline.contains("EDL-first"));
    assert!(bundle.graphics.contains("Generated graphics"));
    assert!(bundle.visuals.contains("working video editor"));
    assert!(bundle
        .shader_background_catalog
        .contains("shadertoy-octagrams-v1"));
    assert!(bundle
        .shader_background_catalog
        .contains("sourceKind built_in"));
}

struct FakeTransport {
    responses: Vec<serde_json::Value>,
    requests: Vec<serde_json::Value>,
    messages: std::collections::VecDeque<video_creater_lib::codex::app_server::AppServerMessage>,
}

impl CodexAppServerTransport for FakeTransport {
    fn send(
        &mut self,
        request: serde_json::Value,
    ) -> Result<(), video_creater_lib::codex::app_server::CodexAppServerError> {
        let id = request["id"].clone();
        let method = request["method"].as_str().unwrap_or_default().to_string();
        let thread_id = request
            .pointer("/params/threadId")
            .cloned()
            .unwrap_or(json!("thread-1"));
        self.requests.push(request);
        let result = self.responses.remove(0);
        if method == "turn/start" {
            let turn_id = result
                .pointer("/turn/id")
                .cloned()
                .unwrap_or(json!("turn-fixture"));
            self.messages.push_back(
                video_creater_lib::codex::app_server::AppServerMessage::Response {
                    id,
                    result: json!({"turn":{"id":turn_id,"items":[],"status":"inProgress"}}),
                },
            );
            self.messages.push_back(video_creater_lib::codex::app_server::AppServerMessage::Notification { method:"turn/completed".into(), params:json!({"threadId":thread_id,"turn":{"id":turn_id,"status":"completed","items":[{"id":"message-fixture","type":"agentMessage","phase":"final_answer","text":serde_json::to_string(&result).unwrap()}]}}) });
        } else {
            self.messages.push_back(
                video_creater_lib::codex::app_server::AppServerMessage::Response { id, result },
            );
        }
        Ok(())
    }
    fn recv_until(
        &mut self,
        _deadline: std::time::Instant,
    ) -> Result<
        video_creater_lib::codex::app_server::AppServerMessage,
        video_creater_lib::codex::app_server::CodexAppServerError,
    > {
        self.messages.pop_front().ok_or_else(|| {
            video_creater_lib::codex::app_server::CodexAppServerError::Transport(
                "missing fake Codex response".into(),
            )
        })
    }
    fn terminate(
        &mut self,
    ) -> Result<
        video_creater_lib::codex::app_server::AppServerCleanupReport,
        video_creater_lib::codex::app_server::CodexAppServerError,
    > {
        Ok(Default::default())
    }
}

fn sample_skill_bundle() -> ProjectSkillBundle {
    ProjectSkillBundle {
        agents_md: "## Video Creater Skill Policy\n## App-Server Video Generation Contract"
            .to_string(),
        video_pipeline: "# Video Creater Video Pipeline\nThe app is EDL-first.".to_string(),
        graphics: "# Video Creater Graphics\nGenerated graphics are timeline assets.".to_string(),
        visuals: "# Video Creater Visuals\nDesign the app as a working video editor.".to_string(),
        shader_background_catalog:
            "- shadertoy-octagrams-v1: Octagrams. kind shader_background. sourceKind built_in"
                .to_string(),
    }
}

fn sample_edit_request() -> EditJobRequest {
    EditJobRequest {
        media_id: "media-1".to_string(),
        preset: EditPreset::TrailerCut,
        prompt: "Make an action edit with bold captions".to_string(),
        target_duration_seconds: Some(45.0),
        language_mode: LanguageMode::English,
        caption_style: CaptionStyle::Bold,
        created_at: "2026-06-12T00:00:00Z".to_string(),
    }
}

fn sample_project() -> VideoProject {
    let mut project = VideoProject::new_empty(
        "project-1".to_string(),
        "Codex Test".to_string(),
        "2026-06-12T00:00:00Z".to_string(),
    );
    project.media.push(MediaAsset {
        id: "media-1".to_string(),
        name: None,
        relative_path: "media/input.mp4".to_string(),
        kind: MediaKind::Video,
        duration_seconds: 120.0,
        width: Some(1080),
        height: Some(1920),
        fps: Some(30.0),
        folder_id: None,
    });
    project.transcripts.push(Transcript {
        id: "transcript-1".to_string(),
        media_id: "media-1".to_string(),
        engine: Some("nvidia/parakeet-tdt-0.6b-v3".to_string()),
        raw_artifact_path: None,
        repairs: Vec::new(),
        segments: Vec::new(),
        words: vec![
            TranscriptWord {
                text: "Strong".to_string(),
                start_seconds: 1.0,
                end_seconds: 1.4,
                confidence: Some(0.95),
                speaker: None,
            },
            TranscriptWord {
                text: "opening".to_string(),
                start_seconds: 1.4,
                end_seconds: 1.9,
                confidence: Some(0.93),
                speaker: None,
            },
        ],
    });
    project
}

fn sample_generated_asset() -> GeneratedAsset {
    GeneratedAsset {
        schema_version: 1,
        id: "generated-shot-1".to_string(),
        kind: MediaKind::Generated,
        status: GeneratedAssetStatus::Completed,
        name: Some("Generated shot".to_string()),
        target_folder_id: None,
        placement_intent: Some("library".to_string()),
        prompt: "A product closeup with kinetic lighting".to_string(),
        model: GenerationModel {
            provider: "fal".to_string(),
            id: "wan-v2".to_string(),
        },
        references: GeneratedAssetReferences {
            media_ids: vec!["media-1".to_string()],
            first_frame_media_id: Some("media-1".to_string()),
            last_frame_media_id: None,
            provider_input_urls: Vec::new(),
            ..Default::default()
        },
        settings: GeneratedAssetSettings {
            width: Some(1080),
            height: Some(1920),
            duration_seconds: Some(4.0),
            fps: Some(24.0),
            aspect_ratio: Some("9:16".to_string()),
            resolution: None,
            generate_audio: None,
            ..GeneratedAssetSettings::default()
        },
        outputs: vec![GeneratedAssetOutput {
            media_id: "generated-shot-1-output".to_string(),
            relative_path: "generated/generated-shot-1/output.mp4".to_string(),
            source_url: None,
            width: 1080,
            height: 1920,
            duration_seconds: 4.0,
            fps: 24.0,
        }],
        created_at: "2026-06-28T10:00:00Z".to_string(),
        parent_asset_id: None,
        retry_of_asset_id: None,
    }
}

fn sample_job(id: &str, kind: &str) -> JobSummary {
    JobSummary {
        id: id.to_string(),
        kind: kind.to_string(),
        status: JobStatus::Queued,
        updated_at: "2026-06-28T10:00:00Z".to_string(),
        workflow: None,
        start_request: None,
        provider_request: None,
        failure_reason: None,
        export_settings: None,
    }
}

fn sample_render_report() -> ProjectRenderReport {
    ProjectRenderReport {
        schema_version: 1,
        id: "render-draft-1".to_string(),
        status: RenderReportStatus::Completed,
        output_path: "renders/render-draft-1/output.webm".to_string(),
        duration_seconds: 4.0,
        quality: None,
        requested_width: None,
        requested_height: None,
        actual_width: None,
        actual_height: None,
        streams: RenderReportStreams {
            video: true,
            audio: true,
        },
        checks: BTreeMap::from([
            ("duration".to_string(), RenderReportCheckStatus::Passed),
            (
                "captionAlignment".to_string(),
                RenderReportCheckStatus::Skipped,
            ),
            ("overlayTiming".to_string(), RenderReportCheckStatus::Passed),
            (
                "visualFrameEvidence".to_string(),
                RenderReportCheckStatus::Skipped,
            ),
            ("artifactPaths".to_string(), RenderReportCheckStatus::Passed),
            ("streams".to_string(), RenderReportCheckStatus::Passed),
            ("logPath".to_string(), RenderReportCheckStatus::Passed),
        ]),
        artifacts: vec!["renders/render-draft-1/output.webm".to_string()],
        preview_comparison_request: None,
        preview_comparison: None,
        log_path: "logs/render-draft-1.log".to_string(),
        created_at: "2026-06-28T10:00:00Z".to_string(),
    }
}

fn assert_openai_strict_schema(schema: &Value, path: &str) {
    let mut errors = Vec::new();
    collect_openai_strict_schema_errors(schema, path, &mut errors);
    assert!(
        errors.is_empty(),
        "Codex response schema is not OpenAI strict compatible:\n{}",
        errors.join("\n")
    );
}

fn collect_openai_strict_schema_errors(schema: &Value, path: &str, errors: &mut Vec<String>) {
    if schema.get("oneOf").is_some() {
        errors.push(format!("{path} must not use oneOf"));
    }

    if schema_type_contains(schema, "array") && schema.get("items").is_none() {
        errors.push(format!("{path} array schema must declare items"));
    }

    if schema_type_contains(schema, "object")
        && schema.get("additionalProperties") != Some(&json!(false))
    {
        errors.push(format!(
            "{path} object schema must set additionalProperties false"
        ));
    }

    if let Some(properties) = schema.get("properties").and_then(Value::as_object) {
        match schema.get("required").and_then(Value::as_array) {
            Some(required) => {
                for key in properties.keys() {
                    if !required.contains(&json!(key)) {
                        errors.push(format!("{path} required must include property {key}"));
                    }
                }
                for required_key in required {
                    let Some(required_key) = required_key.as_str() else {
                        errors.push(format!("{path} required entries must be strings"));
                        continue;
                    };
                    if !properties.contains_key(required_key) {
                        errors.push(format!(
                            "{path} required key {required_key} must have a matching property"
                        ));
                    }
                }
            }
            None => errors.push(format!("{path} object schema must declare required")),
        }
    }

    if let Some(properties) = schema.get("properties").and_then(Value::as_object) {
        for (key, property_schema) in properties {
            collect_openai_strict_schema_errors(
                property_schema,
                &format!("{path}.properties.{key}"),
                errors,
            );
        }
    }
    if let Some(items) = schema.get("items") {
        collect_openai_strict_schema_errors(items, &format!("{path}.items"), errors);
    }
    for keyword in ["anyOf", "allOf"] {
        if let Some(variants) = schema.get(keyword).and_then(Value::as_array) {
            for (index, variant) in variants.iter().enumerate() {
                collect_openai_strict_schema_errors(
                    variant,
                    &format!("{path}.{keyword}[{index}]"),
                    errors,
                );
            }
        }
    }
}

fn schema_type_contains(schema: &Value, expected: &str) -> bool {
    match schema.get("type") {
        Some(Value::String(schema_type)) => schema_type == expected,
        Some(Value::Array(schema_types)) => schema_types.iter().any(|schema_type| {
            schema_type
                .as_str()
                .is_some_and(|schema_type| schema_type == expected)
        }),
        _ => false,
    }
}

struct IsolatedExportRuntimeEnv {
    original_path: Option<std::ffi::OsString>,
}

impl IsolatedExportRuntimeEnv {
    fn new(path: &std::path::Path) -> Self {
        let original_path = std::env::var_os("PATH");
        for env_var in EXPORT_RUNTIME_ENV_VARS {
            std::env::remove_var(env_var);
        }
        std::env::remove_var("VIDEO_CREATER_ENABLE_LOCAL_EXPORT_PROFILES");
        std::env::set_var("PATH", path);
        Self { original_path }
    }
}

impl Drop for IsolatedExportRuntimeEnv {
    fn drop(&mut self) {
        for env_var in EXPORT_RUNTIME_ENV_VARS {
            std::env::remove_var(env_var);
        }
        std::env::remove_var("VIDEO_CREATER_ENABLE_LOCAL_EXPORT_PROFILES");
        if let Some(path) = &self.original_path {
            std::env::set_var("PATH", path);
        } else {
            std::env::remove_var("PATH");
        }
    }
}

fn executable_fixture(dir: &std::path::Path, name: &str) -> std::path::PathBuf {
    let path = dir.join(name);
    fs::write(&path, b"#!/bin/sh\nexit 0\n").expect("write executable fixture");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&path, fs::Permissions::from_mode(0o755))
            .expect("chmod executable fixture");
    }
    path
}

fn sample_export_artifact() -> ProjectExportArtifact {
    ProjectExportArtifact {
        schema_version: 1,
        id: "export-1".to_string(),
        kind: ProjectExportArtifactKind::NleXml,
        format: "premiereXmeml".to_string(),
        path: "exports/project.xml".to_string(),
        mime_type: "application/xml".to_string(),
        job_id: Some("export-job-1".to_string()),
        created_at: "2026-06-28T10:00:00Z".to_string(),
    }
}

fn valid_template_proposal() -> CodexEditProposal {
    CodexEditProposal {
        media_id: "media-1".to_string(),
        clips: vec![CodexProposalClip {
            media_id: "media-1".to_string(),
            source_in: 1.0,
            source_out: 46.0,
            reason: "strong hook and complete thought".to_string(),
        }],
        captions: Vec::new(),
        overlays: vec![serde_json::json!({
            "kind": "lower_third",
            "templateId": "kinetic-lower-third-v1",
            "startSeconds": 1.0,
            "durationSeconds": 2.4,
            "fields": { "headline": "Olha API", "subline": "Founder" },
            "brief": "Introduce the speaker.",
            "visualTreatment": "compact lower-third block with translucent backing",
            "motion": "slide-and-fade in over 8 frames",
            "safeZone": "keep essential text inside 10% margins",
            "avoid": "full-width opaque black slabs"
        })],
        hyperframes: Vec::new(),
        gpu_visuals: Vec::new(),
        project_actions: Vec::new(),
        render_review: CodexRenderReview {
            duration_seconds: 45.0,
            stream_check_required: true,
            caption_alignment_required: true,
            overlay_timing_required: true,
            visual_frame_evidence_required: true,
            artifact_paths_required: true,
            log_reference_required: true,
        },
    }
}
