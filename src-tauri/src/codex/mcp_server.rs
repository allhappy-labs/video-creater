use super::tools::{call_codex_local_tool, list_codex_local_tools, resolve_codex_tool_name};
use crate::project::model::VideoProject;
use crate::project::split::load_split_project;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::io::{BufRead, Write};
use std::path::Path;
use thiserror::Error;

pub const MCP_PROTOCOL_VERSION: &str = "2025-11-25";
pub const MCP_SERVER_NAME: &str = "video-creater";
const MCP_SERVER_INSTRUCTIONS: &str = r#"You are connected to Video Creater, an AI-native video editor. Help the user build and edit their project by calling the tools this server exposes.

# Generation
- Call list_models before generate_video, generate_image, generate_audio, or upscale_media so the model you pick supports the duration, aspect ratio, references, voice, lyrics, or asset type you need.
- Costs real money and is not undoable when live providers are used. Propose the prompt, model, duration, and aspect ratio, then wait for confirmation before calling generation tools.
- Default flow: images first, then video. Iterate on stills until the user approves the look, then pass the approved image as the video's startFrameMediaRef.
- Generation tools return a placeholder asset ID and start request immediately; the generated asset resolves asynchronously and becomes usable once the workflow completes.
- Reuse references for character/location/style consistency: referenceMediaRefs on images; on videos, startFrameMediaRef, endFrameMediaRef, referenceImageMediaRefs, referenceVideoMediaRefs, and referenceAudioMediaRefs according to list_models.
- Video models cannot render readable text reliably. For on-screen text, bake it into a still with generate_image and use that as a startFrameMediaRef, or use editor text tools for true overlays.

# Audio generation
- TTS prompts are the exact text to speak. Use list_models for supported voices and style instructions.
- Music prompts describe style, mood, and genre. Some models accept lyrics with [Verse]/[Chorus] section tags or instrumental mode.
- Video-to-audio models need either a timeline span or a video asset source; prompt text is then an optional style guide.
"#;

const MODEL_RESOURCE_URIS: [(&str, &str, &str); 3] = [
    (
        "video-creater://models/video",
        "Video Models",
        "Available AI video generation models and their capabilities",
    ),
    (
        "video-creater://models/image",
        "Image Models",
        "Available AI image generation models and their capabilities",
    ),
    (
        "video-creater://models/audio",
        "Audio Models",
        "Available AI audio generation models and their capabilities",
    ),
];

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct McpInitializeInfo {
    pub protocol_version: String,
    pub server_name: String,
    pub server_version: String,
}

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum McpInitializeResponseError {
    #[error("MCP initialize response is invalid: {0}")]
    Invalid(String),
    #[error("MCP protocol mismatch: expected {expected}, received {actual}")]
    ProtocolMismatch {
        expected: &'static str,
        actual: String,
    },
}

pub fn build_mcp_initialize_request(id: u64) -> Value {
    json!({
        "jsonrpc": "2.0",
        "id": id,
        "method": "initialize",
        "params": {
            "protocolVersion": MCP_PROTOCOL_VERSION,
            "clientInfo": {
                "name": "video-creater-settings",
                "version": env!("CARGO_PKG_VERSION")
            },
            "capabilities": {}
        }
    })
}

pub fn decode_mcp_initialize_response(
    expected_id: u64,
    response: Value,
) -> Result<McpInitializeInfo, McpInitializeResponseError> {
    if response.get("jsonrpc").and_then(Value::as_str) != Some("2.0") {
        return Err(McpInitializeResponseError::Invalid(
            "jsonrpc must be 2.0".to_string(),
        ));
    }
    if response.get("id").and_then(Value::as_u64) != Some(expected_id) {
        return Err(McpInitializeResponseError::Invalid(format!(
            "response id must be {expected_id}"
        )));
    }
    let result = response
        .get("result")
        .and_then(Value::as_object)
        .ok_or_else(|| {
            McpInitializeResponseError::Invalid("result must be an object".to_string())
        })?;
    let protocol_version = result
        .get("protocolVersion")
        .and_then(Value::as_str)
        .ok_or_else(|| {
            McpInitializeResponseError::Invalid(
                "result.protocolVersion must be a string".to_string(),
            )
        })?;
    if protocol_version != MCP_PROTOCOL_VERSION {
        return Err(McpInitializeResponseError::ProtocolMismatch {
            expected: MCP_PROTOCOL_VERSION,
            actual: protocol_version.to_string(),
        });
    }
    let server_info = result
        .get("serverInfo")
        .and_then(Value::as_object)
        .ok_or_else(|| {
            McpInitializeResponseError::Invalid("result.serverInfo must be an object".to_string())
        })?;
    let server_name = server_info
        .get("name")
        .and_then(Value::as_str)
        .ok_or_else(|| {
            McpInitializeResponseError::Invalid(
                "result.serverInfo.name must be a string".to_string(),
            )
        })?;
    if server_name != MCP_SERVER_NAME {
        return Err(McpInitializeResponseError::Invalid(format!(
            "result.serverInfo.name must be {MCP_SERVER_NAME}"
        )));
    }
    let server_version = server_info
        .get("version")
        .and_then(Value::as_str)
        .filter(|version| !version.is_empty())
        .ok_or_else(|| {
            McpInitializeResponseError::Invalid(
                "result.serverInfo.version must be a non-empty string".to_string(),
            )
        })?;
    let capabilities = result
        .get("capabilities")
        .and_then(Value::as_object)
        .ok_or_else(|| {
            McpInitializeResponseError::Invalid("result.capabilities must be an object".to_string())
        })?;
    for capability in ["resources", "tools"] {
        if capabilities
            .get(capability)
            .and_then(Value::as_object)
            .is_none()
        {
            return Err(McpInitializeResponseError::Invalid(format!(
                "result.capabilities.{capability} must be an object"
            )));
        }
    }

    Ok(McpInitializeInfo {
        protocol_version: protocol_version.to_string(),
        server_name: server_name.to_string(),
        server_version: server_version.to_string(),
    })
}

