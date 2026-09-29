//! The conversation/edit proposal output schema.
//!
//! Provider-neutral JSON Schema, moved verbatim out of `codex/app_server.rs` so that every
//! agent transport can hand the same schema to its backend without depending on the Codex
//! app-server code.

mod actions;
mod assets;
mod graphics;
mod jobs;
mod media;
mod text;
mod timeline;
mod transitions;
mod visual;

pub use actions::project_action_schema;
pub(crate) use assets::*;
pub(crate) use graphics::*;
pub(crate) use jobs::*;
pub(crate) use media::*;
pub(crate) use text::*;
pub(crate) use timeline::*;
pub(crate) use transitions::*;
pub(crate) use visual::*;

use serde_json::{json, Value};

pub fn codex_conversation_proposal_output_schema() -> Value {
    json!({
        "type": "object",
        "required": ["summary", "edl", "projectActions", "renderReview"],
        "additionalProperties": false,
        "properties": {
            "summary": { "type": "string", "minLength": 1 },
            "edl": {
                "type": "array",
                "items": {
                    "type": "object",
                    "required": ["mediaId", "sourceIn", "sourceOut", "reason"],
                    "additionalProperties": false,
                    "properties": {
                        "mediaId": { "type": "string", "minLength": 1 },
                        "sourceIn": { "type": "number", "minimum": 0 },
                        "sourceOut": { "type": "number", "exclusiveMinimum": 0 },
                        "reason": { "type": "string", "minLength": 1 }
                    }
                }
            },
            "projectActions": {
                "type": "array",
                "items": project_action_schema()
            },
            "renderReview": {
                "anyOf": [
                    {
                        "type": "object",
                        "required": [
                            "durationSeconds",
                            "streamCheckRequired",
                            "captionAlignmentRequired",
                            "overlayTimingRequired",
                            "visualFrameEvidenceRequired",
                            "artifactPathsRequired",
                            "logReferenceRequired"
                        ],
                        "additionalProperties": false,
                        "properties": {
                            "durationSeconds": { "type": "number", "minimum": 0.001 },
                            "streamCheckRequired": { "type": "boolean" },
                            "captionAlignmentRequired": { "type": "boolean" },
                            "overlayTimingRequired": { "type": "boolean" },
                            "visualFrameEvidenceRequired": { "type": "boolean" },
                            "artifactPathsRequired": { "type": "boolean" },
                            "logReferenceRequired": { "type": "boolean" }
                        }
                    },
                    { "type": "null" }
                ]
            }
        }
    })
}

