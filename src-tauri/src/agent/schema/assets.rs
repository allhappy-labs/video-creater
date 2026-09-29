use serde_json::{json, Value};

pub(crate) fn update_generated_asset_status_action_schema() -> Value {
    json!({
        "type": "object",
        "required": ["type", "assetId", "status"],
        "additionalProperties": false,
        "properties": {
            "type": {
                "type": "string",
                "enum": ["updateGeneratedAssetStatus"]
            },
            "assetId": { "type": "string", "minLength": 1 },
            "status": {
                "type": "string",
                "enum": ["queued", "running", "failed", "completed"]
            }
        }
    })
}

pub(crate) fn update_generated_asset_references_action_schema() -> Value {
    json!({
        "type": "object",
        "required": ["type", "assetId", "references"],
        "additionalProperties": false,
        "properties": {
            "type": {
                "type": "string",
                "enum": ["updateGeneratedAssetReferences"]
            },
            "assetId": { "type": "string", "minLength": 1 },
            "references": generated_asset_references_schema()
        }
    })
}

pub(crate) fn nullable_non_empty_string_schema() -> Value {
    json!({
        "anyOf": [
            { "type": "string", "minLength": 1 },
            { "type": "null" }
        ]
    })
}

pub(crate) fn generation_model_schema() -> Value {
    json!({
        "type": "object",
        "required": ["provider", "id"],
        "additionalProperties": false,
        "properties": {
            "provider": { "type": "string", "minLength": 1 },
            "id": { "type": "string", "minLength": 1 }
        }
    })
}

pub(crate) fn generated_asset_references_schema() -> Value {
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
            "sourceVideoMediaRef": nullable_non_empty_string_schema(),
            "firstFrameMediaId": nullable_non_empty_string_schema(),
            "lastFrameMediaId": nullable_non_empty_string_schema(),
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
}

pub(crate) fn nullable_positive_integer_schema() -> Value {
    json!({
        "anyOf": [
            { "type": "integer", "minimum": 1 },
            { "type": "null" }
        ]
    })
}

pub(crate) fn nullable_positive_number_schema() -> Value {
    json!({
        "anyOf": [
            { "type": "number", "exclusiveMinimum": 0 },
            { "type": "null" }
        ]
    })
}

pub(crate) fn nullable_non_negative_number_schema() -> Value {
    json!({
        "anyOf": [
            { "type": "number", "minimum": 0 },
            { "type": "null" }
        ]
    })
}

pub(crate) fn generated_asset_settings_schema() -> Value {
    json!({
        "type": "object",
        "required": ["width", "height", "durationSeconds", "fps", "aspectRatio"],
        "additionalProperties": false,
        "properties": {
            "width": nullable_positive_integer_schema(),
            "height": nullable_positive_integer_schema(),
            "durationSeconds": nullable_positive_number_schema(),
            "fps": nullable_positive_number_schema(),
            "aspectRatio": nullable_non_empty_string_schema()
        }
    })
}

pub(crate) fn generated_asset_output_schema() -> Value {
    json!({
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
    })
}

pub(crate) fn record_generated_asset_action_schema() -> Value {
    json!({
        "type": "object",
        "required": ["type", "asset"],
        "additionalProperties": false,
        "properties": {
            "type": {
                "type": "string",
                "enum": ["recordGeneratedAsset"]
            },
            "asset": {
                "type": "object",
                "required": [
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
                ],
                "additionalProperties": false,
                "properties": {
                    "id": { "type": "string", "minLength": 1 },
                    "kind": {
                        "type": "string",
                        "enum": ["generated"]
                    },
                    "status": {
                        "type": "string",
                        "enum": ["queued", "running", "failed", "completed"]
                    },
                    "name": nullable_non_empty_string_schema(),
                    "targetFolderId": nullable_non_empty_string_schema(),
                    "placementIntent": {
                        "anyOf": [
                            { "type": "string", "enum": ["library", "timeline"] },
                            { "type": "null" }
                        ]
                    },
                    "prompt": { "type": "string" },
                    "model": generation_model_schema(),
                    "references": generated_asset_references_schema(),
                    "settings": generated_asset_settings_schema(),
                    "outputs": {
                        "type": "array",
                        "items": generated_asset_output_schema()
                    },
                    "createdAt": { "type": "string", "minLength": 1 },
                    "parentAssetId": nullable_non_empty_string_schema(),
                    "retryOfAssetId": nullable_non_empty_string_schema()
                }
            }
        }
    })
}

pub(crate) fn replace_generated_output_schema() -> Value {
    json!({
        "type": "object",
        "required": ["itemId", "mediaId"],
        "additionalProperties": false,
        "properties": {
            "itemId": { "type": "string", "minLength": 1 },
            "mediaId": { "type": "string", "minLength": 1 }
        }
    })
}

pub(crate) fn complete_generated_asset_action_schema() -> Value {
    json!({
        "type": "object",
        "required": ["type", "assetId", "outputs", "replacement"],
        "additionalProperties": false,
        "properties": {
            "type": {
                "type": "string",
                "enum": ["completeGeneratedAsset"]
            },
            "assetId": { "type": "string", "minLength": 1 },
            "outputs": {
                "type": "array",
                "minItems": 1,
                "items": generated_asset_output_schema()
            },
            "replacement": {
                "anyOf": [
                    replace_generated_output_schema(),
                    { "type": "null" }
                ]
            }
        }
    })
}

pub(crate) fn replace_timeline_item_with_generated_output_action_schema() -> Value {
    json!({
        "type": "object",
        "required": ["type", "replacement"],
        "additionalProperties": false,
        "properties": {
            "type": {
                "type": "string",
                "enum": ["replaceTimelineItemWithGeneratedOutput"]
            },
            "replacement": replace_generated_output_schema()
        }
    })
}
