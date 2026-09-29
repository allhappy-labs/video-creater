use crate::gpu_graphics::profile::supported_profile_ids;
use serde_json::{json, Value};

pub(crate) fn gpu_visual_schema() -> Value {
    json!({
        "type": "object",
        "required": [
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
        ],
        "additionalProperties": false,
        "properties": {
            "id": { "type": "string", "minLength": 1 },
            "kind": { "type": "string", "enum": ["hybrid_scene", "shader_background"] },
            "startSeconds": { "type": "number", "minimum": 0 },
            "durationSeconds": { "type": "number", "minimum": 0.001 },
            "qualityProfile": {
                "type": "string",
                "enum": supported_profile_ids()
            },
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
    })
}

pub(crate) fn graphics_node_schema() -> Value {
    json!({
        "type": "object",
        "required": [
            "type",
            "id",
            "text",
            "box",
            "fontSize",
            "fontWeight",
            "align",
            "fill",
            "maxLines",
            "textReveal",
            "radius",
            "points",
            "stroke",
            "strokeWidth",
            "assetId",
            "fit",
            "opacity",
            "animate"
        ],
        "additionalProperties": false,
        "properties": {
            "type": {
                "type": "string",
                "enum": ["text", "roundedRect", "rect", "polygon", "line", "imageRef"]
            },
            "id": { "type": "string", "minLength": 1 },
            "text": { "type": ["string", "null"] },
            "box": { "anyOf": [graphics_rect_schema(), { "type": "null" }] },
            "fontSize": { "type": ["number", "null"] },
            "fontWeight": { "type": ["integer", "null"] },
            "align": {
                "anyOf": [
                    { "type": "string", "enum": ["left", "center", "right", "start", "end"] },
                    { "type": "null" }
                ]
            },
            "fill": { "type": ["string", "null"] },
            "maxLines": { "type": ["integer", "null"], "minimum": 1 },
            "textReveal": { "anyOf": [text_reveal_schema(), { "type": "null" }] },
            "radius": { "type": ["number", "null"], "minimum": 0 },
            "points": {
                "anyOf": [
                    {
                        "type": "array",
                        "items": graphics_point_schema()
                    },
                    { "type": "null" }
                ]
            },
            "stroke": { "type": ["string", "null"] },
            "strokeWidth": { "type": ["number", "null"], "exclusiveMinimum": 0 },
            "assetId": { "type": ["string", "null"] },
            "fit": {
                "anyOf": [
                    { "type": "string", "enum": ["contain", "cover", "stretch", "none"] },
                    { "type": "null" }
                ]
            },
            "opacity": { "type": ["number", "null"], "minimum": 0, "maximum": 1 },
            "animate": { "anyOf": [animation_schema(), { "type": "null" }] }
        }
    })
}

pub(crate) fn animation_schema() -> Value {
    json!({
        "type": "object",
        "required": ["ease", "delaySeconds", "repeat", "yoyo", "origin", "keyframes"],
        "additionalProperties": false,
        "properties": {
            "ease": {
                "anyOf": [
                    {
                        "type": "string",
                        "enum": [
                            "linear",
                            "inQuad",
                            "outQuad",
                            "inOutQuad",
                            "inCubic",
                            "outCubic",
                            "inOutCubic",
                            "outBack",
                            "outElastic"
                        ]
                    },
                    { "type": "null" }
                ]
            },
            "delaySeconds": { "type": ["number", "null"], "minimum": 0 },
            "repeat": { "type": ["integer", "null"], "minimum": 0 },
            "yoyo": { "type": ["boolean", "null"] },
            "origin": {
                "anyOf": [
                    {
                        "type": "object",
                        "required": ["x", "y"],
                        "additionalProperties": false,
                        "properties": {
                            "x": {
                                "anyOf": [
                                    { "type": "string", "enum": ["left", "center", "right"] },
                                    { "type": "number" }
                                ]
                            },
                            "y": {
                                "anyOf": [
                                    { "type": "string", "enum": ["top", "center", "bottom"] },
                                    { "type": "number" }
                                ]
                            }
                        }
                    },
                    { "type": "null" }
                ]
            },
            "keyframes": {
                "type": "array",
                "minItems": 1,
                "items": {
                    "type": "object",
                    "additionalProperties": false,
                    "required": [
                        "at",
                        "x",
                        "y",
                        "scale",
                        "scaleX",
                        "scaleY",
                        "opacity",
                        "rotationDegrees",
                        "blurRadius",
                        "shadowOpacity",
                        "glowOpacity",
                        "clipProgress",
                        "pathProgress"
                    ],
                    "properties": {
                        "at": { "type": "number", "minimum": 0, "maximum": 1 },
                        "x": {
                            "type": ["number", "null"],
                            "description": "Translation offset from the node's base box or points."
                        },
                        "y": {
                            "type": ["number", "null"],
                            "description": "Translation offset from the node's base box or points."
                        },
                        "scale": { "type": ["number", "null"], "exclusiveMinimum": 0 },
                        "scaleX": { "type": ["number", "null"], "exclusiveMinimum": 0 },
                        "scaleY": { "type": ["number", "null"], "exclusiveMinimum": 0 },
                        "opacity": { "type": ["number", "null"], "minimum": 0, "maximum": 1 },
                        "rotationDegrees": { "type": ["number", "null"], "minimum": -45, "maximum": 45 },
                        "blurRadius": { "type": ["number", "null"], "minimum": 0, "maximum": 64 },
                        "shadowOpacity": { "type": ["number", "null"], "minimum": 0, "maximum": 1 },
                        "glowOpacity": { "type": ["number", "null"], "minimum": 0, "maximum": 1 },
                        "clipProgress": { "type": ["number", "null"], "minimum": 0, "maximum": 1 },
                        "pathProgress": { "type": ["number", "null"], "minimum": 0, "maximum": 1 }
                    }
                }
            }
        }
    })
}

pub(crate) fn text_reveal_schema() -> Value {
    json!({
        "type": "object",
        "required": ["mode", "staggerSeconds", "order", "maxRevealDurationSeconds"],
        "additionalProperties": false,
        "properties": {
            "mode": { "type": "string", "enum": ["whole", "line", "word", "character"] },
            "staggerSeconds": { "type": ["number", "null"], "minimum": 0 },
            "order": {
                "anyOf": [
                    { "type": "string", "enum": ["forward", "reverse", "centerOut"] },
                    { "type": "null" }
                ]
            },
            "maxRevealDurationSeconds": { "type": ["number", "null"], "exclusiveMinimum": 0 }
        }
    })
}

pub(crate) fn graphics_rect_schema() -> Value {
    json!({
        "type": "object",
        "additionalProperties": false,
        "required": ["x", "y", "width", "height"],
        "properties": {
            "x": { "type": "number" },
            "y": { "type": "number" },
            "width": { "type": "number", "exclusiveMinimum": 0 },
            "height": { "type": "number", "exclusiveMinimum": 0 }
        }
    })
}

pub(crate) fn graphics_point_schema() -> Value {
    json!({
        "type": "object",
        "additionalProperties": false,
        "required": ["x", "y"],
        "properties": {
            "x": { "type": "number" },
            "y": { "type": "number" }
        }
    })
}