pub fn handle_mcp_request(project: &VideoProject, request: Value) -> Value {
    handle_mcp_request_with_root(project, None, request)
}

pub fn handle_scoped_mcp_request(
    project: &VideoProject,
    project_dir: &Path,
    request: Value,
) -> Value {
    handle_mcp_request_with_root(project, Some(project_dir), request)
}

fn handle_mcp_request_with_root(
    project: &VideoProject,
    project_dir: Option<&Path>,
    request: Value,
) -> Value {
    let id = request.get("id").cloned().unwrap_or(Value::Null);
    let Some(method) = request.get("method").and_then(Value::as_str) else {
        return json_rpc_error(id, -32600, "invalid JSON-RPC request: missing method");
    };

    match method {
        "initialize" => json_rpc_result(
            id,
            json!({
                "protocolVersion": MCP_PROTOCOL_VERSION,
                "serverInfo": {
                    "name": MCP_SERVER_NAME,
                    "version": env!("CARGO_PKG_VERSION")
                },
                "capabilities": {
                    "resources": {
                        "listChanged": false
                    },
                    "tools": {
                        "listChanged": false
                    }
                },
                "instructions": MCP_SERVER_INSTRUCTIONS
            }),
        ),
        "resources/list" => json_rpc_result(id, generation_model_resources_payload()),
        "resources/read" => handle_resources_read(project, id, request.get("params").cloned()),
        "tools/list" => json_rpc_result(
            id,
            json!({
                "tools": list_codex_local_tools()
                    .into_iter()
                    .map(|tool| {
                        json!({
                            "name": tool.name,
                            "title": tool.title,
                            "description": tool.description,
                            "inputSchema": tool.input_schema,
                            "annotations": {
                                "category": tool.category
                            }
                        })
                    })
                    .collect::<Vec<_>>()
            }),
        ),
        "tools/call" => handle_tools_call(project, project_dir, id, request.get("params").cloned()),
        "notifications/initialized" => json_rpc_result(id, json!({})),
        _ => json_rpc_error(id, -32601, format!("unknown MCP method: {method}")),
    }
}

fn generation_model_resources_payload() -> Value {
    json!({
        "resources": MODEL_RESOURCE_URIS
            .iter()
            .map(|(uri, name, description)| {
                json!({
                    "uri": uri,
                    "name": name,
                    "description": description,
                    "mimeType": "application/json"
                })
            })
            .collect::<Vec<_>>()
    })
}

fn handle_resources_read(project: &VideoProject, id: Value, params: Option<Value>) -> Value {
    let Some(params) = params.and_then(|value| value.as_object().cloned()) else {
        return json_rpc_error(id, -32602, "resources/read params must be an object");
    };
    let Some(uri) = params.get("uri").and_then(Value::as_str) else {
        return json_rpc_error(id, -32602, "resources/read params.uri must be a string");
    };
    let model_type = match uri {
        "video-creater://models/video" => "video",
        "video-creater://models/image" => "image",
        "video-creater://models/audio" => "audio",
        _ => {
            return json_rpc_result(
                id,
                json!({
                    "contents": [
                        {
                            "uri": uri,
                            "mimeType": "text/plain",
                            "text": format!("Unknown resource: {uri}")
                        }
                    ]
                }),
            )
        }
    };

    match call_codex_local_tool(
        project,
        "video_creater.list_models",
        json!({ "type": model_type }),
    ) {
        Ok(result) => {
            let models = result
                .payload
                .get("generationModels")
                .cloned()
                .unwrap_or_else(|| json!([]));
            let text = serde_json::to_string_pretty(&models).unwrap_or_else(|_| "[]".to_string());
            json_rpc_result(
                id,
                json!({
                    "contents": [
                        {
                            "uri": uri,
                            "mimeType": "application/json",
                            "text": text
                        }
                    ]
                }),
            )
        }
        Err(error) => json_rpc_error(id, -32603, error.to_string()),
    }
}

