use super::*;
use crate::workflows::{VIDEO_CREATER_TEMPORAL_TASK_QUEUE, VIDEO_CREATER_TEMPORAL_WORKFLOW_KINDS};
use serde_json::{json, Value};

pub(crate) fn temporal_workflow_metadata_schema() -> Value {
    json!({
        "type": "object",
        "required": ["workflowId", "workflowType", "taskQueue", "runId", "activityTypes"],
        "additionalProperties": false,
        "properties": {
            "workflowId": { "type": "string", "minLength": 1 },
            "workflowType": temporal_workflow_type_schema(),
            "taskQueue": temporal_task_queue_schema(),
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
    })
}

pub(crate) fn temporal_workflow_start_request_schema() -> Value {
    json!({
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
            "workflowType": temporal_workflow_type_schema(),
            "taskQueue": temporal_task_queue_schema(),
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
    })
}

pub(crate) fn temporal_workflow_type_schema() -> Value {
    let workflow_types: Vec<&'static str> = VIDEO_CREATER_TEMPORAL_WORKFLOW_KINDS
        .iter()
        .map(|kind| kind.workflow_type())
        .collect();
    json!({
        "type": "string",
        "enum": workflow_types
    })
}

pub(crate) fn temporal_job_kind_schema() -> Value {
    let job_kinds: Vec<&'static str> = VIDEO_CREATER_TEMPORAL_WORKFLOW_KINDS
        .iter()
        .map(|kind| kind.job_kind())
        .collect();
    json!({
        "type": "string",
        "enum": job_kinds
    })
}

pub(crate) fn temporal_task_queue_schema() -> Value {
    json!({
        "type": "string",
        "enum": [VIDEO_CREATER_TEMPORAL_TASK_QUEUE]
    })
}

pub(crate) fn record_job_action_schema() -> Value {
    json!({
        "type": "object",
        "required": ["type", "job"],
        "additionalProperties": false,
        "properties": {
            "type": {
                "type": "string",
                "enum": ["recordJob"]
            },
            "job": {
                "type": "object",
                "required": ["id", "kind", "status", "updatedAt", "workflow", "startRequest"],
                "additionalProperties": false,
                "properties": {
                    "id": { "type": "string", "minLength": 1 },
                    "kind": temporal_job_kind_schema(),
                    "status": {
                        "type": "string",
                        "enum": ["queued", "running", "progress", "blocked", "failed", "cancelled", "completed"]
                    },
                    "updatedAt": { "type": "string", "minLength": 1 },
                    "workflow": {
                        "anyOf": [
                            temporal_workflow_metadata_schema(),
                            { "type": "null" }
                        ]
                    },
                    "startRequest": {
                        "anyOf": [
                            temporal_workflow_start_request_schema(),
                            { "type": "null" }
                        ]
                    }
                }
            }
        }
    })
}

pub(crate) fn update_job_status_action_schema() -> Value {
    json!({
        "type": "object",
        "required": ["type", "jobId", "status", "updatedAt", "runId"],
        "additionalProperties": false,
        "properties": {
            "type": {
                "type": "string",
                "enum": ["updateJobStatus"]
            },
            "jobId": { "type": "string", "minLength": 1 },
            "status": {
                "type": "string",
                "enum": ["queued", "running", "progress", "blocked", "failed", "cancelled", "completed"]
            },
            "updatedAt": { "type": "string", "minLength": 1 },
            "runId": nullable_non_empty_string_schema()
        }
    })
}

pub(crate) fn render_report_check_status_schema() -> Value {
    json!({
        "type": "string",
        "enum": ["passed", "failed", "skipped"]
    })
}

pub(crate) fn render_report_streams_schema() -> Value {
    json!({
        "type": "object",
        "required": ["video", "audio"],
        "additionalProperties": false,
        "properties": {
            "video": { "type": "boolean" },
            "audio": { "type": "boolean" }
        }
    })
}

pub(crate) fn render_report_checks_schema() -> Value {
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
            "duration": render_report_check_status_schema(),
            "captionAlignment": render_report_check_status_schema(),
            "overlayTiming": render_report_check_status_schema(),
            "visualFrameEvidence": render_report_check_status_schema(),
            "artifactPaths": render_report_check_status_schema()
        }
    })
}

pub(crate) fn attach_render_report_action_schema() -> Value {
    json!({
        "type": "object",
        "required": ["type", "report"],
        "additionalProperties": false,
        "properties": {
            "type": {
                "type": "string",
                "enum": ["attachRenderReport"]
            },
            "report": {
                "type": "object",
                "required": [
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
                ],
                "additionalProperties": false,
                "properties": {
                    "schemaVersion": { "type": "integer", "minimum": 1 },
                    "id": { "type": "string", "minLength": 1 },
                    "status": {
                        "type": "string",
                        "enum": ["queued", "running", "failed", "completed"]
                    },
                    "outputPath": { "type": "string", "minLength": 1 },
                    "durationSeconds": { "type": "number", "exclusiveMinimum": 0 },
                    "streams": render_report_streams_schema(),
                    "checks": render_report_checks_schema(),
                    "artifacts": {
                        "type": "array",
                        "minItems": 1,
                        "items": { "type": "string", "minLength": 1 }
                    },
                    "logPath": { "type": "string", "minLength": 1 },
                    "createdAt": { "type": "string", "minLength": 1 }
                }
            }
        }
    })
}

pub(crate) fn record_export_artifact_action_schema() -> Value {
    json!({
        "type": "object",
        "required": ["type", "artifact"],
        "additionalProperties": false,
        "properties": {
            "type": {
                "type": "string",
                "enum": ["recordExportArtifact"]
            },
            "artifact": {
                "type": "object",
                "required": [
                    "schemaVersion",
                    "id",
                    "kind",
                    "format",
                    "path",
                    "mimeType",
                    "jobId",
                    "createdAt"
                ],
                "additionalProperties": false,
                "properties": {
                    "schemaVersion": { "type": "integer", "minimum": 1 },
                    "id": { "type": "string", "minLength": 1 },
                    "kind": {
                        "type": "string",
                        "enum": ["nle_xml", "webm", "mp4", "mov"]
                    },
                    "format": { "type": "string", "minLength": 1 },
                    "path": { "type": "string", "minLength": 1 },
                    "mimeType": { "type": "string", "minLength": 1 },
                    "jobId": {
                        "anyOf": [
                            { "type": "string", "minLength": 1 },
                            { "type": "null" }
                        ]
                    },
                    "createdAt": { "type": "string", "minLength": 1 }
                }
            }
        }
    })
}