pub fn codex_edit_proposal_output_schema() -> Value {
    let graphics_node_schema = graphics_node_schema();
    let gpu_visual_schema = gpu_visual_schema();
    json!({
        "type": "object",
        "required": ["mediaId", "clips", "captions", "overlays", "hyperframes", "gpuVisuals", "projectActions", "renderReview"],
        "additionalProperties": false,
        "properties": {
            "mediaId": { "type": "string", "minLength": 1 },
            "clips": {
                "type": "array",
                "minItems": 1,
                "items": {
                    "type": "object",
                    "required": ["mediaId", "sourceIn", "sourceOut", "reason"],
                    "additionalProperties": false,
                    "properties": {
                        "mediaId": { "type": "string", "minLength": 1 },
                        "sourceIn": { "type": "number", "minimum": 0 },
                        "sourceOut": { "type": "number", "exclusiveMinimum": 0 },
                        "reason": { "type": "string", "minLength": 1 }
                    }
                }
            },
            "captions": {
                "type": "array",
                "items": {
                    "type": "object",
                    "required": [
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
                    ],
                    "additionalProperties": false,
                    "properties": {
                        "text": { "type": "string", "minLength": 1 },
                        "startSeconds": { "type": "number", "minimum": 0 },
                        "durationSeconds": { "type": "number", "minimum": 0.001 },
                        "sourceBeat": { "type": "string", "minLength": 1 },
                        "dimensions": {
                            "type": "object",
                            "required": ["width", "height"],
                            "additionalProperties": false,
                            "properties": {
                                "width": { "type": "integer", "minimum": 1 },
                                "height": { "type": "integer", "minimum": 1 }
                            }
                        },
                        "frameRate": { "type": "number", "exclusiveMinimum": 0 },
                        "alpha": { "type": "boolean" },
                        "visualTreatment": { "type": "string", "minLength": 1 },
                        "motion": { "type": "string", "minLength": 1 },
                        "safeZone": { "type": "string", "minLength": 1 },
                        "avoid": { "type": "string", "minLength": 1 }
                    }
                }
            },
            "overlays": {
                "type": "array",
                "items": {
                    "type": "object",
                    "required": [
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
                    ],
                    "additionalProperties": false,
                    "properties": {
                        "kind": {
                            "type": "string",
                            "enum": ["overlay", "lower_third", "title_card", "diagram", "transition"]
                        },
                        "templateId": { "type": ["string", "null"] },
                        "fields": {
                            "type": "object",
                            "required": ["headline", "subline", "logoAssetId"],
                            "additionalProperties": false,
                            "properties": {
                                "headline": { "type": ["string", "null"] },
                                "subline": { "type": ["string", "null"] },
                                "logoAssetId": { "type": ["string", "null"] }
                            }
                        },
                        "motionPresetId": {
                            "anyOf": [
                                {
                                    "type": "string",
                                    "enum": [
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
                                    ]
                                },
                                { "type": "null" }
                            ]
                        },
                        "nodes": {
                            "anyOf": [
                                {
                                    "type": "array",
                                    "minItems": 1,
                                    "items": graphics_node_schema
                                },
                                { "type": "null" }
                            ]
                        },
                        "sourceBeat": { "type": "string", "minLength": 1 },
                        "startSeconds": { "type": "number", "minimum": 0 },
                        "durationSeconds": { "type": "number", "minimum": 0.001 },
                        "dimensions": {
                            "type": "object",
                            "required": ["width", "height"],
                            "additionalProperties": false,
                            "properties": {
                                "width": { "type": "integer", "minimum": 1 },
                                "height": { "type": "integer", "minimum": 1 }
                            }
                        },
                        "frameRate": { "type": "number", "exclusiveMinimum": 0 },
                        "alpha": { "type": "boolean" },
                        "brief": { "type": "string", "minLength": 1 },
                        "visualTreatment": { "type": "string", "minLength": 1 },
                        "motion": { "type": "string", "minLength": 1 },
                        "safeZone": { "type": "string", "minLength": 1 },
                        "avoid": { "type": "string", "minLength": 1 }
                    }
                }
            },
            "hyperframes": {
                "type": "array",
                "items": {
                    "type": "object",
                    "required": [
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
                    ],
                    "additionalProperties": false,
                    "properties": {
                        "kind": {
                            "type": "string",
                            "enum": [
                                "template_overlay",
                                "title_card",
                                "lower_third",
                                "diagram",
                                "transition",
                                "immersive_scene"
                            ]
                        },
                        "templateId": { "type": "string", "minLength": 1 },
                        "fields": {
                            "type": "object",
                            "required": ["headline", "subline", "logoAssetId"],
                            "additionalProperties": false,
                            "properties": {
                                "headline": { "type": ["string", "null"] },
                                "subline": { "type": ["string", "null"] },
                                "logoAssetId": { "type": ["string", "null"] }
                            }
                        },
                        "motionPresetId": { "type": ["string", "null"], "minLength": 1 },
                        "sourceBeat": { "type": "string", "minLength": 1 },
                        "startSeconds": { "type": "number", "minimum": 0 },
                        "durationSeconds": { "type": "number", "minimum": 0.001 },
                        "dimensions": {
                            "type": "object",
                            "required": ["width", "height"],
                            "additionalProperties": false,
                            "properties": {
                                "width": { "type": "integer", "minimum": 1 },
                                "height": { "type": "integer", "minimum": 1 }
                            }
                        },
                        "frameRate": { "type": "number", "exclusiveMinimum": 0 },
                        "alpha": { "type": "boolean" },
                        "brief": { "type": "string", "minLength": 1 },
                        "visualTreatment": { "type": "string", "minLength": 1 },
                        "motion": { "type": "string", "minLength": 1 },
                        "safeZone": { "type": "string", "minLength": 1 },
                        "avoid": { "type": "string", "minLength": 1 }
                    }
                }
            },
            "gpuVisuals": {
                "type": "array",
                "items": gpu_visual_schema
            },
            "projectActions": {
                "type": "array",
                "items": project_action_schema()
            },
            "renderReview": {
                "type": "object",
                "required": [
                    "durationSeconds",
                    "streamCheckRequired",
                    "captionAlignmentRequired",
                    "overlayTimingRequired",
                    "visualFrameEvidenceRequired",
                    "artifactPathsRequired",
                    "logReferenceRequired"
                ],
                "additionalProperties": false,
                "properties": {
                    "durationSeconds": { "type": "number", "minimum": 0.001 },
                    "streamCheckRequired": { "type": "boolean" },
                    "captionAlignmentRequired": { "type": "boolean" },
                    "overlayTimingRequired": { "type": "boolean" },
                    "visualFrameEvidenceRequired": { "type": "boolean" },
                    "artifactPathsRequired": { "type": "boolean" },
                    "logReferenceRequired": { "type": "boolean" }
                }
            }
        }
    })
}