pub fn run_mcp_stdio<R, W>(project_dir: &Path, reader: R, mut writer: W) -> Result<(), String>
where
    R: BufRead,
    W: Write,
{
    for line in reader.lines() {
        let line = line.map_err(|error| error.to_string())?;
        if line.trim().is_empty() {
            continue;
        }
        let request = serde_json::from_str::<Value>(&line)
            .map_err(|error| format!("invalid JSON-RPC request: {error}"))?;
        let project = load_split_project(project_dir).map_err(|error| error.to_string())?;
        let response = handle_scoped_mcp_request(&project, project_dir, request);
        if response != Value::Null {
            let response_line =
                serde_json::to_string(&response).map_err(|error| error.to_string())?;
            writer
                .write_all(response_line.as_bytes())
                .map_err(|error| error.to_string())?;
            writer.write_all(b"\n").map_err(|error| error.to_string())?;
            writer.flush().map_err(|error| error.to_string())?;
        }
    }
    Ok(())
}

fn handle_tools_call(
    project: &VideoProject,
    project_dir: Option<&Path>,
    id: Value,
    params: Option<Value>,
) -> Value {
    let Some(params) = params.and_then(|value| value.as_object().cloned()) else {
        return json_rpc_error(id, -32602, "tools/call params must be an object");
    };
    let Some(name) = params.get("name").and_then(Value::as_str) else {
        return json_rpc_error(id, -32602, "tools/call params.name must be a string");
    };
    let Some(canonical_name) = resolve_codex_tool_name(name) else {
        return tool_error_result(id, format!("unknown Codex local tool: {name}"));
    };
    let mut arguments = params
        .get("arguments")
        .cloned()
        .unwrap_or_else(|| json!({}));

    if let Some(project_dir) = project_dir {
        let descriptor = list_codex_local_tools()
            .into_iter()
            .find(|tool| tool.name == canonical_name);
        let accepts_project_dir = descriptor.as_ref().is_some_and(|tool| {
            tool.input_schema
                .pointer("/properties/projectDir")
                .is_some()
        });
        if accepts_project_dir {
            let canonical_root = match project_dir.canonicalize() {
                Ok(root) => root,
                Err(error) => {
                    return tool_error_result(
                        id,
                        format!("failed to resolve MCP launch project root: {error}"),
                    )
                }
            };
            if let Some(requested) = arguments.get("projectDir").and_then(Value::as_str) {
                let requested_root = match Path::new(requested).canonicalize() {
                    Ok(root) => root,
                    Err(error) => {
                        return tool_error_result(
                            id,
                            format!("failed to resolve requested projectDir: {error}"),
                        )
                    }
                };
                if requested_root != canonical_root {
                    return tool_error_result(
                        id,
                        "projectDir must match the MCP server launch project root",
                    );
                }
            }
            let Some(object) = arguments.as_object_mut() else {
                return tool_error_result(id, "tools/call arguments must be an object");
            };
            object.insert(
                "projectDir".to_string(),
                json!(canonical_root.display().to_string()),
            );
        }
    }

    match call_codex_local_tool(project, &canonical_name, arguments) {
        Ok(result) => {
            let payload = result.payload;
            json_rpc_result(
                id,
                json!({
                    "content": [
                        {
                            "type": "text",
                            "text": format!("{} completed", result.tool_name)
                        }
                    ],
                    "structuredContent": payload,
                    "isError": false
                }),
            )
        }
        Err(error) => json_rpc_result(
            id,
            json!({
                "content": [
                    {
                        "type": "text",
                        "text": error.to_string()
                    }
                ],
                "structuredContent": {
                    "error": error.to_string()
                },
                "isError": true
            }),
        ),
    }
}

fn tool_error_result(id: Value, message: impl Into<String>) -> Value {
    let message = message.into();
    json_rpc_result(
        id,
        json!({
            "content": [{"type":"text", "text":message}],
            "structuredContent":{"error":message},
            "isError":true
        }),
    )
}

fn json_rpc_result(id: Value, result: Value) -> Value {
    json!({
        "jsonrpc": "2.0",
        "id": id,
        "result": result
    })
}

fn json_rpc_error(id: Value, code: i64, message: impl Into<String>) -> Value {
    json!({
        "jsonrpc": "2.0",
        "id": id,
        "error": {
            "code": code,
            "message": message.into()
        }
    })
}
