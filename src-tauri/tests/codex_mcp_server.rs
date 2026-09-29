use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::io::{Read, Write};
use std::net::TcpListener;
use std::thread;

use serde_json::{json, Value};
use video_creater_lib::codex::mcp_server::{handle_mcp_request, handle_scoped_mcp_request};
use video_creater_lib::codex::tools::{
    call_codex_local_tool, list_codex_local_tools, CodexLocalToolError,
};
use video_creater_lib::generation::elevenlabs::{
    ELEVENLABS_DEFAULT_VOICE, ELEVENLABS_MUSIC_MODEL_ID, ELEVENLABS_PROVIDER,
    ELEVENLABS_TTS_V3_MODEL_ID,
};
use video_creater_lib::generation::fal::{
    FAL_AURA_SR_MODEL_ID, FAL_FLUX_SCHNELL_MODEL_ID, FAL_KLING_V3_PRO_MOTION_CONTROL_MODEL_ID,
    FAL_KREA_2_TURBO_MODEL_ID, FAL_MIRELO_VIDEO_TO_AUDIO_MODEL_ID,
    FAL_NANO_BANANA_PRO_EDIT_MODEL_ID, FAL_PROVIDER, FAL_RECRAFT_V3_TEXT_TO_IMAGE_MODEL_ID,
    FAL_SONILO_TEXT_TO_MUSIC_MODEL_ID, FAL_SONILO_VIDEO_TO_MUSIC_MODEL_ID,
    FAL_VIDEO_UPSCALER_MODEL_ID, FAL_WAN_IMAGE_TO_VIDEO_MODEL_ID,
    FAL_WAN_REFERENCE_TO_VIDEO_MODEL_ID, FAL_WAN_TEXT_TO_VIDEO_MODEL_ID,
    FAL_WAN_VIDEO_TO_VIDEO_MODEL_ID,
};
use video_creater_lib::generation::google::{
    GOOGLE_GEMINI_TTS_DEFAULT_VOICE, GOOGLE_GEMINI_TTS_MODEL_ID, GOOGLE_LYRIA_3_PRO_MODEL_ID,
    GOOGLE_PROVIDER, GOOGLE_VEO_31_FAST_MODEL_ID,
};
use video_creater_lib::generation::minimax::{MINIMAX_MUSIC_MODEL_ID, MINIMAX_PROVIDER};
use video_creater_lib::generation::openai::{
    OPENAI_GPT_4O_MINI_TTS_MODEL_ID, OPENAI_GPT_IMAGE_2_MODEL_ID, OPENAI_GPT_IMAGE_EDIT_MODEL_ID,
    OPENAI_PROVIDER,
};
use video_creater_lib::generation::replicate::{
    REPLICATE_FLUX_DEV_MODEL_ID, REPLICATE_FLUX_SCHNELL_MODEL_ID, REPLICATE_PROVIDER,
    REPLICATE_SEEDANCE_20_FAST_MODEL_ID, REPLICATE_SEEDANCE_20_MODEL_ID,
};
use video_creater_lib::generation::xai::{
    XAI_GROK_IMAGE_QUALITY_MODEL_ID, XAI_GROK_VIDEO_MODEL_ID, XAI_PROVIDER,
};
use video_creater_lib::project::export_profiles::{
    mp4_export_profile_availability_report, ExportProfile,
};
use video_creater_lib::project::fixtures::sample_project;
use video_creater_lib::project::model::{
    GeneratedAsset, GeneratedAssetOutput, GeneratedAssetReferences, GeneratedAssetSettings,
    GeneratedAssetStatus, GenerationModel, MediaAsset, MediaFolder, MediaKind, MediaSilenceRange,
    TimelineItem, TimelineItemKind, TimelineSource, TimelineTrack, TrackKind, Transcript,
    TranscriptWord,
};
use video_creater_lib::project::split::{load_split_project, save_split_project};
use video_creater_lib::project::storage::load_project;

#[path = "codex_mcp_server/audio_edits.rs"]
mod audio_edits;
#[path = "codex_mcp_server/history_ids.rs"]
mod history_ids;
#[path = "codex_mcp_server/reverse.rs"]
mod reverse;
#[path = "codex_mcp_server/transitions.rs"]
mod transitions;
#[path = "codex_mcp_server/undo_generations.rs"]
mod undo_generations;

static EXPORT_PROFILE_ENV_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());
const EXPORT_RUNTIME_ENV_VARS: &[&str] = &[
    "VIDEO_CREATER_APPROVED_H264_ENCODER",
    "VIDEO_CREATER_APPROVED_H265_ENCODER",
    "VIDEO_CREATER_APPROVED_PRORES_ENCODER",
    "VIDEO_CREATER_APPROVED_AAC_ENCODER",
    "VIDEO_CREATER_APPROVED_AUDIO_ENCODER",
];

fn h264_export_available() -> bool {
    mp4_export_profile_availability_report()
        .into_iter()
        .find(|candidate| candidate.profile == ExportProfile::Mp4H264)
        .map(|availability| availability.available)
        .unwrap_or(false)
}

#[test]
fn mcp_initialize_returns_server_capabilities() {
    let project = sample_project();
    let response = handle_mcp_request(
        &project,
        json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "initialize",
            "params": {
                "protocolVersion": "2025-11-25",
                "clientInfo": { "name": "test", "version": "1" }
            }
        }),
    );

    assert_eq!(response["jsonrpc"], json!("2.0"));
    assert_eq!(response["id"], json!(1));
    assert_eq!(
        response["result"]["serverInfo"]["name"],
        json!("video-creater")
    );
    assert_eq!(
        response["result"]["capabilities"]["tools"]["listChanged"],
        json!(false)
    );
    assert_eq!(
        response["result"]["capabilities"]["resources"]["listChanged"],
        json!(false)
    );
    let instructions = response["result"]["instructions"]
        .as_str()
        .expect("server instructions");
    assert!(instructions.contains("Call list_models before generate_video"));
    assert!(instructions.contains("Costs real money and is not undoable"));
    assert!(instructions.contains("placeholder asset ID"));
    assert!(instructions.contains("Video models cannot render readable text"));
}

#[test]
fn mcp_resources_expose_generation_model_catalogs() {
    let project = sample_project();
    let list_response = handle_mcp_request(
        &project,
        json!({
            "jsonrpc": "2.0",
            "id": "resources-list-1",
            "method": "resources/list"
        }),
    );

    let resources = list_response["result"]["resources"]
        .as_array()
        .expect("resources array");
    assert!(resources
        .iter()
        .any(|resource| resource["uri"] == json!("video-creater://models/video")));
    assert!(resources
        .iter()
        .any(|resource| resource["uri"] == json!("video-creater://models/image")));
    assert!(resources
        .iter()
        .any(|resource| resource["uri"] == json!("video-creater://models/audio")));

    let read_response = handle_mcp_request(
        &project,
        json!({
            "jsonrpc": "2.0",
            "id": "resources-read-1",
            "method": "resources/read",
            "params": {
                "uri": "video-creater://models/audio"
            }
        }),
    );

    let contents = read_response["result"]["contents"]
        .as_array()
        .expect("resource contents");
    assert_eq!(contents[0]["uri"], json!("video-creater://models/audio"));
    assert_eq!(contents[0]["mimeType"], json!("application/json"));
    let text = contents[0]["text"].as_str().expect("resource text");
    let models: Vec<Value> = serde_json::from_str(text).expect("audio model JSON");
    assert!(models.iter().any(|model| {
        model["provider"] == json!(ELEVENLABS_PROVIDER)
            && model["id"] == json!(ELEVENLABS_MUSIC_MODEL_ID)
            && model["supportsLyrics"] == json!(true)
    }));
}

#[test]
fn mcp_tools_list_returns_local_tool_descriptors() {
    let project = sample_project();
    let response = handle_mcp_request(
        &project,
        json!({
            "jsonrpc": "2.0",
            "id": "tools-list-1",
            "method": "tools/list"
        }),
    );

    let tools = response["result"]["tools"].as_array().expect("tools array");
    assert!(tools
        .iter()
        .any(|tool| tool["name"] == json!("video_creater.project_context")));
    let project_context = tools
        .iter()
        .find(|tool| tool["name"] == json!("video_creater.project_context"))
        .expect("project context tool");
    assert_eq!(project_context["inputSchema"]["type"], json!("object"));
    assert_eq!(project_context["annotations"]["category"], json!("query"));

    let import_media = tools
        .iter()
        .find(|tool| tool["name"] == json!("video_creater.import_media"))
        .expect("import media tool");
    assert_eq!(import_media["inputSchema"].get("required"), None);
    assert_eq!(
        import_media["inputSchema"]["properties"]["source"]["properties"]["bytes"]["type"],
        json!("string")
    );
    assert_eq!(
        import_media["inputSchema"]["properties"]["source"]["properties"]["mimeType"]["type"],
        json!("string")
    );

    let inspect_media = tools
        .iter()
        .find(|tool| tool["name"] == json!("video_creater.inspect_media"))
        .expect("inspect media tool");
    assert_eq!(
        inspect_media["inputSchema"]["anyOf"],
        json!([{ "required": ["mediaRef"] }, { "required": ["mediaId"] }])
    );
    assert_eq!(
        inspect_media["inputSchema"]["properties"]["mediaId"]["type"],
        json!("string")
    );

    let upscale_media = tools
        .iter()
        .find(|tool| tool["name"] == json!("video_creater.upscale_media"))
        .expect("upscale media tool");
    assert_eq!(
        upscale_media["inputSchema"]["anyOf"],
        json!([{ "required": ["mediaRef"] }, { "required": ["mediaId"] }])
    );
    assert_eq!(
        upscale_media["inputSchema"]["properties"]["folderId"]["type"],
        json!("string")
    );
    assert_eq!(
        upscale_media["inputSchema"]["properties"]["targetFolderId"]["type"],
        json!("string")
    );
    assert_eq!(
        upscale_media["inputSchema"]["properties"]["placementIntent"]["type"],
        json!("string")
    );
    assert_eq!(
        upscale_media["inputSchema"]["properties"]["replacementItemId"]["type"],
        json!("string")
    );

    let generate_video = tools
        .iter()
        .find(|tool| tool["name"] == json!("video_creater.generate_video"))
        .expect("generate video tool");
    assert_eq!(
        generate_video["inputSchema"]["properties"]["folderId"]["type"],
        json!("string")
    );
    assert_eq!(
        generate_video["inputSchema"]["properties"]["referenceImageMediaRefs"]["items"]["type"],
        json!("string")
    );
    assert_eq!(
        generate_video["inputSchema"]["properties"]["duration"]["type"],
        json!("number")
    );
    assert_eq!(generate_video["inputSchema"]["required"], json!(["prompt"]));
    assert_eq!(
        generate_video["inputSchema"]["properties"]["prompt"]["minLength"],
        json!(1)
    );
    let generate_image = tools
        .iter()
        .find(|tool| tool["name"] == json!("video_creater.generate_image"))
        .expect("generate image tool");
    assert_eq!(generate_image["inputSchema"]["required"], json!(["prompt"]));
    assert_eq!(
        generate_image["inputSchema"]["properties"]["prompt"]["minLength"],
        json!(1)
    );
    let generate_audio = tools
        .iter()
        .find(|tool| tool["name"] == json!("video_creater.generate_audio"))
        .expect("generate audio tool");
    assert_eq!(
        generate_audio["inputSchema"]["properties"]["prompt"]["type"],
        json!("string")
    );
    assert_eq!(
        generate_audio["inputSchema"]["properties"]["prompt"].get("minLength"),
        None
    );

    let add_clips = tools
        .iter()
        .find(|tool| tool["name"] == json!("video_creater.add_clips"))
        .expect("add clips tool");
    assert_eq!(
        add_clips["inputSchema"]["anyOf"],
        json!([
            { "required": ["targetTrackId", "clips"] },
            { "required": ["entries"] },
            { "required": ["projectDir", "actions"] }
        ])
    );
    assert_eq!(
        add_clips["inputSchema"]["properties"]["entries"]["items"]["anyOf"],
        json!([{ "required": ["mediaId"] }, { "required": ["mediaRef"] }])
    );

    let insert_clips = tools
        .iter()
        .find(|tool| tool["name"] == json!("video_creater.insert_clips"))
        .expect("insert clips tool");
    assert_eq!(
        insert_clips["inputSchema"]["anyOf"],
        json!([
            { "required": ["targetTrackId", "insertSeconds", "clips"] },
            { "required": ["trackIndex", "atFrame", "entries"] },
            { "required": ["projectDir", "actions"] }
        ])
    );
    assert_eq!(
        insert_clips["inputSchema"]["properties"]["entries"]["items"]["anyOf"],
        json!([{ "required": ["mediaId"] }, { "required": ["mediaRef"] }])
    );

    let move_clips = tools
        .iter()
        .find(|tool| tool["name"] == json!("video_creater.move_clips"))
        .expect("move clips tool");
    assert_eq!(
        move_clips["inputSchema"]["anyOf"],
        json!([
            { "required": ["moves"] },
            { "required": ["projectDir", "actions"] }
        ])
    );
    assert_eq!(
        move_clips["inputSchema"]["properties"]["moves"]["items"]["anyOf"],
        json!([{ "required": ["itemId"] }, { "required": ["clipId"] }])
    );
    assert_eq!(
        move_clips["inputSchema"]["properties"]["moves"]["items"]["properties"]["toFrame"]["type"],
        json!("integer")
    );
    for name in [
        "video_creater.add_clips",
        "video_creater.insert_clips",
        "video_creater.add_texts",
        "video_creater.add_captions",
        "video_creater.move_clips",
        "video_creater.apply_layout",
        "video_creater.remove_clips",
        "video_creater.remove_tracks",
        "video_creater.set_clip_properties",
        "video_creater.set_keyframes",
        "video_creater.ripple_delete_ranges",
        "video_creater.remove_words",
        "video_creater.remove_silence",
        "video_creater.apply_color",
        "video_creater.create_folder",
        "video_creater.move_to_folder",
        "video_creater.rename_folder",
        "video_creater.delete_folder",
        "video_creater.set_project_settings",
        "video_creater.sync_audio",
    ] {
        let tool = tools
            .iter()
            .find(|tool| tool["name"] == json!(name))
            .unwrap_or_else(|| panic!("missing tool descriptor for {name}"));
        let any_of = tool["inputSchema"]["anyOf"]
            .as_array()
            .unwrap_or_else(|| panic!("{name} should declare anyOf"));
        assert!(
            any_of
                .iter()
                .any(|candidate| candidate["required"] == json!(["projectDir", "actions"])),
            "{name} should advertise projectDir + actions"
        );
        assert_eq!(
            tool["inputSchema"]["properties"]["projectDir"]["type"],
            json!("string"),
            "{name} should describe projectDir"
        );
        assert_eq!(
            tool["inputSchema"]["properties"]["actions"]["type"],
            json!("array"),
            "{name} should describe actions"
        );
        assert_eq!(
            tool["inputSchema"]["properties"]["actions"]["minItems"],
            json!(1),
            "{name} should require at least one project action"
        );
        assert_eq!(
            tool["inputSchema"]["properties"]["actions"]["items"]["type"],
            json!("object"),
            "{name} should describe action entries as objects"
        );
    }

    let create_matte = tools
        .iter()
        .find(|tool| tool["name"] == json!("video_creater.create_matte"))
        .expect("create matte tool");
    assert_eq!(create_matte["inputSchema"]["required"], json!(["hex"]));
    assert_eq!(
        create_matte["inputSchema"]["properties"]["aspectRatio"]["enum"],
        json!(["Project", "16:9", "9:16", "1:1", "4:3", "9:14", "2.4:1"])
    );

    let split_clips = tools
        .iter()
        .find(|tool| tool["name"] == json!("video_creater.split_clips"))
        .expect("split clips tool");
    assert_eq!(
        split_clips["inputSchema"]["anyOf"],
        json!([{ "required": ["splits"] }, { "required": ["trackIndex", "frames"] }])
    );
    assert_eq!(
        split_clips["inputSchema"]["properties"]["splits"]["items"]["properties"]["atFrame"]
            ["type"],
        json!("integer")
    );

    let ripple_delete_ranges = tools
        .iter()
        .find(|tool| tool["name"] == json!("video_creater.ripple_delete_ranges"))
        .expect("ripple delete ranges tool");
    assert_eq!(
        ripple_delete_ranges["inputSchema"]["properties"]["itemId"]["type"],
        json!("string")
    );
    assert_eq!(
        ripple_delete_ranges["inputSchema"]["properties"]["ranges"]["items"]["anyOf"][1]
            ["maxItems"],
        json!(2)
    );

    let set_clip_properties = tools
        .iter()
        .find(|tool| tool["name"] == json!("video_creater.set_clip_properties"))
        .expect("set clip properties tool");
    assert_eq!(
        set_clip_properties["inputSchema"]["anyOf"],
        json!([
            { "required": ["updates"] },
            { "required": ["clipIds"] },
            { "required": ["projectDir", "actions"] }
        ])
    );
    assert_eq!(
        set_clip_properties["inputSchema"]["properties"]["clipIds"]["items"]["type"],
        json!("string")
    );
    assert_eq!(
        set_clip_properties["inputSchema"]["properties"]["durationFrames"]["type"],
        json!("integer")
    );
    assert_eq!(
        set_clip_properties["inputSchema"]["properties"]["blendMode"]["enum"],
        json!([
            "normal",
            "darken",
            "multiply",
            "colorBurn",
            "lighten",
            "screen",
            "colorDodge",
            "overlay",
            "softLight",
            "hardLight",
            "difference",
            "exclusion",
            "hue",
            "saturation",
            "color",
            "luminosity"
        ])
    );

    let set_keyframes = tools
        .iter()
        .find(|tool| tool["name"] == json!("video_creater.set_keyframes"))
        .expect("set keyframes tool");
    assert_eq!(
        set_keyframes["inputSchema"]["anyOf"],
        json!([
            { "required": ["itemId", "property", "keyframes"] },
            { "required": ["clipId", "property", "keyframes"] },
            { "required": ["projectDir", "actions"] }
        ])
    );
    assert_eq!(
        set_keyframes["inputSchema"]["properties"]["clipId"]["type"],
        json!("string")
    );

    let remove_clips = tools
        .iter()
        .find(|tool| tool["name"] == json!("video_creater.remove_clips"))
        .expect("remove clips tool");
    assert_eq!(
        remove_clips["inputSchema"]["anyOf"],
        json!([
            { "required": ["itemIds"] },
            { "required": ["clipIds"] },
            { "required": ["projectDir", "actions"] }
        ])
    );

    let remove_tracks = tools
        .iter()
        .find(|tool| tool["name"] == json!("video_creater.remove_tracks"))
        .expect("remove tracks tool");
    assert_eq!(
        remove_tracks["inputSchema"]["anyOf"],
        json!([
            { "required": ["trackIndexes"] },
            { "required": ["trackIds"] },
            { "required": ["projectDir", "actions"] }
        ])
    );
    assert_eq!(
        remove_tracks["inputSchema"]["properties"]["trackIndexes"]["items"]["type"],
        json!("integer")
    );
    assert_eq!(
        remove_tracks["inputSchema"]["properties"]["trackIds"]["items"]["type"],
        json!("string")
    );

    let inspect_timeline = tools
        .iter()
        .find(|tool| tool["name"] == json!("video_creater.inspect_timeline"))
        .expect("inspect timeline tool");
    assert_eq!(
        inspect_timeline["inputSchema"]["properties"]["startFrame"]["type"],
        json!("integer")
    );
    assert_eq!(
        inspect_timeline["inputSchema"]["properties"]["endFrame"]["type"],
        json!("integer")
    );
    assert_eq!(
        inspect_timeline["inputSchema"]["properties"]["maxFrames"]["maximum"],
        json!(12)
    );

    let get_transcript = tools
        .iter()
        .find(|tool| tool["name"] == json!("video_creater.get_transcript"))
        .expect("get transcript tool");
    assert_eq!(
        get_transcript["inputSchema"]["properties"]["startFrame"]["type"],
        json!("integer")
    );
    assert_eq!(
        get_transcript["inputSchema"]["properties"]["endFrame"]["type"],
        json!("integer")
    );
    assert_eq!(
        get_transcript["inputSchema"]["properties"]["clipId"]["type"],
        json!("string")
    );
    assert_eq!(
        get_transcript["inputSchema"]["properties"]["language"]["type"],
        json!("string")
    );
    assert_eq!(
        get_transcript["inputSchema"]["properties"]["wordTimestamps"]["type"],
        json!("boolean")
    );

    let delete_media = tools
        .iter()
        .find(|tool| tool["name"] == json!("video_creater.delete_media"))
        .expect("delete media tool");
    assert_eq!(
        delete_media["inputSchema"]["anyOf"],
        json!([{ "required": ["assetIds"] }, { "required": ["mediaIds"] }])
    );

    let add_texts = tools
        .iter()
        .find(|tool| tool["name"] == json!("video_creater.add_texts"))
        .expect("add texts tool");
    assert_eq!(
        add_texts["inputSchema"]["anyOf"],
        json!([
            { "required": ["targetTrackId", "texts"] },
            { "required": ["entries"] },
            { "required": ["projectDir", "actions"] }
        ])
    );
    assert_eq!(
        add_texts["inputSchema"]["properties"]["entries"]["items"]["properties"]["content"]["type"],
        json!("string")
    );

    let add_captions = tools
        .iter()
        .find(|tool| tool["name"] == json!("video_creater.add_captions"))
        .expect("add captions tool");
    assert_eq!(
        add_captions["inputSchema"]["anyOf"],
        json!([
            { "required": ["targetTrackId", "captions"] },
            { "required": ["clipIds"] },
            { "required": ["projectDir", "actions"] }
        ])
    );
    assert_eq!(
        add_captions["inputSchema"]["properties"]["clipIds"]["items"]["type"],
        json!("string")
    );

    let export_project = tools
        .iter()
        .find(|tool| tool["name"] == json!("video_creater.export_project"))
        .expect("export project tool");
    assert_eq!(
        export_project["inputSchema"]["properties"]["timelineId"]["type"],
        json!("string")
    );
    assert_eq!(
        export_project["inputSchema"]["properties"]["fcpxmlTarget"]["enum"],
        json!(["resolve", "fcp"])
    );
}

#[test]
fn codex_local_tools_include_palmier_parity_timeline_tools() {
    let names = list_codex_local_tools()
        .into_iter()
        .map(|tool| tool.name)
        .collect::<BTreeSet<_>>();
    let palmier_tool_names = [
        "get_timeline",
        "get_media",
        "add_clips",
        "insert_clips",
        "remove_clips",
        "remove_tracks",
        "move_clips",
        "apply_layout",
        "link_clips",
        "unlink_clips",
        "set_clip_properties",
        "set_keyframes",
        "split_clips",
        "ripple_delete_ranges",
        "remove_words",
        "remove_silence",
        "sync_audio",
        "denoise_audio",
        "undo",
        "add_texts",
        "update_text",
        "add_captions",
        "export_project",
        "generate_video",
        "generate_image",
        "generate_audio",
        "upscale_media",
        "rerun_generated_asset",
        "import_media",
        "create_matte",
        "list_models",
        "inspect_media",
        "get_transcript",
        "inspect_timeline",
        "search_media",
        "build_multi_source_edit",
        "apply_color",
        "apply_effect",
        "inspect_color",
        "list_folders",
        "create_folder",
        "move_to_folder",
        "rename_media",
        "rename_folder",
        "delete_media",
        "delete_folder",
        "send_feedback",
        "set_project_settings",
        "create_timeline",
        "set_active_timeline",
        "duplicate_timeline",
        "read_skill",
        "get_projects",
        "open_project",
        "new_project",
    ];
    let expected_palmier_bare_aliases = palmier_tool_names
        .into_iter()
        .map(str::to_string)
        .collect::<BTreeSet<_>>();
    let actual_palmier_bare_aliases = names
        .iter()
        .filter(|name| !name.contains('.'))
        .cloned()
        .collect::<BTreeSet<_>>();

    assert_eq!(actual_palmier_bare_aliases, expected_palmier_bare_aliases);
    for palmier_tool_name in expected_palmier_bare_aliases {
        let canonical_name = if palmier_tool_name == "undo" {
            "video_creater.undo_agent_edit".to_string()
        } else {
            format!("video_creater.{palmier_tool_name}")
        };
        assert!(
            names.contains(&canonical_name),
            "missing canonical tool for Palmier alias {palmier_tool_name}"
        );
    }
    assert!(names.contains("video_creater.generate_music"));
    assert!(names.contains("video_creater.generate_sfx"));
    assert!(names.contains("video_creater.list_effects"));
    assert!(names.contains("video_creater.export_profiles"));
}

#[test]
fn codex_link_clips_assigns_shared_link_group_and_persists() {
    let mut project = sample_project();
    project.timeline.tracks[4].items.push(TimelineItem {
        id: "item-1-audio".to_string(),
        kind: TimelineItemKind::AudioClip,
        start_seconds: 0.0,
        duration_seconds: 4.0,
        source: TimelineSource::Media {
            media_id: "media-1".to_string(),
        },
        label: "Opening clip audio".to_string(),
        properties: BTreeMap::from([("sourceClipType".to_string(), json!("audio"))]),
    });
    let project_dir = tempfile::tempdir().expect("link clips project dir");
    save_split_project(project_dir.path(), &project).expect("save link clips project");

    let result = call_codex_local_tool(
        &project,
        "link_clips",
        json!({
            "projectDir": project_dir.path().display().to_string(),
            "clipIds": ["item-1", "item-1-audio"]
        }),
    )
    .expect("link_clips should validate");

    assert!(result.mutates_project);
    assert_eq!(
        result.payload["affectedItemIds"],
        json!(["item-1", "item-1-audio"])
    );
    assert_eq!(
        result.payload["linkGroupId"],
        json!("link-item-1-item-1-audio")
    );
    assert_eq!(result.payload["applied"], json!(true));

    let persisted = load_split_project(project_dir.path()).expect("load linked project");
    let link_groups = persisted
        .timeline
        .tracks
        .iter()
        .flat_map(|track| track.items.iter())
        .filter(|item| item.id == "item-1" || item.id == "item-1-audio")
        .map(|item| item.properties.get("linkGroupId").cloned())
        .collect::<Vec<_>>();
    assert_eq!(
        link_groups,
        vec![
            Some(json!("link-item-1-item-1-audio")),
            Some(json!("link-item-1-item-1-audio"))
        ]
    );
}

#[test]
fn codex_unlink_clips_clears_existing_link_group_and_persists() {
    let mut project = sample_project();
    project.timeline.tracks[0].items[0]
        .properties
        .insert("linkGroupId".to_string(), json!("linked-av-1"));
    project.timeline.tracks[4].items.push(TimelineItem {
        id: "item-1-audio".to_string(),
        kind: TimelineItemKind::AudioClip,
        start_seconds: 0.0,
        duration_seconds: 4.0,
        source: TimelineSource::Media {
            media_id: "media-1".to_string(),
        },
        label: "Opening clip audio".to_string(),
        properties: BTreeMap::from([
            ("linkGroupId".to_string(), json!("linked-av-1")),
            ("sourceClipType".to_string(), json!("audio")),
        ]),
    });
    let project_dir = tempfile::tempdir().expect("unlink clips project dir");
    save_split_project(project_dir.path(), &project).expect("save unlink clips project");

    let result = call_codex_local_tool(
        &project,
        "unlink_clips",
        json!({
            "projectDir": project_dir.path().display().to_string(),
            "clipIds": ["item-1"]
        }),
    )
    .expect("unlink_clips should validate");

    assert!(result.mutates_project);
    assert_eq!(
        result.payload["affectedItemIds"],
        json!(["item-1", "item-1-audio"])
    );
    assert_eq!(result.payload["applied"], json!(true));

    let persisted = load_split_project(project_dir.path()).expect("load unlinked project");
    for item in persisted
        .timeline
        .tracks
        .iter()
        .flat_map(|track| track.items.iter())
        .filter(|item| item.id == "item-1" || item.id == "item-1-audio")
    {
        assert!(!item.properties.contains_key("linkGroupId"));
    }
}

#[test]
fn codex_apply_project_actions_records_agent_history_and_undoes_latest_batch() {
    let project = sample_project();
    let project_dir = tempfile::tempdir().expect("project dir");
    save_split_project(project_dir.path(), &project).expect("save split project");
    let baseline_project = load_split_project(project_dir.path()).expect("load baseline project");

    let project_dir_arg = project_dir.path().display().to_string();
    let apply = call_codex_local_tool(
        &project,
        "video_creater.apply_project_actions",
        json!({
            "projectDir": project_dir_arg,
            "actions": [
                {
                    "type": "moveItems",
                    "moves": [
                        {
                            "itemId": "item-1",
                            "targetTrackId": "track-video",
                            "startSeconds": 1.25
                        }
                    ]
                }
            ]
        }),
    )
    .expect("apply project actions");

    assert!(apply.mutates_project);
    assert_eq!(apply.payload["applied"], json!(true));
    assert_eq!(apply.payload["agentHistory"]["entryCount"], json!(1));
    let moved_project = load_split_project(project_dir.path()).expect("load moved project");
    assert_eq!(
        moved_project.timeline.tracks[0].items[0].start_seconds,
        1.25
    );

    let undo = call_codex_local_tool(
        &moved_project,
        "video_creater.undo_agent_edit",
        json!({ "projectDir": project_dir.path().display().to_string() }),
    )
    .expect("undo latest agent edit");

    assert!(undo.mutates_project);
    assert_eq!(undo.payload["undone"], json!(true));
    assert_eq!(undo.payload["actionCount"], json!(1));
    assert_eq!(undo.payload["remainingAgentHistory"], json!(0));
    let restored = load_split_project(project_dir.path()).expect("load restored project");
    // Undo is itself a canonical write: the content revision is a monotonic compare-and-swap
    // token, so it advances past the agent batch instead of rewinding to the baseline.
    assert_eq!(
        moved_project.content_revision,
        baseline_project.content_revision + 1
    );
    assert_eq!(
        restored.content_revision,
        baseline_project.content_revision + 2
    );
    let mut restored_content = restored.clone();
    restored_content.content_revision = baseline_project.content_revision;
    assert_eq!(restored_content, baseline_project);

    let empty = call_codex_local_tool(
        &restored,
        "video_creater.undo_agent_edit",
        json!({ "projectDir": project_dir.path().display().to_string() }),
    );
    assert!(empty.is_err());
    assert!(empty
        .expect_err("empty agent history error")
        .to_string()
        .contains("no agent edit history"));
}

#[test]
fn codex_undo_agent_edit_rejects_after_non_agent_project_change() {
    let project = sample_project();
    let project_dir = tempfile::tempdir().expect("project dir");
    save_split_project(project_dir.path(), &project).expect("save split project");
    let project_dir_arg = project_dir.path().display().to_string();

    call_codex_local_tool(
        &project,
        "video_creater.apply_project_actions",
        json!({
            "projectDir": project_dir_arg,
            "actions": [
                {
                    "type": "moveItems",
                    "moves": [
                        {
                            "itemId": "item-1",
                            "targetTrackId": "track-video",
                            "startSeconds": 1.25
                        }
                    ]
                }
            ]
        }),
    )
    .expect("apply project actions");

    let mut changed_project = load_split_project(project_dir.path()).expect("load moved project");
    changed_project.timeline.tracks[0].items[0].label = "Manual edit".to_string();
    save_split_project(project_dir.path(), &changed_project).expect("save manual edit");

    let undo = call_codex_local_tool(
        &changed_project,
        "video_creater.undo_agent_edit",
        json!({ "projectDir": project_dir.path().display().to_string() }),
    );

    assert!(undo.is_err());
    assert!(undo
        .expect_err("changed project undo error")
        .to_string()
        .contains("project changed after that batch"));
    let stored = load_split_project(project_dir.path()).expect("load stored project");
    assert_eq!(stored.timeline.tracks[0].items[0].label, "Manual edit");
}

#[test]
fn codex_list_effects_returns_executable_palmier_style_catalog() {
    let project = sample_project();

    let result = call_codex_local_tool(&project, "video_creater.list_effects", json!({}))
        .expect("list effects");

    assert!(!result.mutates_project);
    assert_eq!(result.payload["source"], json!("palmier-compatible"));
    assert_eq!(result.payload["effectCount"], json!(20));
    assert_eq!(result.payload["canonicalOrder"][0], json!("color.exposure"));
    assert_eq!(result.payload["canonicalOrder"][19], json!("stylize.glow"));

    let effects = result.payload["effects"]
        .as_array()
        .expect("effects should be array");
    let glow = effects
        .iter()
        .find(|effect| effect["id"] == json!("stylize.glow"))
        .expect("glow descriptor");
    assert_eq!(glow["displayName"], json!("Glow"));
    assert_eq!(glow["category"], json!("Stylize"));
    assert_eq!(glow["colorEffect"], json!(false));
    assert_eq!(glow["params"][0]["key"], json!("intensity"));
    assert_eq!(glow["params"][0]["max"], json!(1.0));

    let lut = effects
        .iter()
        .find(|effect| effect["id"] == json!("color.lut"))
        .expect("LUT descriptor");
    assert_eq!(lut["resourceKey"], json!("path"));

    let chroma = effects
        .iter()
        .find(|effect| effect["id"] == json!("key.chroma"))
        .expect("chroma descriptor");
    assert_eq!(chroma["params"][0]["key"], json!("keyHue"));

    let non_color = result.payload["nonColorEffects"]
        .as_array()
        .expect("non-color effects should be array");
    assert!(non_color
        .iter()
        .any(|effect| effect["id"] == json!("blur.motion")));
    assert!(!non_color
        .iter()
        .any(|effect| effect["id"] == json!("color.exposure")));
}

#[test]
fn codex_export_project_accepts_webm_render_profiles() {
    let mut project = sample_project();
    project.timeline.tracks[0].items[0]
        .properties
        .insert("sourceIn".to_string(), json!(0.0));
    project.timeline.tracks[0].items[0]
        .properties
        .insert("sourceOut".to_string(), json!(4.0));
    let result = call_codex_local_tool(
        &project,
        "video_creater.export_project",
        json!({
            "projectDir": "/tmp/video-creater-project",
            "jobId": "render-draft-1",
            "profile": "draftWebm"
        }),
    )
    .expect("render draft start request");

    assert_eq!(
        result.payload["startRequest"]["workflowType"],
        json!("VideoCreaterRenderDraftWorkflow")
    );
    assert_eq!(
        result.payload["startRequest"]["input"]["profile"],
        json!("draftWebm")
    );
}

#[test]
fn codex_export_project_preflights_webm_render_plan_errors() {
    let mut project = sample_project();
    project.timeline.tracks[0].items[0]
        .properties
        .insert("sourceIn".to_string(), json!(0.0));
    project.timeline.tracks[0].items[0]
        .properties
        .insert("sourceOut".to_string(), json!(4.0));
    project.media.push(MediaAsset {
        id: "media-lottie-1".to_string(),
        name: Some("Brand burst".to_string()),
        relative_path: "media/brand-burst.json".to_string(),
        kind: MediaKind::Lottie,
        duration_seconds: 2.0,
        width: Some(1080),
        height: Some(1080),
        fps: Some(30.0),
        folder_id: None,
    });
    project.timeline.tracks[0].items.push(TimelineItem {
        id: "lottie-item-1".to_string(),
        kind: TimelineItemKind::VideoClip,
        start_seconds: 4.0,
        duration_seconds: 2.0,
        source: TimelineSource::Media {
            media_id: "media-lottie-1".to_string(),
        },
        label: "Lottie burst".to_string(),
        properties: BTreeMap::from([
            ("sourceIn".to_string(), json!(0.0)),
            ("sourceOut".to_string(), json!(2.0)),
        ]),
    });

    let error = call_codex_local_tool(
        &project,
        "video_creater.export_project",
        json!({
            "projectDir": "/tmp/video-creater-project",
            "jobId": "render-draft-lottie",
            "profile": "draftWebm"
        }),
    )
    .expect_err("WebM export should preflight unsupported render media");

    assert!(error
        .to_string()
        .contains("Lottie media must be baked before WebM rendering."));
}

#[test]
fn codex_export_project_accepts_nle_xml_profiles() {
    let project = sample_project();
    let result = call_codex_local_tool(
        &project,
        "video_creater.export_project",
        json!({
            "projectDir": "/tmp/video-creater-project",
            "jobId": "export-premiere-1",
            "profile": "premiereXmeml",
            "outputPath": "exports/project.xml"
        }),
    )
    .expect("NLE XML export start request");

    assert_eq!(
        result.payload["startRequest"]["workflowType"],
        json!("VideoCreaterExportNleXmlWorkflow")
    );
    assert_eq!(
        result.payload["startRequest"]["input"]["format"],
        json!("premiereXmeml")
    );
}

#[test]
fn codex_export_project_preflights_nle_xml_errors() {
    let mut project = sample_project();
    project.timeline.tracks[0].items[0].source = TimelineSource::Media {
        media_id: "missing-media".to_string(),
    };

    let error = call_codex_local_tool(
        &project,
        "video_creater.export_project",
        json!({
            "projectDir": "/tmp/video-creater-project",
            "jobId": "export-premiere-missing-media",
            "profile": "premiereXmeml",
            "outputPath": "exports/project.xml"
        }),
    )
    .expect_err("NLE XML export should preflight missing media");

    assert!(error
        .to_string()
        .contains("references missing media `missing-media`"));
}

#[test]
fn codex_build_export_nle_xml_start_request_preflights_errors() {
    let mut project = sample_project();
    project.timeline.tracks[0].items[0].source = TimelineSource::Media {
        media_id: "missing-media".to_string(),
    };

    let error = call_codex_local_tool(
        &project,
        "video_creater.build_export_nle_xml_start_request",
        json!({
            "projectDir": "/tmp/video-creater-project",
            "jobId": "export-premiere-missing-media",
            "format": "premiereXmeml",
            "outputPath": "exports/project.xml"
        }),
    )
    .expect_err("NLE XML start-request builder should preflight missing media");

    assert!(error
        .to_string()
        .contains("references missing media `missing-media`"));
}

#[test]
fn codex_export_project_accepts_palmier_mode_aliases() {
    let _guard = EXPORT_PROFILE_ENV_LOCK.lock().expect("export env lock");
    let temp = tempfile::tempdir().expect("runtime path");
    let _env = IsolatedExportRuntimeEnv::new(temp.path());
    let runtime = executable_fixture(temp.path(), "approved-export-runtime");
    std::env::set_var("VIDEO_CREATER_APPROVED_H264_ENCODER", &runtime);
    std::env::set_var("VIDEO_CREATER_APPROVED_AAC_ENCODER", &runtime);
    let project = sample_project();

    let video_result = call_codex_local_tool(
        &project,
        "video_creater.export_project",
        json!({
            "projectDir": "/tmp/video-creater-project",
            "jobId": "export-palmier-video",
            "mode": "video",
            "codec": "H.264",
            "resolution": "matchTimeline",
            "outputPath": "exports/final.mp4",
            "overwrite": false
        }),
    );
    if h264_export_available() {
        let video = video_result.expect("Palmier-style video export args");
        assert_eq!(
            video.payload["startRequest"]["input"]["profile"],
            json!("mp4H264")
        );
        assert_eq!(
            video.payload["startRequest"]["input"]["overwrite"],
            json!(false)
        );
        assert_eq!(video.payload["outputPolicy"]["overwrite"], json!(false));
    } else {
        let error = video_result.expect_err("unavailable H.264 runtime should be reported");
        assert!(error.to_string().contains("mp4H264 export is unavailable"));
    }

    let xml = call_codex_local_tool(
        &project,
        "video_creater.export_project",
        json!({
            "projectDir": "/tmp/video-creater-project",
            "jobId": "export-palmier-xml",
            "mode": "xml",
            "outputPath": "exports/timeline.xml"
        }),
    )
    .expect("Palmier-style XML export args");
    assert_eq!(
        xml.payload["startRequest"]["input"]["format"],
        json!("premiereXmeml")
    );

    let xml_no_overwrite = call_codex_local_tool(
        &project,
        "video_creater.export_project",
        json!({
            "projectDir": "/tmp/video-creater-project",
            "jobId": "export-palmier-xml-no-overwrite",
            "mode": "xml",
            "outputPath": "exports/timeline.xml",
            "overwrite": false
        }),
    )
    .expect("Palmier-style XML overwrite policy");
    assert_eq!(
        xml_no_overwrite.payload["startRequest"]["input"]["overwrite"],
        json!(false)
    );

    let fcpxml = call_codex_local_tool(
        &project,
        "video_creater.export_project",
        json!({
            "projectDir": "/tmp/video-creater-project",
            "jobId": "export-palmier-fcpxml",
            "mode": "fcpxml",
            "outputPath": "exports/timeline.fcpxml"
        }),
    )
    .expect("Palmier-style FCPXML export args");
    assert_eq!(
        fcpxml.payload["startRequest"]["input"]["format"],
        json!("davinciFcpxml")
    );

    let bundle = call_codex_local_tool(
        &project,
        "video_creater.export_project",
        json!({
            "projectDir": "/tmp/video-creater-project",
            "jobId": "export-palmier-bundle",
            "mode": "palmier"
        }),
    )
    .expect("Palmier project bundle export args");
    assert_eq!(
        bundle.payload["startRequest"]["workflowType"],
        json!("VideoCreaterExportMediaWorkflow")
    );
    assert_eq!(
        bundle.payload["startRequest"]["input"]["profile"],
        json!("palmierProject")
    );
    assert_eq!(
        bundle.payload["startRequest"]["input"]["outputPath"],
        json!("exports/test-project.palmier")
    );

    let bundle_no_overwrite = call_codex_local_tool(
        &project,
        "video_creater.export_project",
        json!({
            "projectDir": "/tmp/video-creater-project",
            "jobId": "export-palmier-bundle-no-overwrite",
            "mode": "palmier",
            "outputPath": "exports/final.palmier",
            "overwrite": false
        }),
    )
    .expect("Palmier project bundle overwrite policy");
    assert_eq!(
        bundle_no_overwrite.payload["startRequest"]["input"]["overwrite"],
        json!(false)
    );
}

#[test]
fn codex_export_project_accepts_palmier_timeline_and_fcpxml_target_args() {
    let mut project = sample_project();
    project
        .timelines
        .push(video_creater_lib::project::model::ProjectTimeline {
            id: "alternate".to_string(),
            name: "Alternate cut".to_string(),
            timeline: project.timeline.clone(),
        });

    let fcpxml = call_codex_local_tool(
        &project,
        "video_creater.export_project",
        json!({
            "projectDir": "/tmp/video-creater-project",
            "jobId": "export-palmier-fcp-target",
            "mode": "fcpxml",
            "timelineId": "alternate",
            "fcpxmlTarget": "fcp",
            "outputPath": "exports/timeline.fcpxml"
        }),
    )
    .expect("Palmier timelineId and fcpxmlTarget args");

    assert_eq!(
        fcpxml.payload["startRequest"]["input"]["format"],
        json!("davinciFcpxml")
    );
    assert_eq!(
        fcpxml.payload["startRequest"]["input"]["timelineId"],
        json!("alternate")
    );
    assert_eq!(
        fcpxml.payload["startRequest"]["input"]["fcpxmlTarget"],
        json!("fcp")
    );
}

#[test]
fn codex_export_project_rejects_unapplied_video_resolution_presets() {
    let mut project = sample_project();
    project.timeline.tracks[0].items[0]
        .properties
        .insert("sourceIn".to_string(), json!(0.0));
    project.timeline.tracks[0].items[0]
        .properties
        .insert("sourceOut".to_string(), json!(4.0));

    let error = call_codex_local_tool(
        &project,
        "video_creater.export_project",
        json!({
            "projectDir": "/tmp/video-creater-project",
            "mode": "video",
            "codec": "WebM",
            "resolution": "720p",
            "outputPath": "exports/final.webm"
        }),
    )
    .expect_err("unapplied video export resolution should fail closed");

    assert!(error
        .to_string()
        .contains("export_project resolution '720p' is not supported yet"));
    assert!(error.to_string().contains("Match Timeline"));
}

#[test]
fn codex_export_project_defaults_palmier_output_path_when_omitted() {
    let project = sample_project();

    let result = call_codex_local_tool(
        &project,
        "video_creater.export_project",
        json!({
            "projectDir": "/tmp/video-creater-project",
            "jobId": "export-palmier-default-path",
            "mode": "xml"
        }),
    )
    .expect("Palmier-style XML export should default outputPath");

    assert_eq!(
        result.payload["startRequest"]["input"]["outputPath"],
        json!("exports/test-project.xml")
    );
}

#[test]
fn codex_export_project_accepts_palmier_minimal_xml_arguments() {
    let project = sample_project();

    let result = call_codex_local_tool(
        &project,
        "video_creater.export_project",
        json!({
            "mode": "xml"
        }),
    )
    .expect("minimal Palmier XML export args should validate");

    assert!(!result.mutates_project);
    assert_eq!(result.payload["status"], json!("exported"));
    assert_eq!(result.payload["mode"], json!("xml"));
    assert_eq!(result.payload["path"], json!("exports/test-project.xml"));
    assert_eq!(
        result.payload["startRequest"]["input"]["format"],
        json!("premiereXmeml")
    );
    assert_eq!(
        result.payload["startRequest"]["input"]["outputPath"],
        json!("exports/test-project.xml")
    );
    assert_eq!(
        result.payload["startRequest"]["input"]["jobId"],
        json!("export-test-project-xml")
    );
    assert_eq!(
        result.payload["startRequest"]["workflowId"],
        json!("video-creater/project-test/export-nle-xml/export-test-project-xml")
    );
    assert!(result.payload["startRequest"]["input"]["projectDir"]
        .as_str()
        .expect("project dir")
        .ends_with("/test-project"));
}

#[test]
fn codex_export_project_rejects_invalid_palmier_mode_options() {
    let project = sample_project();

    let codec_for_xml = call_codex_local_tool(
        &project,
        "video_creater.export_project",
        json!({
            "projectDir": "/tmp/video-creater-project",
            "jobId": "export-invalid-xml-codec",
            "mode": "xml",
            "codec": "H.264",
            "outputPath": "exports/timeline.xml"
        }),
    )
    .expect_err("XML export should reject video codec options");
    assert!(codec_for_xml
        .to_string()
        .contains("codec only applies to video mode"));

    let codec_for_palmier = call_codex_local_tool(
        &project,
        "video_creater.export_project",
        json!({
            "projectDir": "/tmp/video-creater-project",
            "jobId": "export-invalid-palmier-codec",
            "mode": "palmier",
            "codec": "H.264"
        }),
    )
    .expect_err("Palmier project export should reject video codec options");
    assert!(codec_for_palmier
        .to_string()
        .contains("codec only applies to video mode"));

    let mixed_profile = call_codex_local_tool(
        &project,
        "video_creater.export_project",
        json!({
            "projectDir": "/tmp/video-creater-project",
            "jobId": "export-invalid-mixed",
            "profile": "finalWebm",
            "mode": "video"
        }),
    )
    .expect_err("profile should not be mixed with mode");
    assert!(mixed_profile
        .to_string()
        .contains("profile cannot be combined with mode"));

    let wrong_extension = call_codex_local_tool(
        &project,
        "video_creater.export_project",
        json!({
            "projectDir": "/tmp/video-creater-project",
            "jobId": "export-invalid-xml-extension",
            "mode": "xml",
            "outputPath": "exports/timeline.mp4"
        }),
    )
    .expect_err("XML export should reject mismatched output extension");
    assert!(wrong_extension
        .to_string()
        .contains("xml exports must use .xml"));

    let palmier_timeline = call_codex_local_tool(
        &project,
        "video_creater.export_project",
        json!({
            "projectDir": "/tmp/video-creater-project",
            "jobId": "export-invalid-palmier-timeline",
            "mode": "palmier",
            "timelineId": "project-test"
        }),
    )
    .expect_err("Palmier project package export should reject timelineId");
    assert!(palmier_timeline
        .to_string()
        .contains("timelineId is not valid for palmier mode"));

    let unknown_timeline = call_codex_local_tool(
        &project,
        "video_creater.export_project",
        json!({
            "projectDir": "/tmp/video-creater-project",
            "jobId": "export-invalid-unknown-timeline",
            "mode": "xml",
            "timelineId": "unknown-timeline"
        }),
    )
    .expect_err("single-timeline export should reject unknown timelineId");
    assert!(unknown_timeline
        .to_string()
        .contains("timelineId `unknown-timeline` was not found"));
}

#[test]
fn codex_export_project_defaults_output_path_for_media_exports() {
    let _guard = EXPORT_PROFILE_ENV_LOCK.lock().expect("export env lock");
    let temp = tempfile::tempdir().expect("runtime path");
    let _env = IsolatedExportRuntimeEnv::new(temp.path());
    let runtime = executable_fixture(temp.path(), "approved-export-runtime");
    std::env::set_var("VIDEO_CREATER_APPROVED_H264_ENCODER", &runtime);
    std::env::set_var("VIDEO_CREATER_APPROVED_AAC_ENCODER", &runtime);
    let project = sample_project();
    let default_output_result = call_codex_local_tool(
        &project,
        "video_creater.export_project",
        json!({
            "projectDir": "/tmp/video-creater-project",
            "jobId": "export-mp4-1",
            "profile": "mp4H264"
        }),
    );
    if h264_export_available() {
        let default_output = default_output_result.expect("MP4 export should default output path");
        assert_eq!(
            default_output.payload["startRequest"]["input"]["outputPath"],
            json!("exports/test-project.mp4")
        );
    } else {
        let error =
            default_output_result.expect_err("unavailable H.264 runtime should be reported");
        assert!(error.to_string().contains("mp4H264 export is unavailable"));
    }

    let result = call_codex_local_tool(
        &project,
        "video_creater.export_project",
        json!({
            "projectDir": "/tmp/video-creater-project",
            "jobId": "export-mp4-1",
            "profile": "mp4H264",
            "outputPath": "exports/final.mp4"
        }),
    );
    if !h264_export_available() {
        let error = result.expect_err("unavailable H.264 runtime should be reported");
        assert!(error.to_string().contains("mp4H264 export is unavailable"));
        return;
    }
    let result = result.expect("MP4 export start request");

    assert_eq!(
        result.payload["startRequest"]["workflowType"],
        json!("VideoCreaterExportMediaWorkflow")
    );
    assert_eq!(
        result.payload["startRequest"]["input"]["profile"],
        json!("mp4H264")
    );
}

#[test]
fn codex_export_project_matches_runtime_media_profile_availability() {
    let _guard = EXPORT_PROFILE_ENV_LOCK.lock().expect("export env lock");
    let temp = tempfile::tempdir().expect("runtime path");
    let _env = IsolatedExportRuntimeEnv::new(temp.path());
    let project = sample_project();

    let profiles = call_codex_local_tool(&project, "video_creater.export_profiles", json!({}))
        .expect("export profiles");
    let h264 = profiles.payload["profiles"]
        .as_array()
        .expect("profiles")
        .iter()
        .find(|profile| profile["profile"] == json!("mp4H264"))
        .expect("h264 profile");
    let available = h264["available"].as_bool().expect("availability boolean");

    let result = call_codex_local_tool(
        &project,
        "video_creater.export_project",
        json!({
            "projectDir": "/tmp/video-creater-project",
            "jobId": "export-mp4-unavailable",
            "profile": "mp4H264",
            "outputPath": "exports/final.mp4"
        }),
    );
    if available {
        let output = result.expect("available MP4 profile should build a start request");
        assert_eq!(
            output.payload["startRequest"]["input"]["profile"],
            json!("mp4H264")
        );
    } else {
        let error = result.expect_err("unavailable MP4 profile should not build a start request");
        assert!(error.to_string().contains("mp4H264 export is unavailable"));
        assert!(error.to_string().contains("GStreamer"));
    }
}

#[test]
fn codex_sync_audio_returns_valid_move_action_from_waveform_peaks() {
    let mut project = sample_project();
    project.media.push(MediaAsset {
        id: "camera-audio".to_string(),
        name: Some("Camera audio".to_string()),
        relative_path: "media/camera.wav".to_string(),
        kind: MediaKind::Audio,
        duration_seconds: 24.0,
        width: None,
        height: None,
        fps: None,
        folder_id: None,
    });
    project.media.push(MediaAsset {
        id: "recorder-audio".to_string(),
        name: Some("Recorder audio".to_string()),
        relative_path: "media/recorder.wav".to_string(),
        kind: MediaKind::Audio,
        duration_seconds: 24.0,
        width: None,
        height: None,
        fps: None,
        folder_id: None,
    });
    let audio_track = project
        .timeline
        .tracks
        .iter_mut()
        .find(|track| track.id == "track-audio")
        .expect("audio track");
    audio_track.items.push(TimelineItem {
        id: "camera-clip".to_string(),
        kind: TimelineItemKind::AudioClip,
        start_seconds: 4.0,
        duration_seconds: 24.0,
        source: TimelineSource::Media {
            media_id: "camera-audio".to_string(),
        },
        label: "Camera audio".to_string(),
        properties: BTreeMap::from([(
            "waveformPeaks".to_string(),
            json!([
                0.0, 0.0, 0.1, 0.2, 0.8, 1.0, 0.7, 0.2, 0.1, 0.0, 0.2, 0.6, 0.9, 0.6, 0.2, 0.0,
                0.1, 0.3, 0.7, 0.4, 0.1, 0.0, 0.0, 0.0
            ]),
        )]),
    });
    project.timeline.tracks.push(TimelineTrack {
        transitions: Vec::new(),
        id: "track-recorder".to_string(),
        name: "Recorder audio".to_string(),
        kind: TrackKind::Audio,
        locked: false,
        sync_locked: false,
        enabled: true,
        items: vec![TimelineItem {
            id: "recorder-clip".to_string(),
            kind: TimelineItemKind::AudioClip,
            start_seconds: 7.0,
            duration_seconds: 24.0,
            source: TimelineSource::Media {
                media_id: "recorder-audio".to_string(),
            },
            label: "Recorder audio".to_string(),
            properties: BTreeMap::from([(
                "waveformPeaks".to_string(),
                json!([
                    0.0, 0.0, 0.0, 0.1, 0.2, 0.8, 1.0, 0.7, 0.2, 0.1, 0.0, 0.2, 0.6, 0.9, 0.6, 0.2,
                    0.0, 0.1, 0.3, 0.7, 0.4, 0.1, 0.0, 0.0
                ]),
            )]),
        }],
    });

    let result = call_codex_local_tool(
        &project,
        "video_creater.sync_audio",
        json!({
            "referenceClipId": "camera-clip",
            "targetClipId": "recorder-clip",
            "searchWindowSeconds": 6.0,
            "minConfidence": 0.8
        }),
    )
    .expect("sync audio payload");

    assert!(result.mutates_project);
    assert_eq!(
        result.payload["synced"][0]["clipId"],
        json!("recorder-clip")
    );
    assert_eq!(result.payload["synced"][0]["offsetSeconds"], json!(-4.0));
    assert_eq!(
        result.payload["actions"],
        json!([
            {
                "type": "moveItems",
                "moves": [
                    {
                        "itemId": "recorder-clip",
                        "targetTrackId": "track-recorder",
                        "startSeconds": 3.0
                    }
                ]
            },
            {
                "type": "updateAudioSync",
                "itemId": "recorder-clip",
                "sync": {
                    "referenceClipId": "camera-clip",
                    "offsetSeconds": -4.0,
                    "confidence": 1.0,
                    "syncedAtStartSeconds": 3.0
                }
            }
        ])
    );
    assert_eq!(result.payload["valid"], json!(true));

    let project_dir = tempfile::tempdir().expect("sync audio project dir");
    save_split_project(project_dir.path(), &project).expect("save sync audio project");
    let persisted = call_codex_local_tool(
        &project,
        "video_creater.sync_audio",
        json!({
            "projectDir": project_dir.path().display().to_string(),
            "referenceClipId": "camera-clip",
            "targetClipId": "recorder-clip",
            "searchWindowSeconds": 6.0,
            "minConfidence": 0.8
        }),
    )
    .expect("sync audio payload should persist");
    assert!(persisted.mutates_project);
    assert_eq!(persisted.payload["applied"], json!(true));
    assert_eq!(persisted.payload["agentHistory"]["entryCount"], json!(1));
    let persisted_project = load_split_project(project_dir.path()).expect("load synced project");
    let synced_item = persisted_project
        .timeline
        .tracks
        .iter()
        .flat_map(|track| track.items.iter())
        .find(|item| item.id == "recorder-clip")
        .expect("synced recorder clip");
    assert_eq!(synced_item.start_seconds, 3.0);
    assert_eq!(
        synced_item.properties.get("audioSync"),
        Some(&json!({
            "referenceClipId": "camera-clip",
            "offsetSeconds": -4.0,
            "confidence": 1.0,
            "syncedAtStartSeconds": 3.0
        }))
    );
}

#[test]
fn codex_direct_query_aliases_return_existing_project_payloads() {
    let project = sample_project();

    let timeline = call_codex_local_tool(&project, "video_creater.get_timeline", json!({}))
        .expect("timeline alias payload");
    assert!(!timeline.mutates_project);
    assert_eq!(timeline.payload["durationSeconds"], json!(4.0));
    assert_eq!(timeline.payload["totalFrames"], json!(96));
    assert_eq!(timeline.payload["currentFrame"], json!(0));
    assert_eq!(timeline.payload["fps"], json!(24.0));
    assert_eq!(
        timeline.payload["resolution"],
        json!({ "width": 1920, "height": 1080 })
    );
    assert_eq!(timeline.payload["canGenerate"], json!(true));
    assert_eq!(timeline.payload["timelineId"], json!("main"));
    assert_eq!(timeline.payload["activeTimelineId"], json!("main"));
    assert_eq!(
        timeline.payload["timelines"],
        json!([
            {
                "timelineId": "main",
                "id": "main",
                "name": "Timeline 1",
                "active": true,
                "durationSeconds": 4.0,
                "totalFrames": 96,
                "fps": 24.0
            }
        ])
    );
    assert_eq!(timeline.payload["tracks"][0]["id"], json!("track-video"));

    let media = call_codex_local_tool(&project, "video_creater.get_media", json!({}))
        .expect("media alias payload");
    assert_eq!(media.payload["media"][0]["id"], json!("media-1"));

    let transcript = call_codex_local_tool(
        &project,
        "video_creater.get_transcript",
        json!({ "mediaId": "media-1" }),
    )
    .expect("transcript alias payload");
    assert_eq!(transcript.payload["mediaId"], json!("media-1"));
    assert_eq!(transcript.payload["words"], json!([]));
    assert!(transcript.payload["error"]
        .as_str()
        .expect("missing transcript error")
        .contains("transcript was not found"));
}

#[test]
fn codex_palmier_project_timeline_tools_propose_canonical_actions() {
    let project = sample_project();

    let active = call_codex_local_tool(
        &project,
        "set_active_timeline",
        json!({ "timelineId": "main" }),
    )
    .expect("active timeline should be accepted");
    assert!(active.mutates_project);
    assert_eq!(
        active.payload["projectActions"][0]["type"],
        json!("setActiveTimeline")
    );
    assert_eq!(
        active.payload["projectActions"][0]["timelineId"],
        json!("main")
    );

    let unknown = call_codex_local_tool(
        &project,
        "video_creater.set_active_timeline",
        json!({ "timelineId": "other-timeline" }),
    )
    .expect_err("unknown timeline should fail");
    assert!(unknown
        .to_string()
        .contains("timeline `other-timeline` was not found"));

    let created = call_codex_local_tool(
        &project,
        "create_timeline",
        json!({ "name": "Alternate cut" }),
    )
    .expect("new timeline action should validate");
    assert!(created.mutates_project);
    assert_eq!(
        created.payload["timelineId"],
        json!("timeline-alternate-cut")
    );
    assert_eq!(
        created.payload["projectActions"][0]["type"],
        json!("createTimeline")
    );
    assert_eq!(
        created.payload["projectActions"][0]["duplicateActive"],
        json!(false)
    );

    let duplicate = call_codex_local_tool(
        &project,
        "duplicate_timeline",
        json!({ "name": "Timeline 1 copy" }),
    )
    .expect("duplicate timeline action should validate");
    assert!(duplicate.mutates_project);
    assert_eq!(
        duplicate.payload["projectActions"][0]["type"],
        json!("createTimeline")
    );
    assert_eq!(
        duplicate.payload["projectActions"][0]["duplicateActive"],
        json!(true)
    );

    for tool_name in ["get_projects", "open_project", "new_project"] {
        let error = call_codex_local_tool(&project, tool_name, json!({}))
            .expect_err("Palmier project/timeline limitation should be explicit");
        assert!(
            error.to_string().contains("multi-timeline")
                || error.to_string().contains("project navigation"),
            "unexpected {tool_name} error: {error}"
        );
    }
}

#[test]
fn codex_get_media_returns_palmier_entries_alias() {
    let project = sample_project();

    let media = call_codex_local_tool(&project, "get_media", json!({})).expect("media payload");

    assert_eq!(media.payload["entries"][0]["id"], json!("media-1"));
    assert_eq!(media.payload["entries"][0]["mediaRef"], json!("media-1"));
    assert_eq!(media.payload["entries"][0]["type"], json!("video"));
    assert_eq!(media.payload["entries"][0]["duration"], json!(12.0));
    assert_eq!(
        media.payload["entries"][0]["generationStatus"],
        json!("none")
    );
}

#[test]
fn codex_get_timeline_returns_palmier_clips_alias() {
    let project = sample_project();

    let timeline =
        call_codex_local_tool(&project, "get_timeline", json!({})).expect("timeline payload");

    assert_eq!(
        timeline.payload["tracks"][0]["clips"][0]["clipId"],
        json!("item-1")
    );
    assert_eq!(
        timeline.payload["tracks"][0]["clips"][0]["mediaRef"],
        json!("media-1")
    );
    assert_eq!(
        timeline.payload["tracks"][0]["clips"][0]["mediaType"],
        json!("video")
    );
    assert_eq!(
        timeline.payload["tracks"][0]["clips"][0]["startFrame"],
        json!(0)
    );
    assert_eq!(
        timeline.payload["tracks"][0]["clips"][0]["durationFrames"],
        json!(96)
    );
}

#[test]
fn codex_get_timeline_reports_non_default_speed_as_palmier_clip_field() {
    let mut project = sample_project();
    project.timeline.tracks[0].items[0]
        .properties
        .insert("speed".to_string(), json!(2.0));

    let timeline =
        call_codex_local_tool(&project, "get_timeline", json!({})).expect("timeline payload");

    let clip = &timeline.payload["tracks"][0]["clips"][0];
    assert_eq!(clip["clipId"], json!("item-1"));
    assert_eq!(clip["speed"], json!(2.0));
    assert_eq!(clip["properties"]["speed"], json!(2.0));
}

#[test]
fn codex_get_timeline_reports_non_default_scalars_as_palmier_clip_fields() {
    let mut project = sample_project();
    project.timeline.tracks[0].items[0].properties.extend([
        ("volumeDb".to_string(), json!(-6.021)),
        ("opacity".to_string(), json!(0.4)),
        (
            "transform".to_string(),
            json!({
                "centerX": 0.35,
                "centerY": 0.45,
                "width": 0.5,
                "height": 0.6,
                "flipHorizontal": true
            }),
        ),
    ]);

    let timeline =
        call_codex_local_tool(&project, "get_timeline", json!({})).expect("timeline payload");

    let clip = &timeline.payload["tracks"][0]["clips"][0];
    assert_eq!(clip["clipId"], json!("item-1"));
    assert_eq!(clip["volume"], json!(0.5));
    assert_eq!(clip["opacity"], json!(0.4));
    assert_eq!(
        clip["transform"],
        json!({
            "centerX": 0.35,
            "centerY": 0.45,
            "width": 0.5,
            "height": 0.6,
            "flipHorizontal": true
        })
    );
}

#[test]
fn codex_get_timeline_reports_audio_sync_as_palmier_clip_field() {
    let mut project = sample_project();
    project.timeline.tracks[0].items[0].properties.insert(
        "audioSync".to_string(),
        json!({
            "referenceClipId": "camera-clip",
            "offsetSeconds": -0.375,
            "confidence": 0.92,
            "syncedAtStartSeconds": 1.625
        }),
    );

    let timeline =
        call_codex_local_tool(&project, "get_timeline", json!({})).expect("timeline payload");

    let clip = &timeline.payload["tracks"][0]["clips"][0];
    assert_eq!(clip["clipId"], json!("item-1"));
    assert_eq!(
        clip["audioSync"],
        json!({
            "referenceClipId": "camera-clip",
            "offsetSeconds": -0.375,
            "confidence": 0.92,
            "syncedAtStartSeconds": 1.625
        })
    );
    assert_eq!(clip["properties"]["audioSync"], clip["audioSync"]);
}

#[test]
fn codex_get_timeline_reports_link_group_as_palmier_clip_field() {
    let mut project = sample_project();
    project.timeline.tracks[0].items[0]
        .properties
        .insert("linkGroupId".to_string(), json!("link-main-audio"));

    let timeline =
        call_codex_local_tool(&project, "get_timeline", json!({})).expect("timeline payload");

    let clip = &timeline.payload["tracks"][0]["clips"][0];
    assert_eq!(clip["clipId"], json!("item-1"));
    assert_eq!(clip["linkGroupId"], json!("link-main-audio"));
    assert_eq!(clip["properties"]["linkGroupId"], clip["linkGroupId"]);
}

#[test]
fn codex_get_timeline_reports_effects_and_color_grade_as_palmier_clip_fields() {
    let mut project = sample_project();
    project.timeline.tracks[0].items[0].properties.extend([
        ("blendMode".to_string(), json!("screen")),
        (
            "effects".to_string(),
            json!([
                {
                    "effectType": "stylize.glow",
                    "enabled": true,
                    "params": { "intensity": 0.7 }
                }
            ]),
        ),
        (
            "colorGrade".to_string(),
            json!({
                "exposure": 0.2,
                "contrast": 1.15,
                "extra": {
                    "vibrance": 0.25
                }
            }),
        ),
    ]);

    let timeline =
        call_codex_local_tool(&project, "get_timeline", json!({})).expect("timeline payload");

    let clip = &timeline.payload["tracks"][0]["clips"][0];
    assert_eq!(clip["clipId"], json!("item-1"));
    assert_eq!(clip["blendMode"], json!("screen"));
    assert_eq!(clip["properties"]["blendMode"], clip["blendMode"]);
    assert_eq!(
        clip["effects"],
        json!([
            {
                "effectType": "stylize.glow",
                "enabled": true,
                "params": { "intensity": 0.7 }
            }
        ])
    );
    assert_eq!(
        clip["colorGrade"],
        json!({
            "exposure": 0.2,
            "contrast": 1.15,
            "extra": {
                "vibrance": 0.25
            }
        })
    );
    assert_eq!(clip["properties"]["effects"], clip["effects"]);
    assert_eq!(clip["properties"]["colorGrade"], clip["colorGrade"]);
}

#[test]
fn codex_get_timeline_reports_total_clips_when_window_hides_items() {
    let mut project = sample_project();
    project.timeline.tracks[0].items[0].duration_seconds = 1.0;
    project.timeline.tracks[0].items.push(TimelineItem {
        id: "item-2".to_string(),
        kind: TimelineItemKind::VideoClip,
        start_seconds: 4.0,
        duration_seconds: 1.0,
        source: TimelineSource::Media {
            media_id: "media-1".to_string(),
        },
        label: "Middle".to_string(),
        properties: BTreeMap::new(),
    });
    project.timeline.tracks[0].items.push(TimelineItem {
        id: "item-3".to_string(),
        kind: TimelineItemKind::VideoClip,
        start_seconds: 8.0,
        duration_seconds: 1.0,
        source: TimelineSource::Media {
            media_id: "media-1".to_string(),
        },
        label: "Tail".to_string(),
        properties: BTreeMap::new(),
    });

    let timeline = call_codex_local_tool(
        &project,
        "get_timeline",
        json!({
            "startFrame": 90,
            "endFrame": 130
        }),
    )
    .expect("windowed timeline payload");

    assert_eq!(
        timeline.payload["tracks"][0]["clips"]
            .as_array()
            .unwrap()
            .len(),
        1
    );
    assert_eq!(
        timeline.payload["tracks"][0]["clips"][0]["clipId"],
        json!("item-2")
    );
    assert_eq!(timeline.payload["tracks"][0]["totalClips"], json!(3));
    assert_eq!(timeline.payload["tracks"][0]["totalItems"], json!(3));
}

#[test]
fn codex_get_timeline_survives_huge_palmier_start_frame() {
    let project = sample_project();

    let timeline = call_codex_local_tool(
        &project,
        "get_timeline",
        json!({
            "startFrame": 1.0e19
        }),
    )
    .expect("huge Palmier startFrame should return a bounded timeline payload");

    assert_eq!(
        timeline.payload["tracks"][0]["clips"][0]["clipId"],
        json!("item-1")
    );
}

#[test]
fn codex_list_folders_returns_parent_folder_id_alias() {
    let mut project = sample_project();
    project.media_folders.push(MediaFolder {
        id: "folder-parent".to_string(),
        name: "Parent".to_string(),
        parent_id: None,
    });
    project.media_folders.push(MediaFolder {
        id: "folder-child".to_string(),
        name: "Child".to_string(),
        parent_id: Some("folder-parent".to_string()),
    });

    let folders =
        call_codex_local_tool(&project, "list_folders", json!({})).expect("folders payload");

    assert_eq!(
        folders.payload["folders"][1]["parentFolderId"],
        json!("folder-parent")
    );
    assert_eq!(
        folders.payload["folders"][1]["parentId"],
        json!("folder-parent")
    );
}

#[test]
fn codex_get_timeline_groups_palmier_caption_rows_inside_frame_window() {
    let mut project = sample_project();
    let caption_track = project
        .timeline
        .tracks
        .iter_mut()
        .find(|track| track.kind == video_creater_lib::project::model::TrackKind::Caption)
        .expect("caption track should exist");

    let grouped_caption = |id: &str, text: &str, start_seconds: f64, font_size: u64| TimelineItem {
        id: id.to_string(),
        kind: TimelineItemKind::Caption,
        start_seconds,
        duration_seconds: 0.5,
        source: TimelineSource::Text {
            text: text.to_string(),
        },
        label: text.to_string(),
        properties: BTreeMap::from([
            ("captionGroupId".to_string(), json!("captions-main")),
            ("fontSize".to_string(), json!(font_size)),
            ("fontFamily".to_string(), json!("Inter")),
        ]),
    };
    caption_track
        .items
        .push(grouped_caption("caption-1", "First", 0.5, 42));
    caption_track
        .items
        .push(grouped_caption("caption-2", "Second", 1.25, 42));
    caption_track
        .items
        .push(grouped_caption("caption-3", "Third", 2.0, 64));
    caption_track.items.push(TimelineItem {
        id: "caption-outside-window".to_string(),
        kind: TimelineItemKind::Caption,
        start_seconds: 3.5,
        duration_seconds: 0.25,
        source: TimelineSource::Text {
            text: "Outside".to_string(),
        },
        label: "Outside".to_string(),
        properties: BTreeMap::new(),
    });

    let timeline = call_codex_local_tool(
        &project,
        "video_creater.get_timeline",
        json!({ "startFrame": 0, "endFrame": 72 }),
    )
    .expect("windowed timeline payload");

    assert_eq!(
        timeline.payload["window"],
        json!({
            "startFrame": 0,
            "endFrame": 72,
            "startSeconds": 0.0,
            "endSeconds": 3.0
        })
    );
    let caption_track = timeline.payload["tracks"]
        .as_array()
        .expect("tracks")
        .iter()
        .find(|track| track["kind"] == json!("caption"))
        .expect("caption track payload");
    let visible_item_ids = caption_track["items"]
        .as_array()
        .expect("caption items")
        .iter()
        .map(|item| item["id"].as_str().expect("item id"))
        .collect::<Vec<_>>();
    assert!(!visible_item_ids.contains(&"caption-outside-window"));

    assert_eq!(
        caption_track["captionGroups"][0]["clipFormat"],
        json!(["clipId", "startFrame", "durationFrames", "text"])
    );
    assert_eq!(
        caption_track["captionGroups"][0]["clips"],
        json!([
            ["caption-1", 12, 12, "First"],
            ["caption-2", 30, 12, "Second"]
        ])
    );
    assert_eq!(
        caption_track["captionGroups"][0]["shared"]["properties"],
        json!({
            "fontFamily": "Inter",
            "fontSize": 42
        })
    );
    assert_eq!(
        caption_track["captionGroups"][0]["deviantClipIds"],
        json!(["caption-3"])
    );
}

#[test]
fn codex_get_timeline_caps_palmier_caption_group_rows_and_pages_with_window() {
    let mut project = sample_project();
    let caption_track = project
        .timeline
        .tracks
        .iter_mut()
        .find(|track| track.kind == video_creater_lib::project::model::TrackKind::Caption)
        .expect("caption track should exist");

    for index in 0..250 {
        caption_track.items.push(TimelineItem {
            id: format!("caption-{index}"),
            kind: TimelineItemKind::Caption,
            start_seconds: index as f64,
            duration_seconds: 1.0,
            source: TimelineSource::Text {
                text: format!("t{index}"),
            },
            label: format!("t{index}"),
            properties: BTreeMap::from([
                ("captionGroupId".to_string(), json!("captions-main")),
                ("fontSize".to_string(), json!(42)),
            ]),
        });
    }

    let timeline =
        call_codex_local_tool(&project, "get_timeline", json!({})).expect("timeline payload");
    let caption_track = timeline.payload["tracks"]
        .as_array()
        .expect("tracks")
        .iter()
        .find(|track| track["kind"] == json!("caption"))
        .expect("caption track payload");
    let group = &caption_track["captionGroups"][0];
    assert_eq!(group["clipCount"], json!(250));
    assert_eq!(group["clips"].as_array().expect("caption rows").len(), 200);
    assert!(group["clipsNote"]
        .as_str()
        .expect("clips note")
        .contains("250"));

    let paged = call_codex_local_tool(
        &project,
        "get_timeline",
        json!({
            "startFrame": 4800,
            "endFrame": 6000
        }),
    )
    .expect("windowed timeline payload");
    let paged_caption_track = paged.payload["tracks"]
        .as_array()
        .expect("tracks")
        .iter()
        .find(|track| track["kind"] == json!("caption"))
        .expect("caption track payload");
    let paged_group = &paged_caption_track["captionGroups"][0];
    assert_eq!(
        paged_group["clips"]
            .as_array()
            .expect("paged caption rows")
            .len(),
        50
    );
    assert_eq!(paged_group["clips"][0][0], json!("caption-200"));
    assert!(paged_group["clipsNote"].is_null());
}

#[test]
fn codex_accepts_bare_palmier_tool_names() {
    let mut project = sample_project();
    project.transcripts.push(Transcript {
        id: "transcript-1".to_string(),
        media_id: "media-1".to_string(),
        engine: Some("fixture".to_string()),
        raw_artifact_path: None,
        repairs: Vec::new(),
        segments: Vec::new(),
        words: vec![TranscriptWord {
            text: "launch".to_string(),
            start_seconds: 0.2,
            end_seconds: 0.5,
            confidence: Some(0.99),
            speaker: None,
        }],
    });

    let timeline = call_codex_local_tool(&project, "get_timeline", json!({}))
        .expect("bare get_timeline alias");
    assert_eq!(timeline.payload["tracks"][0]["id"], json!("track-video"));

    let media =
        call_codex_local_tool(&project, "get_media", json!({})).expect("bare get_media alias");
    assert_eq!(media.payload["media"][0]["id"], json!("media-1"));

    let search = call_codex_local_tool(
        &project,
        "search_media",
        json!({ "query": "launch", "scope": "spoken" }),
    )
    .expect("bare search_media alias");
    assert_eq!(
        search.payload["groups"]["spoken"][0]["mediaId"],
        json!("media-1")
    );
}

#[test]
fn codex_get_transcript_returns_timeline_mapped_word_ranges() {
    let mut project = sample_project();
    project.timeline.tracks[0].items[0].start_seconds = 10.0;
    project.timeline.tracks[0].items[0]
        .properties
        .insert("sourceIn".to_string(), json!(2.0));
    project.timeline.tracks[0].items[0]
        .properties
        .insert("sourceOut".to_string(), json!(6.0));
    project.transcripts.push(Transcript {
        id: "transcript-1".to_string(),
        media_id: "media-1".to_string(),
        engine: Some("fixture".to_string()),
        raw_artifact_path: None,
        repairs: Vec::new(),
        segments: Vec::new(),
        words: vec![TranscriptWord {
            text: "launch".to_string(),
            start_seconds: 3.0,
            end_seconds: 3.4,
            confidence: Some(0.99),
            speaker: None,
        }],
    });

    let transcript = call_codex_local_tool(
        &project,
        "video_creater.get_transcript",
        json!({ "mediaId": "media-1" }),
    )
    .expect("transcript alias payload");

    assert_eq!(transcript.payload["words"][0]["startSeconds"], json!(3.0));
    assert_eq!(
        transcript.payload["words"][0]["timelineStartSeconds"],
        json!(11.0)
    );
    assert_eq!(
        transcript.payload["words"][0]["timelineEndSeconds"],
        json!(11.4)
    );
    assert_eq!(
        transcript.payload["words"][0]["timelineRanges"][0]["itemId"],
        json!("item-1")
    );
    assert_eq!(
        transcript.payload["words"][0]["timelineRanges"][0]["sourceIn"],
        json!(2.0)
    );
}

#[test]
fn codex_transcription_readiness_reports_startable_media_without_transcript() {
    let project = sample_project();

    let readiness = call_codex_local_tool(
        &project,
        "video_creater.transcription_readiness",
        json!({ "mediaId": "media-1", "modelId": "nvidia/parakeet-tdt-0.6b-v3" }),
    )
    .expect("transcription readiness payload");

    assert_eq!(readiness.payload["ready"], json!(false));
    assert_eq!(readiness.payload["readyToTranscribe"], json!(true));
    assert_eq!(readiness.payload["transcriptStatus"], json!("missing"));
    assert_eq!(readiness.payload["mediaKind"], json!("video"));
    assert_eq!(
        readiness.payload["nextAction"],
        json!("Start transcription for this media, then call video_creater.get_transcript.")
    );
    assert!(readiness.payload["error"]
        .as_str()
        .expect("missing transcript error")
        .contains("transcript was not found"));
}

#[test]
fn codex_transcription_readiness_rejects_non_transcribable_media() {
    let mut project = sample_project();
    project.media.push(MediaAsset {
        id: "still-1".to_string(),
        name: Some("Poster frame".to_string()),
        relative_path: "media/still.png".to_string(),
        kind: MediaKind::Image,
        duration_seconds: 0.0,
        width: Some(1280),
        height: Some(720),
        fps: None,
        folder_id: None,
    });

    let readiness = call_codex_local_tool(
        &project,
        "video_creater.transcription_readiness",
        json!({ "mediaId": "still-1" }),
    )
    .expect("transcription readiness payload");

    assert_eq!(readiness.payload["ready"], json!(false));
    assert_eq!(readiness.payload["readyToTranscribe"], json!(false));
    assert_eq!(
        readiness.payload["transcriptStatus"],
        json!("unsupportedMedia")
    );
    assert_eq!(readiness.payload["mediaKind"], json!("image"));
    assert!(readiness.payload["error"]
        .as_str()
        .expect("unsupported media error")
        .contains("audio, video, or generated"));
}

#[test]
fn codex_get_transcript_without_media_id_returns_timeline_transcript() {
    let mut project = sample_project();
    project.timeline.tracks[0].items[0].start_seconds = 2.0;
    project.timeline.tracks[0].items[0].duration_seconds = 3.0;
    project.timeline.tracks[0].items[0]
        .properties
        .insert("sourceIn".to_string(), json!(1.0));
    project.timeline.tracks[0].items[0]
        .properties
        .insert("sourceOut".to_string(), json!(4.0));
    project.timeline.tracks[0].items.push(TimelineItem {
        id: "item-2".to_string(),
        kind: TimelineItemKind::VideoClip,
        start_seconds: 6.0,
        duration_seconds: 2.0,
        source: TimelineSource::Media {
            media_id: "media-1".to_string(),
        },
        label: "Second beat".to_string(),
        properties: BTreeMap::from([
            ("sourceIn".to_string(), json!(8.0)),
            ("sourceOut".to_string(), json!(10.0)),
        ]),
    });
    project.transcripts.push(Transcript {
        id: "transcript-1".to_string(),
        media_id: "media-1".to_string(),
        engine: Some("fixture".to_string()),
        raw_artifact_path: None,
        repairs: Vec::new(),
        segments: Vec::new(),
        words: vec![
            TranscriptWord {
                text: "setup".to_string(),
                start_seconds: 1.5,
                end_seconds: 1.8,
                confidence: Some(0.9),
                speaker: None,
            },
            TranscriptWord {
                text: "skip".to_string(),
                start_seconds: 5.0,
                end_seconds: 5.2,
                confidence: Some(0.8),
                speaker: None,
            },
            TranscriptWord {
                text: "payoff".to_string(),
                start_seconds: 8.5,
                end_seconds: 8.9,
                confidence: Some(0.95),
                speaker: None,
            },
        ],
    });

    let transcript = call_codex_local_tool(
        &project,
        "video_creater.get_transcript",
        json!({ "limit": 10 }),
    )
    .expect("timeline transcript payload");

    assert_eq!(transcript.payload["mode"], json!("timeline"));
    assert_eq!(transcript.payload["totalWords"], json!(2));
    assert_eq!(
        transcript.payload["wordFormat"],
        json!(["wordIndex", "text", "startSeconds", "endSeconds"])
    );
    assert_eq!(transcript.payload["words"][0]["text"], json!("setup"));
    assert_eq!(transcript.payload["words"][0]["wordIndex"], json!(0));
    assert_eq!(
        transcript.payload["words"][0]["timelineStartSeconds"],
        json!(2.5)
    );
    assert_eq!(transcript.payload["words"][1]["text"], json!("payoff"));
    assert_eq!(transcript.payload["words"][1]["wordIndex"], json!(1));
    assert_eq!(
        transcript.payload["words"][1]["timelineStartSeconds"],
        json!(6.5)
    );
    assert_eq!(transcript.payload["clips"][0]["clipId"], json!("item-1"));
    assert_eq!(
        transcript.payload["clips"][0]["words"][0][1],
        json!("setup")
    );
    assert_eq!(transcript.payload["clips"][1]["clipId"], json!("item-2"));
    assert_eq!(transcript.payload["clips"][1]["words"][0][0], json!(1));
    assert_eq!(
        transcript.payload["clips"][1]["words"][0][1],
        json!("payoff")
    );
}

#[test]
fn codex_get_transcript_maps_speed_retimed_clip_words_to_timeline() {
    let mut project = sample_project();
    project.render_settings.fps = 24.0;
    project.timeline.tracks[0].items[0].start_seconds = 5.0;
    project.timeline.tracks[0].items[0].duration_seconds = 2.0;
    project.timeline.tracks[0].items[0]
        .properties
        .insert("sourceIn".to_string(), json!(10.0));
    project.timeline.tracks[0].items[0]
        .properties
        .insert("speed".to_string(), json!(2.0));
    project.transcripts.push(Transcript {
        id: "transcript-1".to_string(),
        media_id: "media-1".to_string(),
        engine: Some("fixture".to_string()),
        raw_artifact_path: None,
        repairs: Vec::new(),
        segments: Vec::new(),
        words: vec![
            TranscriptWord {
                text: "fast".to_string(),
                start_seconds: 11.0,
                end_seconds: 11.5,
                confidence: Some(0.9),
                speaker: None,
            },
            TranscriptWord {
                text: "tail".to_string(),
                start_seconds: 13.5,
                end_seconds: 13.9,
                confidence: Some(0.8),
                speaker: None,
            },
        ],
    });

    let timeline_transcript = call_codex_local_tool(
        &project,
        "video_creater.get_transcript",
        json!({ "limit": 10 }),
    )
    .expect("speed-retimed timeline transcript payload");

    assert_eq!(timeline_transcript.payload["mode"], json!("timeline"));
    assert_eq!(timeline_transcript.payload["totalWords"], json!(2));
    assert_eq!(
        timeline_transcript.payload["words"][0]["timelineStartSeconds"],
        json!(5.5)
    );
    assert_eq!(
        timeline_transcript.payload["words"][0]["timelineEndSeconds"],
        json!(5.75)
    );
    assert_eq!(
        timeline_transcript.payload["words"][1]["timelineStartSeconds"],
        json!(6.75)
    );
    assert_eq!(
        timeline_transcript.payload["clips"][0]["sourceOut"],
        json!(14.0)
    );

    let media_transcript = call_codex_local_tool(
        &project,
        "video_creater.get_transcript",
        json!({ "mediaId": "media-1", "startSeconds": 13.0, "endSeconds": 14.0 }),
    )
    .expect("media transcript payload with retimed timeline range");

    assert_eq!(media_transcript.payload["words"][0]["text"], json!("tail"));
    assert_eq!(
        media_transcript.payload["words"][0]["timelineRanges"][0]["timelineStartSeconds"],
        json!(6.75)
    );
    assert_eq!(
        media_transcript.payload["words"][0]["timelineRanges"][0]["timelineEndSeconds"],
        json!(6.95)
    );
    assert_eq!(
        media_transcript.payload["words"][0]["timelineRanges"][0]["sourceOut"],
        json!(14.0)
    );
}

#[test]
fn codex_remove_words_uses_palmier_timeline_word_indices() {
    let mut project = sample_project();
    project.timeline.tracks[0].items[0].start_seconds = 2.0;
    project.timeline.tracks[0].items[0].duration_seconds = 3.0;
    project.timeline.tracks[0].items[0]
        .properties
        .insert("sourceIn".to_string(), json!(1.0));
    project.timeline.tracks[0].items[0]
        .properties
        .insert("sourceOut".to_string(), json!(4.0));
    project.timeline.tracks[0].items.push(TimelineItem {
        id: "item-2".to_string(),
        kind: TimelineItemKind::VideoClip,
        start_seconds: 6.0,
        duration_seconds: 2.0,
        source: TimelineSource::Media {
            media_id: "media-1".to_string(),
        },
        label: "Second beat".to_string(),
        properties: BTreeMap::from([
            ("sourceIn".to_string(), json!(8.0)),
            ("sourceOut".to_string(), json!(10.0)),
        ]),
    });
    project.transcripts.push(Transcript {
        id: "transcript-1".to_string(),
        media_id: "media-1".to_string(),
        engine: Some("fixture".to_string()),
        raw_artifact_path: None,
        repairs: Vec::new(),
        segments: Vec::new(),
        words: vec![
            TranscriptWord {
                text: "setup".to_string(),
                start_seconds: 1.5,
                end_seconds: 1.8,
                confidence: Some(0.9),
                speaker: None,
            },
            TranscriptWord {
                text: "skip".to_string(),
                start_seconds: 5.0,
                end_seconds: 5.2,
                confidence: Some(0.8),
                speaker: None,
            },
            TranscriptWord {
                text: "payoff".to_string(),
                start_seconds: 8.5,
                end_seconds: 8.9,
                confidence: Some(0.95),
                speaker: None,
            },
        ],
    });

    let result = call_codex_local_tool(&project, "remove_words", json!({ "words": [1] }))
        .expect("Palmier timeline word index should remove the second visible word");

    assert!(result.mutates_project);
    assert_eq!(result.payload["valid"], json!(true));
    assert_eq!(
        result.payload["removedWordRanges"][0]["text"],
        json!("payoff")
    );
    assert_eq!(
        result.payload["removedWordRanges"][0]["wordIndex"],
        json!(1)
    );
    assert_eq!(
        result.payload["ranges"][0],
        json!({
            "startSeconds": 6.5,
            "endSeconds": 6.9,
            "trackIds": ["track-video"]
        })
    );
}

#[test]
fn codex_get_transcript_returns_palmier_frame_word_rows() {
    let mut project = sample_project();
    project.timeline.tracks[0].items[0].start_seconds = 2.0;
    project.timeline.tracks[0].items[0].duration_seconds = 3.0;
    project.timeline.tracks[0].items[0]
        .properties
        .insert("sourceIn".to_string(), json!(1.0));
    project.timeline.tracks[0].items[0]
        .properties
        .insert("sourceOut".to_string(), json!(4.0));
    project.transcripts.push(Transcript {
        id: "transcript-1".to_string(),
        media_id: "media-1".to_string(),
        engine: Some("fixture".to_string()),
        raw_artifact_path: None,
        repairs: Vec::new(),
        segments: Vec::new(),
        words: vec![TranscriptWord {
            text: "setup".to_string(),
            start_seconds: 1.5,
            end_seconds: 1.8,
            confidence: Some(0.9),
            speaker: None,
        }],
    });

    let transcript = call_codex_local_tool(
        &project,
        "get_transcript",
        json!({
            "startFrame": 48,
            "endFrame": 96,
            "clipId": "item-1",
            "limit": 10
        }),
    )
    .expect("Palmier frame transcript payload");

    assert_eq!(
        transcript.payload["frameWordFormat"],
        json!(["wordIndex", "text", "startFrame", "endFrame"])
    );
    assert_eq!(transcript.payload["words"][0]["startFrame"], json!(60));
    assert_eq!(transcript.payload["words"][0]["endFrame"], json!(67));
    assert_eq!(
        transcript.payload["clips"][0]["frameWordFormat"],
        json!(["wordIndex", "text", "startFrame", "endFrame"])
    );
    assert_eq!(
        transcript.payload["clips"][0]["frameWords"][0],
        json!([0, "setup", 60, 67])
    );
}

#[test]
fn codex_get_transcript_compact_rows_include_speakers_when_available() {
    let mut project = sample_project();
    project.timeline.tracks[0].items[0].start_seconds = 2.0;
    project.timeline.tracks[0].items[0].duration_seconds = 3.0;
    project.timeline.tracks[0].items[0]
        .properties
        .insert("sourceIn".to_string(), json!(1.0));
    project.timeline.tracks[0].items[0]
        .properties
        .insert("sourceOut".to_string(), json!(4.0));
    project.transcripts.push(Transcript {
        id: "transcript-1".to_string(),
        media_id: "media-1".to_string(),
        engine: Some("fixture".to_string()),
        raw_artifact_path: None,
        repairs: Vec::new(),
        segments: Vec::new(),
        words: vec![
            TranscriptWord {
                text: "founder".to_string(),
                start_seconds: 1.5,
                end_seconds: 1.8,
                confidence: Some(0.9),
                speaker: Some("Speaker 1".to_string()),
            },
            TranscriptWord {
                text: "launch".to_string(),
                start_seconds: 2.2,
                end_seconds: 2.5,
                confidence: Some(0.88),
                speaker: None,
            },
        ],
    });

    let transcript = call_codex_local_tool(
        &project,
        "get_transcript",
        json!({
            "clipId": "item-1",
            "limit": 10
        }),
    )
    .expect("speaker-aware Palmier transcript payload");

    assert_eq!(
        transcript.payload["wordFormat"],
        json!(["wordIndex", "text", "startSeconds", "endSeconds", "speaker"])
    );
    assert_eq!(
        transcript.payload["frameWordFormat"],
        json!(["wordIndex", "text", "startFrame", "endFrame", "speaker"])
    );
    assert_eq!(
        transcript.payload["clips"][0]["wordFormat"],
        json!(["wordIndex", "text", "startSeconds", "endSeconds", "speaker"])
    );
    assert_eq!(
        transcript.payload["clips"][0]["frameWordFormat"],
        json!(["wordIndex", "text", "startFrame", "endFrame", "speaker"])
    );
    assert_eq!(
        transcript.payload["clips"][0]["words"][0],
        json!([0, "founder", 2.5, 2.8, "Speaker 1"])
    );
    assert_eq!(
        transcript.payload["clips"][0]["frameWords"][0],
        json!([0, "founder", 60, 67, "Speaker 1"])
    );
    assert_eq!(
        transcript.payload["clips"][0]["words"][1],
        json!([1, "launch", 3.2, 3.5, null])
    );
}

#[test]
fn codex_get_transcript_tolerates_word_timestamps_flag() {
    let project = sample_project();

    let transcript = call_codex_local_tool(
        &project,
        "get_transcript",
        json!({ "wordTimestamps": true }),
    )
    .expect("get_transcript should tolerate inspect_media wordTimestamps habit");

    assert_eq!(transcript.payload["mode"], json!("timeline"));
}

#[test]
fn codex_transcript_words_requires_media_id() {
    let project = sample_project();

    let error = call_codex_local_tool(&project, "video_creater.transcript_words", json!({}))
        .expect_err("media-scoped transcript_words should require mediaId");

    assert!(error.to_string().contains("mediaId is required"));
}

#[test]
fn codex_inspect_timeline_returns_preview_layers() {
    let mut project = sample_project();
    project.media.push(MediaAsset {
        id: "generated-media-1".to_string(),
        name: Some("Generated skyline".to_string()),
        relative_path: "generated/skyline.mp4".to_string(),
        kind: MediaKind::Generated,
        duration_seconds: 4.0,
        width: Some(1280),
        height: Some(720),
        fps: Some(24.0),
        folder_id: None,
    });
    project.generated_assets.push(GeneratedAsset {
        schema_version: 1,
        id: "generated-shot-1".to_string(),
        kind: MediaKind::Generated,
        status: GeneratedAssetStatus::Completed,
        name: Some("Generated skyline".to_string()),
        target_folder_id: None,
        placement_intent: Some("timeline".to_string()),
        prompt: "A generated skyline shot".to_string(),
        model: GenerationModel {
            provider: "mock".to_string(),
            id: "video".to_string(),
        },
        references: GeneratedAssetReferences {
            media_ids: Vec::new(),
            first_frame_media_id: None,
            last_frame_media_id: None,
            provider_input_urls: Vec::new(),
            ..Default::default()
        },
        settings: GeneratedAssetSettings {
            width: Some(1280),
            height: Some(720),
            duration_seconds: Some(4.0),
            fps: Some(24.0),
            aspect_ratio: Some("16:9".to_string()),
            resolution: None,
            generate_audio: None,
            ..GeneratedAssetSettings::default()
        },
        outputs: vec![GeneratedAssetOutput {
            media_id: "generated-media-1".to_string(),
            relative_path: "generated/skyline.mp4".to_string(),
            source_url: None,
            width: 1280,
            height: 720,
            duration_seconds: 4.0,
            fps: 24.0,
        }],
        created_at: "2026-07-01T00:00:00Z".to_string(),
        parent_asset_id: None,
        retry_of_asset_id: None,
    });

    let video_track = project
        .timeline
        .tracks
        .iter_mut()
        .find(|track| track.kind == video_creater_lib::project::model::TrackKind::Video)
        .expect("video track should exist");
    video_track.items.push(TimelineItem {
        id: "generated-clip-1".to_string(),
        kind: TimelineItemKind::VideoClip,
        start_seconds: 1.0,
        duration_seconds: 2.0,
        source: TimelineSource::Generated {
            artifact_id: "generated-shot-1".to_string(),
        },
        label: "Generated skyline".to_string(),
        properties: BTreeMap::from([("sourceIn".to_string(), json!(0.5))]),
    });
    let caption_track = project
        .timeline
        .tracks
        .iter_mut()
        .find(|track| track.kind == video_creater_lib::project::model::TrackKind::Caption)
        .expect("caption track should exist");
    caption_track.items.push(TimelineItem {
        id: "caption-1".to_string(),
        kind: TimelineItemKind::Caption,
        start_seconds: 1.0,
        duration_seconds: 1.5,
        source: TimelineSource::Text {
            text: "Launch day".to_string(),
        },
        label: "Launch day".to_string(),
        properties: BTreeMap::new(),
    });

    let result = call_codex_local_tool(
        &project,
        "video_creater.inspect_timeline",
        json!({ "startSeconds": 1.25, "endSeconds": 1.5 }),
    )
    .expect("inspect timeline payload");

    assert_eq!(result.payload["previewTimeSeconds"], json!(1.25));
    assert_eq!(
        result.payload["previewLayers"][0]["itemId"],
        json!("item-1")
    );
    assert_eq!(
        result.payload["previewLayers"][1]["itemId"],
        json!("generated-clip-1")
    );
    assert_eq!(
        result.payload["previewLayers"][1]["mediaId"],
        json!("generated-media-1")
    );
    assert_eq!(
        result.payload["previewLayers"][1]["sourceTimeSeconds"],
        json!(0.75)
    );
    assert_eq!(
        result.payload["previewLayers"][2]["overlayKind"],
        json!("caption")
    );
}

fn sample_project_with_completed_generated_asset() -> video_creater_lib::project::model::VideoProject
{
    let mut project = sample_project();
    project.media_folders.push(MediaFolder {
        id: "folder-scene-a".to_string(),
        name: "Scene A".to_string(),
        parent_id: None,
    });
    project.media.push(MediaAsset {
        id: "audio-1".to_string(),
        name: Some("Reference audio".to_string()),
        relative_path: "media/reference.wav".to_string(),
        kind: MediaKind::Audio,
        duration_seconds: 5.0,
        width: None,
        height: None,
        fps: None,
        folder_id: None,
    });
    project.media.push(MediaAsset {
        id: "image-ref".to_string(),
        name: Some("Reference frame".to_string()),
        relative_path: "media/reference-frame.png".to_string(),
        kind: MediaKind::Image,
        duration_seconds: 0.0,
        width: Some(1280),
        height: Some(720),
        fps: None,
        folder_id: None,
    });
    project.generated_assets.push(GeneratedAsset {
        schema_version: 1,
        id: "generated-shot-1".to_string(),
        kind: MediaKind::Generated,
        status: GeneratedAssetStatus::Completed,
        name: Some("Generated skyline".to_string()),
        target_folder_id: Some("folder-scene-a".to_string()),
        placement_intent: Some("timeline".to_string()),
        prompt: "A generated skyline shot".to_string(),
        model: GenerationModel {
            provider: "fal.ai".to_string(),
            id: FAL_WAN_IMAGE_TO_VIDEO_MODEL_ID.to_string(),
        },
        references: GeneratedAssetReferences {
            media_ids: vec!["image-ref".to_string()],
            first_frame_media_id: Some("image-ref".to_string()),
            last_frame_media_id: None,
            provider_input_urls: Vec::new(),
            reference_audio_media_refs: vec!["audio-1".to_string()],
            ..Default::default()
        },
        settings: GeneratedAssetSettings {
            width: Some(1280),
            height: Some(720),
            duration_seconds: Some(5.0),
            fps: Some(24.0),
            aspect_ratio: Some("16:9".to_string()),
            resolution: Some("720p".to_string()),
            generate_audio: Some(true),
            ..GeneratedAssetSettings::default()
        },
        outputs: vec![GeneratedAssetOutput {
            media_id: "generated-media-1".to_string(),
            relative_path: "generated/skyline.mp4".to_string(),
            source_url: Some("https://fal.media/generated/skyline.mp4".to_string()),
            width: 1280,
            height: 720,
            duration_seconds: 5.0,
            fps: 24.0,
        }],
        created_at: "2026-07-01T00:00:00Z".to_string(),
        parent_asset_id: None,
        retry_of_asset_id: None,
    });
    project
}

#[test]
fn codex_rerun_generated_asset_queues_variation_from_stored_recipe() {
    let project = sample_project_with_completed_generated_asset();

    let result = call_codex_local_tool(
        &project,
        "video_creater.rerun_generated_asset",
        json!({
            "projectDir": "/tmp/video-creater-project",
            "assetId": "generated-shot-1-rerun",
            "jobId": "job-generated-shot-1-rerun",
            "mockMode": true,
            "sourceAssetId": "generated-shot-1",
            "prompt": "A warmer skyline rerun"
        }),
    )
    .expect("generated asset rerun should validate");

    assert!(!result.mutates_project);
    assert_eq!(
        result.payload["startRequest"]["input"]["assetId"],
        json!("generated-shot-1-rerun")
    );
    assert_eq!(
        result.payload["startRequest"]["input"]["prompt"],
        json!("A warmer skyline rerun")
    );
    assert_eq!(
        result.payload["startRequest"]["input"]["model"],
        json!({
            "provider": "fal.ai",
            "id": FAL_WAN_IMAGE_TO_VIDEO_MODEL_ID
        })
    );
    assert_eq!(
        result.payload["startRequest"]["input"]["references"]["firstFrameMediaId"],
        json!("image-ref")
    );
    assert_eq!(
        result.payload["startRequest"]["input"]["references"]["referenceAudioMediaRefs"],
        json!(["audio-1"])
    );
    assert_eq!(
        result.payload["startRequest"]["input"]["settings"]["resolution"],
        json!("720p")
    );
    assert_eq!(
        result.payload["projectActions"][0]["asset"]["parentAssetId"],
        json!("generated-shot-1")
    );
    assert_eq!(
        result.payload["projectActions"][0]["asset"]["retryOfAssetId"],
        json!("generated-shot-1")
    );
    assert_eq!(
        result.payload["projectActions"][0]["asset"]["targetFolderId"],
        json!("folder-scene-a")
    );
}

#[test]
fn codex_rerun_generated_asset_persists_placeholder_for_real_split_project() {
    let project = sample_project_with_completed_generated_asset();
    let project_dir = tempfile::tempdir().expect("rerun placeholder project dir");
    save_split_project(project_dir.path(), &project).expect("save rerun placeholder project");

    let result = call_codex_local_tool(
        &project,
        "video_creater.rerun_generated_asset",
        json!({
            "projectDir": project_dir.path().display().to_string(),
            "assetId": "generated-shot-1-rerun",
            "jobId": "job-generated-shot-1-rerun",
            "mockMode": true,
            "sourceAssetId": "generated-shot-1",
            "prompt": "A warmer skyline rerun"
        }),
    )
    .expect("generated asset rerun should persist placeholder actions");

    assert!(result.mutates_project);
    assert_eq!(
        result.payload["projectActionApplication"]["applied"],
        json!(true)
    );
    assert_eq!(
        result.payload["persistedPlaceholderAssetId"],
        json!("generated-shot-1-rerun")
    );

    let persisted = load_split_project(project_dir.path()).expect("load rerun placeholder project");
    let generated = persisted
        .generated_assets
        .iter()
        .find(|asset| asset.id == "generated-shot-1-rerun")
        .expect("persisted rerun generated placeholder");
    assert_eq!(generated.status, GeneratedAssetStatus::Queued);
    assert_eq!(
        generated.parent_asset_id.as_deref(),
        Some("generated-shot-1")
    );
    assert_eq!(
        generated.retry_of_asset_id.as_deref(),
        Some("generated-shot-1")
    );
    let job = persisted
        .jobs
        .iter()
        .find(|job| job.id == "job-generated-shot-1-rerun")
        .expect("persisted rerun generation job");
    assert_eq!(
        job.status,
        video_creater_lib::project::model::JobStatus::Queued
    );
}

#[test]
fn codex_rerun_generated_asset_defaults_missing_video_generate_audio() {
    let mut project = sample_project_with_completed_generated_asset();
    project.generated_assets[0].settings.generate_audio = None;

    let result = call_codex_local_tool(
        &project,
        "video_creater.rerun_generated_asset",
        json!({
            "projectDir": "/tmp/video-creater-project",
            "assetId": "generated-shot-legacy-rerun",
            "jobId": "job-generated-shot-legacy-rerun",
            "mockMode": true,
            "sourceAssetId": "generated-shot-1"
        }),
    )
    .expect("legacy generated video rerun should default generateAudio");

    assert_eq!(
        result.payload["startRequest"]["input"]["settings"]["generateAudio"],
        json!(true)
    );
    assert_eq!(
        result.payload["projectActions"][0]["asset"]["settings"]["generateAudio"],
        json!(true)
    );
}

#[test]
fn codex_rerun_generated_asset_recovers_legacy_source_video_reference() {
    let mut project = sample_project_with_completed_generated_asset();
    let asset = &mut project.generated_assets[0];
    asset.model = GenerationModel {
        provider: FAL_PROVIDER.to_string(),
        id: FAL_WAN_VIDEO_TO_VIDEO_MODEL_ID.to_string(),
    };
    asset.references = GeneratedAssetReferences {
        media_ids: vec!["media-1".to_string()],
        ..GeneratedAssetReferences::default()
    };
    asset.settings.video_source_start_seconds = Some(0.0);
    asset.settings.video_source_end_seconds = Some(5.0);

    let result = call_codex_local_tool(
        &project,
        "video_creater.rerun_generated_asset",
        json!({
            "projectDir": "/tmp/video-creater-project",
            "assetId": "generated-source-video-legacy-rerun",
            "jobId": "job-generated-source-video-legacy-rerun",
            "mockMode": true,
            "sourceAssetId": "generated-shot-1"
        }),
    )
    .expect("legacy source-video generated asset rerun should validate");

    assert_eq!(
        result.payload["startRequest"]["input"]["references"]["sourceVideoMediaRef"],
        json!("media-1")
    );
    assert_eq!(
        result.payload["projectActions"][0]["asset"]["references"]["sourceVideoMediaRef"],
        json!("media-1")
    );
}

#[test]
fn codex_rerun_generated_asset_preserves_kling_motion_control_recipe() {
    let mut project = sample_project_with_completed_generated_asset();
    let asset = &mut project.generated_assets[0];
    asset.model = GenerationModel {
        provider: FAL_PROVIDER.to_string(),
        id: FAL_KLING_V3_PRO_MOTION_CONTROL_MODEL_ID.to_string(),
    };
    asset.references = GeneratedAssetReferences {
        media_ids: vec!["media-1".to_string(), "image-ref".to_string()],
        source_video_media_ref: Some("media-1".to_string()),
        reference_image_media_refs: vec!["image-ref".to_string()],
        provider_input_urls: vec![
            "https://fal.media/uploads/source.mp4".to_string(),
            "https://fal.media/uploads/reference.png".to_string(),
        ],
        ..GeneratedAssetReferences::default()
    };
    asset.settings.video_source_start_seconds = Some(0.0);
    asset.settings.video_source_end_seconds = Some(5.0);

    let result = call_codex_local_tool(
        &project,
        "video_creater.rerun_generated_asset",
        json!({
            "projectDir": "/tmp/video-creater-project",
            "assetId": "generated-kling-motion-control-rerun",
            "jobId": "job-generated-kling-motion-control-rerun",
            "mockMode": true,
            "sourceAssetId": "generated-shot-1",
            "prompt": "A warmer character motion rerun"
        }),
    )
    .expect("Kling motion-control generated asset rerun should validate");

    assert_eq!(
        result.payload["startRequest"]["input"]["model"],
        json!({
            "provider": FAL_PROVIDER,
            "id": FAL_KLING_V3_PRO_MOTION_CONTROL_MODEL_ID
        })
    );
    assert_eq!(
        result.payload["startRequest"]["input"]["references"]["sourceVideoMediaRef"],
        json!("media-1")
    );
    assert_eq!(
        result.payload["startRequest"]["input"]["references"]["referenceImageMediaRefs"],
        json!(["image-ref"])
    );
    assert_eq!(
        result.payload["startRequest"]["input"]["references"]["providerInputUrls"],
        json!([
            "https://fal.media/uploads/source.mp4",
            "https://fal.media/uploads/reference.png"
        ])
    );
}

#[test]
fn codex_rerun_generated_asset_preserves_wan_reference_to_video_recipe() {
    let mut project = sample_project_with_completed_generated_asset();
    let asset = &mut project.generated_assets[0];
    asset.model = GenerationModel {
        provider: FAL_PROVIDER.to_string(),
        id: FAL_WAN_REFERENCE_TO_VIDEO_MODEL_ID.to_string(),
    };
    asset.references = GeneratedAssetReferences {
        media_ids: vec!["image-ref".to_string(), "media-1".to_string()],
        reference_image_media_refs: vec!["image-ref".to_string()],
        reference_video_media_refs: vec!["media-1".to_string()],
        provider_input_urls: vec![
            "https://fal.media/uploads/reference.png".to_string(),
            "https://fal.media/uploads/motion-reference.mp4".to_string(),
        ],
        ..GeneratedAssetReferences::default()
    };

    let result = call_codex_local_tool(
        &project,
        "video_creater.rerun_generated_asset",
        json!({
            "projectDir": "/tmp/video-creater-project",
            "assetId": "generated-wan-reference-rerun",
            "jobId": "job-generated-wan-reference-rerun",
            "mockMode": true,
            "sourceAssetId": "generated-shot-1",
            "prompt": "A refined reference-to-video rerun"
        }),
    )
    .expect("WAN reference-to-video generated asset rerun should validate");

    assert_eq!(
        result.payload["startRequest"]["input"]["model"],
        json!({
            "provider": FAL_PROVIDER,
            "id": FAL_WAN_REFERENCE_TO_VIDEO_MODEL_ID
        })
    );
    assert_eq!(
        result.payload["startRequest"]["input"]["references"]["referenceImageMediaRefs"],
        json!(["image-ref"])
    );
    assert_eq!(
        result.payload["startRequest"]["input"]["references"]["referenceVideoMediaRefs"],
        json!(["media-1"])
    );
    assert_eq!(
        result.payload["startRequest"]["input"]["references"]["providerInputUrls"],
        json!([
            "https://fal.media/uploads/reference.png",
            "https://fal.media/uploads/motion-reference.mp4"
        ])
    );
}

#[test]
fn codex_rerun_generated_asset_can_override_replacement_target() {
    let project = sample_project_with_completed_generated_asset();

    let result = call_codex_local_tool(
        &project,
        "video_creater.rerun_generated_asset",
        json!({
            "sourceAssetId": "generated-shot-1",
            "replacementItemId": "item-1",
            "mockMode": true
        }),
    )
    .expect("generated asset rerun replacement should validate");

    assert_eq!(
        result.payload["startRequest"]["input"]["placementIntent"],
        json!("replace:item-1")
    );
    assert_eq!(
        result.payload["projectActions"][0]["asset"]["placementIntent"],
        json!("replace:item-1")
    );
    assert_eq!(
        result.payload["projectActions"][0]["asset"]["prompt"],
        json!("A generated skyline shot")
    );
}

#[test]
fn codex_rerun_generated_asset_preserves_upscale_model_recipe() {
    let mut project = sample_project();
    project.media.push(MediaAsset {
        id: "image-1".to_string(),
        name: Some("Poster frame".to_string()),
        relative_path: "media/poster.png".to_string(),
        kind: MediaKind::Image,
        duration_seconds: 0.0,
        width: Some(1024),
        height: Some(768),
        fps: None,
        folder_id: None,
    });
    project.generated_assets.push(GeneratedAsset {
        schema_version: 1,
        id: "upscaled-poster-1".to_string(),
        kind: MediaKind::Generated,
        status: GeneratedAssetStatus::Completed,
        name: Some("Upscaled poster".to_string()),
        target_folder_id: None,
        placement_intent: Some("library".to_string()),
        prompt: "Upscale Poster frame".to_string(),
        model: GenerationModel {
            provider: FAL_PROVIDER.to_string(),
            id: FAL_AURA_SR_MODEL_ID.to_string(),
        },
        references: GeneratedAssetReferences {
            media_ids: vec!["image-1".to_string()],
            first_frame_media_id: None,
            last_frame_media_id: None,
            reference_image_media_refs: vec!["image-1".to_string()],
            provider_input_urls: vec!["https://fal.media/uploads/source.png".to_string()],
            ..Default::default()
        },
        settings: GeneratedAssetSettings {
            width: Some(2048),
            height: Some(1536),
            duration_seconds: None,
            fps: None,
            aspect_ratio: Some("4:3".to_string()),
            ..GeneratedAssetSettings::default()
        },
        outputs: vec![GeneratedAssetOutput {
            media_id: "upscaled-poster-media-1".to_string(),
            relative_path: "generated/upscaled-poster.png".to_string(),
            source_url: Some("https://fal.media/generated/upscaled-poster.png".to_string()),
            width: 2048,
            height: 1536,
            duration_seconds: 0.0,
            fps: 0.0,
        }],
        created_at: "2026-07-01T00:00:00Z".to_string(),
        parent_asset_id: None,
        retry_of_asset_id: None,
    });

    let result = call_codex_local_tool(
        &project,
        "video_creater.rerun_generated_asset",
        json!({
            "projectDir": "/tmp/video-creater-project",
            "assetId": "upscaled-poster-rerun",
            "jobId": "job-upscaled-poster-rerun",
            "mockMode": true,
            "sourceAssetId": "upscaled-poster-1"
        }),
    )
    .expect("generated upscale rerun should validate");

    assert_eq!(
        result.payload["startRequest"]["input"]["model"],
        json!({
            "provider": FAL_PROVIDER,
            "id": FAL_AURA_SR_MODEL_ID
        })
    );
    assert_eq!(
        result.payload["startRequest"]["input"]["references"]["providerInputUrls"],
        json!(["https://fal.media/uploads/source.png"])
    );
    assert_eq!(
        result.payload["startRequest"]["input"]["references"]["referenceImageMediaRefs"],
        json!(["image-1"])
    );
    assert_eq!(
        result.payload["projectActions"][0]["asset"]["parentAssetId"],
        json!("upscaled-poster-1")
    );
    assert_eq!(
        result.payload["projectActions"][0]["asset"]["retryOfAssetId"],
        json!("upscaled-poster-1")
    );
}

#[test]
fn codex_inspect_timeline_accepts_palmier_frame_window() {
    let mut project = sample_project();
    project.timeline.tracks[0].items[0].start_seconds = 1.0;
    project.timeline.tracks[0].items[0].duration_seconds = 2.0;
    project.timeline.tracks[0].items[0].properties.insert(
        "keyframes".to_string(),
        json!({
            "opacity": [
                { "atSeconds": 0.0, "value": 0.2, "easing": null },
                { "atSeconds": 1.0, "value": 0.8, "easing": null }
            ],
            "positionX": [
                { "atSeconds": 0.0, "value": 10.0, "easing": null },
                { "atSeconds": 1.0, "value": 30.0, "easing": null }
            ],
            "rotationDegrees": [
                { "atSeconds": 0.0, "value": 0.0, "easing": null },
                { "atSeconds": 1.0, "value": 90.0, "easing": null }
            ]
        }),
    );

    let result = call_codex_local_tool(
        &project,
        "inspect_timeline",
        json!({
            "startFrame": 36,
            "endFrame": 72,
            "maxFrames": 3
        }),
    )
    .expect("Palmier frame inspect timeline payload");

    assert_eq!(result.payload["previewTimeSeconds"], json!(1.5));
    assert_eq!(result.payload["previewFrame"], json!(36));
    assert_eq!(result.payload["frameNumbers"], json!([36, 54, 71]));
    assert_eq!(result.payload["window"]["startFrame"], json!(36));
    assert_eq!(result.payload["window"]["endFrame"], json!(72));
    assert_eq!(result.payload["activeItems"][0]["startFrame"], json!(24));
    assert_eq!(
        result.payload["activeItems"][0]["durationFrames"],
        json!(48)
    );
    assert_eq!(result.payload["previewLayers"][0]["opacity"], json!(0.5));
    assert_eq!(result.payload["previewLayers"][0]["positionX"], json!(20.0));
    assert_eq!(
        result.payload["previewLayers"][0]["rotationDegrees"],
        json!(45.0)
    );
}

#[test]
fn codex_inspect_timeline_survives_huge_palmier_start_frame() {
    let project = sample_project();

    let result = call_codex_local_tool(
        &project,
        "inspect_timeline",
        json!({
            "startFrame": 1.0e19,
            "maxFrames": 3
        }),
    )
    .expect("huge Palmier inspect_timeline startFrame should return a bounded payload");

    assert_eq!(result.payload["previewFrame"], json!(0));
    assert_eq!(result.payload["frameNumbers"][0], json!(0));
    assert_eq!(result.payload["activeItems"][0]["itemId"], json!("item-1"));
}

#[test]
fn codex_search_media_returns_grouped_status_aware_results() {
    let mut project = sample_project();
    project.transcripts.push(Transcript {
        id: "transcript-1".to_string(),
        media_id: "media-1".to_string(),
        engine: Some("fixture".to_string()),
        raw_artifact_path: None,
        repairs: Vec::new(),
        segments: Vec::new(),
        words: vec![
            TranscriptWord {
                text: "launch".to_string(),
                start_seconds: 0.2,
                end_seconds: 0.5,
                confidence: Some(0.99),
                speaker: None,
            },
            TranscriptWord {
                text: "day".to_string(),
                start_seconds: 0.52,
                end_seconds: 0.8,
                confidence: Some(0.98),
                speaker: None,
            },
        ],
    });

    let result = call_codex_local_tool(
        &project,
        "video_creater.search_media",
        json!({ "query": "launch day", "limit": 5 }),
    )
    .expect("search media payload");

    assert_eq!(result.payload["visualStatus"], json!("notInstalled"));
    assert_eq!(
        result.payload["visual"]["status"],
        json!("modelNotInstalled")
    );
    assert_eq!(result.payload["visual"]["indexableAssets"], json!(1));
    assert_eq!(result.payload["visual"]["indexedAssets"], json!(0));
    assert_eq!(result.payload["visual"]["moments"], json!([]));
    assert_eq!(result.payload["spokenStatus"], json!("ready"));
    assert_eq!(
        result.payload["groups"]["spoken"][0]["mediaId"],
        json!("media-1")
    );
    assert_eq!(
        result.payload["groups"]["spoken"][0]["reason"],
        json!("matched transcript words")
    );
    assert_eq!(result.payload["results"][0]["kind"], json!("spoken"));
}

#[test]
fn codex_search_media_uses_project_dir_visual_sidecars() {
    let project = sample_project();
    let project_dir = tempfile::tempdir().expect("project dir");
    save_split_project(project_dir.path(), &project).expect("save split project");
    let sidecar_path = project_dir.path().join("search/visual/media-1/frames.json");
    let mut sidecar: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(&sidecar_path).expect("read visual sidecar"))
            .expect("parse visual sidecar");
    sidecar["status"] = json!("ready");
    sidecar["visualStatus"] = json!("ready");
    sidecar["embeddingModel"] = json!({
        "id": "siglip-local",
        "embeddingDim": 768
    });
    sidecar["frames"] = json!([
        {
            "index": 0,
            "timeSeconds": 0.0,
            "shotStartSeconds": 0.0,
            "shotEndSeconds": 1.8,
            "relativeFramePath": "search/visual/media-1/frames/frame-000000.png",
            "description": "wide harbor at sunset with orange sky",
            "score": 0.91
        }
    ]);
    fs::write(
        &sidecar_path,
        serde_json::to_string_pretty(&sidecar).expect("serialize visual sidecar"),
    )
    .expect("write ready visual sidecar");
    // Production MCP and Tauri tool calls with a projectDir always pass the canonical project
    // loaded from disk. The stored search index fingerprints that canonical project (including
    // its content revision), so an unsaved in-memory copy would be reported as stale.
    let project = load_split_project(project_dir.path()).expect("load canonical project");

    let result = call_codex_local_tool(
        &project,
        "video_creater.search_media",
        json!({
            "projectDir": project_dir.path().display().to_string(),
            "query": "harbor sunset",
            "scope": "visual",
            "limit": 5
        }),
    )
    .expect("search media payload");

    assert_eq!(result.payload["visualStatus"], json!("ready"));
    assert_eq!(result.payload["status"], json!("ready"));
    assert_eq!(
        result.payload["groups"]["visual"][0]["mediaId"],
        json!("media-1")
    );
    assert_eq!(result.payload["moments"][0]["mediaRef"], json!("media-1"));
    assert_eq!(result.payload["results"][0]["kind"], json!("visual"));
}

#[test]
fn codex_search_media_returns_palmier_flat_search_payload() {
    let mut project = sample_project();
    project.media[0].name = Some("Hero take".to_string());
    project.media.push(MediaAsset {
        id: "media-2".to_string(),
        name: Some("Alt take".to_string()),
        relative_path: "media/alt.mp4".to_string(),
        kind: MediaKind::Video,
        duration_seconds: 8.0,
        width: Some(1920),
        height: Some(1080),
        fps: Some(24.0),
        folder_id: None,
    });
    project.transcripts.push(Transcript {
        id: "transcript-1".to_string(),
        media_id: "media-1".to_string(),
        engine: Some("fixture".to_string()),
        raw_artifact_path: None,
        repairs: Vec::new(),
        segments: Vec::new(),
        words: vec![
            TranscriptWord {
                text: "launch".to_string(),
                start_seconds: 0.2,
                end_seconds: 0.5,
                confidence: Some(0.99),
                speaker: None,
            },
            TranscriptWord {
                text: "day".to_string(),
                start_seconds: 0.52,
                end_seconds: 0.8,
                confidence: Some(0.98),
                speaker: None,
            },
        ],
    });
    project.transcripts.push(Transcript {
        id: "transcript-2".to_string(),
        media_id: "media-2".to_string(),
        engine: Some("fixture".to_string()),
        raw_artifact_path: None,
        repairs: Vec::new(),
        segments: Vec::new(),
        words: vec![TranscriptWord {
            text: "launch".to_string(),
            start_seconds: 1.0,
            end_seconds: 1.4,
            confidence: Some(0.97),
            speaker: None,
        }],
    });

    let result = call_codex_local_tool(
        &project,
        "video_creater.search_media",
        json!({
            "query": "launch day",
            "scope": "both",
            "mediaRef": "media-1",
            "limit": 5
        }),
    )
    .expect("Palmier search media payload");

    assert_eq!(result.payload["status"], json!("modelNotInstalled"));
    assert_eq!(result.payload["indexableAssets"], json!(1));
    assert_eq!(result.payload["indexedAssets"], json!(0));
    assert_eq!(result.payload["moments"], json!([]));
    assert_eq!(
        result.payload["spoken"],
        json!([
            {
                "mediaRef": "media-1",
                "name": "Hero take",
                "startSeconds": 0.2,
                "endSeconds": 0.8,
                "text": "launch day"
            }
        ])
    );
}

#[test]
fn codex_search_media_matches_spoken_token_overlap_without_exact_phrase() {
    let mut project = sample_project();
    project.transcripts.push(Transcript {
        id: "transcript-1".to_string(),
        media_id: "media-1".to_string(),
        engine: Some("fixture".to_string()),
        raw_artifact_path: None,
        repairs: Vec::new(),
        segments: Vec::new(),
        words: vec![
            TranscriptWord {
                text: "founder".to_string(),
                start_seconds: 0.2,
                end_seconds: 0.5,
                confidence: Some(0.99),
                speaker: None,
            },
            TranscriptWord {
                text: "says".to_string(),
                start_seconds: 0.52,
                end_seconds: 0.7,
                confidence: Some(0.98),
                speaker: None,
            },
            TranscriptWord {
                text: "launch".to_string(),
                start_seconds: 0.8,
                end_seconds: 1.1,
                confidence: Some(0.98),
                speaker: None,
            },
            TranscriptWord {
                text: "day".to_string(),
                start_seconds: 1.12,
                end_seconds: 1.4,
                confidence: Some(0.98),
                speaker: None,
            },
        ],
    });

    let result = call_codex_local_tool(
        &project,
        "video_creater.search_media",
        json!({ "query": "founder launch", "limit": 5, "scope": "spoken" }),
    )
    .expect("search media payload");

    assert_eq!(
        result.payload["groups"]["spoken"][0]["mediaId"],
        json!("media-1")
    );
    assert_eq!(
        result.payload["groups"]["spoken"][0]["startSeconds"],
        json!(0.2)
    );
    assert_eq!(
        result.payload["groups"]["spoken"][0]["endSeconds"],
        json!(1.1)
    );
    assert_eq!(
        result.payload["groups"]["spoken"][0]["reason"],
        json!("matched transcript token overlap")
    );
    assert!(
        result.payload["groups"]["spoken"][0]["score"]
            .as_f64()
            .expect("score")
            > 0.7
    );
}

#[test]
fn codex_inspect_media_alias_returns_asset_and_transcript_summary() {
    let mut project = sample_project();
    project.transcripts.push(Transcript {
        id: "transcript-1".to_string(),
        media_id: "media-1".to_string(),
        engine: Some("fixture".to_string()),
        raw_artifact_path: None,
        repairs: Vec::new(),
        segments: Vec::new(),
        words: vec![TranscriptWord {
            text: "hello".to_string(),
            start_seconds: 0.1,
            end_seconds: 0.3,
            confidence: Some(0.9),
            speaker: None,
        }],
    });

    let result = call_codex_local_tool(
        &project,
        "video_creater.inspect_media",
        json!({ "mediaRef": "media-1" }),
    )
    .expect("inspect media payload");

    assert_eq!(result.payload["media"]["id"], json!("media-1"));
    assert_eq!(result.payload["transcript"]["wordCount"], json!(1));
}

#[test]
fn codex_inspect_media_accepts_palmier_clip_frame_window() {
    let mut project = sample_project();
    project.transcripts.push(Transcript {
        id: "transcript-1".to_string(),
        media_id: "media-1".to_string(),
        engine: Some("fixture".to_string()),
        raw_artifact_path: None,
        repairs: Vec::new(),
        segments: Vec::new(),
        words: vec![TranscriptWord {
            text: "hello".to_string(),
            start_seconds: 0.5,
            end_seconds: 1.0,
            confidence: Some(0.9),
            speaker: None,
        }],
    });

    let result = call_codex_local_tool(
        &project,
        "inspect_media",
        json!({
            "mediaRef": "media-1",
            "clipId": "item-1",
            "startFrame": 0,
            "endFrame": 24,
            "wordTimestamps": true,
            "language": "en"
        }),
    )
    .expect("Palmier clip-scoped inspect_media args should validate");

    assert_eq!(result.payload["media"]["id"], json!("media-1"));
    assert_eq!(result.payload["clip"]["clipId"], json!("item-1"));
    assert_eq!(
        result.payload["transcript"]["timebase"],
        json!("projectFrames")
    );
    assert_eq!(result.payload["transcript"]["language"], json!("en"));
    assert_eq!(
        result.payload["transcript"]["words"][0]["startFrame"],
        json!(12)
    );
    assert_eq!(
        result.payload["transcript"]["words"][0]["endFrame"],
        json!(24)
    );
}

#[test]
fn codex_inspect_media_maps_speed_retimed_clip_transcript_frames() {
    let mut project = sample_project();
    project.render_settings.fps = 24.0;
    project.timeline.tracks[0].items[0].start_seconds = 5.0;
    project.timeline.tracks[0].items[0].duration_seconds = 2.0;
    project.timeline.tracks[0].items[0]
        .properties
        .insert("sourceIn".to_string(), json!(10.0));
    project.timeline.tracks[0].items[0]
        .properties
        .insert("speed".to_string(), json!(2.0));
    project.transcripts.push(Transcript {
        id: "transcript-1".to_string(),
        media_id: "media-1".to_string(),
        engine: Some("fixture".to_string()),
        raw_artifact_path: None,
        repairs: Vec::new(),
        segments: Vec::new(),
        words: vec![TranscriptWord {
            text: "tail".to_string(),
            start_seconds: 13.5,
            end_seconds: 13.9,
            confidence: Some(0.9),
            speaker: None,
        }],
    });

    let result = call_codex_local_tool(
        &project,
        "inspect_media",
        json!({
            "mediaRef": "media-1",
            "clipId": "item-1",
            "startFrame": 120,
            "endFrame": 168,
            "wordTimestamps": true
        }),
    )
    .expect("Palmier retimed clip-scoped inspect_media args should validate");

    let words = result.payload["transcript"]["words"]
        .as_array()
        .expect("clip-scoped transcript words");
    assert_eq!(words.len(), 1);
    assert_eq!(words[0]["text"], json!("tail"));
    assert_eq!(words[0]["startFrame"], json!(162));
    assert_eq!(words[0]["endFrame"], json!(167));
    assert_eq!(words[0]["timelineStartSeconds"], json!(6.75));
    assert_eq!(words[0]["timelineEndSeconds"], json!(6.95));
}

#[test]
fn codex_inspect_media_returns_lottie_timeline_sampling_metadata() {
    let mut project = sample_project();
    project.media.push(MediaAsset {
        id: "media-lottie-1".to_string(),
        name: Some("Animated Badge".to_string()),
        relative_path: "media/animated-badge.json".to_string(),
        kind: MediaKind::Lottie,
        duration_seconds: 2.0,
        width: Some(1280),
        height: Some(720),
        fps: Some(30.0),
        folder_id: None,
    });

    let result = call_codex_local_tool(
        &project,
        "inspect_media",
        json!({
            "mediaRef": "media-lottie-1",
            "maxFrames": 5
        }),
    )
    .expect("Palmier Lottie inspect_media args should validate");

    assert_eq!(result.payload["media"]["kind"], json!("lottie"));
    assert_eq!(result.payload["visual"]["status"], json!("metadataOnly"));
    assert_eq!(result.payload["visual"]["kind"], json!("lottie"));
    assert_eq!(result.payload["visual"]["framerate"], json!(30.0));
    assert_eq!(result.payload["visual"]["frameCount"], json!(60));
    assert_eq!(result.payload["visual"]["durationSeconds"], json!(2.0));
    assert_eq!(
        result.payload["visual"]["sampledFrameIndices"],
        json!([0, 15, 30, 44, 59])
    );
    assert_eq!(
        result.payload["visual"]["frames"][2]["frameIndex"],
        json!(30)
    );
    assert_eq!(
        result.payload["visual"]["frames"][2]["timestampSeconds"],
        json!(1.0)
    );
}

#[test]
fn codex_direct_generation_tools_build_start_requests_without_credentials() {
    let project = sample_project();

    let models = call_codex_local_tool(&project, "video_creater.list_models", json!({}))
        .expect("models payload");
    assert!(models.payload["generationModels"]
        .as_array()
        .expect("generation models")
        .iter()
        .any(|model| model["provider"] == json!(FAL_PROVIDER)
            && model["id"] == json!(FAL_KREA_2_TURBO_MODEL_ID)
            && model["kind"] == json!("image")
            && model["supportsImageReference"] == json!(false)
            && model["aspectRatios"] == json!(["1:1", "16:9", "9:16", "4:3", "3:4"])));
    assert!(models.payload["generationModels"]
        .as_array()
        .expect("generation models")
        .iter()
        .any(|model| model["provider"] == json!(FAL_PROVIDER)
            && model["id"] == json!(FAL_RECRAFT_V3_TEXT_TO_IMAGE_MODEL_ID)
            && model["kind"] == json!("image")
            && model["supportsImageReference"] == json!(false)
            && model["aspectRatios"] == json!(["1:1", "16:9", "9:16", "4:3", "3:4"])
            && model["resolutions"]
                == json!(["1024x1024", "1280x720", "720x1280", "1280x960", "960x1280"])));
    assert!(models.payload["generationModels"]
        .as_array()
        .expect("generation models")
        .iter()
        .any(|model| model["provider"] == json!(OPENAI_PROVIDER)
            && model["id"] == json!(OPENAI_GPT_IMAGE_2_MODEL_ID)
            && model["kind"] == json!("image")
            && model["supportsImageReference"] == json!(false)
            && model["supportsFlexibleResolution"] == json!(true)
            && model["resolutionFormat"] == json!("WIDTHxHEIGHT")
            && model["maxResolution"] == json!("3840x2160")));
    assert!(models.payload["generationModels"]
        .as_array()
        .expect("generation models")
        .iter()
        .any(|model| model["provider"] == json!(OPENAI_PROVIDER)
            && model["id"] == json!(OPENAI_GPT_IMAGE_EDIT_MODEL_ID)
            && model["kind"] == json!("image")
            && model["supportsImageReference"] == json!(true)
            && model["requiresProviderInputUrl"] == json!(true)));
    assert!(models.payload["generationModels"]
        .as_array()
        .expect("generation models")
        .iter()
        .any(|model| model["provider"] == json!(XAI_PROVIDER)
            && model["id"] == json!(XAI_GROK_IMAGE_QUALITY_MODEL_ID)
            && model["kind"] == json!("image")
            && model["supportsImageReference"] == json!(true)
            && model["requiresProviderInputUrl"] == json!(true)));
    assert!(models.payload["generationModels"]
        .as_array()
        .expect("generation models")
        .iter()
        .any(|model| model["provider"] == json!(GOOGLE_PROVIDER)
            && model["id"] == json!(GOOGLE_VEO_31_FAST_MODEL_ID)
            && model["kind"] == json!("video")
            && model["durations"] == json!([8])
            && model["aspectRatios"] == json!(["16:9", "9:16"])
            && model["resolutions"] == json!(["720p", "1080p", "4k"])));
    assert!(models.payload["generationModels"]
        .as_array()
        .expect("generation models")
        .iter()
        .any(|model| model["id"] == json!("fal-ai/wan-25-preview/text-to-video")));
    assert!(models.payload["generationModels"]
        .as_array()
        .expect("generation models")
        .iter()
        .any(
            |model| model["id"] == json!("fal-ai/wan/v2.7/image-to-video")
                && model["kind"] == json!("video")
                && model["requiresProviderInputUrl"] == json!(true)
        ));
    assert!(models.payload["generationModels"]
        .as_array()
        .expect("generation models")
        .iter()
        .any(
            |model| model["id"] == json!("fal-ai/kling-video/v3/pro/image-to-video")
                && model["kind"] == json!("video")
                && model["requiresProviderInputUrl"] == json!(true)
        ));
    assert!(models.payload["generationModels"]
        .as_array()
        .expect("generation models")
        .iter()
        .any(|model| model["provider"] == json!("replicate")
            && model["id"] == json!("bytedance/seedance-2.0")
            && model["kind"] == json!("video")
            && model["requiresProviderInputUrl"] == json!(true)));
    assert!(models.payload["generationModels"]
        .as_array()
        .expect("generation models")
        .iter()
        .any(|model| model["provider"] == json!(REPLICATE_PROVIDER)
            && model["id"] == json!(REPLICATE_SEEDANCE_20_FAST_MODEL_ID)
            && model["kind"] == json!("video")
            && model["requiresProviderInputUrl"] == json!(true)));
    assert!(models.payload["generationModels"]
        .as_array()
        .expect("generation models")
        .iter()
        .any(|model| model["kind"] == json!("audio")
            && model["provider"] == json!("fal.ai")
            && model["id"] == json!("bytedance/seed-audio-1.0")));
    assert!(models.payload["generationModels"]
        .as_array()
        .expect("generation models")
        .iter()
        .any(|model| model["kind"] == json!("audio")
            && model["provider"] == json!("fal.ai")
            && model["id"] == json!("sonilo/v1.1/video-to-music")
            && model["requiresProviderInputUrl"] == json!(true)));
    assert!(models.payload["generationModels"]
        .as_array()
        .expect("generation models")
        .iter()
        .any(|model| model["kind"] == json!("audio")
            && model["provider"] == json!("fal.ai")
            && model["id"] == json!("mirelo-ai/sfx-v1.5/video-to-audio")
            && model["requiresProviderInputUrl"] == json!(true)));
    assert!(models.payload["generationModels"]
        .as_array()
        .expect("generation models")
        .iter()
        .any(|model| model["kind"] == json!("audio")
            && model["provider"] == json!("openai")
            && model["id"] == json!("gpt-4o-mini-tts")
            && model["category"] == json!("tts")
            && model["voicesSample"] == json!(["alloy", "ash", "ballad"])
            && model["voiceCount"] == json!(13)));
    assert!(models.payload["generationModels"]
        .as_array()
        .expect("generation models")
        .iter()
        .any(|model| model["kind"] == json!("audio")
            && model["provider"] == json!(ELEVENLABS_PROVIDER)
            && model["id"] == json!(ELEVENLABS_TTS_V3_MODEL_ID)
            && model["category"] == json!("tts")
            && model["defaultVoice"] == json!(ELEVENLABS_DEFAULT_VOICE)
            && model["voicesSample"] == json!([ELEVENLABS_DEFAULT_VOICE])
            && model["voiceCount"] == json!(1)));
    assert!(models.payload["generationModels"]
        .as_array()
        .expect("generation models")
        .iter()
        .any(|model| model["kind"] == json!("audio")
            && model["provider"] == json!(ELEVENLABS_PROVIDER)
            && model["id"] == json!(ELEVENLABS_MUSIC_MODEL_ID)
            && model["category"] == json!("music")
            && model["inputs"] == json!(["text"])
            && model["supportsInstrumental"] == json!(true)
            && model["supportsStyleInstructions"] == json!(true)
            && model["durations"] == Value::Null
            && model["minSeconds"] == json!(3)
            && model["maxSeconds"] == json!(600)));
    assert!(models.payload["generationModels"]
        .as_array()
        .expect("generation models")
        .iter()
        .any(|model| model["kind"] == json!("audio")
            && model["provider"] == json!(MINIMAX_PROVIDER)
            && model["id"] == json!(MINIMAX_MUSIC_MODEL_ID)
            && model["category"] == json!("music")
            && model["inputs"] == json!(["text"])
            && model["minPromptLength"] == json!(10)
            && model["supportsLyrics"] == json!(true)
            && model["supportsInstrumental"] == json!(true)
            && model["supportsStyleInstructions"] == json!(true)));
    assert!(models.payload["generationModels"]
        .as_array()
        .expect("generation models")
        .iter()
        .any(|model| model["kind"] == json!("audio")
            && model["provider"] == json!(GOOGLE_PROVIDER)
            && model["id"] == json!(GOOGLE_LYRIA_3_PRO_MODEL_ID)
            && model["category"] == json!("music")
            && model["inputs"] == json!(["text"])
            && model["supportsLyrics"] == json!(true)
            && model["supportsInstrumental"] == json!(true)
            && model["supportsStyleInstructions"] == json!(true)));
    assert!(models.payload["generationModels"]
        .as_array()
        .expect("generation models")
        .iter()
        .any(
            |model| model["id"] == json!(FAL_NANO_BANANA_PRO_EDIT_MODEL_ID)
                && model["aliases"] == json!(["nano-banana-pro"])
        ));
    assert!(models.payload["generationModels"]
        .as_array()
        .expect("generation models")
        .iter()
        .any(
            |model| model["id"] == json!(FAL_SONILO_TEXT_TO_MUSIC_MODEL_ID)
                && model["category"] == json!("music")
                && model["inputs"] == json!(["text"])
                && model["durations"] == Value::Null
                && model["minSeconds"] == json!(1)
                && model["maxSeconds"] == json!(600)
        ));
    assert!(models.payload["generationModels"]
        .as_array()
        .expect("generation models")
        .iter()
        .any(
            |model| model["id"] == json!(FAL_SONILO_VIDEO_TO_MUSIC_MODEL_ID)
                && model["aliases"] == json!(["sonilo-v1.1-video-to-music"])
        ));
    assert!(models.payload["generationModels"]
        .as_array()
        .expect("generation models")
        .iter()
        .any(
            |model| model["id"] == json!(FAL_MIRELO_VIDEO_TO_AUDIO_MODEL_ID)
                && model["aliases"] == json!(["mirelo-sfx-v1.5-video-to-audio"])
        ));
    assert!(models.payload["transcriptionModels"]
        .as_array()
        .expect("transcription models")
        .iter()
        .any(|model| model["id"] == json!("nvidia/parakeet-tdt-0.6b-v3")));

    let result = call_codex_local_tool(
        &project,
        "video_creater.generate_video",
        json!({
            "projectDir": "/tmp/video-creater-project",
            "assetId": "agent-video-1",
            "jobId": "job-agent-video-1",
            "mockMode": true,
            "brief": {
                "prompt": "slow push-in on the product",
                "model": { "provider": "fal.ai", "id": "fal-ai/wan-25-preview/text-to-video" }
            }
        }),
    )
    .expect("generate video start request");

    assert!(!result.mutates_project);
    assert_eq!(
        result.payload["startRequest"]["input"]["assetId"],
        json!("agent-video-1")
    );
    assert_eq!(
        result.payload["startRequest"]["input"]["model"]["id"],
        json!("fal-ai/wan-25-preview/text-to-video")
    );
    assert_eq!(result.payload["projectActionCount"], json!(2));
    assert_eq!(
        result.payload["projectActions"][0]["type"],
        json!("recordGeneratedAsset")
    );
    assert_eq!(
        result.payload["projectActions"][0]["asset"]["id"],
        json!("agent-video-1")
    );
    assert_eq!(
        result.payload["projectActions"][0]["asset"]["status"],
        json!("queued")
    );
    assert_eq!(
        result.payload["projectActions"][1]["type"],
        json!("recordJob")
    );
    assert_eq!(
        result.payload["projectActions"][1]["job"]["startRequest"]["workflowType"],
        json!("VideoCreaterGenerateMediaWorkflow")
    );
}

#[test]
fn codex_direct_generation_persists_placeholder_for_real_split_project() {
    let project = sample_project();
    let project_dir = tempfile::tempdir().expect("generation placeholder project dir");
    save_split_project(project_dir.path(), &project).expect("save generation placeholder project");

    let result = call_codex_local_tool(
        &project,
        "video_creater.generate_image",
        json!({
            "projectDir": project_dir.path().display().to_string(),
            "assetId": "agent-image-placeholder",
            "jobId": "job-agent-image-placeholder",
            "mockMode": true,
            "prompt": "clean editorial product still",
            "model": { "provider": REPLICATE_PROVIDER, "id": REPLICATE_FLUX_SCHNELL_MODEL_ID }
        }),
    )
    .expect("direct image generation should persist placeholder actions");

    assert!(result.mutates_project);
    assert_eq!(result.payload["projectActionCount"], json!(2));
    assert_eq!(
        result.payload["projectActionApplication"]["applied"],
        json!(true)
    );
    assert_eq!(
        result.payload["persistedPlaceholderAssetId"],
        json!("agent-image-placeholder")
    );

    let persisted =
        load_split_project(project_dir.path()).expect("load generated placeholder project");
    let generated = persisted
        .generated_assets
        .iter()
        .find(|asset| asset.id == "agent-image-placeholder")
        .expect("persisted generated placeholder");
    assert_eq!(generated.status, GeneratedAssetStatus::Queued);
    assert_eq!(generated.kind, MediaKind::Generated);
    assert_eq!(generated.model.provider, REPLICATE_PROVIDER);
    assert_eq!(generated.model.id, REPLICATE_FLUX_SCHNELL_MODEL_ID);
    assert!(generated.outputs.is_empty());
    let job = persisted
        .jobs
        .iter()
        .find(|job| job.id == "job-agent-image-placeholder")
        .expect("persisted generation job");
    assert_eq!(
        job.status,
        video_creater_lib::project::model::JobStatus::Queued
    );
    assert_eq!(
        job.start_request
            .as_ref()
            .expect("job start request")
            .workflow_type,
        "VideoCreaterGenerateMediaWorkflow"
    );
}

#[test]
fn codex_generate_music_persists_placeholder_for_real_split_project() {
    let project = sample_project();
    let project_dir = tempfile::tempdir().expect("music placeholder project dir");
    save_split_project(project_dir.path(), &project).expect("save music placeholder project");

    let result = call_codex_local_tool(
        &project,
        "video_creater.generate_music",
        json!({
            "projectDir": project_dir.path().display().to_string(),
            "assetId": "agent-music-placeholder",
            "jobId": "job-agent-music-placeholder",
            "mockMode": true,
            "prompt": "bright launch montage music"
        }),
    )
    .expect("direct music generation should persist placeholder actions");

    assert!(result.mutates_project);
    assert_eq!(
        result.payload["projectActionApplication"]["applied"],
        json!(true)
    );
    assert_eq!(
        result.payload["persistedPlaceholderAssetId"],
        json!("agent-music-placeholder")
    );

    let persisted = load_split_project(project_dir.path()).expect("load music placeholder project");
    let generated = persisted
        .generated_assets
        .iter()
        .find(|asset| asset.id == "agent-music-placeholder")
        .expect("persisted music generated placeholder");
    assert_eq!(generated.status, GeneratedAssetStatus::Queued);
    assert_eq!(generated.settings.category.as_deref(), Some("music"));
    let job = persisted
        .jobs
        .iter()
        .find(|job| job.id == "job-agent-music-placeholder")
        .expect("persisted music generation job");
    assert_eq!(
        job.status,
        video_creater_lib::project::model::JobStatus::Queued
    );
}

#[test]
fn codex_list_models_accepts_upscale_filter() {
    let project = sample_project();

    let models = call_codex_local_tool(
        &project,
        "video_creater.list_models",
        json!({ "type": "upscale" }),
    )
    .expect("filtered model list");

    assert_eq!(models.payload["loaded"], json!(true));
    assert!(models.payload["models"]
        .as_array()
        .expect("flat models")
        .iter()
        .any(|model| model["type"] == json!("upscale")
            && model["id"] == json!("mock-upscale-v1")
            && model["supports"] == json!(["video", "image"])
            && model["supportedTypes"] == json!(["image", "video"])
            && model["speed"] == json!("Fast")));
    assert!(models.payload["models"]
        .as_array()
        .expect("flat models")
        .iter()
        .any(|model| model["type"] == json!("upscale")
            && model["id"] == json!("fal-ai/video-upscaler")
            && model["aliases"] == json!(["bytedance-upscaler"])
            && model["supports"] == json!(["video"])
            && model["supportedTypes"] == json!(["video"])
            && model["speed"] == json!("Slow")
            && model["requiresProviderInputUrl"] == json!(true)));
    assert!(models.payload["models"]
        .as_array()
        .expect("flat models")
        .iter()
        .any(|model| model["type"] == json!("upscale")
            && model["id"] == json!("fal-ai/aura-sr")
            && model["aliases"] == json!(["seedvr-image-upscaler"])
            && model["supports"] == json!(["image"])
            && model["supportedTypes"] == json!(["image"])
            && model["requiresProviderInputUrl"] == json!(true)));
    assert!(models.payload["generationModels"]
        .as_array()
        .expect("generation models")
        .iter()
        .any(|model| model["kind"] == json!("upscale")
            && model["id"] == json!("fal-ai/aura-sr")
            && model["allowedEndpoints"] == json!(["upscale_media"])
            && model["responseShape"] == json!("upscaledImage")
            && model["uiCapabilities"]["supportedTypes"] == json!(["image"])));
}

#[test]
fn codex_generate_video_accepts_direct_prompt_and_placement_args() {
    let mut project = sample_project();
    project.media.push(MediaAsset {
        id: "image-ref".to_string(),
        name: Some("Start frame".to_string()),
        relative_path: "media/start-frame.png".to_string(),
        kind: MediaKind::Image,
        duration_seconds: 0.0,
        width: Some(1280),
        height: Some(720),
        fps: None,
        folder_id: None,
    });

    let result = call_codex_local_tool(
        &project,
        "video_creater.generate_video",
        json!({
            "projectDir": "/tmp/video-creater-project",
            "assetId": "agent-video-2",
            "jobId": "job-agent-video-2",
            "mockMode": true,
            "name": "Launch hero",
            "prompt": "make a launch-day hero shot",
            "placementIntent": "timeline",
            "references": {
                "mediaIds": ["media-1"],
                "firstFrameMediaId": "image-ref",
                "lastFrameMediaId": null
            },
            "settings": {
                "width": 1280,
                "height": 720,
                "durationSeconds": 5,
                "fps": 24,
                "aspectRatio": "16:9"
            }
        }),
    )
    .expect("direct generate video args should validate");

    assert!(!result.mutates_project);
    assert_eq!(result.payload["status"], json!("started"));
    assert_eq!(result.payload["placeholderAssetId"], json!("agent-video-2"));
    assert_eq!(result.payload["assetId"], json!("agent-video-2"));
    assert_eq!(result.payload["jobId"], json!("job-agent-video-2"));
    assert_eq!(
        result.payload["model"],
        json!({
            "provider": FAL_PROVIDER,
            "id": FAL_WAN_IMAGE_TO_VIDEO_MODEL_ID
        })
    );
    assert_eq!(
        result.payload["startRequest"]["input"]["prompt"],
        json!("make a launch-day hero shot")
    );
    assert_eq!(
        result.payload["startRequest"]["input"]["placementIntent"],
        json!("timeline")
    );
    assert_eq!(
        result.payload["startRequest"]["input"]["references"]["mediaIds"],
        json!(["media-1"])
    );
    assert_eq!(
        result.payload["startRequest"]["input"]["settings"]["durationSeconds"],
        json!(5)
    );
    assert_eq!(
        result.payload["startRequest"]["input"]["settings"]["generateAudio"],
        json!(true)
    );
    assert_eq!(
        result.payload["projectActions"][0]["asset"]["name"],
        json!("Launch hero")
    );
    assert_eq!(
        result.payload["projectActions"][0]["asset"]["placementIntent"],
        json!("timeline")
    );
    assert_eq!(
        result.payload["projectActions"][0]["asset"]["references"]["firstFrameMediaId"],
        json!("image-ref")
    );
    assert_eq!(
        result.payload["projectActions"][0]["asset"]["settings"]["generateAudio"],
        json!(true)
    );
    assert_eq!(
        result.payload["projectActions"][0]["asset"]["settings"]["width"],
        json!(1280)
    );
}

#[test]
fn codex_generate_video_can_replace_timeline_item() {
    let project = sample_project();

    let result = call_codex_local_tool(
        &project,
        "video_creater.generate_video",
        json!({
            "projectDir": "/tmp/video-creater-project",
            "assetId": "agent-video-replacement",
            "jobId": "job-agent-video-replacement",
            "mockMode": true,
            "prompt": "replace the selected clip with a better product reveal",
            "replacementItemId": "item-1"
        }),
    )
    .expect("direct video generation should support clip replacement");

    assert_eq!(
        result.payload["startRequest"]["input"]["placementIntent"],
        json!("replace:item-1")
    );
    assert_eq!(
        result.payload["projectActions"][0]["asset"]["placementIntent"],
        json!("replace:item-1")
    );
}

#[test]
fn codex_list_models_exposes_fal_wan_video_to_video_source_video_model() {
    let project = sample_project();

    let result = call_codex_local_tool(
        &project,
        "video_creater.list_models",
        json!({ "type": "video" }),
    )
    .expect("list video generation models");

    let models = result.payload["generationModels"]
        .as_array()
        .expect("generation models array");
    let model = models
        .iter()
        .find(|model| {
            model["provider"] == "fal.ai" && model["id"] == "fal-ai/wan/v2.2-a14b/video-to-video"
        })
        .expect("fal WAN video-to-video model");
    assert_eq!(model["kind"], json!("video"));
    assert_eq!(model["requiresProviderInputUrl"], json!(true));
    assert_eq!(model["maxSourceVideoSeconds"], json!(10.0));

    let flat_models = result.payload["models"]
        .as_array()
        .expect("flat models array");
    let flat_model = flat_models
        .iter()
        .find(|model| {
            model["provider"] == "fal.ai" && model["id"] == "fal-ai/wan/v2.2-a14b/video-to-video"
        })
        .expect("flat fal WAN video-to-video model");
    assert_eq!(flat_model["supports"], json!(["video"]));
    assert_eq!(flat_model["requiresProviderInputUrl"], json!(true));
    assert_eq!(flat_model["maxSourceVideoSeconds"], json!(10.0));

    let motion_model = models
        .iter()
        .find(|model| {
            model["provider"] == FAL_PROVIDER
                && model["id"] == FAL_KLING_V3_PRO_MOTION_CONTROL_MODEL_ID
        })
        .expect("fal Kling motion-control source-video edit model");
    assert_eq!(motion_model["kind"], json!("video"));
    assert_eq!(motion_model["requiresProviderInputUrl"], json!(true));
    assert_eq!(motion_model["requiresSourceVideo"], json!(true));
    assert_eq!(motion_model["maxSourceVideoSeconds"], json!(10.0));
    assert_eq!(motion_model["supportsReferences"], json!(true));
    assert_eq!(motion_model["maxReferenceImages"], json!(1));

    let flat_motion_model = flat_models
        .iter()
        .find(|model| {
            model["provider"] == FAL_PROVIDER
                && model["id"] == FAL_KLING_V3_PRO_MOTION_CONTROL_MODEL_ID
        })
        .expect("flat fal Kling motion-control source-video edit model");
    assert_eq!(flat_motion_model["supports"], json!(["video", "image"]));
    assert_eq!(flat_motion_model["requiresProviderInputUrl"], json!(true));
    assert_eq!(flat_motion_model["requiresSourceVideo"], json!(true));
    assert_eq!(flat_motion_model["maxSourceVideoSeconds"], json!(10.0));
    assert_eq!(flat_motion_model["maxReferenceImages"], json!(1));
}

#[test]
fn codex_list_models_exposes_palmier_style_generation_capabilities() {
    let project = sample_project();

    let result = call_codex_local_tool(&project, "video_creater.list_models", json!({}))
        .expect("list models");

    let models = result.payload["models"].as_array().expect("flat models");
    let generation_models = result.payload["generationModels"]
        .as_array()
        .expect("generation models");

    for model in models.iter().chain(generation_models.iter()) {
        assert!(
            model["displayName"]
                .as_str()
                .is_some_and(|display_name| !display_name.trim().is_empty()),
            "model should expose Palmier-style displayName: {model:?}"
        );
    }

    let seedance = models
        .iter()
        .find(|model| {
            model["provider"] == json!(REPLICATE_PROVIDER)
                && model["id"] == json!(REPLICATE_SEEDANCE_20_MODEL_ID)
        })
        .expect("Replicate Seedance model");
    assert_eq!(seedance["displayName"], json!("Replicate Seedance 2.0"));
    assert_eq!(seedance["type"], json!("video"));
    assert_eq!(seedance["durations"], json!([5, 10]));
    assert_eq!(seedance["aspectRatios"], json!(["16:9", "9:16", "1:1"]));
    assert_eq!(seedance["resolutions"], json!(["720p", "1080p"]));
    assert_eq!(seedance["supportsFirstFrame"], json!(true));
    assert_eq!(seedance["supportsLastFrame"], json!(true));
    assert_eq!(seedance["supportsReferences"], json!(true));
    assert_eq!(seedance["maxReferenceImages"], json!(4));
    assert_eq!(seedance["maxReferenceVideos"], json!(3));
    assert_eq!(seedance["maxReferenceAudios"], json!(3));
    assert_eq!(seedance["maxTotalReferences"], json!(6));
    assert_eq!(seedance["maxCombinedVideoRefSeconds"], json!(15));
    assert_eq!(seedance["maxCombinedAudioRefSeconds"], json!(15));
    assert_eq!(seedance["framesAndReferencesExclusive"], json!(true));
    assert_eq!(seedance["referenceTagNoun"], json!("reference"));
    assert_eq!(
        seedance["creditsPerSecond"],
        json!({"720p": 1.0, "1080p": 1.0})
    );
    assert_eq!(seedance["audioDiscountRate"], json!({"": 0.8}));

    let fal_flux = models
        .iter()
        .find(|model| {
            model["provider"] == json!(FAL_PROVIDER)
                && model["id"] == json!(FAL_FLUX_SCHNELL_MODEL_ID)
        })
        .expect("fal Flux Schnell image model");
    assert_eq!(fal_flux["creditsPerImage"], json!({"": 1.0}));

    let wan_text = generation_models
        .iter()
        .find(|model| {
            model["provider"] == json!(FAL_PROVIDER)
                && model["id"] == json!(FAL_WAN_TEXT_TO_VIDEO_MODEL_ID)
        })
        .expect("fal WAN text-to-video model");
    assert_eq!(wan_text["displayName"], json!("fal.ai WAN Text to Video"));
    assert_eq!(wan_text["durations"], json!([5, 10]));
    assert_eq!(wan_text["aspectRatios"], json!(["16:9", "9:16", "1:1"]));
    assert_eq!(wan_text["supportsFirstFrame"], json!(false));
    assert_eq!(wan_text["supportsLastFrame"], json!(false));
    assert_eq!(wan_text["maxReferenceAudios"], json!(1));

    let wan_reference = generation_models
        .iter()
        .find(|model| {
            model["provider"] == json!(FAL_PROVIDER)
                && model["id"] == json!(FAL_WAN_REFERENCE_TO_VIDEO_MODEL_ID)
        })
        .expect("fal WAN reference-to-video model");
    assert_eq!(wan_reference["kind"], json!("video"));
    assert_eq!(
        wan_reference["durations"],
        json!([2, 3, 4, 5, 6, 7, 8, 9, 10])
    );
    assert_eq!(
        wan_reference["aspectRatios"],
        json!(["16:9", "9:16", "1:1", "4:3", "3:4"])
    );
    assert_eq!(wan_reference["resolutions"], json!(["720p", "1080p"]));
    assert_eq!(wan_reference["supportsFirstFrame"], json!(false));
    assert_eq!(wan_reference["supportsReferences"], json!(true));
    assert_eq!(wan_reference["requiresReferenceImage"], json!(true));
    assert_eq!(wan_reference["maxReferenceImages"], json!(4));
    assert_eq!(wan_reference["maxReferenceVideos"], json!(3));

    let grok_video = generation_models
        .iter()
        .find(|model| {
            model["provider"] == json!("xai") && model["id"] == json!("grok-imagine-video")
        })
        .expect("xAI Grok video model");
    assert_eq!(grok_video["kind"], json!("video"));
    assert_eq!(
        grok_video["durations"],
        json!([1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15])
    );
    assert_eq!(
        grok_video["aspectRatios"],
        json!(["16:9", "9:16", "1:1", "4:3", "3:4", "3:2", "2:3"])
    );
    assert_eq!(grok_video["resolutions"], json!(["480p", "720p"]));
    assert_eq!(grok_video["supportsFirstFrame"], json!(true));
    assert_eq!(grok_video["requiresSourceVideo"], json!(false));
    assert_eq!(grok_video["supportsSourceVideo"], json!(true));
    assert_eq!(grok_video["maxSourceVideoSeconds"], json!(8.7));
    assert_eq!(grok_video["maxReferenceImages"], json!(7));
    assert_eq!(grok_video["maxTotalReferences"], json!(7));
    assert_eq!(grok_video["framesAndReferencesExclusive"], json!(true));

    let google_veo = generation_models
        .iter()
        .find(|model| {
            model["provider"] == json!(GOOGLE_PROVIDER)
                && model["id"] == json!(GOOGLE_VEO_31_FAST_MODEL_ID)
        })
        .expect("Google Veo 3.1 Fast video model");
    assert_eq!(google_veo["displayName"], json!("Google Veo 3.1 Fast"));
    assert_eq!(google_veo["kind"], json!("video"));
    assert_eq!(google_veo["durations"], json!([8]));
    assert_eq!(google_veo["aspectRatios"], json!(["16:9", "9:16"]));
    assert_eq!(google_veo["resolutions"], json!(["720p", "1080p", "4k"]));
    assert_eq!(google_veo["supportsFirstFrame"], json!(true));
    assert_eq!(google_veo["supportsLastFrame"], json!(true));
    assert_eq!(google_veo["supportsReferences"], json!(true));
    assert_eq!(google_veo["requiresSourceVideo"], json!(false));
    assert_eq!(google_veo["maxReferenceImages"], json!(3));
    assert_eq!(google_veo["maxTotalReferences"], json!(3));

    let flat_google_veo = models
        .iter()
        .find(|model| {
            model["provider"] == json!(GOOGLE_PROVIDER)
                && model["id"] == json!(GOOGLE_VEO_31_FAST_MODEL_ID)
        })
        .expect("flat Google Veo 3.1 Fast video model");
    assert_eq!(flat_google_veo["supports"], json!(["video", "image"]));
    assert_eq!(flat_google_veo["supportsFirstFrame"], json!(true));
    assert_eq!(flat_google_veo["supportsLastFrame"], json!(true));
    assert_eq!(flat_google_veo["supportsReferences"], json!(true));
    assert_eq!(flat_google_veo["maxReferenceImages"], json!(3));
    assert_eq!(flat_google_veo["maxTotalReferences"], json!(3));
    assert_eq!(flat_google_veo["referenceTagNoun"], json!("image"));

    let openai_tts = models
        .iter()
        .find(|model| {
            model["provider"] == json!(OPENAI_PROVIDER)
                && model["id"] == json!(OPENAI_GPT_4O_MINI_TTS_MODEL_ID)
        })
        .expect("OpenAI TTS model");
    assert_eq!(openai_tts["displayName"], json!("OpenAI GPT-4o Mini TTS"));
    assert_eq!(openai_tts["category"], json!("tts"));
    assert_eq!(openai_tts["inputs"], json!(["text"]));
    assert_eq!(openai_tts["promptLabel"], json!("Text to speak"));
    assert_eq!(openai_tts["defaultVoice"], json!("alloy"));
    assert_eq!(openai_tts["minPromptLength"], json!(1));
    assert_eq!(openai_tts["supportsLyrics"], json!(false));
    assert_eq!(openai_tts["supportsInstrumental"], json!(false));
    assert_eq!(openai_tts["supportsStyleInstructions"], json!(true));
    assert_eq!(
        openai_tts["audioPricing"],
        json!({"mode": "perThousandChars", "rate": 15.0})
    );

    let google_tts = models
        .iter()
        .find(|model| {
            model["provider"] == json!(GOOGLE_PROVIDER)
                && model["id"] == json!(GOOGLE_GEMINI_TTS_MODEL_ID)
        })
        .expect("Google Gemini TTS model");
    assert_eq!(google_tts["category"], json!("tts"));
    assert_eq!(google_tts["aliases"], json!(["gemini-3.1-flash-tts"]));
    assert_eq!(google_tts["inputs"], json!(["text"]));
    assert_eq!(google_tts["defaultVoice"], json!("Kore"));
    assert_eq!(google_tts["minPromptLength"], json!(1));
    assert_eq!(google_tts["supportsLyrics"], json!(false));
    assert_eq!(google_tts["supportsInstrumental"], json!(false));
    assert_eq!(google_tts["supportsStyleInstructions"], json!(true));
    assert_eq!(
        google_tts["voicesSample"],
        json!(["Kore", "Puck", "Charon"])
    );
    assert_eq!(google_tts["voiceCount"], json!(30));

    let google_lyria = models
        .iter()
        .find(|model| {
            model["provider"] == json!(GOOGLE_PROVIDER)
                && model["id"] == json!(GOOGLE_LYRIA_3_PRO_MODEL_ID)
        })
        .expect("Google Lyria music model");
    assert_eq!(google_lyria["category"], json!("music"));
    assert_eq!(google_lyria["inputs"], json!(["text"]));
    assert_eq!(
        google_lyria["promptLabel"],
        json!("Describe the music style or mood")
    );
    assert_eq!(google_lyria["durations"], Value::Null);

    let minimax_music = models
        .iter()
        .find(|model| {
            model["provider"] == json!(MINIMAX_PROVIDER)
                && model["id"] == json!(MINIMAX_MUSIC_MODEL_ID)
        })
        .expect("MiniMax Music model");
    assert_eq!(minimax_music["category"], json!("music"));
    assert_eq!(minimax_music["inputs"], json!(["text"]));
    assert_eq!(minimax_music["minPromptLength"], json!(10));
    assert_eq!(minimax_music["supportsLyrics"], json!(true));
    assert_eq!(minimax_music["supportsInstrumental"], json!(true));
    assert_eq!(minimax_music["supportsStyleInstructions"], json!(true));
    assert_eq!(minimax_music["durations"], Value::Null);

    let elevenlabs_music = models
        .iter()
        .find(|model| {
            model["provider"] == json!(ELEVENLABS_PROVIDER)
                && model["id"] == json!(ELEVENLABS_MUSIC_MODEL_ID)
        })
        .expect("ElevenLabs Music model");
    assert_eq!(elevenlabs_music["displayName"], json!("ElevenLabs Music"));
    assert_eq!(elevenlabs_music["category"], json!("music"));
    assert_eq!(elevenlabs_music["inputs"], json!(["text"]));
    assert_eq!(
        elevenlabs_music["promptLabel"],
        json!("Describe the music style or mood")
    );
    assert_eq!(elevenlabs_music["supportsLyrics"], json!(true));
    assert_eq!(elevenlabs_music["durations"], Value::Null);
    assert_eq!(elevenlabs_music["minSeconds"], json!(3));
    assert_eq!(elevenlabs_music["maxSeconds"], json!(600));
}

#[test]
fn codex_list_models_keeps_duplicate_generation_catalog_metadata_consistent() {
    let project = sample_project();

    let result = call_codex_local_tool(&project, "video_creater.list_models", json!({}))
        .expect("list models");

    let models = result.payload["models"].as_array().expect("flat models");
    let generation_models = result.payload["generationModels"]
        .as_array()
        .expect("generation models");
    let flat_by_key = models
        .iter()
        .filter_map(|model| {
            let provider = model["provider"].as_str()?;
            let id = model["id"].as_str()?;
            Some((format!("{provider}:{id}"), model))
        })
        .collect::<BTreeMap<_, _>>();

    for generation_model in generation_models {
        let provider = generation_model["provider"]
            .as_str()
            .expect("generation model provider");
        let id = generation_model["id"]
            .as_str()
            .expect("generation model id");
        let key = format!("{provider}:{id}");
        let flat_model = flat_by_key
            .get(&key)
            .unwrap_or_else(|| panic!("flat catalog should include {key}"));
        let generation_object = generation_model
            .as_object()
            .expect("generation model object");
        let flat_object = flat_model.as_object().expect("flat model object");
        let keys = generation_object
            .keys()
            .chain(flat_object.keys())
            .filter(|key| key.as_str() != "type" && key.as_str() != "supports")
            .cloned()
            .collect::<BTreeSet<_>>();

        for field in keys {
            assert_eq!(
                generation_model.get(&field),
                flat_model.get(&field),
                "{key} catalog field {field} should match between generationModels and models"
            );
        }
    }
}

#[test]
fn codex_list_models_exposes_palmier_endpoint_and_response_shape_metadata() {
    let project = sample_project();

    let result = call_codex_local_tool(&project, "video_creater.list_models", json!({}))
        .expect("list models");

    let models = result.payload["models"].as_array().expect("flat models");
    let generation_models = result.payload["generationModels"]
        .as_array()
        .expect("generation models");
    let generation_kinds = BTreeSet::from(["audio", "image", "upscale", "video"]);
    let response_shapes = BTreeSet::from(["audio", "images", "upscaledImage", "video"]);

    for model in models.iter().chain(generation_models.iter()) {
        let Some(kind) = model["kind"].as_str() else {
            continue;
        };
        if !generation_kinds.contains(kind) {
            continue;
        }
        let endpoints = model["allowedEndpoints"]
            .as_array()
            .unwrap_or_else(|| panic!("model should expose allowedEndpoints: {model:?}"));
        assert!(
            endpoints.iter().any(|endpoint| endpoint
                .as_str()
                .is_some_and(|value| !value.trim().is_empty())),
            "model should expose at least one allowed endpoint: {model:?}"
        );
        let response_shape = model["responseShape"]
            .as_str()
            .unwrap_or_else(|| panic!("model should expose responseShape: {model:?}"));
        assert!(
            response_shapes.contains(response_shape),
            "model should expose a Palmier responseShape, got {response_shape}: {model:?}"
        );
        assert!(
            model["paidOnly"].as_bool().is_some(),
            "model should expose Palmier paidOnly metadata: {model:?}"
        );
    }

    let find = |provider: &str, id: &str| {
        models
            .iter()
            .find(|model| model["provider"] == json!(provider) && model["id"] == json!(id))
            .unwrap_or_else(|| panic!("flat catalog should include {provider}:{id}"))
    };

    let seedance = find(REPLICATE_PROVIDER, REPLICATE_SEEDANCE_20_MODEL_ID);
    assert_eq!(seedance["allowedEndpoints"], json!(["generate_video"]));
    assert_eq!(seedance["responseShape"], json!("video"));
    assert_eq!(seedance["paidOnly"], json!(false));

    let fal_flux = find(FAL_PROVIDER, FAL_FLUX_SCHNELL_MODEL_ID);
    assert_eq!(fal_flux["allowedEndpoints"], json!(["generate_image"]));
    assert_eq!(fal_flux["responseShape"], json!("images"));

    let sonilo_music = find(FAL_PROVIDER, FAL_SONILO_TEXT_TO_MUSIC_MODEL_ID);
    assert_eq!(
        sonilo_music["allowedEndpoints"],
        json!(["generate_audio", "generate_music"])
    );
    assert_eq!(sonilo_music["responseShape"], json!("audio"));

    let aura_sr = find(FAL_PROVIDER, FAL_AURA_SR_MODEL_ID);
    assert_eq!(aura_sr["allowedEndpoints"], json!(["upscale_media"]));
    assert_eq!(aura_sr["responseShape"], json!("upscaledImage"));
}

#[test]
fn codex_list_models_exposes_palmier_ui_capabilities_shape() {
    let project = sample_project();

    let result = call_codex_local_tool(&project, "video_creater.list_models", json!({}))
        .expect("list models");

    let models = result.payload["models"].as_array().expect("flat models");
    let find = |provider: &str, id: &str| {
        models
            .iter()
            .find(|model| model["provider"] == json!(provider) && model["id"] == json!(id))
            .unwrap_or_else(|| panic!("flat catalog should include {provider}:{id}"))
    };

    let seedance = find(REPLICATE_PROVIDER, REPLICATE_SEEDANCE_20_MODEL_ID);
    assert_eq!(
        seedance["uiCapabilities"],
        json!({
            "durations": [5, 10],
            "resolutions": ["720p", "1080p"],
            "aspectRatios": ["16:9", "9:16", "1:1"],
            "supportsFirstFrame": true,
            "supportsLastFrame": true,
            "maxReferenceImages": 4,
            "maxReferenceVideos": 3,
            "maxReferenceAudios": 3,
            "maxTotalReferences": 6,
            "maxCombinedVideoRefSeconds": 15,
            "maxCombinedAudioRefSeconds": 15,
            "framesAndReferencesExclusive": true,
            "referenceTagNoun": "reference",
            "requiresSourceVideo": false,
            "requiresReferenceImage": false
        })
    );

    let fal_flux = find(FAL_PROVIDER, FAL_FLUX_SCHNELL_MODEL_ID);
    assert_eq!(
        fal_flux["uiCapabilities"],
        json!({
            "resolutions": Value::Null,
            "aspectRatios": ["1:1", "16:9", "9:16", "4:3", "3:4"],
            "qualities": Value::Null,
            "supportsImageReference": false,
            "maxImages": 4
        })
    );

    let openai_tts = find(OPENAI_PROVIDER, OPENAI_GPT_4O_MINI_TTS_MODEL_ID);
    assert_eq!(
        openai_tts["uiCapabilities"],
        json!({
            "category": "tts",
            "voices": ["alloy", "ash", "ballad", "cedar", "coral", "echo", "fable", "marin", "nova", "onyx", "sage", "shimmer", "verse"],
            "defaultVoice": "alloy",
            "supportsLyrics": false,
            "supportsInstrumental": false,
            "supportsStyleInstructions": true,
            "durations": Value::Null,
            "minPromptLength": 1,
            "inputs": ["text"],
            "promptLabel": "Text to speak",
            "minSeconds": 1,
            "maxSeconds": 900
        })
    );

    let aura_sr = find(FAL_PROVIDER, FAL_AURA_SR_MODEL_ID);
    assert_eq!(
        aura_sr["uiCapabilities"],
        json!({
            "speed": "Medium",
            "p75DurationSeconds": 10,
            "supportedTypes": ["image"]
        })
    );
}

#[test]
fn codex_generate_video_accepts_palmier_folder_id_alias() {
    let mut project = sample_project();
    project.media_folders.push(MediaFolder {
        id: "agent-folder".to_string(),
        name: "Agent selects".to_string(),
        parent_id: None,
    });

    let result = call_codex_local_tool(
        &project,
        "video_creater.generate_video",
        json!({
            "projectDir": "/tmp/video-creater-project",
            "assetId": "agent-video-folder",
            "jobId": "job-agent-video-folder",
            "mockMode": true,
            "name": "Foldered hero",
            "prompt": "make a launch-day hero shot",
            "folderId": "agent-folder"
        }),
    )
    .expect("Palmier folderId should alias targetFolderId");

    assert!(!result.mutates_project);
    assert_eq!(
        result.payload["startRequest"]["input"]["targetFolderId"],
        json!("agent-folder")
    );
    assert_eq!(
        result.payload["projectActions"][0]["asset"]["targetFolderId"],
        json!("agent-folder")
    );
}

#[test]
fn codex_direct_generation_defaults_target_folder_from_references() {
    let mut project = sample_project();
    project.media_folders.push(MediaFolder {
        id: "source-folder".to_string(),
        name: "Source Footage".to_string(),
        parent_id: None,
    });
    project.media_folders.push(MediaFolder {
        id: "reference-folder".to_string(),
        name: "References".to_string(),
        parent_id: None,
    });
    project.media[0].folder_id = Some("source-folder".to_string());
    project.media[0].duration_seconds = 10.0;
    project.media.push(MediaAsset {
        id: "image-ref".to_string(),
        name: Some("Style reference".to_string()),
        relative_path: "media/style-reference.png".to_string(),
        kind: MediaKind::Image,
        duration_seconds: 0.0,
        width: Some(1024),
        height: Some(1024),
        fps: None,
        folder_id: Some("reference-folder".to_string()),
    });

    let source_video = call_codex_local_tool(
        &project,
        "video_creater.generate_video",
        json!({
            "projectDir": "/tmp/video-creater-project",
            "assetId": "agent-source-video-folder",
            "jobId": "job-agent-source-video-folder",
            "mockMode": true,
            "prompt": "restyle the source video",
            "sourceVideoMediaRef": "media-1",
            "referenceImageMediaRefs": ["image-ref"]
        }),
    )
    .expect("source-video generation should inherit source folder");
    assert_eq!(
        source_video.payload["startRequest"]["input"]["targetFolderId"],
        json!("source-folder")
    );
    assert_eq!(
        source_video.payload["projectActions"][0]["asset"]["targetFolderId"],
        json!("source-folder")
    );

    let image = call_codex_local_tool(
        &project,
        "video_creater.generate_image",
        json!({
            "projectDir": "/tmp/video-creater-project",
            "assetId": "agent-image-reference-folder",
            "jobId": "job-agent-image-reference-folder",
            "mockMode": true,
            "prompt": "make a product still in this style",
            "model": { "provider": OPENAI_PROVIDER, "id": OPENAI_GPT_IMAGE_EDIT_MODEL_ID },
            "referenceMediaRefs": ["image-ref"]
        }),
    )
    .expect("image generation should inherit reference folder");
    assert_eq!(
        image.payload["startRequest"]["input"]["targetFolderId"],
        json!("reference-folder")
    );
    assert_eq!(
        image.payload["projectActions"][0]["asset"]["targetFolderId"],
        json!("reference-folder")
    );
}

#[test]
fn codex_generate_audio_does_not_inherit_source_media_folder_without_folder_id() {
    let mut project = sample_project();
    project.media_folders.push(MediaFolder {
        id: "source-folder".to_string(),
        name: "Source Footage".to_string(),
        parent_id: None,
    });
    project.media[0].folder_id = Some("source-folder".to_string());
    project.media[0].duration_seconds = 10.0;

    let result = call_codex_local_tool(
        &project,
        "video_creater.generate_audio",
        json!({
            "projectDir": "/tmp/video-creater-project",
            "assetId": "agent-source-video-audio-root",
            "jobId": "job-agent-source-video-audio-root",
            "mockMode": true,
            "prompt": "bright commercial score",
            "videoSourceMediaRef": "media-1"
        }),
    )
    .expect("source-video audio generation should validate without folderId");

    assert_eq!(
        result.payload["startRequest"]["input"].get("targetFolderId"),
        None
    );
    assert_eq!(
        result.payload["projectActions"][0]["asset"]["targetFolderId"],
        Value::Null
    );
}

#[test]
fn codex_generate_video_accepts_palmier_top_level_reference_and_setting_args() {
    let mut project = sample_project();
    project.media.push(MediaAsset {
        id: "image-ref".to_string(),
        name: Some("Start frame".to_string()),
        relative_path: "media/start-frame.png".to_string(),
        kind: MediaKind::Image,
        duration_seconds: 0.0,
        width: Some(1280),
        height: Some(720),
        fps: None,
        folder_id: None,
    });

    let result = call_codex_local_tool(
        &project,
        "video_creater.generate_video",
        json!({
            "projectDir": "/tmp/video-creater-project",
            "assetId": "agent-video-palmier-refs",
            "jobId": "job-agent-video-palmier-refs",
            "mockMode": true,
            "prompt": "animate the product frame",
            "model": "mock-video-v1",
            "duration": 4,
            "aspectRatio": "16:9",
            "resolution": "1080p",
            "generateAudio": false,
            "sourceVideoMediaRef": "media-1",
            "startFrameMediaRef": "image-ref",
            "endFrameMediaRef": "image-ref",
            "referenceImageMediaRefs": ["image-ref"],
            "referenceVideoMediaRefs": ["media-1"]
        }),
    )
    .expect("Palmier top-level generation args should validate");

    assert!(!result.mutates_project);
    assert_eq!(
        result.payload["startRequest"]["input"]["references"]["mediaIds"],
        json!(["media-1", "image-ref"])
    );
    assert_eq!(
        result.payload["startRequest"]["input"]["references"]["firstFrameMediaId"],
        json!("image-ref")
    );
    assert_eq!(
        result.payload["startRequest"]["input"]["references"]["referenceImageMediaRefs"],
        json!(["image-ref"])
    );
    assert_eq!(
        result.payload["startRequest"]["input"]["references"]["sourceVideoMediaRef"],
        json!("media-1")
    );
    assert_eq!(
        result.payload["startRequest"]["input"]["references"]["referenceVideoMediaRefs"],
        json!(["media-1"])
    );
    assert_eq!(
        result.payload["startRequest"]["input"]["settings"]["durationSeconds"],
        json!(4.0)
    );
    assert_eq!(
        result.payload["startRequest"]["input"]["settings"]["resolution"],
        json!("1080p")
    );
    assert_eq!(
        result.payload["startRequest"]["input"]["settings"]["generateAudio"],
        json!(false)
    );
    assert_eq!(
        result.payload["projectActions"][0]["asset"]["references"]["mediaIds"],
        json!(["media-1", "image-ref"])
    );
    assert_eq!(
        result.payload["projectActions"][0]["asset"]["references"]["sourceVideoMediaRef"],
        json!("media-1")
    );
    assert_eq!(
        result.payload["projectActions"][0]["asset"]["references"]["referenceImageMediaRefs"],
        json!(["image-ref"])
    );
    assert_eq!(
        result.payload["projectActions"][0]["asset"]["references"]["referenceVideoMediaRefs"],
        json!(["media-1"])
    );
    assert_eq!(
        result.payload["projectActions"][0]["asset"]["settings"]["durationSeconds"],
        json!(4.0)
    );
    assert_eq!(
        result.payload["projectActions"][0]["asset"]["settings"]["aspectRatio"],
        json!("16:9")
    );
    assert_eq!(
        result.payload["projectActions"][0]["asset"]["settings"]["resolution"],
        json!("1080p")
    );
    assert_eq!(
        result.payload["projectActions"][0]["asset"]["settings"]["generateAudio"],
        json!(false)
    );
}

#[test]
fn codex_generate_video_defaults_to_palmier_seedance_fast_model() {
    let project = sample_project();

    let result = call_codex_local_tool(
        &project,
        "video_creater.generate_video",
        json!({
            "projectDir": "/tmp/video-creater-project",
            "assetId": "agent-default-seedance-fast-video",
            "jobId": "job-default-seedance-fast-video",
            "mockMode": true,
            "prompt": "slow cinematic product launch video",
            "duration": 5,
            "aspectRatio": "16:9"
        }),
    )
    .expect("default video generation should queue Palmier's default video model");

    assert_eq!(
        result.payload["startRequest"]["input"]["model"],
        json!({
            "provider": REPLICATE_PROVIDER,
            "id": REPLICATE_SEEDANCE_20_FAST_MODEL_ID
        })
    );
    assert_eq!(
        result.payload["projectActions"][0]["asset"]["model"],
        json!({
            "provider": REPLICATE_PROVIDER,
            "id": REPLICATE_SEEDANCE_20_FAST_MODEL_ID
        })
    );
    assert_eq!(
        result.payload["projectActions"][0]["asset"]["settings"]["resolution"],
        json!("720p")
    );
}

#[test]
fn codex_generate_video_accepts_xai_grok_video_model() {
    let project = sample_project();

    let result = call_codex_local_tool(
        &project,
        "video_creater.generate_video",
        json!({
            "projectDir": "/tmp/video-creater-project",
            "assetId": "agent-xai-video",
            "jobId": "job-agent-xai-video",
            "mockMode": true,
            "prompt": "cinematic product launch video",
            "model": format!("{}:{}", XAI_PROVIDER, XAI_GROK_VIDEO_MODEL_ID),
            "duration": 4,
            "aspectRatio": "16:9",
            "resolution": "720p",
            "generateAudio": false
        }),
    )
    .expect("xAI Grok video model should validate");

    assert_eq!(
        result.payload["projectActions"][0]["asset"]["model"]["provider"],
        json!(XAI_PROVIDER)
    );
    assert_eq!(
        result.payload["projectActions"][0]["asset"]["model"]["id"],
        json!(XAI_GROK_VIDEO_MODEL_ID)
    );
    assert_eq!(
        result.payload["projectActions"][0]["asset"]["settings"]["durationSeconds"],
        json!(4.0)
    );
    assert_eq!(
        result.payload["projectActions"][0]["asset"]["settings"]["resolution"],
        json!("720p")
    );
    assert_eq!(
        result.payload["startRequest"]["input"]["settings"]["generateAudio"],
        json!(true)
    );
    assert_eq!(
        result.payload["projectActions"][0]["asset"]["settings"]["generateAudio"],
        json!(true)
    );
}

#[test]
fn codex_generate_video_accepts_xai_grok_video_source_video_edit() {
    let project = sample_project();

    let result = call_codex_local_tool(
        &project,
        "video_creater.generate_video",
        json!({
            "projectDir": "/tmp/video-creater-project",
            "assetId": "agent-xai-video-edit",
            "jobId": "job-agent-xai-video-edit",
            "mockMode": true,
            "prompt": "give the source clip a clean studio background",
            "model": format!("{}:{}", XAI_PROVIDER, XAI_GROK_VIDEO_MODEL_ID),
            "sourceVideoMediaRef": "media-1",
            "sourceClipId": "item-1",
            "duration": 99,
            "aspectRatio": "2:1",
            "resolution": "8k"
        }),
    )
    .expect("xAI Grok video source-video edit should validate and ignore text-video settings");

    assert_eq!(
        result.payload["startRequest"]["input"]["model"],
        json!({
            "provider": XAI_PROVIDER,
            "id": XAI_GROK_VIDEO_MODEL_ID
        })
    );
    assert_eq!(
        result.payload["startRequest"]["input"]["references"]["sourceVideoMediaRef"],
        json!("media-1")
    );
    assert_eq!(
        result.payload["startRequest"]["input"]["settings"]["durationSeconds"],
        json!(4.0)
    );
    assert_eq!(
        result.payload["startRequest"]["input"]["settings"]["aspectRatio"],
        Value::Null
    );
    assert_eq!(
        result.payload["startRequest"]["input"]["settings"]["resolution"],
        Value::Null
    );
    assert_eq!(
        result.payload["projectActions"][0]["asset"]["references"]["sourceVideoMediaRef"],
        json!("media-1")
    );
    assert_eq!(
        result.payload["projectActions"][0]["asset"]["settings"]["durationSeconds"],
        json!(4.0)
    );
}

#[test]
fn codex_generate_video_rejects_xai_grok_video_source_video_over_limit() {
    let project = sample_project();

    let error = call_codex_local_tool(
        &project,
        "video_creater.generate_video",
        json!({
            "projectDir": "/tmp/video-creater-project",
            "assetId": "agent-xai-video-edit-long",
            "jobId": "job-agent-xai-video-edit-long",
            "mockMode": true,
            "prompt": "give the full source video a clean studio background",
            "model": format!("{}:{}", XAI_PROVIDER, XAI_GROK_VIDEO_MODEL_ID),
            "sourceVideoMediaRef": "media-1"
        }),
    )
    .expect_err("xAI Grok video source-video edit should reject over-limit full sources");

    assert!(error
        .to_string()
        .contains("grok-imagine-video accepts at most 8.7s of source video"));
}

#[test]
fn codex_generate_video_accepts_replicate_seedance_fast_model() {
    let mut project = sample_project();
    project.media.push(MediaAsset {
        id: "audio-1".to_string(),
        name: Some("Reference audio".to_string()),
        relative_path: "media/reference.wav".to_string(),
        kind: MediaKind::Audio,
        duration_seconds: 5.0,
        width: None,
        height: None,
        fps: None,
        folder_id: None,
    });

    let result = call_codex_local_tool(
        &project,
        "video_creater.generate_video",
        json!({
            "projectDir": "/tmp/video-creater-project",
            "assetId": "agent-seedance-fast-video",
            "jobId": "job-agent-seedance-fast-video",
            "mockMode": true,
            "prompt": "slow cinematic product launch video with native sound",
            "model": format!("{}:{}", REPLICATE_PROVIDER, REPLICATE_SEEDANCE_20_FAST_MODEL_ID),
            "duration": 5,
            "aspectRatio": "16:9",
            "resolution": "720p",
            "generateAudio": true,
            "referenceAudioMediaRefs": ["audio-1"]
        }),
    )
    .expect("Replicate Seedance Fast model should validate");

    assert_eq!(
        result.payload["projectActions"][0]["asset"]["model"]["provider"],
        json!(REPLICATE_PROVIDER)
    );
    assert_eq!(
        result.payload["projectActions"][0]["asset"]["model"]["id"],
        json!(REPLICATE_SEEDANCE_20_FAST_MODEL_ID)
    );
    assert_eq!(
        result.payload["projectActions"][0]["asset"]["settings"]["durationSeconds"],
        json!(5.0)
    );
    assert_eq!(
        result.payload["projectActions"][0]["asset"]["settings"]["resolution"],
        json!("720p")
    );
    assert_eq!(
        result.payload["projectActions"][0]["asset"]["references"]["referenceAudioMediaRefs"],
        json!(["audio-1"])
    );
    assert!(result.payload["startRequest"]["input"]
        .get("providerCredentialEnvVar")
        .is_none());
}

#[test]
fn codex_generate_video_defaults_text_model_settings() {
    let project = sample_project();

    let result = call_codex_local_tool(
        &project,
        "video_creater.generate_video",
        json!({
            "projectDir": "/tmp/video-creater-project",
            "assetId": "agent-wan-text-video-defaults",
            "jobId": "job-agent-wan-text-video-defaults",
            "mockMode": true,
            "prompt": "cinematic launch teaser with native motion",
            "model": format!("{}:{}", FAL_PROVIDER, FAL_WAN_TEXT_TO_VIDEO_MODEL_ID)
        }),
    )
    .expect("fal.ai WAN text-to-video model should validate with catalog defaults");

    assert_eq!(
        result.payload["startRequest"]["input"]["settings"]["durationSeconds"],
        json!(5)
    );
    assert_eq!(
        result.payload["startRequest"]["input"]["settings"]["aspectRatio"],
        json!("16:9")
    );
    assert_eq!(
        result.payload["startRequest"]["input"]["settings"]["resolution"],
        json!("480p")
    );
    assert_eq!(
        result.payload["startRequest"]["input"]["settings"]["generateAudio"],
        json!(true)
    );
    assert_eq!(
        result.payload["projectActions"][0]["asset"]["settings"]["durationSeconds"],
        json!(5.0)
    );
    assert_eq!(
        result.payload["projectActions"][0]["asset"]["settings"]["aspectRatio"],
        json!("16:9")
    );
    assert_eq!(
        result.payload["projectActions"][0]["asset"]["settings"]["resolution"],
        json!("480p")
    );
    assert_eq!(
        result.payload["projectActions"][0]["asset"]["settings"]["generateAudio"],
        json!(true)
    );
}

#[test]
fn codex_generate_video_accepts_fal_wan_reference_to_video_model() {
    let mut project = sample_project();
    project.media.push(MediaAsset {
        id: "image-ref".to_string(),
        name: Some("Style reference".to_string()),
        relative_path: "media/style.png".to_string(),
        kind: MediaKind::Image,
        duration_seconds: 0.0,
        width: Some(1280),
        height: Some(720),
        fps: None,
        folder_id: None,
    });

    let result = call_codex_local_tool(
        &project,
        "video_creater.generate_video",
        json!({
            "projectDir": "/tmp/video-creater-project",
            "assetId": "agent-wan-reference-video",
            "jobId": "job-agent-wan-reference-video",
            "mockMode": true,
            "prompt": "animate the product using the visual reference",
            "model": format!("{}:{}", FAL_PROVIDER, FAL_WAN_REFERENCE_TO_VIDEO_MODEL_ID),
            "duration": 6,
            "aspectRatio": "4:3",
            "resolution": "720p",
            "referenceImageMediaRefs": ["image-ref"],
            "referenceVideoMediaRefs": ["media-1"]
        }),
    )
    .expect("fal WAN reference-to-video model should validate");

    assert_eq!(
        result.payload["projectActions"][0]["asset"]["model"],
        json!({
            "provider": FAL_PROVIDER,
            "id": FAL_WAN_REFERENCE_TO_VIDEO_MODEL_ID
        })
    );
    assert_eq!(
        result.payload["projectActions"][0]["asset"]["references"]["referenceImageMediaRefs"],
        json!(["image-ref"])
    );
    assert_eq!(
        result.payload["projectActions"][0]["asset"]["references"]["referenceVideoMediaRefs"],
        json!(["media-1"])
    );
    assert_eq!(
        result.payload["projectActions"][0]["asset"]["settings"]["durationSeconds"],
        json!(6.0)
    );
    assert_eq!(
        result.payload["projectActions"][0]["asset"]["settings"]["aspectRatio"],
        json!("4:3")
    );
}

#[test]
fn codex_generate_video_maps_palmier_reference_media_refs_by_kind() {
    let mut project = sample_project();
    project.media.push(MediaAsset {
        id: "image-ref".to_string(),
        name: Some("Style reference".to_string()),
        relative_path: "media/style.png".to_string(),
        kind: MediaKind::Image,
        duration_seconds: 0.0,
        width: Some(1280),
        height: Some(720),
        fps: None,
        folder_id: None,
    });
    project.media.push(MediaAsset {
        id: "audio-ref".to_string(),
        name: Some("Audio reference".to_string()),
        relative_path: "media/reference.wav".to_string(),
        kind: MediaKind::Audio,
        duration_seconds: 4.0,
        width: None,
        height: None,
        fps: None,
        folder_id: None,
    });

    let result = call_codex_local_tool(
        &project,
        "video_creater.generate_video",
        json!({
            "projectDir": "/tmp/video-creater-project",
            "assetId": "agent-seedance-generic-refs",
            "jobId": "job-agent-seedance-generic-refs",
            "mockMode": true,
            "prompt": "use visual, motion, and audio references",
            "model": format!("{}:{}", REPLICATE_PROVIDER, REPLICATE_SEEDANCE_20_MODEL_ID),
            "duration": 5,
            "referenceMediaRefs": ["image-ref", "media-1", "audio-ref"]
        }),
    )
    .expect("Palmier referenceMediaRefs should map to typed video references");

    assert_eq!(
        result.payload["projectActions"][0]["asset"]["references"]["mediaIds"],
        json!(["image-ref", "media-1", "audio-ref"])
    );
    assert_eq!(
        result.payload["projectActions"][0]["asset"]["references"]["referenceImageMediaRefs"],
        json!(["image-ref"])
    );
    assert_eq!(
        result.payload["projectActions"][0]["asset"]["references"]["referenceVideoMediaRefs"],
        json!(["media-1"])
    );
    assert_eq!(
        result.payload["projectActions"][0]["asset"]["references"]["referenceAudioMediaRefs"],
        json!(["audio-ref"])
    );
}

#[test]
fn codex_generate_video_accepts_google_veo_fast_model() {
    let mut project = sample_project();
    project.media.push(MediaAsset {
        id: "first-frame".to_string(),
        name: Some("First frame".to_string()),
        relative_path: "media/first-frame.png".to_string(),
        kind: MediaKind::Image,
        duration_seconds: 0.0,
        width: Some(1280),
        height: Some(720),
        fps: None,
        folder_id: None,
    });
    project.media.push(MediaAsset {
        id: "last-frame".to_string(),
        name: Some("Last frame".to_string()),
        relative_path: "media/last-frame.png".to_string(),
        kind: MediaKind::Image,
        duration_seconds: 0.0,
        width: Some(1280),
        height: Some(720),
        fps: None,
        folder_id: None,
    });
    project.media.push(MediaAsset {
        id: "style-ref".to_string(),
        name: Some("Style reference".to_string()),
        relative_path: "media/style-ref.png".to_string(),
        kind: MediaKind::Image,
        duration_seconds: 0.0,
        width: Some(1280),
        height: Some(720),
        fps: None,
        folder_id: None,
    });

    let result = call_codex_local_tool(
        &project,
        "video_creater.generate_video",
        json!({
            "projectDir": "/tmp/video-creater-project",
            "assetId": "agent-google-veo-video",
            "jobId": "job-agent-google-veo-video",
            "mockMode": true,
            "prompt": "cinematic product launch video with native sound",
            "model": format!("{}:{}", GOOGLE_PROVIDER, GOOGLE_VEO_31_FAST_MODEL_ID),
            "duration": 8,
            "aspectRatio": "16:9",
            "resolution": "720p",
            "generateAudio": true,
            "startFrameMediaRef": "first-frame",
            "endFrameMediaRef": "last-frame",
            "referenceImageMediaRefs": ["style-ref"]
        }),
    )
    .expect("Google Veo model should validate");

    assert_eq!(
        result.payload["projectActions"][0]["asset"]["model"]["provider"],
        json!(GOOGLE_PROVIDER)
    );
    assert_eq!(
        result.payload["projectActions"][0]["asset"]["model"]["id"],
        json!(GOOGLE_VEO_31_FAST_MODEL_ID)
    );
    assert_eq!(
        result.payload["projectActions"][0]["asset"]["settings"]["durationSeconds"],
        json!(8.0)
    );
    assert_eq!(
        result.payload["projectActions"][0]["asset"]["settings"]["aspectRatio"],
        json!("16:9")
    );
    assert_eq!(
        result.payload["projectActions"][0]["asset"]["settings"]["resolution"],
        json!("720p")
    );
    assert_eq!(
        result.payload["projectActions"][0]["asset"]["settings"]["generateAudio"],
        json!(true)
    );
    assert_eq!(
        result.payload["projectActions"][0]["asset"]["references"]["firstFrameMediaId"],
        json!("first-frame")
    );
    assert_eq!(
        result.payload["projectActions"][0]["asset"]["references"]["lastFrameMediaId"],
        json!("last-frame")
    );
    assert_eq!(
        result.payload["projectActions"][0]["asset"]["references"]["referenceImageMediaRefs"],
        json!(["style-ref"])
    );
}

#[test]
fn codex_generate_video_rejects_model_capability_violations_before_queueing() {
    let mut project = sample_project();
    project.media.push(MediaAsset {
        id: "image-ref".to_string(),
        name: Some("Reference image".to_string()),
        relative_path: "media/reference.png".to_string(),
        kind: MediaKind::Image,
        duration_seconds: 0.0,
        width: Some(1280),
        height: Some(720),
        fps: None,
        folder_id: None,
    });
    project.media.push(MediaAsset {
        id: "video-ref-long".to_string(),
        name: Some("Long video reference".to_string()),
        relative_path: "media/long-reference.mp4".to_string(),
        kind: MediaKind::Video,
        duration_seconds: 5.0,
        width: Some(1280),
        height: Some(720),
        fps: Some(24.0),
        folder_id: None,
    });
    project.media.push(MediaAsset {
        id: "audio-ref-a".to_string(),
        name: Some("Audio reference A".to_string()),
        relative_path: "media/audio-a.wav".to_string(),
        kind: MediaKind::Audio,
        duration_seconds: 8.0,
        width: None,
        height: None,
        fps: None,
        folder_id: None,
    });
    project.media.push(MediaAsset {
        id: "audio-ref-b".to_string(),
        name: Some("Audio reference B".to_string()),
        relative_path: "media/audio-b.wav".to_string(),
        kind: MediaKind::Audio,
        duration_seconds: 8.0,
        width: None,
        height: None,
        fps: None,
        folder_id: None,
    });

    let unsupported_duration = call_codex_local_tool(
        &project,
        "video_creater.generate_video",
        json!({
            "prompt": "slow push-in on the product",
            "model": format!("{}:{}", FAL_PROVIDER, FAL_WAN_TEXT_TO_VIDEO_MODEL_ID),
            "duration": 7
        }),
    )
    .expect_err("unsupported model duration should be rejected");
    assert!(unsupported_duration
        .to_string()
        .contains("does not support duration '7s'"));

    let unsupported_default_duration = call_codex_local_tool(
        &project,
        "video_creater.generate_video",
        json!({
            "prompt": "slow push-in on the product",
            "duration": 7
        }),
    )
    .expect_err("default video model should enforce its duration catalog");
    assert!(unsupported_default_duration
        .to_string()
        .contains("bytedance/seedance-2.0-fast does not support duration '7s'"));

    let too_many_audio_refs = call_codex_local_tool(
        &project,
        "video_creater.generate_video",
        json!({
            "prompt": "cut to the beat",
            "model": format!("{}:{}", FAL_PROVIDER, FAL_WAN_TEXT_TO_VIDEO_MODEL_ID),
            "referenceAudioMediaRefs": ["media-1", "media-2"]
        }),
    )
    .expect_err("too many audio references should be rejected");
    assert!(too_many_audio_refs
        .to_string()
        .contains("accepts at most 1 audio reference"));

    let missing_wan_reference_image = call_codex_local_tool(
        &project,
        "video_creater.generate_video",
        json!({
            "prompt": "animate from visual reference context",
            "model": format!("{}:{}", FAL_PROVIDER, FAL_WAN_REFERENCE_TO_VIDEO_MODEL_ID),
            "duration": 4,
            "referenceVideoMediaRefs": ["media-1"]
        }),
    )
    .expect_err("WAN reference-to-video should require an image reference");
    assert!(missing_wan_reference_image
        .to_string()
        .contains("requires an image reference"));

    let mixed_seedance_frames_and_refs = call_codex_local_tool(
        &project,
        "video_creater.generate_video",
        json!({
            "prompt": "blend the first frame and visual reference",
            "model": format!("{}:{}", REPLICATE_PROVIDER, REPLICATE_SEEDANCE_20_MODEL_ID),
            "duration": 5,
            "startFrameMediaRef": "image-ref",
            "referenceImageMediaRefs": ["image-ref"]
        }),
    )
    .expect_err("Seedance frame and reference modes should be exclusive");
    assert!(mixed_seedance_frames_and_refs
        .to_string()
        .contains("uses frames OR references, not both"));

    let long_seedance_video_refs = call_codex_local_tool(
        &project,
        "video_creater.generate_video",
        json!({
            "prompt": "use both videos as motion references",
            "model": format!("{}:{}", REPLICATE_PROVIDER, REPLICATE_SEEDANCE_20_MODEL_ID),
            "duration": 5,
            "referenceVideoMediaRefs": ["media-1", "video-ref-long"]
        }),
    )
    .expect_err("Seedance combined video reference duration should be capped");
    assert!(long_seedance_video_refs
        .to_string()
        .contains("Combined video reference duration exceeds 15s"));

    let long_seedance_audio_refs = call_codex_local_tool(
        &project,
        "video_creater.generate_video",
        json!({
            "prompt": "make visuals react to both audio references",
            "model": format!("{}:{}", REPLICATE_PROVIDER, REPLICATE_SEEDANCE_20_MODEL_ID),
            "duration": 5,
            "referenceAudioMediaRefs": ["audio-ref-a", "audio-ref-b"]
        }),
    )
    .expect_err("Seedance combined audio reference duration should be capped");
    assert!(long_seedance_audio_refs
        .to_string()
        .contains("Combined audio reference duration exceeds 15s"));
}

#[test]
fn codex_generate_video_source_video_edit_allows_only_image_references() {
    let mut project = sample_project();
    project.media[0].duration_seconds = 10.0;
    project.media.push(MediaAsset {
        id: "image-ref".to_string(),
        name: Some("Reference image".to_string()),
        relative_path: "media/reference.png".to_string(),
        kind: MediaKind::Image,
        duration_seconds: 0.0,
        width: Some(1280),
        height: Some(720),
        fps: None,
        folder_id: None,
    });
    project.media.push(MediaAsset {
        id: "audio-ref".to_string(),
        name: Some("Audio reference".to_string()),
        relative_path: "media/reference.wav".to_string(),
        kind: MediaKind::Audio,
        duration_seconds: 3.0,
        width: None,
        height: None,
        fps: None,
        folder_id: None,
    });

    let valid_image_reference = call_codex_local_tool(
        &project,
        "video_creater.generate_video",
        json!({
            "projectDir": "/tmp/video-creater-project",
            "assetId": "agent-video-edit-image-ref",
            "jobId": "job-agent-video-edit-image-ref",
            "mockMode": true,
            "prompt": "restyle the source video with the reference image",
            "model": format!("{}:{}", FAL_PROVIDER, FAL_WAN_VIDEO_TO_VIDEO_MODEL_ID),
            "sourceVideoMediaRef": "media-1",
            "referenceImageMediaRefs": ["image-ref"]
        }),
    )
    .expect("source-video edit should accept image references");
    assert_eq!(
        valid_image_reference.payload["projectActions"][0]["asset"]["references"]
            ["referenceImageMediaRefs"],
        json!(["image-ref"])
    );

    let motion_control_reference = call_codex_local_tool(
        &project,
        "video_creater.generate_video",
        json!({
            "projectDir": "/tmp/video-creater-project",
            "assetId": "agent-kling-motion-control",
            "jobId": "job-agent-kling-motion-control",
            "mockMode": true,
            "prompt": "make the reference character follow the source motion",
            "model": format!("{}:{}", FAL_PROVIDER, FAL_KLING_V3_PRO_MOTION_CONTROL_MODEL_ID),
            "sourceVideoMediaRef": "media-1",
            "referenceImageMediaRefs": ["image-ref"]
        }),
    )
    .expect("Kling motion control should accept source video plus image reference");
    assert_eq!(
        motion_control_reference.payload["startRequest"]["input"]["model"],
        json!({
            "provider": FAL_PROVIDER,
            "id": FAL_KLING_V3_PRO_MOTION_CONTROL_MODEL_ID
        })
    );
    assert_eq!(
        motion_control_reference.payload["startRequest"]["input"]["references"]
            ["sourceVideoMediaRef"],
        json!("media-1")
    );
    assert_eq!(
        motion_control_reference.payload["startRequest"]["input"]["references"]
            ["referenceImageMediaRefs"],
        json!(["image-ref"])
    );

    let missing_motion_control_reference = call_codex_local_tool(
        &project,
        "video_creater.generate_video",
        json!({
            "projectDir": "/tmp/video-creater-project",
            "assetId": "agent-kling-motion-control-missing-ref",
            "jobId": "job-agent-kling-motion-control-missing-ref",
            "mockMode": true,
            "prompt": "make the reference character follow the source motion",
            "model": format!("{}:{}", FAL_PROVIDER, FAL_KLING_V3_PRO_MOTION_CONTROL_MODEL_ID),
            "sourceVideoMediaRef": "media-1"
        }),
    )
    .expect_err("Kling motion control should require an image reference");
    assert!(missing_motion_control_reference
        .to_string()
        .contains("requires an image reference"));

    let too_many_motion_control_references = call_codex_local_tool(
        &project,
        "video_creater.generate_video",
        json!({
            "projectDir": "/tmp/video-creater-project",
            "assetId": "agent-kling-motion-control-too-many-refs",
            "jobId": "job-agent-kling-motion-control-too-many-refs",
            "mockMode": true,
            "prompt": "make the reference character follow the source motion",
            "model": format!("{}:{}", FAL_PROVIDER, FAL_KLING_V3_PRO_MOTION_CONTROL_MODEL_ID),
            "sourceVideoMediaRef": "media-1",
            "referenceImageMediaRefs": ["image-ref", "image-ref"]
        }),
    )
    .expect_err("Kling motion control should accept one image reference");
    assert!(too_many_motion_control_references
        .to_string()
        .contains("accepts at most 1 image reference"));

    for (field_name, value) in [
        ("startFrameMediaRef", json!("image-ref")),
        ("referenceVideoMediaRefs", json!(["media-1"])),
        ("referenceAudioMediaRefs", json!(["audio-ref"])),
    ] {
        let error = call_codex_local_tool(
            &project,
            "video_creater.generate_video",
            json!({
                "projectDir": "/tmp/video-creater-project",
                "assetId": format!("agent-video-edit-{field_name}"),
                "jobId": format!("job-agent-video-edit-{field_name}"),
                "mockMode": true,
                "prompt": "restyle the source video",
                "model": format!("{}:{}", FAL_PROVIDER, FAL_WAN_VIDEO_TO_VIDEO_MODEL_ID),
                "sourceVideoMediaRef": "media-1",
                field_name: value
            }),
        )
        .expect_err("source-video edit should reject non-image references");
        assert!(error
            .to_string()
            .contains("only accepts a source video and image references"));
    }
}

#[test]
fn codex_generate_video_rejects_typed_reference_media_kind_mismatches() {
    let project = sample_project();

    let audio_ref_is_video = call_codex_local_tool(
        &project,
        "video_creater.generate_video",
        json!({
            "prompt": "cut to the beat",
            "model": format!("{}:{}", FAL_PROVIDER, FAL_WAN_TEXT_TO_VIDEO_MODEL_ID),
            "referenceAudioMediaRefs": ["media-1"]
        }),
    )
    .expect_err("video assets should not validate as audio references");

    assert!(audio_ref_is_video
        .to_string()
        .contains("referenceAudioMediaRefs entry 'media-1' must be an audio asset"));

    let video_ref_is_first_frame = call_codex_local_tool(
        &project,
        "video_creater.generate_video",
        json!({
            "prompt": "start from a product still",
            "model": format!("{}:{}", FAL_PROVIDER, FAL_WAN_IMAGE_TO_VIDEO_MODEL_ID),
            "duration": 5,
            "startFrameMediaRef": "media-1"
        }),
    )
    .expect_err("video assets should not validate as frame references");

    assert!(video_ref_is_first_frame
        .to_string()
        .contains("firstFrameMediaId entry 'media-1' must be an image asset"));
}

#[test]
fn codex_generate_video_rejects_missing_reference_media() {
    let project = sample_project();

    let missing_source_video = call_codex_local_tool(
        &project,
        "video_creater.generate_video",
        json!({
            "prompt": "edit the missing source video",
            "sourceVideoMediaRef": "missing-media"
        }),
    )
    .expect_err("missing source-video references should reject before queueing");

    assert!(missing_source_video.to_string().contains("missing-media"));

    let missing_image_reference = call_codex_local_tool(
        &project,
        "video_creater.generate_image",
        json!({
            "prompt": "use the missing image reference",
            "model": format!("{}:{}", OPENAI_PROVIDER, OPENAI_GPT_IMAGE_EDIT_MODEL_ID),
            "referenceImageMediaRefs": ["missing-media"]
        }),
    )
    .expect_err("missing typed image references should reject before queueing");

    let missing_image_reference_error = missing_image_reference.to_string();
    assert!(
        missing_image_reference_error
            .contains("referenceImageMediaRefs entry 'missing-media' references missing media"),
        "{missing_image_reference_error}"
    );

    let missing_palmier_reference = call_codex_local_tool(
        &project,
        "video_creater.generate_video",
        json!({
            "prompt": "use the missing visual reference",
            "model": format!("{}:{}", FAL_PROVIDER, FAL_WAN_REFERENCE_TO_VIDEO_MODEL_ID),
            "duration": 4,
            "referenceMediaRefs": ["missing-media"]
        }),
    )
    .expect_err("missing Palmier referenceMediaRefs should reject before queueing");

    assert!(missing_palmier_reference
        .to_string()
        .contains("referenceMediaRefs entry 'missing-media' references missing media"));
}

#[test]
fn codex_generate_video_source_clip_records_source_video_ref_and_trim_settings() {
    let project = sample_project();

    let result = call_codex_local_tool(
        &project,
        "video_creater.generate_video",
        json!({
            "projectDir": "/tmp/video-creater-project",
            "assetId": "agent-video-source-clip-edit",
            "jobId": "job-agent-video-source-clip-edit",
            "mockMode": true,
            "prompt": "turn this clip into a warm product reveal",
            "model": "fal.ai:fal-ai/wan/v2.2-a14b/video-to-video",
            "sourceClipId": "item-1"
        }),
    )
    .expect("source-clip video edit should validate");

    assert_eq!(
        result.payload["startRequest"]["input"]["references"]["mediaIds"],
        json!(["media-1"])
    );
    assert_eq!(
        result.payload["startRequest"]["input"]["references"]["sourceVideoMediaRef"],
        json!("media-1")
    );
    assert_eq!(
        result.payload["startRequest"]["input"]["settings"]["sourceClipId"],
        json!("item-1")
    );
    assert_eq!(
        result.payload["startRequest"]["input"]["settings"]["sourceIn"],
        json!(0.0)
    );
    assert_eq!(
        result.payload["startRequest"]["input"]["settings"]["sourceOut"],
        json!(4.0)
    );
    assert_eq!(
        result.payload["projectActions"][0]["asset"]["references"]["sourceVideoMediaRef"],
        json!("media-1")
    );
}

#[test]
fn codex_generate_video_defaults_source_video_ref_to_video_to_video_model() {
    let mut project = sample_project();
    project.media[0].duration_seconds = 10.0;

    let result = call_codex_local_tool(
        &project,
        "video_creater.generate_video",
        json!({
            "projectDir": "/tmp/video-creater-project",
            "assetId": "agent-video-source-ref-edit",
            "jobId": "job-agent-video-source-ref-edit",
            "mockMode": true,
            "prompt": "turn this source into a warm product reveal",
            "sourceVideoMediaRef": "media-1"
        }),
    )
    .expect("sourceVideoMediaRef should seed video-to-video generation");

    assert_eq!(
        result.payload["startRequest"]["input"]["model"],
        json!({
            "provider": FAL_PROVIDER,
            "id": FAL_WAN_VIDEO_TO_VIDEO_MODEL_ID
        })
    );
    assert_eq!(
        result.payload["startRequest"]["input"]["references"]["mediaIds"],
        json!(["media-1"])
    );
    assert_eq!(
        result.payload["startRequest"]["input"]["references"]["sourceVideoMediaRef"],
        json!("media-1")
    );
    assert_eq!(
        result.payload["projectActions"][0]["asset"]["references"]["sourceVideoMediaRef"],
        json!("media-1")
    );
}

#[test]
fn codex_generate_video_rejects_over_limit_fal_source_video_edit() {
    let project = sample_project();

    let error = call_codex_local_tool(
        &project,
        "video_creater.generate_video",
        json!({
            "projectDir": "/tmp/video-creater-project",
            "assetId": "agent-video-source-ref-too-long",
            "jobId": "job-agent-video-source-ref-too-long",
            "mockMode": true,
            "prompt": "turn this long source into a warm product reveal",
            "sourceVideoMediaRef": "media-1"
        }),
    )
    .expect_err("over-limit sourceVideoMediaRef should fail before queueing");

    assert!(error
        .to_string()
        .contains("fal-ai/wan/v2.2-a14b/video-to-video accepts at most 10.0s of source video"));
}

#[test]
fn codex_generate_video_source_video_edit_ignores_text_video_settings() {
    let project = sample_project();

    let result = call_codex_local_tool(
        &project,
        "video_creater.generate_video",
        json!({
            "projectDir": "/tmp/video-creater-project",
            "assetId": "agent-video-source-ref-ignores-settings",
            "jobId": "job-agent-video-source-ref-ignores-settings",
            "mockMode": true,
            "prompt": "turn this source into a warm product reveal",
            "model": { "provider": FAL_PROVIDER, "id": FAL_WAN_VIDEO_TO_VIDEO_MODEL_ID },
            "sourceVideoMediaRef": "media-1",
            "sourceClipId": "item-1",
            "duration": 99,
            "aspectRatio": "2:1",
            "resolution": "8k"
        }),
    )
    .expect("source-video edit should ignore text-to-video settings");

    assert_eq!(
        result.payload["startRequest"]["input"]["references"]["sourceVideoMediaRef"],
        json!("media-1")
    );
    assert_eq!(
        result.payload["startRequest"]["input"]["settings"]["durationSeconds"],
        json!(4.0)
    );
    assert_eq!(
        result.payload["startRequest"]["input"]["settings"]["aspectRatio"],
        Value::Null
    );
    assert_eq!(
        result.payload["startRequest"]["input"]["settings"]["resolution"],
        Value::Null
    );
    assert_eq!(
        result.payload["projectActions"][0]["asset"]["settings"]["durationSeconds"],
        json!(4.0)
    );
    assert_eq!(
        result.payload["projectActions"][0]["asset"]["settings"]["aspectRatio"],
        Value::Null
    );
    assert_eq!(
        result.payload["projectActions"][0]["asset"]["settings"]["resolution"],
        Value::Null
    );
}

#[test]
fn codex_generate_video_uses_video_media_ref_as_source_video() {
    let mut project = sample_project();
    project.media[0].duration_seconds = 10.0;

    let result = call_codex_local_tool(
        &project,
        "video_creater.generate_video",
        json!({
            "projectDir": "/tmp/video-creater-project",
            "assetId": "agent-video-media-ref-edit",
            "jobId": "job-agent-video-media-ref-edit",
            "mockMode": true,
            "prompt": "turn this media into a warm product reveal",
            "mediaRef": "media-1"
        }),
    )
    .expect("video mediaRef should seed source-video generation");

    assert_eq!(
        result.payload["startRequest"]["input"]["model"],
        json!({
            "provider": FAL_PROVIDER,
            "id": FAL_WAN_VIDEO_TO_VIDEO_MODEL_ID
        })
    );
    assert_eq!(
        result.payload["startRequest"]["input"]["references"]["mediaIds"],
        json!(["media-1"])
    );
    assert_eq!(
        result.payload["startRequest"]["input"]["references"]["sourceVideoMediaRef"],
        json!("media-1")
    );
    assert_eq!(
        result.payload["projectActions"][0]["asset"]["references"]["sourceVideoMediaRef"],
        json!("media-1")
    );
}

#[test]
fn codex_generate_video_uses_image_media_ref_as_first_frame() {
    let mut project = sample_project();
    project.media.push(MediaAsset {
        id: "image-1".to_string(),
        name: Some("Product still".to_string()),
        relative_path: "media/product-still.png".to_string(),
        kind: MediaKind::Image,
        duration_seconds: 0.0,
        width: Some(1280),
        height: Some(720),
        fps: None,
        folder_id: None,
    });

    let result = call_codex_local_tool(
        &project,
        "video_creater.generate_video",
        json!({
            "projectDir": "/tmp/video-creater-project",
            "assetId": "agent-image-to-video",
            "jobId": "job-agent-image-to-video",
            "mockMode": true,
            "prompt": "animate this product still",
            "mediaRef": "image-1",
            "duration": 5,
            "aspectRatio": "16:9"
        }),
    )
    .expect("image mediaRef should seed image-to-video generation");

    assert_eq!(
        result.payload["startRequest"]["input"]["model"],
        json!({
            "provider": FAL_PROVIDER,
            "id": FAL_WAN_IMAGE_TO_VIDEO_MODEL_ID
        })
    );
    assert_eq!(
        result.payload["startRequest"]["input"]["references"]["mediaIds"],
        json!(["image-1"])
    );
    assert_eq!(
        result.payload["startRequest"]["input"]["references"]["firstFrameMediaId"],
        json!("image-1")
    );
    assert_eq!(
        result.payload["startRequest"]["input"]["references"]["startFrameMediaRef"],
        json!("image-1")
    );
    assert_eq!(
        result.payload["projectActions"][0]["asset"]["references"]["firstFrameMediaId"],
        json!("image-1")
    );
}

#[test]
fn codex_generation_tools_accept_palmier_minimal_arguments() {
    let mut project = sample_project();
    project.media_folders.push(MediaFolder {
        id: "folder-a".to_string(),
        name: "Agent selects".to_string(),
        parent_id: None,
    });

    let video = call_codex_local_tool(
        &project,
        "video_creater.generate_video",
        json!({
            "prompt": "slow push-in on the product",
            "model": "mock-video-v1"
        }),
    )
    .expect("minimal Palmier generate_video args should validate");

    assert!(!video.mutates_project);
    assert_eq!(
        video.payload["startRequest"]["input"]["prompt"],
        json!("slow push-in on the product")
    );
    assert_eq!(
        video.payload["startRequest"]["input"]["assetId"],
        video.payload["projectActions"][0]["asset"]["id"]
    );
    assert_eq!(
        video.payload["projectActions"][0]["asset"]["model"]["id"],
        json!("mock-video-v1")
    );

    let image = call_codex_local_tool(
        &project,
        "video_creater.generate_image",
        json!({
            "prompt": "clean product still",
            "folderId": "folder-a"
        }),
    )
    .expect("minimal Palmier generate_image args should validate");

    assert_eq!(
        image.payload["projectActions"][0]["asset"]["targetFolderId"],
        json!("folder-a")
    );
}

#[test]
fn codex_generate_image_defaults_to_palmier_gpt_image_model() {
    let project = sample_project();

    let image = call_codex_local_tool(
        &project,
        "video_creater.generate_image",
        json!({
            "prompt": "clean product still with crisp readable label",
            "assetId": "agent-default-gpt-image",
            "jobId": "job-default-gpt-image",
            "aspectRatio": "16:9"
        }),
    )
    .expect("minimal Palmier generate_image args should validate");

    assert_eq!(
        image.payload["projectActions"][0]["asset"]["model"]["provider"],
        json!(OPENAI_PROVIDER)
    );
    assert_eq!(
        image.payload["projectActions"][0]["asset"]["model"]["id"],
        json!(OPENAI_GPT_IMAGE_2_MODEL_ID)
    );
    assert_eq!(
        image.payload["startRequest"]["input"]["model"]["provider"],
        json!(OPENAI_PROVIDER)
    );
    assert_eq!(
        image.payload["startRequest"]["input"]["model"]["id"],
        json!(OPENAI_GPT_IMAGE_2_MODEL_ID)
    );
}

#[test]
fn codex_generate_image_preserves_replicate_string_model_args() {
    let mut project = sample_project();
    project.media.push(MediaAsset {
        id: "image-ref".to_string(),
        name: Some("Style reference".to_string()),
        relative_path: "media/style-reference.png".to_string(),
        kind: MediaKind::Image,
        duration_seconds: 0.0,
        width: Some(1024),
        height: Some(768),
        fps: None,
        folder_id: None,
    });

    let provider_qualified = call_codex_local_tool(
        &project,
        "video_creater.generate_image",
        json!({
            "prompt": "editorial product still",
            "model": format!("{}:{}", REPLICATE_PROVIDER, REPLICATE_FLUX_DEV_MODEL_ID)
        }),
    )
    .expect("provider-qualified Replicate model should validate");

    assert_eq!(
        provider_qualified.payload["projectActions"][0]["asset"]["model"]["provider"],
        json!(REPLICATE_PROVIDER)
    );
    assert_eq!(
        provider_qualified.payload["projectActions"][0]["asset"]["model"]["id"],
        json!(REPLICATE_FLUX_DEV_MODEL_ID)
    );
    assert_eq!(
        provider_qualified.payload["startRequest"]["input"]["model"]["provider"],
        json!(REPLICATE_PROVIDER)
    );
    assert_eq!(
        provider_qualified.payload["startRequest"]["input"]["model"]["id"],
        json!(REPLICATE_FLUX_DEV_MODEL_ID)
    );

    let known_bare_model = call_codex_local_tool(
        &project,
        "video_creater.generate_image",
        json!({
            "prompt": "clean product packshot",
            "model": REPLICATE_FLUX_SCHNELL_MODEL_ID
        }),
    )
    .expect("known bare Replicate model should validate");

    assert_eq!(
        known_bare_model.payload["projectActions"][0]["asset"]["model"]["provider"],
        json!(REPLICATE_PROVIDER)
    );
    assert_eq!(
        known_bare_model.payload["projectActions"][0]["asset"]["model"]["id"],
        json!(REPLICATE_FLUX_SCHNELL_MODEL_ID)
    );

    let known_bare_pro_model = call_codex_local_tool(
        &project,
        "video_creater.generate_image",
        json!({
            "prompt": "premium product launch still",
            "model": "black-forest-labs/flux-1.1-pro"
        }),
    )
    .expect("known bare Replicate Pro model should validate");

    assert_eq!(
        known_bare_pro_model.payload["projectActions"][0]["asset"]["model"]["provider"],
        json!(REPLICATE_PROVIDER)
    );
    assert_eq!(
        known_bare_pro_model.payload["projectActions"][0]["asset"]["model"]["id"],
        json!("black-forest-labs/flux-1.1-pro")
    );

    let known_bare_pro_ultra_model = call_codex_local_tool(
        &project,
        "video_creater.generate_image",
        json!({
            "prompt": "premium high resolution launch still",
            "model": "black-forest-labs/flux-1.1-pro-ultra"
        }),
    )
    .expect("known bare Replicate Pro Ultra model should validate");

    assert_eq!(
        known_bare_pro_ultra_model.payload["projectActions"][0]["asset"]["model"]["provider"],
        json!(REPLICATE_PROVIDER)
    );
    assert_eq!(
        known_bare_pro_ultra_model.payload["projectActions"][0]["asset"]["model"]["id"],
        json!("black-forest-labs/flux-1.1-pro-ultra")
    );

    let visible_label_model = call_codex_local_tool(
        &project,
        "video_creater.generate_image",
        json!({
            "prompt": "editorial product still",
            "model": format!("{}/{}", REPLICATE_PROVIDER, REPLICATE_FLUX_DEV_MODEL_ID)
        }),
    )
    .expect("visible Replicate model label should validate");

    assert_eq!(
        visible_label_model.payload["projectActions"][0]["asset"]["model"]["provider"],
        json!(REPLICATE_PROVIDER)
    );
    assert_eq!(
        visible_label_model.payload["projectActions"][0]["asset"]["model"]["id"],
        json!(REPLICATE_FLUX_DEV_MODEL_ID)
    );

    let fal_image_edit_model = call_codex_local_tool(
        &project,
        "video_creater.generate_image",
        json!({
            "prompt": "apply the product style board",
            "model": "fal.ai:fal-ai/nano-banana-pro/edit",
            "referenceImageMediaRefs": ["image-ref"]
        }),
    )
    .expect("provider-qualified fal image edit model should validate");

    assert_eq!(
        fal_image_edit_model.payload["projectActions"][0]["asset"]["model"]["provider"],
        json!("fal.ai")
    );
    assert_eq!(
        fal_image_edit_model.payload["projectActions"][0]["asset"]["model"]["id"],
        json!("fal-ai/nano-banana-pro/edit")
    );
    assert_eq!(
        fal_image_edit_model.payload["projectActions"][0]["asset"]["references"]
            ["referenceImageMediaRefs"],
        json!(["image-ref"])
    );
    assert_eq!(
        fal_image_edit_model.payload["projectActions"][0]["asset"]["references"]["mediaIds"],
        json!(["image-ref"])
    );

    let openai_image_model = call_codex_local_tool(
        &project,
        "video_creater.generate_image",
        json!({
            "prompt": "high fidelity editorial product still",
            "model": format!("{}:{}", OPENAI_PROVIDER, OPENAI_GPT_IMAGE_2_MODEL_ID),
            "resolution": "1536x864"
        }),
    )
    .expect("provider-qualified OpenAI image model should validate");

    assert_eq!(
        openai_image_model.payload["projectActions"][0]["asset"]["model"]["provider"],
        json!(OPENAI_PROVIDER)
    );
    assert_eq!(
        openai_image_model.payload["projectActions"][0]["asset"]["model"]["id"],
        json!(OPENAI_GPT_IMAGE_2_MODEL_ID)
    );
    assert_eq!(
        openai_image_model.payload["startRequest"]["input"]["settings"]["resolution"],
        json!("1536x864")
    );

    let openai_image_edit_model = call_codex_local_tool(
        &project,
        "video_creater.generate_image",
        json!({
            "prompt": "restyle the product with the reference setup",
            "model": format!("{}:{}", OPENAI_PROVIDER, OPENAI_GPT_IMAGE_EDIT_MODEL_ID),
            "referenceImageMediaRefs": ["image-ref"],
            "resolution": "1024x1024"
        }),
    )
    .expect("provider-qualified OpenAI image edit model should validate");

    assert_eq!(
        openai_image_edit_model.payload["projectActions"][0]["asset"]["model"]["provider"],
        json!(OPENAI_PROVIDER)
    );
    assert_eq!(
        openai_image_edit_model.payload["projectActions"][0]["asset"]["model"]["id"],
        json!(OPENAI_GPT_IMAGE_EDIT_MODEL_ID)
    );
    assert_eq!(
        openai_image_edit_model.payload["projectActions"][0]["asset"]["references"]
            ["referenceImageMediaRefs"],
        json!(["image-ref"])
    );
    assert_eq!(
        openai_image_edit_model.payload["projectActions"][0]["asset"]["references"]["mediaIds"],
        json!(["image-ref"])
    );

    let xai_image_model = call_codex_local_tool(
        &project,
        "video_creater.generate_image",
        json!({
            "prompt": "fast simple product iteration",
            "model": format!("{}:{}", XAI_PROVIDER, XAI_GROK_IMAGE_QUALITY_MODEL_ID),
            "aspectRatio": "16:9"
        }),
    )
    .expect("provider-qualified xAI image model should validate");

    assert_eq!(
        xai_image_model.payload["projectActions"][0]["asset"]["model"]["provider"],
        json!(XAI_PROVIDER)
    );
    assert_eq!(
        xai_image_model.payload["projectActions"][0]["asset"]["model"]["id"],
        json!(XAI_GROK_IMAGE_QUALITY_MODEL_ID)
    );

    let xai_image_edit_model = call_codex_local_tool(
        &project,
        "video_creater.generate_image",
        json!({
            "prompt": "turn the product into a rough sketch",
            "model": format!("{}:{}", XAI_PROVIDER, XAI_GROK_IMAGE_QUALITY_MODEL_ID),
            "referenceImageMediaRefs": ["image-ref"],
            "aspectRatio": "1:1"
        }),
    )
    .expect("provider-qualified xAI image edit model should validate");

    assert_eq!(
        xai_image_edit_model.payload["projectActions"][0]["asset"]["model"]["provider"],
        json!(XAI_PROVIDER)
    );
    assert_eq!(
        xai_image_edit_model.payload["projectActions"][0]["asset"]["references"]
            ["referenceImageMediaRefs"],
        json!(["image-ref"])
    );
}

#[test]
fn codex_generate_image_can_replace_timeline_item() {
    let project = sample_project();

    let result = call_codex_local_tool(
        &project,
        "video_creater.generate_image",
        json!({
            "projectDir": "/tmp/video-creater-project",
            "assetId": "agent-image-replacement",
            "jobId": "job-agent-image-replacement",
            "mockMode": true,
            "prompt": "replace the selected clip with a cleaner product still",
            "model": format!("{}:{}", OPENAI_PROVIDER, OPENAI_GPT_IMAGE_2_MODEL_ID),
            "replacementItemId": "item-1"
        }),
    )
    .expect("provider image generation should support clip replacement");

    assert_eq!(
        result.payload["startRequest"]["input"]["placementIntent"],
        json!("replace:item-1")
    );
    assert_eq!(
        result.payload["projectActions"][0]["asset"]["placementIntent"],
        json!("replace:item-1")
    );
    assert_eq!(
        result.payload["projectActions"][0]["asset"]["model"]["provider"],
        json!(OPENAI_PROVIDER)
    );
}

#[test]
fn codex_generate_image_preserves_palmier_num_images_setting() {
    let project = sample_project();

    let result = call_codex_local_tool(
        &project,
        "video_creater.generate_image",
        json!({
            "prompt": "three campaign still variations",
            "model": format!("{}:{}", FAL_PROVIDER, FAL_FLUX_SCHNELL_MODEL_ID),
            "aspectRatio": "1:1",
            "numImages": 3
        }),
    )
    .expect("Palmier numImages should validate for fal Flux Schnell");

    assert_eq!(
        result.payload["projectActions"][0]["asset"]["settings"]["numImages"],
        json!(3)
    );
    assert_eq!(
        result.payload["startRequest"]["input"]["settings"]["numImages"],
        json!(3)
    );
}

#[test]
fn codex_generate_image_accepts_multiple_openai_image_edit_references() {
    let mut project = sample_project();
    project.media.push(MediaAsset {
        id: "style-ref-a".to_string(),
        name: Some("Style reference A".to_string()),
        relative_path: "media/style-reference-a.png".to_string(),
        kind: MediaKind::Image,
        duration_seconds: 0.0,
        width: Some(1024),
        height: Some(1024),
        fps: None,
        folder_id: None,
    });
    project.media.push(MediaAsset {
        id: "style-ref-b".to_string(),
        name: Some("Style reference B".to_string()),
        relative_path: "media/style-reference-b.png".to_string(),
        kind: MediaKind::Image,
        duration_seconds: 0.0,
        width: Some(1024),
        height: Some(1024),
        fps: None,
        folder_id: None,
    });

    let result = call_codex_local_tool(
        &project,
        "video_creater.generate_image",
        json!({
            "prompt": "combine the product styling from both references",
            "model": format!("{}:{}", OPENAI_PROVIDER, OPENAI_GPT_IMAGE_EDIT_MODEL_ID),
            "referenceImageMediaRefs": ["style-ref-a", "style-ref-b"],
            "resolution": "1024x1024"
        }),
    )
    .expect("OpenAI image edit should accept multiple image references");

    assert_eq!(
        result.payload["projectActions"][0]["asset"]["references"]["referenceImageMediaRefs"],
        json!(["style-ref-a", "style-ref-b"])
    );
    assert_eq!(
        result.payload["projectActions"][0]["asset"]["references"]["mediaIds"],
        json!(["style-ref-a", "style-ref-b"])
    );
    assert_eq!(
        result.payload["startRequest"]["input"]["references"]["referenceImageMediaRefs"],
        json!(["style-ref-a", "style-ref-b"])
    );
}

#[test]
fn codex_generate_image_preserves_openai_gpt_image_num_images_setting() {
    let project = sample_project();

    let result = call_codex_local_tool(
        &project,
        "video_creater.generate_image",
        json!({
            "prompt": "two high fidelity campaign still variations",
            "model": format!("{}:{}", OPENAI_PROVIDER, OPENAI_GPT_IMAGE_2_MODEL_ID),
            "resolution": "1024x1024",
            "numImages": 2
        }),
    )
    .expect("OpenAI GPT-image-2 should preserve Palmier numImages");

    assert_eq!(
        result.payload["projectActions"][0]["asset"]["settings"]["numImages"],
        json!(2)
    );
    assert_eq!(
        result.payload["startRequest"]["input"]["settings"]["numImages"],
        json!(2)
    );
    assert!(result.payload["startRequest"]["input"]
        .get("providerCredentialEnvVar")
        .is_none());
}

#[test]
fn codex_generate_image_rejects_num_images_above_model_cap() {
    let project = sample_project();

    let error = call_codex_local_tool(
        &project,
        "video_creater.generate_image",
        json!({
            "prompt": "too many still variations",
            "model": format!("{}:{}", FAL_PROVIDER, FAL_FLUX_SCHNELL_MODEL_ID),
            "numImages": 5
        }),
    )
    .expect_err("fal Flux Schnell should enforce maxImages");

    assert!(error
        .to_string()
        .contains("fal-ai/flux/schnell supports 1..4 images per request (got 5)"));
}

#[test]
fn codex_generate_image_accepts_krea_2_turbo_model_args() {
    let project = sample_project();

    let provider_qualified = call_codex_local_tool(
        &project,
        "video_creater.generate_image",
        json!({
            "prompt": "raw editorial fashion still with saturated wall color",
            "model": format!("{}:{}", FAL_PROVIDER, FAL_KREA_2_TURBO_MODEL_ID),
            "aspectRatio": "16:9",
            "resolution": "1024x576"
        }),
    )
    .expect("provider-qualified Krea model should validate");

    assert_eq!(
        provider_qualified.payload["projectActions"][0]["asset"]["model"]["provider"],
        json!(FAL_PROVIDER)
    );
    assert_eq!(
        provider_qualified.payload["projectActions"][0]["asset"]["model"]["id"],
        json!(FAL_KREA_2_TURBO_MODEL_ID)
    );
    assert_eq!(
        provider_qualified.payload["projectActions"][0]["asset"]["settings"]["width"],
        json!(1024)
    );
    assert_eq!(
        provider_qualified.payload["projectActions"][0]["asset"]["settings"]["height"],
        json!(576)
    );

    let known_bare_model = call_codex_local_tool(
        &project,
        "video_creater.generate_image",
        json!({
            "prompt": "expressive cinematic moodboard still",
            "model": FAL_KREA_2_TURBO_MODEL_ID
        }),
    )
    .expect("known bare Krea model should validate");

    assert_eq!(
        known_bare_model.payload["projectActions"][0]["asset"]["model"]["provider"],
        json!(FAL_PROVIDER)
    );
    assert_eq!(
        known_bare_model.payload["projectActions"][0]["asset"]["model"]["id"],
        json!(FAL_KREA_2_TURBO_MODEL_ID)
    );
}

#[test]
fn codex_generate_image_accepts_recraft_v3_model_args() {
    let project = sample_project();

    let provider_qualified = call_codex_local_tool(
        &project,
        "video_creater.generate_image",
        json!({
            "prompt": "cinematic illustrated travel poster with dramatic evening light",
            "model": format!("{}:{}", FAL_PROVIDER, FAL_RECRAFT_V3_TEXT_TO_IMAGE_MODEL_ID),
            "aspectRatio": "16:9",
            "resolution": "1280x720",
            "quality": "vector_illustration"
        }),
    )
    .expect("provider-qualified Recraft model should validate");

    assert_eq!(
        provider_qualified.payload["projectActions"][0]["asset"]["model"]["provider"],
        json!(FAL_PROVIDER)
    );
    assert_eq!(
        provider_qualified.payload["projectActions"][0]["asset"]["model"]["id"],
        json!(FAL_RECRAFT_V3_TEXT_TO_IMAGE_MODEL_ID)
    );
    assert_eq!(
        provider_qualified.payload["projectActions"][0]["asset"]["settings"]["width"],
        json!(1280)
    );
    assert_eq!(
        provider_qualified.payload["projectActions"][0]["asset"]["settings"]["height"],
        json!(720)
    );
    assert_eq!(
        provider_qualified.payload["projectActions"][0]["asset"]["settings"]["quality"],
        json!("vector_illustration")
    );

    let known_bare_model = call_codex_local_tool(
        &project,
        "video_creater.generate_image",
        json!({
            "prompt": "expressive vector art direction moodboard",
            "model": FAL_RECRAFT_V3_TEXT_TO_IMAGE_MODEL_ID
        }),
    )
    .expect("known bare Recraft model should validate");

    assert_eq!(
        known_bare_model.payload["projectActions"][0]["asset"]["model"]["provider"],
        json!(FAL_PROVIDER)
    );
    assert_eq!(
        known_bare_model.payload["projectActions"][0]["asset"]["model"]["id"],
        json!(FAL_RECRAFT_V3_TEXT_TO_IMAGE_MODEL_ID)
    );
    assert_eq!(
        known_bare_model.payload["projectActions"][0]["asset"]["settings"]["aspectRatio"],
        json!("1:1")
    );
    assert_eq!(
        known_bare_model.payload["projectActions"][0]["asset"]["settings"]["resolution"],
        json!("1024x1024")
    );
    assert_eq!(
        known_bare_model.payload["projectActions"][0]["asset"]["settings"]["quality"],
        json!("vector_illustration")
    );
}

#[test]
fn codex_generate_image_rejects_reference_for_non_reference_model() {
    let project = sample_project();

    let error = call_codex_local_tool(
        &project,
        "video_creater.generate_image",
        json!({
            "prompt": "clean product still",
            "model": format!("{}:{}", OPENAI_PROVIDER, OPENAI_GPT_IMAGE_2_MODEL_ID),
            "referenceImageMediaRefs": ["media-1"]
        }),
    )
    .expect_err("OpenAI GPT-image-2 should reject image references");

    assert!(error
        .to_string()
        .contains("gpt-image-2 does not accept reference images"));
}

#[test]
fn codex_generate_image_rejects_unsupported_resolution() {
    let project = sample_project();

    let error = call_codex_local_tool(
        &project,
        "video_creater.generate_image",
        json!({
            "prompt": "clean product still",
            "model": format!("{}:{}", OPENAI_PROVIDER, OPENAI_GPT_IMAGE_2_MODEL_ID),
            "resolution": "1921x1080"
        }),
    )
    .expect_err("OpenAI GPT-image-2 should reject invalid flexible sizes");

    assert!(error.to_string().contains(
        "gpt-image-2 resolution must use WIDTHxHEIGHT with width and height divisible by 16"
    ));
}

#[test]
fn codex_generate_image_rejects_unsupported_aspect_ratio() {
    let project = sample_project();

    let error = call_codex_local_tool(
        &project,
        "video_creater.generate_image",
        json!({
            "prompt": "clean product still",
            "model": format!("{}:{}", XAI_PROVIDER, XAI_GROK_IMAGE_QUALITY_MODEL_ID),
            "aspectRatio": "4:3"
        }),
    )
    .expect_err("xAI image model should reject unsupported aspect ratios");

    assert!(error
        .to_string()
        .contains("grok-imagine-image-quality does not support aspect ratio '4:3'"));
}

#[test]
fn codex_generate_image_rejects_unsupported_quality() {
    let project = sample_project();

    let error = call_codex_local_tool(
        &project,
        "video_creater.generate_image",
        json!({
            "prompt": "expressive vector art direction moodboard",
            "model": format!("{}:{}", FAL_PROVIDER, FAL_RECRAFT_V3_TEXT_TO_IMAGE_MODEL_ID),
            "quality": "photoreal"
        }),
    )
    .expect_err("Recraft image model should reject unsupported quality labels");

    assert!(error
        .to_string()
        .contains("fal-ai/recraft/v3/text-to-image does not support quality 'photoreal'"));
}

#[test]
fn codex_generate_image_rejects_too_many_reference_images() {
    let project = sample_project();

    let error = call_codex_local_tool(
        &project,
        "video_creater.generate_image",
        json!({
            "prompt": "make two product references consistent",
            "model": format!("{}:{}", FAL_PROVIDER, FAL_NANO_BANANA_PRO_EDIT_MODEL_ID),
            "referenceImageMediaRefs": ["media-1", "media-1"]
        }),
    )
    .expect_err("Nano Banana Pro edit should enforce maxImages");

    assert!(error
        .to_string()
        .contains("fal-ai/nano-banana-pro/edit accepts at most 1 image reference"));
}

#[test]
fn codex_generate_image_treats_palmier_reference_media_refs_as_image_refs() {
    let mut project = sample_project();
    project.media.push(MediaAsset {
        id: "image-1".to_string(),
        name: Some("Style reference".to_string()),
        relative_path: "media/style-reference.png".to_string(),
        kind: MediaKind::Image,
        duration_seconds: 0.0,
        width: Some(1024),
        height: Some(768),
        fps: None,
        folder_id: None,
    });

    let result = call_codex_local_tool(
        &project,
        "video_creater.generate_image",
        json!({
            "prompt": "restyle the product with the reference setup",
            "model": format!("{}:{}", OPENAI_PROVIDER, OPENAI_GPT_IMAGE_EDIT_MODEL_ID),
            "referenceMediaRefs": ["image-1"],
            "resolution": "1024x1024"
        }),
    )
    .expect("Palmier referenceMediaRefs should map to image edit references");

    assert_eq!(
        result.payload["startRequest"]["input"]["references"]["mediaIds"],
        json!(["image-1"])
    );
    assert_eq!(
        result.payload["startRequest"]["input"]["references"]["referenceImageMediaRefs"],
        json!(["image-1"])
    );
    assert_eq!(
        result.payload["projectActions"][0]["asset"]["references"]["referenceImageMediaRefs"],
        json!(["image-1"])
    );
}

#[test]
fn codex_generate_image_rejects_video_media_as_image_reference() {
    let mut project = sample_project();
    project.media.push(MediaAsset {
        id: "audio-ref".to_string(),
        name: Some("Audio reference".to_string()),
        relative_path: "media/reference.wav".to_string(),
        kind: MediaKind::Audio,
        duration_seconds: 4.0,
        width: None,
        height: None,
        fps: None,
        folder_id: None,
    });

    let error = call_codex_local_tool(
        &project,
        "video_creater.generate_image",
        json!({
            "prompt": "restyle the product with the reference setup",
            "model": format!("{}:{}", OPENAI_PROVIDER, OPENAI_GPT_IMAGE_EDIT_MODEL_ID),
            "referenceImageMediaRefs": ["media-1"],
            "resolution": "1024x1024"
        }),
    )
    .expect_err("video assets should not validate as image references");

    assert!(error
        .to_string()
        .contains("referenceImageMediaRefs entry 'media-1' must be an image asset"));

    let video_reference_error = call_codex_local_tool(
        &project,
        "video_creater.generate_image",
        json!({
            "prompt": "restyle the product with the reference setup",
            "model": format!("{}:{}", OPENAI_PROVIDER, OPENAI_GPT_IMAGE_EDIT_MODEL_ID),
            "referenceVideoMediaRefs": ["media-1"],
            "resolution": "1024x1024"
        }),
    )
    .expect_err("image generation should reject typed video references");
    assert!(video_reference_error
        .to_string()
        .contains("gpt-image-1.5 only accepts image references"));

    let audio_reference_error = call_codex_local_tool(
        &project,
        "video_creater.generate_image",
        json!({
            "prompt": "restyle the product with the reference setup",
            "model": format!("{}:{}", OPENAI_PROVIDER, OPENAI_GPT_IMAGE_EDIT_MODEL_ID),
            "referenceAudioMediaRefs": ["audio-ref"],
            "resolution": "1024x1024"
        }),
    )
    .expect_err("image generation should reject typed audio references");
    assert!(audio_reference_error
        .to_string()
        .contains("gpt-image-1.5 only accepts image references"));
}

#[test]
fn codex_generate_image_accepts_palmier_nano_banana_pro_alias() {
    let mut project = sample_project();
    project.media.push(MediaAsset {
        id: "image-ref".to_string(),
        name: Some("Style reference".to_string()),
        relative_path: "media/style-reference.png".to_string(),
        kind: MediaKind::Image,
        duration_seconds: 0.0,
        width: Some(1024),
        height: Some(768),
        fps: None,
        folder_id: None,
    });

    let result = call_codex_local_tool(
        &project,
        "video_creater.generate_image",
        json!({
            "projectDir": "/tmp/video-creater-project",
            "assetId": "agent-nano-banana-alias",
            "jobId": "job-agent-nano-banana-alias",
            "mockMode": true,
            "prompt": "make the product shot cleaner",
            "model": "nano-banana-pro",
            "referenceImageMediaRefs": ["image-ref"],
            "resolution": "2K"
        }),
    )
    .expect("Palmier nano-banana-pro alias should normalize to fal image edit model");

    assert_eq!(
        result.payload["startRequest"]["input"]["model"],
        json!({
            "provider": FAL_PROVIDER,
            "id": FAL_NANO_BANANA_PRO_EDIT_MODEL_ID
        })
    );
    assert_eq!(
        result.payload["projectActions"][0]["asset"]["model"],
        json!({
            "provider": FAL_PROVIDER,
            "id": FAL_NANO_BANANA_PRO_EDIT_MODEL_ID
        })
    );
    assert_eq!(
        result.payload["startRequest"]["input"]["references"]["referenceImageMediaRefs"],
        json!(["image-ref"])
    );
}

#[test]
fn codex_generate_audio_derives_timeline_span_from_palmier_video_source_frames() {
    let project = sample_project();

    let result = call_codex_local_tool(
        &project,
        "video_creater.generate_audio",
        json!({
            "projectDir": "/tmp/video-creater-project",
            "assetId": "agent-audio-span",
            "jobId": "job-agent-audio-span",
            "mockMode": true,
            "prompt": "warm launch-bed music",
            "videoSourceStartFrame": 24,
            "videoSourceEndFrame": 72
        }),
    )
    .expect("Palmier video-source audio span should validate");

    assert!(!result.mutates_project);
    assert_eq!(
        result.payload["startRequest"]["input"]["placementIntent"],
        json!("timeline")
    );
    assert_eq!(
        result.payload["startRequest"]["input"]["model"],
        json!({
            "provider": "fal.ai",
            "id": FAL_SONILO_VIDEO_TO_MUSIC_MODEL_ID
        })
    );
    assert_eq!(
        result.payload["startRequest"]["input"]["settings"]["videoSourceStartFrame"],
        json!(24)
    );
    assert_eq!(
        result.payload["startRequest"]["input"]["settings"]["videoSourceEndFrame"],
        json!(72)
    );
    assert_eq!(
        result.payload["startRequest"]["input"]["settings"]["videoSourceStartSeconds"],
        json!(1.0)
    );
    assert_eq!(
        result.payload["startRequest"]["input"]["settings"]["videoSourceEndSeconds"],
        json!(3.0)
    );
    assert_eq!(
        result.payload["projectActions"][0]["asset"]["placementIntent"],
        json!("timeline")
    );
    assert_eq!(
        result.payload["projectActions"][0]["asset"]["settings"]["durationSeconds"],
        json!(2.0)
    );
    assert_eq!(
        result.payload["projectActions"][0]["asset"]["settings"]["videoSourceStartFrame"],
        json!(24)
    );
    assert_eq!(
        result.payload["projectActions"][0]["asset"]["settings"]["videoSourceEndFrame"],
        json!(72)
    );
    assert_eq!(
        result.payload["projectActions"][0]["asset"]["settings"]["videoSourceStartSeconds"],
        json!(1.0)
    );
    assert_eq!(
        result.payload["projectActions"][0]["asset"]["settings"]["videoSourceEndSeconds"],
        json!(3.0)
    );
    assert_eq!(
        result.payload["projectActions"][0]["asset"]["settings"]["timelineStartSeconds"],
        json!(1.0)
    );
}

#[test]
fn codex_generate_audio_span_persists_timeline_placeholder_for_real_split_project() {
    let project = sample_project();
    let project_dir = tempfile::tempdir().expect("audio span placeholder project dir");
    save_split_project(project_dir.path(), &project).expect("save audio span placeholder project");

    let result = call_codex_local_tool(
        &project,
        "video_creater.generate_audio",
        json!({
            "projectDir": project_dir.path(),
            "assetId": "agent-audio-span",
            "jobId": "job-agent-audio-span",
            "mockMode": true,
            "prompt": "warm launch-bed music",
            "videoSourceStartFrame": 24,
            "videoSourceEndFrame": 72
        }),
    )
    .expect("Palmier video-source audio span should persist placeholder actions");

    assert!(result.mutates_project);
    assert_eq!(
        result.payload["persistedPlaceholderAssetId"],
        json!("agent-audio-span")
    );
    assert_eq!(
        result.payload["projectActionApplication"]["applied"],
        json!(true)
    );

    let persisted =
        load_split_project(project_dir.path()).expect("load audio span placeholder project");
    let generated = persisted
        .generated_assets
        .iter()
        .find(|asset| asset.id == "agent-audio-span")
        .expect("persisted audio span generated placeholder");
    assert_eq!(generated.status, GeneratedAssetStatus::Queued);
    assert_eq!(generated.placement_intent.as_deref(), Some("timeline"));
    assert_eq!(generated.settings.duration_seconds, Some(2.0));
    assert_eq!(generated.settings.timeline_start_seconds, Some(1.0));

    let audio_item = persisted
        .timeline
        .tracks
        .iter()
        .find(|track| track.kind == TrackKind::Audio)
        .and_then(|track| {
            track.items.iter().find(|item| {
                item.properties.get("pendingGeneratedAssetId") == Some(&json!("agent-audio-span"))
            })
        })
        .expect("audio span placeholder should be placed on the timeline");
    assert_eq!(audio_item.kind, TimelineItemKind::AudioClip);
    assert_eq!(audio_item.start_seconds, 1.0);
    assert_eq!(audio_item.duration_seconds, 2.0);
    assert_eq!(
        audio_item.source,
        TimelineSource::Generated {
            artifact_id: "agent-audio-span".to_string()
        }
    );
    assert_eq!(
        audio_item.properties.get("generatedPlaceholder"),
        Some(&json!(true))
    );
}

#[test]
fn codex_generate_audio_defaults_source_video_media_to_video_to_music_model() {
    let project = sample_project();

    let result = call_codex_local_tool(
        &project,
        "video_creater.generate_audio",
        json!({
            "projectDir": "/tmp/video-creater-project",
            "assetId": "agent-audio-video-source-default",
            "jobId": "job-agent-audio-video-source-default",
            "mockMode": true,
            "prompt": "score this product clip",
            "videoSourceMediaRef": "media-1"
        }),
    )
    .expect("video-backed generate_audio should default to a video-to-music model");

    assert_eq!(
        result.payload["startRequest"]["input"]["model"],
        json!({
            "provider": FAL_PROVIDER,
            "id": FAL_SONILO_VIDEO_TO_MUSIC_MODEL_ID
        })
    );
    assert_eq!(
        result.payload["startRequest"]["input"]["references"]["sourceVideoMediaRef"],
        json!("media-1")
    );
    assert_eq!(
        result.payload["startRequest"]["input"]["references"]["referenceVideoMediaRefs"],
        json!(["media-1"])
    );
    assert_eq!(
        result.payload["projectActions"][0]["asset"]["references"]["sourceVideoMediaRef"],
        json!("media-1")
    );
    assert_eq!(
        result.payload["projectActions"][0]["asset"]["references"]["referenceVideoMediaRefs"],
        json!(["media-1"])
    );
}

#[test]
fn codex_generate_audio_source_video_media_ref_defaults_duration_to_source_media_length() {
    let mut project = sample_project();
    project
        .media
        .iter_mut()
        .find(|media| media.id == "media-1")
        .expect("sample source video media")
        .duration_seconds = 7.0;

    let result = call_codex_local_tool(
        &project,
        "video_creater.generate_audio",
        json!({
            "projectDir": "/tmp/video-creater-project",
            "assetId": "agent-audio-video-source-duration",
            "jobId": "job-agent-audio-video-source-duration",
            "mockMode": true,
            "prompt": "score this seven-second product clip",
            "videoSourceMediaRef": "media-1"
        }),
    )
    .expect("video-backed generate_audio should default duration from source media");

    assert_eq!(
        result.payload["startRequest"]["input"]["settings"]["durationSeconds"],
        json!(7.0)
    );
    assert_eq!(
        result.payload["projectActions"][0]["asset"]["settings"]["durationSeconds"],
        json!(7.0)
    );
}

#[test]
fn codex_generate_audio_allows_promptless_video_source_media() {
    let project = sample_project();

    let result = call_codex_local_tool(
        &project,
        "video_creater.generate_audio",
        json!({
            "projectDir": "/tmp/video-creater-project",
            "assetId": "agent-audio-video-source-promptless",
            "jobId": "job-agent-audio-video-source-promptless",
            "mockMode": true,
            "videoSourceMediaRef": "media-1"
        }),
    )
    .expect("video-backed generate_audio should allow promptless source-video audio");

    assert_eq!(
        result.payload["startRequest"]["input"]["model"],
        json!({
            "provider": FAL_PROVIDER,
            "id": FAL_SONILO_VIDEO_TO_MUSIC_MODEL_ID
        })
    );
    assert_eq!(
        result.payload["startRequest"]["input"]["prompt"],
        json!(null)
    );
    assert_eq!(
        result.payload["startRequest"]["input"]["references"]["sourceVideoMediaRef"],
        json!("media-1")
    );
    assert_eq!(
        result.payload["startRequest"]["input"]["references"]["referenceVideoMediaRefs"],
        json!(["media-1"])
    );
    assert_eq!(
        result.payload["projectActions"][0]["asset"]["references"]["sourceVideoMediaRef"],
        json!("media-1")
    );
    assert_eq!(
        result.payload["projectActions"][0]["asset"]["references"]["referenceVideoMediaRefs"],
        json!(["media-1"])
    );
}

#[test]
fn codex_generate_audio_maps_palmier_reference_media_refs_to_video_source() {
    let project = sample_project();

    let result = call_codex_local_tool(
        &project,
        "video_creater.generate_audio",
        json!({
            "projectDir": "/tmp/video-creater-project",
            "assetId": "agent-audio-video-reference-media",
            "jobId": "job-agent-audio-video-reference-media",
            "mockMode": true,
            "referenceMediaRefs": ["media-1"]
        }),
    )
    .expect("video-backed referenceMediaRefs should seed video-to-audio generation");

    assert_eq!(
        result.payload["startRequest"]["input"]["model"],
        json!({
            "provider": FAL_PROVIDER,
            "id": FAL_SONILO_VIDEO_TO_MUSIC_MODEL_ID
        })
    );
    assert_eq!(
        result.payload["startRequest"]["input"]["references"]["sourceVideoMediaRef"],
        json!("media-1")
    );
    assert_eq!(
        result.payload["startRequest"]["input"]["references"]["referenceVideoMediaRefs"],
        json!(["media-1"])
    );
    assert_eq!(
        result.payload["projectActions"][0]["asset"]["references"]["sourceVideoMediaRef"],
        json!("media-1")
    );
    assert_eq!(
        result.payload["projectActions"][0]["asset"]["references"]["referenceVideoMediaRefs"],
        json!(["media-1"])
    );
}

#[test]
fn codex_generate_audio_preserves_openai_tts_voice_settings() {
    let project = sample_project();

    let result = call_codex_local_tool(
        &project,
        "video_creater.generate_audio",
        json!({
            "projectDir": "/tmp/video-creater-project",
            "assetId": "agent-voiceover-1",
            "jobId": "job-agent-voiceover-1",
            "mockMode": true,
            "prompt": "Read this concise product launch narration.",
            "model": { "provider": "openai", "id": "gpt-4o-mini-tts" },
            "voice": "marin",
            "styleInstructions": "calm documentary voice, close mic"
        }),
    )
    .expect("OpenAI TTS audio request should validate");

    assert_eq!(
        result.payload["startRequest"]["input"]["settings"]["voice"],
        json!("marin")
    );
    assert_eq!(
        result.payload["startRequest"]["input"]["settings"]["styleInstructions"],
        json!("calm documentary voice, close mic")
    );
    assert_eq!(
        result.payload["projectActions"][0]["asset"]["settings"]["voice"],
        json!("marin")
    );
    assert_eq!(
        result.payload["projectActions"][0]["asset"]["settings"]["styleInstructions"],
        json!("calm documentary voice, close mic")
    );

    let default_voice = call_codex_local_tool(
        &project,
        "video_creater.generate_audio",
        json!({
            "projectDir": "/tmp/video-creater-project",
            "assetId": "agent-voiceover-default-voice",
            "jobId": "job-agent-voiceover-default-voice",
            "mockMode": true,
            "prompt": "Read this concise product launch narration.",
            "model": { "provider": "openai", "id": "gpt-4o-mini-tts" }
        }),
    )
    .expect("OpenAI TTS audio request should default the voice");

    assert_eq!(
        default_voice.payload["startRequest"]["input"]["settings"]["voice"],
        json!("alloy")
    );
    assert_eq!(
        default_voice.payload["projectActions"][0]["asset"]["settings"]["voice"],
        json!("alloy")
    );

    let cedar_voice = call_codex_local_tool(
        &project,
        "video_creater.generate_audio",
        json!({
            "projectDir": "/tmp/video-creater-project",
            "assetId": "agent-voiceover-cedar",
            "jobId": "job-agent-voiceover-cedar",
            "mockMode": true,
            "prompt": "Read this concise product launch narration.",
            "model": { "provider": "openai", "id": "gpt-4o-mini-tts" },
            "voice": "cedar"
        }),
    )
    .expect("OpenAI TTS audio request should accept cedar");

    assert_eq!(
        cedar_voice.payload["startRequest"]["input"]["settings"]["voice"],
        json!("cedar")
    );
    assert_eq!(
        cedar_voice.payload["projectActions"][0]["asset"]["settings"]["voice"],
        json!("cedar")
    );

    let unsupported_music_fields = call_codex_local_tool(
        &project,
        "video_creater.generate_audio",
        json!({
            "projectDir": "/tmp/video-creater-project",
            "assetId": "agent-voiceover-unsupported-music-fields",
            "jobId": "job-agent-voiceover-unsupported-music-fields",
            "mockMode": true,
            "prompt": "Read this concise product launch narration.",
            "model": { "provider": "openai", "id": "gpt-4o-mini-tts" },
            "lyrics": "[Verse]\nUnsupported lyric line",
            "instrumental": true,
            "styleInstructions": "calm documentary voice, close mic"
        }),
    )
    .expect("OpenAI TTS should ignore unsupported music-only settings");
    assert_eq!(
        unsupported_music_fields.payload["startRequest"]["input"]["settings"]["styleInstructions"],
        json!("calm documentary voice, close mic")
    );
    assert_eq!(
        unsupported_music_fields.payload["startRequest"]["input"]["settings"]["lyrics"],
        Value::Null
    );
    assert_eq!(
        unsupported_music_fields.payload["startRequest"]["input"]["settings"]["instrumental"],
        Value::Null
    );
    assert_eq!(
        unsupported_music_fields.payload["projectActions"][0]["asset"]["settings"]["lyrics"],
        Value::Null
    );
    assert_eq!(
        unsupported_music_fields.payload["projectActions"][0]["asset"]["settings"]["instrumental"],
        Value::Null
    );
}

#[test]
fn codex_generate_audio_preserves_elevenlabs_tts_voice_settings() {
    let project = sample_project();

    let result = call_codex_local_tool(
        &project,
        "video_creater.generate_audio",
        json!({
            "projectDir": "/tmp/video-creater-project",
            "assetId": "agent-elevenlabs-voiceover-1",
            "jobId": "job-agent-elevenlabs-voiceover-1",
            "mockMode": true,
            "prompt": "Read this concise event promo with expressive energy.",
            "model": { "provider": ELEVENLABS_PROVIDER, "id": ELEVENLABS_TTS_V3_MODEL_ID },
            "voice": ELEVENLABS_DEFAULT_VOICE,
            "styleInstructions": "upbeat but natural narration"
        }),
    )
    .expect("ElevenLabs TTS audio request should validate");

    assert_eq!(
        result.payload["startRequest"]["input"]["model"]["provider"],
        json!(ELEVENLABS_PROVIDER)
    );
    assert_eq!(
        result.payload["startRequest"]["input"]["model"]["id"],
        json!(ELEVENLABS_TTS_V3_MODEL_ID)
    );
    assert_eq!(
        result.payload["startRequest"]["input"]["settings"]["voice"],
        json!(ELEVENLABS_DEFAULT_VOICE)
    );
    assert_eq!(
        result.payload["projectActions"][0]["asset"]["settings"]["voice"],
        json!(ELEVENLABS_DEFAULT_VOICE)
    );
}

#[test]
fn codex_generate_audio_preserves_google_gemini_tts_voice_settings() {
    let project = sample_project();

    let result = call_codex_local_tool(
        &project,
        "video_creater.generate_audio",
        json!({
            "projectDir": "/tmp/video-creater-project",
            "assetId": "agent-google-voiceover-1",
            "jobId": "job-agent-google-voiceover-1",
            "mockMode": true,
            "prompt": "Read this concise product launch narration.",
            "model": GOOGLE_GEMINI_TTS_MODEL_ID,
            "voice": "Puck",
            "styleInstructions": "calm documentary voice, close mic"
        }),
    )
    .expect("Google Gemini TTS audio request should validate");

    assert_eq!(
        result.payload["startRequest"]["input"]["model"]["provider"],
        json!(GOOGLE_PROVIDER)
    );
    assert_eq!(
        result.payload["startRequest"]["input"]["model"]["id"],
        json!(GOOGLE_GEMINI_TTS_MODEL_ID)
    );
    assert_eq!(
        result.payload["startRequest"]["input"]["settings"]["voice"],
        json!("Puck")
    );
    assert_eq!(
        result.payload["startRequest"]["input"]["settings"]["styleInstructions"],
        json!("calm documentary voice, close mic")
    );
    assert_eq!(
        result.payload["projectActions"][0]["asset"]["settings"]["voice"],
        json!("Puck")
    );

    let default_voice = call_codex_local_tool(
        &project,
        "video_creater.generate_audio",
        json!({
            "projectDir": "/tmp/video-creater-project",
            "assetId": "agent-google-voiceover-default",
            "jobId": "job-agent-google-voiceover-default",
            "mockMode": true,
            "prompt": "Read this concise product launch narration.",
            "model": GOOGLE_GEMINI_TTS_MODEL_ID
        }),
    )
    .expect("Google Gemini TTS audio request should default the voice");

    assert_eq!(
        default_voice.payload["startRequest"]["input"]["settings"]["voice"],
        json!(GOOGLE_GEMINI_TTS_DEFAULT_VOICE)
    );
    assert_eq!(
        default_voice.payload["projectActions"][0]["asset"]["settings"]["voice"],
        json!(GOOGLE_GEMINI_TTS_DEFAULT_VOICE)
    );
}

#[test]
fn codex_generate_audio_accepts_palmier_gemini_tts_alias() {
    let project = sample_project();

    let result = call_codex_local_tool(
        &project,
        "video_creater.generate_audio",
        json!({
            "projectDir": "/tmp/video-creater-project",
            "assetId": "agent-google-gemini-tts-alias",
            "jobId": "job-agent-google-gemini-tts-alias",
            "mockMode": true,
            "prompt": "Record this line with a bright documentary narrator.",
            "model": "gemini-3.1-flash-tts"
        }),
    )
    .expect("Palmier Gemini TTS alias should normalize to Google TTS model");

    assert_eq!(
        result.payload["startRequest"]["input"]["model"],
        json!({
            "provider": GOOGLE_PROVIDER,
            "id": GOOGLE_GEMINI_TTS_MODEL_ID
        })
    );
    assert_eq!(
        result.payload["projectActions"][0]["asset"]["settings"]["voice"],
        json!(GOOGLE_GEMINI_TTS_DEFAULT_VOICE)
    );
}

#[test]
fn codex_generate_audio_rejects_text_model_video_source() {
    let project = sample_project();

    let error = call_codex_local_tool(
        &project,
        "video_creater.generate_audio",
        json!({
            "projectDir": "/tmp/video-creater-project",
            "assetId": "agent-voiceover-video-source",
            "jobId": "job-agent-voiceover-video-source",
            "mockMode": true,
            "prompt": "Read this concise product launch narration.",
            "model": { "provider": OPENAI_PROVIDER, "id": OPENAI_GPT_4O_MINI_TTS_MODEL_ID },
            "sourceVideoMediaRef": "media-1"
        }),
    )
    .expect_err("text-only TTS model should reject video input");

    assert!(error
        .to_string()
        .contains("gpt-4o-mini-tts does not accept a video input"));
}

#[test]
fn codex_generate_audio_rejects_text_model_video_source_span() {
    let project = sample_project();

    let error = call_codex_local_tool(
        &project,
        "video_creater.generate_audio",
        json!({
            "projectDir": "/tmp/video-creater-project",
            "assetId": "agent-voiceover-video-source-span",
            "jobId": "job-agent-voiceover-video-source-span",
            "mockMode": true,
            "prompt": "Read this concise product launch narration.",
            "model": { "provider": OPENAI_PROVIDER, "id": OPENAI_GPT_4O_MINI_TTS_MODEL_ID },
            "videoSourceStartFrame": 24,
            "videoSourceEndFrame": 72
        }),
    )
    .expect_err("text-only TTS model should reject Palmier video source spans");

    assert!(error
        .to_string()
        .contains("gpt-4o-mini-tts does not accept a video input"));
}

#[test]
fn codex_generate_audio_rejects_video_model_without_video_source() {
    let project = sample_project();

    let error = call_codex_local_tool(
        &project,
        "video_creater.generate_audio",
        json!({
            "projectDir": "/tmp/video-creater-project",
            "assetId": "agent-music-without-video",
            "jobId": "job-agent-music-without-video",
            "mockMode": true,
            "prompt": "warm launch-bed music",
            "model": { "provider": FAL_PROVIDER, "id": FAL_SONILO_VIDEO_TO_MUSIC_MODEL_ID }
        }),
    )
    .expect_err("video-to-music model should require video input");

    assert!(error
        .to_string()
        .contains("sonilo/v1.1/video-to-music requires a video input"));
}

#[test]
fn codex_generate_audio_accepts_palmier_video_to_audio_aliases() {
    let project = sample_project();

    for (model_alias, expected_id, asset_id) in [
        (
            "sonilo-v1.1-video-to-music",
            FAL_SONILO_VIDEO_TO_MUSIC_MODEL_ID,
            "agent-sonilo-alias",
        ),
        (
            "mirelo-sfx-v1.5-video-to-audio",
            FAL_MIRELO_VIDEO_TO_AUDIO_MODEL_ID,
            "agent-mirelo-alias",
        ),
    ] {
        let result = call_codex_local_tool(
            &project,
            "video_creater.generate_audio",
            json!({
                "projectDir": "/tmp/video-creater-project",
                "assetId": asset_id,
                "jobId": format!("job-{asset_id}"),
                "mockMode": true,
                "prompt": "match the source clip",
                "model": model_alias,
                "videoSourceMediaRef": "media-1"
            }),
        )
        .expect("Palmier video-to-audio aliases should normalize to fal models");

        assert_eq!(
            result.payload["startRequest"]["input"]["model"],
            json!({
                "provider": FAL_PROVIDER,
                "id": expected_id
            })
        );
        assert_eq!(
            result.payload["projectActions"][0]["asset"]["references"]["sourceVideoMediaRef"],
            json!("media-1")
        );
    }
}

#[test]
fn codex_generate_music_preserves_elevenlabs_music_settings() {
    let project = sample_project();

    let result = call_codex_local_tool(
        &project,
        "video_creater.generate_music",
        json!({
            "projectDir": "/tmp/video-creater-project",
            "assetId": "agent-elevenlabs-music-1",
            "jobId": "job-agent-elevenlabs-music-1",
            "mockMode": true,
            "prompt": "warm synth pop intro with a bright chorus",
            "model": { "provider": ELEVENLABS_PROVIDER, "id": ELEVENLABS_MUSIC_MODEL_ID },
            "duration": 45,
            "instrumental": true,
            "lyrics": "[Verse]\nMorning light on the launch floor",
            "styleInstructions": "bright, commercial, loopable"
        }),
    )
    .expect("generate ElevenLabs music start request");

    assert_eq!(
        result.payload["startRequest"]["input"]["model"],
        json!({
            "provider": ELEVENLABS_PROVIDER,
            "id": ELEVENLABS_MUSIC_MODEL_ID
        })
    );
    assert_eq!(
        result.payload["startRequest"]["input"]["settings"]["category"],
        json!("music")
    );
    assert_eq!(
        result.payload["startRequest"]["input"]["settings"]["durationSeconds"],
        json!(45.0)
    );
    assert_eq!(
        result.payload["startRequest"]["input"]["settings"]["instrumental"],
        json!(true)
    );
    assert_eq!(
        result.payload["startRequest"]["input"]["settings"]["lyrics"],
        json!("[Verse]\nMorning light on the launch floor")
    );
    assert_eq!(
        result.payload["startRequest"]["input"]["settings"]["styleInstructions"],
        json!("bright, commercial, loopable")
    );
    assert_eq!(
        result.payload["projectActions"][0]["asset"]["settings"]["lyrics"],
        json!("[Verse]\nMorning light on the launch floor")
    );
    assert_eq!(
        result.payload["startRequest"]["input"]["references"]["sourceVideoMediaRef"],
        Value::Null
    );
}

#[test]
fn codex_generate_music_preserves_sonilo_text_to_music_settings() {
    let project = sample_project();

    let result = call_codex_local_tool(
        &project,
        "video_creater.generate_music",
        json!({
            "projectDir": "/tmp/video-creater-project",
            "assetId": "agent-sonilo-text-music-1",
            "jobId": "job-agent-sonilo-text-music-1",
            "mockMode": true,
            "prompt": "cinematic synthwave with a strong chorus lift",
            "model": { "provider": FAL_PROVIDER, "id": FAL_SONILO_TEXT_TO_MUSIC_MODEL_ID },
            "duration": 45
        }),
    )
    .expect("generate Sonilo text-to-music start request without video input");

    assert_eq!(
        result.payload["startRequest"]["input"]["model"],
        json!({
            "provider": FAL_PROVIDER,
            "id": FAL_SONILO_TEXT_TO_MUSIC_MODEL_ID
        })
    );
    assert_eq!(
        result.payload["startRequest"]["input"]["settings"]["category"],
        json!("music")
    );
    assert_eq!(
        result.payload["startRequest"]["input"]["settings"]["durationSeconds"],
        json!(45.0)
    );
    assert_eq!(
        result.payload["startRequest"]["input"]["references"]["sourceVideoMediaRef"],
        Value::Null
    );
    assert_eq!(
        result.payload["projectActions"][0]["asset"]["settings"]["durationSeconds"],
        json!(45.0)
    );
}

#[test]
fn codex_generate_music_preserves_google_lyria_music_settings() {
    let project = sample_project();

    let result = call_codex_local_tool(
        &project,
        "video_creater.generate_music",
        json!({
            "projectDir": "/tmp/video-creater-project",
            "assetId": "agent-google-lyria-1",
            "jobId": "job-agent-google-lyria-1",
            "mockMode": true,
            "prompt": "Create a two minute cinematic synthwave song with a strong chorus.",
            "model": { "provider": GOOGLE_PROVIDER, "id": GOOGLE_LYRIA_3_PRO_MODEL_ID },
            "duration": 45,
            "instrumental": true,
            "lyrics": "[Verse]\nNeon roads below",
            "styleInstructions": "bright, commercial, loopable"
        }),
    )
    .expect("generate Lyria music start request");

    assert_eq!(
        result.payload["startRequest"]["input"]["model"],
        json!({
            "provider": GOOGLE_PROVIDER,
            "id": GOOGLE_LYRIA_3_PRO_MODEL_ID
        })
    );
    assert_eq!(
        result.payload["startRequest"]["input"]["settings"]["category"],
        json!("music")
    );
    assert_eq!(
        result.payload["startRequest"]["input"]["settings"]["durationSeconds"],
        Value::Null
    );
    assert_eq!(
        result.payload["projectActions"][0]["asset"]["settings"]["durationSeconds"],
        Value::Null
    );
    assert_eq!(
        result.payload["startRequest"]["input"]["settings"]["instrumental"],
        json!(true)
    );
    assert_eq!(
        result.payload["startRequest"]["input"]["settings"]["lyrics"],
        json!("[Verse]\nNeon roads below")
    );
    assert_eq!(
        result.payload["startRequest"]["input"]["settings"]["styleInstructions"],
        json!("bright, commercial, loopable")
    );
}

#[test]
fn codex_generate_music_preserves_minimax_music_settings() {
    let project = sample_project();

    let result = call_codex_local_tool(
        &project,
        "video_creater.generate_music",
        json!({
            "projectDir": "/tmp/video-creater-project",
            "assetId": "agent-minimax-music-1",
            "jobId": "job-agent-minimax-music-1",
            "mockMode": true,
            "prompt": "bright indie pop launch bed",
            "model": { "provider": MINIMAX_PROVIDER, "id": MINIMAX_MUSIC_MODEL_ID },
            "lyrics": "[Verse]\nLaunch lights rising",
            "instrumental": false,
            "styleInstructions": "glossy drums, bright hook"
        }),
    )
    .expect("generate MiniMax music start request");

    assert_eq!(
        result.payload["startRequest"]["input"]["model"],
        json!({
            "provider": MINIMAX_PROVIDER,
            "id": MINIMAX_MUSIC_MODEL_ID
        })
    );
    assert_eq!(
        result.payload["startRequest"]["input"]["settings"]["category"],
        json!("music")
    );
    assert_eq!(
        result.payload["startRequest"]["input"]["settings"]["lyrics"],
        json!("[Verse]\nLaunch lights rising")
    );
    assert_eq!(
        result.payload["startRequest"]["input"]["settings"]["instrumental"],
        json!(false)
    );
    assert_eq!(
        result.payload["startRequest"]["input"]["settings"]["styleInstructions"],
        json!("glossy drums, bright hook")
    );
    assert_eq!(
        result.payload["startRequest"]["input"]["references"]["sourceVideoMediaRef"],
        Value::Null
    );
}

#[test]
fn codex_generate_music_ignores_minimax_duration_and_defaults_instrumental() {
    let project = sample_project();

    let result = call_codex_local_tool(
        &project,
        "video_creater.generate_music",
        json!({
            "projectDir": "/tmp/video-creater-project",
            "assetId": "agent-minimax-music-defaults",
            "jobId": "job-agent-minimax-music-defaults",
            "mockMode": true,
            "prompt": "bright indie pop launch bed",
            "model": { "provider": MINIMAX_PROVIDER, "id": MINIMAX_MUSIC_MODEL_ID },
            "duration": 45
        }),
    )
    .expect("generate MiniMax music while ignoring duration");

    assert_eq!(
        result.payload["startRequest"]["input"]["settings"]["category"],
        json!("music")
    );
    assert_eq!(
        result.payload["startRequest"]["input"]["settings"]["durationSeconds"],
        Value::Null
    );
    assert_eq!(
        result.payload["startRequest"]["input"]["settings"]["instrumental"],
        json!(false)
    );
    assert_eq!(
        result.payload["projectActions"][0]["asset"]["settings"]["durationSeconds"],
        Value::Null
    );
    assert_eq!(
        result.payload["projectActions"][0]["asset"]["settings"]["instrumental"],
        json!(false)
    );
}

#[test]
fn codex_generate_audio_rejects_unknown_openai_tts_voice() {
    let project = sample_project();

    let error = call_codex_local_tool(
        &project,
        "video_creater.generate_audio",
        json!({
            "projectDir": "/tmp/video-creater-project",
            "assetId": "agent-voiceover-bad-voice",
            "jobId": "job-agent-voiceover-bad-voice",
            "mockMode": true,
            "prompt": "Read this concise product launch narration.",
            "model": { "provider": OPENAI_PROVIDER, "id": OPENAI_GPT_4O_MINI_TTS_MODEL_ID },
            "voice": "not-a-voice"
        }),
    )
    .expect_err("OpenAI TTS should reject unknown voice names");

    assert!(error
        .to_string()
        .contains("gpt-4o-mini-tts does not support voice 'not-a-voice'"));
}

#[test]
fn codex_generate_audio_rejects_out_of_range_music_durations() {
    let project = sample_project();

    let error = call_codex_local_tool(
        &project,
        "video_creater.generate_audio",
        json!({
            "projectDir": "/tmp/video-creater-project",
            "assetId": "agent-lyria-bad-duration",
            "jobId": "job-agent-lyria-bad-duration",
            "mockMode": true,
            "prompt": "Create a short cinematic launch music bed.",
            "model": { "provider": ELEVENLABS_PROVIDER, "id": ELEVENLABS_MUSIC_MODEL_ID },
            "duration": 2
        }),
    )
    .expect_err("ElevenLabs Music should reject durations below its lower bound");

    assert!(error
        .to_string()
        .contains("elevenlabs-music requires at least 3s of duration"));

    let error = call_codex_local_tool(
        &project,
        "video_creater.generate_audio",
        json!({
            "projectDir": "/tmp/video-creater-project",
            "assetId": "agent-sonilo-bad-duration",
            "jobId": "job-agent-sonilo-bad-duration",
            "mockMode": true,
            "prompt": "Create a short cinematic launch music bed.",
            "model": { "provider": FAL_PROVIDER, "id": FAL_SONILO_TEXT_TO_MUSIC_MODEL_ID },
            "duration": 601
        }),
    )
    .expect_err("Sonilo text-to-music should reject durations above its upper bound");

    assert!(error
        .to_string()
        .contains("sonilo/v1.1/text-to-music accepts at most 600s of duration"));
}

#[test]
fn codex_generate_audio_rejects_empty_openai_tts_prompt() {
    let project = sample_project();

    let error = call_codex_local_tool(
        &project,
        "video_creater.generate_audio",
        json!({
            "projectDir": "/tmp/video-creater-project",
            "assetId": "agent-voiceover-empty-prompt",
            "jobId": "job-agent-voiceover-empty-prompt",
            "mockMode": true,
            "prompt": "   ",
            "model": { "provider": OPENAI_PROVIDER, "id": OPENAI_GPT_4O_MINI_TTS_MODEL_ID }
        }),
    )
    .expect_err("OpenAI TTS should reject empty prompts");

    assert!(error
        .to_string()
        .contains("gpt-4o-mini-tts requires a prompt with at least 1 character"));
}

#[test]
fn codex_generate_audio_rejects_incomplete_palmier_video_source_span() {
    let project = sample_project();

    let error = call_codex_local_tool(
        &project,
        "video_creater.generate_audio",
        json!({
            "projectDir": "/tmp/video-creater-project",
            "assetId": "agent-audio-bad-span",
            "jobId": "job-agent-audio-bad-span",
            "mockMode": true,
            "prompt": "warm launch-bed music",
            "videoSourceStartFrame": 72
        }),
    )
    .expect_err("incomplete Palmier video-source audio span should reject");

    assert!(error
        .to_string()
        .contains("videoSourceStartFrame and videoSourceEndFrame must be passed together"));
}

#[test]
fn codex_generate_audio_rejects_mixed_video_source_media_and_span() {
    let project = sample_project();

    let error = call_codex_local_tool(
        &project,
        "video_creater.generate_audio",
        json!({
            "projectDir": "/tmp/video-creater-project",
            "assetId": "agent-audio-mixed-source",
            "jobId": "job-agent-audio-mixed-source",
            "mockMode": true,
            "prompt": "warm launch-bed music",
            "videoSourceMediaRef": "media-1",
            "videoSourceStartFrame": 24,
            "videoSourceEndFrame": 72
        }),
    )
    .expect_err("Palmier audio generation should reject mixed source modes");

    assert!(error.to_string().contains(
        "videoSourceMediaRef is mutually exclusive with videoSourceStartFrame and videoSourceEndFrame"
    ));
}

#[test]
fn codex_upscale_media_accepts_direct_media_ref_and_source_clip() {
    let project = sample_project();

    let result = call_codex_local_tool(
        &project,
        "video_creater.upscale_media",
        json!({
            "projectDir": "/tmp/video-creater-project",
            "assetId": "agent-upscale-1",
            "jobId": "job-agent-upscale-1",
            "mockMode": true,
            "mediaRef": "media-1",
            "sourceClipId": "item-1",
            "model": "mock-upscale-v1"
        }),
    )
    .expect("direct upscale args should validate");

    assert!(!result.mutates_project);
    assert_eq!(result.payload["status"], json!("started"));
    assert_eq!(
        result.payload["placeholderAssetId"],
        json!("agent-upscale-1")
    );
    assert_eq!(result.payload["assetId"], json!("agent-upscale-1"));
    assert_eq!(result.payload["jobId"], json!("job-agent-upscale-1"));
    assert_eq!(
        result.payload["model"],
        json!({
            "provider": "mock",
            "id": "mock-upscale-v1"
        })
    );
    assert_eq!(result.payload["sourceMediaRef"], json!("media-1"));
    assert_eq!(
        result.payload["startRequest"]["input"]["model"]["id"],
        json!("mock-upscale-v1")
    );
    assert_eq!(
        result.payload["startRequest"]["input"]["references"]["mediaIds"],
        json!(["media-1"])
    );
    assert_eq!(
        result.payload["startRequest"]["input"]["references"]["sourceVideoMediaRef"],
        json!("media-1")
    );
    assert_eq!(
        result.payload["startRequest"]["input"]["references"]["referenceVideoMediaRefs"],
        json!(["media-1"])
    );
    assert_eq!(
        result.payload["startRequest"]["input"]["settings"]["sourceClipId"],
        json!("item-1")
    );
    assert_eq!(
        result.payload["startRequest"]["input"]["settings"]["sourceIn"],
        json!(0.0)
    );
    assert_eq!(
        result.payload["startRequest"]["input"]["settings"]["sourceOut"],
        json!(4.0)
    );
    assert_eq!(
        result.payload["projectActions"][0]["type"],
        json!("recordGeneratedAsset")
    );
    assert_eq!(
        result.payload["projectActions"][0]["asset"]["name"],
        json!("Upscaled input.mp4")
    );
    assert_eq!(
        result.payload["projectActions"][0]["asset"]["references"]["mediaIds"],
        json!(["media-1"])
    );
    assert_eq!(
        result.payload["projectActions"][0]["asset"]["references"]["sourceVideoMediaRef"],
        json!("media-1")
    );
    assert_eq!(
        result.payload["projectActions"][0]["asset"]["references"]["referenceVideoMediaRefs"],
        json!(["media-1"])
    );
    assert_eq!(
        result.payload["projectActions"][0]["asset"]["settings"]["durationSeconds"],
        json!(4.0)
    );
}

#[test]
fn codex_upscale_media_records_source_clip_trim_for_rerun() {
    let mut project = sample_project();
    let source_item = project
        .timeline
        .tracks
        .iter_mut()
        .flat_map(|track| track.items.iter_mut())
        .find(|item| item.id == "item-1")
        .expect("sample source item");
    source_item.duration_seconds = 2.0;
    source_item
        .properties
        .insert("sourceIn".to_string(), json!(1.0));
    source_item
        .properties
        .insert("sourceOut".to_string(), json!(3.0));

    let project_dir = tempfile::tempdir().expect("trimmed upscale project dir");
    save_split_project(project_dir.path(), &project).expect("save trimmed upscale project");

    let result = call_codex_local_tool(
        &project,
        "video_creater.upscale_media",
        json!({
            "projectDir": project_dir.path().display().to_string(),
            "assetId": "agent-upscale-trimmed",
            "jobId": "job-agent-upscale-trimmed",
            "mockMode": true,
            "mediaRef": "media-1",
            "sourceClipId": "item-1",
            "model": "mock-upscale-v1"
        }),
    )
    .expect("trimmed clip upscale should validate");

    assert_eq!(
        result.payload["startRequest"]["input"]["settings"]["sourceIn"],
        json!(1.0)
    );
    assert_eq!(
        result.payload["startRequest"]["input"]["settings"]["sourceOut"],
        json!(3.0)
    );
    assert_eq!(
        result.payload["projectActions"][0]["asset"]["settings"]["durationSeconds"],
        json!(2.0)
    );
    assert_eq!(
        result.payload["projectActions"][0]["asset"]["settings"]["videoSourceStartSeconds"],
        json!(1.0)
    );
    assert_eq!(
        result.payload["projectActions"][0]["asset"]["settings"]["videoSourceEndSeconds"],
        json!(3.0)
    );

    let persisted =
        load_split_project(project_dir.path()).expect("load trimmed upscale placeholder project");
    let rerun = call_codex_local_tool(
        &persisted,
        "video_creater.rerun_generated_asset",
        json!({
            "projectDir": project_dir.path().display().to_string(),
            "sourceAssetId": "agent-upscale-trimmed",
            "assetId": "agent-upscale-trimmed-rerun",
            "jobId": "job-agent-upscale-trimmed-rerun",
            "mockMode": true
        }),
    )
    .expect("trimmed upscale rerun should preserve source clip trim");

    assert_eq!(
        rerun.payload["startRequest"]["input"]["settings"]["videoSourceStartSeconds"],
        json!(1.0)
    );
    assert_eq!(
        rerun.payload["startRequest"]["input"]["settings"]["videoSourceEndSeconds"],
        json!(3.0)
    );
    assert_eq!(
        rerun.payload["projectActions"][0]["asset"]["settings"]["videoSourceStartSeconds"],
        json!(1.0)
    );
    assert_eq!(
        rerun.payload["projectActions"][0]["asset"]["settings"]["videoSourceEndSeconds"],
        json!(3.0)
    );
}

#[test]
fn codex_upscale_media_persists_placeholder_for_real_split_project() {
    let project = sample_project();
    let project_dir = tempfile::tempdir().expect("upscale placeholder project dir");
    save_split_project(project_dir.path(), &project).expect("save upscale placeholder project");

    let result = call_codex_local_tool(
        &project,
        "video_creater.upscale_media",
        json!({
            "projectDir": project_dir.path().display().to_string(),
            "assetId": "agent-upscale-1",
            "jobId": "job-agent-upscale-1",
            "mockMode": true,
            "mediaRef": "media-1",
            "sourceClipId": "item-1",
            "model": "mock-upscale-v1"
        }),
    )
    .expect("direct upscale args should persist placeholder actions");

    assert!(result.mutates_project);
    assert_eq!(
        result.payload["projectActionApplication"]["applied"],
        json!(true)
    );
    assert_eq!(
        result.payload["persistedPlaceholderAssetId"],
        json!("agent-upscale-1")
    );

    let persisted =
        load_split_project(project_dir.path()).expect("load upscale placeholder project");
    let generated = persisted
        .generated_assets
        .iter()
        .find(|asset| asset.id == "agent-upscale-1")
        .expect("persisted upscale generated placeholder");
    assert_eq!(generated.status, GeneratedAssetStatus::Queued);
    assert_eq!(generated.prompt, "Upscale input.mp4");
    assert_eq!(
        generated.references.source_video_media_ref.as_deref(),
        Some("media-1")
    );
    assert_eq!(generated.settings.duration_seconds, Some(4.0));
    let job = persisted
        .jobs
        .iter()
        .find(|job| job.id == "job-agent-upscale-1")
        .expect("persisted upscale generation job");
    assert_eq!(
        job.status,
        video_creater_lib::project::model::JobStatus::Queued
    );
}

#[test]
fn codex_upscale_media_preserves_provider_input_url_for_real_image_upscale() {
    let mut project = sample_project();
    project.media.push(MediaAsset {
        id: "image-1".to_string(),
        name: Some("Poster frame".to_string()),
        relative_path: "media/poster.png".to_string(),
        kind: MediaKind::Image,
        duration_seconds: 0.0,
        width: Some(1024),
        height: Some(768),
        fps: None,
        folder_id: None,
    });

    let result = call_codex_local_tool(
        &project,
        "video_creater.upscale_media",
        json!({
            "projectDir": "/tmp/video-creater-project",
            "assetId": "agent-upscale-1",
            "jobId": "job-agent-upscale-1",
            "mediaRef": "image-1",
            "model": "fal-ai/aura-sr",
            "providerInputUrl": "https://fal.media/uploads/source.png"
        }),
    )
    .expect("real upscale args should validate");

    assert_eq!(
        result.payload["startRequest"]["input"]["references"]["providerInputUrls"],
        json!(["https://fal.media/uploads/source.png"])
    );
    assert_eq!(
        result.payload["startRequest"]["input"]["references"]["referenceImageMediaRefs"],
        json!(["image-1"])
    );
    assert_eq!(
        result.payload["projectActions"][0]["asset"]["references"]["providerInputUrls"],
        json!(["https://fal.media/uploads/source.png"])
    );
    assert_eq!(
        result.payload["projectActions"][0]["asset"]["references"]["referenceImageMediaRefs"],
        json!(["image-1"])
    );
    assert_eq!(
        result.payload["projectActions"][0]["asset"]["model"],
        json!({
            "provider": "fal.ai",
            "id": "fal-ai/aura-sr"
        })
    );
}

#[test]
fn codex_upscale_media_preserves_real_video_upscaler_model() {
    let project = sample_project();

    let result = call_codex_local_tool(
        &project,
        "video_creater.upscale_media",
        json!({
            "projectDir": "/tmp/video-creater-project",
            "assetId": "agent-video-upscale",
            "jobId": "job-agent-video-upscale",
            "mediaRef": "media-1",
            "model": "fal-ai/video-upscaler",
            "providerInputUrl": "https://fal.media/uploads/source-video.mp4"
        }),
    )
    .expect("real video upscale args should validate");

    assert_eq!(
        result.payload["startRequest"]["input"]["model"],
        json!({
            "provider": "fal.ai",
            "id": "fal-ai/video-upscaler"
        })
    );
    assert_eq!(
        result.payload["projectActions"][0]["asset"]["model"],
        json!({
            "provider": "fal.ai",
            "id": "fal-ai/video-upscaler"
        })
    );
    assert_eq!(
        result.payload["projectActions"][0]["asset"]["references"]["mediaIds"],
        json!(["media-1"])
    );
    assert_eq!(
        result.payload["projectActions"][0]["asset"]["references"]["sourceVideoMediaRef"],
        json!("media-1")
    );
    assert_eq!(
        result.payload["projectActions"][0]["asset"]["references"]["referenceVideoMediaRefs"],
        json!(["media-1"])
    );
    assert_eq!(
        result.payload["projectActions"][0]["asset"]["references"]["providerInputUrls"],
        json!(["https://fal.media/uploads/source-video.mp4"])
    );
}

#[test]
fn codex_upscale_media_rejects_video_sources_that_are_already_4k() {
    let mut project = sample_project();
    let media = project
        .media
        .iter_mut()
        .find(|media| media.id == "media-1")
        .expect("fixture video media");
    media.width = Some(3840);
    media.height = Some(2160);

    let error = call_codex_local_tool(
        &project,
        "video_creater.upscale_media",
        json!({
            "projectDir": "/tmp/video-creater-project",
            "mediaRef": "media-1",
            "model": "bytedance-upscaler"
        }),
    )
    .expect_err("4K video sources should not queue another upscale");

    assert!(error.to_string().contains("Already 4K or higher"));
}

#[test]
fn codex_upscale_media_rejects_generated_outputs_that_are_already_upscaled() {
    let mut project = sample_project();
    project.media.push(MediaAsset {
        id: "upscaled-output-1".to_string(),
        name: Some("Upscaled poster".to_string()),
        relative_path: "generated/upscaled-poster.png".to_string(),
        kind: MediaKind::Generated,
        duration_seconds: 0.0,
        width: Some(2048),
        height: Some(1536),
        fps: None,
        folder_id: None,
    });
    project.generated_assets.push(GeneratedAsset {
        schema_version: 1,
        id: "upscaled-poster-1".to_string(),
        kind: MediaKind::Generated,
        status: GeneratedAssetStatus::Completed,
        name: Some("Upscaled poster".to_string()),
        target_folder_id: None,
        placement_intent: Some("library".to_string()),
        prompt: "Upscale Poster frame".to_string(),
        model: GenerationModel {
            provider: FAL_PROVIDER.to_string(),
            id: FAL_AURA_SR_MODEL_ID.to_string(),
        },
        references: GeneratedAssetReferences {
            media_ids: vec!["image-1".to_string()],
            reference_image_media_refs: vec!["image-1".to_string()],
            provider_input_urls: vec!["https://fal.media/uploads/source.png".to_string()],
            ..Default::default()
        },
        settings: GeneratedAssetSettings {
            width: Some(2048),
            height: Some(1536),
            duration_seconds: None,
            fps: None,
            aspect_ratio: Some("4:3".to_string()),
            ..GeneratedAssetSettings::default()
        },
        outputs: vec![GeneratedAssetOutput {
            media_id: "upscaled-output-1".to_string(),
            relative_path: "generated/upscaled-poster.png".to_string(),
            source_url: Some("https://fal.media/generated/upscaled-poster.png".to_string()),
            width: 2048,
            height: 1536,
            duration_seconds: 0.0,
            fps: 0.0,
        }],
        created_at: "2026-07-01T00:00:00Z".to_string(),
        parent_asset_id: None,
        retry_of_asset_id: None,
    });

    let error = call_codex_local_tool(
        &project,
        "video_creater.upscale_media",
        json!({
            "projectDir": "/tmp/video-creater-project",
            "mediaRef": "upscaled-output-1",
            "model": "seedvr-image-upscaler"
        }),
    )
    .expect_err("already-upscaled generated output should reject another upscale");

    assert!(error.to_string().contains("Already upscaled"));
}

#[test]
fn codex_upscale_media_accepts_generated_video_outputs() {
    let mut project = sample_project_with_completed_generated_asset();
    project.media.push(MediaAsset {
        id: "generated-media-1".to_string(),
        name: Some("Generated skyline".to_string()),
        relative_path: "generated/skyline.mp4".to_string(),
        kind: MediaKind::Generated,
        duration_seconds: 5.0,
        width: Some(1280),
        height: Some(720),
        fps: Some(24.0),
        folder_id: None,
    });

    let result = call_codex_local_tool(
        &project,
        "video_creater.upscale_media",
        json!({
            "projectDir": "/tmp/video-creater-project",
            "assetId": "agent-upscale-generated-video",
            "jobId": "job-agent-upscale-generated-video",
            "mockMode": true,
            "mediaRef": "generated-media-1",
            "model": "bytedance-upscaler"
        }),
    )
    .expect("generated video output should upscale like a video asset");

    assert_eq!(
        result.payload["startRequest"]["input"]["model"],
        json!({
            "provider": FAL_PROVIDER,
            "id": FAL_VIDEO_UPSCALER_MODEL_ID
        })
    );
    assert_eq!(
        result.payload["startRequest"]["input"]["references"]["sourceVideoMediaRef"],
        json!("generated-media-1")
    );
    assert_eq!(
        result.payload["startRequest"]["input"]["references"]["referenceVideoMediaRefs"],
        json!(["generated-media-1"])
    );
    assert_eq!(
        result.payload["projectActions"][0]["asset"]["settings"]["durationSeconds"],
        json!(5.0)
    );
}

#[test]
fn codex_upscale_media_accepts_palmier_upscaler_aliases() {
    let mut project = sample_project();
    project.media.push(MediaAsset {
        id: "image-1".to_string(),
        name: Some("Poster frame".to_string()),
        relative_path: "media/poster.png".to_string(),
        kind: MediaKind::Image,
        duration_seconds: 0.0,
        width: Some(1024),
        height: Some(768),
        fps: None,
        folder_id: None,
    });

    let video_result = call_codex_local_tool(
        &project,
        "video_creater.upscale_media",
        json!({
            "projectDir": "/tmp/video-creater-project",
            "assetId": "agent-video-upscale-alias",
            "jobId": "job-agent-video-upscale-alias",
            "mockMode": true,
            "mediaRef": "media-1",
            "model": "bytedance-upscaler"
        }),
    )
    .expect("Palmier bytedance-upscaler alias should normalize to fal video upscaler");

    assert_eq!(
        video_result.payload["startRequest"]["input"]["model"],
        json!({
            "provider": FAL_PROVIDER,
            "id": FAL_VIDEO_UPSCALER_MODEL_ID
        })
    );
    assert_eq!(
        video_result.payload["projectActions"][0]["asset"]["references"]["sourceVideoMediaRef"],
        json!("media-1")
    );

    let image_result = call_codex_local_tool(
        &project,
        "video_creater.upscale_media",
        json!({
            "projectDir": "/tmp/video-creater-project",
            "assetId": "agent-image-upscale-alias",
            "jobId": "job-agent-image-upscale-alias",
            "mockMode": true,
            "mediaRef": "image-1",
            "model": "seedvr-image-upscaler"
        }),
    )
    .expect("Palmier seedvr-image-upscaler alias should normalize to fal image upscaler");

    assert_eq!(
        image_result.payload["startRequest"]["input"]["model"],
        json!({
            "provider": FAL_PROVIDER,
            "id": FAL_AURA_SR_MODEL_ID
        })
    );
    assert_eq!(
        image_result.payload["projectActions"][0]["asset"]["references"]["referenceImageMediaRefs"],
        json!(["image-1"])
    );
}

#[test]
fn codex_upscale_media_rejects_model_media_type_mismatches() {
    let mut project = sample_project();
    project.media.push(MediaAsset {
        id: "image-1".to_string(),
        name: Some("Poster frame".to_string()),
        relative_path: "media/poster.png".to_string(),
        kind: MediaKind::Image,
        duration_seconds: 0.0,
        width: Some(1024),
        height: Some(768),
        fps: None,
        folder_id: None,
    });

    let image_model_on_video = call_codex_local_tool(
        &project,
        "video_creater.upscale_media",
        json!({
            "projectDir": "/tmp/video-creater-project",
            "assetId": "agent-video-with-image-upscaler",
            "jobId": "job-agent-video-with-image-upscaler",
            "mockMode": true,
            "mediaRef": "media-1",
            "model": "seedvr-image-upscaler"
        }),
    )
    .expect_err("image-only upscaler should reject video media");
    assert!(image_model_on_video
        .to_string()
        .contains("fal-ai/aura-sr supports image assets only"));

    let video_model_on_image = call_codex_local_tool(
        &project,
        "video_creater.upscale_media",
        json!({
            "projectDir": "/tmp/video-creater-project",
            "assetId": "agent-image-with-video-upscaler",
            "jobId": "job-agent-image-with-video-upscaler",
            "mockMode": true,
            "mediaRef": "image-1",
            "model": "bytedance-upscaler"
        }),
    )
    .expect_err("video-only upscaler should reject image media");
    assert!(video_model_on_image
        .to_string()
        .contains("fal-ai/video-upscaler supports video assets only"));
}

#[test]
fn codex_upscale_media_preserves_provider_qualified_real_model() {
    let mut project = sample_project();
    project.media.push(MediaAsset {
        id: "image-1".to_string(),
        name: Some("Poster frame".to_string()),
        relative_path: "media/poster.png".to_string(),
        kind: MediaKind::Image,
        duration_seconds: 0.0,
        width: Some(1024),
        height: Some(768),
        fps: None,
        folder_id: None,
    });

    let result = call_codex_local_tool(
        &project,
        "video_creater.upscale_media",
        json!({
            "projectDir": "/tmp/video-creater-project",
            "assetId": "agent-upscale-provider-qualified",
            "jobId": "job-agent-upscale-provider-qualified",
            "mediaRef": "image-1",
            "model": "fal.ai:fal-ai/aura-sr",
            "providerInputUrl": "https://fal.media/uploads/source.png"
        }),
    )
    .expect("provider-qualified real upscale args should validate");

    assert_eq!(
        result.payload["startRequest"]["input"]["model"],
        json!({
            "provider": "fal.ai",
            "id": "fal-ai/aura-sr"
        })
    );
    assert_eq!(
        result.payload["projectActions"][0]["asset"]["model"],
        json!({
            "provider": "fal.ai",
            "id": "fal-ai/aura-sr"
        })
    );

    let visible_label = call_codex_local_tool(
        &project,
        "video_creater.upscale_media",
        json!({
            "projectDir": "/tmp/video-creater-project",
            "assetId": "agent-upscale-visible-label",
            "jobId": "job-agent-upscale-visible-label",
            "mediaRef": "image-1",
            "model": "fal.ai/fal-ai/aura-sr",
            "providerInputUrl": "https://fal.media/uploads/source.png"
        }),
    )
    .expect("visible provider/model upscale label should validate");

    assert_eq!(
        visible_label.payload["projectActions"][0]["asset"]["model"],
        json!({
            "provider": "fal.ai",
            "id": "fal-ai/aura-sr"
        })
    );
}

#[test]
fn codex_upscale_media_accepts_palmier_minimal_arguments() {
    let project = sample_project();

    let result = call_codex_local_tool(
        &project,
        "video_creater.upscale_media",
        json!({
            "mediaRef": "media-1"
        }),
    )
    .expect("minimal Palmier upscale_media args should validate");

    assert!(!result.mutates_project);
    assert_eq!(
        result.payload["startRequest"]["input"]["assetId"],
        result.payload["projectActions"][0]["asset"]["id"]
    );
    assert_eq!(
        result.payload["startRequest"]["input"]["references"]["mediaIds"],
        json!(["media-1"])
    );
    assert_eq!(
        result.payload["projectActions"][0]["asset"]["model"]["id"],
        json!("mock-upscale-v1")
    );
    assert_eq!(
        result.payload["projectActions"][0]["asset"]["references"]["mediaIds"],
        json!(["media-1"])
    );
}

#[test]
fn codex_upscale_media_accepts_palmier_folder_id_alias() {
    let mut project = sample_project();
    project.media_folders.push(MediaFolder {
        id: "source-folder".to_string(),
        name: "Source media".to_string(),
        parent_id: None,
    });
    project.media_folders.push(MediaFolder {
        id: "agent-folder".to_string(),
        name: "Agent selects".to_string(),
        parent_id: None,
    });
    project.media[0].folder_id = Some("source-folder".to_string());

    let result = call_codex_local_tool(
        &project,
        "video_creater.upscale_media",
        json!({
            "projectDir": "/tmp/video-creater-project",
            "assetId": "agent-upscale-folder",
            "jobId": "job-agent-upscale-folder",
            "mockMode": true,
            "mediaRef": "media-1",
            "model": "bytedance-upscaler",
            "folderId": "agent-folder"
        }),
    )
    .expect("Palmier folderId should alias targetFolderId for upscales");

    assert!(!result.mutates_project);
    assert_eq!(
        result.payload["startRequest"]["input"]["targetFolderId"],
        json!("agent-folder")
    );
    assert_eq!(
        result.payload["projectActions"][0]["asset"]["targetFolderId"],
        json!("agent-folder")
    );
}

#[test]
fn codex_upscale_media_can_replace_timeline_item() {
    let project = sample_project();

    let result = call_codex_local_tool(
        &project,
        "video_creater.upscale_media",
        json!({
            "projectDir": "/tmp/video-creater-project",
            "assetId": "agent-upscale-replacement",
            "jobId": "job-agent-upscale-replacement",
            "mockMode": true,
            "mediaRef": "media-1",
            "sourceClipId": "item-1",
            "model": "bytedance-upscaler",
            "replacementItemId": "item-1"
        }),
    )
    .expect("Palmier clip-aware upscale replacement should validate");

    assert!(!result.mutates_project);
    assert_eq!(
        result.payload["startRequest"]["input"]["placementIntent"],
        json!("replace:item-1")
    );
    assert_eq!(
        result.payload["projectActions"][0]["asset"]["placementIntent"],
        json!("replace:item-1")
    );
    assert_eq!(
        result.payload["startRequest"]["input"]["settings"]["sourceClipId"],
        json!("item-1")
    );
}

#[test]
fn codex_generate_music_and_sfx_default_audio_categories() {
    let project = sample_project();

    let music = call_codex_local_tool(
        &project,
        "video_creater.generate_music",
        json!({
            "projectDir": "/tmp/video-creater-project",
            "assetId": "agent-music-1",
            "jobId": "job-agent-music-1",
            "mockMode": true,
            "prompt": "warm synth bed",
            "placementIntent": "timeline"
        }),
    )
    .expect("generate music start request");
    assert_eq!(music.payload["status"], json!("started"));
    assert_eq!(music.payload["placeholderAssetId"], json!("agent-music-1"));
    assert_eq!(music.payload["assetId"], json!("agent-music-1"));
    assert_eq!(music.payload["jobId"], json!("job-agent-music-1"));
    assert_eq!(
        music.payload["model"],
        json!({
            "provider": GOOGLE_PROVIDER,
            "id": GOOGLE_LYRIA_3_PRO_MODEL_ID
        })
    );
    assert_eq!(
        music.payload["startRequest"]["input"]["settings"]["category"],
        json!("music")
    );
    assert_eq!(
        music.payload["startRequest"]["input"]["placementIntent"],
        json!("timeline")
    );
    assert_eq!(
        music.payload["startRequest"]["input"]["model"],
        json!({
            "provider": GOOGLE_PROVIDER,
            "id": GOOGLE_LYRIA_3_PRO_MODEL_ID
        })
    );

    let sfx_error = call_codex_local_tool(
        &project,
        "video_creater.generate_sfx",
        json!({
            "projectDir": "/tmp/video-creater-project",
            "assetId": "agent-sfx-1",
            "jobId": "job-agent-sfx-1",
            "mockMode": true,
            "prompt": "camera shutter",
            "settings": { "durationSeconds": 2 }
        }),
    )
    .expect_err("generate_sfx should require a video source by default");
    assert!(sfx_error
        .to_string()
        .contains("mirelo-ai/sfx-v1.5/video-to-audio requires a video input"));

    let music_from_video = call_codex_local_tool(
        &project,
        "video_creater.generate_music",
        json!({
            "projectDir": "/tmp/video-creater-project",
            "assetId": "agent-video-music-1",
            "jobId": "job-agent-video-music-1",
            "mockMode": true,
            "prompt": "sync the soundtrack to the cut",
            "sourceVideoMediaRef": "media-1"
        }),
    )
    .expect("generate source-video music start request");
    assert_eq!(
        music_from_video.payload["startRequest"]["input"]["model"],
        json!({
            "provider": "fal.ai",
            "id": "sonilo/v1.1/video-to-music"
        })
    );
    assert_eq!(
        music_from_video.payload["startRequest"]["input"]["references"]["sourceVideoMediaRef"],
        json!("media-1")
    );
    assert_eq!(
        music_from_video.payload["startRequest"]["input"]["references"]["referenceVideoMediaRefs"],
        json!(["media-1"])
    );
    assert_eq!(
        music_from_video.payload["projectActions"][0]["asset"]["references"]
            ["referenceVideoMediaRefs"],
        json!(["media-1"])
    );

    let sfx_from_video = call_codex_local_tool(
        &project,
        "video_creater.generate_sfx",
        json!({
            "projectDir": "/tmp/video-creater-project",
            "assetId": "agent-video-sfx-1",
            "jobId": "job-agent-video-sfx-1",
            "mockMode": true,
            "prompt": "sync impacts to movement",
            "sourceVideoMediaRef": "media-1"
        }),
    )
    .expect("generate source-video sfx start request");
    assert_eq!(
        sfx_from_video.payload["startRequest"]["input"]["model"],
        json!({
            "provider": "fal.ai",
            "id": "mirelo-ai/sfx-v1.5/video-to-audio"
        })
    );
    assert_eq!(
        sfx_from_video.payload["startRequest"]["input"]["references"]["sourceVideoMediaRef"],
        json!("media-1")
    );
    assert_eq!(
        sfx_from_video.payload["startRequest"]["input"]["references"]["referenceVideoMediaRefs"],
        json!(["media-1"])
    );
    assert_eq!(
        sfx_from_video.payload["projectActions"][0]["asset"]["references"]
            ["referenceVideoMediaRefs"],
        json!(["media-1"])
    );
}

#[test]
fn codex_generate_audio_rejects_source_video_spans_outside_model_caps() {
    let mut long_project = sample_project();
    long_project
        .media
        .iter_mut()
        .find(|media| media.id == "media-1")
        .expect("sample video media")
        .duration_seconds = 901.0;

    let long_music = call_codex_local_tool(
        &long_project,
        "video_creater.generate_music",
        json!({
            "projectDir": "/tmp/video-creater-project",
            "assetId": "agent-video-music-long",
            "jobId": "job-agent-video-music-long",
            "mockMode": true,
            "prompt": "sync the soundtrack to the whole cut",
            "sourceVideoMediaRef": "media-1"
        }),
    )
    .expect_err("Sonilo video-to-music should reject over-cap source video spans");
    assert!(long_music
        .to_string()
        .contains("sonilo/v1.1/video-to-music accepts at most 900s of source video"));

    let mut short_project = sample_project();
    short_project
        .media
        .iter_mut()
        .find(|media| media.id == "media-1")
        .expect("sample video media")
        .duration_seconds = 0.5;

    let short_sfx = call_codex_local_tool(
        &short_project,
        "video_creater.generate_sfx",
        json!({
            "projectDir": "/tmp/video-creater-project",
            "assetId": "agent-video-sfx-short",
            "jobId": "job-agent-video-sfx-short",
            "mockMode": true,
            "prompt": "sync impacts to movement",
            "sourceVideoMediaRef": "media-1"
        }),
    )
    .expect_err("Mirelo video-to-audio should reject under-cap source video spans");
    assert!(short_sfx
        .to_string()
        .contains("mirelo-ai/sfx-v1.5/video-to-audio requires at least 1s of source video"));
}

#[test]
fn codex_import_media_tool_copies_supported_files_into_project() {
    let project = sample_project();
    let project_dir = tempfile::tempdir().expect("project dir");
    let source_dir = tempfile::tempdir().expect("source dir");
    let source = source_dir.path().join("Hero Take.PNG");
    fs::write(&source, b"fake image").expect("write source image");

    let result = call_codex_local_tool(
        &project,
        "video_creater.import_media",
        json!({
            "projectDir": project_dir.path().display().to_string(),
            "sourcePaths": [source.display().to_string()]
        }),
    )
    .expect("import media payload");

    assert!(result.mutates_project);
    assert_eq!(result.payload["importedCount"], json!(1));
    assert_eq!(result.payload["skippedCount"], json!(0));
    assert_eq!(result.payload["imported"][0]["kind"], json!("image"));
    assert_eq!(result.payload["imported"][0]["name"], json!("hero-take"));
}

#[test]
fn codex_import_media_accepts_palmier_source_path() {
    let project = sample_project();
    let project_dir = tempfile::tempdir().expect("project dir");
    let source_dir = tempfile::tempdir().expect("source dir");
    let source = source_dir.path().join("Interview Clip.mp4");
    fs::write(&source, b"fake video").expect("write source video");

    let result = call_codex_local_tool(
        &project,
        "import_media",
        json!({
            "projectDir": project_dir.path().display().to_string(),
            "source": {
                "path": source.display().to_string()
            }
        }),
    )
    .expect("Palmier source path import should validate");

    assert!(result.mutates_project);
    assert_eq!(result.payload["importedCount"], json!(1));
    assert_eq!(result.payload["skippedCount"], json!(0));
    assert_eq!(result.payload["imported"][0]["kind"], json!("video"));
    assert_eq!(
        result.payload["imported"][0]["name"],
        json!("interview-clip")
    );
}

#[test]
fn codex_import_media_accepts_palmier_source_path_without_project_dir() {
    let project = sample_project();
    let source_dir = tempfile::tempdir().expect("source dir");
    let source = source_dir.path().join("Reference Still.png");
    fs::write(&source, b"fake image").expect("write source image");

    let result = call_codex_local_tool(
        &project,
        "import_media",
        json!({
            "source": {
                "path": source.display().to_string()
            }
        }),
    )
    .expect("Palmier source path import should derive projectDir");

    assert!(result.mutates_project);
    assert_eq!(result.payload["importedCount"], json!(1));
    assert_eq!(result.payload["skippedCount"], json!(0));
    assert_eq!(result.payload["imported"][0]["kind"], json!("image"));
    assert!(result.payload["projectDir"]
        .as_str()
        .expect("project dir")
        .ends_with("/test-project"));
}

#[test]
fn codex_import_media_accepts_lottie_source_path_with_uppercase_json_extension() {
    let project = sample_project();
    let project_dir = tempfile::tempdir().expect("project dir");
    let source_dir = tempfile::tempdir().expect("source dir");
    let source = source_dir.path().join("Animated Badge.JSON");
    fs::write(
        &source,
        br#"{"v":"5.12.0","fr":24,"ip":0,"op":48,"w":1280,"h":720,"layers":[]}"#,
    )
    .expect("write lottie source");

    let result = call_codex_local_tool(
        &project,
        "import_media",
        json!({
            "projectDir": project_dir.path().display().to_string(),
            "source": {
                "path": source.display().to_string()
            }
        }),
    )
    .expect("Palmier Lottie source path import should validate");

    assert!(result.mutates_project);
    assert_eq!(result.payload["importedCount"], json!(1));
    assert_eq!(result.payload["skippedCount"], json!(0));
    assert_eq!(result.payload["imported"][0]["kind"], json!("lottie"));
    assert_eq!(result.payload["imported"][0]["durationSeconds"], json!(2.0));
    assert_eq!(result.payload["imported"][0]["width"], json!(1280));
    assert_eq!(result.payload["imported"][0]["height"], json!(720));
}

#[test]
fn codex_import_media_accepts_palmier_source_bytes() {
    let project = sample_project();
    let project_dir = tempfile::tempdir().expect("project dir");

    let result = call_codex_local_tool(
        &project,
        "import_media",
        json!({
            "projectDir": project_dir.path().display().to_string(),
            "source": {
                "bytes": "aW1hZ2UgYnl0ZXM=",
                "mimeType": "image/png"
            },
            "name": "Inline Product Still"
        }),
    )
    .expect("Palmier source bytes import should validate");

    assert!(result.mutates_project);
    assert_eq!(result.payload["importedCount"], json!(1));
    assert_eq!(result.payload["skippedCount"], json!(0));
    assert_eq!(result.payload["imported"][0]["kind"], json!("image"));
    assert_eq!(
        result.payload["imported"][0]["name"],
        json!("inline-product-still")
    );

    let relative_path = result.payload["imported"][0]["relativePath"]
        .as_str()
        .expect("relative path");
    assert!(relative_path.ends_with(".png"));
    assert_eq!(
        fs::read(project_dir.path().join(relative_path)).expect("imported bytes"),
        b"image bytes"
    );
}

#[test]
fn codex_import_media_accepts_lottie_source_bytes() {
    let project = sample_project();
    let project_dir = tempfile::tempdir().expect("project dir");

    let result = call_codex_local_tool(
        &project,
        "import_media",
        json!({
            "projectDir": project_dir.path().display().to_string(),
            "source": {
                "bytes": "eyJ2IjoiNS4xMi4wIiwiZnIiOjI0LCJpcCI6MCwib3AiOjQ4LCJ3IjoxMjgwLCJoIjo3MjAsImxheWVycyI6W119",
                "mimeType": "application/vnd.lottie+json"
            },
            "name": "Animated Badge"
        }),
    )
    .expect("Lottie source bytes import should validate");

    assert!(result.mutates_project);
    assert_eq!(result.payload["importedCount"], json!(1));
    assert_eq!(result.payload["imported"][0]["kind"], json!("lottie"));
    assert_eq!(
        result.payload["imported"][0]["name"],
        json!("animated-badge")
    );
    assert_eq!(result.payload["imported"][0]["durationSeconds"], json!(2.0));
    assert_eq!(result.payload["imported"][0]["width"], json!(1280));
    assert_eq!(result.payload["imported"][0]["height"], json!(720));

    let relative_path = result.payload["imported"][0]["relativePath"]
        .as_str()
        .expect("relative path");
    assert!(relative_path.ends_with(".json"));
}

#[test]
fn codex_import_media_accepts_palmier_source_url() {
    let project = sample_project();
    let project_dir = tempfile::tempdir().expect("project dir");
    let listener = match TcpListener::bind("127.0.0.1:0") {
        Ok(listener) => listener,
        Err(error) if error.kind() == std::io::ErrorKind::PermissionDenied => return,
        Err(error) => panic!("bind local test server: {error}"),
    };
    let url = format!("http://{}/Remote-Still.png", listener.local_addr().unwrap());
    let server = thread::spawn(move || {
        let (mut stream, _) = listener.accept().expect("accept request");
        let mut request = [0_u8; 1024];
        let _ = stream.read(&mut request);
        stream
            .write_all(
                b"HTTP/1.1 200 OK\r\nContent-Type: image/png\r\nContent-Length: 9\r\nConnection: close\r\n\r\nurl image",
            )
            .expect("write response");
    });

    let result = call_codex_local_tool(
        &project,
        "import_media",
        json!({
            "projectDir": project_dir.path().display().to_string(),
            "source": {
                "url": url
            }
        }),
    )
    .expect("Palmier source url import should validate");

    server.join().expect("server thread");
    assert!(result.mutates_project);
    assert_eq!(result.payload["importedCount"], json!(1));
    assert_eq!(result.payload["imported"][0]["kind"], json!("image"));
    assert_eq!(result.payload["imported"][0]["name"], json!("remote-still"));

    let relative_path = result.payload["imported"][0]["relativePath"]
        .as_str()
        .expect("relative path");
    assert_eq!(
        fs::read(project_dir.path().join(relative_path)).expect("imported url bytes"),
        b"url image"
    );
}

#[test]
fn codex_import_media_rejects_bytes_without_mime_type() {
    let project = sample_project();
    let project_dir = tempfile::tempdir().expect("project dir");

    let error = call_codex_local_tool(
        &project,
        "import_media",
        json!({
            "projectDir": project_dir.path().display().to_string(),
            "source": {
                "bytes": "aW1hZ2UgYnl0ZXM="
            }
        }),
    )
    .expect_err("bytes import without mime type should fail");

    assert!(error
        .to_string()
        .contains("source.mimeType is required when source.bytes is set"));
}

#[test]
fn codex_import_media_applies_palmier_folder_id() {
    let mut project = sample_project();
    project.media_folders.push(MediaFolder {
        id: "agent-folder".to_string(),
        name: "Agent selects".to_string(),
        parent_id: None,
    });
    let project_dir = tempfile::tempdir().expect("project dir");
    let source_dir = tempfile::tempdir().expect("source dir");
    let source = source_dir.path().join("Folder Clip.mp4");
    fs::write(&source, b"fake video").expect("write source video");

    let result = call_codex_local_tool(
        &project,
        "import_media",
        json!({
            "projectDir": project_dir.path().display().to_string(),
            "source": {
                "path": source.display().to_string()
            },
            "folderId": "agent-folder"
        }),
    )
    .expect("Palmier folder import should validate");

    assert!(result.mutates_project);
    assert_eq!(
        result.payload["imported"][0]["folderId"],
        json!("agent-folder")
    );

    let saved = load_project(project_dir.path()).expect("saved project");
    let imported_id = result.payload["imported"][0]["id"]
        .as_str()
        .expect("imported id");
    let imported = saved
        .media
        .iter()
        .find(|media| media.id == imported_id)
        .expect("saved imported media");
    assert_eq!(imported.folder_id.as_deref(), Some("agent-folder"));
}

#[test]
fn codex_create_matte_writes_solid_png_media_asset() {
    let mut project = sample_project();
    project.media_folders.push(MediaFolder {
        id: "agent-folder".to_string(),
        name: "Agent selects".to_string(),
        parent_id: None,
    });
    let project_dir = tempfile::tempdir().expect("project dir");
    save_split_project(project_dir.path(), &project).expect("save split project");

    let result = call_codex_local_tool(
        &project,
        "create_matte",
        json!({
            "projectDir": project_dir.path().display().to_string(),
            "hex": "#3366CC",
            "aspectRatio": "1:1",
            "name": "Brand Blue Matte",
            "folderId": "agent-folder"
        }),
    )
    .expect("Palmier create_matte should write a matte asset");

    assert!(result.mutates_project);
    assert_eq!(result.payload["valid"], json!(true));
    assert_eq!(result.payload["kind"], json!("image"));
    assert_eq!(result.payload["name"], json!("Brand Blue Matte"));
    assert_eq!(result.payload["folderId"], json!("agent-folder"));
    assert_eq!(result.payload["width"], json!(1080));
    assert_eq!(result.payload["height"], json!(1080));

    let relative_path = result.payload["relativePath"]
        .as_str()
        .expect("relative path");
    assert!(relative_path.starts_with("media/"));
    assert!(relative_path.ends_with(".png"));

    let image = image::open(project_dir.path().join(relative_path))
        .expect("matte png should be readable")
        .to_rgba8();
    assert_eq!(image.width(), 1080);
    assert_eq!(image.height(), 1080);
    assert_eq!(image.get_pixel(0, 0).0, [0x33, 0x66, 0xCC, 0xFF]);
    assert_eq!(image.get_pixel(1079, 1079).0, [0x33, 0x66, 0xCC, 0xFF]);

    let saved = load_split_project(project_dir.path()).expect("saved project");
    let media_ref = result.payload["mediaRef"].as_str().expect("media ref");
    let matte = saved
        .media
        .iter()
        .find(|media| media.id == media_ref)
        .expect("saved matte media");
    assert_eq!(matte.kind, MediaKind::Image);
    assert_eq!(matte.name.as_deref(), Some("Brand Blue Matte"));
    assert_eq!(matte.folder_id.as_deref(), Some("agent-folder"));
    assert_eq!(matte.width, Some(1080));
    assert_eq!(matte.height, Some(1080));
    assert_eq!(matte.relative_path, relative_path);
}

#[test]
fn codex_create_matte_rejects_invalid_hex() {
    let project = sample_project();
    let project_dir = tempfile::tempdir().expect("project dir");

    let error = call_codex_local_tool(
        &project,
        "video_creater.create_matte",
        json!({
            "projectDir": project_dir.path().display().to_string(),
            "hex": "not-a-color"
        }),
    )
    .expect_err("invalid matte color should fail");

    assert!(error
        .to_string()
        .contains("create_matte.hex must be a hex color"));
}

#[test]
fn codex_create_matte_rejects_blank_aspect_ratio_when_present() {
    let project = sample_project();
    let project_dir = tempfile::tempdir().expect("project dir");

    let error = call_codex_local_tool(
        &project,
        "video_creater.create_matte",
        json!({
            "projectDir": project_dir.path().display().to_string(),
            "hex": "#3366CC",
            "aspectRatio": "   "
        }),
    )
    .expect_err("blank matte aspect ratio should fail when provided");

    let message = error.to_string();
    assert!(message.contains("create_matte: unknown aspectRatio"));
    assert!(message.contains("Use one of Project, 16:9, 9:16, 1:1, 4:3, 9:14, 2.4:1"));
}

#[test]
fn codex_create_matte_accepts_palmier_short_and_alpha_hex() {
    let project = sample_project();

    for (hex, expected_pixel) in [
        ("#36C", [0x33, 0x66, 0xCC, 0xFF]),
        ("3366CC80", [0x33, 0x66, 0xCC, 0xFF]),
    ] {
        let project_dir = tempfile::tempdir().expect("project dir");

        let result = call_codex_local_tool(
            &project,
            "create_matte",
            json!({
                "projectDir": project_dir.path().display().to_string(),
                "hex": hex,
                "aspectRatio": "1:1",
                "name": format!("Palmier Matte {hex}")
            }),
        )
        .expect("Palmier-compatible matte hex should write a matte asset");

        let relative_path = result.payload["relativePath"]
            .as_str()
            .expect("relative path");
        let image = image::open(project_dir.path().join(relative_path))
            .expect("matte png should be readable")
            .to_rgba8();
        assert_eq!(image.get_pixel(0, 0).0, expected_pixel, "{hex}");
    }
}

#[test]
fn codex_import_media_recursively_imports_palmier_source_directory() {
    let project = sample_project();
    let project_dir = tempfile::tempdir().expect("project dir");
    let source_dir = tempfile::tempdir().expect("source dir");
    let shoot_dir = source_dir.path().join("Shoot Day");
    let nested_dir = shoot_dir.join("B Roll");
    fs::create_dir_all(&nested_dir).expect("nested source dir");
    fs::write(shoot_dir.join("Interview.mp4"), b"fake video").expect("write video");
    fs::write(shoot_dir.join("Dialog.aiff"), b"fake aiff").expect("write aiff");
    fs::write(shoot_dir.join("Music.aifc"), b"fake aifc").expect("write aifc");
    fs::write(nested_dir.join("Cutaway.png"), b"fake image").expect("write image");
    fs::write(nested_dir.join("Scan.tiff"), b"fake tiff").expect("write tiff");
    fs::write(nested_dir.join("Still.heic"), b"fake heic").expect("write heic");
    fs::write(nested_dir.join("notes.txt"), b"ignore me").expect("write unsupported");

    let result = call_codex_local_tool(
        &project,
        "import_media",
        json!({
            "projectDir": project_dir.path().display().to_string(),
            "source": {
                "path": shoot_dir.display().to_string()
            }
        }),
    )
    .expect("Palmier source directory import should validate");

    assert!(result.mutates_project);
    assert_eq!(result.payload["importedCount"], json!(6));
    assert_eq!(result.payload["skippedCount"], json!(1));

    let saved = load_project(project_dir.path()).expect("saved project");
    let folder_names = saved
        .media_folders
        .iter()
        .map(|folder| folder.name.as_str())
        .collect::<Vec<_>>();
    assert!(folder_names.contains(&"Shoot Day"));
    assert!(folder_names.contains(&"B Roll"));

    let interview = saved
        .media
        .iter()
        .find(|media| media.name.as_deref() == Some("interview"))
        .expect("interview media");
    let cutaway = saved
        .media
        .iter()
        .find(|media| media.name.as_deref() == Some("cutaway"))
        .expect("cutaway media");
    let dialog = saved
        .media
        .iter()
        .find(|media| media.name.as_deref() == Some("dialog"))
        .expect("dialog media");
    let music = saved
        .media
        .iter()
        .find(|media| media.name.as_deref() == Some("music"))
        .expect("music media");
    let scan = saved
        .media
        .iter()
        .find(|media| media.name.as_deref() == Some("scan"))
        .expect("scan media");
    let still = saved
        .media
        .iter()
        .find(|media| media.name.as_deref() == Some("still"))
        .expect("still media");
    assert_ne!(interview.folder_id, cutaway.folder_id);
    assert!(interview.folder_id.is_some());
    assert!(cutaway.folder_id.is_some());
    assert_eq!(dialog.kind, MediaKind::Audio);
    assert_eq!(music.kind, MediaKind::Audio);
    assert_eq!(scan.kind, MediaKind::Image);
    assert_eq!(still.kind, MediaKind::Image);
    assert_eq!(dialog.folder_id, interview.folder_id);
    assert_eq!(music.folder_id, interview.folder_id);
    assert_eq!(scan.folder_id, cutaway.folder_id);
    assert_eq!(still.folder_id, cutaway.folder_id);
}

#[test]
fn codex_direct_media_and_settings_tools_accept_palmier_style_arguments() {
    let project = sample_project();

    let rename = call_codex_local_tool(
        &project,
        "video_creater.rename_media",
        json!({
            "entries": [
                { "mediaRef": "media-1", "name": "Hero take" }
            ]
        }),
    )
    .expect("rename media");
    assert!(rename.mutates_project);
    assert_eq!(rename.payload["valid"], json!(true));
    assert_eq!(rename.payload["changedMediaIds"], json!(["media-1"]));
    assert_eq!(
        rename.payload["renamedMedia"][0]["name"],
        json!("Hero take")
    );

    let project_dir = tempfile::tempdir().expect("project dir");
    save_split_project(project_dir.path(), &project).expect("save split project");
    let persisted_rename = call_codex_local_tool(
        &project,
        "video_creater.rename_media",
        json!({
            "projectDir": project_dir.path().display().to_string(),
            "mediaRef": "media-1",
            "name": "Hero take persisted"
        }),
    )
    .expect("rename media should persist");
    assert!(persisted_rename.mutates_project);
    assert_eq!(persisted_rename.payload["applied"], json!(true));
    assert_eq!(
        persisted_rename.payload["agentHistory"]["entryCount"],
        json!(1)
    );
    let persisted = load_split_project(project_dir.path()).expect("load renamed project");
    assert_eq!(
        persisted.media[0].name.as_deref(),
        Some("Hero take persisted")
    );

    let delete = call_codex_local_tool(
        &project,
        "video_creater.delete_media",
        json!({ "assetIds": ["media-1"] }),
    )
    .expect("delete media");
    assert!(delete.mutates_project);
    assert_eq!(delete.payload["valid"], json!(true));
    assert_eq!(delete.payload["deletedMediaIds"], json!(["media-1"]));
    assert_eq!(
        delete.payload["removedTimelineItems"][0]["mediaId"],
        json!("media-1")
    );

    let delete_project_dir = tempfile::tempdir().expect("delete project dir");
    save_split_project(delete_project_dir.path(), &project).expect("save split project");
    let persisted_delete = call_codex_local_tool(
        &project,
        "video_creater.delete_media",
        json!({
            "projectDir": delete_project_dir.path().display().to_string(),
            "assetIds": ["media-1"]
        }),
    )
    .expect("delete media should persist");
    assert!(persisted_delete.mutates_project);
    assert_eq!(persisted_delete.payload["applied"], json!(true));
    assert_eq!(
        persisted_delete.payload["agentHistory"]["entryCount"],
        json!(1)
    );
    let deleted_project =
        load_split_project(delete_project_dir.path()).expect("load deleted-media project");
    assert!(!deleted_project
        .media
        .iter()
        .any(|media| media.id == "media-1"));
    assert!(deleted_project.timeline.tracks[0].items.is_empty());

    let remove_tracks = call_codex_local_tool(
        &project,
        "video_creater.remove_tracks",
        json!({ "trackIndexes": [0] }),
    )
    .expect("remove tracks");
    assert!(remove_tracks.mutates_project);
    assert_eq!(remove_tracks.payload["valid"], json!(true));
    assert_eq!(
        remove_tracks.payload["removedTracks"][0]["trackId"],
        json!("track-video")
    );

    let settings = call_codex_local_tool(
        &project,
        "video_creater.set_project_settings",
        json!({
            "aspectRatio": "9:16",
            "quality": "1080p",
            "fps": 30,
            "captions": "burn_in"
        }),
    )
    .expect("set project settings");
    assert!(settings.mutates_project);
    assert_eq!(settings.payload["valid"], json!(true));
    assert_eq!(settings.payload["settings"]["width"], json!(1080));
    assert_eq!(settings.payload["settings"]["height"], json!(1920));
    assert_eq!(settings.payload["settings"]["fps"], json!(30.0));
    assert_eq!(settings.payload["settings"]["captions"], json!("burn_in"));

    let settings_project_dir = tempfile::tempdir().expect("settings project dir");
    save_split_project(settings_project_dir.path(), &project).expect("save settings project");
    let persisted_settings = call_codex_local_tool(
        &project,
        "video_creater.set_project_settings",
        json!({
            "projectDir": settings_project_dir.path().display().to_string(),
            "aspectRatio": "1:1",
            "quality": "720p",
            "fps": 30
        }),
    )
    .expect("set project settings should persist");
    assert!(persisted_settings.mutates_project);
    assert_eq!(persisted_settings.payload["applied"], json!(true));
    assert_eq!(
        persisted_settings.payload["agentHistory"]["entryCount"],
        json!(1)
    );
    let persisted_settings_project =
        load_split_project(settings_project_dir.path()).expect("load settings project");
    assert_eq!(persisted_settings_project.render_settings.width, 720);
    assert_eq!(persisted_settings_project.render_settings.height, 720);
    assert_eq!(persisted_settings_project.render_settings.fps, 30.0);
}

#[test]
fn codex_remove_tracks_dedupes_palmier_track_indexes() {
    let project = sample_project();

    let result = call_codex_local_tool(
        &project,
        "video_creater.remove_tracks",
        json!({ "trackIndexes": [0, 0] }),
    )
    .expect("Palmier duplicate track indexes should validate");

    assert!(result.mutates_project);
    assert_eq!(result.payload["valid"], json!(true));
    assert_eq!(result.payload["removedTracks"].as_array().unwrap().len(), 1);
    assert_eq!(
        result.payload["removedTracks"][0]["trackId"],
        json!("track-video")
    );
    assert_eq!(result.payload["changedTrackIds"], json!(["track-video"]));
}

#[test]
fn codex_remove_tracks_accepts_direct_track_ids() {
    let project = sample_project();

    let result = call_codex_local_tool(
        &project,
        "video_creater.remove_tracks",
        json!({ "trackIds": ["track-video", "track-video"] }),
    )
    .expect("direct track ids should validate");

    assert!(result.mutates_project);
    assert_eq!(result.payload["valid"], json!(true));
    assert_eq!(result.payload["removedTracks"].as_array().unwrap().len(), 1);
    assert_eq!(result.payload["removedTracks"][0]["trackIndex"], json!(0));
    assert_eq!(result.payload["changedTrackIds"], json!(["track-video"]));
}

#[test]
fn codex_remove_tracks_direct_args_persist_to_split_project() {
    let project = sample_project();
    let project_dir = tempfile::tempdir().expect("project dir");
    save_split_project(project_dir.path(), &project).expect("save split project");

    let result = call_codex_local_tool(
        &project,
        "video_creater.remove_tracks",
        json!({
            "projectDir": project_dir.path().display().to_string(),
            "trackIds": ["track-video"]
        }),
    )
    .expect("direct remove tracks args should persist");

    assert!(result.mutates_project);
    assert_eq!(result.payload["applied"], json!(true));
    assert_eq!(result.payload["actionCount"], json!(1));
    assert_eq!(result.payload["agentHistory"]["entryCount"], json!(1));
    assert_eq!(result.payload["changedTrackIds"], json!(["track-video"]));
    let persisted = load_split_project(project_dir.path()).expect("load removed tracks project");
    assert!(!persisted
        .timeline
        .tracks
        .iter()
        .any(|track| track.id == "track-video"));
}

#[test]
fn codex_remove_tracks_accepts_project_actions_shape() {
    let project = sample_project();
    let project_dir = tempfile::tempdir().expect("project dir");
    save_split_project(project_dir.path(), &project).expect("save split project");

    let result = call_codex_local_tool(
        &project,
        "video_creater.remove_tracks",
        json!({
            "projectDir": project_dir.path().display().to_string(),
            "actions": [
                {
                    "type": "removeTracks",
                    "trackIds": ["track-video"]
                }
            ]
        }),
    )
    .expect("remove tracks should accept projectDir + actions");

    assert!(result.mutates_project);
    assert_eq!(result.payload["applied"], json!(true));
    assert_eq!(result.payload["actionCount"], json!(1));
    assert_eq!(result.payload["agentHistory"]["entryCount"], json!(1));
    let persisted = load_split_project(project_dir.path()).expect("load removed tracks project");
    assert!(!persisted
        .timeline
        .tracks
        .iter()
        .any(|track| track.id == "track-video"));
}

#[test]
fn codex_remove_tracks_rejects_mixed_index_and_id_modes() {
    let project = sample_project();

    let error = call_codex_local_tool(
        &project,
        "video_creater.remove_tracks",
        json!({
            "trackIndexes": [0],
            "trackIds": ["track-video"]
        }),
    )
    .expect_err("mixed remove_tracks modes should reject");

    assert!(error
        .to_string()
        .contains("pass either trackIndexes or trackIds"));
}

#[test]
fn codex_search_media_rejects_unknown_fields() {
    let project = sample_project();

    let error = call_codex_local_tool(
        &project,
        "video_creater.search_media",
        json!({
            "query": "launch",
            "unbounded": true
        }),
    )
    .expect_err("search media should reject unknown fields");

    assert!(error.to_string().contains("unknown field `unbounded`"));
}

#[test]
fn codex_search_media_rejects_unknown_media_ref() {
    let project = sample_project();

    let error = call_codex_local_tool(
        &project,
        "search_media",
        json!({
            "query": "launch",
            "scope": "spoken",
            "mediaRef": "missing-media"
        }),
    )
    .expect_err("Palmier search_media mediaRef should resolve to a known asset");

    assert!(error.to_string().contains("mediaRef was not found"));
}

#[test]
fn codex_direct_tools_reject_unknown_fields_on_strict_schemas() {
    let project = sample_project();
    let cases = [
        (
            "video_creater.inspect_media",
            json!({
                "mediaRef": "media-1",
                "includeSecrets": true
            }),
            "includeSecrets",
        ),
        (
            "video_creater.transcript_words",
            json!({
                "mediaId": "media-1",
                "includeRaw": true
            }),
            "includeRaw",
        ),
        (
            "video_creater.generate_video",
            json!({
                "projectDir": "/tmp/video-creater-project",
                "assetId": "agent-video-unknown-fields",
                "jobId": "job-agent-video-unknown-fields",
                "mockMode": true,
                "prompt": "generate a clean product shot",
                "providerToken": "must-not-be-accepted"
            }),
            "providerToken",
        ),
        (
            "video_creater.export_project",
            json!({
                "projectDir": "/tmp/video-creater-project",
                "jobId": "export-unknown-fields",
                "profile": "draftWebm",
                "shell": "must-not-be-accepted"
            }),
            "shell",
        ),
    ];

    for (tool_name, args, rejected_field) in cases {
        let error = match call_codex_local_tool(&project, tool_name, args) {
            Ok(_) => panic!("{tool_name} should reject unknown fields"),
            Err(error) => error,
        };
        assert!(
            error
                .to_string()
                .contains(&format!("unknown field `{rejected_field}`")),
            "{tool_name} returned unexpected error: {error}"
        );
    }
}

#[test]
fn codex_simple_direct_tools_reject_unknown_fields() {
    let project = sample_project();
    let cases = [
        (
            "video_creater.project_context",
            json!({
                "projectDir": "/tmp/video-creater-project",
                "shell": true
            }),
            "shell",
        ),
        (
            "video_creater.inspect_timeline",
            json!({
                "startSeconds": 0,
                "includeHiddenTracks": true
            }),
            "includeHiddenTracks",
        ),
        (
            "video_creater.generation_defaults",
            json!({
                "prompt": "launch shot",
                "apiKey": "must-not-be-accepted"
            }),
            "apiKey",
        ),
        (
            "video_creater.transcription_readiness",
            json!({
                "mediaId": "media-1",
                "forceDownload": true
            }),
            "forceDownload",
        ),
        (
            "video_creater.build_export_media_start_request",
            json!({
                "projectDir": "/tmp/video-creater-project",
                "jobId": "export-media-unknown",
                "profile": "mp4H264",
                "outputPath": "exports/final.mp4",
                "command": "ffmpeg"
            }),
            "command",
        ),
        (
            "video_creater.build_export_nle_xml_start_request",
            json!({
                "projectDir": "/tmp/video-creater-project",
                "jobId": "export-xml-unknown",
                "format": "premiereXmeml",
                "outputPath": "exports/timeline.xml",
                "command": "xmllint"
            }),
            "command",
        ),
        (
            "video_creater.validate_project_actions",
            json!({
                "actions": [],
                "apply": true
            }),
            "apply",
        ),
        (
            "video_creater.remove_clips",
            json!({
                "itemIds": ["item-1"],
                "deleteMedia": true
            }),
            "deleteMedia",
        ),
        (
            "video_creater.import_media",
            json!({
                "projectDir": "/tmp/video-creater-project",
                "sourcePaths": ["/tmp/source.mp4"],
                "copyMode": "link"
            }),
            "copyMode",
        ),
    ];

    for (tool_name, args, rejected_field) in cases {
        let error = match call_codex_local_tool(&project, tool_name, args) {
            Ok(_) => panic!("{tool_name} should reject unknown fields"),
            Err(error) => error,
        };
        assert!(
            error
                .to_string()
                .contains(&format!("unknown field `{rejected_field}`")),
            "{tool_name} returned unexpected error: {error}"
        );
    }
}

#[test]
fn codex_direct_folder_tools_accept_palmier_style_arguments() {
    let project = sample_project();

    let create = call_codex_local_tool(
        &project,
        "video_creater.create_folder",
        json!({
            "name": "Agent selects"
        }),
    )
    .expect("create folder");
    assert!(create.mutates_project);
    assert_eq!(create.payload["valid"], json!(true));
    assert_eq!(
        create.payload["affectedFolderIds"],
        json!(["folder-agent-selects"])
    );
    assert_eq!(
        create.payload["folders"],
        json!([
            {
                "id": "folder-agent-selects",
                "folderId": "folder-agent-selects",
                "name": "Agent selects",
                "parentFolderId": null
            }
        ])
    );

    let batch_create = call_codex_local_tool(
        &project,
        "video_creater.create_folder",
        json!({
            "entries": [
                { "name": "Agent selects" },
                { "name": "Agent selects" }
            ]
        }),
    )
    .expect("batch create folders");
    assert!(batch_create.mutates_project);
    assert_eq!(batch_create.payload["valid"], json!(true));
    assert_eq!(
        batch_create.payload["affectedFolderIds"],
        json!(["folder-agent-selects", "folder-agent-selects-2"])
    );

    let project_dir = tempfile::tempdir().expect("project dir");
    save_split_project(project_dir.path(), &project).expect("save split project");
    let persisted_create = call_codex_local_tool(
        &project,
        "video_creater.create_folder",
        json!({
            "projectDir": project_dir.path().display().to_string(),
            "name": "Agent selects"
        }),
    )
    .expect("create folder should persist");
    assert!(persisted_create.mutates_project);
    assert_eq!(persisted_create.payload["applied"], json!(true));
    assert_eq!(
        persisted_create.payload["agentHistory"]["entryCount"],
        json!(1)
    );
    let persisted_project = load_split_project(project_dir.path()).expect("load folder project");
    assert!(persisted_project
        .media_folders
        .iter()
        .any(|folder| folder.id == "folder-agent-selects"));

    let mut project_with_folder = project.clone();
    project_with_folder.media_folders.push(MediaFolder {
        id: "agent-folder".to_string(),
        name: "Agent selects".to_string(),
        parent_id: None,
    });

    let move_to_folder = call_codex_local_tool(
        &project_with_folder,
        "video_creater.move_to_folder",
        json!({
            "assetIds": ["media-1"],
            "folderId": "agent-folder"
        }),
    )
    .expect("move media to folder");
    assert!(move_to_folder.mutates_project);
    assert_eq!(move_to_folder.payload["valid"], json!(true));
    assert_eq!(
        move_to_folder.payload["changedMediaIds"],
        json!(["media-1"])
    );
    assert_eq!(
        move_to_folder.payload["targetFolderId"],
        json!("agent-folder")
    );

    let move_project_dir = tempfile::tempdir().expect("move project dir");
    save_split_project(move_project_dir.path(), &project_with_folder)
        .expect("save move split project");
    let persisted_move = call_codex_local_tool(
        &project_with_folder,
        "video_creater.move_to_folder",
        json!({
            "projectDir": move_project_dir.path().display().to_string(),
            "assetIds": ["media-1"],
            "folderId": "agent-folder"
        }),
    )
    .expect("move media to folder should persist");
    assert!(persisted_move.mutates_project);
    assert_eq!(persisted_move.payload["applied"], json!(true));
    assert_eq!(
        persisted_move.payload["agentHistory"]["entryCount"],
        json!(1)
    );
    let persisted_move_project =
        load_split_project(move_project_dir.path()).expect("load moved project");
    assert_eq!(
        persisted_move_project
            .media
            .iter()
            .find(|media| media.id == "media-1")
            .and_then(|media| media.folder_id.as_deref()),
        Some("agent-folder")
    );

    let mut project_with_media_and_folders = project_with_folder.clone();
    project_with_media_and_folders.media.push(MediaAsset {
        id: "media-2".to_string(),
        name: Some("Cutaway".to_string()),
        relative_path: "media/cutaway.mp4".to_string(),
        kind: MediaKind::Video,
        duration_seconds: 8.0,
        width: Some(1920),
        height: Some(1080),
        fps: Some(24.0),
        folder_id: None,
    });
    project_with_media_and_folders
        .media_folders
        .push(MediaFolder {
            id: "b-roll".to_string(),
            name: "B-roll".to_string(),
            parent_id: None,
        });

    let batch_move = call_codex_local_tool(
        &project_with_media_and_folders,
        "video_creater.move_to_folder",
        json!({
            "entries": [
                { "assetIds": ["media-1"], "folderId": "agent-folder" },
                { "assetIds": ["media-2"], "folderId": "b-roll" }
            ]
        }),
    )
    .expect("batch move media to folders");
    assert!(batch_move.mutates_project);
    assert_eq!(batch_move.payload["valid"], json!(true));
    assert_eq!(
        batch_move.payload["changedMediaIds"],
        json!(["media-1", "media-2"])
    );
    assert_eq!(
        batch_move.payload["moves"],
        json!([
            { "assetIds": ["media-1"], "folderId": "agent-folder" },
            { "assetIds": ["media-2"], "folderId": "b-roll" }
        ])
    );

    let rename = call_codex_local_tool(
        &project_with_folder,
        "video_creater.rename_folder",
        json!({
            "folderId": "agent-folder",
            "name": "Selected takes"
        }),
    )
    .expect("rename folder");
    assert!(rename.mutates_project);
    assert_eq!(rename.payload["valid"], json!(true));
    assert_eq!(rename.payload["affectedFolderIds"], json!(["agent-folder"]));

    let rename_project_dir = tempfile::tempdir().expect("rename project dir");
    save_split_project(rename_project_dir.path(), &project_with_folder)
        .expect("save rename split project");
    let persisted_rename = call_codex_local_tool(
        &project_with_folder,
        "video_creater.rename_folder",
        json!({
            "projectDir": rename_project_dir.path().display().to_string(),
            "folderId": "agent-folder",
            "name": "Selected takes"
        }),
    )
    .expect("rename folder should persist");
    assert!(persisted_rename.mutates_project);
    assert_eq!(persisted_rename.payload["applied"], json!(true));
    assert_eq!(
        persisted_rename.payload["agentHistory"]["entryCount"],
        json!(1)
    );
    let persisted_rename_project =
        load_split_project(rename_project_dir.path()).expect("load renamed project");
    assert_eq!(
        persisted_rename_project
            .media_folders
            .iter()
            .find(|folder| folder.id == "agent-folder")
            .map(|folder| folder.name.as_str()),
        Some("Selected takes")
    );

    let batch_rename = call_codex_local_tool(
        &project_with_media_and_folders,
        "video_creater.rename_folder",
        json!({
            "entries": [
                { "folderId": "agent-folder", "name": "Selected takes" },
                { "folderId": "b-roll", "name": "Cutaways" }
            ]
        }),
    )
    .expect("batch rename folders");
    assert!(batch_rename.mutates_project);
    assert_eq!(batch_rename.payload["valid"], json!(true));
    assert_eq!(
        batch_rename.payload["affectedFolderIds"],
        json!(["agent-folder", "b-roll"])
    );
    assert_eq!(
        batch_rename.payload["renamedFolders"],
        json!([
            { "folderId": "agent-folder", "name": "Selected takes" },
            { "folderId": "b-roll", "name": "Cutaways" }
        ])
    );

    let delete = call_codex_local_tool(
        &project_with_folder,
        "video_creater.delete_folder",
        json!({ "folderIds": ["agent-folder"] }),
    )
    .expect("delete folder");
    assert!(delete.mutates_project);
    assert_eq!(delete.payload["valid"], json!(true));
    assert_eq!(delete.payload["affectedFolderIds"], json!(["agent-folder"]));

    let delete_project_dir = tempfile::tempdir().expect("delete project dir");
    save_split_project(delete_project_dir.path(), &project_with_folder)
        .expect("save delete split project");
    let persisted_delete = call_codex_local_tool(
        &project_with_folder,
        "video_creater.delete_folder",
        json!({
            "projectDir": delete_project_dir.path().display().to_string(),
            "folderIds": ["agent-folder"]
        }),
    )
    .expect("delete folder should persist");
    assert!(persisted_delete.mutates_project);
    assert_eq!(persisted_delete.payload["applied"], json!(true));
    assert_eq!(
        persisted_delete.payload["agentHistory"]["entryCount"],
        json!(1)
    );
    let persisted_delete_project =
        load_split_project(delete_project_dir.path()).expect("load delete project");
    assert!(!persisted_delete_project
        .media_folders
        .iter()
        .any(|folder| folder.id == "agent-folder"));
}

#[test]
fn codex_create_folder_direct_result_returns_palmier_top_level_id() {
    let project = sample_project();

    let create = call_codex_local_tool(
        &project,
        "create_folder",
        json!({
            "name": "Agent selects"
        }),
    )
    .expect("create folder");

    assert!(create.mutates_project);
    assert_eq!(create.payload["id"], json!("folder-agent-selects"));
    assert_eq!(create.payload["name"], json!("Agent selects"));
    assert_eq!(create.payload["parentFolderId"], Value::Null);
}

#[test]
fn codex_color_and_effect_tools_accept_palmier_style_arguments() {
    let project = sample_project();

    let effect = call_codex_local_tool(
        &project,
        "video_creater.apply_effect",
        json!({
            "clipIds": ["item-1"],
            "effects": [
                {
                    "type": "stylize.glow",
                    "params": { "intensity": 0.7 },
                    "enabled": true
                },
                {
                    "type": "blur.motion",
                    "params": { "radius": 240, "angle": -240 },
                    "enabled": true
                }
            ]
        }),
    )
    .expect("apply effect");
    assert!(effect.mutates_project);
    assert_eq!(effect.payload["valid"], json!(true));
    assert_eq!(effect.payload["changedItemIds"], json!(["item-1"]));
    assert_eq!(
        effect.payload["effects"][0]["effectType"],
        json!("blur.motion")
    );
    assert_eq!(
        effect.payload["effects"][0]["params"]["radius"],
        json!(100.0)
    );
    assert_eq!(
        effect.payload["effects"][0]["params"]["angle"],
        json!(-180.0)
    );
    assert_eq!(
        effect.payload["effects"][1]["effectType"],
        json!("stylize.glow")
    );

    let effect_project_dir = tempfile::tempdir().expect("effect project dir");
    save_split_project(effect_project_dir.path(), &project).expect("save effect project");
    let persisted_effect = call_codex_local_tool(
        &project,
        "video_creater.apply_effect",
        json!({
            "projectDir": effect_project_dir.path().display().to_string(),
            "clipIds": ["item-1"],
            "effects": [
                {
                    "type": "stylize.glow",
                    "params": { "intensity": 0.7 },
                    "enabled": true
                }
            ]
        }),
    )
    .expect("apply effect should persist");
    assert!(persisted_effect.mutates_project);
    assert_eq!(persisted_effect.payload["applied"], json!(true));
    assert_eq!(
        persisted_effect.payload["agentHistory"]["entryCount"],
        json!(1)
    );
    let persisted_effect_project =
        load_split_project(effect_project_dir.path()).expect("load effect project");
    let persisted_effect_item = persisted_effect_project
        .timeline
        .tracks
        .iter()
        .flat_map(|track| track.items.iter())
        .find(|item| item.id == "item-1")
        .expect("persisted effect item");
    assert_eq!(
        persisted_effect_item.properties["effects"][0]["effectType"],
        json!("stylize.glow")
    );

    let color = call_codex_local_tool(
        &project,
        "video_creater.apply_color",
        json!({
            "clipIds": ["item-1"],
            "exposure": 0.2,
            "contrast": 1.15,
            "temperature": 7200
        }),
    )
    .expect("apply color");
    assert!(color.mutates_project);
    assert_eq!(color.payload["valid"], json!(true));
    assert_eq!(color.payload["changedItemIds"], json!(["item-1"]));
    assert_eq!(color.payload["grade"]["exposure"], json!(0.2));
    assert_eq!(color.payload["grade"]["contrast"], json!(1.15));
    assert_eq!(color.payload["grade"]["temperature"], json!(7200.0));

    let color_project_dir = tempfile::tempdir().expect("color project dir");
    save_split_project(color_project_dir.path(), &project).expect("save color project");
    let persisted_color = call_codex_local_tool(
        &project,
        "video_creater.apply_color",
        json!({
            "projectDir": color_project_dir.path().display().to_string(),
            "clipIds": ["item-1"],
            "exposure": 0.2,
            "contrast": 1.15
        }),
    )
    .expect("apply color should persist");
    assert!(persisted_color.mutates_project);
    assert_eq!(persisted_color.payload["applied"], json!(true));
    assert_eq!(
        persisted_color.payload["agentHistory"]["entryCount"],
        json!(1)
    );
    let persisted_color_project =
        load_split_project(color_project_dir.path()).expect("load color project");
    let persisted_color_item = persisted_color_project
        .timeline
        .tracks
        .iter()
        .flat_map(|track| track.items.iter())
        .find(|item| item.id == "item-1")
        .expect("persisted color item");
    assert_eq!(
        persisted_color_item.properties["colorGrade"]["exposure"],
        json!(0.2)
    );
    assert_eq!(
        persisted_color_item.properties["colorGrade"]["contrast"],
        json!(1.15)
    );
}

#[test]
fn codex_apply_effect_preserves_each_selected_clip_stack() {
    let mut project = sample_project();
    let mut second_item = project.timeline.tracks[0].items[0].clone();
    second_item.id = "item-2".to_string();
    second_item.start_seconds = 4.0;
    second_item.properties.insert(
        "effects".to_string(),
        json!([
            {
                "effectType": "blur.motion",
                "enabled": true,
                "params": { "radius": 12.0, "angle": 45.0 }
            }
        ]),
    );
    project.timeline.tracks[0].items.push(second_item);
    project.timeline.tracks[0].items[0].properties.insert(
        "effects".to_string(),
        json!([
            {
                "effectInstanceId": "legacy:stylize.grain:1",
                "effectType": "stylize.grain",
                "enabled": true,
                "params": { "amount": 0.2, "size": 1.5 }
            }
        ]),
    );
    let project_dir = tempfile::tempdir().expect("multi-effect project dir");
    save_split_project(project_dir.path(), &project).expect("save multi-effect project");

    let result = call_codex_local_tool(
        &project,
        "video_creater.apply_effect",
        json!({
            "projectDir": project_dir.path().display().to_string(),
            "clipIds": ["item-1", "item-2"],
            "effects": [
                {
                    "type": "stylize.glow",
                    "params": { "intensity": 0.7 },
                    "enabled": true
                }
            ]
        }),
    )
    .expect("multi-clip effect update should preserve per-clip stacks");

    assert!(result.mutates_project);
    assert_eq!(result.payload["applied"], json!(true));
    assert_eq!(result.payload["actionCount"], json!(2));
    assert_eq!(
        result.payload["changedItemIds"],
        json!(["item-1", "item-2"])
    );

    let persisted_project =
        load_split_project(project_dir.path()).expect("load multi-effect project");
    let persisted_items = persisted_project
        .timeline
        .tracks
        .iter()
        .flat_map(|track| track.items.iter())
        .map(|item| (item.id.as_str(), item))
        .collect::<BTreeMap<_, _>>();
    let first_effects = &persisted_items["item-1"].properties["effects"];
    let second_effects = &persisted_items["item-2"].properties["effects"];
    assert_eq!(
        first_effects,
        &json!([
            {
                "effectInstanceId": "legacy:stylize.grain:1",
                "effectType": "stylize.grain",
                "enabled": true,
                "params": { "amount": 0.2, "size": 1.5 }
            },
            {
                "effectInstanceId": "legacy:stylize.glow:1",
                "effectType": "stylize.glow",
                "enabled": true,
                "params": { "intensity": 0.7 }
            }
        ])
    );
    assert_eq!(
        second_effects,
        &json!([
            {
                "effectInstanceId": "legacy:blur.motion:1",
                "effectType": "blur.motion",
                "enabled": true,
                "params": { "angle": 45.0, "radius": 12.0 }
            },
            {
                "effectInstanceId": "legacy:stylize.glow:1",
                "effectType": "stylize.glow",
                "enabled": true,
                "params": { "intensity": 0.7 }
            }
        ])
    );
}

#[test]
fn codex_apply_color_accepts_full_palmier_grade_stack() {
    let project = sample_project();

    let color = call_codex_local_tool(
        &project,
        "video_creater.apply_color",
        json!({
            "clipIds": ["item-1"],
            "vibrance": 0.25,
            "highlights": -0.4,
            "shadows": 0.35,
            "blacks": -0.2,
            "whites": 0.1,
            "shadowsHue": 180,
            "shadowsAmount": 0.15,
            "shadowsLum": 0.05,
            "midsHue": 32,
            "midsAmount": 0.1,
            "midsGamma": 1.08,
            "highsHue": 48,
            "highsAmount": 0.07,
            "highsGain": 1.04,
            "masterCurve": [[0, 0.04], [1, 0.96]],
            "blueCurve": [[0, 0], [0.7, 0.7], [1, 0.85]],
            "hueCurves": {
                "targets": [
                    {
                        "targetHue": 120,
                        "hueShift": -6,
                        "satScale": 0.8,
                        "lumShift": -0.05
                    }
                ]
            },
            "lut": {
                "path": "luts/cinematic.cube",
                "strength": 0.6
            }
        }),
    )
    .expect("full Palmier color grade");

    assert!(color.mutates_project);
    assert_eq!(color.payload["valid"], json!(true));
    assert_eq!(color.payload["changedItemIds"], json!(["item-1"]));
    assert_eq!(color.payload["grade"]["vibrance"], json!(0.25));
    assert_eq!(color.payload["grade"]["highlights"], json!(-0.4));
    assert_eq!(color.payload["grade"]["shadows"], json!(0.35));
    assert_eq!(color.payload["grade"]["blacks"], json!(-0.2));
    assert_eq!(color.payload["grade"]["whites"], json!(0.1));
    assert_eq!(color.payload["grade"]["shadowsHue"], json!(180));
    assert_eq!(color.payload["grade"]["midsGamma"], json!(1.08));
    assert_eq!(color.payload["grade"]["highsGain"], json!(1.04));
    assert_eq!(
        color.payload["grade"]["masterCurve"],
        json!([[0, 0.04], [1, 0.96]])
    );
    assert_eq!(
        color.payload["grade"]["blueCurve"],
        json!([[0, 0], [0.7, 0.7], [1, 0.85]])
    );
    assert_eq!(
        color.payload["grade"]["hueCurves"]["targets"][0]["targetHue"],
        json!(120)
    );
    assert_eq!(color.payload["grade"]["lut"]["strength"], json!(0.6));
    assert_eq!(
        color.payload["grade"]["lut"]["path"],
        json!("luts/cinematic.cube")
    );
}

#[test]
fn codex_apply_color_rejects_unknown_palmier_grade_fields() {
    let project = sample_project();

    let error = call_codex_local_tool(
        &project,
        "video_creater.apply_color",
        json!({
            "clipIds": ["item-1"],
            "cinematicMood": 0.75
        }),
    )
    .expect_err("unknown Palmier color grade fields should be rejected");

    assert!(
        matches!(
            error,
            CodexLocalToolError::ProjectActionValidation(ref message)
                if message.contains("unknown color grade control cinematicMood")
        ),
        "unexpected error for unknown color grade field: {error:?}"
    );
}

#[test]
fn codex_denoise_audio_persists_palmier_audio_denoise_effect() {
    let mut project = sample_project();
    project.timeline.tracks[0].items.push(TimelineItem {
        id: "item-audio".to_string(),
        kind: TimelineItemKind::AudioClip,
        start_seconds: 0.0,
        duration_seconds: 4.0,
        source: TimelineSource::Media {
            media_id: "media-1".to_string(),
        },
        label: "Audio".to_string(),
        properties: BTreeMap::new(),
    });
    let project_dir = tempfile::tempdir().expect("denoise project dir");
    save_split_project(project_dir.path(), &project).expect("save denoise project");

    let enabled = call_codex_local_tool(
        &project,
        "denoise_audio",
        json!({
            "projectDir": project_dir.path().display().to_string(),
            "clipIds": ["item-audio"],
            "strength": 42
        }),
    )
    .expect("denoise audio should persist");

    assert!(enabled.mutates_project);
    assert_eq!(enabled.payload["applied"], json!(true));
    assert_eq!(enabled.payload["changedItemIds"], json!(["item-audio"]));
    assert_eq!(
        enabled.payload["effects"][0]["effectType"],
        json!("audio.denoise")
    );
    assert_eq!(enabled.payload["effects"][0]["enabled"], json!(true));
    assert_eq!(
        enabled.payload["effects"][0]["params"]["amount"],
        json!(0.42)
    );

    let enabled_project = load_split_project(project_dir.path()).expect("load enabled project");
    let enabled_item = enabled_project
        .timeline
        .tracks
        .iter()
        .flat_map(|track| track.items.iter())
        .find(|item| item.id == "item-audio")
        .expect("enabled audio item");
    assert_eq!(
        enabled_item.properties["effects"][0]["effectType"],
        json!("audio.denoise")
    );
    assert_eq!(
        enabled_item.properties["effects"][0]["enabled"],
        json!(true)
    );
    assert_eq!(
        enabled_item.properties["effects"][0]["params"]["amount"],
        json!(0.42)
    );

    let disabled = call_codex_local_tool(
        &enabled_project,
        "video_creater.denoise_audio",
        json!({
            "projectDir": project_dir.path().display().to_string(),
            "clipIds": ["item-audio"],
            "enabled": false
        }),
    )
    .expect("disable denoise should persist");

    assert!(disabled.mutates_project);
    assert_eq!(
        disabled.payload["effects"][0]["effectType"],
        json!("audio.denoise")
    );
    assert_eq!(disabled.payload["effects"][0]["enabled"], json!(false));
    assert_eq!(
        disabled.payload["effects"][0]["params"]["amount"],
        json!(0.42)
    );

    let disabled_project = load_split_project(project_dir.path()).expect("load disabled project");
    let disabled_item = disabled_project
        .timeline
        .tracks
        .iter()
        .flat_map(|track| track.items.iter())
        .find(|item| item.id == "item-audio")
        .expect("disabled audio item");
    assert_eq!(
        disabled_item.properties["effects"][0]["enabled"],
        json!(false)
    );
}

#[test]
fn codex_denoise_audio_rejects_non_audio_clips_and_invalid_strength() {
    let project = sample_project();

    let video_clip = call_codex_local_tool(
        &project,
        "denoise_audio",
        json!({
            "clipIds": ["item-1"],
            "strength": 50
        }),
    )
    .expect_err("video clip should fail");
    assert!(video_clip
        .to_string()
        .contains("Clip item-1 is a video_clip clip; denoise_audio needs an audio clip"));

    let strength = call_codex_local_tool(
        &project,
        "denoise_audio",
        json!({
            "clipIds": ["item-1"],
            "strength": 120
        }),
    )
    .expect_err("out of range strength should fail");
    assert!(strength
        .to_string()
        .contains("strength must be 0-100 (got 120)"));
}

#[test]
fn codex_apply_effect_rejects_unknown_effects_and_params() {
    let project = sample_project();

    let unknown_effect = call_codex_local_tool(
        &project,
        "video_creater.apply_effect",
        json!({
            "clipIds": ["item-1"],
            "effects": [{ "type": "future.hologram", "params": { "wobble": 1.0 } }]
        }),
    )
    .expect_err("unknown effect should fail");
    assert!(unknown_effect
        .to_string()
        .contains("unknown effect `future.hologram`"));

    let unknown_param = call_codex_local_tool(
        &project,
        "video_creater.apply_effect",
        json!({
            "clipIds": ["item-1"],
            "effects": [{ "type": "stylize.glow", "params": { "unknown": 1.0 } }]
        }),
    )
    .expect_err("unknown param should fail");
    assert!(unknown_param
        .to_string()
        .contains("stylize.glow unknown effect parameter `unknown`"));

    let color_effect = call_codex_local_tool(
        &project,
        "video_creater.apply_effect",
        json!({
            "clipIds": ["item-1"],
            "effects": [{ "type": "color.exposure", "params": { "ev": 1.0 } }]
        }),
    )
    .expect_err("color effect should fail");
    assert!(color_effect
        .to_string()
        .contains("color.exposure is a color effect; use video_creater.apply_color"));

    let audio_effect = call_codex_local_tool(
        &project,
        "video_creater.apply_effect",
        json!({
            "clipIds": ["item-1"],
            "effects": [{ "type": "audio.denoise", "params": { "amount": 0.5 } }]
        }),
    )
    .expect_err("audio denoise should use the dedicated tool");
    assert!(audio_effect
        .to_string()
        .contains("audio.denoise is an audio effect; use video_creater.denoise_audio"));
}

#[test]
fn codex_inspect_color_returns_stored_grade_and_effects() {
    let mut project = sample_project();
    let item = project.timeline.tracks[0]
        .items
        .iter_mut()
        .find(|item| item.id == "item-1")
        .expect("item");
    item.properties.insert(
        "colorGrade".to_string(),
        json!({
            "exposure": 0.2,
            "contrast": 1.15
        }),
    );
    item.properties.insert(
        "effects".to_string(),
        json!([
            {
                "effectType": "stylize.glow",
                "enabled": true,
                "params": { "intensity": 0.7 }
            }
        ]),
    );

    let result = call_codex_local_tool(
        &project,
        "video_creater.inspect_color",
        json!({ "clipId": "item-1" }),
    )
    .expect("inspect color");

    assert!(!result.mutates_project);
    assert_eq!(result.payload["target"]["clipId"], json!("item-1"));
    assert_eq!(result.payload["grade"]["exposure"], json!(0.2));
    assert_eq!(
        result.payload["effects"][0]["effectType"],
        json!("stylize.glow")
    );
}

#[test]
fn codex_inspect_color_measures_image_media_scopes() {
    let project_dir = tempfile::tempdir().expect("project dir");
    let media_dir = project_dir.path().join("media");
    fs::create_dir_all(&media_dir).expect("media dir");
    let image_path = media_dir.join("scope.png");
    let mut image = image::RgbaImage::new(2, 1);
    image.put_pixel(0, 0, image::Rgba([255, 0, 0, 255]));
    image.put_pixel(1, 0, image::Rgba([0, 255, 0, 255]));
    image.save(&image_path).expect("write test image");

    let mut project = sample_project();
    project.media.push(MediaAsset {
        id: "scope-image".to_string(),
        name: Some("Scope image".to_string()),
        relative_path: "media/scope.png".to_string(),
        kind: MediaKind::Image,
        duration_seconds: 0.0,
        width: Some(2),
        height: Some(1),
        fps: None,
        folder_id: None,
    });

    let result = call_codex_local_tool(
        &project,
        "video_creater.inspect_color",
        json!({
            "projectDir": project_dir.path().display().to_string(),
            "mediaRef": "scope-image"
        }),
    )
    .expect("inspect image color scopes");

    assert_eq!(
        result.payload["scopeStatus"],
        json!("measured_image_scopes")
    );
    assert_eq!(result.payload["media"]["meanRGB"], json!([0.5, 0.5, 0.0]));
    assert_eq!(result.payload["media"]["luma"]["black"], json!(0.213));
    assert_eq!(result.payload["media"]["luma"]["white"], json!(0.715));
    assert_eq!(result.payload["media"]["luma"]["mean"], json!(0.464));
    assert_eq!(
        result.payload["media"]["luma"]["histogram16"]
            .as_array()
            .expect("histogram16")
            .len(),
        16
    );
    assert_eq!(
        result.payload["media"]["hueHistogram12"]
            .as_array()
            .expect("hue histogram")
            .len(),
        12
    );
}

#[test]
fn codex_inspect_color_measures_image_reference_gap() {
    let project_dir = tempfile::tempdir().expect("project dir");
    let media_dir = project_dir.path().join("media");
    fs::create_dir_all(&media_dir).expect("media dir");
    image::RgbaImage::from_pixel(1, 1, image::Rgba([255, 0, 0, 255]))
        .save(media_dir.join("subject.png"))
        .expect("write subject image");
    image::RgbaImage::from_pixel(1, 1, image::Rgba([0, 0, 255, 255]))
        .save(media_dir.join("reference.png"))
        .expect("write reference image");

    let mut project = sample_project();
    project.media.push(MediaAsset {
        id: "subject-image".to_string(),
        name: Some("Subject image".to_string()),
        relative_path: "media/subject.png".to_string(),
        kind: MediaKind::Image,
        duration_seconds: 0.0,
        width: Some(1),
        height: Some(1),
        fps: None,
        folder_id: None,
    });
    project.media.push(MediaAsset {
        id: "reference-image".to_string(),
        name: Some("Reference image".to_string()),
        relative_path: "media/reference.png".to_string(),
        kind: MediaKind::Image,
        duration_seconds: 0.0,
        width: Some(1),
        height: Some(1),
        fps: None,
        folder_id: None,
    });

    let result = call_codex_local_tool(
        &project,
        "video_creater.inspect_color",
        json!({
            "projectDir": project_dir.path().display().to_string(),
            "mediaRef": "subject-image",
            "reference": "reference-image"
        }),
    )
    .expect("inspect image reference gap");

    assert_eq!(
        result.payload["scopeStatus"],
        json!("measured_image_scopes")
    );
    assert_eq!(
        result.payload["reference"]["meanRGB"],
        json!([0.0, 0.0, 1.0])
    );
    assert_eq!(result.payload["gap"]["lumaMean"], json!(0.141));
    assert_eq!(result.payload["gap"]["warmCool"], json!(2.0));
    assert_eq!(result.payload["gap"]["saturation"], json!(0.0));
    assert!(result.payload["gap"]["hints"]
        .as_array()
        .expect("gap hints")
        .iter()
        .any(|hint| hint.as_str().is_some_and(|hint| hint.contains("warmer"))));
}

#[test]
fn codex_inspect_color_measures_image_backed_clip_scopes() {
    let project_dir = tempfile::tempdir().expect("project dir");
    let media_dir = project_dir.path().join("media");
    fs::create_dir_all(&media_dir).expect("media dir");
    image::RgbaImage::from_pixel(1, 1, image::Rgba([0, 255, 0, 255]))
        .save(media_dir.join("clip-source.png"))
        .expect("write clip source image");

    let mut project = sample_project();
    project.media.push(MediaAsset {
        id: "clip-image".to_string(),
        name: Some("Clip image".to_string()),
        relative_path: "media/clip-source.png".to_string(),
        kind: MediaKind::Image,
        duration_seconds: 0.0,
        width: Some(1),
        height: Some(1),
        fps: None,
        folder_id: None,
    });
    project.timeline.tracks[0].items[0] = TimelineItem {
        id: "image-clip-1".to_string(),
        kind: TimelineItemKind::VideoClip,
        start_seconds: 0.0,
        duration_seconds: 1.0,
        source: TimelineSource::Media {
            media_id: "clip-image".to_string(),
        },
        label: "Image clip".to_string(),
        properties: BTreeMap::from([("opacity".to_string(), json!(0.75))]),
    };

    let result = call_codex_local_tool(
        &project,
        "video_creater.inspect_color",
        json!({
            "projectDir": project_dir.path().display().to_string(),
            "clipId": "image-clip-1"
        }),
    )
    .expect("inspect image-backed clip scopes");

    assert_eq!(
        result.payload["scopeStatus"],
        json!("measured_image_clip_scopes")
    );
    assert_eq!(result.payload["target"]["clipId"], json!("image-clip-1"));
    assert_eq!(result.payload["target"]["mediaId"], json!("clip-image"));
    assert_eq!(result.payload["clip"]["meanRGB"], json!([0.0, 1.0, 0.0]));
    assert_eq!(result.payload["clip"]["luma"]["mean"], json!(0.715));
    assert_eq!(result.payload["grade"], json!({}));
    assert_eq!(result.payload["effects"], json!([]));
}

#[test]
fn codex_inspect_color_measures_image_clip_after_color_grade() {
    let project_dir = tempfile::tempdir().expect("project dir");
    let media_dir = project_dir.path().join("media");
    fs::create_dir_all(&media_dir).expect("media dir");
    image::RgbaImage::from_pixel(1, 1, image::Rgba([64, 64, 64, 255]))
        .save(media_dir.join("graded-source.png"))
        .expect("write graded source image");

    let mut project = sample_project();
    project.media.push(MediaAsset {
        id: "graded-image".to_string(),
        name: Some("Graded image".to_string()),
        relative_path: "media/graded-source.png".to_string(),
        kind: MediaKind::Image,
        duration_seconds: 0.0,
        width: Some(1),
        height: Some(1),
        fps: None,
        folder_id: None,
    });
    project.timeline.tracks[0].items[0] = TimelineItem {
        id: "graded-image-clip".to_string(),
        kind: TimelineItemKind::VideoClip,
        start_seconds: 0.0,
        duration_seconds: 1.0,
        source: TimelineSource::Media {
            media_id: "graded-image".to_string(),
        },
        label: "Graded image clip".to_string(),
        properties: BTreeMap::from([(
            "colorGrade".to_string(),
            json!({
                "exposure": 1.0,
                "contrast": 1.0,
                "saturation": 1.0
            }),
        )]),
    };

    let result = call_codex_local_tool(
        &project,
        "video_creater.inspect_color",
        json!({
            "projectDir": project_dir.path().display().to_string(),
            "clipId": "graded-image-clip"
        }),
    )
    .expect("inspect graded image clip scopes");

    assert_eq!(
        result.payload["scopeStatus"],
        json!("measured_image_clip_scopes")
    );
    assert_eq!(
        result.payload["clip"]["meanRGB"],
        json!([0.502, 0.502, 0.502])
    );
    assert_eq!(result.payload["clip"]["luma"]["mean"], json!(0.502));
    assert_eq!(result.payload["grade"]["exposure"], json!(1.0));
}

#[test]
fn codex_inspect_color_measures_image_clip_saturation_grade() {
    let project_dir = tempfile::tempdir().expect("project dir");
    let media_dir = project_dir.path().join("media");
    fs::create_dir_all(&media_dir).expect("media dir");
    image::RgbaImage::from_pixel(1, 1, image::Rgba([255, 0, 0, 255]))
        .save(media_dir.join("saturated-source.png"))
        .expect("write saturated source image");

    let mut project = sample_project();
    project.media.push(MediaAsset {
        id: "saturated-image".to_string(),
        name: Some("Saturated image".to_string()),
        relative_path: "media/saturated-source.png".to_string(),
        kind: MediaKind::Image,
        duration_seconds: 0.0,
        width: Some(1),
        height: Some(1),
        fps: None,
        folder_id: None,
    });
    project.timeline.tracks[0].items[0] = TimelineItem {
        id: "saturated-image-clip".to_string(),
        kind: TimelineItemKind::VideoClip,
        start_seconds: 0.0,
        duration_seconds: 1.0,
        source: TimelineSource::Media {
            media_id: "saturated-image".to_string(),
        },
        label: "Saturated image clip".to_string(),
        properties: BTreeMap::from([(
            "colorGrade".to_string(),
            json!({
                "saturation": 0.0
            }),
        )]),
    };

    let result = call_codex_local_tool(
        &project,
        "video_creater.inspect_color",
        json!({
            "projectDir": project_dir.path().display().to_string(),
            "clipId": "saturated-image-clip"
        }),
    )
    .expect("inspect saturation-graded image clip scopes");

    assert_eq!(
        result.payload["clip"]["meanRGB"],
        json!([0.212, 0.212, 0.212])
    );
    assert_eq!(result.payload["clip"]["saturation"], json!(0.0));
    assert_eq!(result.payload["grade"]["saturation"], json!(0.0));
}

#[test]
fn codex_inspect_color_measures_image_clip_levels_grade() {
    let project_dir = tempfile::tempdir().expect("project dir");
    let media_dir = project_dir.path().join("media");
    fs::create_dir_all(&media_dir).expect("media dir");
    image::RgbaImage::from_pixel(1, 1, image::Rgba([179, 179, 179, 255]))
        .save(media_dir.join("levels-source.png"))
        .expect("write levels source image");

    let mut project = sample_project();
    project.media.push(MediaAsset {
        id: "levels-image".to_string(),
        name: Some("Levels image".to_string()),
        relative_path: "media/levels-source.png".to_string(),
        kind: MediaKind::Image,
        duration_seconds: 0.0,
        width: Some(1),
        height: Some(1),
        fps: None,
        folder_id: None,
    });
    project.timeline.tracks[0].items[0] = TimelineItem {
        id: "levels-image-clip".to_string(),
        kind: TimelineItemKind::VideoClip,
        start_seconds: 0.0,
        duration_seconds: 1.0,
        source: TimelineSource::Media {
            media_id: "levels-image".to_string(),
        },
        label: "Levels image clip".to_string(),
        properties: BTreeMap::from([(
            "colorGrade".to_string(),
            json!({
                "blacks": 0.0,
                "whites": 1.0
            }),
        )]),
    };

    let result = call_codex_local_tool(
        &project,
        "video_creater.inspect_color",
        json!({
            "projectDir": project_dir.path().display().to_string(),
            "clipId": "levels-image-clip"
        }),
    )
    .expect("inspect levels-graded image clip scopes");

    assert_eq!(result.payload["clip"]["meanRGB"], json!([1.0, 1.0, 1.0]));
    assert_eq!(result.payload["clip"]["luma"]["mean"], json!(1.0));
    assert_eq!(result.payload["grade"]["whites"], json!(1.0));
}

#[test]
fn codex_inspect_color_measures_image_clip_highlights_grade() {
    let project_dir = tempfile::tempdir().expect("project dir");
    let media_dir = project_dir.path().join("media");
    fs::create_dir_all(&media_dir).expect("media dir");
    image::RgbaImage::from_pixel(1, 1, image::Rgba([217, 217, 217, 255]))
        .save(media_dir.join("highlights-source.png"))
        .expect("write highlights source image");

    let mut project = sample_project();
    project.media.push(MediaAsset {
        id: "highlights-image".to_string(),
        name: Some("Highlights image".to_string()),
        relative_path: "media/highlights-source.png".to_string(),
        kind: MediaKind::Image,
        duration_seconds: 0.0,
        width: Some(1),
        height: Some(1),
        fps: None,
        folder_id: None,
    });
    project.timeline.tracks[0].items[0] = TimelineItem {
        id: "highlights-image-clip".to_string(),
        kind: TimelineItemKind::VideoClip,
        start_seconds: 0.0,
        duration_seconds: 1.0,
        source: TimelineSource::Media {
            media_id: "highlights-image".to_string(),
        },
        label: "Highlights image clip".to_string(),
        properties: BTreeMap::from([(
            "colorGrade".to_string(),
            json!({
                "highlights": 1.0,
                "shadows": 0.0
            }),
        )]),
    };

    let result = call_codex_local_tool(
        &project,
        "video_creater.inspect_color",
        json!({
            "projectDir": project_dir.path().display().to_string(),
            "clipId": "highlights-image-clip"
        }),
    )
    .expect("inspect highlights-graded image clip scopes");

    assert_eq!(result.payload["clip"]["meanRGB"], json!([1.0, 1.0, 1.0]));
    assert_eq!(result.payload["clip"]["luma"]["mean"], json!(1.0));
    assert_eq!(result.payload["grade"]["highlights"], json!(1.0));
}

#[test]
fn codex_inspect_color_measures_image_clip_wheel_gain_grade() {
    let project_dir = tempfile::tempdir().expect("project dir");
    let media_dir = project_dir.path().join("media");
    fs::create_dir_all(&media_dir).expect("media dir");
    image::RgbaImage::from_pixel(1, 1, image::Rgba([200, 100, 50, 255]))
        .save(media_dir.join("wheel-gain-source.png"))
        .expect("write wheel gain source image");

    let mut project = sample_project();
    project.media.push(MediaAsset {
        id: "wheel-gain-image".to_string(),
        name: Some("Wheel gain image".to_string()),
        relative_path: "media/wheel-gain-source.png".to_string(),
        kind: MediaKind::Image,
        duration_seconds: 0.0,
        width: Some(1),
        height: Some(1),
        fps: None,
        folder_id: None,
    });
    project.timeline.tracks[0].items[0] = TimelineItem {
        id: "wheel-gain-image-clip".to_string(),
        kind: TimelineItemKind::VideoClip,
        start_seconds: 0.0,
        duration_seconds: 1.0,
        source: TimelineSource::Media {
            media_id: "wheel-gain-image".to_string(),
        },
        label: "Wheel gain image clip".to_string(),
        properties: BTreeMap::from([(
            "colorGrade".to_string(),
            json!({
                "highsGain": 0.5
            }),
        )]),
    };

    let result = call_codex_local_tool(
        &project,
        "video_creater.inspect_color",
        json!({
            "projectDir": project_dir.path().display().to_string(),
            "clipId": "wheel-gain-image-clip"
        }),
    )
    .expect("inspect wheel-gain-graded image clip scopes");

    assert_eq!(
        result.payload["clip"]["meanRGB"],
        json!([0.392, 0.196, 0.098])
    );
    assert_eq!(result.payload["grade"]["highsGain"], json!(0.5));
}

#[test]
fn codex_inspect_color_measures_image_clip_channel_curve_grade() {
    let project_dir = tempfile::tempdir().expect("project dir");
    let media_dir = project_dir.path().join("media");
    fs::create_dir_all(&media_dir).expect("media dir");
    image::RgbaImage::from_pixel(1, 1, image::Rgba([128, 128, 128, 255]))
        .save(media_dir.join("red-curve-source.png"))
        .expect("write red curve source image");

    let mut project = sample_project();
    project.media.push(MediaAsset {
        id: "red-curve-image".to_string(),
        name: Some("Red curve image".to_string()),
        relative_path: "media/red-curve-source.png".to_string(),
        kind: MediaKind::Image,
        duration_seconds: 0.0,
        width: Some(1),
        height: Some(1),
        fps: None,
        folder_id: None,
    });
    project.timeline.tracks[0].items[0] = TimelineItem {
        id: "red-curve-image-clip".to_string(),
        kind: TimelineItemKind::VideoClip,
        start_seconds: 0.0,
        duration_seconds: 1.0,
        source: TimelineSource::Media {
            media_id: "red-curve-image".to_string(),
        },
        label: "Red curve image clip".to_string(),
        properties: BTreeMap::from([(
            "colorGrade".to_string(),
            json!({
                "redCurve": [[0.0, 0.0], [0.5, 0.8], [1.0, 1.0]]
            }),
        )]),
    };

    let result = call_codex_local_tool(
        &project,
        "video_creater.inspect_color",
        json!({
            "projectDir": project_dir.path().display().to_string(),
            "clipId": "red-curve-image-clip"
        }),
    )
    .expect("inspect red-curve-graded image clip scopes");

    assert_eq!(
        result.payload["clip"]["meanRGB"],
        json!([0.8, 0.502, 0.502])
    );
    assert_eq!(
        result.payload["grade"]["redCurve"],
        json!([[0.0, 0.0], [0.5, 0.8], [1.0, 1.0]])
    );
}

#[test]
fn codex_read_skill_returns_allowlisted_project_skill_body() {
    let project = sample_project();

    let result = call_codex_local_tool(
        &project,
        "video_creater.read_skill",
        json!({ "id": "video-creater-video-pipeline" }),
    )
    .expect("read project skill");

    assert!(!result.mutates_project);
    assert_eq!(result.payload["id"], json!("video-creater-video-pipeline"));
    assert_eq!(
        result.payload["path"],
        json!(".agents/skills/video-creater-video-pipeline/SKILL.md")
    );
    assert!(result.payload["body"]
        .as_str()
        .expect("skill body")
        .contains("Video Creater"));
}

#[test]
fn codex_read_skill_rejects_unknown_or_extra_inputs() {
    let project = sample_project();

    let unknown = call_codex_local_tool(
        &project,
        "video_creater.read_skill",
        json!({ "id": "open-ended-filesystem" }),
    );
    assert!(unknown
        .expect_err("unknown skill")
        .to_string()
        .contains("unknown project skill"));

    let extra = call_codex_local_tool(
        &project,
        "video_creater.read_skill",
        json!({ "id": "video-creater-visuals", "path": "/tmp/anything" }),
    );
    assert!(extra
        .expect_err("unknown field")
        .to_string()
        .contains("unknown field"));
}

#[test]
fn codex_send_feedback_returns_sanitized_local_handoff_payload() {
    let project = sample_project();

    let result = call_codex_local_tool(
        &project,
        "video_creater.send_feedback",
        json!({
            "category": "missing_capability",
            "summary": "Audio sync needs waveform correlation",
            "details": "The agent could not complete a timeline alignment because local audio correlation is not wired yet.",
            "severity": "medium"
        }),
    )
    .expect("send feedback");

    assert!(!result.mutates_project);
    assert_eq!(result.payload["accepted"], json!(true));
    assert_eq!(result.payload["submitted"], json!(false));
    assert_eq!(result.payload["delivery"], json!("local_handoff"));
    assert_eq!(
        result.payload["report"]["category"],
        json!("missing_capability")
    );
    assert_eq!(
        result.payload["report"]["summary"],
        json!("Audio sync needs waveform correlation")
    );
    assert_eq!(result.payload["report"]["severity"], json!("medium"));
    assert_eq!(
        result.payload["policy"]["projectContentIncluded"],
        json!(false)
    );
    assert_eq!(
        result.payload["policy"]["providerCredentialsExposed"],
        json!(false)
    );
}

#[test]
fn codex_send_feedback_rejects_invalid_or_unsafe_inputs() {
    let project = sample_project();

    let invalid_category = call_codex_local_tool(
        &project,
        "video_creater.send_feedback",
        json!({
            "category": "verbatim_project_dump",
            "summary": "Report"
        }),
    );
    assert!(invalid_category
        .expect_err("invalid category")
        .to_string()
        .contains("invalid feedback category"));

    let blank_summary = call_codex_local_tool(
        &project,
        "video_creater.send_feedback",
        json!({
            "category": "failure",
            "summary": "   "
        }),
    );
    assert!(blank_summary
        .expect_err("blank summary")
        .to_string()
        .contains("summary must not be blank"));

    let extra = call_codex_local_tool(
        &project,
        "video_creater.send_feedback",
        json!({
            "category": "failure",
            "summary": "Report",
            "screenshot": "/tmp/screen.png"
        }),
    );
    assert!(extra
        .expect_err("unknown field")
        .to_string()
        .contains("unknown field"));
}

#[test]
fn codex_direct_media_tools_reject_unknown_fields_before_validation() {
    let project = sample_project();

    let result = call_codex_local_tool(
        &project,
        "video_creater.rename_media",
        json!({
            "mediaRef": "media-1",
            "name": "Hero take",
            "extra": true
        }),
    );

    assert!(result.is_err());
    assert!(result
        .expect_err("unknown field error")
        .to_string()
        .contains("unknown field"));
}

#[test]
fn codex_add_clips_validates_project_actions_and_marks_mutation() {
    let project = sample_project();

    let empty = call_codex_local_tool(
        &project,
        "video_creater.add_clips",
        json!({ "actions": [] }),
    );
    assert!(empty.is_err());

    let result = call_codex_local_tool(
        &project,
        "video_creater.add_clips",
        json!({
            "actions": [
                {
                    "type": "addItems",
                    "targetTrackId": "track-video",
                    "items": [
                        {
                            "id": "clip-added-by-tool",
                            "kind": "video_clip",
                            "startSeconds": 4.0,
                            "durationSeconds": 1.0,
                            "source": { "type": "media", "mediaId": "media-1" },
                            "label": "Tool added clip",
                            "properties": { "sourceIn": 0.0, "sourceOut": 1.0 }
                        }
                    ]
                }
            ]
        }),
    )
    .expect("valid add clips action");

    assert!(result.mutates_project);
    assert_eq!(result.payload["valid"], json!(true));
}

#[test]
fn codex_add_clips_project_actions_variant_persists_to_split_project() {
    let project = sample_project();
    let project_dir = tempfile::tempdir().expect("project dir");
    save_split_project(project_dir.path(), &project).expect("save split project");

    let result = call_codex_local_tool(
        &project,
        "video_creater.add_clips",
        json!({
            "projectDir": project_dir.path().display().to_string(),
            "actions": [
                {
                    "type": "addItems",
                    "targetTrackId": "track-video",
                    "items": [
                        {
                            "id": "clip-added-by-tool",
                            "kind": "video_clip",
                            "startSeconds": 4.0,
                            "durationSeconds": 1.0,
                            "source": { "type": "media", "mediaId": "media-1" },
                            "label": "Tool added clip",
                            "properties": { "sourceIn": 0.0, "sourceOut": 1.0 }
                        }
                    ]
                }
            ]
        }),
    )
    .expect("projectActions add clips args should persist");

    assert!(result.mutates_project);
    assert_eq!(result.payload["applied"], json!(true));
    assert_eq!(result.payload["actionCount"], json!(1));
    assert_eq!(result.payload["agentHistory"]["entryCount"], json!(1));
    let persisted = load_split_project(project_dir.path()).expect("load added project");
    assert!(persisted.timeline.tracks[0]
        .items
        .iter()
        .any(|item| item.id == "clip-added-by-tool"));
}

#[test]
fn codex_add_clips_accepts_direct_media_entries() {
    let project = sample_project();

    let result = call_codex_local_tool(
        &project,
        "video_creater.add_clips",
        json!({
            "targetTrackId": "track-video",
            "clips": [
                {
                    "id": "clip-added-directly",
                    "mediaId": "media-1",
                    "startSeconds": 4.0,
                    "durationSeconds": 1.5,
                    "sourceIn": 1.0,
                    "sourceOut": 2.5,
                    "label": "Direct clip"
                }
            ]
        }),
    )
    .expect("direct add clips args should validate");

    assert!(result.mutates_project);
    assert_eq!(result.payload["valid"], json!(true));
    assert_eq!(result.payload["actionCount"], json!(1));
    assert_eq!(
        result.payload["affectedItemIds"],
        json!(["clip-added-directly"])
    );
    assert_eq!(result.payload["projectAfter"]["timelineItems"], json!(2));
}

#[test]
fn codex_add_clips_direct_args_persist_to_split_project() {
    let project = sample_project();
    let project_dir = tempfile::tempdir().expect("project dir");
    save_split_project(project_dir.path(), &project).expect("save split project");

    let result = call_codex_local_tool(
        &project,
        "video_creater.add_clips",
        json!({
            "projectDir": project_dir.path().display().to_string(),
            "targetTrackId": "track-video",
            "clips": [
                {
                    "id": "clip-added-directly",
                    "mediaId": "media-1",
                    "startSeconds": 4.0,
                    "durationSeconds": 1.5,
                    "sourceIn": 1.0,
                    "sourceOut": 2.5,
                    "label": "Direct clip"
                }
            ]
        }),
    )
    .expect("direct add clips args should persist");

    assert!(result.mutates_project);
    assert_eq!(result.payload["applied"], json!(true));
    assert_eq!(result.payload["actionCount"], json!(1));
    assert_eq!(result.payload["agentHistory"]["entryCount"], json!(1));
    assert_eq!(
        result.payload["affectedItemIds"],
        json!(["clip-added-directly"])
    );
    let persisted = load_split_project(project_dir.path()).expect("load added project");
    assert!(persisted.timeline.tracks[0]
        .items
        .iter()
        .any(|item| item.id == "clip-added-directly"));
}

#[test]
fn codex_add_clips_accepts_palmier_frame_entries() {
    let project = sample_project();

    let result = call_codex_local_tool(
        &project,
        "add_clips",
        json!({
            "entries": [
                {
                    "mediaRef": "media-1",
                    "trackIndex": 0,
                    "startFrame": 48,
                    "durationFrames": 36,
                    "trimStartFrame": 24
                }
            ]
        }),
    )
    .expect("Palmier frame add_clips args should validate");

    assert!(result.mutates_project);
    assert_eq!(result.payload["valid"], json!(true));
    assert_eq!(result.payload["actionCount"], json!(2));
    assert_eq!(
        result.payload["affectedItemIds"],
        json!(["clip-media-1-48", "clip-media-1-48-audio"])
    );
    let item = &result.payload["affectedItems"][0];
    assert_eq!(item["id"], json!("clip-media-1-48"));
    assert_eq!(item["startSeconds"], json!(2.0));
    assert_eq!(item["durationSeconds"], json!(1.5));
    assert_eq!(item["properties"]["sourceIn"], json!(1.0));
    assert_eq!(item["properties"]["sourceOut"], json!(2.5));

    let project_dir = tempfile::tempdir().expect("Palmier add project dir");
    save_split_project(project_dir.path(), &project).expect("save Palmier add project");
    let persisted = call_codex_local_tool(
        &project,
        "add_clips",
        json!({
            "projectDir": project_dir.path().display().to_string(),
            "entries": [
                {
                    "mediaRef": "media-1",
                    "trackIndex": 0,
                    "startFrame": 48,
                    "durationFrames": 36,
                    "trimStartFrame": 24
                }
            ]
        }),
    )
    .expect("Palmier frame add_clips args should persist");
    assert!(persisted.mutates_project);
    assert_eq!(persisted.payload["applied"], json!(true));
    assert_eq!(persisted.payload["agentHistory"]["entryCount"], json!(1));
    let persisted_project =
        load_split_project(project_dir.path()).expect("load Palmier added project");
    assert!(persisted_project
        .timeline
        .tracks
        .iter()
        .flat_map(|track| track.items.iter())
        .any(|item| item.id == "clip-media-1-48"));
}

#[test]
fn codex_add_clips_creates_linked_audio_on_explicit_palmier_video_track() {
    let project = sample_project();

    let result = call_codex_local_tool(
        &project,
        "add_clips",
        json!({
            "entries": [
                {
                    "mediaRef": "media-1",
                    "trackIndex": 0,
                    "startFrame": 48,
                    "durationFrames": 36
                }
            ]
        }),
    )
    .expect("Palmier add_clips should create linked audio on an explicit video track");

    assert!(result.mutates_project);
    assert_eq!(result.payload["valid"], json!(true));
    assert_eq!(result.payload["actionCount"], json!(2));
    assert_eq!(
        result.payload["affectedTrackIds"],
        json!(["track-audio", "track-video"])
    );
    assert_eq!(
        result.payload["affectedItemIds"],
        json!(["clip-media-1-48", "clip-media-1-48-audio"])
    );
    assert_eq!(
        result.payload["affectedItems"][0]["properties"]["linkGroupId"],
        json!("link-clip-media-1-48")
    );
    assert_eq!(
        result.payload["affectedItems"][1]["kind"],
        json!("audio_clip")
    );
    assert_eq!(
        result.payload["affectedItems"][1]["properties"]["sourceClipType"],
        json!("audio")
    );
    assert_eq!(
        result.payload["affectedItems"][1]["properties"]["linkGroupId"],
        json!("link-clip-media-1-48")
    );
    assert_eq!(result.payload["projectAfter"]["timelineItems"], json!(4));
}

#[test]
fn codex_add_clips_auto_creates_track_when_palmier_track_index_is_omitted() {
    let project = sample_project();

    let result = call_codex_local_tool(
        &project,
        "add_clips",
        json!({
            "entries": [
                {
                    "mediaRef": "media-1",
                    "startFrame": 48,
                    "durationFrames": 36
                }
            ]
        }),
    )
    .expect("Palmier add_clips without trackIndex should auto-create a target track");

    assert!(result.mutates_project);
    assert_eq!(result.payload["valid"], json!(true));
    assert_eq!(result.payload["actionCount"], json!(4));
    assert_eq!(
        result.payload["affectedTrackIds"],
        json!(["track-palmier-audio-1", "track-palmier-video-1"])
    );
    assert_eq!(
        result.payload["affectedItemIds"],
        json!(["clip-media-1-48", "clip-media-1-48-audio"])
    );
    assert_eq!(
        result.payload["affectedItems"][0]["properties"]["linkGroupId"],
        json!("link-clip-media-1-48")
    );
    assert_eq!(
        result.payload["affectedItems"][1]["kind"],
        json!("audio_clip")
    );
    assert_eq!(
        result.payload["affectedItems"][1]["properties"]["linkGroupId"],
        json!("link-clip-media-1-48")
    );
    assert_eq!(result.payload["projectAfter"]["timelineTracks"], json!(7));
    assert_eq!(result.payload["projectAfter"]["timelineItems"], json!(3));
}

#[test]
fn codex_add_clips_auto_routes_lottie_media_to_palmier_video_track() {
    let mut project = sample_project();
    project.media.push(MediaAsset {
        id: "media-lottie-1".to_string(),
        name: Some("Animated Badge".to_string()),
        relative_path: "media/animated-badge.json".to_string(),
        kind: MediaKind::Lottie,
        duration_seconds: 2.0,
        width: Some(1280),
        height: Some(720),
        fps: Some(24.0),
        folder_id: None,
    });

    let result = call_codex_local_tool(
        &project,
        "add_clips",
        json!({
            "entries": [
                {
                    "mediaRef": "media-lottie-1",
                    "startFrame": 24,
                    "durationFrames": 48
                }
            ]
        }),
    )
    .expect("Palmier add_clips should auto-route Lottie media to a visual track");

    assert!(result.mutates_project);
    assert_eq!(result.payload["valid"], json!(true));
    assert_eq!(result.payload["actionCount"], json!(2));
    assert_eq!(
        result.payload["affectedTrackIds"],
        json!(["track-palmier-video-1"])
    );
    assert_eq!(
        result.payload["affectedItemIds"],
        json!(["clip-media-lottie-1-24"])
    );
    assert_eq!(
        result.payload["affectedItems"][0]["source"]["mediaId"],
        json!("media-lottie-1")
    );
    assert_eq!(result.payload["projectAfter"]["timelineTracks"], json!(6));
    assert_eq!(result.payload["projectAfter"]["timelineItems"], json!(2));
}

#[test]
fn codex_add_clips_rejects_mixed_palmier_track_index_modes() {
    let project = sample_project();

    let error = call_codex_local_tool(
        &project,
        "add_clips",
        json!({
            "entries": [
                {
                    "mediaRef": "media-1",
                    "trackIndex": 0,
                    "startFrame": 48,
                    "durationFrames": 36
                },
                {
                    "mediaRef": "media-1",
                    "startFrame": 84,
                    "durationFrames": 24
                }
            ]
        }),
    )
    .expect_err("mixed explicit and omitted trackIndex modes should be rejected");

    assert!(error
        .to_string()
        .contains("entries.trackIndex must be provided on every entry or omitted on every entry"));
}

#[test]
fn codex_add_clips_derives_palmier_duration_from_source_when_omitted() {
    let project = sample_project();

    let result = call_codex_local_tool(
        &project,
        "add_clips",
        json!({
            "entries": [
                {
                    "mediaRef": "media-1",
                    "trackIndex": 0,
                    "startFrame": 24,
                    "trimStartFrame": 12,
                    "trimEndFrame": 24
                }
            ]
        }),
    )
    .expect("Palmier add_clips should derive duration from source when omitted");

    let item = &result.payload["affectedItems"][0];
    assert_eq!(item["startSeconds"], json!(1.0));
    assert_eq!(item["durationSeconds"], json!(10.5));
    assert_eq!(item["properties"]["sourceIn"], json!(0.5));
    assert_eq!(item["properties"]["sourceOut"], json!(11.0));
}

#[test]
fn codex_add_clips_rejects_duration_frames_with_trim_end_frame() {
    let project = sample_project();

    let error = call_codex_local_tool(
        &project,
        "add_clips",
        json!({
            "entries": [
                {
                    "mediaRef": "media-1",
                    "trackIndex": 0,
                    "startFrame": 24,
                    "durationFrames": 48,
                    "trimEndFrame": 24
                }
            ]
        }),
    )
    .expect_err("Palmier add_clips should reject durationFrames with trimEndFrame");

    assert!(error
        .to_string()
        .contains("set durationFrames OR trimEndFrame, not both"));
}

#[test]
fn codex_insert_clips_accepts_direct_media_entries() {
    let project = sample_project();

    let result = call_codex_local_tool(
        &project,
        "video_creater.insert_clips",
        json!({
            "targetTrackId": "track-video",
            "insertSeconds": 2.0,
            "clips": [
                {
                    "id": "clip-inserted-directly",
                    "mediaId": "media-1",
                    "durationSeconds": 1.0,
                    "sourceIn": 3.0,
                    "sourceOut": 4.0,
                    "label": "Inserted direct clip"
                }
            ]
        }),
    )
    .expect("direct insert clips args should validate");

    assert!(result.mutates_project);
    assert_eq!(result.payload["valid"], json!(true));
    assert_eq!(result.payload["actionCount"], json!(1));
    assert_eq!(
        result.payload["affectedItemIds"],
        json!(["clip-inserted-directly"])
    );
    // Inserting inside the existing 0-4s clip partitions that clip around the
    // insertion, so the project contains both retained fragments plus the new clip.
    assert_eq!(result.payload["projectAfter"]["timelineItems"], json!(3));
}

#[test]
fn codex_insert_clips_direct_args_persist_to_split_project() {
    let project = sample_project();
    let project_dir = tempfile::tempdir().expect("project dir");
    save_split_project(project_dir.path(), &project).expect("save split project");

    let result = call_codex_local_tool(
        &project,
        "video_creater.insert_clips",
        json!({
            "projectDir": project_dir.path().display().to_string(),
            "targetTrackId": "track-video",
            "insertSeconds": 2.0,
            "clips": [
                {
                    "id": "clip-inserted-directly",
                    "mediaId": "media-1",
                    "durationSeconds": 1.0,
                    "sourceIn": 3.0,
                    "sourceOut": 4.0,
                    "label": "Inserted direct clip"
                }
            ]
        }),
    )
    .expect("direct insert clips args should persist");

    assert!(result.mutates_project);
    assert_eq!(result.payload["applied"], json!(true));
    assert_eq!(result.payload["actionCount"], json!(1));
    assert_eq!(result.payload["agentHistory"]["entryCount"], json!(1));
    assert_eq!(
        result.payload["affectedItemIds"],
        json!(["clip-inserted-directly"])
    );
    let persisted = load_split_project(project_dir.path()).expect("load inserted project");
    assert!(persisted.timeline.tracks[0]
        .items
        .iter()
        .any(|item| item.id == "clip-inserted-directly"));
}

#[test]
fn codex_insert_clips_accepts_palmier_frame_entries_with_linked_audio() {
    let project = sample_project();

    let result = call_codex_local_tool(
        &project,
        "insert_clips",
        json!({
            "trackIndex": 0,
            "atFrame": 48,
            "entries": [
                {
                    "mediaRef": "media-1",
                    "durationFrames": 36,
                    "trimStartFrame": 24
                }
            ]
        }),
    )
    .expect("Palmier insert_clips args should validate");

    assert!(result.mutates_project);
    assert_eq!(result.payload["valid"], json!(true));
    assert_eq!(result.payload["actionCount"], json!(2));
    assert_eq!(
        result.payload["affectedTrackIds"],
        json!(["track-audio", "track-video"])
    );
    assert_eq!(
        result.payload["affectedItemIds"],
        json!(["clip-media-1-48", "clip-media-1-48-audio"])
    );
    assert_eq!(
        result.payload["affectedItems"][0]["properties"]["sourceIn"],
        json!(1.0)
    );
    assert_eq!(
        result.payload["affectedItems"][0]["properties"]["sourceOut"],
        json!(2.5)
    );
    assert_eq!(
        result.payload["affectedItems"][1]["kind"],
        json!("audio_clip")
    );
    assert_eq!(
        result.payload["affectedItems"][1]["properties"]["linkGroupId"],
        json!("link-clip-media-1-48")
    );
    // The existing video is partitioned around the insertion and the inserted
    // source contributes linked video/audio items.
    assert_eq!(result.payload["projectAfter"]["timelineItems"], json!(4));

    let project_dir = tempfile::tempdir().expect("Palmier insert project dir");
    save_split_project(project_dir.path(), &project).expect("save Palmier insert project");
    let persisted = call_codex_local_tool(
        &project,
        "insert_clips",
        json!({
            "projectDir": project_dir.path().display().to_string(),
            "trackIndex": 0,
            "atFrame": 48,
            "entries": [
                {
                    "mediaRef": "media-1",
                    "durationFrames": 36,
                    "trimStartFrame": 24
                }
            ]
        }),
    )
    .expect("Palmier insert_clips args should persist");
    assert!(persisted.mutates_project);
    assert_eq!(persisted.payload["applied"], json!(true));
    assert_eq!(persisted.payload["agentHistory"]["entryCount"], json!(1));
    let persisted_project =
        load_split_project(project_dir.path()).expect("load Palmier inserted project");
    assert!(persisted_project
        .timeline
        .tracks
        .iter()
        .flat_map(|track| track.items.iter())
        .any(|item| item.id == "clip-media-1-48"));
}

#[test]
fn codex_remove_clips_accepts_direct_item_ids() {
    let project = sample_project();

    let result = call_codex_local_tool(
        &project,
        "video_creater.remove_clips",
        json!({ "itemIds": ["item-1"] }),
    )
    .expect("direct remove clips args should validate");

    assert!(result.mutates_project);
    assert_eq!(result.payload["valid"], json!(true));
    assert_eq!(result.payload["actionCount"], json!(1));
    assert_eq!(result.payload["affectedItemIds"], json!(["item-1"]));
    assert_eq!(result.payload["projectAfter"]["timelineItems"], json!(0));
}

#[test]
fn codex_remove_clips_direct_args_persist_to_split_project() {
    let project = sample_project();
    let project_dir = tempfile::tempdir().expect("project dir");
    save_split_project(project_dir.path(), &project).expect("save split project");

    let result = call_codex_local_tool(
        &project,
        "video_creater.remove_clips",
        json!({
            "projectDir": project_dir.path().display().to_string(),
            "itemIds": ["item-1"]
        }),
    )
    .expect("direct remove clips args should persist");

    assert!(result.mutates_project);
    assert_eq!(result.payload["applied"], json!(true));
    assert_eq!(result.payload["actionCount"], json!(1));
    assert_eq!(result.payload["agentHistory"]["entryCount"], json!(1));
    assert_eq!(result.payload["affectedItemIds"], json!(["item-1"]));
    let persisted = load_split_project(project_dir.path()).expect("load removed project");
    assert!(persisted.timeline.tracks[0].items.is_empty());
}

#[test]
fn codex_remove_clips_accepts_palmier_clip_ids_alias() {
    let project = sample_project();

    let result = call_codex_local_tool(&project, "remove_clips", json!({ "clipIds": ["item-1"] }))
        .expect("Palmier clipIds alias should validate");

    assert!(result.mutates_project);
    assert_eq!(result.payload["valid"], json!(true));
    assert_eq!(result.payload["affectedItemIds"], json!(["item-1"]));
}

#[test]
fn codex_remove_clips_expands_palmier_link_group() {
    let mut project = sample_project();
    project.timeline.tracks[0].items[0]
        .properties
        .insert("linkGroupId".to_string(), json!("link-item-1"));
    project.timeline.tracks[4].items.push(TimelineItem {
        id: "item-1-audio".to_string(),
        kind: TimelineItemKind::AudioClip,
        start_seconds: 0.0,
        duration_seconds: 4.0,
        source: TimelineSource::Media {
            media_id: "media-1".to_string(),
        },
        label: "Opening clip audio".to_string(),
        properties: BTreeMap::from([
            ("linkGroupId".to_string(), json!("link-item-1")),
            ("sourceClipType".to_string(), json!("audio")),
        ]),
    });

    let result = call_codex_local_tool(&project, "remove_clips", json!({ "clipIds": ["item-1"] }))
        .expect("linked Palmier remove_clips should validate");

    assert!(result.mutates_project);
    assert_eq!(result.payload["valid"], json!(true));
    assert_eq!(
        result.payload["affectedItemIds"],
        json!(["item-1", "item-1-audio"])
    );
    assert_eq!(result.payload["projectAfter"]["timelineItems"], json!(0));
}

#[test]
fn codex_remove_clips_accepts_unique_short_item_id_prefixes() {
    let mut project = sample_project();
    project.timeline.tracks[0].items[0].id = "ABCD1234-0000-0000-0000-000000000000".to_string();

    let result = call_codex_local_tool(
        &project,
        "video_creater.remove_clips",
        json!({ "itemIds": ["ABCD1234"] }),
    )
    .expect("unique short item id prefix should validate");

    assert!(result.mutates_project);
    assert_eq!(result.payload["valid"], json!(true));
    assert_eq!(result.payload["actionCount"], json!(1));
    assert_eq!(
        result.payload["affectedItemIds"],
        json!(["ABCD1234-0000-0000-0000-000000000000"])
    );
    assert_eq!(result.payload["projectAfter"]["timelineItems"], json!(0));
}

#[test]
fn codex_remove_clips_rejects_ambiguous_short_item_id_prefixes() {
    let mut project = sample_project();
    project.timeline.tracks[0].items[0].id = "ABCD1234-0000-0000-0000-000000000000".to_string();
    let mut second_item = project.timeline.tracks[0].items[0].clone();
    second_item.id = "ABCD1234-FFFF-0000-0000-000000000000".to_string();
    second_item.start_seconds = 12.0;
    project.timeline.tracks[0].items.push(second_item);

    let err = call_codex_local_tool(
        &project,
        "video_creater.remove_clips",
        json!({ "itemIds": ["ABCD1234"] }),
    )
    .expect_err("ambiguous short item id prefix should be rejected");

    assert!(
        err.to_string().contains("ambiguous"),
        "unexpected error: {err}"
    );
}

#[test]
fn codex_split_clips_accepts_direct_split_entries() {
    let project = sample_project();

    let result = call_codex_local_tool(
        &project,
        "video_creater.split_clips",
        json!({
            "splits": [
                {
                    "itemId": "item-1",
                    "splitSeconds": 2.0,
                    "newItemId": "item-1-tail"
                }
            ]
        }),
    )
    .expect("direct split clips args should validate");

    assert!(result.mutates_project);
    assert_eq!(result.payload["valid"], json!(true));
    assert_eq!(result.payload["actionCount"], json!(1));
    assert_eq!(
        result.payload["affectedItemIds"],
        json!(["item-1", "item-1-tail"])
    );
    assert_eq!(result.payload["projectAfter"]["timelineItems"], json!(2));
}

#[test]
fn codex_split_clips_direct_args_persist_to_split_project() {
    let project = sample_project();
    let project_dir = tempfile::tempdir().expect("project dir");
    save_split_project(project_dir.path(), &project).expect("save split project");

    let result = call_codex_local_tool(
        &project,
        "video_creater.split_clips",
        json!({
            "projectDir": project_dir.path().display().to_string(),
            "splits": [
                {
                    "itemId": "item-1",
                    "splitSeconds": 2.0,
                    "newItemId": "item-1-tail"
                }
            ]
        }),
    )
    .expect("direct split clips args should persist");

    assert!(result.mutates_project);
    assert_eq!(result.payload["applied"], json!(true));
    assert_eq!(result.payload["actionCount"], json!(1));
    assert_eq!(result.payload["agentHistory"]["entryCount"], json!(1));
    assert_eq!(
        result.payload["affectedItemIds"],
        json!(["item-1", "item-1-tail"])
    );
    let persisted = load_split_project(project_dir.path()).expect("load split project");
    assert_eq!(persisted.timeline.tracks[0].items.len(), 2);
    assert!(persisted.timeline.tracks[0]
        .items
        .iter()
        .any(|item| item.id == "item-1-tail"));
}

#[test]
fn codex_split_clips_accepts_palmier_explicit_frame_splits() {
    let project = sample_project();

    let result = call_codex_local_tool(
        &project,
        "split_clips",
        json!({
            "splits": [
                {
                    "clipId": "item-1",
                    "atFrame": 48
                }
            ]
        }),
    )
    .expect("Palmier explicit frame split args should validate");

    assert!(result.mutates_project);
    assert_eq!(result.payload["valid"], json!(true));
    assert_eq!(
        result.payload["affectedItemIds"],
        json!(["item-1", "item-1-split-2000"])
    );
    assert_eq!(
        result.payload["splits"][0],
        json!({
            "itemId": "item-1",
            "newItemId": "item-1-split-2000",
            "splitSeconds": 2.0
        })
    );
}

#[test]
fn codex_split_clips_treats_clip_at_frame_as_clip_local() {
    let mut project = sample_project();
    project.timeline.tracks[0].items[0].start_seconds = 10.0;
    project.timeline.tracks[0].items[0].duration_seconds = 2.0;
    project.timeline.tracks[0].items[0]
        .properties
        .insert("sourceIn".to_string(), json!(2.0));
    project.timeline.tracks[0].items[0]
        .properties
        .insert("sourceOut".to_string(), json!(4.0));

    let result = call_codex_local_tool(
        &project,
        "split_clips",
        json!({
            "splits": [
                {
                    "clipId": "item-1",
                    "atFrame": 12
                }
            ]
        }),
    )
    .expect("Palmier clip atFrame should be clip-local");

    assert!(result.mutates_project);
    assert_eq!(result.payload["valid"], json!(true));
    assert_eq!(
        result.payload["affectedItemIds"],
        json!(["item-1", "item-1-split-10500"])
    );
    assert_eq!(
        result.payload["splits"][0],
        json!({
            "itemId": "item-1",
            "newItemId": "item-1-split-10500",
            "splitSeconds": 10.5
        })
    );
    assert_eq!(result.payload["projectAfter"]["timelineItems"], json!(2));
}

#[test]
fn codex_split_clips_expands_palmier_link_group_at_frame() {
    let mut project = sample_project();
    project.timeline.tracks[0].items[0]
        .properties
        .insert("linkGroupId".to_string(), json!("link-item-1"));
    project.timeline.tracks[4].items.push(TimelineItem {
        id: "item-1-audio".to_string(),
        kind: TimelineItemKind::AudioClip,
        start_seconds: 0.5,
        duration_seconds: 4.0,
        source: TimelineSource::Media {
            media_id: "media-1".to_string(),
        },
        label: "Opening clip audio".to_string(),
        properties: BTreeMap::from([
            ("linkGroupId".to_string(), json!("link-item-1")),
            ("sourceClipType".to_string(), json!("audio")),
        ]),
    });

    let result = call_codex_local_tool(
        &project,
        "split_clips",
        json!({
            "splits": [
                {
                    "clipId": "item-1",
                    "atFrame": 48
                }
            ]
        }),
    )
    .expect("linked Palmier split_clips should validate");

    assert!(result.mutates_project);
    assert_eq!(result.payload["valid"], json!(true));
    assert_eq!(
        result.payload["affectedItemIds"],
        json!([
            "item-1",
            "item-1-split-2000",
            "item-1-audio",
            "item-1-audio-split-2000"
        ])
    );
    assert_eq!(
        result.payload["splits"],
        json!([
            {
                "itemId": "item-1",
                "newItemId": "item-1-split-2000",
                "splitSeconds": 2.0
            },
            {
                "itemId": "item-1-audio",
                "newItemId": "item-1-audio-split-2000",
                "splitSeconds": 2.0
            }
        ])
    );
    assert_eq!(result.payload["projectAfter"]["timelineItems"], json!(4));
}

#[test]
fn codex_split_clips_accepts_palmier_track_frame_splits() {
    let project = sample_project();

    let result = call_codex_local_tool(
        &project,
        "split_clips",
        json!({
            "trackIndex": 0,
            "frames": [72]
        }),
    )
    .expect("Palmier track frame split args should validate");

    assert!(result.mutates_project);
    assert_eq!(result.payload["valid"], json!(true));
    assert_eq!(
        result.payload["affectedItemIds"],
        json!(["item-1", "item-1-split-3000"])
    );
    assert_eq!(
        result.payload["splits"][0],
        json!({
            "itemId": "item-1",
            "newItemId": "item-1-split-3000",
            "splitSeconds": 3.0
        })
    );
}

#[test]
fn codex_move_clips_accepts_direct_move_entries() {
    let project = sample_project();

    let result = call_codex_local_tool(
        &project,
        "video_creater.move_clips",
        json!({
            "moves": [
                {
                    "itemId": "item-1",
                    "targetTrackId": "track-video",
                    "startSeconds": 2.5
                }
            ]
        }),
    )
    .expect("direct move clips args should validate");

    assert!(result.mutates_project);
    assert_eq!(result.payload["valid"], json!(true));
    assert_eq!(result.payload["actionCount"], json!(1));
    assert_eq!(result.payload["affectedItemIds"], json!(["item-1"]));
    assert_eq!(result.payload["projectAfter"]["timelineItems"], json!(1));
}

#[test]
fn codex_move_clips_direct_args_persist_to_split_project() {
    let project = sample_project();
    let project_dir = tempfile::tempdir().expect("project dir");
    save_split_project(project_dir.path(), &project).expect("save split project");

    let result = call_codex_local_tool(
        &project,
        "video_creater.move_clips",
        json!({
            "projectDir": project_dir.path().display().to_string(),
            "moves": [
                {
                    "itemId": "item-1",
                    "targetTrackId": "track-video",
                    "startSeconds": 2.5
                }
            ]
        }),
    )
    .expect("direct move clips args should persist");

    assert!(result.mutates_project);
    assert_eq!(result.payload["applied"], json!(true));
    assert_eq!(result.payload["actionCount"], json!(1));
    assert_eq!(result.payload["agentHistory"]["entryCount"], json!(1));
    assert_eq!(result.payload["affectedItemIds"], json!(["item-1"]));
    let persisted = load_split_project(project_dir.path()).expect("load moved project");
    assert_eq!(persisted.timeline.tracks[0].items[0].start_seconds, 2.5);
}

#[test]
fn codex_move_clips_project_actions_variant_persists_to_split_project() {
    let project = sample_project();
    let project_dir = tempfile::tempdir().expect("project dir");
    save_split_project(project_dir.path(), &project).expect("save split project");

    let result = call_codex_local_tool(
        &project,
        "video_creater.move_clips",
        json!({
            "projectDir": project_dir.path().display().to_string(),
            "actions": [
                {
                    "type": "moveItems",
                    "moves": [
                        {
                            "itemId": "item-1",
                            "targetTrackId": "track-video",
                            "startSeconds": 3.25
                        }
                    ]
                }
            ]
        }),
    )
    .expect("projectActions move clips args should persist");

    assert!(result.mutates_project);
    assert_eq!(result.payload["applied"], json!(true));
    assert_eq!(result.payload["actionCount"], json!(1));
    assert_eq!(result.payload["agentHistory"]["entryCount"], json!(1));
    let persisted = load_split_project(project_dir.path()).expect("load moved project");
    assert_eq!(persisted.timeline.tracks[0].items[0].start_seconds, 3.25);
}

#[test]
fn codex_move_clips_accepts_palmier_frame_moves() {
    let project = sample_project();

    let result = call_codex_local_tool(
        &project,
        "move_clips",
        json!({
            "moves": [
                {
                    "clipId": "item-1",
                    "toFrame": 60
                }
            ]
        }),
    )
    .expect("Palmier frame move args should validate");

    assert!(result.mutates_project);
    assert_eq!(result.payload["valid"], json!(true));
    assert_eq!(result.payload["affectedItemIds"], json!(["item-1"]));
    assert_eq!(
        result.payload["moves"][0],
        json!({
            "itemId": "item-1",
            "targetTrackId": "track-video",
            "startSeconds": 2.5
        })
    );
}

#[test]
fn codex_move_clips_expands_palmier_link_group_frame_delta() {
    let mut project = sample_project();
    project.timeline.tracks[0].items[0]
        .properties
        .insert("linkGroupId".to_string(), json!("link-item-1"));
    project.timeline.tracks[4].items.push(TimelineItem {
        id: "item-1-audio".to_string(),
        kind: TimelineItemKind::AudioClip,
        start_seconds: 0.5,
        duration_seconds: 4.0,
        source: TimelineSource::Media {
            media_id: "media-1".to_string(),
        },
        label: "Opening clip audio".to_string(),
        properties: BTreeMap::from([
            ("linkGroupId".to_string(), json!("link-item-1")),
            ("sourceClipType".to_string(), json!("audio")),
        ]),
    });

    let result = call_codex_local_tool(
        &project,
        "move_clips",
        json!({
            "moves": [
                {
                    "clipId": "item-1",
                    "toFrame": 60
                }
            ]
        }),
    )
    .expect("linked Palmier move_clips should validate");

    assert!(result.mutates_project);
    assert_eq!(result.payload["valid"], json!(true));
    assert_eq!(
        result.payload["affectedItemIds"],
        json!(["item-1", "item-1-audio"])
    );
    assert_eq!(
        result.payload["moves"],
        json!([
            {
                "itemId": "item-1",
                "targetTrackId": "track-video",
                "startSeconds": 2.5
            },
            {
                "itemId": "item-1-audio",
                "targetTrackId": "track-audio",
                "startSeconds": 3.0
            }
        ])
    );
}

#[test]
fn codex_set_clip_properties_accepts_direct_updates() {
    let project = sample_project();

    let result = call_codex_local_tool(
        &project,
        "video_creater.set_clip_properties",
        json!({
            "updates": [
                {
                    "itemId": "item-1",
                    "opacity": 0.5
                }
            ]
        }),
    )
    .expect("direct clip property args should validate");

    assert!(result.mutates_project);
    assert_eq!(result.payload["valid"], json!(true));
    assert_eq!(result.payload["actionCount"], json!(1));
    assert_eq!(result.payload["affectedItemIds"], json!(["item-1"]));
}

#[test]
fn codex_set_clip_properties_direct_args_persist_to_split_project() {
    let project = sample_project();
    let project_dir = tempfile::tempdir().expect("project dir");
    save_split_project(project_dir.path(), &project).expect("save split project");

    let result = call_codex_local_tool(
        &project,
        "video_creater.set_clip_properties",
        json!({
            "projectDir": project_dir.path().display().to_string(),
            "updates": [
                {
                    "itemId": "item-1",
                    "opacity": 0.5
                }
            ]
        }),
    )
    .expect("direct clip property args should persist");

    assert!(result.mutates_project);
    assert_eq!(result.payload["applied"], json!(true));
    assert_eq!(result.payload["actionCount"], json!(1));
    assert_eq!(result.payload["agentHistory"]["entryCount"], json!(1));
    assert_eq!(result.payload["affectedItemIds"], json!(["item-1"]));
    let persisted = load_split_project(project_dir.path()).expect("load updated project");
    assert_eq!(
        persisted.timeline.tracks[0].items[0].properties["opacity"],
        json!(0.5)
    );
}

#[test]
fn codex_set_clip_properties_accepts_palmier_blend_mode() {
    let mut project = sample_project();
    let mut audio_item = project.timeline.tracks[0].items[0].clone();
    audio_item.id = "item-audio".to_string();
    audio_item.kind = TimelineItemKind::AudioClip;
    audio_item.source = TimelineSource::Media {
        media_id: "media-1".to_string(),
    };
    project.timeline.tracks.push(TimelineTrack {
        transitions: Vec::new(),
        id: "track-audio".to_string(),
        name: "Audio".to_string(),
        kind: TrackKind::Audio,
        items: vec![audio_item],
        locked: false,
        sync_locked: false,
        enabled: true,
    });

    let project_dir = tempfile::tempdir().expect("blend mode project dir");
    save_split_project(project_dir.path(), &project).expect("save blend mode project");

    let screen = call_codex_local_tool(
        &project,
        "set_clip_properties",
        json!({
            "projectDir": project_dir.path().display().to_string(),
            "clipIds": ["item-1"],
            "blendMode": "screen"
        }),
    )
    .expect("Palmier blend mode should persist");
    assert!(screen.mutates_project);
    assert_eq!(screen.payload["applied"], json!(true));
    assert_eq!(
        screen.payload["updates"][0],
        json!({
            "itemId": "item-1",
            "blendMode": "screen"
        })
    );

    let persisted_screen = load_split_project(project_dir.path()).expect("load screen project");
    assert_eq!(
        persisted_screen.timeline.tracks[0].items[0].properties["blendMode"],
        json!("screen")
    );

    let normal = call_codex_local_tool(
        &persisted_screen,
        "set_clip_properties",
        json!({
            "projectDir": project_dir.path().display().to_string(),
            "clipIds": ["item-1"],
            "blendMode": "normal"
        }),
    )
    .expect("Palmier normal blend mode should clear custom mode");
    assert!(normal.mutates_project);
    assert_eq!(normal.payload["updates"][0]["blendMode"], json!("normal"));
    let persisted_normal = load_split_project(project_dir.path()).expect("load normal project");
    assert!(!persisted_normal.timeline.tracks[0].items[0]
        .properties
        .contains_key("blendMode"));

    let invalid = call_codex_local_tool(
        &project,
        "set_clip_properties",
        json!({
            "clipIds": ["item-1"],
            "blendMode": "glow"
        }),
    )
    .expect_err("invalid Palmier blend mode should fail");
    assert!(invalid.to_string().contains("blendMode"));

    let audio = call_codex_local_tool(
        &project,
        "set_clip_properties",
        json!({
            "clipIds": ["item-audio"],
            "blendMode": "screen"
        }),
    )
    .expect_err("blend mode should require visual clips");
    assert!(audio.to_string().contains("video or image"));
}

#[test]
fn codex_set_clip_properties_accepts_full_palmier_blend_mode_enum() {
    let mut project = sample_project();
    let project_dir = tempfile::tempdir().expect("blend enum project dir");
    save_split_project(project_dir.path(), &project).expect("save blend enum project");

    for mode in ["hue", "saturation", "color", "luminosity"] {
        let result = call_codex_local_tool(
            &project,
            "set_clip_properties",
            json!({
                "projectDir": project_dir.path().display().to_string(),
                "clipIds": ["item-1"],
                "blendMode": mode
            }),
        )
        .expect("Palmier blend mode should persist");
        assert!(result.mutates_project);
        assert_eq!(result.payload["updates"][0]["blendMode"], json!(mode));

        project = load_split_project(project_dir.path()).expect("load blend enum project");
        assert_eq!(
            project.timeline.tracks[0].items[0].properties["blendMode"],
            json!(mode)
        );
    }

    let timeline =
        call_codex_local_tool(&project, "get_timeline", json!({})).expect("timeline payload");
    assert_eq!(
        timeline.payload["tracks"][0]["clips"][0]["blendMode"],
        json!("luminosity")
    );
}

#[test]
fn codex_set_clip_properties_accepts_palmier_frame_batch_updates() {
    let project = sample_project();

    let result = call_codex_local_tool(
        &project,
        "set_clip_properties",
        json!({
            "clipIds": ["item-1"],
            "durationFrames": 48,
            "trimStartFrame": 24,
            "trimEndFrame": 72,
            "opacity": 0.5
        }),
    )
    .expect("Palmier frame batch clip properties should validate");

    assert!(result.mutates_project);
    assert_eq!(result.payload["valid"], json!(true));
    assert_eq!(result.payload["affectedItemIds"], json!(["item-1"]));
    assert_eq!(
        result.payload["updates"][0],
        json!({
            "itemId": "item-1",
            "durationSeconds": 2.0,
            "sourceIn": 1.0,
            "sourceOut": 3.0,
            "opacity": 0.5
        })
    );
}

#[test]
fn codex_set_clip_properties_expands_palmier_link_group_timing_updates() {
    let mut project = sample_project();
    project.timeline.tracks[0].items[0]
        .properties
        .insert("linkGroupId".to_string(), json!("link-item-1"));
    project.timeline.tracks[4].items.push(TimelineItem {
        id: "item-1-audio".to_string(),
        kind: TimelineItemKind::AudioClip,
        start_seconds: 0.0,
        duration_seconds: 4.0,
        source: TimelineSource::Media {
            media_id: "media-1".to_string(),
        },
        label: "Opening clip audio".to_string(),
        properties: BTreeMap::from([
            ("linkGroupId".to_string(), json!("link-item-1")),
            ("sourceClipType".to_string(), json!("audio")),
        ]),
    });

    let result = call_codex_local_tool(
        &project,
        "set_clip_properties",
        json!({
            "clipIds": ["item-1"],
            "durationFrames": 48,
            "trimStartFrame": 24,
            "trimEndFrame": 72
        }),
    )
    .expect("linked Palmier timing properties should validate");

    assert!(result.mutates_project);
    assert_eq!(result.payload["valid"], json!(true));
    assert_eq!(
        result.payload["affectedItemIds"],
        json!(["item-1", "item-1-audio"])
    );
    assert_eq!(
        result.payload["updates"],
        json!([
            {
                "itemId": "item-1",
                "durationSeconds": 2.0,
                "sourceIn": 1.0,
                "sourceOut": 3.0
            },
            {
                "itemId": "item-1-audio",
                "durationSeconds": 2.0,
                "sourceIn": 1.0,
                "sourceOut": 3.0
            }
        ])
    );
}

#[test]
fn codex_set_clip_properties_does_not_apply_trim_or_speed_to_linked_text_partners() {
    let mut project = sample_project();
    project.timeline.tracks[0].items[0]
        .properties
        .insert("linkGroupId".to_string(), json!("link-text-1"));
    project.timeline.tracks[1].items.push(TimelineItem {
        id: "title-1".to_string(),
        kind: TimelineItemKind::Overlay,
        start_seconds: 0.0,
        duration_seconds: 4.0,
        source: TimelineSource::Text {
            text: "Launch".to_string(),
        },
        label: "Launch".to_string(),
        properties: BTreeMap::from([
            ("linkGroupId".to_string(), json!("link-text-1")),
            ("text".to_string(), json!("Launch")),
        ]),
    });
    let project_dir = tempfile::tempdir().expect("linked text project dir");
    save_split_project(project_dir.path(), &project).expect("save linked text project");

    let result = call_codex_local_tool(
        &project,
        "set_clip_properties",
        json!({
            "projectDir": project_dir.path().display().to_string(),
            "clipIds": ["item-1"],
            "durationFrames": 48,
            "trimStartFrame": 24,
            "trimEndFrame": 120,
            "speed": 2.0
        }),
    )
    .expect("linked Palmier timing properties should validate");

    assert!(result.mutates_project);
    assert_eq!(result.payload["applied"], json!(true));
    assert_eq!(
        result.payload["affectedItemIds"],
        json!(["item-1", "title-1"])
    );
    assert_eq!(
        result.payload["updates"],
        json!([
            {
                "itemId": "item-1",
                "durationSeconds": 2.0,
                "sourceIn": 1.0,
                "sourceOut": 5.0,
                "speed": 2.0
            },
            {
                "itemId": "title-1",
                "durationSeconds": 2.0
            }
        ])
    );

    let persisted_project =
        load_split_project(project_dir.path()).expect("load linked text project");
    let title = persisted_project
        .timeline
        .tracks
        .iter()
        .flat_map(|track| track.items.iter())
        .find(|item| item.id == "title-1")
        .expect("persisted title");
    assert_eq!(title.duration_seconds, 2.0);
    assert!(!title.properties.contains_key("sourceIn"));
    assert!(!title.properties.contains_key("sourceOut"));
}

#[test]
fn codex_set_clip_properties_does_not_apply_speed_only_duration_to_linked_text_partners() {
    let mut project = sample_project();
    project.timeline.tracks[0].items[0]
        .properties
        .insert("linkGroupId".to_string(), json!("link-text-speed"));
    project.timeline.tracks[1].items.push(TimelineItem {
        id: "title-1".to_string(),
        kind: TimelineItemKind::Overlay,
        start_seconds: 0.0,
        duration_seconds: 4.0,
        source: TimelineSource::Text {
            text: "Launch".to_string(),
        },
        label: "Launch".to_string(),
        properties: BTreeMap::from([
            ("linkGroupId".to_string(), json!("link-text-speed")),
            ("text".to_string(), json!("Launch")),
        ]),
    });
    let project_dir = tempfile::tempdir().expect("linked text speed project dir");
    save_split_project(project_dir.path(), &project).expect("save linked text speed project");

    let result = call_codex_local_tool(
        &project,
        "set_clip_properties",
        json!({
            "projectDir": project_dir.path().display().to_string(),
            "clipIds": ["item-1"],
            "speed": 2.0
        }),
    )
    .expect("linked Palmier speed property should validate");

    assert!(result.mutates_project);
    assert_eq!(result.payload["applied"], json!(true));
    assert_eq!(result.payload["affectedItemIds"], json!(["item-1"]));

    let persisted_project =
        load_split_project(project_dir.path()).expect("load linked text speed project");
    let title = persisted_project
        .timeline
        .tracks
        .iter()
        .flat_map(|track| track.items.iter())
        .find(|item| item.id == "title-1")
        .expect("persisted title");
    assert_eq!(title.duration_seconds, 4.0);
}

#[test]
fn codex_set_clip_properties_propagates_speed_to_linked_non_text_partners() {
    let mut project = sample_project();
    project.timeline.tracks[0].items[0]
        .properties
        .insert("linkGroupId".to_string(), json!("link-speed-1"));
    project.timeline.tracks[4].items.push(TimelineItem {
        id: "item-1-audio".to_string(),
        kind: TimelineItemKind::AudioClip,
        start_seconds: 0.0,
        duration_seconds: 4.0,
        source: TimelineSource::Media {
            media_id: "media-1".to_string(),
        },
        label: "Opening clip audio".to_string(),
        properties: BTreeMap::from([
            ("linkGroupId".to_string(), json!("link-speed-1")),
            ("sourceClipType".to_string(), json!("audio")),
        ]),
    });
    let project_dir = tempfile::tempdir().expect("linked speed project dir");
    save_split_project(project_dir.path(), &project).expect("save linked speed project");

    let result = call_codex_local_tool(
        &project,
        "set_clip_properties",
        json!({
            "projectDir": project_dir.path().display().to_string(),
            "clipIds": ["item-1"],
            "speed": 2.0
        }),
    )
    .expect("linked Palmier speed property should validate");

    assert!(result.mutates_project);
    assert_eq!(result.payload["applied"], json!(true));
    assert_eq!(
        result.payload["affectedItemIds"],
        json!(["item-1", "item-1-audio"])
    );
    assert_eq!(
        result.payload["updates"],
        json!([
            {
                "itemId": "item-1",
                "durationSeconds": 2.0,
                "speed": 2.0
            },
            {
                "itemId": "item-1-audio",
                "durationSeconds": 2.0,
                "speed": 2.0
            }
        ])
    );

    let persisted_project =
        load_split_project(project_dir.path()).expect("load linked speed project");
    let items = persisted_project
        .timeline
        .tracks
        .iter()
        .flat_map(|track| track.items.iter())
        .filter(|item| item.id == "item-1" || item.id == "item-1-audio")
        .collect::<Vec<_>>();
    assert_eq!(items.len(), 2);
    for item in items {
        assert_eq!(item.duration_seconds, 2.0);
        assert_eq!(item.properties["speed"], json!(2.0));
    }
}

#[test]
fn codex_set_clip_properties_persists_palmier_speed_metadata() {
    let project = sample_project();
    let project_dir = tempfile::tempdir().expect("speed metadata project dir");
    save_split_project(project_dir.path(), &project).expect("save speed metadata project");

    let result = call_codex_local_tool(
        &project,
        "set_clip_properties",
        json!({
            "projectDir": project_dir.path().display().to_string(),
            "clipIds": ["item-1"],
            "speed": 2.0
        }),
    )
    .expect("Palmier speed property should validate");

    assert!(result.mutates_project);
    assert_eq!(result.payload["applied"], json!(true));
    assert_eq!(result.payload["affectedItemIds"], json!(["item-1"]));
    assert_eq!(
        result.payload["updates"][0],
        json!({
            "itemId": "item-1",
            "durationSeconds": 2.0,
            "speed": 2.0
        })
    );

    let persisted_project =
        load_split_project(project_dir.path()).expect("load speed metadata project");
    let item = persisted_project
        .timeline
        .tracks
        .iter()
        .flat_map(|track| track.items.iter())
        .find(|item| item.id == "item-1")
        .expect("persisted speed item");
    assert_eq!(item.duration_seconds, 2.0);
    assert_eq!(item.properties["speed"], json!(2.0));
}

#[test]
fn codex_set_clip_properties_rescales_from_existing_palmier_speed() {
    let mut project = sample_project();
    project.timeline.tracks[0].items[0].duration_seconds = 4.0;
    project.timeline.tracks[0].items[0]
        .properties
        .insert("speed".to_string(), json!(2.0));
    let project_dir = tempfile::tempdir().expect("retime project dir");
    save_split_project(project_dir.path(), &project).expect("save retime project");

    let result = call_codex_local_tool(
        &project,
        "set_clip_properties",
        json!({
            "projectDir": project_dir.path().display().to_string(),
            "clipIds": ["item-1"],
            "speed": 4.0
        }),
    )
    .expect("Palmier speed retime should preserve consumed source range");

    assert!(result.mutates_project);
    assert_eq!(result.payload["applied"], json!(true));
    assert_eq!(
        result.payload["updates"][0],
        json!({
            "itemId": "item-1",
            "durationSeconds": 2.0,
            "speed": 4.0
        })
    );

    let persisted_project = load_split_project(project_dir.path()).expect("load retime project");
    let item = persisted_project
        .timeline
        .tracks
        .iter()
        .flat_map(|track| track.items.iter())
        .find(|item| item.id == "item-1")
        .expect("persisted retimed item");
    assert_eq!(item.duration_seconds, 2.0);
    assert_eq!(item.properties["speed"], json!(4.0));
}

fn project_with_ranged_item_1(
    speed: Option<f64>,
    source_in: f64,
    source_out: f64,
) -> video_creater_lib::project::model::VideoProject {
    let mut project = sample_project();
    let item = &mut project.timeline.tracks[0].items[0];
    item.duration_seconds = 4.0;
    item.properties
        .insert("sourceIn".to_string(), json!(source_in));
    item.properties
        .insert("sourceOut".to_string(), json!(source_out));
    if let Some(speed) = speed {
        item.properties.insert("speed".to_string(), json!(speed));
    }
    project
}

fn apply_set_clip_properties_and_load_item_1(
    project: &video_creater_lib::project::model::VideoProject,
    args: Value,
) -> TimelineItem {
    let project_dir = tempfile::tempdir().expect("clip timing project dir");
    save_split_project(project_dir.path(), project).expect("save clip timing project");
    let mut args = args;
    args["projectDir"] = json!(project_dir.path().display().to_string());

    let result = call_codex_local_tool(project, "set_clip_properties", args)
        .expect("clip timing update should apply");
    assert!(result.mutates_project);
    assert_eq!(result.payload["applied"], json!(true));

    load_split_project(project_dir.path())
        .expect("load clip timing project")
        .timeline
        .tracks
        .iter()
        .flat_map(|track| track.items.iter())
        .find(|item| item.id == "item-1")
        .cloned()
        .expect("persisted item-1")
}

#[test]
fn codex_set_clip_properties_scales_duration_source_range_by_item_speed() {
    let project = project_with_ranged_item_1(Some(2.0), 1.0, 9.0);

    let item = apply_set_clip_properties_and_load_item_1(
        &project,
        json!({
            "updates": [
                {
                    "itemId": "item-1",
                    "sourceIn": 1.0,
                    "durationSeconds": 2.0
                }
            ]
        }),
    );

    assert_eq!(item.duration_seconds, 2.0);
    assert_eq!(item.properties["sourceIn"], json!(1.0));
    assert_eq!(item.properties["sourceOut"], json!(5.0));
    assert_eq!(item.properties["speed"], json!(2.0));
}

#[test]
fn codex_set_clip_properties_scales_source_out_duration_by_item_speed() {
    let project = project_with_ranged_item_1(Some(2.0), 1.0, 9.0);

    let item = apply_set_clip_properties_and_load_item_1(
        &project,
        json!({
            "updates": [
                {
                    "itemId": "item-1",
                    "sourceOut": 5.0
                }
            ]
        }),
    );

    assert_eq!(item.duration_seconds, 2.0);
    assert_eq!(item.properties["sourceIn"], json!(1.0));
    assert_eq!(item.properties["sourceOut"], json!(5.0));
}

#[test]
fn codex_set_clip_properties_keeps_unit_speed_source_range_math() {
    let project = project_with_ranged_item_1(None, 1.0, 5.0);

    let duration_item = apply_set_clip_properties_and_load_item_1(
        &project,
        json!({
            "updates": [
                {
                    "itemId": "item-1",
                    "sourceIn": 1.0,
                    "durationSeconds": 2.0
                }
            ]
        }),
    );
    assert_eq!(duration_item.duration_seconds, 2.0);
    assert_eq!(duration_item.properties["sourceOut"], json!(3.0));

    let source_out_item = apply_set_clip_properties_and_load_item_1(
        &project,
        json!({
            "updates": [
                {
                    "itemId": "item-1",
                    "sourceOut": 3.0
                }
            ]
        }),
    );
    assert_eq!(source_out_item.duration_seconds, 2.0);
    assert_eq!(source_out_item.properties["sourceIn"], json!(1.0));
    assert_eq!(source_out_item.properties["sourceOut"], json!(3.0));
}

#[test]
fn codex_set_clip_properties_uses_payload_speed_for_palmier_trim_ranges() {
    let project = project_with_ranged_item_1(None, 0.0, 4.0);

    let duration_item = apply_set_clip_properties_and_load_item_1(
        &project,
        json!({
            "clipIds": ["item-1"],
            "durationFrames": 48,
            "trimStartFrame": 24,
            "speed": 2.0
        }),
    );
    assert_eq!(duration_item.duration_seconds, 2.0);
    assert_eq!(duration_item.properties["sourceIn"], json!(1.0));
    assert_eq!(duration_item.properties["sourceOut"], json!(5.0));
    assert_eq!(duration_item.properties["speed"], json!(2.0));

    let source_out_item = apply_set_clip_properties_and_load_item_1(
        &project,
        json!({
            "clipIds": ["item-1"],
            "trimStartFrame": 24,
            "trimEndFrame": 168,
            "speed": 2.0
        }),
    );
    assert_eq!(source_out_item.duration_seconds, 3.0);
    assert_eq!(source_out_item.properties["sourceIn"], json!(1.0));
    assert_eq!(source_out_item.properties["sourceOut"], json!(7.0));
    assert_eq!(source_out_item.properties["speed"], json!(2.0));
}

#[test]
fn codex_set_clip_properties_speed_retime_rescales_existing_keyframes() {
    let mut project = sample_project();
    project.timeline.tracks[0].items[0].duration_seconds = 4.0;
    project.timeline.tracks[0].items[0]
        .properties
        .insert("speed".to_string(), json!(2.0));
    project.timeline.tracks[0].items[0].properties.insert(
        "keyframes".to_string(),
        json!({
            "opacity": [
                { "atSeconds": 0.0, "value": 1.0 },
                { "atSeconds": 2.0, "value": 0.6 },
                { "atSeconds": 4.0, "value": 0.2 }
            ]
        }),
    );
    let project_dir = tempfile::tempdir().expect("retime keyframe project dir");
    save_split_project(project_dir.path(), &project).expect("save retime keyframe project");

    let result = call_codex_local_tool(
        &project,
        "set_clip_properties",
        json!({
            "projectDir": project_dir.path().display().to_string(),
            "clipIds": ["item-1"],
            "speed": 4.0
        }),
    )
    .expect("Palmier speed retime should rescale keyframes");

    assert!(result.mutates_project);
    assert_eq!(result.payload["applied"], json!(true));

    let persisted_project =
        load_split_project(project_dir.path()).expect("load retime keyframe project");
    let item = persisted_project
        .timeline
        .tracks
        .iter()
        .flat_map(|track| track.items.iter())
        .find(|item| item.id == "item-1")
        .expect("persisted retimed keyframe item");
    assert_eq!(item.duration_seconds, 2.0);
    assert_eq!(
        item.properties["keyframes"]["opacity"],
        json!([
            { "atSeconds": 0.0, "value": 1.0 },
            { "atSeconds": 1.0, "value": 0.6 },
            { "atSeconds": 2.0, "value": 0.2 }
        ])
    );
}

#[test]
fn codex_set_clip_properties_maps_palmier_volume_to_audio_decibels() {
    let mut project = sample_project();
    project.timeline.tracks[0].items.push(TimelineItem {
        id: "audio-1".to_string(),
        kind: TimelineItemKind::AudioClip,
        start_seconds: 0.0,
        duration_seconds: 4.0,
        source: TimelineSource::Media {
            media_id: "media-1".to_string(),
        },
        label: "Audio".to_string(),
        properties: BTreeMap::new(),
    });

    let result = call_codex_local_tool(
        &project,
        "set_clip_properties",
        json!({
            "clipIds": ["audio-1"],
            "volume": 0.5
        }),
    )
    .expect("Palmier volume property should validate");

    assert!(result.mutates_project);
    assert_eq!(result.payload["valid"], json!(true));
    assert_eq!(result.payload["affectedItemIds"], json!(["audio-1"]));
    assert_eq!(
        result.payload["updates"][0],
        json!({
            "itemId": "audio-1",
            "volumeDb": -6.021
        })
    );
}

#[test]
fn codex_set_clip_properties_maps_palmier_fade_out_frames_to_audio_fade() {
    let mut project = sample_project();
    project.timeline.tracks[0].items.push(TimelineItem {
        id: "item-audio".to_string(),
        kind: TimelineItemKind::AudioClip,
        start_seconds: 0.0,
        duration_seconds: 4.0,
        source: TimelineSource::Media {
            media_id: "media-1".to_string(),
        },
        label: "Audio".to_string(),
        properties: BTreeMap::new(),
    });
    let project_dir = tempfile::tempdir().expect("fade project dir");
    save_split_project(project_dir.path(), &project).expect("save fade project");

    let result = call_codex_local_tool(
        &project,
        "set_clip_properties",
        json!({
            "projectDir": project_dir.path().display().to_string(),
            "clipIds": ["item-audio"],
            "fadeOutFrames": 36
        }),
    )
    .expect("Palmier fadeOutFrames should map to audio fade seconds");

    assert!(result.mutates_project);
    assert_eq!(result.payload["applied"], json!(true));
    assert_eq!(result.payload["affectedItemIds"], json!(["item-audio"]));
    assert_eq!(result.payload["updates"][0]["fadeOutSeconds"], json!(1.5));

    let persisted_project = load_split_project(project_dir.path()).expect("load fade project");
    let persisted_item = persisted_project
        .timeline
        .tracks
        .iter()
        .flat_map(|track| track.items.iter())
        .find(|item| item.id == "item-audio")
        .expect("persisted audio item");
    assert_eq!(persisted_item.properties["fadeOutSeconds"], json!(1.5));
}

#[test]
fn codex_set_clip_properties_scalar_volume_clears_existing_keyframes() {
    let mut project = sample_project();
    project.timeline.tracks[0].items.push(TimelineItem {
        id: "audio-1".to_string(),
        kind: TimelineItemKind::AudioClip,
        start_seconds: 0.0,
        duration_seconds: 4.0,
        source: TimelineSource::Media {
            media_id: "media-1".to_string(),
        },
        label: "Audio".to_string(),
        properties: BTreeMap::from([(
            "keyframes".to_string(),
            json!({
                "volumeDb": [
                    { "atSeconds": 0.0, "value": 0.0 },
                    { "atSeconds": 1.0, "value": -60.0 }
                ]
            }),
        )]),
    });

    let result = call_codex_local_tool(
        &project,
        "set_clip_properties",
        json!({
            "clipIds": ["audio-1"],
            "volume": 0.5
        }),
    )
    .expect("Palmier scalar volume should clear existing volume keyframes");

    assert!(result.mutates_project);
    assert_eq!(result.payload["valid"], json!(true));
    assert_eq!(result.payload["actionCount"], json!(2));
    assert_eq!(
        result.payload["clearedKeyframes"],
        json!([
            {
                "itemId": "audio-1",
                "property": "volumeDb"
            }
        ])
    );
}

#[test]
fn codex_set_clip_properties_scalar_opacity_clears_existing_keyframes() {
    let mut project = sample_project();
    project.timeline.tracks[0].items[0].properties.insert(
        "keyframes".to_string(),
        json!({
            "opacity": [
                { "atSeconds": 0.0, "value": 1.0 },
                { "atSeconds": 1.0, "value": 0.0 }
            ]
        }),
    );

    let result = call_codex_local_tool(
        &project,
        "set_clip_properties",
        json!({
            "clipIds": ["item-1"],
            "opacity": 0.4
        }),
    )
    .expect("scalar opacity should clear existing opacity keyframes");

    assert!(result.mutates_project);
    assert_eq!(result.payload["valid"], json!(true));
    assert_eq!(result.payload["actionCount"], json!(2));
    assert_eq!(
        result.payload["clearedKeyframes"],
        json!([
            {
                "itemId": "item-1",
                "property": "opacity"
            }
        ])
    );
}

#[test]
fn codex_set_clip_properties_scalar_transform_clears_existing_transform_keyframes() {
    let mut project = sample_project();
    project.timeline.tracks[0].items[0].properties.insert(
        "keyframes".to_string(),
        json!({
            "positionX": [
                { "atSeconds": 0.0, "value": 0.25 },
                { "atSeconds": 1.0, "value": 0.75 }
            ],
            "positionY": [
                { "atSeconds": 0.0, "value": 0.2 },
                { "atSeconds": 1.0, "value": 0.8 }
            ],
            "scaleX": [
                { "atSeconds": 0.0, "value": 1.0 },
                { "atSeconds": 1.0, "value": 0.65 }
            ],
            "scaleY": [
                { "atSeconds": 0.0, "value": 1.0 },
                { "atSeconds": 1.0, "value": 0.7 }
            ],
            "opacity": [
                { "atSeconds": 0.0, "value": 1.0 },
                { "atSeconds": 1.0, "value": 0.5 }
            ]
        }),
    );
    let project_dir = tempfile::tempdir().expect("transform project dir");
    save_split_project(project_dir.path(), &project).expect("save transform project");

    let result = call_codex_local_tool(
        &project,
        "set_clip_properties",
        json!({
            "projectDir": project_dir.path().display().to_string(),
            "clipIds": ["item-1"],
            "transform": {
                "centerX": 0.5,
                "centerY": 0.5,
                "width": 0.8,
                "height": 0.75
            }
        }),
    )
    .expect("scalar transform should clear existing transform keyframes");

    assert!(result.mutates_project);
    assert_eq!(result.payload["applied"], json!(true));
    assert_eq!(result.payload["actionCount"], json!(5));
    assert_eq!(
        result.payload["clearedKeyframes"],
        json!([
            {
                "itemId": "item-1",
                "property": "positionX"
            },
            {
                "itemId": "item-1",
                "property": "positionY"
            },
            {
                "itemId": "item-1",
                "property": "scaleX"
            },
            {
                "itemId": "item-1",
                "property": "scaleY"
            }
        ])
    );

    let persisted_project = load_split_project(project_dir.path()).expect("load transform project");
    let persisted_item = persisted_project
        .timeline
        .tracks
        .iter()
        .flat_map(|track| track.items.iter())
        .find(|item| item.id == "item-1")
        .expect("persisted transform item");
    assert_eq!(
        persisted_item.properties["transform"],
        json!({
            "centerX": 0.5,
            "centerY": 0.5,
            "width": 0.8,
            "height": 0.75
        })
    );
    assert!(persisted_item.properties["keyframes"]["opacity"].is_array());
    assert!(persisted_item.properties["keyframes"]
        .get("positionX")
        .is_none());
    assert!(persisted_item.properties["keyframes"]
        .get("positionY")
        .is_none());
    assert!(persisted_item.properties["keyframes"]
        .get("scaleX")
        .is_none());
    assert!(persisted_item.properties["keyframes"]
        .get("scaleY")
        .is_none());
}

#[test]
fn codex_set_clip_properties_accepts_palmier_text_style_fields() {
    let mut project = sample_project();
    project.timeline.tracks[0].items.push(TimelineItem {
        id: "overlay-1".to_string(),
        kind: TimelineItemKind::Overlay,
        start_seconds: 1.0,
        duration_seconds: 2.0,
        source: TimelineSource::Text {
            text: "Old title".to_string(),
        },
        label: "Old title".to_string(),
        properties: BTreeMap::from([
            ("text".to_string(), json!("Old title")),
            (
                "visualTreatment".to_string(),
                json!("compact lower third with translucent backing and accent stroke"),
            ),
            (
                "motion".to_string(),
                json!("quick slide-in, two-beat hold, soft fade-out"),
            ),
            (
                "safeZone".to_string(),
                json!("keep inside 10% margins and away from faces"),
            ),
            (
                "avoid".to_string(),
                json!("full-width opaque black slabs and default-font boxes"),
            ),
        ]),
    });

    let result = call_codex_local_tool(
        &project,
        "set_clip_properties",
        json!({
            "clipIds": ["overlay-1"],
            "content": "Launch title",
            "fontName": "Helvetica-Bold",
            "fontSize": 72,
            "color": "#ffffff",
            "alignment": "center"
        }),
    )
    .expect("Palmier text style properties should validate");

    assert!(result.mutates_project);
    assert_eq!(result.payload["valid"], json!(true));
    assert_eq!(result.payload["actionCount"], json!(1));
    assert_eq!(result.payload["affectedItemIds"], json!(["overlay-1"]));
    assert_eq!(
        result.payload["updates"][0],
        json!({
            "itemId": "overlay-1",
            "text": "Launch title",
            "fontName": "Helvetica-Bold",
            "fontSize": 72.0,
            "color": "#ffffff",
            "alignment": "center"
        })
    );
}

#[test]
fn codex_update_text_updates_overlay_content_and_style_persistently() {
    let mut project = sample_project();
    project.timeline.tracks[0].items.push(TimelineItem {
        id: "overlay-title".to_string(),
        kind: TimelineItemKind::Overlay,
        start_seconds: 1.0,
        duration_seconds: 2.0,
        source: TimelineSource::Text {
            text: "Old title".to_string(),
        },
        label: "Old title".to_string(),
        properties: BTreeMap::from([
            ("text".to_string(), json!("Old title")),
            (
                "visualTreatment".to_string(),
                json!("compact title with transparent backing"),
            ),
            ("motion".to_string(), json!("quick slide-in")),
            ("safeZone".to_string(), json!("inside 10% margins")),
            ("avoid".to_string(), json!("opaque black slabs")),
        ]),
    });
    let project_dir = tempfile::tempdir().expect("project dir");
    save_split_project(project_dir.path(), &project).expect("save split project");

    let result = call_codex_local_tool(
        &project,
        "update_text",
        json!({
            "projectDir": project_dir.path().display().to_string(),
            "clipIds": ["overlay-title"],
            "content": "Launch title",
            "fontName": "Helvetica-Bold",
            "fontSize": 72,
            "color": "#ffffff",
            "alignment": "center"
        }),
    )
    .expect("Palmier update_text should update overlay text");

    assert!(result.mutates_project);
    assert_eq!(result.payload["valid"], json!(true));
    assert_eq!(result.payload["actionCount"], json!(1));
    assert_eq!(result.payload["affectedItemIds"], json!(["overlay-title"]));
    assert_eq!(
        result.payload["updates"][0],
        json!({
            "itemId": "overlay-title",
            "text": "Launch title",
            "fontName": "Helvetica-Bold",
            "fontSize": 72.0,
            "color": "#ffffff",
            "alignment": "center"
        })
    );

    let saved = load_split_project(project_dir.path()).expect("load updated project");
    let saved_item = saved.timeline.tracks[0]
        .items
        .iter()
        .find(|item| item.id == "overlay-title")
        .expect("saved overlay item");
    assert_eq!(saved_item.label, "Launch title");
    assert_eq!(
        saved_item.source,
        TimelineSource::Text {
            text: "Launch title".to_string()
        }
    );
    assert_eq!(saved_item.properties["fontName"], json!("Helvetica-Bold"));
    assert_eq!(saved_item.properties["fontSize"], json!(72.0));
    assert_eq!(saved_item.properties["color"], json!("#ffffff"));
    assert_eq!(saved_item.properties["alignment"], json!("center"));
}

#[test]
fn codex_update_text_updates_caption_group_content_persistently() {
    let mut project = sample_project();
    let caption_track = project
        .timeline
        .tracks
        .iter_mut()
        .find(|track| track.kind == TrackKind::Caption)
        .expect("caption track should exist");
    for (id, text, start_seconds) in [("caption-a", "First", 0.5), ("caption-b", "Second", 1.25)] {
        caption_track.items.push(TimelineItem {
            id: id.to_string(),
            kind: TimelineItemKind::Caption,
            start_seconds,
            duration_seconds: 0.5,
            source: TimelineSource::Text {
                text: text.to_string(),
            },
            label: text.to_string(),
            properties: BTreeMap::from([("captionGroupId".to_string(), json!("captions-main"))]),
        });
    }
    let project_dir = tempfile::tempdir().expect("project dir");
    save_split_project(project_dir.path(), &project).expect("save split project");

    let result = call_codex_local_tool(
        &project,
        "video_creater.update_text",
        json!({
            "projectDir": project_dir.path().display().to_string(),
            "captionGroupId": "captions-main",
            "content": "Updated caption"
        }),
    )
    .expect("Palmier update_text should expand caption groups");

    assert!(result.mutates_project);
    assert_eq!(result.payload["valid"], json!(true));
    assert_eq!(result.payload["actionCount"], json!(2));
    assert_eq!(
        result.payload["affectedItemIds"],
        json!(["caption-a", "caption-b"])
    );

    let saved = load_split_project(project_dir.path()).expect("load updated project");
    let caption_items = saved
        .timeline
        .tracks
        .iter()
        .find(|track| track.kind == TrackKind::Caption)
        .expect("caption track")
        .items
        .iter()
        .filter(|item| item.properties.get("captionGroupId") == Some(&json!("captions-main")))
        .collect::<Vec<_>>();
    assert_eq!(caption_items.len(), 2);
    for item in caption_items {
        assert_eq!(
            item.source,
            TimelineSource::Text {
                text: "Updated caption".to_string()
            }
        );
        assert_eq!(item.properties["textEdited"], json!(true));
    }
}

#[test]
fn codex_set_clip_properties_accepts_palmier_transform_fields() {
    let project = sample_project();

    let result = call_codex_local_tool(
        &project,
        "set_clip_properties",
        json!({
            "clipIds": ["item-1"],
            "transform": {
                "centerX": 0.5,
                "centerY": 0.82,
                "width": 0.75,
                "height": 0.5,
                "flipHorizontal": true
            }
        }),
    )
    .expect("Palmier transform properties should validate");

    assert!(result.mutates_project);
    assert_eq!(result.payload["valid"], json!(true));
    assert_eq!(result.payload["actionCount"], json!(1));
    assert_eq!(result.payload["affectedItemIds"], json!(["item-1"]));
    assert_eq!(
        result.payload["updates"][0]["transform"],
        json!({
            "centerX": 0.5,
            "centerY": 0.82,
            "width": 0.75,
            "height": 0.5,
            "flipHorizontal": true
        })
    );
}

#[test]
fn codex_apply_layout_relayouts_existing_visual_clips() {
    let mut project = sample_project();
    project.media.push(MediaAsset {
        id: "media-2".to_string(),
        name: Some("Reaction angle".to_string()),
        relative_path: "media/reaction.mp4".to_string(),
        kind: MediaKind::Video,
        duration_seconds: 8.0,
        width: Some(1920),
        height: Some(1080),
        fps: Some(24.0),
        folder_id: None,
    });
    project.timeline.tracks.insert(
        1,
        TimelineTrack::empty("track-video-b", "Video B", TrackKind::Video),
    );
    project.timeline.tracks[1].items.push(TimelineItem {
        id: "item-2".to_string(),
        kind: TimelineItemKind::VideoClip,
        start_seconds: 0.0,
        duration_seconds: 4.0,
        source: TimelineSource::Media {
            media_id: "media-2".to_string(),
        },
        label: "Reaction clip".to_string(),
        properties: BTreeMap::from([(
            "keyframes".to_string(),
            json!({
                "positionX": [{ "atSeconds": 0.0, "value": 0.1 }],
                "scaleX": [{ "atSeconds": 0.0, "value": 0.6 }],
                "cropLeft": [{ "atSeconds": 0.0, "value": 0.1 }]
            }),
        )]),
    });

    let result = call_codex_local_tool(
        &project,
        "apply_layout",
        json!({
            "layout": "side_by_side",
            "slots": [
                { "slot": "left", "clipIds": ["item-1"] },
                { "slot": "right", "clipIds": ["item-2"], "anchor": "right" }
            ]
        }),
    )
    .expect("Palmier apply_layout should relayout existing clips");

    assert!(result.mutates_project);
    assert_eq!(result.payload["valid"], json!(true));
    assert_eq!(result.payload["layout"], json!("side_by_side"));
    assert_eq!(result.payload["mode"], json!("clipIds"));
    assert_eq!(result.payload["fit"], json!("fill"));
    assert_eq!(
        result.payload["affectedItemIds"],
        json!(["item-1", "item-2"])
    );
    assert_eq!(
        result.payload["slotUpdates"][0],
        json!({
            "slot": "left",
            "itemId": "item-1",
            "transform": {
                "centerX": 0.25,
                "centerY": 0.5,
                "width": 0.5,
                "height": 1.0
            },
            "crop": {
                "top": 0.0,
                "right": 0.25,
                "bottom": 0.0,
                "left": 0.25
            }
        })
    );
    assert_eq!(
        result.payload["slotUpdates"][1]["crop"],
        json!({
            "top": 0.0,
            "right": 0.0,
            "bottom": 0.0,
            "left": 0.5
        })
    );
    assert_eq!(
        result.payload["nextRecommendedInspection"],
        json!("video_creater.inspect_timeline")
    );
}

#[test]
fn codex_apply_layout_places_visual_media_refs() {
    let mut project = sample_project();
    project.media.push(MediaAsset {
        id: "media-image-1".to_string(),
        name: Some("Chart still".to_string()),
        relative_path: "media/chart.png".to_string(),
        kind: MediaKind::Image,
        duration_seconds: 0.0,
        width: Some(1080),
        height: Some(1080),
        fps: None,
        folder_id: None,
    });

    let result = call_codex_local_tool(
        &project,
        "video_creater.apply_layout",
        json!({
            "layout": "pip_bottom_right",
            "fit": "fit",
            "startFrame": 24,
            "durationFrames": 48,
            "slots": [
                { "slot": "main", "mediaRef": "media-1" },
                { "slot": "inset", "mediaRef": "media-image-1", "anchorX": 1.0, "anchorY": 1.0 }
            ]
        }),
    )
    .expect("Palmier apply_layout should place visual media refs");

    assert!(result.mutates_project);
    assert_eq!(result.payload["valid"], json!(true));
    assert_eq!(result.payload["mode"], json!("mediaRef"));
    assert_eq!(
        result.payload["createdTrackIds"],
        json!([
            "track-palmier-layout-main-1",
            "track-palmier-layout-inset-1"
        ])
    );
    assert_eq!(
        result.payload["affectedItemIds"],
        json!(["layout-main-24", "layout-main-24-audio", "layout-inset-24"])
    );
    assert_eq!(
        result.payload["createdVisualItemIds"],
        json!(["layout-main-24", "layout-inset-24"])
    );
    assert_eq!(
        result.payload["createdAudioItemIds"],
        json!(["layout-main-24-audio"])
    );
    assert_eq!(
        result.payload["slotUpdates"][1]["transform"],
        json!({
            "centerX": 0.886,
            "centerY": 0.825,
            "width": 0.158,
            "height": 0.28
        })
    );
    assert_eq!(result.payload["projectAfter"]["timelineItems"], json!(4));
}

#[test]
fn codex_apply_layout_rejects_missing_slots() {
    let project = sample_project();

    let error = call_codex_local_tool(
        &project,
        "apply_layout",
        json!({
            "layout": "side_by_side",
            "slots": [
                { "slot": "left", "clipIds": ["item-1"] }
            ]
        }),
    )
    .expect_err("apply_layout should require every slot");

    assert!(error.to_string().contains("needs every slot filled"));
}

#[test]
fn codex_remove_words_maps_transcript_indexes_to_ripple_ranges() {
    let mut project = sample_project();
    project.transcripts.push(Transcript {
        id: "transcript-1".to_string(),
        media_id: "media-1".to_string(),
        engine: Some("fixture".to_string()),
        raw_artifact_path: None,
        repairs: Vec::new(),
        segments: Vec::new(),
        words: vec![
            TranscriptWord {
                text: "keep".to_string(),
                start_seconds: 0.1,
                end_seconds: 0.5,
                confidence: Some(0.99),
                speaker: None,
            },
            TranscriptWord {
                text: "remove".to_string(),
                start_seconds: 1.0,
                end_seconds: 1.5,
                confidence: Some(0.98),
                speaker: None,
            },
            TranscriptWord {
                text: "after".to_string(),
                start_seconds: 2.0,
                end_seconds: 2.4,
                confidence: Some(0.97),
                speaker: None,
            },
        ],
    });

    let result = call_codex_local_tool(
        &project,
        "video_creater.remove_words",
        json!({
            "mediaId": "media-1",
            "wordIndexes": [1]
        }),
    )
    .expect("direct remove words args should validate");

    assert!(result.mutates_project);
    assert_eq!(result.payload["valid"], json!(true));
    assert_eq!(result.payload["actionCount"], json!(1));
    assert_eq!(result.payload["projectAfter"]["timelineItems"], json!(2));
    assert_eq!(
        result.payload["removedWordRanges"][0]["wordIndex"],
        json!(1)
    );
    assert_eq!(
        result.payload["removedWordRanges"][0]["timelineStartSeconds"],
        json!(1.0)
    );
    assert_eq!(
        result.payload["removedWordRanges"][0]["timelineEndSeconds"],
        json!(1.5)
    );

    let project_dir = tempfile::tempdir().expect("remove words project dir");
    save_split_project(project_dir.path(), &project).expect("save remove words project");
    let persisted = call_codex_local_tool(
        &project,
        "video_creater.remove_words",
        json!({
            "projectDir": project_dir.path().display().to_string(),
            "mediaId": "media-1",
            "wordIndexes": [1]
        }),
    )
    .expect("direct remove words args should persist");
    assert!(persisted.mutates_project);
    assert_eq!(persisted.payload["applied"], json!(true));
    assert_eq!(persisted.payload["agentHistory"]["entryCount"], json!(1));
    let persisted_project =
        load_split_project(project_dir.path()).expect("load remove words project");
    let persisted_item_count = persisted_project
        .timeline
        .tracks
        .iter()
        .flat_map(|track| track.items.iter())
        .count();
    assert_eq!(persisted_item_count, 2);
}

#[test]
fn codex_remove_words_accepts_palmier_word_indices_without_media_id() {
    let mut project = sample_project();
    project.transcripts.push(Transcript {
        id: "transcript-1".to_string(),
        media_id: "media-1".to_string(),
        engine: Some("fixture".to_string()),
        raw_artifact_path: None,
        repairs: Vec::new(),
        segments: Vec::new(),
        words: vec![
            TranscriptWord {
                text: "keep".to_string(),
                start_seconds: 0.1,
                end_seconds: 0.5,
                confidence: Some(0.99),
                speaker: None,
            },
            TranscriptWord {
                text: "remove".to_string(),
                start_seconds: 1.0,
                end_seconds: 1.5,
                confidence: Some(0.98),
                speaker: None,
            },
        ],
    });

    let result = call_codex_local_tool(
        &project,
        "remove_words",
        json!({
            "words": [1]
        }),
    )
    .expect("Palmier remove_words args should validate");

    assert!(result.mutates_project);
    assert_eq!(result.payload["valid"], json!(true));
    assert_eq!(result.payload["projectAfter"]["timelineItems"], json!(2));
    assert_eq!(
        result.payload["removedWordRanges"][0]["mediaId"],
        json!("media-1")
    );
    assert_eq!(
        result.payload["removedWordRanges"][0]["wordIndex"],
        json!(1)
    );
}

#[test]
fn codex_remove_words_expands_palmier_link_group_tracks() {
    let mut project = sample_project();
    project.media.push(MediaAsset {
        id: "media-audio-1".to_string(),
        name: Some("Opening mic".to_string()),
        relative_path: "media/input.wav".to_string(),
        kind: MediaKind::Audio,
        duration_seconds: 12.0,
        width: None,
        height: None,
        fps: None,
        folder_id: None,
    });
    project.timeline.tracks[0].items[0]
        .properties
        .insert("linkGroupId".to_string(), json!("link-item-1"));
    project.timeline.tracks[4].items.push(TimelineItem {
        id: "item-1-audio".to_string(),
        kind: TimelineItemKind::AudioClip,
        start_seconds: 0.0,
        duration_seconds: 4.0,
        source: TimelineSource::Media {
            media_id: "media-audio-1".to_string(),
        },
        label: "Opening clip audio".to_string(),
        properties: BTreeMap::from([
            ("linkGroupId".to_string(), json!("link-item-1")),
            ("sourceClipType".to_string(), json!("audio")),
        ]),
    });
    project.transcripts.push(Transcript {
        id: "transcript-1".to_string(),
        media_id: "media-1".to_string(),
        engine: Some("fixture".to_string()),
        raw_artifact_path: None,
        repairs: Vec::new(),
        segments: Vec::new(),
        words: vec![
            TranscriptWord {
                text: "keep".to_string(),
                start_seconds: 0.1,
                end_seconds: 0.5,
                confidence: Some(0.99),
                speaker: None,
            },
            TranscriptWord {
                text: "remove".to_string(),
                start_seconds: 1.0,
                end_seconds: 1.5,
                confidence: Some(0.98),
                speaker: None,
            },
        ],
    });

    let result = call_codex_local_tool(&project, "remove_words", json!({ "words": [1] }))
        .expect("linked Palmier remove_words should validate");

    assert!(result.mutates_project);
    assert_eq!(result.payload["valid"], json!(true));
    assert_eq!(
        result.payload["affectedTrackIds"],
        json!(["track-audio", "track-video"])
    );
    assert_eq!(
        result.payload["ranges"][0],
        json!({
            "startSeconds": 1.0,
            "endSeconds": 1.5,
            "trackIds": ["track-audio", "track-video"]
        })
    );
    assert_eq!(result.payload["projectAfter"]["timelineItems"], json!(4));
}

#[test]
fn codex_remove_silence_maps_timeline_transcript_gaps_to_linked_ripple_ranges() {
    let mut project = sample_project();
    project.media.push(MediaAsset {
        id: "media-audio-1".to_string(),
        name: Some("Opening mic".to_string()),
        relative_path: "media/input.wav".to_string(),
        kind: MediaKind::Audio,
        duration_seconds: 12.0,
        width: None,
        height: None,
        fps: None,
        folder_id: None,
    });
    project.timeline.tracks[0].items[0]
        .properties
        .insert("linkGroupId".to_string(), json!("link-item-1"));
    project.timeline.tracks[4].items.push(TimelineItem {
        id: "item-1-audio".to_string(),
        kind: TimelineItemKind::AudioClip,
        start_seconds: 0.0,
        duration_seconds: 4.0,
        source: TimelineSource::Media {
            media_id: "media-audio-1".to_string(),
        },
        label: "Opening clip audio".to_string(),
        properties: BTreeMap::from([
            ("linkGroupId".to_string(), json!("link-item-1")),
            ("sourceClipType".to_string(), json!("audio")),
        ]),
    });
    project.transcripts.push(Transcript {
        id: "transcript-1".to_string(),
        media_id: "media-1".to_string(),
        engine: Some("fixture".to_string()),
        raw_artifact_path: None,
        repairs: Vec::new(),
        segments: Vec::new(),
        words: vec![
            TranscriptWord {
                text: "before".to_string(),
                start_seconds: 0.1,
                end_seconds: 0.5,
                confidence: Some(0.99),
                speaker: None,
            },
            TranscriptWord {
                text: "after".to_string(),
                start_seconds: 2.0,
                end_seconds: 2.3,
                confidence: Some(0.98),
                speaker: None,
            },
            TranscriptWord {
                text: "kept".to_string(),
                start_seconds: 2.6,
                end_seconds: 2.8,
                confidence: Some(0.97),
                speaker: None,
            },
        ],
    });

    let result = call_codex_local_tool(&project, "remove_silence", json!({}))
        .expect("Palmier remove_silence should derive transcript dead-air ranges");

    assert!(result.mutates_project);
    assert_eq!(result.payload["valid"], json!(true));
    assert_eq!(result.payload["actionCount"], json!(1));
    assert_eq!(result.payload["sectionsRemoved"], json!(1));
    assert_eq!(
        result.payload["affectedTrackIds"],
        json!(["track-audio", "track-video"])
    );
    assert_eq!(
        result.payload["ranges"][0],
        json!({
            "startSeconds": 0.62,
            "endSeconds": 1.88,
            "trackIds": ["track-audio", "track-video"]
        })
    );
    assert_eq!(
        result.payload["removedSilenceRanges"][0],
        json!({
            "mediaId": "media-1",
            "transcriptId": "transcript-1",
            "trackId": "track-video",
            "itemId": "item-1",
            "previousWordIndex": 0,
            "nextWordIndex": 1,
            "timelineStartSeconds": 0.62,
            "timelineEndSeconds": 1.88,
            "sourceStartSeconds": 0.62,
            "sourceEndSeconds": 1.88,
            "gapSeconds": 1.5
        })
    );
    assert_eq!(
        result.payload["nextRecommendedInspection"],
        json!("video_creater.get_timeline")
    );
    assert_eq!(result.payload["projectAfter"]["timelineItems"], json!(4));
}

#[test]
fn codex_remove_silence_uses_stored_dead_air_without_transcript() {
    let mut project = sample_project();
    project.media_silence_ranges = vec![MediaSilenceRange {
        media_id: "media-1".to_string(),
        source_in: 1.0,
        source_out: 2.5,
        confidence: 0.93,
        label: "quiet speech-free section".to_string(),
    }];
    project.timeline.tracks[0].items[0]
        .properties
        .insert("linkGroupId".to_string(), json!("link-item-1"));
    project.timeline.tracks[4].items.push(TimelineItem {
        id: "item-1-audio".to_string(),
        kind: TimelineItemKind::AudioClip,
        start_seconds: 0.0,
        duration_seconds: 4.0,
        source: TimelineSource::Media {
            media_id: "media-1".to_string(),
        },
        label: "Opening clip audio".to_string(),
        properties: BTreeMap::from([
            ("linkGroupId".to_string(), json!("link-item-1")),
            ("sourceClipType".to_string(), json!("audio")),
        ]),
    });

    let result = call_codex_local_tool(&project, "remove_silence", json!({}))
        .expect("Palmier remove_silence should use stored dead-air analysis without transcript");

    assert!(result.mutates_project);
    assert_eq!(result.payload["valid"], json!(true));
    assert_eq!(result.payload["sectionsRemoved"], json!(1));
    assert_eq!(
        result.payload["affectedTrackIds"],
        json!(["track-audio", "track-video"])
    );
    assert_eq!(
        result.payload["ranges"][0],
        json!({
            "startSeconds": 1.12,
            "endSeconds": 2.38,
            "trackIds": ["track-audio", "track-video"]
        })
    );
    assert_eq!(
        result.payload["removedSilenceRanges"][0],
        json!({
            "mediaId": "media-1",
            "trackId": "track-video",
            "itemId": "item-1",
            "timelineStartSeconds": 1.12,
            "timelineEndSeconds": 2.38,
            "sourceStartSeconds": 1.12,
            "sourceEndSeconds": 2.38,
            "confidence": 0.93,
            "label": "quiet speech-free section"
        })
    );
}

#[test]
fn codex_remove_silence_refuses_when_no_dead_air_is_available() {
    let project = sample_project();

    let error = call_codex_local_tool(&project, "remove_silence", json!({}))
        .expect_err("remove_silence should refuse without analysis or transcript gaps");

    assert!(error.to_string().contains("No dead air on the timeline"));
}

#[test]
fn codex_ripple_delete_ranges_accepts_direct_ranges() {
    let project = sample_project();

    let result = call_codex_local_tool(
        &project,
        "video_creater.ripple_delete_ranges",
        json!({
            "ranges": [
                {
                    "startSeconds": 1.0,
                    "endSeconds": 1.5,
                    "trackIds": ["track-video"]
                }
            ]
        }),
    )
    .expect("direct ripple delete ranges args should validate");

    assert!(result.mutates_project);
    assert_eq!(result.payload["valid"], json!(true));
    assert_eq!(result.payload["actionCount"], json!(1));
    assert_eq!(result.payload["projectAfter"]["timelineItems"], json!(2));
    assert_eq!(result.payload["affectedTrackIds"], json!(["track-video"]));

    let project_dir = tempfile::tempdir().expect("ripple project dir");
    save_split_project(project_dir.path(), &project).expect("save ripple project");
    let persisted = call_codex_local_tool(
        &project,
        "video_creater.ripple_delete_ranges",
        json!({
            "projectDir": project_dir.path().display().to_string(),
            "ranges": [
                {
                    "startSeconds": 1.0,
                    "endSeconds": 1.5,
                    "trackIds": ["track-video"]
                }
            ]
        }),
    )
    .expect("direct ripple delete ranges args should persist");
    assert!(persisted.mutates_project);
    assert_eq!(persisted.payload["applied"], json!(true));
    assert_eq!(persisted.payload["agentHistory"]["entryCount"], json!(1));
    let persisted_project = load_split_project(project_dir.path()).expect("load ripple project");
    let persisted_item_count = persisted_project
        .timeline
        .tracks
        .iter()
        .flat_map(|track| track.items.iter())
        .count();
    assert_eq!(persisted_item_count, 2);
}

#[test]
fn codex_ripple_delete_ranges_accepts_palmier_track_frame_ranges() {
    let project = sample_project();

    let result = call_codex_local_tool(
        &project,
        "ripple_delete_ranges",
        json!({
            "trackIndex": 0,
            "ranges": [[24, 36]],
            "units": "frames"
        }),
    )
    .expect("Palmier track frame ripple delete args should validate");

    assert!(result.mutates_project);
    assert_eq!(result.payload["valid"], json!(true));
    assert_eq!(result.payload["affectedTrackIds"], json!(["track-video"]));
    assert_eq!(
        result.payload["ranges"][0],
        json!({
            "startSeconds": 1.0,
            "endSeconds": 1.5,
            "trackIds": ["track-video"]
        })
    );
}

#[test]
fn codex_ripple_delete_ranges_rejects_huge_palmier_frame_ranges() {
    let project = sample_project();

    let error = call_codex_local_tool(
        &project,
        "ripple_delete_ranges",
        json!({
            "trackIndex": 0,
            "ranges": [[0.0, 1.0e300]],
            "units": "frames"
        }),
    )
    .expect_err("huge Palmier frame ranges should reject without mutating");

    assert!(error
        .to_string()
        .contains("frame values must be non-negative integers"));
}

#[test]
fn codex_ripple_delete_ranges_merges_overlapping_palmier_ranges() {
    let project = sample_project();

    let result = call_codex_local_tool(
        &project,
        "ripple_delete_ranges",
        json!({
            "trackIndex": 0,
            "ranges": [[24, 48], [36, 60]],
            "units": "frames"
        }),
    )
    .expect("overlapping Palmier ripple ranges should validate");

    assert!(result.mutates_project);
    assert_eq!(result.payload["valid"], json!(true));
    assert_eq!(result.payload["actionCount"], json!(1));
    assert_eq!(result.payload["ranges"].as_array().unwrap().len(), 1);
    assert_eq!(
        result.payload["ranges"][0],
        json!({
            "startSeconds": 1.0,
            "endSeconds": 2.5,
            "trackIds": ["track-video"]
        })
    );
    assert_eq!(result.payload["projectAfter"]["timelineItems"], json!(2));
}

#[test]
fn codex_ripple_delete_ranges_accepts_palmier_clip_second_ranges() {
    let project = sample_project();

    let result = call_codex_local_tool(
        &project,
        "ripple_delete_ranges",
        json!({
            "clipId": "item-1",
            "ranges": [[1.0, 1.5]],
            "units": "seconds"
        }),
    )
    .expect("Palmier clip second ripple delete args should validate");

    assert!(result.mutates_project);
    assert_eq!(result.payload["valid"], json!(true));
    assert_eq!(result.payload["affectedTrackIds"], json!(["track-video"]));
    assert_eq!(
        result.payload["ranges"][0],
        json!({
            "startSeconds": 1.0,
            "endSeconds": 1.5,
            "trackIds": ["track-video"]
        })
    );
}

#[test]
fn codex_ripple_delete_ranges_returns_palmier_resulting_fragments() {
    let project = sample_project();

    let result = call_codex_local_tool(
        &project,
        "ripple_delete_ranges",
        json!({
            "clipId": "item-1",
            "ranges": [[40, 50]]
        }),
    )
    .expect("Palmier ripple delete should report resulting fragments");

    assert!(result.mutates_project);
    assert_eq!(result.payload["valid"], json!(true));
    assert_eq!(result.payload["removedFrames"], json!(10));
    let clips = result.payload["resultingClips"]
        .as_array()
        .expect("resulting clips");
    assert_eq!(clips.len(), 2);
    assert_eq!(clips[0]["clipId"], json!("item-1"));
    assert_eq!(clips[0]["durationFrames"], json!(40));
    assert_eq!(clips[1]["startFrame"], json!(40));
    assert_eq!(clips[1]["durationFrames"], json!(46));
    assert_ne!(clips[1]["clipId"], json!("item-1"));
}

#[test]
fn codex_ripple_delete_ranges_treats_clip_frame_ranges_as_clip_local() {
    let mut project = sample_project();
    project.timeline.tracks[0].items[0].start_seconds = 10.0;
    project.timeline.tracks[0].items[0].duration_seconds = 2.0;
    project.timeline.tracks[0].items[0]
        .properties
        .insert("sourceIn".to_string(), json!(2.0));
    project.timeline.tracks[0].items[0]
        .properties
        .insert("sourceOut".to_string(), json!(4.0));

    let result = call_codex_local_tool(
        &project,
        "ripple_delete_ranges",
        json!({
            "clipId": "item-1",
            "ranges": [[12, 24]],
            "units": "frames"
        }),
    )
    .expect("Palmier clip frame ranges should be clip-local");

    assert!(result.mutates_project);
    assert_eq!(result.payload["valid"], json!(true));
    assert_eq!(result.payload["removedFrames"], json!(12));
    assert_eq!(
        result.payload["ranges"][0],
        json!({
            "startSeconds": 10.5,
            "endSeconds": 11.0,
            "trackIds": ["track-video"]
        })
    );
    let clips = result.payload["resultingClips"]
        .as_array()
        .expect("resulting clips");
    assert_eq!(clips.len(), 2);
    assert_eq!(clips[0]["clipId"], json!("item-1"));
    assert_eq!(clips[0]["startFrame"], json!(240));
    assert_eq!(clips[0]["durationFrames"], json!(12));
    assert_eq!(clips[1]["startFrame"], json!(252));
    assert_eq!(clips[1]["durationFrames"], json!(24));
}

#[test]
fn codex_add_texts_accepts_direct_overlay_entries() {
    let project = sample_project();

    let result = call_codex_local_tool(
        &project,
        "video_creater.add_texts",
        json!({
            "targetTrackId": "track-overlays",
            "texts": [
                {
                    "id": "overlay-direct",
                    "text": "Launch day",
                    "startSeconds": 1.0,
                    "durationSeconds": 2.0,
                    "visualTreatment": "compact upper-left kinetic label with translucent backing and accent stroke",
                    "motion": "quick slide-in, two-beat hold, soft fade-out",
                    "safeZone": "keep inside 10% margins and away from faces or product details",
                    "avoid": "full-width opaque black slabs, centered static title-card layout, default-font look"
                }
            ]
        }),
    )
    .expect("direct text overlay args should validate");

    assert!(result.mutates_project);
    assert_eq!(result.payload["valid"], json!(true));
    assert_eq!(result.payload["actionCount"], json!(1));
    assert_eq!(result.payload["affectedItemIds"], json!(["overlay-direct"]));
    assert_eq!(result.payload["projectAfter"]["timelineItems"], json!(2));

    let project_dir = tempfile::tempdir().expect("text project dir");
    save_split_project(project_dir.path(), &project).expect("save text project");
    let persisted = call_codex_local_tool(
        &project,
        "video_creater.add_texts",
        json!({
            "projectDir": project_dir.path().display().to_string(),
            "targetTrackId": "track-overlays",
            "texts": [
                {
                    "id": "overlay-persisted",
                    "text": "Launch day",
                    "startSeconds": 1.0,
                    "durationSeconds": 2.0,
                    "visualTreatment": "compact upper-left kinetic label with translucent backing and accent stroke",
                    "motion": "quick slide-in, two-beat hold, soft fade-out",
                    "safeZone": "keep inside 10% margins and away from faces or product details",
                    "avoid": "full-width opaque black slabs, centered static title-card layout, default-font look"
                }
            ]
        }),
    )
    .expect("direct text overlay args should persist");
    assert!(persisted.mutates_project);
    assert_eq!(persisted.payload["applied"], json!(true));
    assert_eq!(persisted.payload["agentHistory"]["entryCount"], json!(1));
    let persisted_project = load_split_project(project_dir.path()).expect("load text project");
    assert!(persisted_project
        .timeline
        .tracks
        .iter()
        .flat_map(|track| track.items.iter())
        .any(|item| item.id == "overlay-persisted"));
}

#[test]
fn codex_add_texts_accepts_palmier_frame_entries() {
    let project = sample_project();

    let result = call_codex_local_tool(
        &project,
        "add_texts",
        json!({
            "entries": [
                {
                    "startFrame": 24,
                    "durationFrames": 48,
                    "content": "Launch day",
                    "fontName": "Helvetica-Bold",
                    "fontSize": 72,
                    "color": "#FFFFFF",
                    "alignment": "center",
                    "transform": {
                        "centerX": 0.5,
                        "centerY": 0.82
                    }
                }
            ]
        }),
    )
    .expect("Palmier text entries should validate");

    assert!(result.mutates_project);
    assert_eq!(result.payload["valid"], json!(true));
    assert_eq!(result.payload["affectedItemIds"], json!(["text-1000-0"]));
    assert_eq!(
        result.payload["items"][0],
        json!({
            "itemId": "text-1000-0",
            "kind": "overlay",
            "startSeconds": 1.0,
            "durationSeconds": 2.0,
            "text": "Launch day"
        })
    );
}

#[test]
fn codex_add_texts_rejects_mixed_palmier_track_index_usage() {
    let project = sample_project();

    let error = call_codex_local_tool(
        &project,
        "add_texts",
        json!({
            "entries": [
                {
                    "trackIndex": 1,
                    "startFrame": 0,
                    "durationFrames": 24,
                    "content": "A"
                },
                {
                    "startFrame": 48,
                    "durationFrames": 24,
                    "content": "B"
                }
            ]
        }),
    )
    .expect_err("mixed Palmier trackIndex usage should reject");

    assert!(error.to_string().contains("Mixed trackIndex"));
}

#[test]
fn codex_add_texts_rejects_palmier_audio_target_track() {
    let project = sample_project();

    let error = call_codex_local_tool(
        &project,
        "add_texts",
        json!({
            "entries": [
                {
                    "trackIndex": 4,
                    "startFrame": 0,
                    "durationFrames": 24,
                    "content": "Subtitle"
                }
            ]
        }),
    )
    .expect_err("Palmier text entries should reject audio tracks");

    assert!(error.to_string().to_lowercase().contains("audio"));
}

#[test]
fn codex_add_captions_accepts_direct_caption_entries() {
    let project = sample_project();

    let result = call_codex_local_tool(
        &project,
        "video_creater.add_captions",
        json!({
            "targetTrackId": "track-captions",
            "captions": [
                {
                    "id": "caption-direct",
                    "text": "This is the hook",
                    "startSeconds": 0.4,
                    "durationSeconds": 1.2
                }
            ]
        }),
    )
    .expect("direct caption args should validate");

    assert!(result.mutates_project);
    assert_eq!(result.payload["valid"], json!(true));
    assert_eq!(result.payload["actionCount"], json!(1));
    assert_eq!(result.payload["affectedItemIds"], json!(["caption-direct"]));
    assert_eq!(result.payload["projectAfter"]["timelineItems"], json!(2));

    let project_dir = tempfile::tempdir().expect("caption project dir");
    save_split_project(project_dir.path(), &project).expect("save caption project");
    let persisted = call_codex_local_tool(
        &project,
        "video_creater.add_captions",
        json!({
            "projectDir": project_dir.path().display().to_string(),
            "targetTrackId": "track-captions",
            "captions": [
                {
                    "id": "caption-persisted",
                    "text": "This is the hook",
                    "startSeconds": 0.4,
                    "durationSeconds": 1.2
                }
            ]
        }),
    )
    .expect("direct caption args should persist");
    assert!(persisted.mutates_project);
    assert_eq!(persisted.payload["applied"], json!(true));
    assert_eq!(persisted.payload["agentHistory"]["entryCount"], json!(1));
    let persisted_project = load_split_project(project_dir.path()).expect("load caption project");
    assert!(persisted_project
        .timeline
        .tracks
        .iter()
        .flat_map(|track| track.items.iter())
        .any(|item| item.id == "caption-persisted"));
}

#[test]
fn codex_add_captions_accepts_palmier_clip_caption_request() {
    let mut project = sample_project();
    project.transcripts.push(Transcript {
        id: "transcript-1".to_string(),
        media_id: "media-1".to_string(),
        engine: Some("fixture".to_string()),
        raw_artifact_path: None,
        repairs: Vec::new(),
        segments: Vec::new(),
        words: vec![
            TranscriptWord {
                text: "Launch".to_string(),
                start_seconds: 1.0,
                end_seconds: 1.4,
                confidence: Some(0.99),
                speaker: None,
            },
            TranscriptWord {
                text: "day".to_string(),
                start_seconds: 1.45,
                end_seconds: 1.8,
                confidence: Some(0.98),
                speaker: None,
            },
        ],
    });

    let result = call_codex_local_tool(
        &project,
        "add_captions",
        json!({
            "clipIds": ["item-1"],
            "language": "en",
            "fontName": "Helvetica-Bold",
            "fontSize": 48,
            "color": "#FFFFFF",
            "centerX": 0.5,
            "centerY": 0.9,
            "textCase": "upper",
            "censorProfanity": false
        }),
    )
    .expect("Palmier clip caption request should validate");

    assert!(result.mutates_project);
    assert_eq!(result.payload["valid"], json!(true));
    assert_eq!(
        result.payload["affectedItemIds"],
        json!(["caption-item-1-0", "caption-item-1-1"])
    );
    assert_eq!(
        result.payload["items"][0],
        json!({
            "itemId": "caption-item-1-0",
            "kind": "caption",
            "startSeconds": 1.0,
            "durationSeconds": 0.4,
            "text": "LAUNCH"
        })
    );
    assert_eq!(
        result.payload["items"][1],
        json!({
            "itemId": "caption-item-1-1",
            "kind": "caption",
            "startSeconds": 1.45,
            "durationSeconds": 0.35,
            "text": "DAY"
        })
    );
}

#[test]
fn codex_add_captions_persists_source_word_ranges_for_trimmed_clips() {
    let mut project = sample_project();
    project.timeline.tracks[0].items[0].start_seconds = 10.0;
    project.timeline.tracks[0].items[0].duration_seconds = 1.5;
    project.timeline.tracks[0].items[0]
        .properties
        .insert("sourceIn".to_string(), json!(2.0));
    project.timeline.tracks[0].items[0]
        .properties
        .insert("sourceOut".to_string(), json!(3.5));
    project.transcripts.push(Transcript {
        id: "transcript-1".to_string(),
        media_id: "media-1".to_string(),
        engine: Some("fixture".to_string()),
        raw_artifact_path: None,
        repairs: Vec::new(),
        segments: Vec::new(),
        words: vec![
            TranscriptWord {
                text: "Outside".to_string(),
                start_seconds: 1.2,
                end_seconds: 1.5,
                confidence: Some(0.9),
                speaker: None,
            },
            TranscriptWord {
                text: "Trimmed".to_string(),
                start_seconds: 2.25,
                end_seconds: 2.7,
                confidence: Some(0.99),
                speaker: None,
            },
            TranscriptWord {
                text: "Range".to_string(),
                start_seconds: 2.75,
                end_seconds: 3.2,
                confidence: Some(0.98),
                speaker: None,
            },
        ],
    });
    let project_dir = tempfile::tempdir().expect("caption source project dir");
    save_split_project(project_dir.path(), &project).expect("save caption source project");

    let result = call_codex_local_tool(
        &project,
        "add_captions",
        json!({
            "projectDir": project_dir.path().display().to_string(),
            "clipIds": ["item-1"],
            "textCase": "upper"
        }),
    )
    .expect("Palmier clip caption request should persist source word ranges");

    assert!(result.mutates_project);
    assert_eq!(result.payload["applied"], json!(true));
    assert_eq!(
        result.payload["affectedItemIds"],
        json!(["caption-item-1-1", "caption-item-1-2"])
    );

    let persisted_project =
        load_split_project(project_dir.path()).expect("load caption source project");
    let persisted_captions = persisted_project
        .timeline
        .tracks
        .iter()
        .flat_map(|track| track.items.iter())
        .filter(|item| matches!(item.kind, TimelineItemKind::Caption))
        .map(|item| (item.id.as_str(), item))
        .collect::<BTreeMap<_, _>>();
    let first_caption = persisted_captions["caption-item-1-1"];
    assert_eq!(first_caption.start_seconds, 10.25);
    assert_eq!(first_caption.duration_seconds, 0.45);
    assert_eq!(
        first_caption.properties["transcriptId"],
        json!("transcript-1")
    );
    assert_eq!(first_caption.properties["wordIndex"], json!(1));
    assert_eq!(first_caption.properties["sourceIn"], json!(2.25));
    assert_eq!(first_caption.properties["sourceOut"], json!(2.7));
    let second_caption = persisted_captions["caption-item-1-2"];
    assert_eq!(second_caption.start_seconds, 10.75);
    assert_eq!(second_caption.duration_seconds, 0.45);
    assert_eq!(
        second_caption.properties["transcriptId"],
        json!("transcript-1")
    );
    assert_eq!(second_caption.properties["wordIndex"], json!(2));
    assert_eq!(second_caption.properties["sourceIn"], json!(2.75));
    assert_eq!(second_caption.properties["sourceOut"], json!(3.2));
}

fn project_with_speed_two_opening_clip() -> video_creater_lib::project::model::VideoProject {
    let mut project = sample_project();
    project.timeline.tracks[0].items[0]
        .properties
        .insert("speed".to_string(), json!(2.0));
    project
}

fn transcript_for_media_1(words: &[(&str, f64, f64)]) -> Transcript {
    Transcript {
        id: "transcript-1".to_string(),
        media_id: "media-1".to_string(),
        engine: Some("fixture".to_string()),
        raw_artifact_path: None,
        repairs: Vec::new(),
        segments: Vec::new(),
        words: words
            .iter()
            .map(|(text, start_seconds, end_seconds)| TranscriptWord {
                text: (*text).to_string(),
                start_seconds: *start_seconds,
                end_seconds: *end_seconds,
                confidence: Some(0.9),
                speaker: None,
            })
            .collect(),
    }
}

#[test]
fn codex_remove_silence_maps_stored_dead_air_through_clip_speed() {
    let mut project = project_with_speed_two_opening_clip();
    project.media_silence_ranges = vec![
        MediaSilenceRange {
            media_id: "media-1".to_string(),
            source_in: 1.0,
            source_out: 2.5,
            confidence: 0.93,
            label: "early quiet section".to_string(),
        },
        MediaSilenceRange {
            media_id: "media-1".to_string(),
            source_in: 5.0,
            source_out: 6.5,
            confidence: 0.91,
            label: "late quiet section".to_string(),
        },
    ];

    let result = call_codex_local_tool(&project, "remove_silence", json!({}))
        .expect("remove_silence should map dead air through clip speed");

    assert_eq!(result.payload["sectionsRemoved"], json!(2));
    assert_eq!(
        result.payload["ranges"],
        json!([
            { "startSeconds": 0.56, "endSeconds": 1.19, "trackIds": ["track-video"] },
            { "startSeconds": 2.56, "endSeconds": 3.19, "trackIds": ["track-video"] }
        ])
    );
    assert_eq!(
        result.payload["removedSilenceRanges"][1]["sourceStartSeconds"],
        json!(5.12)
    );
    assert_eq!(
        result.payload["removedSilenceRanges"][1]["sourceEndSeconds"],
        json!(6.38)
    );
}

#[test]
fn codex_ripple_delete_ranges_maps_clip_second_ranges_through_clip_speed() {
    let mut project = project_with_speed_two_opening_clip();
    project.timeline.tracks[0].items[0]
        .properties
        .insert("sourceIn".to_string(), json!(2.0));

    let result = call_codex_local_tool(
        &project,
        "ripple_delete_ranges",
        json!({
            "clipId": "item-1",
            "ranges": [[6.0, 7.0]],
            "units": "seconds"
        }),
    )
    .expect("clip second ranges should map through clip speed");

    assert_eq!(
        result.payload["ranges"][0],
        json!({
            "startSeconds": 2.0,
            "endSeconds": 2.5,
            "trackIds": ["track-video"]
        })
    );
}

#[test]
fn codex_remove_words_maps_media_word_ranges_through_clip_speed() {
    let mut project = project_with_speed_two_opening_clip();
    project.transcripts.push(transcript_for_media_1(&[
        ("early", 1.0, 1.5),
        ("late", 5.0, 6.0),
    ]));

    let result = call_codex_local_tool(
        &project,
        "video_creater.remove_words",
        json!({
            "mediaId": "media-1",
            "wordIndexes": [0, 1]
        }),
    )
    .expect("media word ranges should map through clip speed");

    assert_eq!(
        result.payload["removedWordRanges"][0]["timelineStartSeconds"],
        json!(0.5)
    );
    assert_eq!(
        result.payload["removedWordRanges"][0]["timelineEndSeconds"],
        json!(0.75)
    );
    assert_eq!(
        result.payload["removedWordRanges"][1]["timelineStartSeconds"],
        json!(2.5)
    );
    assert_eq!(
        result.payload["removedWordRanges"][1]["timelineEndSeconds"],
        json!(3.0)
    );
}

#[test]
fn codex_add_captions_maps_clip_words_through_clip_speed() {
    let mut project = project_with_speed_two_opening_clip();
    let item = &mut project.timeline.tracks[0].items[0];
    item.start_seconds = 10.0;
    item.duration_seconds = 1.5;
    item.properties.insert("sourceIn".to_string(), json!(2.0));
    project.transcripts.push(transcript_for_media_1(&[
        ("Inside", 2.5, 3.0),
        ("Tail", 4.0, 4.5),
        ("Outside", 5.5, 6.0),
    ]));

    let result = call_codex_local_tool(
        &project,
        "add_captions",
        json!({ "clipIds": ["item-1"], "textCase": "upper" }),
    )
    .expect("clip captions should map through clip speed");

    assert_eq!(
        result.payload["affectedItemIds"],
        json!(["caption-item-1-0", "caption-item-1-1"])
    );
    assert_eq!(result.payload["items"][0]["startSeconds"], json!(10.25));
    assert_eq!(result.payload["items"][0]["durationSeconds"], json!(0.25));
    assert_eq!(result.payload["items"][1]["startSeconds"], json!(11.0));
    assert_eq!(result.payload["items"][1]["durationSeconds"], json!(0.25));
}

#[test]
fn codex_get_timeline_trim_end_frame_uses_clip_speed_source_span() {
    let project = project_with_speed_two_opening_clip();

    let timeline =
        call_codex_local_tool(&project, "get_timeline", json!({})).expect("timeline payload");

    let clip = &timeline.payload["tracks"][0]["clips"][0];
    assert_eq!(clip["trimStartFrame"], json!(0));
    assert_eq!(clip["trimEndFrame"], json!(96));
}

#[test]
fn codex_inspect_media_clip_source_out_uses_clip_speed() {
    let project = project_with_speed_two_opening_clip();

    let result = call_codex_local_tool(
        &project,
        "inspect_media",
        json!({ "mediaRef": "media-1", "clipId": "item-1" }),
    )
    .expect("clip-scoped inspect_media should validate");

    assert_eq!(result.payload["clip"]["sourceIn"], json!(0.0));
    assert_eq!(result.payload["clip"]["sourceOut"], json!(8.0));
}

#[test]
fn codex_generate_video_source_clip_source_out_uses_clip_speed() {
    let project = project_with_speed_two_opening_clip();

    let result = call_codex_local_tool(
        &project,
        "video_creater.generate_video",
        json!({
            "projectDir": "/tmp/video-creater-project",
            "assetId": "agent-video-source-clip-speed",
            "jobId": "job-agent-video-source-clip-speed",
            "mockMode": true,
            "prompt": "turn this clip into a warm product reveal",
            "model": "fal.ai:fal-ai/wan/v2.2-a14b/video-to-video",
            "sourceClipId": "item-1"
        }),
    )
    .expect("source-clip video edit should validate");

    let settings = &result.payload["startRequest"]["input"]["settings"];
    assert_eq!(settings["sourceIn"], json!(0.0));
    assert_eq!(settings["sourceOut"], json!(8.0));
    assert_eq!(settings["videoSourceEndSeconds"], json!(8.0));
}

#[test]
fn codex_upscale_media_source_clip_range_uses_clip_speed() {
    let project = project_with_speed_two_opening_clip();

    let result = call_codex_local_tool(
        &project,
        "video_creater.upscale_media",
        json!({
            "projectDir": "/tmp/video-creater-project",
            "assetId": "agent-upscale-speed",
            "jobId": "job-agent-upscale-speed",
            "mockMode": true,
            "mediaRef": "media-1",
            "sourceClipId": "item-1",
            "model": "mock-upscale-v1"
        }),
    )
    .expect("source-clip upscale should validate");

    assert_eq!(
        result.payload["startRequest"]["input"]["settings"]["sourceOut"],
        json!(8.0)
    );
    assert_eq!(
        result.payload["projectActions"][0]["asset"]["settings"]["durationSeconds"],
        json!(8.0)
    );
}

#[test]
fn codex_inspect_timeline_preview_source_time_uses_clip_speed() {
    let mut project = project_with_speed_two_opening_clip();
    project.timeline.tracks[0].items[0]
        .properties
        .insert("sourceIn".to_string(), json!(1.0));

    let result = call_codex_local_tool(
        &project,
        "video_creater.inspect_timeline",
        json!({ "startSeconds": 1.25, "endSeconds": 1.5 }),
    )
    .expect("inspect timeline payload");

    assert_eq!(
        result.payload["previewLayers"][0]["itemId"],
        json!("item-1")
    );
    assert_eq!(
        result.payload["previewLayers"][0]["sourceTimeSeconds"],
        json!(3.5)
    );
}

#[test]
fn codex_ripple_delete_ranges_expands_palmier_link_group_tracks() {
    let mut project = sample_project();
    project.timeline.tracks[0].items[0]
        .properties
        .insert("linkGroupId".to_string(), json!("link-item-1"));
    project.timeline.tracks[4].items.push(TimelineItem {
        id: "item-1-audio".to_string(),
        kind: TimelineItemKind::AudioClip,
        start_seconds: 0.0,
        duration_seconds: 4.0,
        source: TimelineSource::Media {
            media_id: "media-1".to_string(),
        },
        label: "Opening clip audio".to_string(),
        properties: BTreeMap::from([
            ("linkGroupId".to_string(), json!("link-item-1")),
            ("sourceClipType".to_string(), json!("audio")),
        ]),
    });

    let result = call_codex_local_tool(
        &project,
        "ripple_delete_ranges",
        json!({
            "clipId": "item-1",
            "ranges": [[1.0, 1.5]],
            "units": "seconds"
        }),
    )
    .expect("linked Palmier ripple_delete_ranges should validate");

    assert!(result.mutates_project);
    assert_eq!(result.payload["valid"], json!(true));
    assert_eq!(
        result.payload["affectedTrackIds"],
        json!(["track-audio", "track-video"])
    );
    assert_eq!(
        result.payload["ranges"][0],
        json!({
            "startSeconds": 1.0,
            "endSeconds": 1.5,
            "trackIds": ["track-audio", "track-video"]
        })
    );
    assert_eq!(result.payload["projectAfter"]["timelineItems"], json!(4));
}

#[test]
fn codex_set_keyframes_accepts_direct_opacity_keyframes() {
    let project = sample_project();

    let result = call_codex_local_tool(
        &project,
        "video_creater.set_keyframes",
        json!({
            "itemId": "item-1",
            "property": "opacity",
            "keyframes": [
                { "atSeconds": 0.0, "value": 0.0, "easing": "linear" },
                { "atSeconds": 1.0, "value": 1.0, "easing": "easeOut" }
            ]
        }),
    )
    .expect("direct keyframe args should validate");

    assert!(result.mutates_project);
    assert_eq!(result.payload["valid"], json!(true));
    assert_eq!(result.payload["actionCount"], json!(1));
    assert_eq!(result.payload["affectedItemIds"], json!(["item-1"]));

    let project_dir = tempfile::tempdir().expect("keyframes project dir");
    save_split_project(project_dir.path(), &project).expect("save keyframes project");
    let persisted = call_codex_local_tool(
        &project,
        "video_creater.set_keyframes",
        json!({
            "projectDir": project_dir.path().display().to_string(),
            "itemId": "item-1",
            "property": "opacity",
            "keyframes": [
                { "atSeconds": 0.0, "value": 0.0, "easing": "linear" },
                { "atSeconds": 1.0, "value": 1.0, "easing": "easeOut" }
            ]
        }),
    )
    .expect("direct keyframe args should persist");
    assert!(persisted.mutates_project);
    assert_eq!(persisted.payload["applied"], json!(true));
    assert_eq!(persisted.payload["agentHistory"]["entryCount"], json!(1));
    let persisted_project = load_split_project(project_dir.path()).expect("load keyframes project");
    let persisted_item = persisted_project
        .timeline
        .tracks
        .iter()
        .flat_map(|track| track.items.iter())
        .find(|item| item.id == "item-1")
        .expect("persisted keyframe item");
    assert_eq!(
        persisted_item.properties["keyframes"]["opacity"][1]["value"],
        json!(1.0)
    );
}

#[test]
fn codex_set_keyframes_accepts_palmier_frame_rows() {
    let project = sample_project();

    let result = call_codex_local_tool(
        &project,
        "set_keyframes",
        json!({
            "clipId": "item-1",
            "property": "opacity",
            "keyframes": [
                [0, 0.0, "linear"],
                [24, 1.0, "hold"]
            ]
        }),
    )
    .expect("Palmier keyframe rows should validate");

    assert!(result.mutates_project);
    assert_eq!(result.payload["valid"], json!(true));
    assert_eq!(result.payload["actionCount"], json!(1));
    assert_eq!(result.payload["affectedItemIds"], json!(["item-1"]));
    assert_eq!(
        result.payload["keyframes"],
        json!([
            { "atSeconds": 0.0, "value": 0.0, "easing": "linear" },
            { "atSeconds": 1.0, "value": 1.0, "easing": "hold" }
        ])
    );
}

#[test]
fn codex_set_keyframes_sorts_and_dedupes_palmier_frame_rows() {
    let project = sample_project();

    let result = call_codex_local_tool(
        &project,
        "set_keyframes",
        json!({
            "clipId": "item-1",
            "property": "opacity",
            "keyframes": [
                [48, 0.3],
                [0, 1.0],
                [24, 0.5],
                [24, 0.8, "linear"]
            ]
        }),
    )
    .expect("Palmier keyframe rows should sort and dedupe");

    assert!(result.mutates_project);
    assert_eq!(result.payload["valid"], json!(true));
    assert_eq!(
        result.payload["keyframes"],
        json!([
            { "atSeconds": 0.0, "value": 1.0 },
            { "atSeconds": 1.0, "value": 0.8, "easing": "linear" },
            { "atSeconds": 2.0, "value": 0.3 }
        ])
    );
}

#[test]
fn codex_set_keyframes_rejects_huge_palmier_frame_rows() {
    let project = sample_project();

    let error = call_codex_local_tool(
        &project,
        "set_keyframes",
        json!({
            "clipId": "item-1",
            "property": "opacity",
            "keyframes": [[1.0e19, 0.5]]
        }),
    )
    .expect_err("huge Palmier keyframe frames should reject");

    assert!(error
        .to_string()
        .contains("keyframe row frame must be an integer"));
}

#[test]
fn codex_set_keyframes_accepts_empty_rows_to_clear_track() {
    let project = sample_project();

    let result = call_codex_local_tool(
        &project,
        "set_keyframes",
        json!({
            "clipId": "item-1",
            "property": "opacity",
            "keyframes": []
        }),
    )
    .expect("empty Palmier keyframe rows should clear the property track");

    assert!(result.mutates_project);
    assert_eq!(result.payload["valid"], json!(true));
    assert_eq!(result.payload["actionCount"], json!(1));
    assert_eq!(result.payload["affectedItemIds"], json!(["item-1"]));
    assert_eq!(result.payload["keyframes"], json!([]));
}

#[test]
fn codex_set_keyframes_accepts_palmier_position_rows() {
    let project = sample_project();

    let result = call_codex_local_tool(
        &project,
        "set_keyframes",
        json!({
            "clipId": "item-1",
            "property": "position",
            "keyframes": [
                [0, 0.25, 0.35, "linear"],
                [24, 0.5, 0.6, "smooth"]
            ]
        }),
    )
    .expect("Palmier position keyframe rows should validate");

    assert!(result.mutates_project);
    assert_eq!(result.payload["valid"], json!(true));
    assert_eq!(result.payload["actionCount"], json!(2));
    assert_eq!(result.payload["affectedItemIds"], json!(["item-1"]));
    assert_eq!(
        result.payload["keyframes"],
        json!({
            "positionX": [
                { "atSeconds": 0.0, "value": 0.25, "easing": "linear" },
                { "atSeconds": 1.0, "value": 0.5, "easing": "smooth" }
            ],
            "positionY": [
                { "atSeconds": 0.0, "value": 0.35, "easing": "linear" },
                { "atSeconds": 1.0, "value": 0.6, "easing": "smooth" }
            ]
        })
    );
}

#[test]
fn codex_set_keyframes_accepts_palmier_scale_rows() {
    let project = sample_project();

    let result = call_codex_local_tool(
        &project,
        "set_keyframes",
        json!({
            "clipId": "item-1",
            "property": "scale",
            "keyframes": [
                [0, 1.0, 1.0],
                [24, 0.75, 0.5, "linear"]
            ]
        }),
    )
    .expect("Palmier scale keyframe rows should validate");

    assert!(result.mutates_project);
    assert_eq!(result.payload["valid"], json!(true));
    assert_eq!(result.payload["actionCount"], json!(2));
    assert_eq!(
        result.payload["keyframes"],
        json!({
            "scaleX": [
                { "atSeconds": 0.0, "value": 1.0 },
                { "atSeconds": 1.0, "value": 0.75, "easing": "linear" }
            ],
            "scaleY": [
                { "atSeconds": 0.0, "value": 1.0 },
                { "atSeconds": 1.0, "value": 0.5, "easing": "linear" }
            ]
        })
    );
}

#[test]
fn codex_set_keyframes_clears_palmier_scale_axis_rows() {
    let project = sample_project();

    let result = call_codex_local_tool(
        &project,
        "set_keyframes",
        json!({
            "clipId": "item-1",
            "property": "scale",
            "keyframes": []
        }),
    )
    .expect("empty Palmier scale keyframe rows should clear axis tracks");

    assert!(result.mutates_project);
    assert_eq!(result.payload["valid"], json!(true));
    assert_eq!(result.payload["actionCount"], json!(2));
    assert_eq!(
        result.payload["keyframes"],
        json!({
            "scaleX": [],
            "scaleY": []
        })
    );
}

#[test]
fn codex_set_keyframes_accepts_palmier_crop_rows() {
    let project = sample_project();

    let result = call_codex_local_tool(
        &project,
        "set_keyframes",
        json!({
            "clipId": "item-1",
            "property": "crop",
            "keyframes": [
                [0, 0.0, 0.1, 0.2, 0.3],
                [24, 0.05, 0.15, 0.25, 0.35, "hold"]
            ]
        }),
    )
    .expect("Palmier crop keyframe rows should validate");

    assert!(result.mutates_project);
    assert_eq!(result.payload["valid"], json!(true));
    assert_eq!(result.payload["actionCount"], json!(4));
    assert_eq!(
        result.payload["keyframes"],
        json!({
            "cropTop": [
                { "atSeconds": 0.0, "value": 0.0 },
                { "atSeconds": 1.0, "value": 0.05, "easing": "hold" }
            ],
            "cropRight": [
                { "atSeconds": 0.0, "value": 0.1 },
                { "atSeconds": 1.0, "value": 0.15, "easing": "hold" }
            ],
            "cropBottom": [
                { "atSeconds": 0.0, "value": 0.2 },
                { "atSeconds": 1.0, "value": 0.25, "easing": "hold" }
            ],
            "cropLeft": [
                { "atSeconds": 0.0, "value": 0.3 },
                { "atSeconds": 1.0, "value": 0.35, "easing": "hold" }
            ]
        })
    );
}

#[test]
fn codex_get_timeline_emits_palmier_tuple_keyframes() {
    let mut project = sample_project();
    project.timeline.tracks[0].items[0].properties.insert(
        "keyframes".to_string(),
        json!({
            "opacity": [
                { "atSeconds": 0.0, "value": 1.0 },
                { "atSeconds": 1.0, "value": 0.0, "easing": "linear" }
            ],
            "positionX": [
                { "atSeconds": 0.0, "value": 0.5 },
                { "atSeconds": 1.0, "value": 0.7 }
            ],
            "positionY": [
                { "atSeconds": 0.0, "value": 0.25 },
                { "atSeconds": 1.0, "value": 0.3 }
            ],
            "scaleX": [
                { "atSeconds": 0.0, "value": 1.0 },
                { "atSeconds": 1.0, "value": 0.75 }
            ],
            "scaleY": [
                { "atSeconds": 0.0, "value": 1.0 },
                { "atSeconds": 1.0, "value": 0.5 }
            ],
            "cropTop": [
                { "atSeconds": 0.0, "value": 0.0 },
                { "atSeconds": 1.0, "value": 0.05 }
            ],
            "cropRight": [
                { "atSeconds": 0.0, "value": 0.1 },
                { "atSeconds": 1.0, "value": 0.15 }
            ],
            "cropBottom": [
                { "atSeconds": 0.0, "value": 0.2 },
                { "atSeconds": 1.0, "value": 0.25 }
            ],
            "cropLeft": [
                { "atSeconds": 0.0, "value": 0.3 },
                { "atSeconds": 1.0, "value": 0.35, "easing": "hold" }
            ]
        }),
    );

    let timeline =
        call_codex_local_tool(&project, "get_timeline", json!({})).expect("timeline payload");

    let clip = &timeline.payload["tracks"][0]["clips"][0];
    assert_eq!(
        clip["keyframes"]["opacity"],
        json!([[0, 1.0], [24, 0.0, "linear"]])
    );
    assert_eq!(
        clip["keyframes"]["position"],
        json!([[0, 0.5, 0.25], [24, 0.7, 0.3]])
    );
    assert_eq!(
        clip["keyframes"]["scale"],
        json!([[0, 1.0, 1.0], [24, 0.75, 0.5]])
    );
    assert_eq!(
        clip["keyframes"]["crop"],
        json!([
            [0, 0.0, 0.1, 0.2, 0.3],
            [24, 0.05, 0.15, 0.25, 0.35, "hold"]
        ])
    );
    assert!(clip["volumeTrack"].is_null());
    assert!(clip["positionTrack"].is_null());
}

#[test]
fn codex_set_keyframes_accepts_direct_visual_transform_keyframes() {
    let project = sample_project();

    let result = call_codex_local_tool(
        &project,
        "video_creater.set_keyframes",
        json!({
            "itemId": "item-1",
            "property": "rotationDegrees",
            "keyframes": [
                { "atSeconds": 0.0, "value": -6.0, "easing": "easeOut" },
                { "atSeconds": 1.0, "value": 0.0, "easing": "linear" }
            ]
        }),
    )
    .expect("direct transform keyframe args should validate");

    assert!(result.mutates_project);
    assert_eq!(result.payload["valid"], json!(true));
    assert_eq!(result.payload["actionCount"], json!(1));
    assert_eq!(result.payload["affectedItemIds"], json!(["item-1"]));
}

#[test]
fn mcp_tools_call_returns_content_and_structured_content() {
    let project = sample_project();
    let response = handle_mcp_request(
        &project,
        json!({
            "jsonrpc": "2.0",
            "id": 2,
            "method": "tools/call",
            "params": {
                "name": "video_creater.project_context",
                "arguments": {}
            }
        }),
    );

    assert_eq!(response["id"], json!(2));
    assert_eq!(response["result"]["isError"], json!(false));
    assert_eq!(response["result"]["content"][0]["type"], json!("text"));
    assert!(response["result"]["content"][0]["text"]
        .as_str()
        .expect("text")
        .contains("video_creater.project_context"));
    assert_eq!(
        response["result"]["structuredContent"]["project"]["id"],
        json!("project-test")
    );
}

#[test]
fn scoped_mcp_rejects_cross_root_mutation_and_allows_launch_root() {
    let launch = tempfile::tempdir().expect("launch project");
    let other = tempfile::tempdir().expect("other project");
    let project = sample_project();
    save_split_project(launch.path(), &project).expect("save launch project");
    save_split_project(other.path(), &project).expect("save other project");
    let tool_request = |id: u64, name: &str, project_dir: &std::path::Path| {
        json!({
            "jsonrpc":"2.0", "id":id, "method":"tools/call", "params":{
                "name":name, "arguments":{
                    "projectDir":project_dir.display().to_string(),
                    "actions":[{"type":"moveItems","moves":[{
                        "itemId":"item-1", "targetTrackId":"track-video", "startSeconds":1.25
                    }]}]
                }
            }
        })
    };

    for (offset, name) in [
        "video_creater.apply_project_actions",
        "apply_project_actions",
        "video_creater.undo_agent_edit",
        "undo_agent_edit",
        "undo",
    ]
    .into_iter()
    .enumerate()
    {
        let denied = handle_scoped_mcp_request(
            &project,
            launch.path(),
            tool_request(20 + offset as u64, name, other.path()),
        );
        assert_eq!(denied["result"]["isError"], json!(true), "{name}");
        assert!(
            denied["result"]["structuredContent"]["error"]
                .as_str()
                .unwrap()
                .contains("launch project root"),
            "{name}"
        );
        assert_eq!(
            load_split_project(other.path()).unwrap().timeline.tracks[0].items[0].start_seconds,
            0.0,
            "{name}"
        );
    }

    let allowed = handle_scoped_mcp_request(
        &project,
        launch.path(),
        tool_request(30, "video_creater.apply_project_actions", launch.path()),
    );
    assert_eq!(allowed["result"]["isError"], json!(false));
    assert_eq!(
        load_split_project(launch.path()).unwrap().timeline.tracks[0].items[0].start_seconds,
        1.25
    );
}

#[test]
fn mcp_unknown_method_returns_json_rpc_error() {
    let project = sample_project();
    let response = handle_mcp_request(
        &project,
        json!({
            "jsonrpc": "2.0",
            "id": 3,
            "method": "missing/method"
        }),
    );

    assert_eq!(response["error"]["code"], json!(-32601));
    assert!(response["error"]["message"]
        .as_str()
        .expect("message")
        .contains("unknown MCP method"));
}

#[test]
fn mcp_tool_error_is_returned_as_tool_result_error() {
    let project = sample_project();
    let response = handle_mcp_request(
        &project,
        json!({
            "jsonrpc": "2.0",
            "id": 4,
            "method": "tools/call",
            "params": {
                "name": "video_creater.missing",
                "arguments": {}
            }
        }),
    );

    assert_eq!(response["result"]["isError"], json!(true));
    assert!(response["result"]["structuredContent"]["error"]
        .as_str()
        .expect("error")
        .contains("unknown Codex local tool"));
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
